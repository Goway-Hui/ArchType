//! macOS 的键表：`NSEvent` 的 keyCode、`characters`、修饰键的设备位。

use super::{keysym, KeyInput, Modifiers};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
