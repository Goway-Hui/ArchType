//! macOS 按键 → 核心的键码（XKB keysym 子集）和修饰键。
//!
//! 平台无关：Linux 上也能编、能测。输入法外壳把 NSEvent 的 keyCode、characters、
//! modifierFlags 原样传进来（见 `macos::controller`）。

/// 核心认的键码（和 X11 keysym 取值一样）。
pub mod keysym {
    pub const BACKSPACE: u32 = 0xff08;
    pub const TAB: u32 = 0xff09;
    pub const RETURN: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const HOME: u32 = 0xff50;
    pub const LEFT: u32 = 0xff51;
    pub const UP: u32 = 0xff52;
    pub const RIGHT: u32 = 0xff53;
    pub const DOWN: u32 = 0xff54;
    pub const PAGE_UP: u32 = 0xff55;
    pub const PAGE_DOWN: u32 = 0xff56;
    pub const END: u32 = 0xff57;
    pub const KP_ENTER: u32 = 0xff8d;
    pub const SHIFT_L: u32 = 0xffe1;
    pub const SHIFT_R: u32 = 0xffe2;
    pub const DELETE: u32 = 0xffff;
}

/// 核心的修饰键位掩码（X11/fcitx5 的 KeyState 取值）。
pub mod modifier {
    pub const SHIFT: u32 = 1 << 0;
    pub const CAPS: u32 = 1 << 1;
    pub const CTRL: u32 = 1 << 2;
    pub const ALT: u32 = 1 << 3;
    pub const SUPER: u32 = 1 << 6;
}

/// 修饰键集合。和 Swift 版一样，只带核心认识的那几个位。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers(pub u32);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(modifier::SHIFT);
    pub const CAPS: Self = Self(modifier::CAPS);
    pub const CTRL: Self = Self(modifier::CTRL);
    pub const ALT: Self = Self(modifier::ALT);
    pub const SUPER: Self = Self(modifier::SUPER);

    /// NSEvent.ModifierFlags 的原始值 → 核心的掩码（不引 AppKit，直接按位取）。
    ///
    /// Cocoa 的位：Caps 1<<16、Shift 1<<17、Control 1<<18、Option 1<<19、Command 1<<20。
    pub fn from_cocoa(flags: u64) -> Self {
        let mut m = 0;
        if flags & (1 << 16) != 0 {
            m |= modifier::CAPS;
        }
        if flags & (1 << 17) != 0 {
            m |= modifier::SHIFT;
        }
        if flags & (1 << 18) != 0 {
            m |= modifier::CTRL;
        }
        if flags & (1 << 19) != 0 {
            m |= modifier::ALT;
        }
        if flags & (1 << 20) != 0 {
            m |= modifier::SUPER;
        }
        Self(m)
    }

    pub fn raw(self) -> u32 {
        self.0
    }

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// 和另一个修饰键集合的交集（快捷键比较用）。
    pub fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// macOS 的虚拟键码（Carbon kVK_*），只列用到的。
pub mod virtual_key {
    pub const RETURN: u16 = 0x24;
    pub const TAB: u16 = 0x30;
    pub const SPACE: u16 = 0x31;
    pub const BACKSPACE: u16 = 0x33;
    pub const ESCAPE: u16 = 0x35;
    pub const SHIFT_LEFT: u16 = 0x38;
    pub const SHIFT_RIGHT: u16 = 0x3c;
    pub const KEYPAD_ENTER: u16 = 0x4c;
    pub const HOME: u16 = 0x73;
    pub const PAGE_UP: u16 = 0x74;
    pub const FORWARD_DELETE: u16 = 0x75;
    pub const END: u16 = 0x77;
    pub const PAGE_DOWN: u16 = 0x79;
    pub const LEFT: u16 = 0x7b;
    pub const RIGHT: u16 = 0x7c;
    pub const DOWN: u16 = 0x7d;
    pub const UP: u16 = 0x7e;
}

/// 一次按键，已经翻译成核心的键码。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyInput {
    pub keysym: u32,
    pub modifiers: Modifiers,
    pub release: bool,
}

impl KeyInput {
    pub fn new(keysym: u32, modifiers: Modifiers, release: bool) -> Self {
        Self {
            keysym,
            modifiers,
            release,
        }
    }

    pub fn key(keysym: u32, modifiers: Modifiers) -> Self {
        Self::new(keysym, modifiers, false)
    }

    pub fn char(c: char, modifiers: Modifiers) -> Self {
        Self::new(c as u32, modifiers, false)
    }

    /// 可见的 ASCII 字符（字母、数字、标点、空格）；功能键为 nil。
    pub fn ascii(self) -> Option<u8> {
        if (0x20..=0x7e).contains(&self.keysym) {
            Some(self.keysym as u8)
        } else {
            None
        }
    }

    pub fn is_letter(self) -> bool {
        self.ascii().is_some_and(|c| c.is_ascii_alphabetic())
    }

    pub fn is_digit(self) -> bool {
        self.ascii().is_some_and(|c| c.is_ascii_digit())
    }
}

/// NSEvent 的 keyDown → KeyInput。`characters` 是考虑了 Shift 的字符（Shift+1 是「!」），
/// 带 Control / Option / Command 时用 `characters_ignoring_modifiers`，免得拿到 Option 组合出来的特殊字符。
pub fn translate_key_down(
    key_code: u16,
    characters: Option<&str>,
    characters_ignoring_modifiers: Option<&str>,
    cocoa_flags: u64,
) -> Option<KeyInput> {
    let modifiers = Modifiers::from_cocoa(cocoa_flags);
    let special: Option<u32> = match key_code {
        virtual_key::RETURN => Some(keysym::RETURN),
        virtual_key::KEYPAD_ENTER => Some(keysym::KP_ENTER),
        virtual_key::TAB => Some(keysym::TAB),
        virtual_key::SPACE => Some(0x20),
        virtual_key::BACKSPACE => Some(keysym::BACKSPACE),
        virtual_key::FORWARD_DELETE => Some(keysym::DELETE),
        virtual_key::ESCAPE => Some(keysym::ESCAPE),
        virtual_key::LEFT => Some(keysym::LEFT),
        virtual_key::RIGHT => Some(keysym::RIGHT),
        virtual_key::UP => Some(keysym::UP),
        virtual_key::DOWN => Some(keysym::DOWN),
        virtual_key::PAGE_UP => Some(keysym::PAGE_UP),
        virtual_key::PAGE_DOWN => Some(keysym::PAGE_DOWN),
        virtual_key::HOME => Some(keysym::HOME),
        virtual_key::END => Some(keysym::END),
        _ => None,
    };
    if let Some(special) = special {
        return Some(KeyInput::new(special, modifiers, false));
    }

    let plain = !modifiers.contains(
        Modifiers::CTRL
            .union(Modifiers::ALT)
            .union(Modifiers::SUPER),
    );
    let text = if plain {
        characters
    } else {
        characters_ignoring_modifiers
    };
    let mut chars = text?.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let value = first as u32;
    if !(0x20..=0x7e).contains(&value) {
        return None;
    }
    Some(KeyInput::new(value, modifiers, false))
}

/// 快捷键（设置里的 EmojiKey）。键名和 fcitx5 一样：修饰键 + 键，用 + 连起来，
/// 比如 Control+period、Control+Shift+E、Alt+semicolon。键只能是可见的 ASCII 字符
/// （X keysym 名字，或者字符本身）。
///
/// macOS 上 Command 组合键多半被应用的菜单先拿走，到不了输入法，所以 Super（Linux 上的
/// Windows 键）当 Control：Linux 的默认值 Super+period 拷过来就是 Control+.。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// 可见 ASCII；字母存小写
    pub keysym: u32,
    /// 只有 shift、control、option
    pub modifiers: Modifiers,
}

impl Shortcut {
    /// 表情面板的默认快捷键（macOS）：Control+.
    pub const EMOJI_DEFAULT: Self = Self {
        keysym: 0x2e,
        modifiers: Modifiers::CTRL,
    };

    /// 设置窗口里切换键、进出「译」的键能选的（空串是不用），和 spec/config.json 的
    /// presets.macos 一样。
    pub const TOGGLE_KEY_PRESETS: [&'static str; 2] = ["", "Control+Shift+space"];
    pub const TRANSLATE_MODE_KEY_PRESETS: [&'static str; 2] = ["", "Control+Shift+Y"];

    pub fn new(keysym: u32, modifiers: Modifiers) -> Self {
        Self {
            keysym: Self::fold(keysym),
            modifiers: modifiers.intersection(
                Modifiers::SHIFT
                    .union(Modifiers::CTRL)
                    .union(Modifiers::ALT),
            ),
        }
    }

    /// 解析 fcitx5 的键名；认不出返回 nil（设置里写错了就保持默认）。
    pub fn parse(name: &str) -> Option<Self> {
        let parts: Vec<String> = name.split('+').map(trim_spaces).collect();
        let key = parts.last()?.clone();
        if key.is_empty() {
            return None;
        }
        let mut modifiers = Modifiers::NONE;
        for part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "control" | "ctrl" | "super" => modifiers.insert(Modifiers::CTRL),
                "alt" | "option" => modifiers.insert(Modifiers::ALT),
                "shift" => modifiers.insert(Modifiers::SHIFT),
                _ => return None,
            }
        }
        let keysym = Self::keysym_named(&key)?;
        Some(Self::new(keysym, modifiers))
    }

    /// 写回设置文件、记日志用的名字（Control+period）。
    pub fn name(self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.modifiers.contains(Modifiers::CTRL) {
            parts.push("Control".into());
        }
        if self.modifiers.contains(Modifiers::ALT) {
            parts.push("Alt".into());
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            parts.push("Shift".into());
        }
        // 按着 Shift 的字母写大写（和 fcitx5 一样：Control+Shift+E），两端设置里的预设才对得上
        let shifted =
            self.modifiers.contains(Modifiers::SHIFT) && (0x61..=0x7a).contains(&self.keysym);
        let index = if shifted {
            self.keysym as i32 - 0x20 - 0x20
        } else {
            self.keysym as i32 - 0x20
        };
        parts.push(
            Self::ascii_names()
                .get(index.max(0) as usize)
                .copied()
                .unwrap_or("?")
                .to_string(),
        );
        parts.join("+")
    }

    /// 设置窗口里给人看的叫法（Mac 键盘上的名字）：Control + .、Option + X、Control + Shift + 空格。
    pub fn label(self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.modifiers.contains(Modifiers::CTRL) {
            parts.push("Control".into());
        }
        if self.modifiers.contains(Modifiers::ALT) {
            parts.push("Option".into());
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            parts.push("Shift".into());
        }
        let key = if self.keysym == 0x20 {
            "空格".to_string()
        } else {
            char::from_u32(self.keysym)
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| "?".to_string())
        };
        parts.push(key);
        parts.join(" + ")
    }

    /// 这次按键是不是这个快捷键。CapsLock 不算；字母不分大小写（按着 Shift 时 macOS 给的是大写）。
    pub fn matches(self, key: KeyInput) -> bool {
        let relevant = Modifiers::SHIFT
            .union(Modifiers::CTRL)
            .union(Modifiers::ALT)
            .union(Modifiers::SUPER);
        key.modifiers.intersection(relevant) == self.modifiers
            && Self::fold(key.keysym) == self.keysym
    }

    fn fold(keysym: u32) -> u32 {
        if (0x41..=0x5a).contains(&keysym) {
            keysym | 0x20
        } else {
            keysym
        }
    }

    fn keysym_named(name: &str) -> Option<u32> {
        let mut chars = name.chars();
        if let Some(first) = chars.next() {
            if chars.next().is_none() {
                let value = first as u32;
                if (0x21..=0x7e).contains(&value) {
                    return Some(value);
                }
            }
        }
        let lower = name.to_lowercase();
        let names = Self::ascii_names();
        names
            .iter()
            .position(|n| n.to_lowercase() == lower)
            .map(|index| 0x20 + index as u32)
    }

    /// 0x20–0x7e 的 X keysym 名字（字母、数字是它自己）。
    fn ascii_names() -> &'static [&'static str] {
        const NAMES: [&str; 95] = [
            "space",
            "exclam",
            "quotedbl",
            "numbersign",
            "dollar",
            "percent",
            "ampersand",
            "apostrophe",
            "parenleft",
            "parenright",
            "asterisk",
            "plus",
            "comma",
            "minus",
            "period",
            "slash",
            "0",
            "1",
            "2",
            "3",
            "4",
            "5",
            "6",
            "7",
            "8",
            "9",
            "colon",
            "semicolon",
            "less",
            "equal",
            "greater",
            "question",
            "at",
            "A",
            "B",
            "C",
            "D",
            "E",
            "F",
            "G",
            "H",
            "I",
            "J",
            "K",
            "L",
            "M",
            "N",
            "O",
            "P",
            "Q",
            "R",
            "S",
            "T",
            "U",
            "V",
            "W",
            "X",
            "Y",
            "Z",
            "bracketleft",
            "backslash",
            "bracketright",
            "asciicircum",
            "underscore",
            "grave",
            "a",
            "b",
            "c",
            "d",
            "e",
            "f",
            "g",
            "h",
            "i",
            "j",
            "k",
            "l",
            "m",
            "n",
            "o",
            "p",
            "q",
            "r",
            "s",
            "t",
            "u",
            "v",
            "w",
            "x",
            "y",
            "z",
            "braceleft",
            "bar",
            "braceright",
            "asciitilde",
        ];
        &NAMES
    }
}

fn trim_spaces(s: &str) -> String {
    s.trim().to_string()
}

/// 语音键：设置里的 VoiceKey 用 fcitx5 的键名（和 Linux 一样，一份 archtype.conf 两边通用），
/// 这里翻成 macOS 的键码。修饰键（右 Option 等）在 flagsChanged 里看按下 / 松开，F 键看 keyDown / keyUp。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceKey {
    /// macOS 虚拟键码（kVK_*）
    pub key_code: u16,
    /// 修饰键在 NSEvent.modifierFlags 里的「分左右」那一位（NX_DEVICE*KEYMASK）；不是修饰键为 0
    pub device_mask: u64,
    /// 同一种修饰键左右两边的设备位（Option 是 0x20 | 0x40）；不是修饰键为 0
    pub sides_mask: u64,
    /// 不分左右的修饰键位（NSEvent.ModifierFlags.option 等）；不是修饰键为 0
    pub generic_mask: u64,
    /// 设置里写的键名
    pub name: &'static str,
}

impl VoiceKey {
    pub fn is_modifier(&self) -> bool {
        self.device_mask != 0
    }

    /// fcitx5 键名 → 键；留空、none、Mac 上没有的键（Menu）返回 None，语音输入就不用热键。
    pub fn parse(name: &str) -> Option<Self> {
        let (key_code, device_mask, sides_mask, generic_mask) = match name {
            "Alt_R" => (0x3d, 0x40, 0x60, 0x80000),
            "Alt_L" => (0x3a, 0x20, 0x60, 0x80000),
            "Control_R" => (0x3e, 0x2000, 0x2001, 0x40000),
            "Control_L" => (0x3b, 0x01, 0x2001, 0x40000),
            "Super_R" => (0x36, 0x10, 0x18, 0x100000),
            "Super_L" => (0x37, 0x08, 0x18, 0x100000),
            "F1" => (0x7a, 0, 0, 0),
            "F2" => (0x78, 0, 0, 0),
            "F3" => (0x63, 0, 0, 0),
            "F4" => (0x76, 0, 0, 0),
            "F5" => (0x60, 0, 0, 0),
            "F6" => (0x61, 0, 0, 0),
            "F7" => (0x62, 0, 0, 0),
            "F8" => (0x64, 0, 0, 0),
            "F9" => (0x65, 0, 0, 0),
            "F10" => (0x6d, 0, 0, 0),
            "F11" => (0x67, 0, 0, 0),
            "F12" => (0x6f, 0, 0, 0),
            _ => return None,
        };
        Some(Self {
            key_code,
            device_mask,
            sides_mask,
            generic_mask,
            name: match name {
                "Alt_R" => "Alt_R",
                "Alt_L" => "Alt_L",
                "Control_R" => "Control_R",
                "Control_L" => "Control_L",
                "Super_R" => "Super_R",
                "Super_L" => "Super_L",
                "F1" => "F1",
                "F2" => "F2",
                "F3" => "F3",
                "F4" => "F4",
                "F5" => "F5",
                "F6" => "F6",
                "F7" => "F7",
                "F8" => "F8",
                "F9" => "F9",
                "F10" => "F10",
                "F11" => "F11",
                "F12" => "F12",
                _ => unreachable!(),
            },
        })
    }

    /// 这次 flagsChanged 里语音键是不是按着（raw_flags 是 NSEvent.modifierFlags.rawValue）。
    /// 有的键盘、远程桌面、键位映射工具发来的事件不带分左右的设备位，只有 Option 这种通用位：
    /// 这时看通用位（调用的地方已经按键码认准了是右边那个键）。
    pub fn is_down(&self, raw_flags: u64) -> bool {
        if raw_flags & self.sides_mask != 0 {
            raw_flags & self.device_mask != 0
        } else {
            raw_flags & self.generic_mask != 0
        }
    }

    /// 设置窗口里能选的（和 spec/config.json 里 VoiceKey 的 presets.macos 一样）
    pub const PRESETS: [(&'static str, &'static str); 5] = [
        ("右 Option", "Alt_R"),
        ("右 Control", "Control_R"),
        ("F8", "F8"),
        ("F9", "F9"),
        ("F10", "F10"),
    ];
}

/// 正在听时音量画成一排竖条（和 Linux 插件的 voiceWave 一样，不足 6 个时左边补最低的）。
pub fn voice_wave(levels: &[i32]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let shown = &levels[levels.len().saturating_sub(6)..];
    let mut out = String::new();
    for _ in 0..6usize.saturating_sub(shown.len()) {
        out.push(BARS[0]);
    }
    for level in shown {
        let index = (*level).clamp(0, 100) as usize * (BARS.len() - 1) / 100;
        out.push(BARS[index]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cocoa_flags_to_modifiers() {
        let m = Modifiers::from_cocoa((1 << 17) | (1 << 18) | (1 << 20));
        assert_eq!(
            m,
            Modifiers::SHIFT
                .union(Modifiers::CTRL)
                .union(Modifiers::SUPER)
        );
        assert!(!m.contains(Modifiers::ALT));
    }

    #[test]
    fn shift_digit_is_the_shifted_character() {
        // Shift+1：macOS 的 characters 是 "!"，键码不是功能键
        let key = translate_key_down(0x12, Some("!"), Some("1"), 1 << 17).unwrap();
        assert_eq!(key.keysym, 0x21);
        assert_eq!(key.modifiers, Modifiers::SHIFT);
        assert!(!key.release);
    }

    #[test]
    fn control_uses_characters_ignoring_modifiers() {
        // Control+Option+字母：characters 可能是特殊字符，用 ignoring 的原字符
        let key =
            translate_key_down(0x00, Some("\u{1}"), Some("a"), (1 << 18) | (1 << 19)).unwrap();
        assert_eq!(key.keysym, 'a' as u32);
        assert!(key.modifiers.contains(Modifiers::CTRL));
        assert!(key.modifiers.contains(Modifiers::ALT));
    }

    #[test]
    fn special_keys_ignore_characters() {
        let key = translate_key_down(virtual_key::LEFT, Some("\u{1}"), None, 0).unwrap();
        assert_eq!(key.keysym, keysym::LEFT);
        let key = translate_key_down(virtual_key::RETURN, Some("\r"), None, 0).unwrap();
        assert_eq!(key.keysym, keysym::RETURN);
        let key = translate_key_down(virtual_key::FORWARD_DELETE, None, None, 0).unwrap();
        assert_eq!(key.keysym, keysym::DELETE);
    }

    #[test]
    fn function_keys_and_non_ascii_are_dropped() {
        assert!(translate_key_down(0x60, Some("\u{f705}"), None, 0).is_none()); // F5
        assert!(translate_key_down(0x00, Some("中"), None, 0).is_none());
        assert!(translate_key_down(0x00, None, None, 0).is_none());
    }

    #[test]
    fn shortcut_parse_and_name_round_trip() {
        let s = Shortcut::parse("Control+period").unwrap();
        assert_eq!(s.keysym, 0x2e);
        assert_eq!(s.modifiers, Modifiers::CTRL);
        assert_eq!(s.name(), "Control+period");

        let s = Shortcut::parse("Control+Shift+E").unwrap();
        assert_eq!(s.keysym, 'e' as u32);
        assert_eq!(s.name(), "Control+Shift+E");

        // Super 当 Control（Linux 的默认值拷到 macOS）
        let s = Shortcut::parse("Super+period").unwrap();
        assert_eq!(s.modifiers, Modifiers::CTRL);

        assert!(Shortcut::parse("").is_none());
        assert!(Shortcut::parse("Hyper+x").is_none());
        assert!(Shortcut::parse("Control+").is_none());
    }

    #[test]
    fn shortcut_matches_ignores_caps_and_case() {
        let s = Shortcut::parse("Control+Shift+E").unwrap();
        let key = KeyInput::new(
            'E' as u32,
            Modifiers::CTRL
                .union(Modifiers::SHIFT)
                .union(Modifiers::CAPS),
            false,
        );
        assert!(s.matches(key));
        let key = KeyInput::new('e' as u32, Modifiers::CTRL.union(Modifiers::SHIFT), false);
        assert!(s.matches(key));
        let key = KeyInput::new('E' as u32, Modifiers::CTRL, false);
        assert!(!s.matches(key));
        // Command 组合键不算（外壳把它当 Control，但这里只比 shift/control/option）
        let key = KeyInput::new(
            'E' as u32,
            Modifiers::CTRL
                .union(Modifiers::SHIFT)
                .union(Modifiers::SUPER),
            false,
        );
        assert!(!s.matches(key));
    }

    #[test]
    fn voice_key_names_and_masks() {
        let key = VoiceKey::parse("Alt_R").unwrap();
        assert_eq!(key.key_code, 0x3d);
        assert!(key.is_modifier());
        // 右 Option 的设备位
        assert!(key.is_down(0x40));
        assert!(!key.is_down(0x20));
        // 没有设备位时看通用位
        assert!(key.is_down(0x80000));
        assert!(!key.is_down(0x40000));

        let key = VoiceKey::parse("F8").unwrap();
        assert!(!key.is_modifier());
        assert_eq!(key.key_code, 0x64);

        assert!(VoiceKey::parse("").is_none());
        assert!(VoiceKey::parse("Menu").is_none());
        for (_, name) in VoiceKey::PRESETS {
            assert!(VoiceKey::parse(name).is_some(), "{name} 解析不了");
        }
    }

    #[test]
    fn voice_wave_always_has_six_bars() {
        assert_eq!(voice_wave(&[]), "▁▁▁▁▁▁");
        assert_eq!(voice_wave(&[100]), "▁▁▁▁▁█");
        assert_eq!(voice_wave(&[0, 50, 100]).chars().count(), 6);
    }

    #[test]
    fn shortcut_label_uses_mac_names() {
        assert_eq!(Shortcut::EMOJI_DEFAULT.label(), "Control + .");
        let s = Shortcut::parse("Control+Shift+space").unwrap();
        assert_eq!(s.label(), "Control + Shift + 空格");
    }
}
