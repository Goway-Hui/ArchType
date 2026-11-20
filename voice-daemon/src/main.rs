//! archtype-voiced：语音输入常驻进程。
//!
//! ```
//! archtype-voiced --hotkey alt_r --mode tap --silence-ms 1500
//! ```
//!
//! 链路：全局热键 -> 采集 -> 重采样/分帧 -> VAD -> 识别 -> 上屏。
//! 识别渠道目前是 Mock（回显音频长度），用于先验证按键到上屏的整条链路；
//! 接真实引擎的位置在 `platform::build_channel`。

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(not(target_os = "linux"))]
mod other;
#[cfg(not(target_os = "linux"))]
use other as imp;

use std::process::ExitCode;
use std::time::Duration;
use archtype_voice::TriggerConfig;

/// 启动参数
#[derive(Debug, Clone)]
pub struct Options {
    pub hotkey: String,
    pub mode: String,
    pub silence_ms: u64,
    pub device: Option<String>,
    /// 外部识别命令，末尾会自动追加 WAV 路径。
    pub asr_cmd: Option<String>,
    /// 识别渠道：mock / cmd / wtlocal（微信离线）。为空时按 asr_cmd 自动判断。
    pub asr: Option<String>,
    /// 微信离线模型的目录，默认 ~/.local/share/archtype/wtlocal。
    pub asr_dir: Option<std::path::PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            hotkey: "alt_r".into(),
            mode: "tap".into(),
            silence_ms: 1500,
            device: None,
            asr_cmd: None,
            asr: None,
            asr_dir: None,
        }
    }
}

fn value(it: &mut impl Iterator<Item = String>, flag: &str) -> String {
    it.next().unwrap_or_else(|| {
        eprintln!("{flag} 要一个值");
        std::process::exit(2);
    })
}

fn parse_args() -> Options {
    let mut opt = Options::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--hotkey" => opt.hotkey = value(&mut it, "--hotkey"),
            "--mode" => {
                opt.mode = value(&mut it, "--mode");
                if opt.mode != "tap" && opt.mode != "hold" {
                    eprintln!("--mode 只能是 tap 或 hold");
                    std::process::exit(2);
                }
            }
            "--silence-ms" => {
                let raw = value(&mut it, "--silence-ms");
                opt.silence_ms = match raw.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("--silence-ms 要一个整数（毫秒）");
                        std::process::exit(2);
                    }
                };
            }
            "--device" => opt.device = Some(value(&mut it, "--device")),
            "--asr-cmd" => opt.asr_cmd = Some(value(&mut it, "--asr-cmd")),
            "--asr" => {
                let mode = value(&mut it, "--asr");
                if !matches!(mode.as_str(), "mock" | "cmd" | "wtlocal") {
                    eprintln!("--asr 只能是 mock、cmd 或 wtlocal");
                    std::process::exit(2);
                }
                opt.asr = Some(mode);
            }
            "--asr-dir" => opt.asr_dir = Some(std::path::PathBuf::from(value(&mut it, "--asr-dir"))),
            "--help" | "-h" => {
                println!(
                    "archtype-voiced --hotkey alt_r|ctrl_r --mode tap|hold --silence-ms 1500 [--device 名称] [--asr-cmd '命令 参数']"
                );
                println!("  --asr mock|cmd|wtlocal  识别渠道（wtlocal = 微信输入法离线，首次会下载约 100MB）");
                println!(
                    "  --asr-dir <目录>        离线模型目录，默认 ~/.local/share/archtype/wtlocal"
                );
                println!("  例：--asr-cmd 'whisper-cli -m ggml-large-v3-turbo.bin -l zh -nt -f'");
                std::process::exit(0);
            }
            other => {
                eprintln!("未知参数：{other}");
                std::process::exit(2);
            }
        }
    }
    opt
}

pub fn trigger_config(opt: &Options) -> TriggerConfig {
    TriggerConfig {
        hold_threshold: Duration::from_millis(300),
        silence_timeout: Duration::from_millis(opt.silence_ms),
        tap: opt.mode != "hold",
    }
}

fn main() -> ExitCode {
    let opt = parse_args();
    println!(
        "archtype-voiced 启动：hotkey={} mode={} silence={}ms",
        opt.hotkey, opt.mode, opt.silence_ms
    );
    match imp::run(opt) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("退出：{:#}", e);
            ExitCode::FAILURE
        }
    }
}
