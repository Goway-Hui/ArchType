//! macOS 输入法外壳的入口：`ArchType`（输入法）、`--settings`、`--selftest`、`--version`。
//!
//! 引擎、会话、设置都在平台无关的模块里（`crate::input`、`crate::config`、`crate::engine`）；
//! 这里只接 InputMethodKit / AppKit。

mod app;
mod controller;
mod external;
mod installer;
mod panel;

use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSWorkspace, NSWorkspaceOpenConfiguration,
};
use objc2_foundation::{NSArray, NSString, NSURL};
use objc2_input_method_kit::IMKServer;

use crate::config::Config;
use crate::engine::Layout;
use crate::input::Input;

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => println!("ArchType {}", app::version()),
        Some("--selftest") => {
            let ok = selftest();
            std::process::exit(if ok { 0 } else { 1 });
        }
        Some("--settings") => open_settings(args.get(1).map(String::as_str)),
        Some("--mode") => exit_with(external::mode_command(
            args.get(1).map(String::as_str).unwrap_or(""),
        )),
        Some("--register") => exit_with(installer::register()),
        Some("--enable") => exit_with(installer::enable()),
        Some("--select") => exit_with(installer::select()),
        Some("--disable") => exit_with(installer::disable()),
        Some("--install-lexicon") | Some("--use-lexicon") => {
            let layout = &app::start().layout;
            let env = LexiconEnv { layout };
            let argument = args.get(1).map(String::as_str).unwrap_or("");
            let code = if args[0] == "--use-lexicon" {
                archtype_lexicon::use_lexicon(&env, argument)
            } else {
                archtype_lexicon::install_lexicon(&env, argument)
            };
            std::process::exit(code);
        }
        Some("--uninstall") => {
            let layout = &app::start().layout;
            let (ok, message) = installer::uninstall(&layout.support_dir);
            println!("{message}");
            std::process::exit(if ok { 0 } else { 1 });
        }
        Some("--help") | Some("-h") => usage(),
        Some(other) => {
            eprintln!("不认识的参数：{other}");
            usage();
            std::process::exit(2);
        }
        None => run_input_method(),
    }
}

/// 词库命令用的路径与配置读写：macOS 上数据在 Application Support，配置是 archtype.conf。
/// 输入法自己盯着配置文件，写完就算生效（reload 返回 true）。
struct LexiconEnv<'a> {
    layout: &'a crate::engine::Layout,
}

impl archtype_lexicon::Env for LexiconEnv<'_> {
    fn data_dir(&self) -> String {
        self.layout.support_dir.to_string_lossy().into_owned()
    }

    fn get(&self, key: &str) -> String {
        let config = Config::load(&self.layout.config_path().to_string_lossy());
        config.value(key).unwrap_or_default()
    }

    fn set(&self, key: &str, value: &str) -> bool {
        Config::set_key_in_file(&self.layout.config_path(), key, value)
    }

    fn reload(&self) -> bool {
        true
    }
}

fn exit_with(ok: bool) -> ! {
    std::process::exit(if ok { 0 } else { 1 });
}

/// 被 kill（安装新版、注销）时先存学习数据：信号处理器只立个标志，主线程的定时器看到再存。
fn install_signal_handlers() {
    const SIGTERM: i32 = 15;
    extern "C" fn on_terminate(_signal: i32) {
        app::request_terminate();
    }
    unsafe {
        signal(SIGTERM, on_terminate as usize);
    }
}

unsafe extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

fn usage() {
    println!(
        "用法：ArchType [--register | --enable | --select | --disable | --selftest | --settings [页面] |\n\
         \t\t\t--install-lexicon [wanxiang|文件.tar.gz] | --use-lexicon rime-ice|wanxiang |\n\
         \t\t\t--mode [zh | en | translate | toggle] |\n\
         \t\t\t--uninstall | --version | --help]\n\
         \n\
         不带参数时是输入法进程（系统按 Info.plist 把它拉起来）。\n\
         --register / --enable / --select / --disable 由安装包的 postinstall 调用。\n\
         设置界面是「ArchType 设置.app」（随安装包带的跨平台 app）。"
    );
}

fn run_input_method() {
    let mtm = MainThreadMarker::new().expect("输入法外壳必须跑在主线程");
    // 后台载入词库和引擎；输入控制器在引擎就绪前不接按键
    app::start();

    let name = NSString::from_str(&app::connection_name());
    let bundle = NSString::from_str(&app::bundle_id());
    // 外部命令（ArchType --mode）切中英文
    external::start_listener();
    let server = unsafe {
        IMKServer::initWithName_bundleIdentifier(IMKServer::alloc(), Some(&name), Some(&bundle))
    };
    if server.is_none() {
        eprintln!(
            "ArchType: IMKServer 起不来（连接名 {}）",
            app::connection_name()
        );
    }

    let application = NSApplication::sharedApplication(mtm);
    let _ = application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    install_signal_handlers();
    application.run();
    std::mem::forget(server);
}

/// 打开设置窗口：另起一个进程（输入法进程里开窗口会和输入框抢焦点）。
fn open_settings(page: Option<&str>) {
    let settings_app = app::resource_dir().join("ArchType 设置.app");
    if settings_app.exists() {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&settings_app.to_string_lossy()));
        let configuration = NSWorkspaceOpenConfiguration::configuration();
        configuration.setActivates(true);
        if let Some(page) = page {
            let arguments = NSArray::from_retained_slice(&[
                NSString::from_str("--page"),
                NSString::from_str(page),
            ]);
            configuration.setArguments(&arguments);
        }
        NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(
            &url,
            &configuration,
            None,
        );
        return;
    }
    // 开发目录里没有打包的 NSH app：直接跑 nsh 的产物
    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.with_file_name("ArchType 设置");
        if candidate.exists() {
            let mut command = std::process::Command::new(candidate);
            if let Some(page) = page {
                command.arg("--page").arg(page);
            }
            let _ = command.spawn();
            return;
        }
    }
    eprintln!("ArchType: 安装包里没有「ArchType 设置.app」");
}

/// CI 的自检：载入安装包里的词库，打 nihao 看第一个候选和上屏。
fn selftest() -> bool {
    let layout = Layout::new(app::resource_dir(), Layout::support_dir_standard());
    println!("资源目录 {}", layout.resource_dir.display());
    println!("数据目录 {}", layout.support_dir.display());
    println!("配置文件 {}", layout.config_path().display());
    let config = Config::load(&layout.config_path().to_string_lossy());
    let mut input = match Input::load(layout.clone(), config) {
        Ok(input) => input,
        Err(error) => {
            println!("FAIL {error}");
            return false;
        }
    };
    for name in [
        "lexicon.tsv",
        "english.txt",
        "words.lm",
        "s2t.tsv",
        "wubi86.tsv",
        "nnlm.bin",
    ] {
        println!(
            "  {name}：{}",
            if layout.resource(name).is_some() {
                "有"
            } else {
                "没有"
            }
        );
    }

    let keys: &[char] = &['n', 'i', 'h', 'a', 'o'];
    let mut frame = input.frame(false);
    for c in keys {
        frame = input.key(&crate::input::make_key(
            *c as u32, 0, false, None, None, 0, 0, false, 0,
        ));
    }
    let first = input
        .shown()
        .first()
        .map(|c| c.text.clone())
        .unwrap_or_default();
    println!("nihao → 第一个候选「{first}」，编辑区「{}」", frame.preedit);
    let frame = input.key(&crate::input::make_key(
        0x20, 0, false, None, None, 0, 0, false, 0,
    ));
    let committed = frame.commit.unwrap_or_default();
    println!("nihao 空格 → 上屏「{committed}」");
    committed == "你好"
}
