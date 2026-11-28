//! ArchType 的 Windows 外壳：TSF 文本服务（COM in-proc DLL）+ `archtype.exe` 工具。
//!
//! 逻辑在 `archtype-shell` / `archtype-core`，这里只做 Windows 胶水：
//! COM 注册、TSF 激活、键事件、编辑区上屏（M2）。

#![cfg(windows)]

mod candidate_window;
pub mod paths;
pub mod register;
mod text_service;

pub use register::{disable_profile, enable_profile, register, unregister};
pub use text_service::Factory;

use std::ffi::c_void;

use windows::core::{Interface, GUID, HRESULT};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, E_INVALIDARG, S_FALSE, S_OK};
use windows::Win32::System::Com::IClassFactory;

/// 文本服务的 CLSID（固定值，注册表、README、卸载脚本都用它）。
pub const CLSID_TEXTSERVICE: GUID = GUID::from_u128(0x6f2d3c1a_7b4e_4c2d_9e31_2a5b8f0c7d11);
/// 语言 profile 的 GUID（中文 zh-CN 键盘）。
pub const GUID_PROFILE: GUID = GUID::from_u128(0x4a8b2c6d_1f3e_4a9b_8c7d_2e5f6a7b8c9d);
/// 「文本服务」类别。
pub const GUID_TEXTSERVICE_CAT: GUID = GUID::from_u128(0x34745c63_b2f0_4784_8b67_5e12c8701a31);
/// 「键盘」类别。
pub const GUID_KEYBOARD_CAT: GUID = GUID::from_u128(0x13a016df_560b_46cd_947a_4c3af1e0e35d);

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }
    if unsafe { *rclsid } != CLSID_TEXTSERVICE {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory.into();
    unsafe { factory.query(riid, ppv) }
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    // 文本服务被 ctfmon 长期持有，不做卸载判断
    S_FALSE
}

#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    match register::register() {
        Ok(()) => S_OK,
        Err(error) => {
            eprintln!("ArchType: {error}");
            HRESULT(0x80004005u32 as i32)
        }
    }
}

#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    match register::unregister() {
        Ok(()) => S_OK,
        Err(error) => {
            eprintln!("ArchType: {error}");
            HRESULT(0x80004005u32 as i32)
        }
    }
}
