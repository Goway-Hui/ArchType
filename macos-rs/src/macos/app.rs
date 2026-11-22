//! macOS 侧的平台信息：资源目录、数据目录、连接名、版本号。
//!
//! 进程级共用状态（配置热重载、后台载引擎、引擎/翻译器/统计）在 `archtype_shell::runtime`。

use std::path::PathBuf;
use std::sync::LazyLock;

use objc2_foundation::{ns_string, NSBundle, NSString};

use archtype_shell::engine::Layout;
use archtype_shell::runtime::Runtime;

pub use archtype_shell::runtime::{request_terminate, terminate_requested};

/// 安装包里的数据目录：`ArchType.app/Contents/Resources`。
pub fn resource_dir() -> PathBuf {
    let bundle = NSBundle::mainBundle();
    if let Some(path) = bundle.resourcePath() {
        return PathBuf::from(path.to_string());
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .map(|p| p.join("../Resources"))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 用户数据目录：`ARCHTYPE_DATA_DIR` > `~/Library/Application Support/ArchType`。
pub fn support_dir_standard() -> PathBuf {
    if let Ok(dir) = std::env::var("ARCHTYPE_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("ArchType")
}

static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new(Layout::new(resource_dir(), support_dir_standard())));

/// 进程启动时调一次：读设置，后台载入引擎。
pub fn start() -> &'static Runtime {
    let shared = &RUNTIME;
    shared.start_loading();
    shared
}

/// 退出前把学习数据交出去（系统可能随时杀掉输入法进程）。
pub fn shutdown() {
    archtype_shell::runtime::shutdown(&RUNTIME);
}

/// 输入法连接名（Info.plist 的 InputMethodConnectionName）：IMKServer 用它注册，系统按它连进来。
pub fn connection_name() -> String {
    let key = ns_string!("InputMethodConnectionName");
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(key)
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("{}_Connection", bundle_id()))
}

/// 安装包的 bundle id（Info.plist 的 CFBundleIdentifier）。
pub fn bundle_id() -> String {
    NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "io.github.gloridust.inputmethod.ArchType".to_string())
}

/// `ArchType --version` 用的版本号（Info.plist 的 CFBundleShortVersionString）。
pub fn version() -> String {
    let key = ns_string!("CFBundleShortVersionString");
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(key)
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}
