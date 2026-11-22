//! IMK 输入控制器：每个输入框一个。按键翻成核心的 `Key` 交给会话，结果写回输入框和候选框。

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{define_class, msg_send, sel, MainThreadMarker};
use objc2_app_kit::{
    NSAlert, NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem,
    NSPoint, NSRect, NSSize,
};
use objc2_foundation::{NSNotFound, NSObjectProtocol, NSRange, NSString};
use objc2_input_method_kit::IMKInputController;

use crate::config::Config;
use crate::input::{make_key, Input};
use crate::keys::{keysym, virtual_key, KeyInput, Modifiers};
use crate::macos::app::{self, BUNDLE_ID};
use crate::macos::panel::{CandidatePanel, Page};

struct Ivars {
    input: RefCell<Option<Input>>,
    panel: RefCell<Option<CandidatePanel>>,
    /// 上一次给输入框的编辑区和光标（一样就不再发）
    last_marked: RefCell<(String, usize)>,
}

define_class!(
    #[unsafe(super(IMKInputController))]
    #[name = "ArchTypeController"]
    #[ivars = Ivars]
    struct Controller;

    unsafe impl NSObjectProtocol for Controller {}

    impl Controller {
        #[unsafe(method(initWithServer:delegate:client:))]
        fn init_with_server(
            this: objc2::rc::Allocated<Self>,
            server: Option<&objc2_input_method_kit::IMKServer>,
            delegate: Option<&AnyObject>,
            client: Option<&AnyObject>,
        ) -> Option<Retained<Self>> {
            let this = this.set_ivars(Ivars {
                input: RefCell::new(None),
                panel: RefCell::new(None),
                last_marked: RefCell::new((String::new(), 0)),
            });
            unsafe {
                msg_send![super(this), initWithServer: server, delegate: delegate, client: client]
            }
        }

        /// 系统把按键交给我们：返回 true 表示这个键不交给应用。
        #[unsafe(method(handleEvent:client:))]
        fn handle_event(&self, event: Option<&NSEvent>, client: Option<&AnyObject>) -> Bool {
            let (Some(event), Some(client)) = (event, client) else {
                return Bool::NO;
            };
            if !self.prepare(client) {
                return Bool::NO;
            }
            let event_type = event.r#type();
            if event_type == NSEventType::KeyDown {
                self.handle_key_down(event, client)
            } else if event_type == NSEventType::FlagsChanged {
                self.handle_flags_changed(event, client);
                Bool::NO
            } else {
                Bool::NO
            }
        }

        #[unsafe(method(activateServer:))]
        fn activate(&self, client: Option<&AnyObject>) {
            *self.ivars().last_marked.borrow_mut() = (String::new(), 0);
            self.prepare(client);
        }

        #[unsafe(method(deactivateServer:))]
        fn deactivate(&self, client: Option<&AnyObject>) {
            self.flush(client);
            self.hide_panel();
        }

        #[unsafe(method(commitComposition:))]
        fn commit_composition(&self, client: Option<&AnyObject>) {
            self.flush(client);
        }

        #[unsafe(method(hidePalettes))]
        fn hide_palettes(&self) {
            self.hide_panel();
        }

        #[unsafe(method_id(menu))]
        fn menu(&self) -> Option<Retained<NSMenu>> {
            let mtm = MainThreadMarker::new()?;
            let menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("ArchType"));
            let settings = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc(),
                    &NSString::from_str("设置…"),
                    Some(sel!(openSettings:)),
                    &NSString::from_str(","),
                )
            };
            unsafe { settings.setTarget(Some(self.as_any())) };
            menu.addItem(&settings);
            let about = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc(),
                    &NSString::from_str("关于 ArchType"),
                    Some(sel!(showAbout:)),
                    &NSString::from_str(""),
                )
            };
            unsafe { about.setTarget(Some(self.as_any())) };
            menu.addItem(&about);
            Some(menu)
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            open_settings_app(&self.ivars());
        }

        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            show_about_panel();
        }
    }
);

impl Controller {
    fn as_any(&self) -> &AnyObject {
        // SAFETY: 所有 NSObject 子类都布局成 AnyObject；这里只用来当菜单项的 target。
        unsafe { &*(self as *const Self as *const AnyObject) }
    }

    /// 引擎就绪了就把会话建好（第一次按键、激活时可能还没载入完）。
    fn prepare(&self, client: Option<&AnyObject>) -> bool {
        let shared = app::start();
        let Some(engine) = shared.engine() else {
            return false;
        };
        let mut input = self.ivars().input.borrow_mut();
        if input.is_none() {
            *input = Some(Input::attach(
                engine,
                shared.layout.clone(),
                shared.config(),
            ));
        }
        let mut config_changed = false;
        if let Some(config) = shared.reload_config_if_changed() {
            if let Some(input) = input.as_mut() {
                let problems = input.apply_config(&config, translate_available(&config), false);
                for problem in problems {
                    eprintln!("ArchType: {problem}");
                }
            }
            config_changed = true;
        }
        if config_changed {
            self.hide_panel();
        }
        let _ = client;
        true
    }

    fn handle_key_down(&self, event: &NSEvent, client: &AnyObject) -> Bool {
        let key_code = event.keyCode();
        let characters = event.characters().map(|s| s.to_string());
        let ignoring = event.charactersIgnoringModifiers().map(|s| s.to_string());
        let flags = event.modifierFlags().0 as u64;
        let Some(translated) = crate::keys::translate_key_down(
            key_code,
            characters.as_deref(),
            ignoring.as_deref(),
            flags,
        ) else {
            // 翻不出来的键（F 键、别的键盘布局打出的非 ASCII 字符）：打字途中吃掉，
            // 免得应用里的光标跑到别处
            let mut input = self.ivars().input.borrow_mut();
            if let Some(input) = input.as_mut() {
                if input.composing() {
                    return true;
                }
            }
            return false;
        };

        let time_ms = (event.timestamp() * 1000.0) as u64;
        let repeat = event.isARepeat();
        let key = make_key(
            translated.keysym,
            translated.modifiers.raw(),
            translated.release,
            None,
            None,
            0,
            key_code as u32,
            repeat,
            time_ms,
        );

        let frame = {
            let mut input = self.ivars().input.borrow_mut();
            let Some(input) = input.as_mut() else {
                return false;
            };
            input.key(&key)
        };

        self.apply_frame(&frame, client);
        frame.handled
    }

    /// 修饰键：Shift 单按切中英（会话按按住多久、有没有按别的键判断）。
    fn handle_flags_changed(&self, event: &NSEvent, _client: &AnyObject) {
        let key_code = event.keyCode();
        if key_code != virtual_key::SHIFT_LEFT && key_code != virtual_key::SHIFT_RIGHT {
            return;
        }
        let down = event.modifierFlags().contains(NSEventModifierFlags::Shift);
        let keysym = if key_code == virtual_key::SHIFT_LEFT {
            keysym::SHIFT_L
        } else {
            keysym::SHIFT_R
        };
        let time_ms = (event.timestamp() * 1000.0) as u64;
        let key = make_key(
            keysym,
            0,
            !down,
            None,
            None,
            0,
            key_code as u32,
            false,
            time_ms,
        );
        let mut input = self.ivars().input.borrow_mut();
        if let Some(input) = input.as_mut() {
            let _ = input.key(&key);
        }
    }

    /// 把会话的结果写回输入框和候选框。
    fn apply_frame(&self, frame: &archtype_core::session::Frame, client: &AnyObject) {
        if let Some(text) = &frame.commit {
            let ns = NSString::from_str(text);
            let range = NSRange::new(NSNotFound as usize, 0);
            unsafe {
                let _: () = msg_send![client, insertText: &*ns, replacementRange: range];
            }
        }

        let marked = (
            frame.preedit.clone(),
            utf16_offset(&frame.preedit, frame.preedit_cursor),
        );
        if marked.0.is_empty() {
            self.clear_marked(client);
        } else if *self.ivars().last_marked.borrow() != marked {
            let ns = NSString::from_str(&marked.0);
            let selection = NSRange::new(marked.1.min(marked.0.len()), 0);
            let range = NSRange::new(NSNotFound as usize, 0);
            unsafe {
                let _: () = msg_send![client, setMarkedText: &*ns, selectionRange: selection, replacementRange: range];
            }
            *self.ivars().last_marked.borrow_mut() = marked;
        }

        self.update_panel(frame, client);
    }

    fn clear_marked(&self, client: &AnyObject) {
        if self.ivars().last_marked.borrow().0.is_empty() {
            return;
        }
        let empty = NSString::from_str("");
        let range = NSRange::new(NSNotFound as usize, 0);
        unsafe {
            let _: () = msg_send![client, setMarkedText: &*empty, selectionRange: range, replacementRange: range];
        }
        *self.ivars().last_marked.borrow_mut() = (String::new(), 0);
    }

    /// 候选框：会话给的一页，画在光标下面。
    fn update_panel(&self, frame: &archtype_core::session::Frame, client: &AnyObject) {
        let page = {
            let mut input = self.ivars().input.borrow_mut();
            let Some(input) = input.as_mut() else {
                return;
            };
            if !input.composing() {
                self.hide_panel();
                return;
            }
            let shown = input.shown();
            let mut texts = Vec::with_capacity(shown.len());
            let mut comments = Vec::with_capacity(shown.len());
            for candidate in shown {
                texts.push(candidate.text.clone());
                comments.push(candidate.comment.clone());
            }
            Page {
                texts,
                comments,
                cursor: frame.cand_cursor,
                has_prev: frame.has_prev,
                has_next: frame.has_next,
                vertical: !input.config().horizontal,
                expanded: matches!(frame.panel, archtype_core::session::Panel::Expanded),
                aux: None,
                font_size: input.config().candidate_font_size,
            }
        };
        if page.is_empty() {
            self.hide_panel();
            return;
        }

        let panel = {
            let mut panel = self.ivars().panel.borrow_mut();
            if panel.is_none() {
                let Some(mtm) = MainThreadMarker::new() else {
                    return;
                };
                *panel = Some(CandidatePanel::new(mtm));
            }
            panel.as_ref().unwrap()
        };
        panel.show(&page, caret_rect(client));
    }

    fn hide_panel(&self) {
        if let Some(panel) = self.ivars().panel.borrow().as_ref() {
            panel.hide();
        }
    }

    /// 把还没上屏的输入按当前状态交出去（切输入法、提交）。
    fn flush(&self, client: Option<&AnyObject>) {
        let frame = {
            let mut input = self.ivars().input.borrow_mut();
            match input.as_mut() {
                Some(input) => input.reset(true),
                None => return,
            }
        };
        if let Some(client) = client {
            self.apply_frame(&frame, client);
        }
    }
}

/// 编辑区的光标：核心给的是 UTF-8 字节偏移，`setMarkedText` 要 UTF-16 码元偏移。
fn utf16_offset(text: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(text.len());
    let mut boundary = clamped;
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    text[..boundary].encode_utf16().count()
}

/// 配好了翻译服务（有本地模型或填了接口）才给「译」这一档。
fn translate_available(config: &Config) -> bool {
    config.translate && (config.translate_local || !config.translate_base_url.is_empty())
}

/// 光标（插入点）的屏幕矩形，候选框画在它下面。
fn caret_rect(client: &AnyObject) -> NSRect {
    let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
    unsafe {
        let _: *mut AnyObject = msg_send![
            client,
            attributesForCharacterIndex: 0usize,
            lineHeightRectangle: &mut rect
        ];
    }
    rect
}

/// 输入法菜单里的「设置…」：另起一个进程打开设置窗口（输入法进程里开窗口会和输入框抢焦点）。
fn open_settings_app(_ivars: &Ivars) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe).arg("--settings").spawn();
}

fn show_about_panel() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("ArchType 输入法"));
    let text = format!("版本 {}（{}）", app::version(), BUNDLE_ID);
    alert.setInformativeText(&NSString::from_str(&text));
    alert.runModal();
    let _ = NSApplication::sharedApplication(mtm);
}

/// 给 `mod.rs` 用：当前进程的输入控制器类名要和 Info.plist 一致。
#[allow(dead_code)]
pub fn controller_class_name() -> &'static str {
    "ArchTypeController"
}
