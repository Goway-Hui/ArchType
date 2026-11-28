//! TSF 文本服务的注册与卸载（写注册表）。
//!
//! 安装程序（或 `archtype --register`）以管理员身份调用；没有管理员权限时
//! `HKLM` 写入会失败，返回具体错误码。
//!
//! 用户输入法列表（`HKCU\Control Panel\International\User Profile`）按用户存，
//! 由 `--enable-profile` / `--disable-profile` 维护；MSI 在安装后和卸载前各调一次。

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, WIN32_ERROR,
};
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegEnumKeyExW, RegOpenKeyExW,
    RegSetValueExW, HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ,
    KEY_WRITE, REG_CREATE_KEY_DISPOSITION, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
};

use crate::{CLSID_TEXTSERVICE, GUID_KEYBOARD_CAT, GUID_PROFILE, GUID_TEXTSERVICE_CAT};

fn guid_string(guid: &windows::core::GUID) -> String {
    let d = &guid.data4;
    format!(
        "{{{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
        guid.data1, guid.data2, guid.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    )
}

const TIP_KEY: &str = r"SOFTWARE\Microsoft\CTF\TIP";
/// 当前用户的输入法列表（每个语言一个子键）。
const USER_PROFILE_KEY: &str = r"Control Panel\International\User Profile";

pub fn register() -> Result<(), String> {
    let dll = dll_path()?;
    let clsid = guid_string(&CLSID_TEXTSERVICE);
    let profile = guid_string(&GUID_PROFILE);

    let clsid_key = format!(r"CLSID\{clsid}");
    check(set_sz(HKEY_CLASSES_ROOT, &clsid_key, "", "ArchType 拼音"))?;
    check(set_sz(
        HKEY_CLASSES_ROOT,
        &format!(r"{clsid_key}\InprocServer32"),
        "",
        &dll,
    ))?;
    check(set_sz(
        HKEY_CLASSES_ROOT,
        &format!(r"{clsid_key}\InprocServer32"),
        "ThreadingModel",
        "Apartment",
    ))?;
    check(set_sz(
        HKEY_CLASSES_ROOT,
        &format!(
            r"{clsid_key}\Implemented Categories\{{{}}}",
            guid_string(&GUID_TEXTSERVICE_CAT)
        ),
        "",
        "",
    ))?;
    check(set_sz(
        HKEY_CLASSES_ROOT,
        &format!(
            r"{clsid_key}\Implemented Categories\{{{}}}",
            guid_string(&GUID_KEYBOARD_CAT)
        ),
        "",
        "",
    ))?;

    let tip = format!(r"{TIP_KEY}\{clsid}");
    check(set_dword(HKEY_LOCAL_MACHINE, &tip, "Enable", 1))?;
    check(set_sz(
        HKEY_LOCAL_MACHINE,
        &format!(r"{tip}\LanguageProfile\0x00000804\{{{profile}}}"),
        "Description",
        "ArchType 拼音",
    ))?;

    let install = std::path::Path::new(&dll)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    check(set_sz(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\ArchType",
        "InstallDir",
        &install,
    ))?;
    Ok(())
}

pub fn unregister() -> Result<(), String> {
    let clsid = guid_string(&CLSID_TEXTSERVICE);
    for (root, path) in [
        (HKEY_CLASSES_ROOT, format!(r"CLSID\{clsid}")),
        (HKEY_LOCAL_MACHINE, format!(r"{TIP_KEY}\{clsid}")),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\ArchType".to_string()),
    ] {
        let sub = crate::paths::wide(&path);
        unsafe {
            let _ = RegDeleteTreeW(root, PCWSTR(sub.as_ptr()));
        }
    }
    // HKLM 的注册没了，当前用户列表里的条目也要一起撤掉，不然语言栏留个点不动的幽灵项。
    let _ = disable_profile();
    Ok(())
}

/// 用户输入法列表里的值名：`<LCID>:<CLSID><Profile>`（和微软拼音条目同格式）。
fn profile_value_name(language_id: u16, clsid: &str, profile: &str) -> String {
    format!("{language_id:04X}:{clsid}{profile}")
}

/// 语言子键（BCP-47，如 `zh-Hans-CN`；旧系统也可能直接是十六进制 LCID）→ 值名里的 LCID。
///
/// 认不出的 `zh` 变体返回 None：宁可不写，也不要把简体拼音塞进别的语言下面。
fn language_id_for(key: &str) -> Option<u16> {
    let lower = key.to_ascii_lowercase();
    let mapped = match lower.as_str() {
        "zh" | "zh-cn" | "zh-chs" | "zh-hans" | "zh-hans-cn" => 0x0804,
        "zh-sg" | "zh-hans-sg" => 0x1004,
        "zh-tw" | "zh-cht" | "zh-hant" | "zh-hant-tw" => 0x0404,
        "zh-hk" | "zh-hant-hk" => 0x0C04,
        "zh-mo" | "zh-hant-mo" => 0x1404,
        _ => {
            // 旧式键名就是 LCID 的十六进制（0804、00000804 这类）。
            if lower.len() >= 4 && lower.len() <= 8 && lower.chars().all(|c| c.is_ascii_hexdigit())
            {
                u16::from_str_radix(&lower[lower.len() - 4..], 16).ok()?
            } else {
                return None;
            }
        }
    };
    Some(mapped)
}

/// 枚举当前用户输入法列表里的语言子键。
fn profile_languages() -> Result<Vec<String>, String> {
    let base_w = crate::paths::wide(USER_PROFILE_KEY);
    let mut root = HKEY::default();
    unsafe {
        check(RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(base_w.as_ptr()),
            0,
            KEY_READ,
            &mut root,
        ))?;
    }
    let mut names = Vec::new();
    let mut index = 0u32;
    let mut buf = vec![0u16; 64];
    loop {
        let mut len = buf.len() as u32;
        let err = unsafe {
            RegEnumKeyExW(
                root,
                index,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
                None,
                PWSTR::null(),
                None,
                None,
            )
        };
        match err {
            ERROR_NO_MORE_ITEMS => break,
            // 名字比缓冲区长：加长再来（索引不变）。
            ERROR_MORE_DATA if buf.len() < 1024 => buf.resize(buf.len() * 2, 0),
            ERROR_SUCCESS => {
                names.push(String::from_utf16_lossy(&buf[..len as usize]));
                index += 1;
            }
            other => {
                unsafe {
                    let _ = RegCloseKey(root);
                }
                return Err(format!("读输入法列表失败：错误码 {}", other.0));
            }
        }
    }
    unsafe {
        let _ = RegCloseKey(root);
    }
    Ok(names)
}

/// 把 ArchType 加进当前用户的中文输入法列表（每个 zh 语言键下按该语言的 LCID 写
/// `<LCID>:{CLSID}{Profile}` 的 DWORD 1，和微软拼音条目同一格式）。幂等。
pub fn enable_profile() -> Result<(), String> {
    let clsid = guid_string(&CLSID_TEXTSERVICE);
    let profile = guid_string(&GUID_PROFILE);
    let targets: Vec<(String, u16)> = profile_languages()?
        .into_iter()
        .filter_map(|key| language_id_for(&key).map(|id| (key, id)))
        .collect();
    if targets.is_empty() {
        return Err("没找到中文（zh）的语言键：请先在系统设置里添加中文语言".into());
    }
    for (key, language_id) in targets {
        check(set_dword(
            HKEY_CURRENT_USER,
            &format!(r"{USER_PROFILE_KEY}\{key}"),
            &profile_value_name(language_id, &clsid, &profile),
            1,
        ))?;
    }
    Ok(())
}

/// 从当前用户的输入法列表里删掉 ArchType（卸载时调）。没装过、没有中文语言都算成功。
pub fn disable_profile() -> Result<(), String> {
    let clsid = guid_string(&CLSID_TEXTSERVICE);
    let profile = guid_string(&GUID_PROFILE);
    let names = match profile_languages() {
        Ok(names) => names,
        Err(_) => return Ok(()),
    };
    for key in names {
        let Some(language_id) = language_id_for(&key) else {
            continue;
        };
        match delete_value(
            HKEY_CURRENT_USER,
            &format!(r"{USER_PROFILE_KEY}\{key}"),
            &profile_value_name(language_id, &clsid, &profile),
        ) {
            ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => {}
            other => return Err(format!("删输入法条目失败：错误码 {}", other.0)),
        }
    }
    Ok(())
}

fn check(err: WIN32_ERROR) -> Result<(), String> {
    if err == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("注册表操作失败：错误码 {}", err.0))
    }
}

fn set_sz(root: HKEY, path: &str, name: &str, value: &str) -> WIN32_ERROR {
    let data: Vec<u8> = value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    set_value(root, path, name, REG_SZ, &data)
}

fn set_dword(root: HKEY, path: &str, name: &str, value: u32) -> WIN32_ERROR {
    set_value(root, path, name, REG_DWORD, &value.to_le_bytes())
}

fn set_value(
    root: HKEY,
    path: &str,
    name: &str,
    kind: windows::Win32::System::Registry::REG_VALUE_TYPE,
    data: &[u8],
) -> WIN32_ERROR {
    let path_w = crate::paths::wide(path);
    let name_w = crate::paths::wide(name);
    unsafe {
        let mut key = HKEY::default();
        let mut disposition = REG_CREATE_KEY_DISPOSITION(0);
        let err = RegCreateKeyExW::<HKEY, PCWSTR, PCWSTR>(
            root,
            PCWSTR(path_w.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            Some(&mut disposition),
        );
        if err != ERROR_SUCCESS {
            return err;
        }
        let err = RegSetValueExW::<HKEY, PCWSTR>(key, PCWSTR(name_w.as_ptr()), 0, kind, Some(data));
        let _ = RegCloseKey(key);
        err
    }
}

fn delete_value(root: HKEY, path: &str, name: &str) -> WIN32_ERROR {
    let path_w = crate::paths::wide(path);
    let name_w = crate::paths::wide(name);
    unsafe {
        let mut key = HKEY::default();
        let err = RegOpenKeyExW(root, PCWSTR(path_w.as_ptr()), 0, KEY_WRITE, &mut key);
        if err != ERROR_SUCCESS {
            return err;
        }
        let err = RegDeleteValueW(key, PCWSTR(name_w.as_ptr()));
        let _ = RegCloseKey(key);
        err
    }
}

fn dll_path() -> Result<String, String> {
    let module_path = unsafe {
        let module = GetModuleHandleW(PCWSTR::null()).map_err(|err| err.to_string())?;
        let mut buf = [0u16; 1024];
        let len = GetModuleFileNameW(module, &mut buf) as usize;
        if len == 0 || len >= buf.len() {
            return Err("取不到 DLL 路径".into());
        }
        String::from_utf16_lossy(&buf[..len])
    };
    // regsvr32 / DLL 自己调 DllRegisterServer 时就是 DLL；从 archtype.exe 调用时换成同目录的 DLL
    let path = std::path::Path::new(&module_path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name == "archtype.exe" {
        if let Some(dir) = path.parent() {
            let dll = dir.join("archtype_tsf.dll");
            if dll.is_file() {
                return Ok(dll.to_string_lossy().to_string());
            }
        }
        return Err(format!("找不到同目录的 archtype_tsf.dll（{module_path}）"));
    }
    Ok(module_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_name_matches_the_microsoft_format() {
        assert_eq!(
            profile_value_name(0x0804, "{CLSID}", "{PROFILE}"),
            "0804:{CLSID}{PROFILE}"
        );
        assert_eq!(
            profile_value_name(0x0404, "{CLSID}", "{PROFILE}"),
            "0404:{CLSID}{PROFILE}"
        );
        assert_eq!(
            profile_value_name(0x0C04, "{CLSID}", "{PROFILE}"),
            "0C04:{CLSID}{PROFILE}"
        );
    }

    #[test]
    fn language_ids_follow_the_language_key() {
        for (key, expected) in [
            ("zh-Hans-CN", 0x0804),
            ("zh-CN", 0x0804),
            ("zh", 0x0804),
            ("zh-Hans-SG", 0x1004),
            ("zh-Hant-TW", 0x0404),
            ("zh-Hant-HK", 0x0C04),
            ("zh-Hant-MO", 0x1404),
            ("0804", 0x0804),
            ("00000804", 0x0804),
            ("0404", 0x0404),
        ] {
            assert_eq!(language_id_for(key), Some(expected), "{key}");
        }
        assert_eq!(language_id_for("en-US"), None);
        assert_eq!(language_id_for("zh-Hant-XX"), None);
    }
}
