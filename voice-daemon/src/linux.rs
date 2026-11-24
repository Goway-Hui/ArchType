//! Linux 平台层：X11 全局热键 + cpal 采集。
//!
//! 只能在 Linux 上编译。Wayland 下没有全局热键 API，需要外部按键重映射工具
//! （keyd / kanata）把键映射成给本进程发信号，或只在 fcitx5 激活时用插件路径。

use crate::Options;
use anyhow::{anyhow, Context, Result};
use archtype_voice::{
    asr::{MockChannel, MultiChannel, ProcessChannel},
    ClipboardSink, CommandSink, TextSink, VoiceSession,
};
use std::sync::mpsc::{channel, Sender};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub enum KeyMsg {
    Down,
    Up,
}

pub fn run(opt: Options) -> Result<()> {
    // 采集线程通过 channel 把 16k 单声道 PCM 送过来
    let (audio_tx, audio_rx) = channel::<Vec<i16>>();
    start_capture(opt.device.clone(), audio_tx).context("启动麦克风失败")?;

    // 热键线程
    let (key_tx, key_rx) = channel::<KeyMsg>();
    start_hotkey(opt.hotkey.clone(), key_tx).context("启动全局热键失败")?;

    let mut session = new_session(&opt)?;
    let mut last_tick = Instant::now();

    loop {
        while let Ok(msg) = key_rx.try_recv() {
            let now = Instant::now();
            match msg {
                KeyMsg::Down => {
                    let actions = session.on_key_down(now);
                    log_actions(&actions);
                    // 点按模式下第二次按下会返回 Stop（voice/src/hotkey.rs）：和 Up 分支一样收尾，
                    // 否则这一轮录音永远不走 finish()，残留音频混进下一次
                    if stops(&actions) {
                        finish_and_reset(&mut session, &opt);
                    }
                }
                KeyMsg::Up => {
                    let actions = session.on_key_up(now);
                    log_actions(&actions);
                    if stops(&actions) {
                        finish_and_reset(&mut session, &opt);
                    }
                }
            }
        }

        while let Ok(pcm) = audio_rx.try_recv() {
            session.push_pcm(&pcm, Instant::now());
        }

        if last_tick.elapsed() >= Duration::from_millis(100) {
            last_tick = Instant::now();
            if session.tick(Instant::now()).is_some() {
                finish_and_reset(&mut session, &opt);
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn stops(actions: &[archtype_voice::Action]) -> bool {
    actions.contains(&archtype_voice::Action::Stop)
}

fn log_actions(actions: &[archtype_voice::Action]) {
    for a in actions {
        println!("[热键] {:?}", a);
    }
}

fn new_session(opt: &Options) -> Result<VoiceSession> {
    Ok(VoiceSession::new(
        crate::trigger_config(opt),
        build_channel(opt)?,
        build_sink(),
        false,
    ))
}

fn finish_and_reset(session: &mut VoiceSession, opt: &Options) {
    let results = session.finish();
    for t in &results {
        println!("[定稿] {} -> {}", t.channel, t.text);
    }
    if results.is_empty() {
        println!("[定稿] 没有识别结果");
    }
    match new_session(opt) {
        Ok(fresh) => *session = fresh,
        Err(e) => eprintln!("重建语音会话失败：{e:#}"),
    }
}

/// 有 --asr-cmd 就走外部识别器，否则用 Mock 验证链路。
/// 识别渠道：`--asr` 显式指定，否则按是否有 `--asr-cmd` 判断。
pub fn build_channel(opt: &Options) -> Result<MultiChannel> {
    let mode = opt.asr.clone().unwrap_or_else(|| {
        if opt.asr_cmd.as_deref().is_some_and(|c| !c.trim().is_empty()) {
            "cmd".into()
        } else {
            "mock".into()
        }
    });

    match mode.as_str() {
        "wtlocal" => {
            let dir = opt
                .asr_dir
                .clone()
                .unwrap_or_else(archtype_voice::wtlocal::download::default_dir);
            println!("识别渠道：微信离线（{}）", dir.display());
            if let Err(e) = archtype_voice::wtlocal::download::ensure(&dir) {
                return Err(anyhow!("离线模型不可用：{e:#}"));
            }
            let ch = archtype_voice::wtlocal::WtLocalChannel::new(dir)
                .map_err(|e| anyhow!("加载离线模型失败：{e:#}"))?;
            Ok(MultiChannel::new(vec![Box::new(ch)]))
        }
        "cmd" => match opt.asr_cmd.as_deref() {
            Some(cmd) if !cmd.trim().is_empty() => {
                // 只打印程序名：参数里可能有 API key/token，别写进日志
                let program = cmd.split_whitespace().next().unwrap_or("cmd");
                println!("识别渠道：外部命令 {program}（参数不打印）");
                Ok(MultiChannel::new(vec![Box::new(
                    ProcessChannel::from_command_line("cmd", cmd),
                )]))
            }
            _ => Err(anyhow!("--asr cmd 需要配合 --asr-cmd")),
        },
        "mock" => {
            println!("识别渠道：Mock（仅用于验证链路）");
            Ok(MultiChannel::new(vec![Box::new(MockChannel::new("mock"))]))
        }
        other => Err(anyhow!("未知识别渠道 --asr {other}（mock|cmd|wtlocal）")),
    }
}

/// 上屏后端选择。
///
/// 默认走剪贴板：`ydotool type` 靠键码模拟按键，发不出汉字；
/// 环境变量 `ARCHTYPE_SINK` 可以强制指定 wtype / ydotool / clipboard / x11 / wl。
fn build_sink() -> Box<dyn TextSink> {
    let choice = std::env::var("ARCHTYPE_SINK").unwrap_or_default();
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    match choice.as_str() {
        "wtype" => Box::new(CommandSink::wtype()),
        "ydotool" => Box::new(CommandSink::ydotool()),
        "wl" => Box::new(ClipboardSink::wl()),
        "x11" => Box::new(ClipboardSink::x11()),
        "clipboard" => Box::new(if wayland {
            ClipboardSink::wl()
        } else {
            ClipboardSink::x11()
        }),
        _ => Box::new(if wayland {
            ClipboardSink::wl()
        } else {
            ClipboardSink::x11()
        }),
    }
}

fn start_capture(device: Option<String>, tx: Sender<Vec<i16>>) -> Result<()> {
    let cap = archtype_voice::capture::start(device.as_deref(), move |pcm| {
        let _ = tx.send(pcm);
    })
    .context("启动麦克风失败")?;
    println!(
        "麦克风：{}  {} Hz  {} 声道  {}",
        cap.info.device, cap.info.sample_rate, cap.info.channels, cap.info.format
    );
    // 进程常驻，故意泄漏以保持录音流存活
    std::mem::forget(cap);
    Ok(())
}

fn start_hotkey(name: String, tx: Sender<KeyMsg>) -> Result<()> {
    if std::env::var_os("DISPLAY").is_none() {
        return Err(anyhow!(
            "没有 DISPLAY。Wayland 下请用 keyd/kanata 把热键映射为调用本程序，或改用 fcitx5 插件路径"
        ));
    }
    if std::env::var("XDG_SESSION_TYPE")
        .map(|t| t.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
    {
        eprintln!(
            "提示：当前是 Wayland 会话，X11 全局热键只对 XWayland 程序有效；原生 Wayland 应用里请用 keyd/kanata 或 fcitx5 插件路径"
        );
    }
    // 等初始化（连 X、找键位、抢键）的结果：失败要让主程序退出，而不是只打一行日志
    let (ready_tx, ready_rx) = channel::<Result<()>>();
    std::thread::spawn(move || {
        if let Err(e) = hotkey_loop(&name, tx, ready_tx) {
            eprintln!("热键线程退出：{:#}", e);
        }
    });
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(anyhow!("热键初始化超时（5 秒）")),
    }
}

fn keysym_for(name: &str) -> Result<u32> {
    match name {
        "alt_r" | "alt" => Ok(0xffea),   // XK_Alt_R
        "ctrl_r" | "ctrl" => Ok(0xffe4), // XK_Control_R
        other => Err(anyhow!("不支持的热键：{}（可选 alt_r / ctrl_r）", other)),
    }
}

/// 连 X、找 keycode、开可检测自动重复、抢键。返回连接和 keycode。
fn hotkey_init(name: &str) -> Result<(x11rb::rust_connection::RustConnection, u8)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, GrabMode, ModMask};

    let keysym = keysym_for(name)?;
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;

    // keysym -> keycode
    let setup = conn.setup();
    let min = setup.min_keycode;
    let count = setup.max_keycode - min + 1;
    let mapping = conn.get_keyboard_mapping(min, count)?.reply()?;
    let per = mapping.keysyms_per_keycode as usize;
    let mut keycode = None;
    for (i, chunk) in mapping.keysyms.chunks(per).enumerate() {
        if chunk.contains(&keysym) {
            keycode = Some(min + i as u8);
            break;
        }
    }
    let keycode = keycode.ok_or_else(|| anyhow!("键盘映射里找不到该键"))?;

    // 打开 XKB 的「可检测自动重复」：否则按住热键超过重复延迟，X 会先发 Release 再发 Press，
    // 长按模式会被当成松开、录音被截断重来
    use x11rb::protocol::xkb::{self, ConnectionExt as XkbExt};
    let _ = conn.xkb_use_extension(1, 0);
    let _ = conn
        .xkb_per_client_flags(
            xkb::ID::USE_CORE_KBD.into(),
            xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
            xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
            xkb::BoolCtrl::from(0u32),
            xkb::BoolCtrl::from(0u32),
            xkb::BoolCtrl::from(0u32),
        )
        .ok()
        .and_then(|cookie| cookie.reply().ok());

    // 只抓这个键本身、锁定修饰（CapsLock/NumLock）组合，以及它作为 Mod1 按下的情况：
    // AnyModifier 会把 Ctrl+Alt_R、AltGr 组合字符一起抢走
    for modifiers in [ModMask::from(0u16), ModMask::LOCK, ModMask::M2, ModMask::M1] {
        conn.grab_key(
            true,
            root,
            modifiers,
            keycode,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?;
    }
    conn.flush()?;
    println!("已占用 X11 键位 keycode={keycode} ");
    Ok((conn, keycode))
}

fn hotkey_loop(name: &str, tx: Sender<KeyMsg>, ready: Sender<Result<()>>) -> Result<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::Event;

    let (conn, keycode) = match hotkey_init(name) {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(anyhow!("{e:#}")));
            return Err(e);
        }
    };
    let _ = ready.send(Ok(()));

    loop {
        match conn.wait_for_event()? {
            Event::KeyPress(e) if e.detail == keycode => {
                let _ = tx.send(KeyMsg::Down);
            }
            Event::KeyRelease(e) if e.detail == keycode => {
                let _ = tx.send(KeyMsg::Up);
            }
            _ => {}
        }
    }
}
