//! TSF 文本服务：激活/停用、键事件 → 会话 → 上屏。
//!
//! M1 只做骨架：激活后把键交给 `archtype_shell::input::Input`，有整词固定的结果
//! （`Frame.commit`）就插入；编辑区（composition / preedit）留给 M2。没有 preedit 时
//! 会话「还没上屏的拼音」不会显示，`handled` 的键要放给应用，避免吞键。

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{LazyLock, Once};

use archtype_core::session::Key;
use archtype_shell::input::Input;
use archtype_shell::keys::{translate_key_down, Modifiers};
use archtype_shell::runtime::Runtime;
use windows::core::{implement, Interface, Result};
use windows::Win32::Foundation::{BOOL, FALSE, LPARAM, TRUE, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardState, GetKeyState, ToUnicodeEx,
};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection, ITfKeyEventSink,
    ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfTextInputProcessor, ITfTextInputProcessorEx,
    ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl, ITfThreadMgr,
    TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_ES_READWRITE, TF_ES_SYNC, TF_IAS_NOQUERY, TS_TEXTCHANGE,
};

/// 进程级：配置、引擎、翻译器、统计（DLL 在每个宿主进程里各一份，和 fcitx5 插件一样）。
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new(crate::paths::layout()));

pub fn runtime() -> &'static Runtime {
    static STARTED: Once = Once::new();
    let runtime = &RUNTIME;
    STARTED.call_once(|| runtime.start_loading());
    runtime
}

pub struct State {
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    keystroke: RefCell<Option<ITfKeystrokeMgr>>,
    tid: Cell<u32>,
    sink: RefCell<Option<ITfKeyEventSink>>,
    input: RefCell<Option<Input>>,
}

impl State {
    fn new() -> Self {
        Self {
            thread_mgr: RefCell::new(None),
            keystroke: RefCell::new(None),
            tid: Cell::new(0),
            sink: RefCell::new(None),
            input: RefCell::new(None),
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
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for TextService {
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        self.activate(ptim, tid)
    }

    fn Deactivate(&self) -> Result<()> {
        self.deactivate()
    }
}

impl ITfTextInputProcessorEx_Impl for TextService {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _flags: u32) -> Result<()> {
        self.activate(ptim, tid)
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
        ensure_input(&self.state);
        let vk = (wparam.0 & 0xffff) as u16;
        let characters = key_characters(vk, lparam);
        let Some(translated) = translate_key_down(vk, characters.as_deref(), current_modifiers())
        else {
            return Ok(FALSE);
        };
        let frame = {
            let mut input = self.state.input.borrow_mut();
            let Some(input) = input.as_mut() else {
                return Ok(FALSE);
            };
            let mut key = Key::default();
            key.keysym = translated.keysym;
            key.modifiers = translated.modifiers.0;
            input.key(&key)
        };
        if let Some(text) = &frame.commit {
            if !text.is_empty() {
                let _ = insert_text(pic, self.state.tid.get(), text);
            }
        }
        // M1 没有编辑区：有还没上屏的拼音时也放给应用（M2 有了 composition 再吃掉）
        Ok(BOOL::from(frame.handled && frame.commit.is_some()))
    }
}

impl ITfKeyEventSink_Impl for KeySink {
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
        self.handle(pic, wparam, lparam)
    }

    fn OnKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
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

/// 在 edit session 里把文字插到光标处。
fn insert_text(context: &ITfContext, tid: u32, text: &str) -> Result<()> {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let session: ITfEditSession = InsertText {
        context: context.clone(),
        text: utf16,
    }
    .into();
    let flags = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READWRITE.0);
    let hr = unsafe { context.RequestEditSession(tid, &session, flags)? };
    hr.ok()
}

#[implement(ITfEditSession)]
struct InsertText {
    context: ITfContext,
    text: Vec<u16>,
}

impl ITfEditSession_Impl for InsertText {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let mut start = 0i32;
            let mut end = 0i32;
            let mut change = TS_TEXTCHANGE::default();
            insert.InsertTextAtSelection(
                TF_IAS_NOQUERY.0,
                &self.text,
                &mut start,
                &mut end,
                &mut change,
            )?;
        }
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
        if !GetKeyboardState(&mut state).as_bool() {
            return None;
        }
        let layout = GetKeyboardLayout(0);
        let scan = ((lparam.0 >> 16) & 0xff) as u32;
        let count = ToUnicodeEx(vk as u32, scan, state.as_ptr(), &mut buf, 0, layout);
        if count <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..count as usize]))
    }
}

/// COM 类工厂：TSF 通过它创建文本服务。
#[implement(windows::Win32::System::Com::IClassFactory)]
pub struct Factory;

impl windows::Win32::System::Com::IClassFactory_Impl for Factory {
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
