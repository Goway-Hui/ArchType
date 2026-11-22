//! 打字统计：直接复用核心的记录器（`archtype_stats_*`，Linux 插件、命令行和 macOS 共用一套）。
//!
//! 口径：聚合计数（按天、按字符类别、按键热力图、候选位置），不保存打过的文字；
//! 密码框（安全输入）里的按键和上屏不算。统计文件是数据目录下的 stats.json，
//! 设置界面的「统计」页读同一份。

use std::ffi::CString;
use std::path::Path;

use archtype_core::stats::SENSITIVE;
use archtype_core::stats_ffi::{
    archtype_stats_commit, archtype_stats_enabled, archtype_stats_flush, archtype_stats_free,
    archtype_stats_key, archtype_stats_new, archtype_stats_now_ms, archtype_stats_set_enabled,
    archtype_stats_wait, ArchtypeStats,
};

/// macOS 的按键码平台编号（`ArchtypeStats::key` 的 platform 参数）
const PLATFORM_MACOS: i32 = 1;

pub struct Stats {
    handle: *mut ArchtypeStats,
}

// SAFETY: 记录器内部有自己的锁和写文件线程；外壳只在主线程上调这几个函数。
unsafe impl Send for Stats {}
unsafe impl Sync for Stats {}

impl Stats {
    /// 建一个记录器（默认关着）；`path` 是 stats.json 的绝对路径。
    pub fn new(path: &Path, enabled: bool) -> Option<Self> {
        let c_path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        let handle = unsafe { archtype_stats_new(c_path.as_ptr()) };
        if handle.is_null() {
            return None;
        }
        let stats = Self { handle };
        stats.set_enabled(enabled);
        Some(stats)
    }

    pub fn set_enabled(&self, enabled: bool) {
        unsafe { archtype_stats_set_enabled(self.handle, i32::from(enabled)) };
    }

    pub fn enabled(&self) -> bool {
        unsafe { archtype_stats_enabled(self.handle) != 0 }
    }

    /// 一次物理按键（NSEvent.keyCode）。
    pub fn record_key(&self, key_code: u16, sensitive: bool) {
        if !self.enabled() || sensitive {
            return;
        }
        unsafe {
            archtype_stats_key(
                self.handle,
                PLATFORM_MACOS,
                u32::from(key_code),
                1,
                0,
                archtype_stats_now_ms(),
            );
        }
    }

    /// 一次上屏（只数类别，不留文字）。`candidate_index` 是候选位置（1 起），不是选候选时为 0。
    pub fn record_commit(
        &self,
        text: &str,
        source: &str,
        scheme: &str,
        candidate_index: usize,
        sensitive: bool,
    ) {
        if !self.enabled() || sensitive {
            return;
        }
        let Ok(text) = CString::new(text) else {
            return;
        };
        let source = CString::new(source).unwrap_or_default();
        let scheme = CString::new(scheme).unwrap_or_default();
        unsafe {
            archtype_stats_commit(
                self.handle,
                text.as_ptr(),
                source.as_ptr(),
                scheme.as_ptr(),
                candidate_index as i32,
                0,
                archtype_stats_now_ms(),
            );
        }
    }

    /// 把攒着的计数交给写文件线程（失焦、退出时调）。
    pub fn flush(&self) {
        if !self.enabled() {
            return;
        }
        unsafe {
            archtype_stats_flush(self.handle, archtype_stats_now_ms());
        }
    }

    /// 等写文件线程写完（退出前用）。
    pub fn wait(&self, timeout_ms: i64) {
        unsafe {
            archtype_stats_wait(self.handle, timeout_ms);
        }
    }
}

impl Drop for Stats {
    fn drop(&mut self) {
        unsafe { archtype_stats_free(self.handle) };
    }
}

/// 统计文件的路径（数据目录下的 stats.json）。
pub fn stats_path(support_dir: &Path) -> std::path::PathBuf {
    support_dir.join("stats.json")
}

/// 敏感输入时给记录器用的标志（密码框、终端的安全键盘输入）。
pub const SENSITIVE_FLAG: u32 = SENSITIVE;

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "archtype-macos-stats-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn records_keys_and_commits_when_enabled() {
        let dir = temp_dir("on");
        let path = stats_path(&dir);
        let stats = Stats::new(&path, true).expect("建记录器");
        assert!(stats.enabled());
        stats.record_key(0x00, false);
        stats.record_commit("你好", "pinyin", "quanpin", 1, false);
        stats.flush();
        stats.wait(5000);
        assert!(path.exists(), "统计文件没写出来");

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"chars\""), "{text}");
        assert!(text.contains("\"keys\""), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_and_sensitive_input_are_not_recorded() {
        let dir = temp_dir("off");
        let path = stats_path(&dir);
        let stats = Stats::new(&path, false).expect("建记录器");
        assert!(!stats.enabled());
        stats.record_key(0x00, false);
        stats.record_commit("你好", "", "", 0, false);
        stats.flush();
        stats.wait(1000);
        assert!(!path.exists(), "统计关着也写了文件");

        stats.set_enabled(true);
        stats.record_key(0x00, true);
        stats.record_commit("秘密", "", "", 0, true);
        stats.flush();
        stats.wait(1000);
        assert!(!path.exists() || !std::fs::read_to_string(&path).unwrap().contains("chars"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
