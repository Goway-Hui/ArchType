//! IMK 输入控制器：每个输入框一个。按键翻成核心的 `Key` 交给会话，结果写回输入框和候选框。

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType,
    NSMenu, NSMenuItem, NSPasteboard, NSPasteboardTypeString, NSTextField,
};
use objc2_foundation::{
    NSNotFound, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString, NSTimer,
};
use objc2_input_method_kit::IMKInputController;

use archtype_core::session::Mode;

use archtype_shell::config::{InitialMode, WindowMode};
use archtype_shell::input::{make_key, Input};
use archtype_shell::keys::{keysym, virtual_key, VoiceKey};
use crate::macos::app::{self};
use crate::macos::panel::CandidatePanel;
use archtype_shell::voice::{self, Voice};
use archtype_shell::window_mode::WindowFocus;

struct Ivars {
    input: RefCell<Option<Input>>,
    panel: RefCell<Option<CandidatePanel>>,
    /// 上一次给输入框的编辑区和光标（一样就不再发）
    last_marked: RefCell<(String, usize)>,
    /// 语音输入（第一次用到时建）
    voice: RefCell<Option<Voice>>,
    /// 语音键（设置里的 VoiceKey 翻成 macOS 键码）
    voice_key: Cell<Option<VoiceKey>>,
    /// 语音状态轮询的定时器
    voice_timer: RefCell<Option<Retained<NSTimer>>>,
    /// 这个输入框是不是已经按 InitialMode 定过模式
    initial_mode_done: Cell<bool>,
    /// 上一次的应用 / 窗口（WindowMode）
    focus: RefCell<WindowFocus>,
    /// 面板现在显示的是语音提示（不是候选）
    voice_panel: Cell<bool>,
}

define_class!(
    #[unsafe(super(IMKInputController))]
    #[name = "ArchTypeController"]
    #[ivars = Ivars]
    struct Controller;

    unsafe impl NSObjectProtocol for Controller {}

    impl Controller {
        #[unsafe(method_id(initWithServer:delegate:client:))]
        #[unsafe(method_family = init)]
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
                voice: RefCell::new(None),
                voice_key: Cell::new(None),
                voice_timer: RefCell::new(None),
                initial_mode_done: Cell::new(false),
                focus: RefCell::new(WindowFocus::new()),
                voice_panel: Cell::new(false),
            });
            unsafe {
                msg_send![super(this), initWithServer: server, delegate: delegate, client: client]
            }
        }

        /// 按键、松开、修饰键都要（语音键要看到松开；F 键松开才算说完）。
        #[unsafe(method(recognizedEvents:))]
        fn recognized_events(&self, _sender: Option<&AnyObject>) -> usize {
            (NSEventMask::KeyDown | NSEventMask::KeyUp | NSEventMask::FlagsChanged).0 as usize
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
                if self.voice_key_down(event) {
                    return Bool::YES;
                }
                self.handle_key_down(event, client)
            } else if event_type == NSEventType::KeyUp {
                if self.voice_key_up(event) {
                    return Bool::YES;
                }
                Bool::NO
            } else if event_type == NSEventType::FlagsChanged {
                if self.voice_modifier_changed(event) {
                    return Bool::NO;
                }
                self.handle_flags_changed(event, client);
                Bool::NO
            } else {
                Bool::NO
            }
        }

        /// 语音状态轮询：定时器每 50 毫秒调一次（录音中看音量、松开后等识别结果）。
        #[unsafe(method(pollVoice:))]
        fn poll_voice(&self, _timer: Option<&NSTimer>) {
            if app::terminate_requested() {
                app::shutdown();
                std::process::exit(0);
            }
            let state = {
                let voice = self.ivars().voice.borrow();
                match voice.as_ref() {
                    Some(voice) => voice.poll(),
                    None => return,
                }
            };
            if state == voice::VOICE_LISTENING {
                let level = self
                    .ivars()
                    .voice
                    .borrow()
                    .as_ref()
                    .map(|voice| voice.level())
                    .unwrap_or(0);
                self.show_voice_hint(&format!("正在听… {}", archtype_shell::keys::voice_wave(&[level])));
            } else if state == voice::VOICE_RECOGNIZING {
                self.show_voice_hint("识别中…");
            } else if state == voice::VOICE_IDLE && self.ivars().voice_panel.get() {
                self.ivars().voice_panel.set(false);
                self.hide_panel();
            }
            if state == voice::VOICE_RESULT {
                let text = self
                    .ivars()
                    .voice
                    .borrow()
                    .as_ref()
                    .map(|voice| voice.text())
                    .unwrap_or_default();
                if !text.is_empty() {
                    if let Some(client) = self.current_client() {
                        insert_text(&client, &text);
                        app::start().stats().flush();
                    }
                }
                if let Some(voice) = self.ivars().voice.borrow().as_ref() {
                    voice.cancel();
                }
                self.ivars().voice_panel.set(false);
                self.hide_panel();
            } else if state == voice::VOICE_ERROR {
                if let Some(voice) = self.ivars().voice.borrow().as_ref() {
                    voice.cancel();
                }
                self.ivars().voice_panel.set(false);
                self.hide_panel();
            }
        }

        #[unsafe(method(activateServer:))]
        fn activate(&self, client: Option<&AnyObject>) {
            *self.ivars().last_marked.borrow_mut() = (String::new(), 0);
            super::external::set_active(self.as_any());
            if let Some(client) = client {
                if self.prepare(client) {
                    self.apply_modes_on_activate(client);
                }
            }
            self.sample_clipboard();
        }

        #[unsafe(method(deactivateServer:))]
        fn deactivate(&self, client: Option<&AnyObject>) {
            super::external::clear_active(self.as_any());
            self.flush(client);
            self.hide_panel();
            app::start().stats().flush();
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
            (|| -> Option<Retained<NSMenu>> {
            let mtm = MainThreadMarker::new()?;
            let menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("ArchType"));
            let version = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc(),
                    &NSString::from_str(&format!("ArchType {}", app::version())),
                    None,
                    &NSString::from_str(""),
                )
            };
            version.setEnabled(false);
            menu.addItem(&version);
            menu.addItem(&NSMenuItem::separatorItem(mtm));
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
            let check = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc(),
                    &NSString::from_str("检查更新…"),
                    Some(sel!(checkUpdate:)),
                    &NSString::from_str(""),
                )
            };
            unsafe { check.setTarget(Some(self.as_any())) };
            menu.addItem(&check);
            for (title, action) in [
                ("自定义词…", sel!(openCustomWords:)),
                ("设置互译 API Key…", sel!(editTranslateKey:)),
                ("打开数据目录", sel!(openDataFolder:)),
            ] {
                let item = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        mtm.alloc(),
                        &NSString::from_str(title),
                        Some(action),
                        &NSString::from_str(""),
                    )
                };
                unsafe { item.setTarget(Some(self.as_any())) };
                menu.addItem(&item);
            }
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
            })()
        }

        /// `ArchType --mode`：切最近用过的输入框，返回切完的模式（zh/en/translate）。
        #[unsafe(method_id(externalMode:))]
        fn external_mode(&self, request: Option<&NSString>) -> Option<Retained<NSString>> {
            (|| -> Option<Retained<NSString>> {
                let request = request.map(|value| value.to_string())?;
                let mode_name = {
                    let mut input = self.ivars().input.borrow_mut();
                    let input = input.as_mut()?;
                    let target = match request.as_str() {
                        "zh" => Some(Mode::Chinese),
                        "en" => Some(Mode::English),
                        "translate" => Some(Mode::Translate),
                        "toggle" => Some(match input.mode() {
                            Mode::Chinese => Mode::English,
                            Mode::English | Mode::Translate => Mode::Chinese,
                        }),
                        _ => None,
                    };
                    if let Some(target) = target {
                        let frame = input.set_mode(target);
                        if let Some(client) = self.current_client() {
                            self.apply_frame(&frame, &client);
                        }
                    }
                    mode_name(input.mode())
                };
                Some(NSString::from_str(mode_name))
            })()
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            open_settings_app(None);
        }

        /// 菜单「检查更新…」：问发布页有没有新版，有就下载 pkg 装上（只装给当前用户）。
        #[unsafe(method(checkUpdate:))]
        fn check_update(&self, _sender: Option<&AnyObject>) {
            let current = app::version();
            let Some(latest) = archtype_shell::update::latest_version() else {
                show_message("检查更新", "连不上发布页，过一会儿再试。");
                return;
            };
            if !archtype_shell::update::is_newer(&latest, &current) {
                show_message("检查更新", &format!("已经是最新版（{current}）。"));
                return;
            }
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let alert = NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str(&format!("ArchType {latest} 可以更新")));
            alert.setInformativeText(&NSString::from_str(
                "下载并安装新版（只装给当前用户，不需要管理员密码）。",
            ));
            alert.addButtonWithTitle(&NSString::from_str("下载并安装"));
            alert.addButtonWithTitle(&NSString::from_str("取消"));
            if alert.runModal() != NSAlertFirstButtonReturn {
                return;
            }
            let layout = app::start().layout.clone();
            let base = format!(
                "https://github.com/Goway-Hui/ArchType/releases/download/v{latest}"
            );
            match super::update::install(&latest, &base, &layout) {
                Ok(()) => show_message("检查更新", "装好了。下次切到 ArchType 就是新版。"),
                Err(error) => show_message("检查更新", &format!("没装好：{error}")),
            }
        }

        /// 菜单「自定义词…」：打开设置窗口的「自定义词」页（没有 custom.tsv 时由界面建）。
        #[unsafe(method(openCustomWords:))]
        fn open_custom_words(&self, _sender: Option<&AnyObject>) {
            open_settings_app(Some("words"));
        }

        /// 菜单「打开数据目录」。
        #[unsafe(method(openDataFolder:))]
        fn open_data_folder(&self, _sender: Option<&AnyObject>) {
            let layout = &app::start().layout;
            let _ = std::fs::create_dir_all(&layout.support_dir);
            let _ = std::process::Command::new("/usr/bin/open")
                .arg(&layout.support_dir)
                .status();
        }

        /// 菜单「设置互译 API Key…」：一个输入框，写 translate.key（0600；空串删掉）。
        #[unsafe(method(editTranslateKey:))]
        fn edit_translate_key(&self, _sender: Option<&AnyObject>) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let layout = &app::start().layout;
            let key_path = layout.support_dir.join("translate.key");
            let current = std::fs::read_to_string(&key_path).unwrap_or_default();
            let alert = NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str("互译 API Key"));
            alert.setInformativeText(&NSString::from_str(
                "只保存在这台电脑上（translate.key，只有你能读）。留空表示不用 Key（本地模型、哔哩哔哩）。",
            ));
            let field = NSTextField::initWithFrame(
                mtm.alloc(),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 24.0)),
            );
            field.setStringValue(&NSString::from_str(current.trim()));
            alert.setAccessoryView(Some(&field));
            alert.addButtonWithTitle(&NSString::from_str("保存"));
            alert.addButtonWithTitle(&NSString::from_str("取消"));
            if alert.runModal() != NSAlertFirstButtonReturn {
                return;
            }
            let value = field.stringValue().to_string();
            let _ = std::fs::create_dir_all(&layout.support_dir);
            if value.trim().is_empty() {
                let _ = std::fs::remove_file(&key_path);
            } else if std::fs::write(&key_path, format!("{}
", value.trim())).is_ok() {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
            }
        }

        /// 面板上点了一个候选（第几个，按显示的顺序）。
        #[unsafe(method(candidateClicked:))]
        fn candidate_clicked(&self, index: usize) {
            let frame = {
                let mut input = self.ivars().input.borrow_mut();
                match input.as_mut() {
                    Some(input) => input.select(index),
                    None => return,
                }
            };
            if let Some(client) = self.current_client() {
                self.apply_frame(&frame, &client);
            }
            let composing = self
                .ivars()
                .input
                .borrow()
                .as_ref()
                .map(|input| input.composing())
                .unwrap_or(false);
            if !composing {
                self.ivars().voice_panel.set(false);
                self.hide_panel();
            }
        }

        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            show_about_panel();
        }
    }
);

impl Controller {
    /// 这个输入框能不能按位置改字（自动配对要能挪光标、替换旁边的字）。
    /// 终端这类应用读不到光标周围，照 Swift 版的做法直接不配对。
    fn can_edit_around_caret(&self) -> bool {
        let Some(client) = self.current_client() else {
            return false;
        };
        let bundle: Option<Retained<NSString>> = unsafe { msg_send![&*client, bundleIdentifier] };
        let id = bundle.map(|value| value.to_string()).unwrap_or_default();
        const TERMINALS: [&str; 6] = [
            "com.apple.Terminal",
            "com.googlecode.iterm2",
            "net.kovidgoyal.kitty",
            "org.alacritty",
            "com.github.wez.wezterm",
            "io.alacritty",
        ];
        if TERMINALS
            .iter()
            .any(|terminal| id.eq_ignore_ascii_case(terminal))
        {
            return false;
        }
        let responds: bool =
            unsafe { msg_send![&*client, respondsToSelector: sel!(selectedRange)] };
        responds
    }

    /// 切换输入框时看一眼剪贴板（设置里开了「记录剪贴板历史」才记，只在内存里）。
    fn sample_clipboard(&self) {
        if !app::start().config().clipboard_history {
            return;
        }
        static LAST_CHANGE: std::sync::atomic::AtomicIsize =
            std::sync::atomic::AtomicIsize::new(-1);
        let pasteboard = NSPasteboard::generalPasteboard();
        let change = pasteboard.changeCount();
        if change == LAST_CHANGE.swap(change, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        // 密码管理器复制的密码会标成 Concealed / Transient / AutoGenerated：这类不记
        let secret = pasteboard.types().is_some_and(|types| {
            types.iter().any(|kind| {
                let name = kind.to_string();
                name.contains("Concealed")
                    || name.contains("Transient")
                    || name.contains("AutoGenerated")
            })
        });
        if secret {
            return;
        }
        // SAFETY: 系统的 NSString 类型常量。
        let kind = unsafe { NSPasteboardTypeString };
        if let Some(text) = pasteboard.stringForType(kind) {
            archtype_core::clipboard::remember(&text.to_string());
        }
    }

    /// 语音提示：在光标下面画一行（正在听 + 波形、识别中）。
    fn show_voice_hint(&self, text: &str) {
        let Some(client) = self.current_client() else {
            return;
        };
        let font_size = app::start().config().candidate_font_size;
        let page = archtype_shell::view::voice_hint_page(text, font_size);
        let mut panel = self.ivars().panel.borrow_mut();
        if panel.is_none() {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let created = CandidatePanel::new(mtm);
            created.set_target(self.as_any());
            *panel = Some(created);
        }
        if let Some(panel) = panel.as_ref() {
            panel.show(&page, caret_rect(&client));
            self.ivars().voice_panel.set(true);
        }
    }

    /// 输入框启用了：进程里第一次按 InitialMode 定中 / 英；换了应用 / 窗口按 WindowMode 定一次。
    fn apply_modes_on_activate(&self, _client: &AnyObject) {
        let shared = app::start();
        let config = shared.config();
        let mut target: Option<Mode> = None;
        if !self.ivars().initial_mode_done.get() {
            self.ivars().initial_mode_done.set(true);
            target = match config.initial_mode {
                InitialMode::Chinese => Some(Mode::Chinese),
                InitialMode::English => Some(Mode::English),
                InitialMode::Last => None,
            };
        }
        if let Some(key) = self.focus_key() {
            let moved = self.ivars().focus.borrow_mut().moved(&key);
            if moved && target.is_none() {
                target = match config.window_mode {
                    WindowMode::Chinese => Some(Mode::Chinese),
                    WindowMode::English => Some(Mode::English),
                    WindowMode::Same => None,
                };
            }
        }
        let Some(mode) = target else {
            return;
        };
        let frame = {
            let mut input = self.ivars().input.borrow_mut();
            match input.as_mut() {
                Some(input) => input.set_mode(mode),
                None => return,
            }
        };
        if let Some(client) = self.current_client() {
            self.apply_frame(&frame, &client);
        }
    }

    /// 现在这个输入框在哪个应用、哪个窗口（问不到窗口号时只看应用）。
    fn focus_key(&self) -> Option<String> {
        let client = self.current_client()?;
        let bundle: Option<Retained<NSString>> = unsafe { msg_send![&*client, bundleIdentifier] };
        let app = bundle.map(|value| value.to_string()).unwrap_or_default();
        let window: Option<Retained<AnyObject>> = unsafe { msg_send![&*client, window] };
        let number: Option<isize> =
            window.map(|window| unsafe { msg_send![&*window, windowNumber] });
        Some(WindowFocus::key(&app, number))
    }

    /// 语音键（修饰键）：flagsChanged 里看按下 / 松开；是语音键就吃了，别当成普通 Shift 处理。
    fn voice_modifier_changed(&self, event: &NSEvent) -> bool {
        let Some(key) = self.ivars().voice_key.get() else {
            return false;
        };
        if !key.is_modifier() || event.keyCode() != key.key_code {
            return false;
        }
        let flags = event.modifierFlags().0 as u64;
        let down = key.is_down(flags);
        self.with_voice(|voice| {
            if down {
                voice.key_down();
            } else {
                voice.key_up();
            }
        });
        true
    }

    /// 语音键（F 键）：keyDown 时开始听。
    fn voice_key_down(&self, event: &NSEvent) -> bool {
        let Some(key) = self.ivars().voice_key.get() else {
            return false;
        };
        if key.is_modifier() || event.keyCode() != key.key_code {
            return false;
        }
        self.with_voice(|voice| voice.key_down());
        true
    }

    /// 语音键（F 键）：keyUp 时说完。
    fn voice_key_up(&self, event: &NSEvent) -> bool {
        let Some(key) = self.ivars().voice_key.get() else {
            return false;
        };
        if key.is_modifier() || event.keyCode() != key.key_code {
            return false;
        }
        self.with_voice(|voice| voice.key_up());
        true
    }

    fn with_voice(&self, action: impl FnOnce(&Voice)) {
        self.ensure_voice();
        if let Some(voice) = self.ivars().voice.borrow().as_ref() {
            action(voice);
        }
        self.ensure_voice_timer();
    }

    /// 第一次用到语音时建句柄（模型目录在数据目录下的 wtlocal）。
    fn ensure_voice(&self) {
        let shared = app::start();
        let config = shared.config();
        let key = VoiceKey::parse(&config.voice_key).filter(|_| config.voice);
        self.ivars().voice_key.set(key);
        if key.is_none() {
            return;
        }
        if self.ivars().voice.borrow().is_none() {
            let dir = voice::model_dir(&shared.layout.support_dir);
            *self.ivars().voice.borrow_mut() = Voice::new(&dir);
        }
        // macOS 没有单独的录音设备设置项（spec 的 MicDevice 只给 Linux），用系统默认输入
        if let Some(voice) = self.ivars().voice.borrow().as_ref() {
            voice.set_device("");
        }
    }

    /// 语音状态轮询的定时器（50 毫秒一次，主线程 run loop）。
    fn ensure_voice_timer(&self) {
        let mut timer = self.ivars().voice_timer.borrow_mut();
        if timer.is_some() {
            return;
        }
        let Some(_mtm) = MainThreadMarker::new() else {
            return;
        };
        let created = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                0.05,
                self.as_any(),
                sel!(pollVoice:),
                None,
                true,
            )
        };
        *timer = Some(created);
    }

    /// 当前输入框（定时器回调里用；会话调用时用的是同一个）。
    fn current_client(&self) -> Option<Retained<AnyObject>> {
        let client: Option<Retained<AnyObject>> = unsafe { msg_send![self, client] };
        client
    }

    fn as_any(&self) -> &AnyObject {
        // SAFETY: 所有 NSObject 子类都布局成 AnyObject；这里只用来当菜单项的 target。
        unsafe { &*(self as *const Self as *const AnyObject) }
    }

    /// 引擎就绪了就把会话建好（第一次按键、激活时可能还没载入完）。
    fn prepare(&self, client: &AnyObject) -> bool {
        let shared = app::start();
        let Some(engine) = shared.engine() else {
            return false;
        };
        let mut input = self.ivars().input.borrow_mut();
        if input.is_none() {
            *input = Some(Input::attach(
                engine,
                Some(shared.translator()),
                shared.layout.clone(),
                shared.config(),
            ));
        }
        let mut config_changed = false;
        if let Some(config) = shared.reload_config_if_changed() {
            if let Some(input) = input.as_mut() {
                let available = archtype_shell::translate::translate_available(&config, &shared.layout);
                let can_edit = self.can_edit_around_caret();
                let problems = input.apply_config(&config, available, can_edit);
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
        if !event.isARepeat() {
            // 自动重复不算；密码框（安全输入）里的也不记
            app::start().stats().record_key(key_code, secure_input());
        }
        let characters = event.characters().map(|s| s.to_string());
        let ignoring = event.charactersIgnoringModifiers().map(|s| s.to_string());
        let flags = event.modifierFlags().0 as u64;
        let Some(translated) = archtype_shell::keys::translate_key_down(
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
                    return Bool::YES;
                }
            }
            return Bool::NO;
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
                return Bool::NO;
            };
            let frame = input.key(&key);
            if let Some(emoji) = input.last_choice_emoji() {
                archtype_core::recent_emoji::remember(&emoji);
            }
            if let Some(text) = &frame.commit {
                app::start().stats().record_commit(
                    text,
                    input.commit_source(),
                    &input.commit_scheme(),
                    input.last_choice_index(),
                    secure_input(),
                );
            }
            frame
        };

        self.apply_frame(&frame, client);
        Bool::from(frame.handled)
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
            insert_text(client, text);
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
        // 工具栏打开时也看一眼剪贴板（和切换输入框时一样，只在内存里）
        if matches!(frame.panel, archtype_core::session::Panel::Tool) {
            self.sample_clipboard();
        }
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
            let config = input.config().clone();
            let shown = input.shown();
            archtype_shell::view::page_from_frame(frame, shown, &config)
        };
        if page.is_empty() {
            self.hide_panel();
            return;
        }

        let mut panel = self.ivars().panel.borrow_mut();
        if panel.is_none() {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let created = CandidatePanel::new(mtm);
            created.set_target(self.as_any());
            *panel = Some(created);
        }
        if let Some(panel) = panel.as_ref() {
            panel.show(&page, caret_rect(client));
        }
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

/// 外部命令回给命令行的模式名（和 Linux 的 SetMode 一样）。
fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Chinese => "zh",
        Mode::English => "en",
        Mode::Translate => "translate",
    }
}

/// 往输入框里上屏一段文字。
fn insert_text(client: &AnyObject, text: &str) {
    let ns = NSString::from_str(text);
    let range = NSRange::new(NSNotFound as usize, 0);
    unsafe {
        let _: () = msg_send![client, insertText: &*ns, replacementRange: range];
    }
}

/// 系统现在是不是安全输入（密码框、终端的安全键盘输入）：统计和学习都不记。
fn secure_input() -> bool {
    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn IsSecureEventInputEnabled() -> u8;
    }
    unsafe { IsSecureEventInputEnabled() != 0 }
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
/// 另起设置窗口（输入法进程里开窗口会和输入框抢焦点）。`page` 是 NSH 的页名（general/words/voice）。
fn open_settings_app(page: Option<&str>) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = std::process::Command::new(exe);
    command.arg("--settings");
    if let Some(page) = page {
        command.arg(page);
    }
    let _ = command.spawn();
}

fn show_message(title: &str, text: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(text));
    alert.runModal();
}

fn show_about_panel() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("ArchType 输入法"));
    let text = format!("版本 {}（{}）", app::version(), app::bundle_id());
    alert.setInformativeText(&NSString::from_str(&text));
    alert.runModal();
}

/// 给 `mod.rs` 用：当前进程的输入控制器类名要和 Info.plist 一致。
#[allow(dead_code)]
pub fn controller_class_name() -> &'static str {
    "ArchTypeController"
}
