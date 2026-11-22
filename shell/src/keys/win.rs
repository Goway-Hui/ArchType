//! Windows 的键表：虚拟键码（VK）+ `ToUnicodeEx` 的字符 → core 的键码。
//!
//! `modifiers` 由平台层从 `GetKeyState(VK_SHIFT / VK_CONTROL / VK_MENU / VK_CAPITAL)` 组合；
//! 这个模块是纯数据，Linux 上也能编、能测。

use super::{keysym, KeyInput, Modifiers};

/// Win32 的虚拟键码（`winuser.h` 的 `VK_*`），只列用到的。
pub mod virtual_key {
    pub const BACK: u16 = 0x08;
    pub const TAB: u16 = 0x09;
    pub const RETURN: u16 = 0x0d;
    pub const ESCAPE: u16 = 0x1b;
    pub const SPACE: u16 = 0x20;
    pub const PRIOR: u16 = 0x21;
    pub const NEXT: u16 = 0x22;
    pub const END: u16 = 0x23;
    pub const HOME: u16 = 0x24;
    pub const LEFT: u16 = 0x25;
    pub const UP: u16 = 0x26;
    pub const RIGHT: u16 = 0x27;
    pub const DOWN: u16 = 0x28;
    pub const DELETE: u16 = 0x2e;
    pub const LWIN: u16 = 0x5b;
    pub const RWIN: u16 = 0x5c;
    pub const LCONTROL: u16 = 0xa2;
    pub const RCONTROL: u16 = 0xa3;
    pub const LMENU: u16 = 0xa4;
    pub const RMENU: u16 = 0xa5;
    pub const F1: u16 = 0x70;
    pub const F12: u16 = 0x7b;
}

/// 一次 keyDown → `KeyInput`。`characters` 是 `ToUnicodeEx` 的结果（考虑 Shift 和布局）；
/// 功能键、修饰键没有字符，走特殊键表。拿不到可见 ASCII 时返回 None（交给应用）。
pub fn translate_key_down(
    vk: u16,
    characters: Option<&str>,
    modifiers: Modifiers,
) -> Option<KeyInput> {
    let special: Option<u32> = match vk {
        virtual_key::BACK => Some(keysym::BACKSPACE),
        virtual_key::TAB => Some(keysym::TAB),
        virtual_key::RETURN => Some(keysym::RETURN),
        virtual_key::ESCAPE => Some(keysym::ESCAPE),
        virtual_key::SPACE => Some(0x20),
        virtual_key::PRIOR => Some(keysym::PAGE_UP),
        virtual_key::NEXT => Some(keysym::PAGE_DOWN),
        virtual_key::END => Some(keysym::END),
        virtual_key::HOME => Some(keysym::HOME),
        virtual_key::LEFT => Some(keysym::LEFT),
        virtual_key::UP => Some(keysym::UP),
        virtual_key::RIGHT => Some(keysym::RIGHT),
        virtual_key::DOWN => Some(keysym::DOWN),
        virtual_key::DELETE => Some(keysym::DELETE),
        _ => None,
    };
    if let Some(symbol) = special {
        return Some(KeyInput::new(symbol, modifiers, false));
    }

    let mut chars = characters?.chars();
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

/// 语音键：设置里的 VoiceKey 用 fcitx5 的键名，这里翻成 Windows 的 VK。
/// 修饰键（右 Option 对应的 Alt_R、右 Control）由平台看 `GetKeyState` 的左右键。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceKey {
    /// Win32 虚拟键码
    pub vk: u16,
    /// 设置里写的键名
    pub name: &'static str,
}

impl VoiceKey {
    /// 留空、none、Windows 上没有的键（Menu）返回 None，语音输入就不用热键。
    pub fn parse(name: &str) -> Option<Self> {
        let (vk, name) = match name {
            "Alt_R" => (virtual_key::RMENU, "Alt_R"),
            "Alt_L" => (virtual_key::LMENU, "Alt_L"),
            "Control_R" => (virtual_key::RCONTROL, "Control_R"),
            "Control_L" => (virtual_key::LCONTROL, "Control_L"),
            "Super_R" => (virtual_key::RWIN, "Super_R"),
            "Super_L" => (virtual_key::LWIN, "Super_L"),
            "F1" => (0x70, "F1"),
            "F2" => (0x71, "F2"),
            "F3" => (0x72, "F3"),
            "F4" => (0x73, "F4"),
            "F5" => (0x74, "F5"),
            "F6" => (0x75, "F6"),
            "F7" => (0x76, "F7"),
            "F8" => (0x77, "F8"),
            "F9" => (0x78, "F9"),
            "F10" => (0x79, "F10"),
            "F11" => (0x7a, "F11"),
            "F12" => (0x7b, "F12"),
            _ => return None,
        };
        Some(Self { vk, name })
    }

    pub fn is_modifier(&self) -> bool {
        matches!(
            self.vk,
            virtual_key::LMENU
                | virtual_key::RMENU
                | virtual_key::LCONTROL
                | virtual_key::RCONTROL
                | virtual_key::LWIN
                | virtual_key::RWIN
        )
    }

    /// 设置窗口里能选的（和 spec/config.json 里 VoiceKey 的 presets 一样）。
    pub const PRESETS: [(&'static str, &'static str); 5] = [
        ("右 Alt", "Alt_R"),
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
    fn special_keys_map_to_core_keysyms() {
        let key = translate_key_down(virtual_key::LEFT, None, Modifiers::NONE).unwrap();
        assert_eq!(key.keysym, keysym::LEFT);
        let key = translate_key_down(virtual_key::RETURN, None, Modifiers::NONE).unwrap();
        assert_eq!(key.keysym, keysym::RETURN);
        let key = translate_key_down(virtual_key::DELETE, None, Modifiers::NONE).unwrap();
        assert_eq!(key.keysym, keysym::DELETE);
    }

    #[test]
    fn characters_keep_modifiers() {
        let key = translate_key_down(0x41, Some("A"), Modifiers::SHIFT).unwrap();
        assert_eq!(key.keysym, 'A' as u32);
        assert_eq!(key.modifiers, Modifiers::SHIFT);
        // 中文、非 ASCII：不处理，交给应用
        assert!(translate_key_down(0x41, Some("中"), Modifiers::NONE).is_none());
        assert!(translate_key_down(0x41, None, Modifiers::NONE).is_none());
    }

    #[test]
    fn voice_key_names() {
        let key = VoiceKey::parse("Alt_R").unwrap();
        assert_eq!(key.vk, virtual_key::RMENU);
        assert!(key.is_modifier());
        let key = VoiceKey::parse("F8").unwrap();
        assert!(!key.is_modifier());
        assert!(VoiceKey::parse("Menu").is_none());
        for (_, name) in VoiceKey::PRESETS {
            assert!(VoiceKey::parse(name).is_some(), "{name} 解析不了");
        }
    }
}
