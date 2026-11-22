//! `archtype.exe`：注册/卸载、自检、版本（和 macOS 的 `ArchType --selftest` 一个意思）。

#![cfg(windows)]

use archtype_shell::config::Config;
use archtype_shell::input::{make_key, Input};

fn main() {
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        "--register" => finish(archtype_tsf::register()),
        "--unregister" => finish(archtype_tsf::unregister()),
        "--version" | "-V" => println!("ArchType {}", env!("CARGO_PKG_VERSION")),
        "--selftest" => std::process::exit(if selftest() { 0 } else { 1 }),
        _ => {
            println!("用法：archtype <--register|--unregister|--selftest|--version>");
        }
    }
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
