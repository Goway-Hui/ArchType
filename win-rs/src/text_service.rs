//! TSF 文本服务：激活/停用、键事件 → 会话 → 上屏。
//!
//! M1 只做骨架：激活后把键交给 `archtype_shell::input::Input`，有整词固定的结果
//! （`Frame.commit`）就插入；编辑区（composition / preedit）留给 M2。没有 preedit 时
//! 会话「还没上屏的拼音」不会显示，`handled` 的键要放给应用，避免吞键。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{LazyLock, Once};

use archtype_core::session::{Frame, Key};
use archtype_shell::input::Input;
use archtype_shell::keys::{translate_key_down, Modifiers};
use archtype_shell::runtime::Runtime;
use windows::core::{implement, Interface, Result};
use windows::Win32::Foundation::{BOOL, FALSE, LPARAM, TRUE, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardState, GetKeyState, ToUnicodeEx,
};
use windows::Win32::UI::TextServices::{
    ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext,
    ITfContextComposition, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection,
    ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfRange, ITfRangeACP,
    ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl,
    ITfThreadMgr, TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_ES_READWRITE, TF_ES_SYNC, TF_IAS_NOQUERY,
    TS_SELECTION_ACP,
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
    /// 正在组合的（编辑区）文本，按输入框（ITfContext 指针）分开
    compositions: RefCell<HashMap<isize, ITfComposition>>,
    /// 组合的终止通知（用户点别处时 TSF 会结束组合）
    composition_sinks: RefCell<HashMap<isize, ITfCompositionSink>>,
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
        let context_key = unsafe { Interface::as_raw(pic) as isize };
        apply_frame(
            pic,
            self.state.tid.get(),
            Rc::clone(&self.state),
            context_key,
            &frame,
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

enum EditKind {
    /// 编辑区（组合）显示还没选的字
    Preedit(Vec<u16>),
    /// 上屏：替换组合，或直接插到光标处
    Commit(Vec<u16>),
    /// 丢掉组合（取消、切走）
    Cancel,
}

/// 把一帧排到 TSF 的 edit session 里执行（TSF 只允许在 edit session 里改文本）。
fn apply_frame(context: &ITfContext, tid: u32, state: Rc<State>, key: isize, frame: &Frame) {
    let kind = if let Some(commit) = &frame.commit {
        if commit.is_empty() {
            return;
        }
        EditKind::Commit(commit.encode_utf16().collect())
    } else if !frame.preedit.is_empty() {
        EditKind::Preedit(frame.preedit.encode_utf16().collect())
    } else if frame.handled {
        EditKind::Cancel
    } else {
        return;
    };
    let session: ITfEditSession = FrameEditSession {
        context: context.clone(),
        state,
        key,
        kind,
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
        Ok(())
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
        let mut selection: [TS_SELECTION_ACP; 1] = [TS_SELECTION_ACP::default()];
        let mut fetched = 0u32;
        context.GetSelection(0, &mut selection, &mut fetched)?;
        let start: ITfRange = context.GetStart(ec)?;
        let extent: ITfRangeACP = start.cast()?;
        extent.SetExtent(selection[0].acpStart, 0)?;

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
        context_composition.StartComposition(ec, &start, &sink)
    }
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
