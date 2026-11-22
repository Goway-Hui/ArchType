//! macOS 输入法的 Rust 外壳 POC：验证 `objc2-input-method-kit` 能不能替代 Swift 外壳。
//!
//! 目标链路：系统按 Info.plist 的 `InputMethodServerControllerClass = ArchTypeController`
//! 实例化下面的控制器 → `handleEvent:client:` 收到按键 → 直接上屏。
//! 跑通以后再接 `archtype-core`（引擎/会话），把候选框、语音、菜单逐步搬过来。
//!
//! 构建（只能在 macOS 上）：`cargo build --release --manifest-path macos-rs/Cargo.toml`
//! 打包：需要把可执行文件放进 `.app/Contents/MacOS/`，Info.plist 的
//! `InputMethodConnectionName` / `InputMethodServerControllerClass` 指到本进程和 `ArchTypeController`。

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{define_class, AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSEvent, NSMenu};
use objc2_foundation::{NSNotFound, NSObjectProtocol, NSRange, NSString};

struct Ivars;

define_class!(
    #[unsafe(super(objc2_input_method_kit::IMKInputController))]
    #[name = "ArchTypeController"]
    #[ivars = Ivars]
    struct ArchTypeController;

    unsafe impl NSObjectProtocol for ArchTypeController {}

    impl ArchTypeController {
        /// POC：普通字符直接交给应用上屏；功能键/组合键原样放行。
        #[unsafe(method(handleEvent:client:))]
        fn handle_event(&self, event: Option<&NSEvent>, client: Option<&AnyObject>) -> Bool {
            let (Some(event), Some(client)) = (event, client) else {
                return Bool::NO;
            };
            let Some(chars) = (unsafe { event.characters() }) else {
                return Bool::NO;
            };
            let len = unsafe { chars.length() };
            if len == 0 {
                return Bool::NO;
            }
            unsafe {
                let _: () = msg_send![client, insertText: &*chars,
                                       replacementRange: NSRange::new(NSNotFound as usize, 0)];
            }
            Bool::YES
        }

        /// 输入法菜单：先交系统默认（设置界面由 NSH app 提供，后续在这里挂「设置…」）。
        #[unsafe(method_id(menu))]
        fn menu(&self) -> Option<Retained<NSMenu>> {
            None
        }
    }
);

pub fn run() {
    let mtm = MainThreadMarker::new().expect("输入法外壳必须跑在主线程");

    // 名字要和 Info.plist 的 InputMethodConnectionName 一致；bundle id 用本产品的。
    let name = NSString::from_str("ArchType_Connection");
    let bundle = NSString::from_str("io.github.gloridust.inputmethod.ArchType");
    let server = unsafe {
        objc2_input_method_kit::IMKServer::initWithName_bundleIdentifier(
            objc2_input_method_kit::IMKServer::alloc(),
            Some(&name),
            Some(&bundle),
        )
    };

    let app = unsafe { NSApplication::sharedApplication(mtm) };
    unsafe { app.run() };

    std::mem::forget(server);
}
