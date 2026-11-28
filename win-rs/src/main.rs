//! `archtype.exe`：注册/卸载、自检、版本（和 macOS 的 `ArchType --selftest` 一个意思）。

#![cfg(windows)]

use archtype_shell::config::Config;
use archtype_shell::engine::Layout;
use archtype_shell::input::{make_key, Input};

fn main() {
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        "--register" => finish(register_for_current_user()),
        "--unregister" => finish(archtype_tsf::unregister()),
        "--enable-profile" => finish(archtype_tsf::enable_profile()),
        "--disable-profile" => finish(archtype_tsf::disable_profile()),
        "--version" | "-V" => println!("ArchType {}", env!("CARGO_PKG_VERSION")),
        "--selftest" => std::process::exit(if selftest() { 0 } else { 1 }),
        "--check-update" => {
            let current = env!("CARGO_PKG_VERSION");
            match archtype_shell::update::latest_version() {
                Some(latest) if archtype_shell::update::is_newer(&latest, current) => {
                    println!("有新版本 {latest}（当前 {current}）")
                }
                Some(_) => println!("已是最新（{current}）"),
                None => {
                    println!("检查更新失败");
                    std::process::exit(1);
                }
            }
        }
        "--install-update" => std::process::exit(install_update()),
        "--mode" => {
            let mode = std::env::args().nth(2).unwrap_or_else(|| "toggle".into());
            let path = archtype_tsf::paths::support_dir().join("mode-request");
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::process::exit(if std::fs::write(&path, mode).is_ok() {
                0
            } else {
                1
            });
        }
        "--install-lexicon" | "--use-lexicon" => {
            let layout = archtype_tsf::paths::layout();
            let env = LexiconEnv { layout };
            let argument = std::env::args().nth(2).unwrap_or_default();
            let code = if command == "--use-lexicon" {
                archtype_lexicon::use_lexicon(&env, &argument)
            } else {
                archtype_lexicon::install_lexicon(&env, &argument)
            };
            std::process::exit(code);
        }
        _ => {
            println!(
                "用法：archtype <--register|--unregister|--selftest|--version|\
                 --check-update|--install-update|\
                 --install-lexicon [wanxiang|文件.tar.gz]|--use-lexicon rime-ice|wanxiang|\
                 --mode zh|en|translate|toggle|--enable-profile|--disable-profile>"
            );
        }
    }
}

/// 注册 HKLM 之后顺手把当前用户加进输入法列表：手动/源码安装也不用再去语言设置里添加。
/// 没装中文语言时只提示，不把注册判失败（MSI 的 Active Setup 会在各用户登录时再试）。
fn register_for_current_user() -> Result<(), String> {
    archtype_tsf::register()?;
    if let Err(error) = archtype_tsf::enable_profile() {
        eprintln!("ArchType: {error}；装好中文语言后运行 archtype --enable-profile");
    }
    Ok(())
}

fn finish(result: Result<(), String>) {
    match result {
        Ok(()) => println!("OK"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

/// 下载并安装新版 MSI（需要管理员）。
fn install_update() -> i32 {
    let current = env!("CARGO_PKG_VERSION");
    let Some(latest) = archtype_shell::update::latest_version() else {
        eprintln!("检查更新失败");
        return 1;
    };
    if !archtype_shell::update::is_newer(&latest, current) {
        println!("已是最新（{current}）");
        return 0;
    }
    let base = format!("https://github.com/Goway-Hui/ArchType/releases/download/v{latest}");
    let name = format!("ArchType-{latest}-x64.msi");
    let dir = std::env::temp_dir().join("archtype-update");
    let msi = match archtype_shell::update::download_verified(&name, &base, &dir) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("下载安装包失败：{error}");
            return 1;
        }
    };
    println!("下载完成：{}", msi.display());
    match std::process::Command::new("msiexec")
        .args(["/i", &msi.to_string_lossy(), "/qn", "/norestart"])
        .status()
    {
        Ok(status) if status.success() => {
            println!("已安装 {latest}");
            0
        }
        Ok(status) => {
            eprintln!("msiexec 失败：{status}");
            1
        }
        Err(error) => {
            eprintln!("跑不了 msiexec：{error}");
            1
        }
    }
}

/// 词库包（万象）安装/切换用的环境：数据目录与 archtype.conf。
struct LexiconEnv {
    layout: Layout,
}

impl archtype_lexicon::Env for LexiconEnv {
    fn data_dir(&self) -> String {
        self.layout.support_dir.to_string_lossy().into_owned()
    }

    fn get(&self, key: &str) -> String {
        Config::load(&self.layout.config_path().to_string_lossy())
            .value(key)
            .unwrap_or_default()
    }

    fn set(&self, key: &str, value: &str) -> bool {
        Config::set_key_in_file(&self.layout.config_path(), key, value)
    }

    fn reload(&self) -> bool {
        true
    }
}

/// 载入资源目录里的词库打 `nihao`，和 macOS 的自检同一套。
fn selftest() -> bool {
    let layout = archtype_tsf::paths::layout();
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

    let mut frame = input.frame(false);
    for c in ['n', 'i', 'h', 'a', 'o'] {
        frame = input.key(&make_key(c as u32, 0, false, None, None, 0, 0, false, 0));
    }
    let first = input
        .shown()
        .first()
        .map(|c| c.text.clone())
        .unwrap_or_default();
    println!("nihao → 第一个候选「{first}」，编辑区「{}」", frame.preedit);
    let frame = input.key(&make_key(0x20, 0, false, None, None, 0, 0, false, 0));
    let committed = frame.commit.unwrap_or_default();
    println!("nihao 空格 → 上屏「{committed}」");
    committed == "你好"
}
