//! 候选框：一个不抢焦点的浮动面板，自绘（背景、序号、候选、注释、高亮）。
//!
//! 横排 / 竖排跟着设置；展开面板（下键）和工具栏以后在这里加网格。现在先画一页。

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSColor, NSFloatingWindowLevel, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSPanel, NSStringDrawing, NSView, NSWindowStyleMask,
};
use objc2_foundation::{NSAttributedStringKey, NSDictionary, NSPoint, NSRect, NSSize, NSString};

/// 一页要画的候选。
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub texts: Vec<String>,
    pub comments: Vec<String>,
    /// 光标在这一页里的位置；不在这一页上是 None
    pub cursor: Option<usize>,
    pub has_prev: bool,
    pub has_next: bool,
    pub vertical: bool,
    /// 展开面板（每行 6 个）；先按竖排一列画
    pub expanded: bool,
    /// 候选上方的一行提示（工具栏、互译）
    pub aux: Option<String>,
    pub font_size: f64,
}

impl Page {
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty() && self.aux.is_none()
    }
}

struct ViewIvars {
    page: RefCell<Page>,
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
    }
);

impl CandidateView {
    /// 建一个自绘视图（父类是 NSView，用它的 initWithFrame）。
    fn make(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ViewIvars {
            page: RefCell::new(Page::default()),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set_page(&self, page: Page) {
        *self.ivars().page.borrow_mut() = page;
        self.setNeedsDisplay(true);
    }

    fn page_size(&self) -> NSSize {
        let page = self.ivars().page.borrow();
        measure(&page).1
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

const PADDING_X: f64 = 10.0;
const PADDING_Y: f64 = 7.0;
const ITEM_GAP: f64 = 12.0;
const RADIUS: f64 = 9.0;

/// 算一页的尺寸、每项的宽度。
fn measure(page: &Page) -> (Vec<f64>, NSSize) {
    let font = NSFont::systemFontOfSize(page.font_size);
    let color = NSColor::labelColor();
    let attrs = attributes(&font, &color);
    let label_font = NSFont::systemFontOfSize(page.font_size * 0.78);
    let label_attrs = attributes(&label_font, &NSColor::secondaryLabelColor());

    let mut widths = Vec::with_capacity(page.texts.len());
    let line = text_size("国", &attrs).height.max(1.0);
    for (i, text) in page.texts.iter().enumerate() {
        let mut width = text_size(&format!("{}.", i + 1), &label_attrs).width;
        width += text_size(text, &attrs).width;
        if let Some(comment) = page.comments.get(i) {
            if !comment.is_empty() {
                width += 4.0 + text_size(comment, &label_attrs).width;
            }
        }
        widths.push(width);
    }

    let row_height = line + PADDING_Y * 2.0;
    let aux_height = if page.aux.is_some() { row_height } else { 0.0 };

    let (width, height) = if let Some(aux) = &page.aux {
        let aux_width = text_size(aux, &label_attrs).width + PADDING_X * 2.0;
        let body_width = if page.vertical {
            widths.iter().copied().fold(0.0_f64, f64::max) + PADDING_X * 2.0 + ITEM_GAP
        } else {
            widths.iter().sum::<f64>()
                + ITEM_GAP * page.texts.len().saturating_sub(1) as f64
                + PADDING_X * 2.0
        };
        let body_height = if page.vertical {
            row_height * page.texts.len() as f64
        } else {
            row_height
        };
        (aux_width.max(body_width), aux_height + body_height)
    } else if page.vertical {
        let width = widths.iter().copied().fold(0.0_f64, f64::max) + PADDING_X * 2.0;
        (width, row_height * page.texts.len() as f64)
    } else {
        let width = widths.iter().sum::<f64>()
            + ITEM_GAP * page.texts.len().saturating_sub(1) as f64
            + PADDING_X * 2.0;
        (width.max(1.0), row_height)
    };

    (widths, NSSize::new(width, height.max(row_height)))
}

/// 画一页（视图坐标，y 向下）。
fn draw_page(page: &Page) {
    if page.is_empty() {
        return;
    }
    let (widths, size) = measure(page);
    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), size);

    // 背景：圆角、浅色 / 深色跟着系统
    let background = NSColor::windowBackgroundColor();
    background.setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, RADIUS, RADIUS).fill();

    // 描边
    let border = NSColor::separatorColor();
    border.setStroke();
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, RADIUS, RADIUS);
    path.setLineWidth(1.0);
    path.stroke();

    let font = NSFont::systemFontOfSize(page.font_size);
    let label_font = NSFont::systemFontOfSize(page.font_size * 0.78);
    let normal_attrs = attributes(&font, &NSColor::labelColor());
    let highlight_attrs = attributes(&font, &NSColor::whiteColor());
    let label_attrs = attributes(&label_font, &NSColor::secondaryLabelColor());
    let highlight_label_attrs = attributes(&label_font, &NSColor::whiteColor());

    let mut y = PADDING_Y;
    if let Some(aux) = &page.aux {
        let aux_size = text_size(aux, &label_attrs);
        let _ = aux_size;
        let s = NSString::from_str(aux);
        unsafe {
            s.drawAtPoint_withAttributes(
                NSPoint::new(PADDING_X, y + PADDING_Y / 2.0),
                Some(&label_attrs),
            );
        }
        y += aux_size.height.max(page.font_size) + PADDING_Y * 2.0;
    }

    let mut x = PADDING_X;
    for (i, text) in page.texts.iter().enumerate() {
        let width = widths[i];
        let row = NSRect::new(
            NSPoint::new(x, y - PADDING_Y / 2.0),
            NSSize::new(width + PADDING_X, page.font_size * 1.6 + PADDING_Y),
        );
        let selected = page.cursor == Some(i);
        if selected {
            NSColor::selectedContentBackgroundColor().setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                row,
                RADIUS - 2.0,
                RADIUS - 2.0,
            )
            .fill();
        }

        let number = format!("{}.", i + 1);
        let number_attrs = if selected {
            &highlight_label_attrs
        } else {
            &label_attrs
        };
        let number_string = NSString::from_str(&number);
        unsafe {
            number_string.drawAtPoint_withAttributes(NSPoint::new(x + 2.0, y), Some(number_attrs));
        }
        let number_width = text_size(&number, number_attrs).width;

        let attrs = if selected {
            &highlight_attrs
        } else {
            &normal_attrs
        };
        let body = NSString::from_str(text);
        unsafe {
            body.drawAtPoint_withAttributes(NSPoint::new(x + number_width + 4.0, y), Some(attrs));
        }

        if let Some(comment) = page.comments.get(i) {
            if !comment.is_empty() {
                let comment_attrs = if selected {
                    &highlight_label_attrs
                } else {
                    &label_attrs
                };
                let comment_width = text_size(comment, comment_attrs).width;
                let comment_string = NSString::from_str(comment);
                unsafe {
                    comment_string.drawAtPoint_withAttributes(
                        NSPoint::new(x + width - comment_width, y + page.font_size * 0.15),
                        Some(comment_attrs),
                    );
                }
            }
        }

        if page.vertical {
            x = PADDING_X;
            y += page.font_size * 1.6 + PADDING_Y;
        } else {
            x += width + PADDING_X + ITEM_GAP;
        }
    }
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
        let window = unsafe {
            NSPanel::initWithContentRect_styleMask_backing_defer(
                mtm.alloc::<NSPanel>(),
                frame,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setOpaque(false);
        window.setHasShadow(true);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setLevel(NSFloatingWindowLevel);
        window.setIgnoresMouseEvents(true);

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

    pub fn hide(&self) {
        self.window.orderOut(None);
    }
}
