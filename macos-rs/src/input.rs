//! 会话驱动：把外壳的按键交给核心的 [`Session`]，把结果（编辑区、候选、上屏）交回外壳。
//!
//! 平台无关：Linux 上也能编、能测。macOS 层只负责把 NSEvent 翻成 [`Key`]，
//! 再把 [`Frame`] 画到候选框 / 交给输入框。

use std::path::PathBuf;

use archtype_core::session::{Candidate, Frame, Key, Mode, Session};
use archtype_core::Engine;

use archtype_core::translate::Translator;

use crate::config::Config;
use crate::engine::{apply_engine_settings, load_engine, EngineStore, Layout};
use crate::translate::translate_config;

/// 每上屏这么多次存一次（系统可能随时杀掉输入法进程）。
const SAVE_EVERY: usize = 20;

/// 一个输入框一份：引擎（进程里共用的那一个）和会话（编辑区、候选、已选词）。
pub struct Input {
    layout: Layout,
    config: Config,
    commits_since_save: usize,
    /// 自己载入的引擎（测试、自检用）；进程里共用引擎时是 None（见 `attach`）。
    owned_engine: Option<Box<Engine>>,
    /// 进程里共用的引擎（`attach` 传进来的）。
    shared_engine: Option<EngineStore>,
    /// 进程里共用的翻译器（`attach` 传进来的，所有输入框共用一个本地 server）
    shared_translator: Option<&'static Translator>,
    session: Session<'static>,
}

impl Input {
    /// 用现成的引擎建会话（引擎由进程共用的那一份 load 好）。
    ///
    /// 引擎必须在 `'static` 的地方（进程级 `OnceLock<Box<Engine>>`，永不释放），
    /// 外壳在配置变了的时候可以往它上面重新套设置（[`apply_engine_settings`]）。
    pub fn attach(
        engine: EngineStore,
        translator: Option<&'static Translator>,
        layout: Layout,
        config: Config,
    ) -> Self {
        let mut session = Session::with_engine(engine.get());
        session
            .set_translator(translator.map(|t| t as &dyn archtype_core::session::TranslateBackend));
        let mut input = Self {
            layout,
            config: config.clone(),
            commits_since_save: 0,
            owned_engine: None,
            shared_engine: Some(engine),
            shared_translator: translator,
            session,
        };
        input.apply_config(&config, false, false);
        input
    }

    /// 自己载入一份引擎再建会话（测试、自检用；输入法进程里用 `attach` 共用引擎）。
    pub fn load(layout: Layout, config: Config) -> Result<Self, String> {
        let engine = Box::new(load_engine(&layout, &config)?);
        // SAFETY: engine 在 Box 里，地址不会变；Drop 时先 set_engine(None)，之后才 drop。
        let reference: &'static Engine = unsafe { &*(&*engine as *const Engine) };
        let session = Session::with_engine(reference);
        let mut input = Self {
            layout,
            config: config.clone(),
            commits_since_save: 0,
            owned_engine: Some(engine),
            shared_engine: None,
            shared_translator: None,
            session,
        };
        input.apply_config(&config, false, false);
        Ok(input)
    }

    /// 设置变了：引擎的（模糊音、双拼、繁体、五笔）和会话的（页大小、标点、方向键……）。
    ///
    /// `translate_available`：外壳配好了翻译服务。`auto_pair_ok`：这个输入框能按位置改字。
    /// 返回要记日志的问题。
    pub fn apply_config(
        &mut self,
        config: &Config,
        translate_available: bool,
        auto_pair_ok: bool,
    ) -> Vec<String> {
        self.config = config.clone();
        let engine = match self.owned_engine.as_deref_mut() {
            Some(engine) => engine,
            None => {
                let shared = self.shared_engine.expect("attach 时给了共用引擎");
                shared.get_mut()
            }
        };
        let problems = apply_engine_settings(engine, config, &self.layout);
        if let Some(translator) = self.shared_translator {
            translator.configure(translate_config(config, &self.layout));
        }
        self.session
            .configure(config.session_config(translate_available, auto_pair_ok));
        problems
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn engine(&self) -> &Engine {
        match (self.owned_engine.as_deref(), self.shared_engine) {
            (Some(engine), _) => engine,
            (None, Some(engine)) => engine.get(),
            (None, None) => unreachable!("会话没有引擎"),
        }
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// 一个键交给会话。
    pub fn key(&mut self, key: &Key) -> Frame {
        let frame = self.session.key(key);
        self.after_frame(&frame);
        frame
    }

    /// 时间到了（双击 Shift 的等待、互译轮询）：会话按 `wake_after_ms` 让外壳叫它。
    pub fn tick(&mut self, now_ms: u64) -> Frame {
        let frame = self.session.tick(now_ms);
        self.after_frame(&frame);
        frame
    }

    pub fn mode(&self) -> Mode {
        self.session.mode()
    }

    pub fn set_mode(&mut self, mode: Mode) -> Frame {
        let frame = self.session.set_mode(mode);
        self.after_frame(&frame);
        frame
    }

    pub fn composing(&self) -> bool {
        self.session.composing()
    }

    pub fn input(&self) -> &str {
        self.session.input()
    }

    pub fn selected_text(&self) -> String {
        self.session.selected_text()
    }

    /// 现在这一页（或展开面板的这一屏）要画的候选。
    pub fn shown(&mut self) -> &[Candidate] {
        self.session.shown()
    }

    /// 这次上屏的输入来源（统计口径：quanpin/shuangpin/wubi/english/translate）。
    pub fn commit_source(&self) -> &'static str {
        match self.session.mode() {
            Mode::English => "english",
            Mode::Translate => "translate",
            Mode::Chinese => {
                if self.config.wubi() {
                    "wubi"
                } else if self.config.shuangpin() {
                    "shuangpin"
                } else {
                    "quanpin"
                }
            }
        }
    }

    /// 双拼方案名（只有双拼时统计才记）。
    pub fn commit_scheme(&self) -> String {
        if self.config.shuangpin() {
            self.config.shuangpin_scheme.clone()
        } else {
            String::new()
        }
    }

    /// 这次上屏的候选是不是 emoji（是的话返回 emoji 本身，外壳记进最近用过的）。
    pub fn last_choice_emoji(&self) -> Option<String> {
        self.session
            .last_choice()
            .filter(|candidate| !candidate.emoji.is_empty())
            .map(|candidate| candidate.emoji.clone())
    }

    /// 上屏的是第几个候选（1 起）；原样上屏输入、标点、emoji 时是 0。
    pub fn last_choice_index(&self) -> usize {
        self.session.last_choice_index()
    }

    /// 选候选框里显示的第 `index` 个（鼠标点击用）。
    pub fn select(&mut self, index: usize) -> Frame {
        let frame = self.session.select(index);
        self.after_frame(&frame);
        frame
    }

    pub fn frame(&mut self, handled: bool) -> Frame {
        self.session.frame(handled)
    }

    pub fn reset(&mut self, commit_pending: bool) -> Frame {
        let frame = self.session.reset(commit_pending);
        self.after_frame(&frame);
        frame
    }

    /// 上屏以后：攒够次数存用户词库和输入习惯（忘了学过的东西时马上存）。
    fn after_frame(&mut self, frame: &Frame) {
        if frame.commit.is_none() && !frame.learned && !frame.forgot {
            return;
        }
        if frame.forgot {
            self.save();
            return;
        }
        self.commits_since_save += 1;
        if self.commits_since_save >= SAVE_EVERY {
            self.save();
        }
    }

    /// 存用户词库和输入习惯。输入法进程可能随时被杀，外壳在退出、切输入法时也调它。
    pub fn save(&mut self) {
        self.commits_since_save = 0;
        let engine = self.engine();
        let _ = engine.save_user_dict_async(self.layout.user_dict_path());
        let _ = engine.save_history_async(self.layout.history_path());
    }

    /// 等后台保存写完（退出前用）。
    pub fn save_wait(&self, timeout: std::time::Duration) {
        let _ = self.engine().save_wait(Some(timeout));
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        // 先把会话对引擎、翻译器的引用卸掉，再按字段顺序 drop
        self.session.set_translator(None);
        self.session.set_engine(None);
    }
}

/// 把外壳给的按键拼成核心的 [`Key`]。
pub fn make_key(
    keysym: u32,
    modifiers: u32,
    release: bool,
    prev_char: Option<char>,
    next_char: Option<char>,
    hints: u32,
    code: u32,
    repeat: bool,
    time_ms: u64,
) -> Key {
    Key {
        keysym,
        modifiers,
        release,
        prev_char,
        action: Default::default(),
        hints,
        code,
        repeat,
        time_ms,
        next_char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{KeyInput, Modifiers};
    use archtype_core::session::{key_hints, keysym, Mode};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "archtype-macos-input-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn input(name: &str) -> Input {
        let resource = temp_dir(&format!("{name}-res"));
        let support = temp_dir(&format!("{name}-data"));
        std::fs::write(
            resource.join("lexicon.tsv"),
            "ni hao\t你好\t900000\nzhong guo\t中国\t800000\n",
        )
        .unwrap();
        let layout = Layout::new(resource, support);
        Input::load(layout, Config::default()).expect("载入引擎")
    }

    /// 按一串键，和 key_harness 的 type() 一样：<sp> <bs> <cr> <esc> 是特殊键。
    fn type_keys(input: &mut Input, keys: &str) -> Frame {
        let mut frame = input.frame(false);
        let mut rest = keys;
        while let Some(c) = rest.chars().next() {
            let key = if c == '<' {
                let end = rest.find('>').expect("特殊键没有 >");
                let name = &rest[1..end];
                rest = &rest[end + 1..];
                match name {
                    "sp" => KeyInput::key(keysym::SPACE, Modifiers::NONE),
                    "bs" => KeyInput::key(keysym::BACKSPACE, Modifiers::NONE),
                    "cr" => KeyInput::key(keysym::RETURN, Modifiers::NONE),
                    "esc" => KeyInput::key(keysym::ESCAPE, Modifiers::NONE),
                    "left" => KeyInput::key(keysym::LEFT, Modifiers::NONE),
                    "right" => KeyInput::key(keysym::RIGHT, Modifiers::NONE),
                    "up" => KeyInput::key(keysym::UP, Modifiers::NONE),
                    "down" => KeyInput::key(keysym::DOWN, Modifiers::NONE),
                    other => panic!("不认识的键 {other}"),
                }
            } else {
                rest = &rest[c.len_utf8()..];
                KeyInput::char(c, Modifiers::NONE)
            };
            frame = input.key(&make_key(
                key.keysym,
                key.modifiers.raw(),
                false,
                None,
                None,
                0,
                0,
                false,
                0,
            ));
        }
        frame
    }

    #[test]
    fn types_nihao_and_commits_hello() {
        let mut input = input("nihao");
        let frame = type_keys(&mut input, "nihao");
        assert_eq!(frame.preedit, "ni'hao");
        assert!(frame.cand_count > 0);
        assert_eq!(input.shown()[0].text, "你好");
        let frame = type_keys(&mut input, "<sp>");
        assert_eq!(frame.commit.as_deref(), Some("你好"));
        assert!(!input.composing());
    }

    #[test]
    fn digits_pick_a_candidate() {
        let mut input = input("digit");
        type_keys(&mut input, "zhongguo");
        let frame = type_keys(&mut input, "1");
        assert_eq!(frame.commit.as_deref(), Some("中国"));
    }

    #[test]
    fn backspace_edits_the_preedit() {
        let mut input = input("bs");
        type_keys(&mut input, "nihao");
        let frame = type_keys(&mut input, "<bs>");
        assert_eq!(frame.preedit, "ni'ha");
        assert!(input.composing());
    }

    #[test]
    fn escape_clears_the_composition() {
        let mut input = input("esc");
        type_keys(&mut input, "nihao");
        let frame = type_keys(&mut input, "<esc>");
        assert!(frame.commit.is_none());
        assert!(!input.composing());
    }

    #[test]
    fn english_mode_passes_letters_through() {
        let mut input = input("en");
        input.set_mode(Mode::English);
        let frame = type_keys(&mut input, "a");
        assert!(!frame.handled);
        assert_eq!(input.mode(), Mode::English);
    }

    #[test]
    fn password_hint_does_not_learn() {
        let mut input = input("secure");
        type_keys(&mut input, "nihao");
        let frame = input.key(&make_key(
            keysym::SPACE,
            0,
            false,
            None,
            None,
            key_hints::NO_LEARN,
            0,
            false,
            0,
        ));
        assert_eq!(frame.commit.as_deref(), Some("你好"));
        assert!(!frame.learned);
    }

    #[test]
    fn save_writes_user_dict_and_history() {
        let mut input = input("save");
        let support = input.layout().support_dir.clone();
        type_keys(&mut input, "nihao<sp>");
        input.save();
        input.save_wait(std::time::Duration::from_secs(5));
        assert!(support.join("user.tsv").exists() || support.join("history.tsv").exists());
    }
}
