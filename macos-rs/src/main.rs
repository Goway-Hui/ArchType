//! ArchType 的 macOS 输入法外壳。
//!
//! 平台无关的模块（按键翻译、设置、引擎加载、会话驱动）在 Linux 上也能编、能测；
//! 只有 `macos` 模块依赖 objc2 / AppKit / InputMethodKit（见 Cargo.toml 的 target 依赖）。

pub mod config;
pub mod engine;
pub mod input;
pub mod keys;
pub mod stats;
pub mod translate;
pub mod voice;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!(
        "archtype-macos 是 macOS 输入法外壳，只能在 macOS 上运行；\
         平台无关的模块用 `cargo test` 测。"
    );
    std::process::exit(1);
}
