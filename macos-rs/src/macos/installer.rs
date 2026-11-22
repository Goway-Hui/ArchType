//! 向系统登记输入法：`--register`、`--enable`、`--select`、`--disable`、`--uninstall`。
//!
//! 和 Squirrel 的做法一样：`TISRegisterInputSource` 登记安装包，`TISEnableInputSource` 打开，
//! `TISSelectInputSource` 切过去。安装包的 postinstall 会在装好后依次调用它们。

use std::ffi::c_void;
use std::path::Path;

use objc2_foundation::NSBundle;

use crate::macos::app::BUNDLE_ID;

/// `noErr`
const NO_ERR: i32 = 0;
/// `kCFStringEncodingUTF8`
const UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    fn CFStringGetCString(string: *const c_void, buffer: *mut i8, size: isize, encoding: u32)
        -> u8;
    fn CFBooleanGetValue(value: *const c_void) -> u8;
    fn CFRelease(value: *const c_void);
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISRegisterInputSource(url: *const c_void) -> i32;
    fn TISCreateInputSourceList(properties: *const c_void, include_all: u8) -> *const c_void;
    fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
    fn TISEnableInputSource(source: *const c_void) -> i32;
    fn TISDisableInputSource(source: *const c_void) -> i32;
    fn TISSelectInputSource(source: *const c_void) -> i32;

    static kTISPropertyBundleID: *const c_void;
    static kTISPropertyInputSourceID: *const c_void;
    static kTISPropertyInputSourceIsEnabled: *const c_void;
    static kTISPropertyInputSourceIsSelected: *const c_void;
}

fn mode_id() -> String {
    format!("{BUNDLE_ID}.Hans")
}

fn bundle_url() -> Option<*const c_void> {
    let url = NSBundle::mainBundle().bundleURL();
    Some(&*url as *const objc2_foundation::NSURL as *const c_void)
}

/// 这个安装包登记过的输入源（includeAllInstalled：没打开的也要列出来）。
fn sources() -> Vec<*const c_void> {
    let mut out = Vec::new();
    let array = unsafe { TISCreateInputSourceList(std::ptr::null(), 1) };
    if array.is_null() {
        return out;
    }
    let count = unsafe { CFArrayGetCount(array) };
    for index in 0..count {
        let source = unsafe { CFArrayGetValueAtIndex(array, index) };
        if source.is_null() {
            continue;
        }
        if property_string(source, unsafe { kTISPropertyBundleID }).as_deref() == Some(BUNDLE_ID) {
            out.push(source);
        }
    }
    unsafe { CFRelease(array) };
    out
}

fn property_string(source: *const c_void, key: *const c_void) -> Option<String> {
    let value = unsafe { TISGetInputSourceProperty(source, key) };
    if value.is_null() {
        return None;
    }
    let mut buffer = [0i8; 1024];
    let ok = unsafe { CFStringGetCString(value, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) };
    if ok == 0 {
        return None;
    }
    let bytes: Vec<u8> = buffer
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8(bytes).ok()
}

fn property_bool(source: *const c_void, key: *const c_void) -> bool {
    let value = unsafe { TISGetInputSourceProperty(source, key) };
    !value.is_null() && unsafe { CFBooleanGetValue(value) } != 0
}

fn mode_source() -> Option<*const c_void> {
    let wanted = mode_id();
    sources().into_iter().find(|source| {
        property_string(*source, unsafe { kTISPropertyInputSourceID }).as_deref()
            == Some(wanted.as_str())
    })
}

/// `--register`
pub fn register() -> bool {
    let Some(url) = bundle_url() else {
        eprintln!("ArchType: 读不到安装包路径");
        return false;
    };
    let status = unsafe { TISRegisterInputSource(url) };
    if status == NO_ERR {
        println!("已登记 {}", NSBundle::mainBundle().bundlePath());
        true
    } else {
        println!("登记失败（{status}）");
        false
    }
}

/// `--enable`
pub fn enable() -> bool {
    let Some(source) = mode_source() else {
        println!(
            "没找到输入源 {}：先 --register，可能要等几秒或重新登录",
            mode_id()
        );
        return false;
    };
    if property_bool(source, unsafe { kTISPropertyInputSourceIsEnabled }) {
        println!("已经打开了");
        return true;
    }
    let status = unsafe { TISEnableInputSource(source) };
    if status == NO_ERR {
        println!("已打开 {}", mode_id());
        true
    } else {
        println!("打开失败（{status}）");
        false
    }
}

/// `--select`
pub fn select() -> bool {
    let Some(source) = mode_source() else {
        println!("没找到输入源 {}", mode_id());
        return false;
    };
    let status = unsafe { TISSelectInputSource(source) };
    if status == NO_ERR {
        println!("已切到 ArchType");
        true
    } else {
        println!("切换失败（{status}）");
        false
    }
}

/// `--disable`
pub fn disable() -> bool {
    let mut ok = true;
    for source in sources() {
        if property_bool(source, unsafe { kTISPropertyInputSourceIsEnabled }) {
            ok = unsafe { TISDisableInputSource(source) } == NO_ERR && ok;
        }
    }
    println!("{}", if ok { "已关闭" } else { "关闭失败" });
    ok
}

/// 系统现在用的输入源是不是 ArchType；问不到是 None。
pub fn is_current_source() -> Option<bool> {
    let source = mode_source();
    source.map(|source| property_bool(source, unsafe { kTISPropertyInputSourceIsSelected }))
}

/// 诊断用：登记了没有、打开了没有、是不是正在用。
pub fn source_summary() -> String {
    match mode_source() {
        None => format!("系统里没有登记（没找到 {}）", mode_id()),
        Some(source) => {
            let enabled = property_bool(source, unsafe { kTISPropertyInputSourceIsEnabled });
            let selected = property_bool(source, unsafe { kTISPropertyInputSourceIsSelected });
            format!(
                "已登记，{}{}",
                if enabled { "已打开" } else { "没打开" },
                if selected { "，正在用" } else { "" }
            )
        }
    }
}

/// `--uninstall`：关掉输入源、停掉别的 ArchType 进程、从 LaunchServices 去掉登记、删掉程序。
/// 用户数据（`~/Library/Application Support/ArchType`）不动。
pub fn uninstall(support_dir: &Path) -> (bool, String) {
    let apps = installed_apps();
    if apps.is_empty() {
        return (
            false,
            "没找到装好的 ArchType.app（~/Library/Input Methods 里没有）".to_string(),
        );
    }
    let _ = disable();

    // 别的 ArchType 进程：输入法（系统按需启动）、另一个设置窗口
    let me = std::process::id();
    let others = running_archtype_pids(me);
    for pid in &others {
        unsafe { kill(*pid as i32, SIGTERM) };
    }
    std::thread::sleep(std::time::Duration::from_secs(1));

    let mut problems = Vec::new();
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/\
                      LaunchServices.framework/Support/lsregister";
    for app in &apps {
        let _ = std::process::Command::new(lsregister)
            .arg("-u")
            .arg(app)
            .status();
        if let Err(error) = std::fs::remove_dir_all(app) {
            problems.push(format!("删不掉 {app}：{error}"));
        }
    }
    // 菜单栏的输入法菜单会缓存输入法列表和图标，重启一下它
    let _ = std::process::Command::new("/usr/bin/killall")
        .arg("TextInputMenuAgent")
        .status();

    let data = support_dir.to_string_lossy();
    if problems.is_empty() {
        (
            true,
            format!(
                "已卸载 ArchType。自定义词、学到的词和设置还在 {data}，不要了可以手动删掉。\
                 输入法列表里还看得到 ArchType 的话，注销重新登录一次就没了。"
            ),
        )
    } else {
        (
            false,
            format!(
                "{}\n可以在访达里手动删掉。用户数据还在 {data}。",
                problems.join("\n")
            ),
        )
    }
}

/// 装好的 ArchType.app：现在跑的这个（在「Input Methods」目录里才算）和 ~/Library/Input Methods 里的那个。
fn installed_apps() -> Vec<String> {
    let mut apps = Vec::new();
    let bundle = NSBundle::mainBundle().bundlePath().to_string();
    if bundle.ends_with("/Input Methods/ArchType.app") {
        apps.push(bundle);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let user = format!("{home}/Library/Input Methods/ArchType.app");
    if Path::new(&user).exists() && !apps.contains(&user) {
        apps.push(user);
    }
    apps
}

/// 还在跑的 ArchType 进程（除了自己）。
fn running_archtype_pids(me: u32) -> Vec<u32> {
    let Ok(output) = std::process::Command::new("/usr/bin/pgrep")
        .arg("-x")
        .arg("ArchType")
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .filter(|pid| *pid != me)
        .collect()
}

const SIGTERM: i32 = 15;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}
