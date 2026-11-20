//! archtype-config 的命令行部分（原来在 settings/archtype-config 的 Python 里）。
//!
//! 只做系统集成：fcitx5 的输入法列表、默认主题、词库和模型下载、诊断导出、
//! 更新检查。图形设置在 settings-nsh（NSH）里。

mod config;
mod diagnostics;
mod fcitx;
mod lexicon;
mod net;
mod setup;
mod startup;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = dispatch(&args);
    std::process::exit(code);
}

fn dispatch(args: &[String]) -> i32 {
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    if flag("--version") || flag("-V") {
        println!("archtype-config {}", config::version());
        return 0;
    }
    if flag("--help") || flag("-h") {
        print_help();
        return 0;
    }
    if flag("--restart-fcitx") {
        let ok = fcitx::restart();
        println!("{}", if ok { "已重启 fcitx5" } else { "重启 fcitx5 失败，请注销后重新登录" });
        return if ok { 0 } else { 1 };
    }
    if flag("--setup") {
        return setup::setup(args);
    }
    if flag("--add-to-group") {
        return setup::add_to_group(flag("--restart"), |text| println!("{text}"));
    }
    if flag("--notify-setup") {
        return setup::notify_setup();
    }
    if flag("--startup") || flag("--check-update") {
        return startup::tasks();
    }
    if flag("--use-lexicon") {
        let kind = value_after(args, "--use-lexicon").unwrap_or("rime-ice");
        return lexicon::use_lexicon(kind);
    }
    if flag("--install-lexicon") {
        let source = value_after(args, "--install-lexicon").unwrap_or("wanxiang");
        return lexicon::install_lexicon(source);
    }
    if flag("--self-test") {
        return self_test();
    }
    if flag("--export-log") {
        return export_log(args);
    }
    if args.is_empty() {
        // 图形界面在 NSH 打的 archtype-settings 里（桌面入口通常直接指它）
        let status = std::process::Command::new("archtype-settings").status();
        match status {
            Ok(status) => return status.code().unwrap_or(0),
            Err(_) => {
                eprintln!(
                    "图形界面是「ArchType 设置」（archtype-settings，NSH 打包的）。\n\
                     命令行用法见 archtype-config --help"
                );
                return 1;
            }
        }
    }
    eprintln!("未知参数：{}", args.join(" "));
    print_help();
    2
}

fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == flag)?;
    args.get(at + 1)
        .filter(|value| !value.starts_with("--"))
        .map(String::as_str)
}

fn export_log(args: &[String]) -> i32 {
    let position = args.iter().position(|arg| arg == "--export-log");
    let target = position
        .and_then(|at| args.get(at + 1))
        .map(String::as_str)
        .unwrap_or("-");
    let text = diagnostics::collect();
    if target == "-" {
        print!("{text}");
        return 0;
    }
    match diagnostics::export_to(target, &text) {
        Ok(path) => {
            println!("已保存到 {path}");
            0
        }
        Err(error) => {
            eprintln!("写不进 {target}：{error}");
            1
        }
    }
}

fn self_test() -> i32 {
    println!("self-test ok");
    0
}

fn print_help() {
    println!(
        "用法：archtype-config [选项]\n\
         \n\
         --setup [--yes] [--no-im-config] [--no-restart]  一键设置：切到 fcitx5、写好输入法列表\n\
         --add-to-group [--restart]                       把 ArchType 加进当前输入法列表\n\
         --restart-fcitx                                  重启 fcitx5\n\
         --startup / --check-update                       插件切到 ArchType 时在后台跑\n\
         --notify-setup                                   还差哪一步就发一条通知\n\
         --export-log [路径|-]                            导出诊断信息\n\
         --install-lexicon [wanxiang|包.tar.gz]           下载并切换万象词库\n\
         --use-lexicon <rime-ice|wanxiang>                切换词库\n\
         --self-test                                      自检\n\
         \n\
         语音模型、离线翻译模型的下载在「ArchType 设置」的「语音」「翻译」页。"
    );
}
