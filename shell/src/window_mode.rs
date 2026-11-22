//! 换到别的应用、别的窗口了没有（`WindowMode`，#254）。
//!
//! 外壳在输入控制器启用时把「哪个应用的哪个窗口」交过来，和上一次启用时的比：
//! 不一样就按 `WindowMode` 定一次中 / 英。同一个窗口里换输入框不算。

/// 上一次启用 ArchType 的应用和窗口；还没有过是 None。
#[derive(Debug, Default)]
pub struct WindowFocus {
    last: Option<String>,
}

impl WindowFocus {
    pub fn new() -> Self {
        Self { last: None }
    }

    /// 应用（bundle id）和它最前面的窗口号拼成一个键；问不到窗口号时只看应用。
    pub fn key(app: &str, window: Option<isize>) -> String {
        match window {
            Some(number) if number != 0 => format!("{app}#{number}"),
            _ => app.to_string(),
        }
    }

    /// 记下这一次的应用和窗口，返回是不是换了（第一次也算换了）。
    pub fn moved(&mut self, key: &str) -> bool {
        let moved = self.last.as_deref() != Some(key);
        self.last = Some(key.to_string());
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_time_counts_as_moved() {
        let mut focus = WindowFocus::new();
        assert!(focus.moved("com.apple.Terminal#1"));
        assert!(!focus.moved("com.apple.Terminal#1"));
        assert!(focus.moved("com.apple.Terminal#2"));
        assert!(focus.moved("com.apple.Safari#1"));
    }

    #[test]
    fn key_falls_back_to_the_app_without_a_window_number() {
        assert_eq!(WindowFocus::key("a", Some(7)), "a#7");
        assert_eq!(WindowFocus::key("a", None), "a");
        assert_eq!(WindowFocus::key("a", Some(0)), "a");
    }
}
