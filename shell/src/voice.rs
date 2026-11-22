//! 语音输入：复用核心的语音句柄（`archtype_voice_*`）：热键状态机、麦克风、微信离线模型、上屏。
//!
//! 外壳只做两件事：按住语音键时告诉它（key_down / key_up），开着定时器 poll 状态；
//! 状态到 RESULT 就把 `text` 上屏。模型在设置界面里下载，这里从不下载。

use std::ffi::{CStr, CString};
use std::path::Path;

use archtype_core::voice_ffi::{
    archtype_voice_cancel, archtype_voice_free, archtype_voice_is_tap, archtype_voice_key_down,
    archtype_voice_key_up, archtype_voice_level, archtype_voice_model_state, archtype_voice_new,
    archtype_voice_poll, archtype_voice_set_device, archtype_voice_text, ArchtypeVoice,
};

pub use archtype_core::voice_ffi::{
    VOICE_ERROR, VOICE_IDLE, VOICE_LISTENING, VOICE_RECOGNIZING, VOICE_RESULT,
};

pub struct Voice {
    handle: *mut ArchtypeVoice,
}

// SAFETY: 语音句柄内部有自己的锁和录音线程；外壳只在主线程上调这几个函数。
unsafe impl Send for Voice {}
unsafe impl Sync for Voice {}

impl Voice {
    /// 建语音句柄；`model_dir` 是数据目录下的 wtlocal（模型解包后的目录）。
    /// 不碰麦克风、不读模型，第一次按热键时才读。
    pub fn new(model_dir: &Path) -> Option<Self> {
        let dir = CString::new(model_dir.to_string_lossy().as_bytes()).ok()?;
        let handle = unsafe { archtype_voice_new(dir.as_ptr()) };
        if handle.is_null() {
            return None;
        }
        Some(Self { handle })
    }

    /// 指定麦克风（cpal 的设备名）；空串 = 系统默认。下一次开麦生效。
    pub fn set_device(&self, name: &str) {
        let Ok(name) = CString::new(name) else {
            return;
        };
        unsafe { archtype_voice_set_device(self.handle, name.as_ptr()) };
    }

    pub fn key_down(&self) {
        unsafe { archtype_voice_key_down(self.handle) };
    }

    pub fn key_up(&self) {
        unsafe { archtype_voice_key_up(self.handle) };
    }

    pub fn cancel(&self) {
        unsafe { archtype_voice_cancel(self.handle) };
    }

    /// 定时调用：推进状态机（点按模式的静音结束），返回 `VOICE_*` 状态。
    pub fn poll(&self) -> i32 {
        unsafe { archtype_voice_poll(self.handle) }
    }

    /// 上一次 poll 对应的文本。
    pub fn text(&self) -> String {
        let ptr = unsafe { archtype_voice_text(self.handle) };
        if ptr.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }

    /// 正在听时最近的音量，0–100。
    pub fn level(&self) -> i32 {
        unsafe { archtype_voice_level(self.handle) }
    }

    /// 模型状态（`MODEL_*`）。
    pub fn model_state(&self) -> i32 {
        unsafe { archtype_voice_model_state(self.handle) }
    }

    /// 是不是点按模式（松开热键后还在听）。
    pub fn is_tap(&self) -> bool {
        unsafe { archtype_voice_is_tap(self.handle) != 0 }
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        unsafe { archtype_voice_free(self.handle) };
    }
}

/// 模型目录（数据目录下的 wtlocal，和设置界面下载到的地方一致）。
pub fn model_dir(support_dir: &Path) -> std::path::PathBuf {
    support_dir.join("wtlocal")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_without_a_model_and_key_events_do_not_panic() {
        let dir = std::env::temp_dir().join(format!("archtype-macos-voice-{}", std::process::id()));
        let voice = Voice::new(&model_dir(&dir)).expect("建语音句柄");
        assert_eq!(voice.poll(), VOICE_IDLE);
        assert_eq!(voice.level(), 0);
        assert!(voice.text().is_empty());
        // 没有模型时按键不崩，状态回到 idle/error
        voice.key_down();
        let state = voice.poll();
        assert!(state == VOICE_IDLE || state == VOICE_ERROR, "state {state}");
        voice.key_up();
        voice.cancel();
        assert_eq!(voice.poll(), VOICE_IDLE);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
