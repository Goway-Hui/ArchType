//! Windows 候选窗口：Win32 弹出窗口 + GDI 自绘。
//!
//! 排版（横/竖排、展开网格、序号宽度、点击命中）在 `archtype_shell::panel`，
//! 这里只做文字测量、绘制和窗口定位（和 macOS 的 `CandidatePanel` 对应）。

use std::cell::RefCell;

use archtype_shell::panel::{
    clicked_index, layout, Layout, Page, TextMeasurer, LABEL_SCALE, PADDING_X, PADDING_Y, RADIUS,
};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, EndPaint, GetTextExtentPoint32W, RoundRect,
    SelectObject, SetBkMode, SetTextAlign, SetTextColor, TextOutW, CLEARTYPE_QUALITY,
    DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL, HDC, HFONT, HGDIOBJ,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, SRCCOPY, TA_LEFT, TA_TOP, TRANSPARENT,
};
use windows::Win32::Graphics::Gdi::{GetDC, InvalidateRect, ReleaseDC};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, PostMessageW, RegisterClassW, SetWindowPos,
    ShowWindow, CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
    WM_APP, WM_LBUTTONDOWN, WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

const WM_ARCHTYPE_CLICK: u32 = WM_APP + 1;

const CLASS_NAME: PCWSTR = w!("ArchTypeCandidate");

thread_local! {
    static WINDOW: RefCell<Option<CandidateWindow>> = RefCell::new(None);
}

struct CandidateWindow {
    hwnd: HWND,
    page: Page,
    layout: Layout,
    scale: f64,
    on_click: Option<Box<dyn Fn(usize)>>,
}

pub fn show(page: &Page, caret: RECT, on_click: Box<dyn Fn(usize)>) {
    WINDOW.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let Some(hwnd) = create_window() else {
                return;
            };
            *slot = Some(CandidateWindow {
                hwnd,
                page: Page::default(),
                layout: Layout {
                    widths: Vec::new(),
                    positions: Vec::new(),
                    size: (1.0, 1.0),
                    row_height: 1.0,
                    top_height: 0.0,
                },
                scale: 1.0,
                on_click: None,
            });
        }
        let window = slot.as_mut().unwrap();
        let scale = unsafe {
            let dpi = GetDpiForWindow(window.hwnd);
            if dpi == 0 {
                1.0
            } else {
                dpi as f64 / 96.0
            }
        };
        let measurer = GdiMeasurer { scale };
        window.page = page.clone();
        window.layout = layout(page, &measurer);
        window.scale = scale;
        window.on_click = Some(on_click);

        let width = (window.layout.size.0 * scale).ceil().max(1.0) as i32;
        let height = (window.layout.size.1 * scale).ceil().max(1.0) as i32;
        let gap = (6.0 * scale) as i32;
        let (left, top, right, bottom) = unsafe {
            let monitor = MonitorFromRect(&caret, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let work = info.rcWork;
                (work.left, work.top, work.right, work.bottom)
            } else {
                (
                    GetSystemMetrics(SM_XVIRTUALSCREEN),
                    GetSystemMetrics(SM_YVIRTUALSCREEN),
                    GetSystemMetrics(SM_CXVIRTUALSCREEN),
                    GetSystemMetrics(SM_CYVIRTUALSCREEN),
                )
            }
        };
        let x = caret.left.clamp(left, (right - width).max(left));
        let mut y = caret.bottom + gap;
        if y + height > bottom {
            y = caret.top - height - gap;
        }
        let y = y.clamp(top, (bottom - height).max(top));
        unsafe {
            let _ = SetWindowPos(
                window.hwnd,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(window.hwnd, None, BOOL::from(true));
        }
    });
}

pub fn hide() {
    WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            unsafe {
                let _ = ShowWindow(window.hwnd, SW_HIDE);
            }
        }
    });
}

fn create_window() -> Option<HWND> {
    unsafe {
        let instance =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR::null()).ok()?;
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: windows::Win32::Foundation::HINSTANCE(instance.0),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            CLASS_NAME,
            w!("ArchType"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            windows::Win32::Foundation::HINSTANCE(instance.0),
            None,
        )
        .ok()?;
        let _ = ShowWindow(hwnd, SW_HIDE);
        Some(hwnd)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            WINDOW.with(|slot| {
                let slot = slot.borrow();
                if let Some(window) = slot.as_ref() {
                    if window.hwnd == hwnd {
                        paint(window);
                    }
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as i16 as isize;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as isize;
            let _ = PostMessageW(
                hwnd,
                WM_ARCHTYPE_CLICK,
                WPARAM(x as usize),
                LPARAM(y as isize),
            );
            LRESULT(0)
        }
        WM_ARCHTYPE_CLICK => {
            let index = WINDOW.with(|slot| {
                let slot = slot.borrow();
                let Some(window) = slot.as_ref() else {
                    return None;
                };
                if window.hwnd != hwnd {
                    return None;
                }
                let measurer = GdiMeasurer {
                    scale: window.scale,
                };
                clicked_index(
                    &window.page,
                    &measurer,
                    wparam.0 as f64 / window.scale,
                    lparam.0 as f64 / window.scale,
                )
            });
            if let Some(index) = index {
                let callback =
                    WINDOW.with(|slot| slot.borrow_mut().as_mut().and_then(|w| w.on_click.take()));
                if let Some(callback) = callback {
                    callback(index);
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn paint(window: &CandidateWindow) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(window.hwnd, &mut ps);
        let mut rect = RECT::default();
        let _ = GetClientRect(window.hwnd, &mut rect);
        let width = (rect.right - rect.left).max(1);
        let height = (rect.bottom - rect.top).max(1);

        // 双缓冲，避免闪烁
        let mem = CreateCompatibleDC(hdc);
        let bitmap = CreateCompatibleBitmap(hdc, width, height);
        let old_bitmap = SelectObject(mem, HGDIOBJ(bitmap.0));
        SetBkMode(mem, TRANSPARENT);

        let radius = (RADIUS * window.scale) as i32;
        let white = CreateSolidBrush(COLORREF(0x00ff_ffff));
        let border = CreatePen(PS_SOLID, 1, COLORREF(0x00df_e3dc));
        let old_pen = SelectObject(mem, HGDIOBJ(border.0));
        let old_brush = SelectObject(mem, HGDIOBJ(white.0));
        let _ = RoundRect(mem, 0, 0, width, height, radius * 2, radius * 2);
        let _ = SelectObject(mem, old_pen);
        let _ = SelectObject(mem, old_brush);
        let _ = DeleteObject(HGDIOBJ(white.0));
        let _ = DeleteObject(HGDIOBJ(border.0));

        let scale = window.scale;
        let font_size = window.page.font_size;
        let main_font = font_for(font_size * scale, FW_NORMAL.0 as i32);
        let label_font = font_for(font_size * scale * LABEL_SCALE, FW_NORMAL.0 as i32);
        let _ = SetTextAlign(mem, TA_LEFT | TA_TOP);

        for (index, text) in window.page.texts.iter().enumerate() {
            let (left, top) = window.layout.positions[index];
            let left = left * scale;
            let top = top * scale;
            let item_width = (window.layout.widths[index] + PADDING_X) * scale;
            let item_height = (font_size * 1.6 + PADDING_Y) * scale;
            let selected = window.page.cursor == Some(index);
            if selected {
                let brush = CreateSolidBrush(COLORREF(0x00ed_6f2f));
                let old = SelectObject(mem, HGDIOBJ(brush.0));
                let r = (RADIUS * scale) as i32;
                let _ = RoundRect(
                    mem,
                    left as i32,
                    (top - PADDING_Y / 2.0 * scale) as i32,
                    (left + item_width) as i32,
                    (top - PADDING_Y / 2.0 * scale + item_height) as i32,
                    r * 2,
                    r * 2,
                );
                let _ = SelectObject(mem, old);
                let _ = DeleteObject(HGDIOBJ(brush.0));
                let _ = SetTextColor(mem, COLORREF(0x00ff_ffff));
            } else {
                let _ = SetTextColor(mem, COLORREF(0x0027_1f1f));
            }

            let mut x = left;
            if window.page.labels {
                let label = format!("{}.", index + 1);
                let old = SelectObject(mem, HGDIOBJ(label_font.0));
                let _ = TextOutW(mem, x as i32, top as i32, &utf16(&label));
                let _ = SelectObject(mem, old);
                let (w, _) = measure_with(mem, label_font, &label);
                x += w;
            }
            let old = SelectObject(mem, HGDIOBJ(main_font.0));
            let _ = TextOutW(mem, x as i32, top as i32, &utf16(text));
            let _ = SelectObject(mem, old);
            if let Some(comment) = window.page.comments.get(index) {
                if !comment.is_empty() {
                    let (w, _) = measure_with(mem, main_font, text);
                    let old = SelectObject(mem, HGDIOBJ(label_font.0));
                    let _ = SetTextColor(mem, COLORREF(0x0060_7570));
                    let _ = TextOutW(
                        mem,
                        (x + w + 4.0 * scale) as i32,
                        top as i32,
                        &utf16(comment),
                    );
                    let _ = SelectObject(mem, old);
                }
            }
        }

        // 上方/下方的提示行
        let label_size = font_size * LABEL_SCALE * scale;
        if let Some(text) = &window.page.aux_top {
            let font = font_for(label_size, FW_NORMAL.0 as i32);
            let old = SelectObject(mem, HGDIOBJ(font.0));
            let _ = SetTextColor(mem, COLORREF(0x0060_7570));
            let _ = TextOutW(
                mem,
                (PADDING_X * scale) as i32,
                (PADDING_Y * scale) as i32,
                &utf16(text),
            );
            let _ = SelectObject(mem, old);
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
        if let Some(text) = &window.page.aux_bottom {
            let font = font_for(label_size, FW_NORMAL.0 as i32);
            let old = SelectObject(mem, HGDIOBJ(font.0));
            let _ = SetTextColor(mem, COLORREF(0x0060_7570));
            let y = ((height as f64 / scale) - PADDING_Y - font_size * LABEL_SCALE) * scale;
            let _ = TextOutW(mem, (PADDING_X * scale) as i32, y as i32, &utf16(text));
            let _ = SelectObject(mem, old);
            let _ = DeleteObject(HGDIOBJ(font.0));
        }

        let _ = DeleteObject(HGDIOBJ(main_font.0));
        let _ = DeleteObject(HGDIOBJ(label_font.0));
        let _ = BitBlt(hdc, 0, 0, width, height, mem, 0, 0, SRCCOPY);
        let _ = SelectObject(mem, old_bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(mem);
        let _ = EndPaint(window.hwnd, &ps);
    }
}

struct GdiMeasurer {
    scale: f64,
}

impl TextMeasurer for GdiMeasurer {
    fn measure(&self, text: &str, font_size: f64) -> (f64, f64) {
        unsafe {
            let hdc = GetDC(None);
            if hdc.is_invalid() {
                return (text.chars().count() as f64 * font_size, font_size);
            }
            let font = font_for(font_size * self.scale, FW_NORMAL.0 as i32);
            let old = SelectObject(hdc, HGDIOBJ(font.0));
            let (width, height) = measure_with(hdc, font, text);
            let _ = SelectObject(hdc, old);
            let _ = DeleteObject(HGDIOBJ(font.0));
            let _ = ReleaseDC(None, hdc);
            (width / self.scale, height / self.scale)
        }
    }
}

fn measure_with(hdc: HDC, _font: HFONT, text: &str) -> (f64, f64) {
    unsafe {
        let mut size = windows::Win32::Foundation::SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, &utf16(text), &mut size);
        (size.cx as f64, size.cy as f64)
    }
}

fn font_for(size: f64, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -(size.round() as i32).max(1),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            0,
            CLEARTYPE_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Microsoft YaHei UI"),
        )
    }
}

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}
