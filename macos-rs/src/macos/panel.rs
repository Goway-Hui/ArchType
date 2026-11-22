//! 候选框：一个不抢焦点的浮动面板，自绘（背景、序号、候选、注释、高亮、提示行）。
//!
//! 横排 / 竖排跟着设置；展开面板（下键）每行 6 个，表情面板同样是网格，工具栏是一列；
//! 互译时上方是方向标签、下方是状态/提示。

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
    /// 展开面板（每行 6 个）；None 是普通的一行/一列
    pub columns: Option<usize>,
    /// 候选上方的一行（互译的方向标签、工具栏标题）
    pub aux_top: Option<String>,
    /// 候选下方的一行（互译的「翻译中…」「回车发送」，工具栏提示）
    pub aux_bottom: Option<String>,
    /// 画序号（联想没按 Tab 进联想行时不画：数字照常交给应用）
    pub labels: bool,
    pub font_size: f64,
}

impl Page {
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty() && self.aux_top.is_none() && self.aux_bottom.is_none()
    }

    /// 展开面板用的列数（工具栏是一列；表情面板和展开候选一样 6 列）。
    pub const GRID_COLUMNS: usize = 6;
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
        layout(&page).size
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
const ITEM_GAP: f64 = 14.0;
const RADIUS: f64 = 9.0;

/// 一页的排版：每项的宽度、每项的位置、总尺寸。
struct Layout {
    widths: Vec<f64>,
    positions: Vec<NSPoint>,
    size: NSSize,
    row_height: f64,
    top_height: f64,
}

/// 算一页的尺寸和每个候选的位置（视图坐标，y 向下）。
fn layout(page: &Page) -> Layout {
    let font = NSFont::systemFontOfSize(page.font_size);
    let attrs = attributes(&font, &NSColor::labelColor());
    let label_font = NSFont::systemFontOfSize(page.font_size * 0.78);
    let label_attrs = attributes(&label_font, &NSColor::secondaryLabelColor());

    let line = text_size("国", &attrs).height.max(1.0);
    let row_height = line + PADDING_Y * 2.0;

    let mut widths = Vec::with_capacity(page.texts.len());
    for (i, text) in page.texts.iter().enumerate() {
        let mut width = if page.labels {
            text_size(&format!("{}.", i + 1), &label_attrs).width
        } else {
            0.0
        };
        width += text_size(text, &attrs).width;
        if let Some(comment) = page.comments.get(i) {
            if !comment.is_empty() {
                width += 4.0 + text_size(comment, &label_attrs).width;
            }
        }
        widths.push(width);
    }

    let top_height = if page.aux_top.is_some() {
        row_height
    } else {
        0.0
    };
    let bottom_height = if page.aux_bottom.is_some() {
        row_height
    } else {
        0.0
    };
    let count = page.texts.len();

    let mut positions = vec![NSPoint::new(PADDING_X, top_height + PADDING_Y); count];
    let body_width;
    let body_height;

    if let Some(columns) = page.columns.filter(|c| *c > 0) {
        let mut col_widths = vec![0.0_f64; columns];
        for (i, width) in widths.iter().enumerate() {
            let slot = &mut col_widths[i % columns];
            *slot = slot.max(*width);
        }
        let rows = count.div_ceil(columns);
        body_width = col_widths.iter().sum::<f64>()
            + ITEM_GAP * columns.saturating_sub(1) as f64
            + PADDING_X * 2.0;
        body_height = row_height * rows as f64;
        let mut x = PADDING_X;
        for (i, position) in positions.iter_mut().enumerate() {
            let column = i % columns;
            if column == 0 {
                x = PADDING_X;
            }
            let row = i / columns;
            *position = NSPoint::new(x, top_height + row as f64 * row_height + PADDING_Y);
            x += col_widths[column] + ITEM_GAP;
        }
    } else if page.vertical {
        body_width = widths.iter().copied().fold(0.0_f64, f64::max) + PADDING_X * 2.0;
        body_height = row_height * count as f64;
        for (i, position) in positions.iter_mut().enumerate() {
            *position = NSPoint::new(PADDING_X, top_height + i as f64 * row_height + PADDING_Y);
        }
    } else {
        body_width = widths.iter().sum::<f64>()
            + ITEM_GAP * count.saturating_sub(1) as f64
            + PADDING_X * 2.0;
        body_height = row_height;
        let mut x = PADDING_X;
        for (i, position) in positions.iter_mut().enumerate() {
            *position = NSPoint::new(x, top_height + PADDING_Y);
            x += widths[i] + ITEM_GAP;
        }
    }

    let aux_width = [page.aux_top.as_deref(), page.aux_bottom.as_deref()]
        .into_iter()
        .flatten()
        .map(|text| text_size(text, &label_attrs).width + PADDING_X * 2.0)
        .fold(0.0_f64, f64::max);
    let width = body_width.max(aux_width).max(1.0);
    let height = (top_height + body_height + bottom_height).max(row_height);
    Layout {
        widths,
        positions,
        size: NSSize::new(width, height),
        row_height,
        top_height,
    }
}

/// 画一页（视图坐标，y 向下）。
fn draw_page(page: &Page) {
    if page.is_empty() {
        return;
    }
    let layout = layout(page);
    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), layout.size);

    // 背景：圆角、浅色 / 深色跟着系统
    NSColor::windowBackgroundColor().setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, RADIUS, RADIUS).fill();

    // 描边
    NSColor::separatorColor().setStroke();
    let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, RADIUS, RADIUS);
    path.setLineWidth(1.0);
    path.stroke();

    let font = NSFont::systemFontOfSize(page.font_size);
    let label_font = NSFont::systemFontOfSize(page.font_size * 0.78);
    let normal_attrs = attributes(&font, &NSColor::labelColor());
    let highlight_attrs = attributes(&font, &NSColor::whiteColor());
    let label_attrs = attributes(&label_font, &NSColor::secondaryLabelColor());
    let highlight_label_attrs = attributes(&label_font, &NSColor::whiteColor());

    // 上方一行（互译的方向标签、工具栏标题）
    if let Some(aux) = &page.aux_top {
        draw_aux(aux, NSPoint::new(PADDING_X, PADDING_Y), &label_attrs);
    }
    // 下方一行（翻译状态、工具栏提示）
    if let Some(aux) = &page.aux_bottom {
        draw_aux(
            aux,
            NSPoint::new(
                PADDING_X,
                layout.size.height - layout.row_height + PADDING_Y,
            ),
            &label_attrs,
        );
    }

    let item_height = page.font_size * 1.6 + PADDING_Y;
    for (i, text) in page.texts.iter().enumerate() {
        let position = layout.positions[i];
        let width = layout.widths[i];
        let row = NSRect::new(
            NSPoint::new(position.x, position.y - PADDING_Y / 2.0),
            NSSize::new(width + PADDING_X, item_height),
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
        let number_width = if page.labels {
            draw_aux(&number, position, number_attrs);
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
                    position.x + number_width + if page.labels { 4.0 } else { 0.0 },
                    position.y,
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
                        position.x + width - comment_width,
                        position.y + page.font_size * 0.15,
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
