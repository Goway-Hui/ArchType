//! ArchType 的 macOS 输入法外壳。
//!
//! 平台无关的部分（按键、设置、引擎装载、会话驱动、统计、翻译、更新、语音）在
//! `archtype-shell` crate 里，macOS 与 Windows 共用；这里只有 InputMethodKit / AppKit 绑定。

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("archtype-macos 是 macOS 输入法外壳，只能在 macOS 上运行；共用逻辑的测试在 archtype-shell。");
    std::process::exit(1);
}
