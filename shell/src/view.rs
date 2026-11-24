//! 会话的一帧（`Frame`）→ 候选框的一页（`Page`）：平台无关的展示决策。
//!
//! 候选排列、展开面板、表情/工具栏、联想行、互译的上下两行都在这里定；平台层只管画。

use archtype_core::session::{Candidate, Frame, Panel};

use crate::config::Config;
use crate::panel::Page;

/// 这一帧要画的候选框；没有候选也没有提示时是一个空页（`Page::is_empty()`）。
pub fn page_from_frame(frame: &Frame, shown: &[Candidate], config: &Config) -> Page {
    let (vertical, columns) = match frame.panel {
        Panel::Normal => (!config.horizontal, None),
        Panel::Expanded | Panel::Emoji => (false, Some(Page::GRID_COLUMNS)),
        Panel::Tool => (true, None),
        // 联想照普通候选画；没按 Tab 进联想行时不画序号（数字照常交给应用）
        Panel::Association => (!config.horizontal, None),
    };
    let labels = !(matches!(frame.panel, Panel::Association) && frame.cand_cursor.is_none());
    let (aux_top, aux_bottom) = match &frame.translate {
        Some(view) => (
            Some(view.title.clone()),
            (!view.status.is_empty()).then(|| view.status.clone()),
        ),
        None => (
            (!frame.aux.is_empty()).then(|| frame.aux.clone()),
            None,
        ),
    };

    let mut texts = Vec::with_capacity(shown.len());
    let mut comments = Vec::with_capacity(shown.len());
    for candidate in shown {
        texts.push(candidate.text.clone());
        comments.push(candidate.comment.clone());
    }

    Page {
        texts,
        comments,
        cursor: frame.cand_cursor,
        has_prev: frame.has_prev,
        has_next: frame.has_next,
        vertical,
        columns,
        aux_top,
        aux_bottom,
        labels,
        font_size: config.candidate_font_size,
    }
}

/// 语音提示的一页（正在听 + 波形、识别中）：没有候选，只有上方一行。
pub fn voice_hint_page(text: &str, font_size: f64) -> Page {
    Page {
        aux_top: Some(text.to_string()),
        labels: false,
        font_size,
        ..Page::default()
    }
}

#[cfg(test)]
mod tests {
    // 测试里先 Default 再改一两项更好读
    #![allow(clippy::field_reassign_with_default)]
    use super::*;
    use archtype_core::session::{Candidate, Frame, Panel};

    fn candidates(texts: &[&str]) -> Vec<Candidate> {
        texts
            .iter()
            .map(|text| Candidate {
                text: text.to_string(),
                ..Candidate::default()
            })
            .collect()
    }

    #[test]
    fn normal_page_follows_layout_setting() {
        let frame = Frame::default();
        let shown = candidates(&["你好", "拟好"]);
        let config = Config::default();
        let page = page_from_frame(&frame, &shown, &config);
        assert!(!page.vertical);
        assert_eq!(page.columns, None);
        assert!(page.labels);
        assert_eq!(page.texts, vec!["你好", "拟好"]);

        let mut vertical = Config::default();
        vertical.horizontal = false;
        let page = page_from_frame(&frame, &shown, &vertical);
        assert!(page.vertical);
    }

    #[test]
    fn expanded_and_emoji_are_six_column_grids() {
        let shown = candidates(&["😀", "😁"]);
        let config = Config::default();
        let mut frame = Frame::default();
        frame.panel = Panel::Expanded;
        let page = page_from_frame(&frame, &shown, &config);
        assert_eq!(page.columns, Some(Page::GRID_COLUMNS));
        assert!(!page.vertical);

        frame.panel = Panel::Emoji;
        assert_eq!(
            page_from_frame(&frame, &shown, &config).columns,
            Some(Page::GRID_COLUMNS)
        );
    }

    #[test]
    fn tool_panel_is_a_single_column() {
        let mut frame = Frame::default();
        frame.panel = Panel::Tool;
        let page = page_from_frame(&frame, &candidates(&["剪贴板"]), &Config::default());
        assert!(page.vertical);
        assert_eq!(page.columns, None);
    }

    #[test]
    fn association_hides_labels_until_you_enter_the_row() {
        let shown = candidates(&["吧", "吗"]);
        let config = Config::default();
        let mut frame = Frame::default();
        frame.panel = Panel::Association;
        assert!(!page_from_frame(&frame, &shown, &config).labels);
        frame.cand_cursor = Some(0);
        assert!(page_from_frame(&frame, &shown, &config).labels);
    }

    #[test]
    fn translate_puts_title_above_and_status_below() {
        let mut frame = Frame::default();
        frame.panel = Panel::Normal;
        frame.translate = Some(archtype_core::session::TranslateView {
            title: "中 → 英".to_string(),
            status: "回车上屏".to_string(),
            ..Default::default()
        });
        let page = page_from_frame(&frame, &candidates(&["你好"]), &Config::default());
        assert_eq!(page.aux_top.as_deref(), Some("中 → 英"));
        assert_eq!(page.aux_bottom.as_deref(), Some("回车上屏"));

        // 互译没有状态时不画下面一行
        frame.translate = Some(archtype_core::session::TranslateView {
            title: "中 → 英".to_string(),
            ..Default::default()
        });
        let page = page_from_frame(&frame, &[], &Config::default());
        assert!(page.aux_bottom.is_none());
        assert!(!page.is_empty());
    }

    #[test]
    fn toolbar_hint_goes_on_top() {
        let mut frame = Frame::default();
        frame.aux = "工具栏：数字选，Esc 关".to_string();
        let page = page_from_frame(&frame, &[], &Config::default());
        assert_eq!(page.aux_top.as_deref(), Some("工具栏：数字选，Esc 关"));
    }

    #[test]
    fn voice_hint_is_not_empty() {
        let page = voice_hint_page("正在听… ▁▂▃", 17.0);
        assert!(!page.is_empty());
        assert!(page.texts.is_empty());
        assert!(!page.labels);
    }
}
