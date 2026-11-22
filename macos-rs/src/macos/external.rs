//! 用命令切中 / 英 / 译（vim 退出插入模式时切英文这类用法），和 Linux 的会话总线一样。
//!
//! macOS 没有会话总线，用分布式通知：命令行（`ArchType --mode [zh|en|translate|toggle]`）
//! 发一个请求，输入法切最近用过的输入框，再把切完的模式发回来。

use std::cell::RefCell;
use std::sync::atomic::{AtomicPtr, Ordering};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass};
use objc2_foundation::{
    NSDate, NSDefaultRunLoopMode, NSDistributedNotificationCenter, NSNotification,
    NSNotificationName, NSNotificationSuspensionBehavior, NSRunLoop, NSString,
};

/// 请求：object 是要切到的模式（zh、en、translate、toggle），空串只问现在的模式
const REQUEST: &str = "io.github.gloridust.ArchType.SetMode";
/// 回复：object 是切完以后的模式（zh、en、translate）；还没有输入框用过 ArchType 时是空串
const REPLY: &str = "io.github.gloridust.ArchType.Mode";

/// 最近启用的输入框（不持有它：系统管它的生命周期，deactivate 时清掉）。
static ACTIVE: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());

/// 输入框启用了（activateServer）。
pub fn set_active(controller: &AnyObject) {
    ACTIVE.store(
        controller as *const AnyObject as *mut AnyObject,
        Ordering::SeqCst,
    );
}

/// 输入框停用了（deactivateServer）：只清掉自己，别把别的输入框挤掉。
pub fn clear_active(controller: &AnyObject) {
    let me = controller as *const AnyObject as *mut AnyObject;
    let _ = ACTIVE.compare_exchange(me, std::ptr::null_mut(), Ordering::SeqCst, Ordering::SeqCst);
}

fn active() -> Option<&'static AnyObject> {
    let pointer = ACTIVE.load(Ordering::SeqCst);
    if pointer.is_null() {
        None
    } else {
        // SAFETY: 指针是 activateServer 时从系统持有的控制器来的，deactivate 时清掉；
        // 生命周期由系统保证到 deactivate 为止。
        Some(unsafe { &*pointer })
    }
}

struct ListenerIvars;

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[name = "ArchTypeModeListener"]
    #[ivars = ListenerIvars]
    struct ModeListener;

    impl ModeListener {
        #[unsafe(method(receivedMode:))]
        fn received_mode(&self, note: Option<&NSNotification>) {
            let request = note
                .and_then(|note| note.object())
                .and_then(|object| object.downcast::<NSString>().ok())
                .map(|value| value.to_string())
                .unwrap_or_default();
            let reply = dispatch(&request);
            post_reply(&reply);
        }
    }
);

impl ModeListener {
    fn make() -> Retained<Self> {
        let this = Self::alloc().set_ivars(ListenerIvars);
        unsafe { msg_send![super(this), init] }
    }
}

/// 输入法进程里启动：收 `--mode` 的请求。
pub fn start_listener() {
    let center = NSDistributedNotificationCenter::defaultCenter();
    let listener = ModeListener::make();
    let name = NSString::from_str(REQUEST);
    let observer: &AnyObject = unsafe { &*(&*listener as *const ModeListener as *const AnyObject) };
    unsafe {
        center.addObserver_selector_name_object_suspensionBehavior(
            observer,
            sel!(receivedMode:),
            Some(&name),
            None,
            NSNotificationSuspensionBehavior::DeliverImmediately,
        );
    }
    // 观察者要活到进程结束
    std::mem::forget(listener);
}

/// 把请求交给最近的输入框，返回切完的模式（没有输入框时是空串）。
fn dispatch(request: &str) -> String {
    let Some(controller) = active() else {
        return String::new();
    };
    let request = NSString::from_str(request);
    let reply: Option<Retained<NSString>> =
        unsafe { msg_send![controller, externalMode: &*request] };
    reply.map(|value| value.to_string()).unwrap_or_default()
}

fn post_reply(mode: &str) {
    let center = NSDistributedNotificationCenter::defaultCenter();
    let name = NSString::from_str(REPLY);
    let mode = NSString::from_str(mode);
    unsafe {
        center.postNotificationName_object_userInfo_deliverImmediately(
            &name,
            Some(&mode),
            None,
            true,
        );
    }
}

struct ClientIvars {
    answer: RefCell<Option<String>>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[name = "ArchTypeModeClient"]
    #[ivars = ClientIvars]
    struct ModeClient;

    impl ModeClient {
        #[unsafe(method(repliedMode:))]
        fn replied_mode(&self, note: Option<&NSNotification>) {
            let answer = note
                .and_then(|note| note.object())
                .and_then(|object| object.downcast::<NSString>().ok())
                .map(|value| value.to_string())
                .unwrap_or_default();
            *self.ivars().answer.borrow_mut() = Some(answer);
        }
    }
);

impl ModeClient {
    fn make() -> Retained<Self> {
        let this = Self::alloc().set_ivars(ClientIvars {
            answer: RefCell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// 命令行这边：发请求，等输入法回话（最多 2 秒）。
pub fn request_mode(request: &str) -> Option<String> {
    let center = NSDistributedNotificationCenter::defaultCenter();
    let client = ModeClient::make();
    let reply_name = NSString::from_str(REPLY);
    let observer: &AnyObject = unsafe { &*(&*client as *const ModeClient as *const AnyObject) };
    unsafe {
        center.addObserver_selector_name_object_suspensionBehavior(
            observer,
            sel!(repliedMode:),
            Some(&reply_name),
            None,
            NSNotificationSuspensionBehavior::DeliverImmediately,
        );
    }
    let request_name = NSString::from_str(REQUEST);
    let value = NSString::from_str(request);
    unsafe {
        center.postNotificationName_object_userInfo_deliverImmediately(
            &request_name,
            Some(&value),
            None,
            true,
        );
    }

    let deadline = NSDate::dateWithTimeIntervalSinceNow(2.0);
    let runloop = NSRunLoop::currentRunLoop();
    loop {
        if client.ivars().answer.borrow().is_some() {
            break;
        }
        let now = NSDate::dateWithTimeIntervalSinceNow(0.0);
        if now.compare(&deadline) != objc2_foundation::NSComparisonResult::Ascending {
            break;
        }
        let limit = NSDate::dateWithTimeIntervalSinceNow(0.05);
        // SAFETY: 系统的默认 run loop mode 常量。
        let mode: &NSNotificationName = unsafe { NSDefaultRunLoopMode };
        runloop.runMode_beforeDate(mode, &limit);
    }
    unsafe {
        center.removeObserver_name_object(observer, None::<&NSNotificationName>, None);
    }
    let answer = client.ivars().answer.borrow().clone();
    answer
}

/// `--mode` 的实现：打印模式，退出码按是否有输入框。
pub fn mode_command(request: &str) -> bool {
    match request_mode(request) {
        Some(mode) if !mode.is_empty() => {
            println!("{mode}");
            true
        }
        _ => {
            eprintln!("ArchType 输入法没在运行（切到 ArchType 打一个字以后再试）");
            false
        }
    }
}
