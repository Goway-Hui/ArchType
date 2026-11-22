//! 候选框的数据与布局（平台无关）：一页的尺寸、每个候选的位置、点击命中。
//!
//! 平台层只提供文字测量（macOS 用 `NSString.sizeWithAttributes`，Windows 用 DirectWrite）
//! 和绘制；横/竖排、6 列网格、序号宽度、点击命中都在这里，两端完全一致。

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
    /// 候选上方的一行（互译的方向标签、工具栏标题、语音提示）
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

/// 量一段文字（宽、高），字号由调用方给。
pub trait TextMeasurer {
    fn measure(&self, text: &str, font_size: f64) -> (f64, f64);
}

pub const PADDING_X: f64 = 10.0;
pub const PADDING_Y: f64 = 7.0;
pub const ITEM_GAP: f64 = 14.0;
pub const RADIUS: f64 = 9.0;

/// 小字（序号、注释、提示行）相对正文的字号。
pub const LABEL_SCALE: f64 = 0.78;

/// 一页的排版：每项的宽度、每项左上角的位置、总尺寸。
pub struct Layout {
    pub widths: Vec<f64>,
    pub positions: Vec<(f64, f64)>,
    pub size: (f64, f64),
    pub row_height: f64,
    pub top_height: f64,
}

/// 算一页的尺寸和每个候选的位置（视图坐标，y 向下）。
pub fn layout(page: &Page, measurer: &impl TextMeasurer) -> Layout {
    let label_size = page.font_size * LABEL_SCALE;
    let line = measurer.measure("国", page.font_size).1.max(1.0);
    let row_height = line + PADDING_Y * 2.0;

    let mut widths = Vec::with_capacity(page.texts.len());
    for (i, text) in page.texts.iter().enumerate() {
        let mut width = if page.labels {
            measurer.measure(&format!("{}.", i + 1), label_size).0
        } else {
            0.0
        };
        width += measurer.measure(text, page.font_size).0;
        if let Some(comment) = page.comments.get(i) {
            if !comment.is_empty() {
                width += 4.0 + measurer.measure(comment, label_size).0;
            }
        }
        widths.push(width);
    }

    let top_height = if page.aux_top.is_some() { row_height } else { 0.0 };
    let bottom_height = if page.aux_bottom.is_some() { row_height } else { 0.0 };
    let count = page.texts.len();

    let mut positions = vec![(PADDING_X, top_height + PADDING_Y); count];
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
            *position = (x, top_height + row as f64 * row_height + PADDING_Y);
            x += col_widths[column] + ITEM_GAP;
        }
    } else if page.vertical {
        body_width = widths.iter().copied().fold(0.0_f64, f64::max) + PADDING_X * 2.0;
        body_height = row_height * count as f64;
        for (i, position) in positions.iter_mut().enumerate() {
            *position = (PADDING_X, top_height + i as f64 * row_height + PADDING_Y);
        }
    } else {
        body_width =
            widths.iter().sum::<f64>() + ITEM_GAP * count.saturating_sub(1) as f64 + PADDING_X * 2.0;
        body_height = row_height;
        let mut x = PADDING_X;
        for (i, position) in positions.iter_mut().enumerate() {
            *position = (x, top_height + PADDING_Y);
            x += widths[i] + ITEM_GAP;
        }
    }

    let aux_width = [page.aux_top.as_deref(), page.aux_bottom.as_deref()]
        .into_iter()
        .flatten()
        .map(|text| measurer.measure(text, label_size).0 + PADDING_X * 2.0)
        .fold(0.0_f64, f64::max);
    let width = body_width.max(aux_width).max(1.0);
    let height = (top_height + body_height + bottom_height).max(row_height);
    Layout {
        widths,
        positions,
        size: (width, height),
        row_height,
        top_height,
    }
}

/// 点在哪个候选上（视图坐标，y 向下）。
pub fn clicked_index(page: &Page, measurer: &impl TextMeasurer, x: f64, y: f64) -> Option<usize> {
    let layout = layout(page, measurer);
    let item_height = page.font_size * 1.6 + PADDING_Y;
    for (i, (left, top)) in layout.positions.iter().enumerate() {
        let width = layout.widths[i] + PADDING_X;
        let row_top = top - PADDING_Y / 2.0;
        if x >= *left && x <= left + width && y >= row_top && y <= row_top + item_height {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 假测量：宽 = 字符数 × 字号 × 0.6，高 = 字号 × 1.2。
    struct Fake;

    impl TextMeasurer for Fake {
        fn measure(&self, text: &str, font_size: f64) -> (f64, f64) {
            (text.chars().count() as f64 * font_size * 0.6, font_size * 1.2)
        }
    }

    fn page(texts: &[&str]) -> Page {
        Page {
            texts: texts.iter().map(|text| text.to_string()).collect(),
            labels: true,
            font_size: 17.0,
            ..Page::default()
        }
    }

    #[test]
    fn horizontal_page_puts_items_on_one_row() {
        let page = page(&["你好", "拟好", "你"]);
        let frame = layout(&page, &Fake);
        assert_eq!(frame.positions.len(), 3);
        assert_eq!(frame.positions[0].1, frame.positions[2].1);
        assert!(frame.positions[1].0 > frame.positions[0].0);
        assert!(frame.size.0 > frame.size.1);
        assert_eq!(frame.top_height, 0.0);
    }

    #[test]
    fn vertical_page_stacks_rows() {
        let mut page = page(&["你好", "拟好"]);
        page.vertical = true;
        let frame = layout(&page, &Fake);
        assert_eq!(frame.positions[0].0, frame.positions[1].0);
        assert!(frame.positions[1].1 > frame.positions[0].1);
        assert!(frame.size.1 > frame.size.0);
    }

    #[test]
    fn grid_wraps_after_six_columns() {
        let mut page = page(&["1", "2", "3", "4", "5", "6", "7"]);
        page.columns = Some(Page::GRID_COLUMNS);
        let frame = layout(&page, &Fake);
        assert_eq!(frame.positions[0].1, frame.positions[5].1);
        assert!(frame.positions[6].1 > frame.positions[0].1);
        assert_eq!(frame.positions[6].0, frame.positions[0].0);
    }

    #[test]
    fn aux_lines_add_height_and_width() {
        let mut page = page(&["你好"]);
        page.aux_top = Some("中 → 英".to_string());
        page.aux_bottom = Some("翻译中…".to_string());
        let frame = layout(&page, &Fake);
        assert_eq!(frame.top_height, frame.row_height);
        let bare = layout(&page_without_aux(), &Fake);
        assert!(frame.size.1 > bare.size.1);
    }

    fn page_without_aux() -> Page {
        page(&["你好"])
    }

    #[test]
    fn clicked_index_hits_items_and_ignores_gaps() {
        let page = page(&["你好", "拟好"]);
        let frame = layout(&page, &Fake);
        let (x, y) = frame.positions[1];
        assert_eq!(clicked_index(&page, &Fake, x + 2.0, y + 2.0), Some(1));
        // 两个候选之间、上方空白都不算
        assert_eq!(clicked_index(&page, &Fake, 0.5, y), None);
        assert_eq!(clicked_index(&page, &Fake, x, 0.5), None);
    }
}
