//! TSF 文本服务的注册与卸载（写注册表）。
//!
//! 安装程序（或 `archtype --register`）以管理员身份调用；没有管理员权限时
//! `HKLM` 写入会失败，返回具体错误码。

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CLASSES_ROOT,
    HKEY_LOCAL_MACHINE, KEY_WRITE, REG_CREATE_KEY_DISPOSITION, REG_DWORD, REG_OPTION_NON_VOLATILE,
    REG_SZ,
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
    Ok(())
}

fn check(err: WIN32_ERROR) -> Result<(), String> {
    if err == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("写注册表失败：错误码 {}", err.0))
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

#[allow(dead_code)]
fn _assert_send<T: Send>() {}
