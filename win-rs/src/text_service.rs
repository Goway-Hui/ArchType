//! TSF 文本服务：激活/停用、键事件 → 会话 → 上屏。
//!
//! M1 只做骨架：激活后把键交给 `archtype_shell::input::Input`，有整词固定的结果
//! （`Frame.commit`）就插入；编辑区（composition / preedit）留给 M2。没有 preedit 时
//! 会话「还没上屏的拼音」不会显示，`handled` 的键要放给应用，避免吞键。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{LazyLock, Once};

use archtype_core::session::{key_hints, Frame, Key, Mode};
use archtype_shell::input::Input;
use archtype_shell::keys::win::VoiceKey;
use archtype_shell::panel::Page;
use archtype_shell::view::page_from_frame;
use archtype_shell::voice::{
    Voice, VOICE_ERROR, VOICE_LISTENING, VOICE_RECOGNIZING, VOICE_RESULT,
};
use archtype_shell::keys::{translate_key_down, Modifiers};
use archtype_shell::runtime::Runtime;
use windows::core::{implement, Interface, Result, PCWSTR};
use windows::Win32::Foundation::{BOOL, FALSE, HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, KillTimer, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardState, GetKeyState, ToUnicodeEx,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::TextServices::{
    InputScope, ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext,
    ITfContextComposition, ITfEditSession, ITfEditSession_Impl, ITfInputScope,
    ITfInsertAtSelection, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfRange, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl,
    ITfThreadMgr, GUID_PROP_INPUTSCOPE, IS_PASSWORD, TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_ES_READ,
    TF_ES_READWRITE, TF_ES_SYNC, TF_IAS_NOQUERY, TF_SELECTION,
};

/// 进程级：配置、引擎、翻译器、统计（DLL 在每个宿主进程里各一份，和 fcitx5 插件一样）。
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new(crate::paths::layout()));

pub fn runtime() -> &'static Runtime {
    static STARTED: Once = Once::new();
    let runtime = &RUNTIME;
    STARTED.call_once(|| runtime.start_loading());
    runtime
}

thread_local! {
    /// 定时器回调要拿到当前输入法对象（同一个 TSF 线程）
    static VOICE_STATE: RefCell<Option<Rc<State>>> = RefCell::new(None);
}

pub struct State {
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    keystroke: RefCell<Option<ITfKeystrokeMgr>>,
    tid: Cell<u32>,
    sink: RefCell<Option<ITfKeyEventSink>>,
    input: RefCell<Option<Input>>,
    /// 正在组合的（编辑区）文本，按输入框（ITfContext 指针）分开
    compositions: RefCell<HashMap<isize, ITfComposition>>,
    /// 组合的终止通知（用户点别处时 TSF 会结束组合）
    composition_sinks: RefCell<HashMap<isize, ITfCompositionSink>>,
    /// 语音输入（第一次按语音键时建）
    voice: RefCell<Option<Voice>>,
    /// 最近一次按键时的输入框（语音结果要上屏到这里）
    last_context: RefCell<Option<ITfContext>>,
    /// 最近一次拿到的光标矩形（语音提示用）
    last_caret: Cell<RECT>,
    /// 候选窗口现在显示的是不是语音提示
    hint_shown: Cell<bool>,
    /// 全局语音热键钩子（WH_KEYBOARD_LL）；0 表示没装
    hook: Cell<isize>,
    /// 当前输入框是不是密码框（探测结果，下一次按键用）
    password: Cell<bool>,
}

impl State {
    fn new() -> Self {
        Self {
            thread_mgr: RefCell::new(None),
            keystroke: RefCell::new(None),
            tid: Cell::new(0),
            sink: RefCell::new(None),
            input: RefCell::new(None),
            compositions: RefCell::new(HashMap::new()),
            composition_sinks: RefCell::new(HashMap::new()),
            voice: RefCell::new(None),
            last_context: RefCell::new(None),
            last_caret: Cell::new(RECT::default()),
            hint_shown: Cell::new(false),
            hook: Cell::new(0),
            password: Cell::new(false),
        }
    }
}

/// 文本服务本体：只管激活/停用；键事件由单独的 [`KeySink`] 接（便于 Advise/Unadvise）。
#[implement(ITfTextInputProcessorEx)]
pub struct TextService {
    state: Rc<State>,
}

impl TextService {
    pub fn new() -> Self {
        Self { state: Rc::new(State::new()) }
    }

    fn activate(&self, thread_mgr: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        let Some(thread_mgr) = thread_mgr else {
            return Ok(());
        };
        *self.state.thread_mgr.borrow_mut() = Some(thread_mgr.clone());
        self.state.tid.set(tid);

        let keystroke: ITfKeystrokeMgr = thread_mgr.cast()?;
        let sink: ITfKeyEventSink = KeySink {
            state: Rc::clone(&self.state),
        }
        .into();
        unsafe {
            keystroke.AdviseKeyEventSink(tid, &sink, TRUE)?;
        }
        *self.state.keystroke.borrow_mut() = Some(keystroke);
        *self.state.sink.borrow_mut() = Some(sink);
        // 50ms 轮询语音状态（听音量、等识别结果）
        unsafe {
            SetTimer(None, 0, 50, Some(voice_timer));
        }
        // 全局语音热键：输入法在后台时也能按（低级键盘钩子，只在自己的消息线程里回调）
        if self.state.hook.get() == 0 {
            if let Ok(module) = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR::null()) } {
                if let Ok(hook) = unsafe {
                    SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(keyboard_hook),
                        windows::Win32::Foundation::HINSTANCE(module.0),
                        0,
                    )
                } {
                    self.state.hook.set(hook.0 as isize);
                }
            }
        }
        VOICE_STATE.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&self.state)));
        Ok(())
    }

    fn deactivate(&self) -> Result<()> {
        let tid = self.state.tid.get();
        if let Some(keystroke) = self.state.keystroke.borrow_mut().take() {
            unsafe {
                let _ = keystroke.UnadviseKeyEventSink(tid);
            }
        }
        *self.state.sink.borrow_mut() = None;
        *self.state.thread_mgr.borrow_mut() = None;
        unsafe {
            let _ = KillTimer(None, 0);
        }
        let hook = self.state.hook.replace(0);
        if hook != 0 {
            unsafe {
                let _ = UnhookWindowsHookEx(HHOOK(hook as *mut std::ffi::c_void));
            }
        }
        // 切走应用时把学到的词和统计异步落盘（不阻塞宿主，别用 save_wait）
        let runtime = runtime();
        if let Some(engine) = runtime.engine() {
            let engine = engine.get();
            let layout = &runtime.layout;
            let _ = engine.save_user_dict_async(layout.user_dict_path());
            let _ = engine.save_history_async(layout.history_path());
        }
        runtime.stats().flush();
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        self.this.activate(ptim, tid)
    }

    fn Deactivate(&self) -> Result<()> {
        self.this.deactivate()
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _flags: u32) -> Result<()> {
        self.this.activate(ptim, tid)
    }
}

/// 键事件接收器：和文本服务共享同一份 [`State`]。
#[implement(ITfKeyEventSink)]
struct KeySink {
    state: Rc<State>,
}

impl KeySink {
    fn handle(&self, pic: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let Some(pic) = pic else {
            return Ok(FALSE);
        };
        if runtime().engine().is_none() {
            return Ok(FALSE);
        }
        // 设置窗口改了 archtype.conf：输入法这边跟着重读（不用重启宿主）
        if let Some(config) = runtime().reload_config_if_changed() {
            let available =
                archtype_shell::translate::translate_available(&config, &runtime().layout);
            if let Some(input) = self.state.input.borrow_mut().as_mut() {
                for problem in input.apply_config(&config, available, false) {
                    eprintln!("ArchType: {problem}");
                }
            }
            crate::candidate_window::hide();
        }
        let vk = (wparam.0 & 0xffff) as u16;
        // 密码框：探测一次（同步 edit session），这次按键不学习、不进统计
        let sensitive = probe_password(pic, &self.state);
        // 自动重复不算按键统计
        if lparam.0 & (1 << 30) == 0 {
            runtime().stats().record_key(vk, sensitive);
        }
        // 语音键：按下即开始录，和 macOS / fcitx5 插件一致（只在输入法激活时）
        let config = runtime().config();
        if config.voice {
            if let Some(voice_key) = VoiceKey::parse(&config.voice_key) {
                if vk == voice_key.vk {
                    ensure_voice(&self.state);
                    if let Some(voice) = self.state.voice.borrow().as_ref() {
                        voice.key_down();
                    }
                    self.state.last_context.borrow_mut().replace(pic.clone());
                    VOICE_STATE.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&self.state)));
                    return Ok(TRUE);
                }
            }
        }
        ensure_input(&self.state);
        let characters = key_characters(vk, lparam);
        let Some(translated) = translate_key_down(vk, characters.as_deref(), current_modifiers())
        else {
            return Ok(FALSE);
        };
        let (frame, page) = {
            let mut input = self.state.input.borrow_mut();
            let Some(input) = input.as_mut() else {
                return Ok(FALSE);
            };
            let mut key = Key::default();
            key.keysym = translated.keysym;
            key.modifiers = translated.modifiers.0;
            if sensitive {
                key.hints = key_hints::NO_LEARN | key_hints::NO_TRANSLATE;
            }
            let frame = input.key(&key);
            if let Some(text) = &frame.commit {
                runtime().stats().record_commit(
                    text,
                    input.commit_source(),
                    &input.commit_scheme(),
                    input.last_choice_index(),
                    sensitive,
                );
            }
            let config = input.config().clone();
            let page = page_from_frame(&frame, input.shown(), &config);
            (frame, page)
        };
        let page = (!page.is_empty()).then_some(page);
        let context_key = Interface::as_raw(pic) as isize;
        apply_frame(
            pic,
            self.state.tid.get(),
            Rc::clone(&self.state),
            context_key,
            &frame,
            page,
        );
        Ok(BOOL::from(frame.handled))
    }
}

impl ITfKeyEventSink_Impl for KeySink_Impl {
    fn OnSetFocus(&self, _foreground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(BOOL::from(
            pic.is_some() && runtime().engine().is_some(),
        ))
    }

    fn OnTestKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnKeyDown(&self, pic: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        self.this.handle(pic, wparam, lparam)
    }

    fn OnKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let config = runtime().config();
        if config.voice {
            if let Some(voice_key) = VoiceKey::parse(&config.voice_key) {
                if (wparam.0 & 0xffff) as u16 == voice_key.vk {
                    if let Some(voice) = self.state.voice.borrow().as_ref() {
                        voice.key_up();
                    }
                    return Ok(TRUE);
                }
            }
        }
        Ok(FALSE)
    }

    fn OnPreservedKey(
        &self,
        _pic: Option<&ITfContext>,
        _rguid: *const windows::core::GUID,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }
}

fn ensure_input(state: &State) {
    if state.input.borrow().is_some() {
        return;
    }
    let runtime = runtime();
    let Some(engine) = runtime.engine() else {
        return;
    };
    let input = Input::attach(
        engine,
        Some(runtime.translator()),
        runtime.layout.clone(),
        runtime.config(),
    );
    *state.input.borrow_mut() = Some(input);
}

enum EditKind {
    /// 编辑区（组合）显示还没选的字
    Preedit(Vec<u16>),
    /// 上屏：替换组合，或直接插到光标处
    Commit(Vec<u16>),
    /// 丢掉组合（取消、切走）
    Cancel,
}

/// 把一帧排到 TSF 的 edit session 里执行（TSF 只允许在 edit session 里改文本）。
fn apply_frame(
    context: &ITfContext,
    tid: u32,
    state: Rc<State>,
    key: isize,
    frame: &Frame,
    page: Option<Page>,
) {
    let kind = if let Some(commit) = &frame.commit {
        if commit.is_empty() {
            return;
        }
        EditKind::Commit(commit.encode_utf16().collect())
    } else if !frame.preedit.is_empty() {
        EditKind::Preedit(frame.preedit.encode_utf16().collect())
    } else {
        EditKind::Cancel
    };
    let session: ITfEditSession = FrameEditSession {
        context: context.clone(),
        state,
        key,
        kind,
        page,
        tid,
    }
    .into();
    let flags = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READWRITE.0);
    unsafe {
        let _ = context.RequestEditSession(tid, &session, flags);
    }
}

#[implement(ITfEditSession)]
struct FrameEditSession {
    context: ITfContext,
    state: Rc<State>,
    key: isize,
    kind: EditKind,
    page: Option<Page>,
    tid: u32,
}

impl ITfEditSession_Impl for FrameEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let this = &self.this;
        match &this.kind {
            EditKind::Preedit(text) => {
                let existing = this.state.compositions.borrow().get(&this.key).cloned();
                let composition = match existing {
                    Some(composition) => composition,
                    None => {
                        let composition = start_composition(&this.context, &this.state, this.key, ec)?;
                        this.state
                            .compositions
                            .borrow_mut()
                            .insert(this.key, composition.clone());
                        composition
                    }
                };
                let range = unsafe { composition.GetRange() }?;
                unsafe {
                    range.SetText(ec, 0, text)?;
                }
            }
            EditKind::Commit(text) => {
                let composition = this.state.compositions.borrow_mut().remove(&this.key);
                this.state.composition_sinks.borrow_mut().remove(&this.key);
                match composition {
                    Some(composition) => {
                        let range = unsafe { composition.GetRange() }?;
                        unsafe {
                            range.SetText(ec, 0, text)?;
                            composition.EndComposition(ec)?;
                        }
                    }
                    None => unsafe {
                        let insert: ITfInsertAtSelection = this.context.cast()?;
                        insert.InsertTextAtSelection(ec, TF_IAS_NOQUERY, text)?;
                    },
                }
            }
            EditKind::Cancel => {
                let composition = this.state.compositions.borrow_mut().remove(&this.key);
                this.state.composition_sinks.borrow_mut().remove(&this.key);
                if let Some(composition) = composition {
                    let range = unsafe { composition.GetRange() }?;
                    unsafe {
                        range.SetText(ec, 0, &[])?;
                        composition.EndComposition(ec)?;
                    }
                }
            }
        }
        match &this.page {
            Some(page) => {
                if let Ok(caret) = caret_rect(&this.context, ec) {
                    this.state.last_caret.set(caret);
                    let state = Rc::clone(&this.state);
                    let context = this.context.clone();
                    let tid = this.tid;
                    let key = this.key;
                    let on_click: Box<dyn Fn(usize)> = Box::new(move |index| {
                        click_candidate(&state, &context, tid, key, index);
                    });
                    crate::candidate_window::show(page, caret, on_click);
                }
            }
            None => crate::candidate_window::hide(),
        }
        Ok(())
    }
}

/// 点候选：选词后重新应用一帧（上屏或更新候选）。
fn click_candidate(state: &Rc<State>, context: &ITfContext, tid: u32, key: isize, index: usize) {
    let (frame, page) = {
        let mut input = state.input.borrow_mut();
        let Some(input) = input.as_mut() else {
            return;
        };
        let frame = input.select(index);
        let config = input.config().clone();
        let page = page_from_frame(&frame, input.shown(), &config);
        (frame, page)
    };
    let page = (!page.is_empty()).then_some(page);
    apply_frame(context, tid, Rc::clone(state), key, &frame, page);
}

/// 光标在屏幕上的矩形（物理像素），候选窗口按它定位。
fn caret_rect(context: &ITfContext, ec: u32) -> Result<windows::Win32::Foundation::RECT> {
    unsafe {
        let mut selection: [TF_SELECTION; 1] = std::mem::zeroed();
        let mut fetched = 0u32;
        context.GetSelection(ec, 0, &mut selection, &mut fetched)?;
        let range = selection[0]
            .range
            .as_ref()
            .ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
        let view = context.GetActiveView()?;
        let mut rect = windows::Win32::Foundation::RECT::default();
        let mut clipped = windows::Win32::Foundation::BOOL::default();
        view.GetTextExt(ec, range, &mut rect, &mut clipped)?;
        Ok(rect)
    }
}

/// 在光标处开始一个空组合（编辑区）。
fn start_composition(
    context: &ITfContext,
    state: &Rc<State>,
    key: isize,
    ec: u32,
) -> Result<ITfComposition> {
    unsafe {
        // 当前选区的 range 就是光标处（空选区时是插入点）
        let mut selection: [TF_SELECTION; 1] = std::mem::zeroed();
        let mut fetched = 0u32;
        context.GetSelection(ec, 0, &mut selection, &mut fetched)?;
        let range = selection[0]
            .range
            .as_ref()
            .ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;

        let sink: ITfCompositionSink = CompositionSink {
            state: Rc::clone(state),
            key,
        }
        .into();
        state
            .composition_sinks
            .borrow_mut()
            .insert(key, sink.clone());
        let context_composition: ITfContextComposition = context.cast()?;
        context_composition.StartComposition(ec, range, &sink)
    }
}

/// 密码框探测：在同步 edit session 里读 `GUID_PROP_INPUTSCOPE`，看有没有 `IS_PASSWORD`。
#[implement(ITfEditSession)]
struct ProbePassword {
    context: ITfContext,
    state: Rc<State>,
}

impl ITfEditSession_Impl for ProbePassword_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let this = &self.this;
        unsafe {
            let mut selection: [TF_SELECTION; 1] = std::mem::zeroed();
            let mut fetched = 0u32;
            this.context.GetSelection(ec, 0, &mut selection, &mut fetched)?;
            if let Some(range) = selection[0].range.as_ref() {
                this.state.password.set(input_scope_password(&this.context, ec, range));
            }
        }
        Ok(())
    }
}

fn probe_password(context: &ITfContext, state: &Rc<State>) -> bool {
    state.password.set(false);
    let session: ITfEditSession = ProbePassword {
        context: context.clone(),
        state: Rc::clone(state),
    }
    .into();
    let flags = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READ.0);
    unsafe {
        let _ = context.RequestEditSession(state.tid.get(), &session, flags);
    }
    state.password.get()
}

/// `GUID_PROP_INPUTSCOPE` 里有没有 `IS_PASSWORD`。
unsafe fn input_scope_password(context: &ITfContext, ec: u32, range: &ITfRange) -> bool {
    let Ok(property) = context.GetProperty(&GUID_PROP_INPUTSCOPE) else {
        return false;
    };
    let Ok(value) = property.GetValue(ec, range) else {
        return false;
    };
    // VARIANT 是 windows-core 的不透明类型：用 TryFrom 拿 IUnknown，再 QI 到 ITfInputScope
    let Ok(unknown) = windows::core::IUnknown::try_from(&value) else {
        return false;
    };
    let Ok(scope) = unknown.cast::<ITfInputScope>() else {
        return false;
    };
    let mut scopes: *mut InputScope = std::ptr::null_mut();
    let mut count = 0u32;
    if scope.GetInputScopes(&mut scopes, &mut count).is_err() {
        return false;
    }
    let mut password = false;
    if !scopes.is_null() {
        for index in 0..count as usize {
            if *scopes.add(index) == IS_PASSWORD {
                password = true;
                break;
            }
        }
        CoTaskMemFree(Some(scopes as *const std::ffi::c_void));
    }
    password
}

fn mode_request_path() -> std::path::PathBuf {
    crate::paths::support_dir().join("mode-request")
}

/// 低级键盘钩子：匹配语音键时按/松交给语音句柄，并吃掉这个键。
unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let down = wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
        let up = wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize;
        if down || up {
            let config = runtime().config();
            if config.voice {
                if let Some(key) = VoiceKey::parse(&config.voice_key) {
                    if info.vkCode as u16 == key.vk {
                        VOICE_STATE.with(|slot| {
                            if let Some(state) = slot.borrow().as_ref() {
                                if let Some(voice) = state.voice.borrow().as_ref() {
                                    if down {
                                        voice.key_down();
                                    } else {
                                        voice.key_up();
                                    }
                                }
                            }
                        });
                        return LRESULT(1);
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// 第一次按语音键时建句柄（模型在 `%APPDATA%\ArchType\wtlocal`）。
fn ensure_voice(state: &State) {
    if state.voice.borrow().is_some() {
        return;
    }
    let dir = crate::paths::support_dir().join("wtlocal");
    if let Some(voice) = Voice::new(&dir) {
        voice.set_device("");
        *state.voice.borrow_mut() = Some(voice);
    }
}

/// 50ms 定时器：推进语音状态机，显示提示 / 上屏结果。
unsafe extern "system" fn voice_timer(_hwnd: HWND, _msg: u32, _id: usize, _time: u32) {
    let Some(state) = VOICE_STATE.with(|slot| slot.borrow().clone()) else {
        return;
    };
    // 命令行 `archtype --mode zh|en|translate|toggle`：读一次请求文件就删
    if let Ok(text) = std::fs::read_to_string(mode_request_path()) {
        let _ = std::fs::remove_file(mode_request_path());
        let current = state.input.borrow().as_ref().map(|input| input.mode());
        let target = match text.trim() {
            "zh" => Some(Mode::Chinese),
            "en" => Some(Mode::English),
            "translate" => Some(Mode::Translate),
            "toggle" => Some(match current {
                Some(Mode::Chinese) => Mode::English,
                Some(Mode::English) => Mode::Chinese,
                _ => Mode::Chinese,
            }),
            _ => None,
        };
        if let Some(target) = target {
            if let Some(context) = state.last_context.borrow().clone() {
                let (frame, page) = {
                    let mut input = state.input.borrow_mut();
                    match input.as_mut() {
                        Some(input) => {
                            let frame = input.set_mode(target);
                            let config = input.config().clone();
                            let page = page_from_frame(&frame, input.shown(), &config);
                            (frame, page)
                        }
                        None => return,
                    }
                };
                let page = (!page.is_empty()).then_some(page);
                let key = Interface::as_raw(&context) as isize;
                apply_frame(&context, state.tid.get(), Rc::clone(&state), key, &frame, page);
            }
        }
    }
    let phase = {
        let voice = state.voice.borrow();
        match voice.as_ref() {
            Some(voice) => voice.poll(),
            None => return,
        }
    };
    match phase {
        VOICE_LISTENING => {
            let level = state
                .voice
                .borrow()
                .as_ref()
                .map(|voice| voice.level())
                .unwrap_or(0);
            let wave = archtype_shell::keys::voice_wave(&[level]);
            show_hint(&state, &format!("正在听… {wave}"));
        }
        VOICE_RECOGNIZING => show_hint(&state, "识别中…"),
        VOICE_RESULT => {
            let text = state
                .voice
                .borrow()
                .as_ref()
                .map(|voice| voice.text())
                .unwrap_or_default();
            if let Some(voice) = state.voice.borrow().as_ref() {
                voice.cancel();
            }
            hide_hint(&state);
            if !text.is_empty() {
                commit_voice(&state, &text);
            }
        }
        VOICE_ERROR => {
            let text = state
                .voice
                .borrow()
                .as_ref()
                .map(|voice| voice.text())
                .unwrap_or_default();
            if let Some(voice) = state.voice.borrow().as_ref() {
                voice.cancel();
            }
            show_hint(&state, &text);
        }
        _ => {
            if state.hint_shown.get() {
                hide_hint(&state);
            }
        }
    }
}

fn show_hint(state: &State, text: &str) {
    let page = archtype_shell::view::voice_hint_page(text, runtime().config().candidate_font_size);
    crate::candidate_window::show(&page, state.last_caret.get(), Box::new(|_| {}));
    state.hint_shown.set(true);
}

fn hide_hint(state: &State) {
    crate::candidate_window::hide();
    state.hint_shown.set(false);
}

fn commit_voice(state: &Rc<State>, text: &str) {
    let context = state.last_context.borrow().clone();
    let Some(context) = context else {
        return;
    };
    let key = Interface::as_raw(&context) as isize;
    let frame = Frame {
        commit: Some(text.to_string()),
        handled: true,
        ..Default::default()
    };
    apply_frame(&context, state.tid.get(), Rc::clone(state), key, &frame, None);
}

/// 组合被 TSF 结束（用户点别处、切走）：把记着的状态清掉。
#[implement(ITfCompositionSink)]
struct CompositionSink {
    state: Rc<State>,
    key: isize,
}

impl ITfCompositionSink_Impl for CompositionSink_Impl {
    fn OnCompositionTerminated(
        &self,
        _ec: u32,
        _composition: Option<&ITfComposition>,
    ) -> Result<()> {
        let this = &self.this;
        this.state.compositions.borrow_mut().remove(&this.key);
        this.state.composition_sinks.borrow_mut().remove(&this.key);
        Ok(())
    }
}

/// 当前修饰键（左右 Alt/Ctrl 都算同一个位；语音键的左右区分留给 M4 的钩子）。
fn current_modifiers() -> Modifiers {
    let down = |vk: i32| unsafe { GetKeyState(vk) < 0 };
    let mut modifiers = Modifiers::NONE;
    if down(0x10) {
        modifiers.0 |= Modifiers::SHIFT.0;
    }
    if unsafe { GetKeyState(0x14) & 1 != 0 } {
        modifiers.0 |= Modifiers::CAPS.0;
    }
    if down(0x11) {
        modifiers.0 |= Modifiers::CTRL.0;
    }
    if down(0x12) {
        modifiers.0 |= Modifiers::ALT.0;
    }
    if down(0x5b) || down(0x5c) {
        modifiers.0 |= Modifiers::SUPER.0;
    }
    modifiers
}

/// `ToUnicodeEx` 取这次按键打出的字符（考虑 Shift 和键盘布局）。
fn key_characters(vk: u16, lparam: LPARAM) -> Option<String> {
    let mut state = [0u8; 256];
    let mut buf = [0u16; 8];
    unsafe {
        if GetKeyboardState(&mut state).is_err() {
            return None;
        }
        let layout = GetKeyboardLayout(0);
        let scan = ((lparam.0 >> 16) & 0xff) as u32;
        let count = ToUnicodeEx(vk as u32, scan, &state, &mut buf, 0, layout);
        if count <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..count as usize]))
    }
}

/// COM 类工厂：TSF 通过它创建文本服务。
#[implement(windows::Win32::System::Com::IClassFactory)]
pub struct Factory;

impl windows::Win32::System::Com::IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        _outer: Option<&windows::core::IUnknown>,
        riid: *const windows::core::GUID,
        ppv: *mut *mut std::ffi::c_void,
    ) -> Result<()> {
        let service: ITfTextInputProcessorEx = TextService::new().into();
        unsafe { service.query(riid, ppv).ok() }
    }

    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}
