//! 语音触发状态机：长按说话 / 点按说话。
//!
//! 语义（对齐主流输入法习惯）：
//! - 按下即开始录音
//! - 松开时按住时长 < hold_threshold(默认 300ms) => 判定为「点按」，
//!   继续录音，直到再次点按或静音超时
//! - 松开时按住时长 >= hold_threshold => 判定为「长按」，立即结束
//! - 同一个键可以同时充当长按键和点按键

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Hold,
    Tap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// 开始录音
    Start,
    /// 继续录音（点按已识别，等待静音或再次点按）
    Continue,
    /// 结束录音
    Stop,
}

/// 长按模式最长录多久；松手事件丢失时的兜底
const MAX_HOLD: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug)]
pub struct TriggerConfig {
    pub hold_threshold: Duration,
    pub silence_timeout: Duration,
    /// 松开时短按是否算点按（`--mode hold` 时 false：松手就结束）
    pub tap: bool,
    /// 一次会话结束后的冷却：这期间再按不开始新会话。
    /// 上一句的定稿（整段推理）可能还没跑完，马上开新会话会把它的结果顶掉；
    /// 顺带把按键抖动（按下-松开-按下）也挡在外面。
    pub cooldown: Duration,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            hold_threshold: Duration::from_millis(300),
            silence_timeout: Duration::from_millis(1500),
            tap: true,
            cooldown: Duration::from_millis(450),
        }
    }
}

#[derive(Debug, Default)]
pub struct VoiceTrigger {
    cfg: TriggerConfig,
    recording: bool,
    mode: Mode,
    pressed_at: Option<Instant>,
    last_voice: Option<Instant>,
    /// 上一次会话结束的时间（冷却用）
    last_stop: Option<Instant>,
}

impl VoiceTrigger {
    pub fn new(cfg: TriggerConfig) -> Self {
        Self {
            cfg,
            recording: false,
            mode: Mode::Hold,
            pressed_at: None,
            last_voice: None,
            last_stop: None,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// 键按下
    pub fn on_down(&mut self, now: Instant) -> Vec<Action> {
        if let Some(stop) = self.last_stop {
            if now.saturating_duration_since(stop) < self.cfg.cooldown {
                return Vec::new();
            }
        }
        if self.recording {
            // 点按模式下再次按下 = 提前结束
            if self.mode == Mode::Tap {
                self.reset();
                self.last_stop = Some(now);
                return vec![Action::Stop];
            }
            return Vec::new();
        }
        self.recording = true;
        self.mode = Mode::Hold;
        self.pressed_at = Some(now);
        self.last_voice = Some(now);
        vec![Action::Start]
    }

    /// 键松开
    pub fn on_up(&mut self, now: Instant) -> Vec<Action> {
        let Some(pressed) = self.pressed_at.take() else {
            return Vec::new();
        };
        if !self.recording {
            return Vec::new();
        }
        let held = now.saturating_duration_since(pressed);
        if self.cfg.tap && held < self.cfg.hold_threshold {
            self.mode = Mode::Tap;
            self.last_voice = Some(now);
            vec![Action::Continue]
        } else {
            self.reset();
            self.last_stop = Some(now);
            vec![Action::Stop]
        }
    }

    /// 有语音活动（VAD 判定为说话）
    pub fn note_voice(&mut self, now: Instant) {
        if self.recording {
            self.last_voice = Some(now);
        }
    }

    /// 周期调用；点按模式下静音超时、长按模式下 key-up 丢失超时都返回 Stop
    pub fn tick(&mut self, now: Instant) -> Option<Action> {
        if !self.recording {
            return None;
        }
        if self.mode == Mode::Tap {
            let last = self.last_voice.unwrap_or(now);
            if now.saturating_duration_since(last) >= self.cfg.silence_timeout {
                self.reset();
                self.last_stop = Some(now);
                return Some(Action::Stop);
            }
            return None;
        }
        // 长按模式靠松手结束；松手事件丢了（焦点切换、X11 抓键异常、Wayland 转发丢失）
        // 时也要收尾，不能一直录下去
        if let Some(pressed) = self.pressed_at {
            if now.saturating_duration_since(pressed) >= MAX_HOLD {
                self.reset();
                self.last_stop = Some(now);
                return Some(Action::Stop);
            }
        }
        None
    }

    fn reset(&mut self) {
        self.recording = false;
        self.mode = Mode::Hold;
        self.pressed_at = None;
        self.last_voice = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> TriggerConfig {
        TriggerConfig {
            hold_threshold: Duration::from_millis(300),
            silence_timeout: Duration::from_millis(1500),
            tap: true,
            cooldown: Duration::from_millis(450),
        }
    }

    #[test]
    fn hold_stops_on_release() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        assert_eq!(tr.on_down(t0), vec![Action::Start]);
        assert!(tr.is_recording());
        assert_eq!(
            tr.on_up(t0 + Duration::from_millis(600)),
            vec![Action::Stop]
        );
        assert!(!tr.is_recording());
    }

    #[test]
    fn tap_keeps_recording() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        assert_eq!(tr.on_down(t0), vec![Action::Start]);
        assert_eq!(
            tr.on_up(t0 + Duration::from_millis(100)),
            vec![Action::Continue]
        );
        assert!(tr.is_recording());
        assert_eq!(tr.mode(), Mode::Tap);
    }

    #[test]
    fn tap_stops_on_silence() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        tr.on_up(t0 + Duration::from_millis(100));
        assert_eq!(tr.tick(t0 + Duration::from_millis(1000)), None);
        assert_eq!(
            tr.tick(t0 + Duration::from_millis(1700)),
            Some(Action::Stop)
        );
        assert!(!tr.is_recording());
    }

    #[test]
    fn voice_activity_extends_tap() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        tr.on_up(t0 + Duration::from_millis(100));
        tr.note_voice(t0 + Duration::from_millis(1500));
        assert_eq!(tr.tick(t0 + Duration::from_millis(2000)), None);
        assert_eq!(
            tr.tick(t0 + Duration::from_millis(3100)),
            Some(Action::Stop)
        );
    }

    #[test]
    fn second_tap_stops_immediately() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        tr.on_up(t0 + Duration::from_millis(80));
        assert_eq!(
            tr.on_down(t0 + Duration::from_millis(500)),
            vec![Action::Stop]
        );
        assert!(!tr.is_recording());
    }

    #[test]
    fn repeated_down_while_holding_is_ignored() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        assert!(tr.on_down(t0 + Duration::from_millis(50)).is_empty());
        assert!(tr.is_recording());
    }

    #[test]
    fn cooldown_blocks_restart_right_after_stop() {
        // 上一句刚结束（整段定稿可能还在跑）时按下去不算新会话；
        // 冷却过了再按才重新开始
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        assert_eq!(
            tr.on_up(t0 + Duration::from_millis(600)),
            vec![Action::Stop]
        );
        assert!(tr.on_down(t0 + Duration::from_millis(700)).is_empty());
        assert!(!tr.is_recording());
        assert_eq!(
            tr.on_down(t0 + Duration::from_millis(1100)),
            vec![Action::Start]
        );
        assert!(tr.is_recording());
    }

    #[test]
    fn cooldown_also_covers_tap_stop() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        tr.on_up(t0 + Duration::from_millis(80));
        assert_eq!(
            tr.on_down(t0 + Duration::from_millis(500)),
            vec![Action::Stop]
        );
        assert!(tr.on_down(t0 + Duration::from_millis(600)).is_empty());
        assert_eq!(
            tr.on_down(t0 + Duration::from_millis(1000)),
            vec![Action::Start]
        );
    }

    #[test]
    fn up_without_down_is_noop() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        assert!(tr.on_up(t0).is_empty());
        assert!(!tr.is_recording());
    }

    #[test]
    fn tick_when_idle_is_none() {
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        assert_eq!(tr.tick(t0 + Duration::from_secs(10)), None);
    }

    #[test]
    fn hold_mode_tick_never_stops() {
        // 长按模式靠松手结束，不该被静音超时打断
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        assert_eq!(tr.tick(t0 + Duration::from_secs(5)), None);
        assert!(tr.is_recording());
    }

    #[test]
    fn hold_mode_stops_when_key_up_is_lost() {
        // 松手事件丢了（焦点切换、抓键异常）也要收尾，不能一直录
        let t0 = Instant::now();
        let mut tr = VoiceTrigger::new(cfg());
        tr.on_down(t0);
        assert_eq!(tr.tick(t0 + Duration::from_secs(119)), None);
        assert!(tr.is_recording());
        assert_eq!(tr.tick(t0 + Duration::from_secs(121)), Some(Action::Stop));
        assert!(!tr.is_recording());
    }
}
