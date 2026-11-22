//! 候选框：一个不抢焦点的浮动面板，自绘（背景、序号、候选、注释、高亮、提示行）。
//!
//! 数据与布局（横/竖排、6 列网格、序号宽度、点击命中）在 `archtype_shell::panel`；
//! 这里只做 macOS 的文字测量与绘制。

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSColor, NSEvent, NSFloatingWindowLevel, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSPanel, NSStringDrawing, NSView,
    NSWindowStyleMask,
};
use objc2_foundation::{NSAttributedStringKey, NSDictionary, NSPoint, NSRect, NSSize, NSString};

use archtype_shell::panel::{
    clicked_index, layout, Page, TextMeasurer, LABEL_SCALE, PADDING_X, PADDING_Y, RADIUS,
};

/// macOS 的文字测量（`NSString.sizeWithAttributes`）。
struct MacMeasurer;

impl TextMeasurer for MacMeasurer {
    fn measure(&self, text: &str, font_size: f64) -> (f64, f64) {
        let font = NSFont::systemFontOfSize(font_size);
        let color = NSColor::labelColor();
        let attrs = attributes(&font, &color);
        let size = text_size(text, &attrs);
        (size.width, size.height)
    }
}

struct ViewIvars {
    page: RefCell<Page>,
    /// 点击候选时通知谁（控制器的裸指针：面板活在控制器里，不会比它长）
    target: Cell<*mut AnyObject>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[name = "ArchTypeCandidateView"]
    #[ivars = ViewIvars]
    struct CandidateView;

    impl CandidateView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            // 从上往下画（和候选列表的顺序一致）
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let page = self.ivars().page.borrow();
            draw_page(&page);
        }

        /// 点候选：算出点的是第几个，交给控制器选。
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: Option<&NSEvent>) {
            let Some(event) = event else {
                return;
            };
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            let index = {
                let page = self.ivars().page.borrow();
                clicked_index(&page, &MacMeasurer, point.x, point.y)
            };
            let Some(index) = index else {
                return;
            };
            let target = self.ivars().target.get();
            if !target.is_null() {
                unsafe {
                    let _: () = msg_send![target, candidateClicked: index];
                }
            }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl CandidateView {
    /// 建一个自绘视图（父类是 NSView，用它的 initWithFrame）。
    fn make(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ViewIvars {
            page: RefCell::new(Page::default()),
            target: Cell::new(std::ptr::null_mut()),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set_target(&self, target: &AnyObject) {
        self.ivars()
            .target
            .set(target as *const AnyObject as *mut AnyObject);
    }

    fn set_page(&self, page: Page) {
        *self.ivars().page.borrow_mut() = page;
        self.setNeedsDisplay(true);
    }

    fn page_size(&self) -> NSSize {
        let page = self.ivars().page.borrow();
        let (width, height) = layout(&page, &MacMeasurer).size;
        NSSize::new(width, height)
    }
}

/// 生成一张属性字典：字体 + 颜色。
fn attributes(
    font: &NSFont,
    color: &NSColor,
) -> Retained<NSDictionary<NSAttributedStringKey, AnyObject>> {
    // SAFETY: NSAttributedStringKey 就是 NSString；NSFont / NSColor 是 NSObject 子类，
    // 当 AnyObject 传进字典只用来读。
    let font_obj: &AnyObject = unsafe { &*(font as *const NSFont as *const AnyObject) };
    let color_obj: &AnyObject = unsafe { &*(color as *const NSColor as *const AnyObject) };
    // SAFETY: 这两个是 AppKit 提供的全局 NSString 键，进程活多久它们活多久。
    let font_key = unsafe { NSFontAttributeName };
    let color_key = unsafe { NSForegroundColorAttributeName };
    NSDictionary::from_slices(&[font_key, color_key], &[font_obj, color_obj])
}

/// 一段文字的宽度、高度。
fn text_size(text: &str, attrs: &NSDictionary<NSAttributedStringKey, AnyObject>) -> NSSize {
    if text.is_empty() {
        return NSSize::new(0.0, 0.0);
    }
    let s = NSString::from_str(text);
    unsafe { s.sizeWithAttributes(Some(attrs)) }
}

/// 画一页（视图坐标，y 向下）。
fn draw_page(page: &Page) {
    if page.is_empty() {
        return;
    }
    let frame = layout(page, &MacMeasurer);
    let bounds = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(frame.size.0, frame.size.1),
    );

    // 背景：圆角、浅色 / 深色跟着系统
    NSColor::windowBackgroundColor().setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, RADIUS, RADIUS).fill();

    // 描边
    NSColor::separatorColor().setStroke();
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, RADIUS, RADIUS);
    path.setLineWidth(1.0);
    path.stroke();

    let font = NSFont::systemFontOfSize(page.font_size);
    let label_font = NSFont::systemFontOfSize(page.font_size * LABEL_SCALE);
    let normal_attrs = attributes(&font, &NSColor::labelColor());
    let highlight_attrs = attributes(&font, &NSColor::whiteColor());
    let label_attrs = attributes(&label_font, &NSColor::secondaryLabelColor());
    let highlight_label_attrs = attributes(&label_font, &NSColor::whiteColor());

    // 上方一行（互译的方向标签、工具栏标题、语音提示）
    if let Some(aux) = &page.aux_top {
        draw_aux(aux, NSPoint::new(PADDING_X, PADDING_Y), &label_attrs);
    }
    // 下方一行（翻译状态、工具栏提示）
    if let Some(aux) = &page.aux_bottom {
        draw_aux(
            aux,
            NSPoint::new(
                PADDING_X,
                frame.size.1 - frame.row_height + PADDING_Y,
            ),
            &label_attrs,
        );
    }

    let item_height = page.font_size * 1.6 + PADDING_Y;
    for (i, text) in page.texts.iter().enumerate() {
        let (left, top) = frame.positions[i];
        let width = frame.widths[i];
        let row = NSRect::new(
            NSPoint::new(left, top - PADDING_Y / 2.0),
            NSSize::new(width + PADDING_X, item_height),
        );
        let selected = page.cursor == Some(i);
        if selected {
            NSColor::selectedContentBackgroundColor().setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(row, RADIUS - 2.0, RADIUS - 2.0)
                .fill();
        }

        let number = format!("{}.", i + 1);
        let number_attrs = if selected {
            &highlight_label_attrs
        } else {
            &label_attrs
        };
        let number_width = if page.labels {
            draw_aux(&number, NSPoint::new(left, top), number_attrs);
            text_size(&number, number_attrs).width
        } else {
            0.0
        };

        let attrs = if selected {
            &highlight_attrs
        } else {
            &normal_attrs
        };
        let body = NSString::from_str(text);
        unsafe {
            body.drawAtPoint_withAttributes(
                NSPoint::new(
                    left + number_width + if page.labels { 4.0 } else { 0.0 },
                    top,
                ),
                Some(attrs),
            );
        }

        if let Some(comment) = page.comments.get(i) {
            if !comment.is_empty() {
                let comment_attrs = if selected {
                    &highlight_label_attrs
                } else {
                    &label_attrs
                };
                let comment_width = text_size(comment, comment_attrs).width;
                draw_aux(
                    comment,
                    NSPoint::new(
                        left + width - comment_width,
                        top + page.font_size * 0.15,
                    ),
                    comment_attrs,
                );
            }
        }
    }
}

fn draw_aux(text: &str, point: NSPoint, attrs: &NSDictionary<NSAttributedStringKey, AnyObject>) {
    let string = NSString::from_str(text);
    unsafe { string.drawAtPoint_withAttributes(point, Some(attrs)) };
}

/// 候选面板：一个无边框、不抢焦点、浮在别的窗口上面的小窗。
pub struct CandidatePanel {
    window: Retained<NSPanel>,
    view: Retained<CandidateView>,
}

impl CandidatePanel {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0));
        let window = NSPanel::initWithContentRect_styleMask_backing_defer(
            mtm.alloc::<NSPanel>(),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setOpaque(false);
        window.setHasShadow(true);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setLevel(NSFloatingWindowLevel);
        // 鼠标能点候选（点空白处不抢焦点，NonactivatingPanel 保证）
        window.setIgnoresMouseEvents(false);

        let view = CandidateView::make(mtm, frame);
        window.setContentView(Some(&view));
        Self { window, view }
    }

    /// 画这一页，放在光标下面（`caret` 是插入点矩形，屏幕坐标）。
    pub fn show(&self, page: &Page, caret: NSRect) {
        if page.is_empty() {
            self.hide();
            return;
        }
        self.view.set_page(page.clone());
        let size = self.view.page_size();
        let mut origin = NSPoint::new(caret.origin.x, caret.origin.y - size.height - 4.0);
        // 屏幕底部放不下就画在光标上面
        if origin.y < 0.0 {
            origin.y = caret.origin.y + caret.size.height + 4.0;
        }
        self.window.setContentSize(size);
        self.window.setFrameOrigin(origin);
        self.window.orderFrontRegardless();
    }

    /// 点击候选时通知谁（控制器）。
    pub fn set_target(&self, target: &AnyObject) {
        self.view.set_target(target);
    }

    pub fn hide(&self) {
        self.window.orderOut(None);
    }
}
