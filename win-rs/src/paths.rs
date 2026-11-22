//! Windows 的目录：安装目录（资源）与 `%APPDATA%\ArchType`（用户数据）。
//!
//! DLL 在宿主应用里跑，`current_exe` 是宿主，不是 ArchType：安装目录从注册表读
//! （安装时写 `HKLM\SOFTWARE\ArchType\InstallDir`）。

use std::path::PathBuf;

use archtype_shell::engine::Layout;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, REG_VALUE_TYPE,
};

pub fn install_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ARCHTYPE_RESOURCE_DIR") {
        return dir.into();
    }
    if let Some(dir) = registry_install_dir() {
        return dir;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn support_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ARCHTYPE_DATA_DIR") {
        return dir.into();
    }
    std::env::var_os("APPDATA")
        .map(|dir| PathBuf::from(dir).join("ArchType"))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn layout() -> Layout {
    Layout::new(install_dir(), support_dir())
}

fn registry_install_dir() -> Option<PathBuf> {
    let sub = wide(r"SOFTWARE\ArchType");
    unsafe {
        let mut key = HKEY::default();
        let mut kind = REG_SZ;
        let mut buf = [0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        let opened = windows::Win32::System::Registry::RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            windows::core::PCWSTR(sub.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        );
        if opened != ERROR_SUCCESS {
            return None;
        }
        let name = wide("InstallDir");
        let err = RegQueryValueExW(
            key,
            windows::core::PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind as *mut REG_VALUE_TYPE),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if err != ERROR_SUCCESS || kind != REG_SZ {
            return None;
        }
        let len = (size as usize / 2).saturating_sub(1);
        let path = String::from_utf16_lossy(&buf[..len.min(buf.len())]);
        (!path.is_empty()).then(|| PathBuf::from(path))
    }
}

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
