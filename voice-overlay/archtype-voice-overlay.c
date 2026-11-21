//! 语音浮条（Wayland / layer-shell 版）：和 GNOME 扩展的 voice.js 画同一条——
//! 蓝色话筒（外圈跟着音量呼吸）、「正在听… 离线」、24 根跟着真实音量跳的竖条，
//! 光标右边再画 5 根蓝色竖条，像光标裂成了声波。
//!
//! 数据来自 fcitx5 里的 ArchType 插件：它发现 BUS_NAME 被注册，就不再在候选框里写文字提示，
//! 改为每 50 毫秒发一次 State 信号（状态、文字、音量、光标矩形）。每画好一个状态回一个 Ack；
//! 一次说话里等不到回执，插件就退回候选框里的文字提示（#170），所以没有回执会退化成方块。
//!
//! 构建：pkg-config gtk4 gtk4-layer-shell-0
//! 用：archtype-voice-overlay（常驻；退出后插件自动退回候选框提示）

#include <math.h>
#include <stdio.h>
#include <string.h>

#include <gtk/gtk.h>
#include <gtk4-layer-shell.h>

#define BUS_NAME "io.github.gowayhui.ArchType.VoiceOverlay"
#define OBJECT_PATH "/io/github/gowayhui/ArchType"
#define INTERFACE "io.github.gowayhui.ArchType.Voice1"

enum {
    ST_HIDE = 0,
    ST_LISTENING = 1,
    ST_RECOGNIZING = 2,
    ST_DONE = 3,
    ST_ERROR = 4,
};

#define BAR_COUNT 24
#define BAR_W 3.0
#define BAR_PITCH 5.5
#define BAR_MIN 4.0
#define BAR_MAX 30.0
#define BARS_H 34.0
#define MIC_SIZE 46.0
#define MIC_R 15.0
#define HALO_R0 16.0
#define HALO_R1 22.5

static const double BLUE[3] = {47.0 / 255.0, 111.0 / 255.0, 237.0 / 255.0};

typedef struct {
    GtkApplication *app;
    GtkWindow *win;

    GtkWidget *mic;
    GtkWidget *label;
    GtkWidget *wave;

    guint32 state;
    char label_text[256];
    double level_smooth;
    double level_target;
    double bars[BAR_COUNT];
    double t;
    int64_t last_frame_us;
    int64_t last_signal_us;

    gboolean has_rect;
    gboolean relative;
    double rect_x, rect_y, rect_w, rect_h;

    double opacity;
    double opacity_target;

    guint tick_id;
    guint hide_id;
    guint tick_count;

    GDBusConnection *bus;
} Overlay;

static double clampd(double v, double lo, double hi) {
    return v < lo ? lo : (v > hi ? hi : v);
}

static void rounded_rect(cairo_t *cr, double x, double y, double w, double h, double r) {
    r = fmin(r, fmin(w / 2, h / 2));
    cairo_new_sub_path(cr);
    cairo_arc(cr, x + w - r, y + r, r, -M_PI / 2, 0);
    cairo_arc(cr, x + w - r, y + h - r, r, 0, M_PI / 2);
    cairo_arc(cr, x + r, y + h - r, r, M_PI / 2, M_PI);
    cairo_arc(cr, x + r, y + r, r, M_PI, 1.5 * M_PI);
    cairo_close_path(cr);
}

/// 插件给的音量 0–100（-60 dB 到 -10 dB），安静房间大概 20 以下。
/// 说话时整体在 0.5–1 之间，乘个增益让波形像宣传片里那样站得起来。
static double level_to_voice(int level) {
    double v = clampd((level - 20.0) / 60.0, 0, 1);
    return clampd(1.6 * pow(v, 0.9), 0, 1);
}

static void draw_mic(GtkDrawingArea *area, cairo_t *cr, int width, int height, gpointer data) {
    (void)area;
    (void)width;
    Overlay *o = data;
    double k = height / MIC_SIZE;
    cairo_scale(cr, k, k);
    double cx = MIC_SIZE / 2, cy = MIC_SIZE / 2;

    cairo_set_source_rgba(cr, BLUE[0], BLUE[1], BLUE[2],
                          0.16);
    cairo_arc(cr, cx, cy, HALO_R0 + (HALO_R1 - HALO_R0) * o->level_smooth, 0, 2 * M_PI);
    cairo_fill(cr);

    cairo_set_source_rgb(cr, BLUE[0], BLUE[1], BLUE[2]);
    cairo_arc(cr, cx, cy, MIC_R, 0, 2 * M_PI);
    cairo_fill(cr);

    // 白色话筒：头、托、柄
    cairo_set_source_rgb(cr, 1, 1, 1);
    rounded_rect(cr, cx - 3.2, cy - 8.2, 6.4, 10.8, 3.2);
    cairo_fill(cr);
    cairo_set_line_width(cr, 1.6);
    cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
    cairo_arc(cr, cx, cy - 1.4, 5.4, 0.15 * M_PI, 0.85 * M_PI);
    cairo_stroke(cr);
    cairo_move_to(cr, cx, cy + 4);
    cairo_line_to(cr, cx, cy + 7.2);
    cairo_stroke(cr);
}

static void draw_wave(GtkDrawingArea *area, cairo_t *cr, int width, int height, gpointer data) {
    (void)area;
    (void)width;
    Overlay *o = data;
    double k = height / BARS_H;
    cairo_scale(cr, k, k);
    double env = o->state == ST_LISTENING ? o->level_smooth : 0;
    cairo_set_source_rgba(cr, BLUE[0], BLUE[1], BLUE[2], 0.55 + 0.45 * env);
    for (int i = 0; i < BAR_COUNT; i++) {
        double bh = BAR_MIN + (BAR_MAX - BAR_MIN) * clampd(o->bars[i], 0, 1);
        rounded_rect(cr, i * BAR_PITCH, (BARS_H - bh) / 2, BAR_W, bh, BAR_W / 2);
    }
    cairo_fill(cr);
}

/// 第 i 根竖条该有多高（0–1）：真实音量乘上一道从左往右走的起伏，安静时全部矮下去。
static double bar_target(Overlay *o, int i) {
    double t = o->t;
    if (o->state == ST_RECOGNIZING) {
        double v = sin(t * 7 - i * 0.45);
        return 0.12 * (v > 0 ? v : 0);
    }
    if (o->state != ST_LISTENING) {
        return 0;
    }
    double syl = 0.5 + 0.5 * sin(2 * M_PI * 3.1 * t - i * 0.35 + 0.8 * sin(2 * M_PI * 0.8 * t));
    double v = 0.3 + 0.7 * fabs(sin(2 * M_PI * (1.7 + 0.23 * i) * t + i * 2.1));
    return o->level_smooth * (0.25 + 0.75 * syl) * v;
}

static gboolean tick_cb(gpointer data) {
    Overlay *o = data;
    int64_t now = g_get_monotonic_time();
    double dt = o->last_frame_us ? fmin(0.1, (now - o->last_frame_us) / 1e6) : 1.0 / 60;
    o->last_frame_us = now;
    o->t += dt;

    // 插件没了（fcitx5 退出、崩溃）就自己收起
    if (o->state != ST_HIDE && o->state != ST_DONE && o->last_signal_us &&
        now - o->last_signal_us > 1500 * 1000) {
        o->state = ST_HIDE;
        o->opacity_target = 0;
    }

    double rate = o->level_target > o->level_smooth ? 18 : 5;
    o->level_smooth += (o->level_target - o->level_smooth) * (1 - exp(-rate * dt));
    for (int i = 0; i < BAR_COUNT; i++) {
        o->bars[i] += (bar_target(o, i) - o->bars[i]) * (1 - exp(-16 * dt));
    }

    o->opacity += (o->opacity_target - o->opacity) * (1 - exp(-14 * dt));
    if (o->state == ST_HIDE && o->opacity < 0.02) {
        o->opacity = 0;
        gtk_widget_set_visible(GTK_WIDGET(o->win), FALSE);
    }
    gtk_widget_set_opacity(GTK_WIDGET(o->win), o->opacity);

    gtk_widget_queue_draw(o->mic);
    gtk_widget_queue_draw(o->wave);
    return G_SOURCE_CONTINUE;
}

static GdkMonitor *monitor_at(GdkDisplay *display, int x, int y) {
    GListModel *mons = gdk_display_get_monitors(display);
    guint n = g_list_model_get_n_items(mons);
    for (guint i = 0; i < n; i++) {
        GdkMonitor *m = g_list_model_get_item(mons, i);
        GdkRectangle geo;
        gdk_monitor_get_geometry(m, &geo);
        if (x >= geo.x && x < geo.x + geo.width && y >= geo.y && y < geo.y + geo.height) {
            return m;
        }
        g_object_unref(m);
    }
    return n ? g_list_model_get_item(mons, 0) : NULL;
}

static void place(Overlay *o) {
    GtkWidget *pill = gtk_window_get_child(o->win);
    if (!pill) {
        return;
    }
    int pw = 0, ph = 0, minw = 0, minh = 0;
    gtk_widget_measure(pill, GTK_ORIENTATION_HORIZONTAL, -1, &minw, &pw, NULL, NULL);
    gtk_widget_measure(pill, GTK_ORIENTATION_VERTICAL, -1, &minh, &ph, NULL, NULL);
    if (pw < minw) pw = minw;
    if (ph < minh) ph = minh;
    gtk_window_set_default_size(o->win, pw, ph);

    GdkDisplay *display = gtk_widget_get_display(GTK_WIDGET(o->win));
    GdkMonitor *monitor = monitor_at(display, o->has_rect ? (int)o->rect_x : 0,
                                     o->has_rect ? (int)o->rect_y : 0);
    GdkRectangle geo;
    gdk_monitor_get_geometry(monitor, &geo);
    gtk_layer_set_monitor(o->win, monitor);

    int x, y;
    if (o->has_rect && !o->relative && o->rect_w > 0 && o->rect_h > 0) {
        x = (int)(o->rect_x - 10);
        y = (int)(o->rect_y + o->rect_h + 10);
        if (y + ph > geo.y + geo.height) {
            y = (int)(o->rect_y - ph - 10);
        }
    } else {
        // 应用没给光标位置：放在屏幕下方正中
        x = geo.x + (geo.width - pw) / 2;
        y = geo.y + geo.height - ph - 96;
    }
    x = (int)clampd(x, geo.x + 8, geo.x + geo.width - pw - 8);
    y = (int)clampd(y, geo.y + 8, geo.y + geo.height - ph - 8);

    gtk_layer_set_margin(o->win, GTK_LAYER_SHELL_EDGE_LEFT, x - geo.x);
    gtk_layer_set_margin(o->win, GTK_LAYER_SHELL_EDGE_TOP, y - geo.y);
}

static void hide_now(Overlay *o) {
    if (o->hide_id) {
        g_source_remove(o->hide_id);
        o->hide_id = 0;
    }
    o->state = ST_HIDE;
    o->opacity_target = 0;
}

static gboolean hide_cb(gpointer data) {
    Overlay *o = data;
    o->hide_id = 0;
    hide_now(o);
    return G_SOURCE_REMOVE;
}

static void on_state(Overlay *o, guint32 state, const char *label, int level,
                     int x, int y, int w, int h, gboolean relative, double scale) {
    (void)scale;
    o->relative = relative;
    o->last_signal_us = g_get_monotonic_time();
    if (state == ST_HIDE) {
        if (o->state != ST_DONE) {
            hide_now(o);
        }
        return;
    }
    if (w > 0 && h > 0) {
        o->has_rect = TRUE;
        o->rect_x = x;
        o->rect_y = y;
        o->rect_w = w;
        o->rect_h = h;
    } else {
        o->has_rect = FALSE;
    }

    if (o->hide_id) {
        g_source_remove(o->hide_id);
        o->hide_id = 0;
    }
    o->state = state;
    g_strlcpy(o->label_text, label ? label : "", sizeof(o->label_text));
    gtk_label_set_text(GTK_LABEL(o->label), o->label_text);
    gtk_widget_remove_css_class(o->label, "error");
    if (state == ST_ERROR) {
        gtk_widget_add_css_class(o->label, "error");
    }

    o->level_target = state == ST_LISTENING ? level_to_voice(level) : 0;

    gtk_widget_set_visible(GTK_WIDGET(o->win), TRUE);
    o->opacity_target = 1;
    place(o);
    if (state == ST_DONE) {
        o->hide_id = g_timeout_add(350, hide_cb, o);
    }
}

static void on_signal(GDBusConnection *conn, const gchar *sender, const gchar *path,
                      const gchar *iface, const gchar *signal, GVariant *params, gpointer data) {

    (void)path;
    (void)iface;
    (void)signal;
    Overlay *o = data;
    guint32 state = ST_HIDE;
    const gchar *label = "";
    gint32 level = 0, x = 0, y = 0, w = 0, h = 0;
    gboolean relative = FALSE;
    gdouble scale = 1.0;
    g_variant_get(params, "(usiiiiibd)", &state, &label, &level, &x, &y, &w, &h, &relative, &scale);
    on_state(o, state, label, level, x, y, w, h, relative, scale);
    if (state != ST_HIDE) {
        GError *err = NULL;
        g_dbus_connection_emit_signal(conn, sender, OBJECT_PATH, INTERFACE, "Ack",
                                      g_variant_new("(u)", state), &err);
        if (err) {
            g_warning("Ack 发不出去：%s", err->message);
            g_error_free(err);
        }
    }
}

static void on_name_acquired(GDBusConnection *conn, const gchar *name, gpointer data) {
    Overlay *o = data;
    o->bus = conn;
    g_dbus_connection_signal_subscribe(conn, NULL, INTERFACE, "State", OBJECT_PATH, NULL,
                                       G_DBUS_SIGNAL_FLAGS_NONE, on_signal, o, NULL);
    g_print("语音浮条已就绪（%s）\n", name);
}

static void on_name_lost(GDBusConnection *conn, const gchar *name, gpointer data) {
    (void)conn;
    (void)data;
    g_printerr("注册 %s 失败（可能已有别的浮条在跑）\n", name);
}

static void activate(GtkApplication *app, gpointer data) {
    Overlay *o = data;

    GtkWidget *win = gtk_application_window_new(app);
    o->win = GTK_WINDOW(win);
    gtk_window_set_decorated(o->win, FALSE);
    gtk_window_set_resizable(o->win, FALSE);
    gtk_widget_set_focusable(GTK_WIDGET(o->win), FALSE);

    gtk_layer_init_for_window(o->win);
    gtk_layer_set_namespace(o->win, "archtype-voice-overlay");
    gtk_layer_set_layer(o->win, GTK_LAYER_SHELL_LAYER_OVERLAY);
    gtk_layer_set_anchor(o->win, GTK_LAYER_SHELL_EDGE_TOP, TRUE);
    gtk_layer_set_anchor(o->win, GTK_LAYER_SHELL_EDGE_LEFT, TRUE);
    gtk_layer_set_keyboard_mode(o->win, GTK_LAYER_SHELL_KEYBOARD_MODE_NONE);

    GtkWidget *pill = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0);
    gtk_widget_add_css_class(pill, "archtype-voice-pill");

    o->mic = gtk_drawing_area_new();
    gtk_widget_set_size_request(o->mic, (int)MIC_SIZE, (int)MIC_SIZE);
    gtk_widget_set_valign(o->mic, GTK_ALIGN_CENTER);
    gtk_widget_set_halign(o->mic, GTK_ALIGN_CENTER);
    gtk_drawing_area_set_draw_func(GTK_DRAWING_AREA(o->mic), draw_mic, o, NULL);
    gtk_box_append(GTK_BOX(pill), o->mic);

    o->label = gtk_label_new("正在听…");
    gtk_widget_add_css_class(o->label, "archtype-voice-label");
    gtk_widget_set_valign(o->label, GTK_ALIGN_CENTER);
    gtk_box_append(GTK_BOX(pill), o->label);

    o->wave = gtk_drawing_area_new();
    gtk_widget_set_size_request(o->wave, 130, (int)BARS_H);
    gtk_widget_set_valign(o->wave, GTK_ALIGN_CENTER);
    gtk_drawing_area_set_draw_func(GTK_DRAWING_AREA(o->wave), draw_wave, o, NULL);
    gtk_box_append(GTK_BOX(pill), o->wave);

    gtk_window_set_child(o->win, pill);

    GtkCssProvider *css = gtk_css_provider_new();
    gtk_css_provider_load_from_string(css,
        ".archtype-voice-pill {"
        "  background-color: #ffffff;"
        "  border: 1.5px solid #dce3df;"
        "  border-radius: 14px;"
        "  box-shadow: 0 4px 18px 0 rgba(29, 35, 33, 0.16);"
        "  padding: 3px 16px 3px 4px;"
        "}"
        ".archtype-voice-label {"
        "  color: #1d2321;"
        "  font-size: 15px;"
        "  font-weight: 500;"
        "  margin-left: 4px;"
        "}"
        ".archtype-voice-label.error { color: #e5484d; }"
        ".archtype-voice-wave { margin-left: 18px; }");
    gtk_style_context_add_provider_for_display(gtk_widget_get_display(GTK_WIDGET(o->win)),
                                               GTK_STYLE_PROVIDER(css),
                                               GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);

    // 指针穿透：只显示，不接收点击
    GdkSurface *surface = gtk_native_get_surface(GTK_NATIVE(o->win));
    if (surface) {
        cairo_region_t *empty = cairo_region_create();
        gdk_surface_set_input_region(surface, empty);
        cairo_region_destroy(empty);
    }

    gtk_widget_set_visible(GTK_WIDGET(o->win), FALSE);
    o->opacity_target = 0;

    o->tick_id = g_timeout_add(16, tick_cb, o);

    g_bus_own_name(G_BUS_TYPE_SESSION, BUS_NAME, G_BUS_NAME_OWNER_FLAGS_NONE,
                   NULL, on_name_acquired, on_name_lost, o, NULL);
}

int main(int argc, char **argv) {
    Overlay o;
    memset(&o, 0, sizeof(o));
    o.opacity = 0;

    GtkApplication *app = gtk_application_new("io.github.gowayhui.ArchType.VoiceOverlayApp",
                                              G_APPLICATION_DEFAULT_FLAGS);
    o.app = app;
    g_signal_connect(app, "activate", G_CALLBACK(activate), &o);
    setvbuf(stdout, NULL, _IONBF, 0);
    setvbuf(stderr, NULL, _IONBF, 0);
    int status = g_application_run(G_APPLICATION(app), argc, argv);
    g_object_unref(app);
    return status;
}
