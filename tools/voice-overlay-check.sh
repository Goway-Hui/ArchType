#!/usr/bin/env bash
# 语音浮条自检，两件事：
# 1) 静态检查：窗口背景透明的 CSS 不能被改坏。GTK CSS 不支持 !important（写了整条规则
#    会被丢掉），主题用 window.background（比 .transparent 更具体），所以必须点名覆盖；
#    圆角靠 gtk_widget_set_overflow(HIDDEN) 裁剪。改掉任何一条，深色主题下四角就会露主题色。
# 2) 有 Wayland + grim 时起一次浮条截图，打印四角像素和截图路径，人眼对照桌面。
#
# 用法：tools/voice-overlay-check.sh [archtype-voice-overlay]
set -euo pipefail

cd "$(dirname "$0")/.."
SRC=voice-overlay/archtype-voice-overlay.c
die() { echo "失败：$*" >&2; exit 1; }

[ -f "$SRC" ] || die "找不到 $SRC"
# 只看代码（去掉 // 注释），注释里会提到 !important 这个词
if awk '{ sub(/\/\/.*/, ""); print }' "$SRC" | grep -q '!important'; then
    die "CSS 里出现了 !important：GTK 不支持，整条规则会被丢掉，四角会露主题色"
fi
grep -q 'window.transparent.background' "$SRC" \
    || die "CSS 必须覆盖 window.transparent.background（否则深色主题盖掉透明）"
grep -q 'gtk_widget_set_overflow' "$SRC" \
    || die "窗口要用 gtk_widget_set_overflow(HIDDEN) 裁圆角"
echo "静态检查：OK"

# 运行时截图是附加项：没有 Wayland/工具就只做静态检查
BIN=${1:-${ARCHTYPE_VOICE_OVERLAY:-}}
[ -n "${WAYLAND_DISPLAY:-}" ] || { echo "（没有 Wayland 会话，跳过截图）"; exit 0; }
command -v grim >/dev/null 2>&1 || { echo "（没有 grim，跳过截图）"; exit 0; }
command -v dbus-send >/dev/null 2>&1 || { echo "（没有 dbus-send，跳过截图）"; exit 0; }
[ -n "$BIN" ] || BIN=$(command -v archtype-voice-overlay || true)
[ -n "$BIN" ] || { echo "（没给浮条路径，跳过截图）"; exit 0; }

NAME=io.github.gowayhui.ArchType.VoiceOverlay
if gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.GetNameOwner "$NAME" >/dev/null 2>&1; then
    echo "（已有浮条在跑，跳过截图；要截图先停掉它）"
    exit 0
fi

tmp=$(mktemp -d)
shot=/tmp/archtype-voice-overlay-check.png
trap 'kill "$pid" 2>/dev/null || true; rm -rf "$tmp"' EXIT
"$BIN" >"$tmp/log" 2>&1 &
pid=$!
for _ in $(seq 1 30); do
    gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.GetNameOwner "$NAME" >/dev/null 2>&1 && break
    sleep 0.1
done
dbus-send --session --type=signal /io/github/gowayhui/ArchType \
    io.github.gowayhui.ArchType.Voice1.State \
    uint32:1 string:"正在听…" int32:50 int32:0 int32:0 int32:0 int32:0 boolean:false double:1.0
sleep 0.8
grim "$shot"
echo "截图：${shot}（对照桌面看四角是否透明）"
python3 - "$shot" <<'PY2' || true
import sys
try:
    from PIL import Image
except ImportError:
    sys.exit(0)
im = Image.open(sys.argv[1]).convert("RGB")
w, h = im.size
px = im.load()
for y in range(h // 2, h):
    run = 0
    for x in range(w):
        p = px[x, y]
        if all(c > 245 for c in p):
            run += 1
            if run > 80:
                xs = [x for x in range(w) if all(c > 245 for c in px[x, y])]
                x0, x1 = min(xs), max(xs)
                ys = [yy for yy in range(max(0, y - 200), h)
                      if all(c > 245 for c in px[(x0 + x1) // 2, yy])]
                y0, y1 = min(ys), max(ys)
                print("四角像素：",
                      px[x0 - 4, y0 - 4], px[x1 + 4, y0 - 4],
                      px[x0 - 4, y1 + 4], px[x1 + 4, y1 + 4])
                sys.exit(0)
        else:
            run = 0
PY2
