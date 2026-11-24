//! 设置：~/Library/Application Support/ArchType/archtype.conf。
//!
//! 键名、取值、默认值和 Linux 的 ~/.config/fcitx5/conf/archtype.conf 一样，一份配置两边都能用。
//! 以 spec/config.json 为准（三端的测试都拿它对照）：这里每一项的默认值都能在 spec 里找到，
//! 单测会读 spec 逐项核对。
//!
//! 平台无关：Linux 上也能编、能测。

use archtype_core::session::{self, ArrowKeys, PunctStyle, SelectKeys, ShiftToggle};

use crate::keys::Shortcut;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputScheme {
    Quanpin,
    Shuangpin,
    Wubi86,
}

impl InputScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quanpin => "Quanpin",
            Self::Shuangpin => "Shuangpin",
            Self::Wubi86 => "Wubi86",
        }
    }

    /// 设置窗口里的名字（和 Linux 设置、fcitx5 的枚举名一样）
    pub fn name(self) -> &'static str {
        match self {
            Self::Quanpin => "拼音输入",
            Self::Shuangpin => "双拼输入",
            Self::Wubi86 => "五笔输入",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Quanpin" => Some(Self::Quanpin),
            "Shuangpin" => Some(Self::Shuangpin),
            "Wubi86" => Some(Self::Wubi86),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitialMode {
    Chinese,
    English,
    Last,
}

impl InitialMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chinese => "Chinese",
            Self::English => "English",
            Self::Last => "Last",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Chinese" => Some(Self::Chinese),
            "English" => Some(Self::English),
            "Last" => Some(Self::Last),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowMode {
    Same,
    Chinese,
    English,
}

impl WindowMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Same => "Same",
            Self::Chinese => "Chinese",
            Self::English => "English",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Same" => Some(Self::Same),
            "Chinese" => Some(Self::Chinese),
            "English" => Some(Self::English),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTheme {
    System,
    Light,
    Dark,
}

impl SettingsTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "System" => Some(Self::System),
            "Light" => Some(Self::Light),
            "Dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

/// 模糊音分组的位掩码（和核心的 syllable::fuzzy、Linux 的 applyConfig 一致）。
pub mod fuzzy_groups {
    pub const ZH: u32 = 1;
    pub const CH: u32 = 2;
    pub const SH: u32 = 4;
    pub const NL: u32 = 8;
    pub const FH: u32 = 16;
    pub const RL: u32 = 32;
    pub const AN: u32 = 64;
    pub const EN: u32 = 128;
    pub const IN: u32 = 256;
    pub const IAN: u32 = 512;
    pub const UAN: u32 = 1024;
    pub const UN: u32 = 2048;
    pub const AN_AI: u32 = 4096;
    pub const HUANG_WANG: u32 = 8192;
    pub const UN_IONG: u32 = 16384;
    pub const ENG_ONG: u32 = 32768;
}

/// 双拼方案在配置文件里的名字（和 Linux 一样），核心也认小写、别名。
pub const SHUANGPIN_SCHEMES: [&str; 7] = [
    "Ziranma",
    "Xiaohe",
    "Microsoft",
    "Sogou",
    "ABC",
    "Ziguang",
    "PinyinJiajia",
];

/// 互译语言：设置窗口里的名字、配置文件里的代码。
pub const TRANSLATE_LANGUAGES: [(&str, &str); 13] = [
    ("英语", "en"),
    ("日语", "ja"),
    ("韩语", "ko"),
    ("法语", "fr"),
    ("德语", "de"),
    ("西班牙语", "es"),
    ("俄语", "ru"),
    ("葡萄牙语", "pt"),
    ("意大利语", "it"),
    ("越南语", "vi"),
    ("泰语", "th"),
    ("印尼语", "id"),
    ("阿拉伯语", "ar"),
];

pub const TRANSLATE_TARGETS: [&str; 13] = [
    "en", "ja", "ko", "fr", "de", "es", "ru", "pt", "it", "vi", "th", "id", "ar",
];

/// 设置窗口的强调色。三端用同一张表（spec/config.json 的 SettingsAccent）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccentPreset {
    pub value: &'static str,
    pub name: &'static str,
    pub color: u32,
    pub strong: u32,
    pub light: u32,
}

pub const ACCENT_PRESETS: [AccentPreset; 5] = [
    AccentPreset {
        value: "Green",
        name: "绿色",
        color: 0x0eaf76,
        strong: 0x0a8358,
        light: 0x1ab37d,
    },
    AccentPreset {
        value: "Blue",
        name: "蓝色",
        color: 0x3584e4,
        strong: 0x2d70c2,
        light: 0x68a3eb,
    },
    AccentPreset {
        value: "Purple",
        name: "紫色",
        color: 0x9141ac,
        strong: 0x9141ac,
        light: 0xbd8dcd,
    },
    AccentPreset {
        value: "Orange",
        name: "橙色",
        color: 0xe66100,
        strong: 0xb84e00,
        light: 0xec8840,
    },
    AccentPreset {
        value: "Pink",
        name: "粉色",
        color: 0xd56199,
        strong: 0xaa4e7a,
        light: 0xdd81ad,
    },
];

/// 所有认识的键，顺序同模板（文件里没有、又不是默认值的键照这个顺序补在后面）。
pub const KEYS: [&str; 71] = [
    "CandidateLayout",
    "ArrowKeys",
    "PageSize",
    "CandidateFontSize",
    "PageKeysMinusEqual",
    "PageKeysBracket",
    "PageKeysCommaPeriod",
    "PageKeysTab",
    "SelectKeys",
    "PickChar",
    "Association",
    "Fuzzy",
    "FuzzyZh",
    "FuzzyCh",
    "FuzzySh",
    "FuzzyNL",
    "FuzzyFH",
    "FuzzyRL",
    "FuzzyAn",
    "FuzzyEn",
    "FuzzyIn",
    "FuzzyIan",
    "FuzzyUan",
    "FuzzyUn",
    "FuzzyAnAi",
    "FuzzyHuangWang",
    "FuzzyUnIong",
    "FuzzyEngOng",
    "TypoCorrection",
    "RareExtChars",
    "InputScheme",
    "ShuangpinScheme",
    "ShuangpinShowKeys",
    "ShuangpinAbbrev",
    "WubiAutoCommit",
    "WubiHint",
    "ShiftToggle",
    "InitialMode",
    "WindowMode",
    "Traditional",
    "PunctuationStyle",
    "AutoPairPunct",
    "DateTime",
    "Calculator",
    "EmojiSuggest",
    "AutoSpace",
    "AutoSpaceLetters",
    "AutoSpaceDigits",
    "Lexicon",
    "Toolbar",
    "ClipboardHistory",
    "Voice",
    "VoiceKey",
    "VoiceCopy",
    "EmojiKey",
    "ToggleKey",
    "TranslateModeKey",
    "CheckUpdate",
    "BetaUpdates",
    "TypingStats",
    "SettingsTheme",
    "SettingsAccent",
    "Translate",
    "TranslateTarget",
    "TranslateBaseUrl",
    "TranslateModel",
    "TranslateFormal",
    "TranslateLocal",
    "TranslateLocalModel",
    "TranslateLlamaServer",
    "TranslateIdleMinutes",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub horizontal: bool,
    pub arrow_keys: ArrowKeys,
    pub page_size: usize,
    pub page_keys_minus: bool,
    pub page_keys_bracket: bool,
    pub page_keys_comma: bool,
    pub page_keys_tab: bool,
    pub fuzzy: bool,
    pub fuzzy_zh: bool,
    pub fuzzy_ch: bool,
    pub fuzzy_sh: bool,
    pub fuzzy_nl: bool,
    pub fuzzy_fh: bool,
    pub fuzzy_rl: bool,
    pub fuzzy_an: bool,
    pub fuzzy_en: bool,
    pub fuzzy_in: bool,
    pub fuzzy_ian: bool,
    pub fuzzy_uan: bool,
    pub fuzzy_un: bool,
    pub fuzzy_an_ai: bool,
    pub fuzzy_huang_wang: bool,
    pub fuzzy_un_iong: bool,
    pub fuzzy_eng_ong: bool,
    pub typo_correction: bool,
    pub rare_ext_chars: bool,
    pub input_scheme: InputScheme,
    pub shuangpin_scheme: String,
    /// 双拼时编辑区显示打的键（uivs），不翻成全拼（#122）
    pub shuangpin_show_keys: bool,
    /// 双拼的简拼：每个键都是声母时也当成只打了声母（sm -> 什么，#268）
    pub shuangpin_abbrev: bool,
    /// 五笔打满四码、只有一个字词时直接上屏
    pub wubi_auto_commit: bool,
    /// 五笔候选后面显示还要打的码
    pub wubi_hint: bool,
    pub shift_toggle: ShiftToggle,
    /// 切到 ArchType 时的模式（换窗口不算）；关了 Shift 切换、也没另设切换键时总是中文
    pub initial_mode: InitialMode,
    /// 换到别的应用、别的窗口时的中 / 英（#254）；Same 是每个应用记着自己的模式
    pub window_mode: WindowMode,
    pub traditional: bool,
    pub halfwidth_punct: bool,
    /// 中文标点自动配对：打 ( [ < { " ' 上屏成对的（）【】《》「」“”‘’
    pub auto_pair_punct: bool,
    pub select_keys: SelectKeys,
    pub pick_char: bool,
    /// 联想：上屏以后给出常跟在后面的词
    pub association: bool,
    pub date_time: bool,
    pub calculator: bool,
    pub emoji_suggest: bool,
    pub auto_space: bool,
    pub auto_space_letters: bool,
    pub auto_space_digits: bool,
    /// v + 空格打开工具栏（剪贴板、常用语、符号、表情；双拼、五笔时不开）
    pub toolbar: bool,
    /// 记录剪贴板历史（只在内存里，最多 20 条）
    pub clipboard_history: bool,
    /// 语音输入：按住 voice_key 说话，松开出字
    pub voice: bool,
    /// 语音键，fcitx5 键名：Alt_R（右 Option）、Control_R、F8…；留空不用
    pub voice_key: String,
    pub voice_copy: bool,
    pub emoji_key: Option<Shortcut>,
    pub toggle_key: Option<Shortcut>,
    pub translate_mode_key: Option<Shortcut>,
    pub check_update: bool,
    pub beta_updates: bool,
    pub typing_stats: bool,
    pub settings_theme: SettingsTheme,
    pub settings_accent: String,
    /// 词库文件，空串用安装包里的雾凇词库；文件不在也用自带的
    pub lexicon: String,
    /// 候选字号（磅），只有 macOS 有
    pub candidate_font_size: f64,
    pub translate: bool,
    pub translate_target: String,
    pub translate_base_url: String,
    pub translate_model: String,
    pub translate_formal: bool,
    /// 用本地模型（HY-MT），默认是；配置文件里没写这一项、又填了 TranslateBaseUrl 时是 False
    pub translate_local: bool,
    pub translate_local_model: String,
    pub translate_llama_server: String,
    pub translate_idle_minutes: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            horizontal: true,
            arrow_keys: ArrowKeys::Candidates,
            page_size: 9,
            page_keys_minus: true,
            page_keys_bracket: false,
            page_keys_comma: false,
            page_keys_tab: false,
            fuzzy: false,
            fuzzy_zh: true,
            fuzzy_ch: true,
            fuzzy_sh: true,
            fuzzy_nl: false,
            fuzzy_fh: false,
            fuzzy_rl: false,
            fuzzy_an: true,
            fuzzy_en: true,
            fuzzy_in: true,
            fuzzy_ian: true,
            fuzzy_uan: true,
            fuzzy_un: false,
            fuzzy_an_ai: false,
            fuzzy_huang_wang: false,
            fuzzy_un_iong: false,
            fuzzy_eng_ong: false,
            typo_correction: true,
            rare_ext_chars: false,
            input_scheme: InputScheme::Quanpin,
            shuangpin_scheme: "Ziranma".to_string(),
            shuangpin_show_keys: false,
            shuangpin_abbrev: true,
            wubi_auto_commit: true,
            wubi_hint: true,
            shift_toggle: ShiftToggle::Both,
            initial_mode: InitialMode::Chinese,
            window_mode: WindowMode::Same,
            traditional: false,
            halfwidth_punct: false,
            auto_pair_punct: false,
            select_keys: SelectKeys::Off,
            pick_char: true,
            association: false,
            date_time: true,
            calculator: true,
            emoji_suggest: true,
            auto_space: false,
            auto_space_letters: true,
            auto_space_digits: true,
            toolbar: true,
            clipboard_history: false,
            voice: true,
            voice_key: "Alt_R".to_string(),
            voice_copy: false,
            // macOS 的默认值是 Control+period（Linux 是 Super+period，见平台默认值）
            emoji_key: Some(Shortcut::EMOJI_DEFAULT),
            toggle_key: None,
            translate_mode_key: None,
            check_update: true,
            beta_updates: false,
            typing_stats: false,
            settings_theme: SettingsTheme::System,
            settings_accent: "Green".to_string(),
            lexicon: String::new(),
            candidate_font_size: 17.0,
            translate: false,
            translate_target: "en".to_string(),
            translate_base_url: String::new(),
            translate_model: String::new(),
            translate_formal: false,
            translate_local: true,
            translate_local_model: String::new(),
            translate_llama_server: String::new(),
            translate_idle_minutes: 5,
        }
    }
}

impl Config {
    /// 读配置文件；读不到（还没建）就是默认值。
    pub fn load(path: &str) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => Self::parse(&String::from_utf8_lossy(&bytes)),
            Err(_) => Self::default(),
        }
    }

    /// 解析 fcitx5 的 INI 格式：一行一个 Key=Value，# 开头是注释，值可以带双引号。
    /// 只认最外层的键（[分组] 下面的不是 ArchType 的设置）。同一个键写了几次，算最后一次。
    pub fn parse(text: &str) -> Self {
        let mut config = Self::default();
        let mut in_group = false;
        let mut saw_translate_local = false;
        let mut saw_input_scheme = false;
        let mut old_shuangpin = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') {
                in_group = true;
                continue;
            }
            if in_group {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = unquote(value.trim());
            if key == "Shuangpin" {
                // 0.3.12 及以前的双拼开关：只在没有 InputScheme 时算
                old_shuangpin = value == "True";
                continue;
            }
            config.set_value(key, &value);
            saw_translate_local |= key == "TranslateLocal";
            saw_input_scheme |= key == "InputScheme";
        }
        if !saw_translate_local {
            config.translate_local = implied_translate_local(&config.translate_base_url);
        }
        if !saw_input_scheme && old_shuangpin {
            config.input_scheme = InputScheme::Shuangpin;
        }
        config
    }

    /// 文件里没写 TranslateLocal 时它是什么：0.3.10 及以前没有这一项，已经填好翻译接口
    /// （TranslateBaseUrl）的接着用接口（False），没填的用默认的本地模型。三端一样。
    pub fn implied_translate_local(base_url: &str) -> bool {
        implied_translate_local(base_url)
    }

    /// 按文件里的写法改一项。不认识的键、认不出的值不管（保持原值）。
    pub fn set_value(&mut self, key: &str, value: &str) {
        match key {
            "PageKeysMinusEqual" => set_bool(&mut self.page_keys_minus, value),
            "PageKeysBracket" => set_bool(&mut self.page_keys_bracket, value),
            "PageKeysCommaPeriod" => set_bool(&mut self.page_keys_comma, value),
            "PageKeysTab" => set_bool(&mut self.page_keys_tab, value),
            "Fuzzy" => set_bool(&mut self.fuzzy, value),
            "FuzzyZh" => set_bool(&mut self.fuzzy_zh, value),
            "FuzzyCh" => set_bool(&mut self.fuzzy_ch, value),
            "FuzzySh" => set_bool(&mut self.fuzzy_sh, value),
            "FuzzyNL" => set_bool(&mut self.fuzzy_nl, value),
            "FuzzyFH" => set_bool(&mut self.fuzzy_fh, value),
            "FuzzyRL" => set_bool(&mut self.fuzzy_rl, value),
            "FuzzyAn" => set_bool(&mut self.fuzzy_an, value),
            "FuzzyEn" => set_bool(&mut self.fuzzy_en, value),
            "FuzzyIn" => set_bool(&mut self.fuzzy_in, value),
            "FuzzyIan" => set_bool(&mut self.fuzzy_ian, value),
            "FuzzyUan" => set_bool(&mut self.fuzzy_uan, value),
            "FuzzyUn" => set_bool(&mut self.fuzzy_un, value),
            "FuzzyAnAi" => set_bool(&mut self.fuzzy_an_ai, value),
            "FuzzyHuangWang" => set_bool(&mut self.fuzzy_huang_wang, value),
            "FuzzyUnIong" => set_bool(&mut self.fuzzy_un_iong, value),
            "FuzzyEngOng" => set_bool(&mut self.fuzzy_eng_ong, value),
            "TypoCorrection" => set_bool(&mut self.typo_correction, value),
            "RareExtChars" => set_bool(&mut self.rare_ext_chars, value),
            "ShuangpinShowKeys" => set_bool(&mut self.shuangpin_show_keys, value),
            "ShuangpinAbbrev" => set_bool(&mut self.shuangpin_abbrev, value),
            "WubiAutoCommit" => set_bool(&mut self.wubi_auto_commit, value),
            "WubiHint" => set_bool(&mut self.wubi_hint, value),
            "Traditional" => set_bool(&mut self.traditional, value),
            "AutoPairPunct" => set_bool(&mut self.auto_pair_punct, value),
            "PickChar" => set_bool(&mut self.pick_char, value),
            "Association" => set_bool(&mut self.association, value),
            "DateTime" => set_bool(&mut self.date_time, value),
            "Calculator" => set_bool(&mut self.calculator, value),
            "EmojiSuggest" => set_bool(&mut self.emoji_suggest, value),
            "AutoSpace" => set_bool(&mut self.auto_space, value),
            "AutoSpaceLetters" => set_bool(&mut self.auto_space_letters, value),
            "AutoSpaceDigits" => set_bool(&mut self.auto_space_digits, value),
            "Toolbar" => set_bool(&mut self.toolbar, value),
            "ClipboardHistory" => set_bool(&mut self.clipboard_history, value),
            "Voice" => set_bool(&mut self.voice, value),
            "VoiceCopy" => set_bool(&mut self.voice_copy, value),
            "Translate" => set_bool(&mut self.translate, value),
            "TranslateFormal" => set_bool(&mut self.translate_formal, value),
            "TranslateLocal" => set_bool(&mut self.translate_local, value),
            "CheckUpdate" => set_bool(&mut self.check_update, value),
            "BetaUpdates" => set_bool(&mut self.beta_updates, value),
            "TypingStats" => set_bool(&mut self.typing_stats, value),
            "CandidateLayout" => {
                if let Some(v) = match_name(value, &["Horizontal", "Vertical"]) {
                    self.horizontal = v == "Horizontal";
                }
            }
            "ArrowKeys" => {
                if let Some(v) = match_name(value, &["Candidates", "Caret"]) {
                    self.arrow_keys = if v == "Candidates" {
                        ArrowKeys::Candidates
                    } else {
                        ArrowKeys::Caret
                    };
                }
            }
            "PageSize" => {
                if let Ok(n) = value.parse::<usize>() {
                    self.page_size = n.clamp(3, 9);
                }
            }
            "InputScheme" => {
                if let Some(v) = InputScheme::parse(value) {
                    self.input_scheme = v;
                }
            }
            "ShuangpinScheme" => {
                if let Some(v) = match_name(value, &SHUANGPIN_SCHEMES) {
                    self.shuangpin_scheme = v;
                }
            }
            "ShiftToggle" => {
                if let Some(v) = match_name(value, &["Both", "Left", "Right", "Off"]) {
                    self.shift_toggle = match v.as_str() {
                        "Both" => ShiftToggle::Both,
                        "Left" => ShiftToggle::Left,
                        "Right" => ShiftToggle::Right,
                        _ => ShiftToggle::Off,
                    };
                }
            }
            "InitialMode" => {
                if let Some(v) = InitialMode::parse(value) {
                    self.initial_mode = v;
                }
            }
            "WindowMode" => {
                if let Some(v) = WindowMode::parse(value) {
                    self.window_mode = v;
                }
            }
            "PunctuationStyle" => {
                if let Some(v) = match_name(value, &["Fullwidth", "Halfwidth"]) {
                    self.halfwidth_punct = v == "Halfwidth";
                }
            }
            "SelectKeys" => {
                if let Some(v) = match_name(value, &["Off", "Semicolon", "SemicolonQuote"]) {
                    self.select_keys = match v.as_str() {
                        "Off" => SelectKeys::Off,
                        "Semicolon" => SelectKeys::Semicolon,
                        _ => SelectKeys::SemicolonQuote,
                    };
                }
            }
            "Lexicon" => self.lexicon = value.to_string(),
            "CandidateFontSize" => {
                if let Ok(n) = value.parse::<f64>() {
                    self.candidate_font_size = n.clamp(10.0, 40.0);
                }
            }
            "VoiceKey" => self.voice_key = value.to_string(),
            "EmojiKey" => {
                // 留空是不用快捷键；写错了保持默认
                if value.is_empty() {
                    self.emoji_key = None;
                } else if let Some(key) = Shortcut::parse(value) {
                    self.emoji_key = Some(key);
                }
            }
            "ToggleKey" | "TranslateModeKey" => {
                // 留空是不用；写错了保持原样
                let target = if key == "ToggleKey" {
                    &mut self.toggle_key
                } else {
                    &mut self.translate_mode_key
                };
                if value.is_empty() {
                    *target = None;
                } else if let Some(shortcut) = Shortcut::parse(value) {
                    *target = Some(shortcut);
                }
            }
            "SettingsTheme" => {
                if let Some(v) = SettingsTheme::parse(value) {
                    self.settings_theme = v;
                }
            }
            "SettingsAccent" => {
                if let Some(v) = match_name(value, &ACCENT_PRESETS.map(|p| p.value)) {
                    self.settings_accent = v;
                }
            }
            "TranslateTarget" => {
                if let Some(v) = match_name(value, &TRANSLATE_TARGETS) {
                    self.translate_target = v;
                }
            }
            "TranslateBaseUrl" => self.translate_base_url = value.to_string(),
            "TranslateModel" => self.translate_model = value.to_string(),
            "TranslateLocalModel" => self.translate_local_model = value.to_string(),
            "TranslateLlamaServer" => self.translate_llama_server = value.to_string(),
            "TranslateIdleMinutes" => {
                if let Ok(n) = value.parse::<i64>() {
                    self.translate_idle_minutes = n.clamp(0, 240);
                }
            }
            _ => {}
        }
    }

    /// 这一项在文件里的写法（True / False、枚举名、数字、原样的路径）；不认识的键是 None。
    pub fn value(&self, key: &str) -> Option<String> {
        let boolean = match key {
            "PageKeysMinusEqual" => Some(self.page_keys_minus),
            "PageKeysBracket" => Some(self.page_keys_bracket),
            "PageKeysCommaPeriod" => Some(self.page_keys_comma),
            "PageKeysTab" => Some(self.page_keys_tab),
            "Fuzzy" => Some(self.fuzzy),
            "FuzzyZh" => Some(self.fuzzy_zh),
            "FuzzyCh" => Some(self.fuzzy_ch),
            "FuzzySh" => Some(self.fuzzy_sh),
            "FuzzyNL" => Some(self.fuzzy_nl),
            "FuzzyFH" => Some(self.fuzzy_fh),
            "FuzzyRL" => Some(self.fuzzy_rl),
            "FuzzyAn" => Some(self.fuzzy_an),
            "FuzzyEn" => Some(self.fuzzy_en),
            "FuzzyIn" => Some(self.fuzzy_in),
            "FuzzyIan" => Some(self.fuzzy_ian),
            "FuzzyUan" => Some(self.fuzzy_uan),
            "FuzzyUn" => Some(self.fuzzy_un),
            "FuzzyAnAi" => Some(self.fuzzy_an_ai),
            "FuzzyHuangWang" => Some(self.fuzzy_huang_wang),
            "FuzzyUnIong" => Some(self.fuzzy_un_iong),
            "FuzzyEngOng" => Some(self.fuzzy_eng_ong),
            "TypoCorrection" => Some(self.typo_correction),
            "RareExtChars" => Some(self.rare_ext_chars),
            "ShuangpinShowKeys" => Some(self.shuangpin_show_keys),
            "ShuangpinAbbrev" => Some(self.shuangpin_abbrev),
            "WubiAutoCommit" => Some(self.wubi_auto_commit),
            "WubiHint" => Some(self.wubi_hint),
            "Traditional" => Some(self.traditional),
            "AutoPairPunct" => Some(self.auto_pair_punct),
            "PickChar" => Some(self.pick_char),
            "Association" => Some(self.association),
            "DateTime" => Some(self.date_time),
            "Calculator" => Some(self.calculator),
            "EmojiSuggest" => Some(self.emoji_suggest),
            "AutoSpace" => Some(self.auto_space),
            "AutoSpaceLetters" => Some(self.auto_space_letters),
            "AutoSpaceDigits" => Some(self.auto_space_digits),
            "Toolbar" => Some(self.toolbar),
            "ClipboardHistory" => Some(self.clipboard_history),
            "Voice" => Some(self.voice),
            "VoiceCopy" => Some(self.voice_copy),
            "Translate" => Some(self.translate),
            "TranslateFormal" => Some(self.translate_formal),
            "TranslateLocal" => Some(self.translate_local),
            "CheckUpdate" => Some(self.check_update),
            "BetaUpdates" => Some(self.beta_updates),
            "TypingStats" => Some(self.typing_stats),
            _ => None,
        };
        if let Some(value) = boolean {
            return Some(if value { "True" } else { "False" }.to_string());
        }
        Some(match key {
            "CandidateLayout" => if self.horizontal {
                "Horizontal"
            } else {
                "Vertical"
            }
            .to_string(),
            "ArrowKeys" => match self.arrow_keys {
                ArrowKeys::Candidates => "Candidates",
                ArrowKeys::Caret => "Caret",
            }
            .to_string(),
            "PageSize" => self.page_size.to_string(),
            "InputScheme" => self.input_scheme.as_str().to_string(),
            "ShuangpinScheme" => self.shuangpin_scheme.clone(),
            "ShiftToggle" => match self.shift_toggle {
                ShiftToggle::Both => "Both",
                ShiftToggle::Left => "Left",
                ShiftToggle::Right => "Right",
                ShiftToggle::Off => "Off",
            }
            .to_string(),
            "InitialMode" => self.initial_mode.as_str().to_string(),
            "WindowMode" => self.window_mode.as_str().to_string(),
            "PunctuationStyle" => if self.halfwidth_punct {
                "Halfwidth"
            } else {
                "Fullwidth"
            }
            .to_string(),
            "SelectKeys" => match self.select_keys {
                SelectKeys::Off => "Off",
                SelectKeys::Semicolon => "Semicolon",
                SelectKeys::SemicolonQuote => "SemicolonQuote",
            }
            .to_string(),
            "Lexicon" => self.lexicon.clone(),
            "CandidateFontSize" => format_number(self.candidate_font_size),
            "VoiceKey" => self.voice_key.clone(),
            "EmojiKey" => self.emoji_key.map(|s| s.name()).unwrap_or_default(),
            "ToggleKey" => self.toggle_key.map(|s| s.name()).unwrap_or_default(),
            "TranslateModeKey" => self
                .translate_mode_key
                .map(|s| s.name())
                .unwrap_or_default(),
            "SettingsTheme" => self.settings_theme.as_str().to_string(),
            "SettingsAccent" => self.settings_accent.clone(),
            "TranslateTarget" => self.translate_target.clone(),
            "TranslateBaseUrl" => self.translate_base_url.clone(),
            "TranslateModel" => self.translate_model.clone(),
            "TranslateLocalModel" => self.translate_local_model.clone(),
            "TranslateLlamaServer" => self.translate_llama_server.clone(),
            "TranslateIdleMinutes" => self.translate_idle_minutes.to_string(),
            _ => return None,
        })
    }

    /// 把 archtype.conf 里的一项改成 value（别的行、注释、顺序都不动；没有就加在最后）。
    ///
    /// 和设置界面的 `set_key` 一样：只动这一行，不改文件里其它内容。
    pub fn set_key_in_file(path: &std::path::Path, key: &str, value: &str) -> bool {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut out = String::new();
        let mut done = false;
        for line in text.lines() {
            let trimmed = line.trim();
            let is_key = !trimmed.starts_with('#')
                && !trimmed.starts_with(';')
                && !trimmed.starts_with('[')
                && trimmed
                    .split_once('=')
                    .map(|(k, _)| k.trim() == key)
                    .unwrap_or(false);
            if is_key && !done {
                out.push_str(&format!("{key}={value}\n"));
                done = true;
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !done {
            out.push_str(&format!("{key}={value}\n"));
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path, out).is_ok()
    }

    /// 模糊音分组的位掩码（和核心的 fuzzy 分组一致）。
    pub fn fuzzy_groups(&self) -> u32 {
        let mut mask = 0;
        let groups: [(bool, u32); 16] = [
            (self.fuzzy_zh, fuzzy_groups::ZH),
            (self.fuzzy_ch, fuzzy_groups::CH),
            (self.fuzzy_sh, fuzzy_groups::SH),
            (self.fuzzy_nl, fuzzy_groups::NL),
            (self.fuzzy_fh, fuzzy_groups::FH),
            (self.fuzzy_rl, fuzzy_groups::RL),
            (self.fuzzy_an, fuzzy_groups::AN),
            (self.fuzzy_en, fuzzy_groups::EN),
            (self.fuzzy_in, fuzzy_groups::IN),
            (self.fuzzy_ian, fuzzy_groups::IAN),
            (self.fuzzy_uan, fuzzy_groups::UAN),
            (self.fuzzy_un, fuzzy_groups::UN),
            (self.fuzzy_an_ai, fuzzy_groups::AN_AI),
            (self.fuzzy_huang_wang, fuzzy_groups::HUANG_WANG),
            (self.fuzzy_un_iong, fuzzy_groups::UN_IONG),
            (self.fuzzy_eng_ong, fuzzy_groups::ENG_ONG),
        ];
        for (on, bit) in groups {
            if on {
                mask |= bit;
            }
        }
        mask
    }

    /// 翻页键的位掩码（和 core 的 session::page_keys 一致）。
    pub fn page_keys(&self) -> u32 {
        let mut mask = 0;
        if self.page_keys_minus {
            mask |= session::page_keys::MINUS;
        }
        if self.page_keys_bracket {
            mask |= session::page_keys::BRACKET;
        }
        if self.page_keys_comma {
            mask |= session::page_keys::COMMA;
        }
        if self.page_keys_tab {
            mask |= session::page_keys::TAB;
        }
        mask
    }

    /// archtype_autospace 的 flags：1 汉字和字母之间、2 汉字和数字之间；总开关关着是 0
    pub fn auto_space_flags(&self) -> u32 {
        if !self.auto_space {
            return 0;
        }
        (if self.auto_space_letters { 1 } else { 0 }) | (if self.auto_space_digits { 2 } else { 0 })
    }

    pub fn shuangpin(&self) -> bool {
        self.input_scheme == InputScheme::Shuangpin
    }

    pub fn wubi(&self) -> bool {
        self.input_scheme == InputScheme::Wubi86
    }

    /// 会话的设置（外壳每次配置变了都抄一份过去）。
    ///
    /// `translate_available`：外壳配好了翻译服务（有「译」这一档）。
    /// `auto_pair_ok`：这个输入框能按位置改字（自动配对要能挪光标才开）。
    pub fn session_config(&self, translate_available: bool, auto_pair_ok: bool) -> session::Config {
        session::Config {
            page_size: self.page_size.max(1),
            calculator: self.calculator,
            date_time: self.date_time,
            emoji_suggest: self.emoji_suggest,
            horizontal: self.horizontal,
            page_keys: self.page_keys(),
            select_keys: self.select_keys,
            pick_char: self.pick_char,
            punct_style: if self.halfwidth_punct {
                PunctStyle::Halfwidth
            } else {
                PunctStyle::Fullwidth
            },
            auto_space: self.auto_space_flags(),
            shift_toggle: self.shift_toggle,
            shift_tap_ms: 500,
            // 双击 Shift 不切换（#253）：等着的单按在 tick/observe 的 frame 里切
            shift_double_tap: true,
            translate_mode: translate_available,
            translate_key: self.translate_mode_key.is_some(),
            toolbar: self.toolbar,
            clipboard_history: self.clipboard_history,
            shuangpin_keys: self.shuangpin_show_keys,
            arrow_keys: self.arrow_keys,
            wubi: self.wubi(),
            wubi_auto_commit: self.wubi_auto_commit,
            wubi_hint: self.wubi_hint,
            association: self.association,
            auto_pair_punct: self.auto_pair_punct && auto_pair_ok,
        }
    }
}

fn implied_translate_local(base_url: &str) -> bool {
    base_url
        .chars()
        .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

fn set_bool(target: &mut bool, value: &str) {
    *target = match value {
        "True" => true,
        "False" => false,
        _ => return,
    }
}

/// 枚举值区分大小写，和 fcitx5 一样（Horizontal 不认）；不认识返回 None，保持默认。
fn match_name(value: &str, names: &[&str]) -> Option<String> {
    names.iter().find(|n| **n == value).map(|n| n.to_string())
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let spec_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../spec/config.json");
        let text = std::fs::read_to_string(&spec_path).expect("读 spec/config.json");
        let spec: serde_json::Value = serde_json::from_str(&text).expect("spec 是 JSON");
        let settings = spec["settings"].as_array().expect("settings 是数组");
        let config = Config::default();
        let mut checked = 0;
        for item in settings {
            let key = item["key"].as_str().expect("key");
            let Some(value) = config.value(key) else {
                // macOS 还没有的设置项（只给 fcitx5 / settings 的）不算
                let platforms = item["platforms"].as_array();
                if platforms
                    .map(|p| p.iter().any(|x| x == "macos"))
                    .unwrap_or(false)
                {
                    panic!("spec 里有 macOS 的设置项 {key}，外壳不认识");
                }
                continue;
            };
            let expected = match &item["platformDefault"]["macos"] {
                serde_json::Value::String(s) => s.clone(),
                _ => match &item["default"] {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => {
                        if *b {
                            "True".into()
                        } else {
                            "False".into()
                        }
                    }
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                },
            };
            assert_eq!(value, expected, "默认值 {key} 和 spec 对不上");
            checked += 1;
        }
        assert!(checked > 40, "只核对了 {checked} 项，spec 没读全");
    }

    fn spec() -> serde_json::Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../spec/config.json");
        let text = std::fs::read_to_string(path).expect("读 spec/config.json");
        serde_json::from_str(&text).expect("spec 是 JSON")
    }

    #[test]
    fn keys_are_exactly_the_macos_settings_in_the_spec() {
        let spec = spec();
        let settings = spec["settings"].as_array().expect("settings 是数组");
        let macos: Vec<&str> = settings
            .iter()
            .filter(|item| {
                item["platforms"]
                    .as_array()
                    .map(|p| p.iter().any(|x| x == "macos"))
                    .unwrap_or(false)
            })
            .filter_map(|item| item["key"].as_str())
            .collect();
        assert_eq!(
            KEYS.len(),
            macos.len(),
            "认识的键和 spec 的 macOS 项数量对不上"
        );
        for key in KEYS {
            assert!(
                macos.contains(&key),
                "外壳认识的 {key} 不在 spec 的 macOS 项里"
            );
        }
    }

    #[test]
    fn shortcut_presets_match_the_spec() {
        let spec = spec();
        let settings = spec["settings"].as_array().expect("settings 是数组");
        let preset = |key: &str| -> Vec<String> {
            let item = settings
                .iter()
                .find(|item| item["key"] == key)
                .unwrap_or_else(|| panic!("spec 里没有 {key}"));
            item["presets"]["macos"]
                .as_array()
                .unwrap_or_else(|| panic!("{key} 没有 macOS 预设"))
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()
        };
        assert_eq!(
            preset("ToggleKey"),
            Shortcut::TOGGLE_KEY_PRESETS.map(String::from)
        );
        assert_eq!(
            preset("TranslateModeKey"),
            Shortcut::TRANSLATE_MODE_KEY_PRESETS.map(String::from)
        );
        // 每个预设都能解析（空串除外，表示不用）
        for name in Shortcut::TOGGLE_KEY_PRESETS
            .into_iter()
            .chain(Shortcut::TRANSLATE_MODE_KEY_PRESETS)
        {
            if !name.is_empty() {
                assert!(Shortcut::parse(name).is_some(), "预设 {name} 解析不了");
            }
        }
    }

    #[test]
    fn set_key_in_file_keeps_other_lines() {
        let dir =
            std::env::temp_dir().join(format!("archtype-macos-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("archtype.conf");
        std::fs::write(&path, "# 注释\nPageSize=9\n[Other]\nPageSize=3\n").unwrap();
        assert!(Config::set_key_in_file(&path, "PageSize", "5"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# 注释"));
        assert!(text.contains("PageSize=5"));
        // 分组里的同名键不动
        assert!(text.contains("[Other]\nPageSize=3"));
        assert!(Config::set_key_in_file(&path, "Lexicon", "/tmp/x.tsv"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("Lexicon=/tmp/x.tsv"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_ignores_groups_comments_and_unknown_keys() {
        let config = Config::parse(
            "# 注释\n\
             PageSize=7\n\
             CandidateLayout=\"Vertical\"\n\
             PageKeysBracket=True\n\
             [Other]\n\
             PageSize=3\n\
             ; 分号注释\n\
             Unknown=1\n\
             EmojiSuggest=False\n",
        );
        assert_eq!(config.page_size, 7);
        assert!(!config.horizontal);
        assert!(config.page_keys_bracket);
        // [Other] 下面的键不是 ArchType 的设置
        assert!(config.emoji_suggest);
        assert_eq!(config.select_keys, SelectKeys::Off);
    }

    #[test]
    fn parse_clamps_numbers_and_keeps_bad_enum_values() {
        let config = Config::parse(
            "PageSize=99\nCandidateFontSize=5\nInputScheme=Wubi86\nArrowKeys=nonsense\n",
        );
        assert_eq!(config.page_size, 9);
        assert_eq!(config.candidate_font_size, 10.0);
        assert_eq!(config.input_scheme, InputScheme::Wubi86);
        assert_eq!(config.arrow_keys, ArrowKeys::Candidates);
    }

    #[test]
    fn old_shuangpin_switch_only_without_input_scheme() {
        let config = Config::parse("Shuangpin=True\n");
        assert_eq!(config.input_scheme, InputScheme::Shuangpin);
        let config = Config::parse("Shuangpin=True\nInputScheme=Quanpin\n");
        assert_eq!(config.input_scheme, InputScheme::Quanpin);
    }

    #[test]
    fn translate_local_is_implied_by_base_url() {
        let config = Config::parse("TranslateBaseUrl=https://api.example.com/v1\n");
        assert!(!config.translate_local);
        let config = Config::parse("TranslateBaseUrl=\n");
        assert!(config.translate_local);
        let config =
            Config::parse("TranslateBaseUrl=https://api.example.com/v1\nTranslateLocal=True\n");
        assert!(config.translate_local);
    }

    #[test]
    fn value_round_trips_through_set_value() {
        let config = Config::parse(
            "PageSize=5\nCandidateLayout=Vertical\nEmojiKey=Control+Shift+E\nSelectKeys=Semicolon\n\
             Fuzzy=True\nFuzzyZh=False\nShuangpinScheme=Xiaohe\n",
        );
        for key in KEYS {
            let Some(value) = config.value(key) else {
                panic!("认识的键 {key} 没有写法");
            };
            let mut copy = config.clone();
            copy.set_value(key, &value);
            assert_eq!(
                copy.value(key).as_deref(),
                Some(value.as_str()),
                "键 {key} 写回后变了"
            );
        }
        assert_eq!(config.emoji_key, Shortcut::parse("Control+Shift+E"));
        assert_eq!(config.shuangpin_scheme, "Xiaohe");
    }

    #[test]
    fn session_config_takes_the_engine_settings() {
        let config =
            Config::parse("PageSize=6\nArrowKeys=Caret\nAutoSpace=True\nAutoSpaceDigits=False\n");
        let session = config.session_config(true, true);
        assert_eq!(session.page_size, 6);
        assert_eq!(session.arrow_keys, ArrowKeys::Caret);
        assert_eq!(session.auto_space, 1);
        assert!(session.translate_mode);
        assert!(session.shift_double_tap);
        assert_eq!(session.page_keys, session::page_keys::MINUS);
    }

    #[test]
    fn fuzzy_mask_matches_the_groups() {
        let config = Config::parse("FuzzyAn=False\nFuzzyUn=True\nFuzzyAnAi=True\n");
        let mask = config.fuzzy_groups();
        assert_eq!(mask & fuzzy_groups::AN, 0);
        assert_eq!(mask & fuzzy_groups::UN, fuzzy_groups::UN);
        assert_eq!(mask & fuzzy_groups::AN_AI, fuzzy_groups::AN_AI);
        assert_eq!(mask & fuzzy_groups::ZH, fuzzy_groups::ZH);
    }
}
