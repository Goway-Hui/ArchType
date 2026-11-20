//! 一键设置：把 ArchType 加进 fcitx5 的输入法列表，必要时切输入法框架。

use crate::config;
use crate::fcitx;
use std::io::IsTerminal;
use std::process::Command;

pub const SETUP_OK: i32 = 0;
pub const SETUP_FAILED: i32 = 1;
pub const SETUP_RELOGIN: i32 = 2;
pub const SETUP_RESTARTED: i32 = 3;
pub const SETUP_UNFINISHED: i32 = 4;

const RESTART_NOTE: &str =
    "重启后，Chrome、VS Code、微信等 Chromium / Electron 程序要彻底退出再打开才能打中文。";

/// im-config 的当前设置；没有 im-config 或读不出来是 None。
fn im_config_active() -> Option<String> {
    let output = Command::new("im-config").arg("-m").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let lines: Vec<&str> = text.lines().collect();
    let mut active = lines.get(1).map(|line| line.trim().to_string()).unwrap_or_default();
    if active.is_empty() || active == "missing" {
        active = lines.first().map(|line| line.trim().to_string()).unwrap_or_default();
    }
    if active.is_empty() { None } else { Some(active) }
}

/// 系统自己管输入法框架、不该去动：deepin。
fn own_im_framework() -> bool {
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    text.lines().any(|line| {
        line.strip_prefix("ID=")
            .map(|value| value.trim().trim_matches('"') == "deepin")
            .unwrap_or(false)
    })
}

fn os_release_pretty() -> String {
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
            return rest.trim().trim_matches('"').to_string();
        }
    }
    "未知".to_string()
}

/// 一键设置该做什么：add / restart / im-config / profile。
fn setup_action(known: Option<bool>, im_active: &Option<String>) -> &'static str {
    match known {
        Some(true) => "add",
        Some(false) => "restart",
        None => match im_active {
            Some(active) if active != "fcitx5" => "im-config",
            _ => "profile",
        },
    }
}

fn ask_terminal(question: &str, assume_yes: bool) -> bool {
    if assume_yes || !std::io::stdin().is_terminal() {
        return true;
    }
    print!("{question} [Y/n] ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return true;
    }
    let answer = answer.trim();
    answer.is_empty() || answer.starts_with(['Y', 'y', '是'])
}

/// 把 ArchType 加进当前输入法列表。返回退出码：0 成功（含本来就在），1 失败，3 要先重启。
pub fn add_to_group(restart: bool, say: impl Fn(&str)) -> i32 {
    let first = fcitx::first_time();
    let known = fcitx::knows_archtype();
    if known.is_none() {
        match fcitx::profile_add_archtype() {
            Ok(text) => say(&text),
            Err(error) => {
                say(&error);
                return 1;
            }
        }
        if first {
            free_left_shift_first_time(false, &say);
        }
        return 0;
    }
    let mut known = known.unwrap_or(false);
    if !known && !restart {
        say("fcitx5 还没载入 ArchType，要先重启 fcitx5");
        return 3;
    }
    if restart {
        fcitx::restart();
        known = false;
        for _ in 0..60 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if fcitx::knows_archtype() == Some(true) {
                known = true;
                break;
            }
        }
        if !known {
            say("重启 fcitx5 以后还是找不到 ArchType，请注销重新登录后在 fcitx5 配置里添加");
            return 1;
        }
        say("已重启 fcitx5");
    }
    let Some(group) = fcitx::current_group() else {
        say("读不到当前的输入法分组，请在 fcitx5 配置里手动添加 ArchType");
        return 1;
    };
    let Some((_, layout, items)) = fcitx::group_info(&group) else {
        say("读不到输入法列表，请在 fcitx5 配置里手动添加 ArchType");
        return 1;
    };
    if items.iter().any(|(name, _)| name == "archtype") {
        say(&format!("ArchType 已经在输入法列表「{group}」里"));
        return 0;
    }
    let mut updated = items.clone();
    updated.push(("archtype".to_string(), String::new()));
    if !fcitx::set_group_info(&group, &layout, &updated) {
        say("添加失败，请在 fcitx5 配置里手动添加 ArchType");
        return 1;
    }
    say(&format!("已把 ArchType 加进输入法列表「{group}」"));
    if first {
        free_left_shift_first_time(true, &say);
    }
    0
}

/// 第一次配置 fcitx5 时，把默认占着的左 Shift（临时切换输入法）让给 ArchType。
/// fcitx5 在跑时不动（它自己存文件，要经 D-Bus 改；这里只处理还没跑的情况）。
fn free_left_shift_first_time(running: bool, say: &dyn Fn(&str)) -> bool {
    let toggle = config::get("ShiftToggle");
    let toggle = if toggle.is_empty() { "Both".to_string() } else { toggle };
    let needed = matches!(toggle.as_str(), "Both" | "Left");
    if !needed || running {
        return false;
    }
    let path = format!("{}/fcitx5/config", config::config_home());
    let base = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        std::fs::read_to_string("/etc/xdg/fcitx5/config").unwrap_or_default()
    });
    let updated = match strip_shift_l(&base) {
        Some(text) => text,
        None => return false,
    };
    config::ensure_parent(&path);
    if std::fs::write(&path, updated).is_err() {
        return false;
    }
    let _ = config::state_set("fcitx_shift_freed", "true");
    say("左 Shift 让给了 ArchType（单按切中英文），切换输入法用 Ctrl+空格；想还给 fcitx5，到「ArchType 设置」的「输入」页关掉");
    true
}

/// 从 fcitx5 全局配置里把 `[Hotkey]` 的 AltTriggerKeys 中的 Shift_L 去掉。
fn strip_shift_l(text: &str) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    let mut section = String::new();
    let mut changed = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed[1..trimmed.len() - 1].to_string();
            out.push(line.to_string());
            continue;
        }
        if section == "Hotkey" && trimmed.starts_with("AltTriggerKeys=") {
            let keys: Vec<&str> = trimmed
                .trim_start_matches("AltTriggerKeys=")
                .split(',')
                .filter(|key| !key.trim().is_empty() && key.trim() != "Shift_L")
                .collect();
            out.push(format!("AltTriggerKeys={}", keys.join(",")));
            changed = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !changed {
        return None;
    }
    let mut result = out.join("\n");
    result.push('\n');
    Some(result)
}

/// `archtype-config --setup`。
pub fn setup(args: &[String]) -> i32 {
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    let yes = flag("--yes") || std::env::var("ARCHTYPE_YES").is_ok();
    let im_allowed =
        !flag("--no-im-config") && std::env::var("ARCHTYPE_NO_IM_CONFIG").is_err();
    let allow_restart = !flag("--no-restart") && std::env::var("ARCHTYPE_NO_RESTART").is_err();
    let first = fcitx::first_time();
    let known = fcitx::knows_archtype();
    let im_active = if im_allowed && known.is_none() && !own_im_framework() {
        im_config_active()
    } else {
        None
    };
    let mut known = known;
    if known == Some(true) {
        let running = fcitx::running_addon_version();
        if fcitx::restart_needed(&running, &config::version()) {
            known = Some(false);
        }
    }
    let action = setup_action(known, &im_active);
    let say = |text: &str| println!("ArchType：{text}");
    let question = match action {
        "im-config" => Some(format!(
            "现在的输入法框架是「{}」，切换到 fcitx5 吗？（要注销重新登录一次）",
            im_active.clone().unwrap_or_default()
        )),
        "restart" => Some(format!(
            "fcitx5 正在运行，要重启一次才能载入 ArchType。现在重启它并把 ArchType 加进输入法列表吗？{RESTART_NOTE}"
        )),
        _ => None,
    };
    let confirmed = question.is_none()
        || (action == "restart" && !allow_restart)
        || question
            .as_deref()
            .map(|question| ask_terminal(question, yes))
            .unwrap_or(true);
    if action == "restart" && !(confirmed && allow_restart) {
        say("没重启 fcitx5。之后重启它（archtype-config --restart-fcitx）或注销再登录，再运行 archtype-config --setup，或在「ArchType 设置」第一页点「一键设置」");
        return SETUP_UNFINISHED;
    }
    if action == "add" || action == "restart" {
        if add_to_group(action == "restart", |text| say(text)) != 0 {
            return SETUP_FAILED;
        }
        return if action == "restart" { SETUP_RESTARTED } else { SETUP_OK };
    }
    let mut code = SETUP_RELOGIN;
    if action == "im-config" {
        if !confirmed {
            say("没切换输入法框架，ArchType 要在 fcitx5 下才能用");
            code = SETUP_UNFINISHED;
        } else {
            let status = Command::new("im-config")
                .args(["-n", "fcitx5"])
                .status();
            match status {
                Ok(status) if status.success() => say("已把输入法框架切到 fcitx5（im-config）"),
                _ => {
                    say("im-config 切换失败，请手动运行：im-config -n fcitx5，然后注销重新登录");
                    code = SETUP_UNFINISHED;
                }
            }
        }
    } else if im_allowed && !own_im_framework() && Command::new("im-config").arg("-m").output().is_err() {
        say(&format!(
            "请在桌面设置中启用 Fcitx 5 并注销重新登录（{}）",
            os_release_pretty()
        ));
    }
    match fcitx::profile_add_archtype() {
        Ok(text) => say(&text),
        Err(error) => {
            say(&error);
            return SETUP_FAILED;
        }
    }
    if first {
        free_left_shift_first_time(false, &say);
    }
    code
}

/// `archtype-config --notify-setup`：还差哪一步就发一条通知。
pub fn notify_setup() -> i32 {
    let known = fcitx::knows_archtype();
    let mut in_group = false;
    if known == Some(true) {
        if let Some(group) = fcitx::current_group() {
            if let Some((_, _, items)) = fcitx::group_info(&group) {
                in_group = items.iter().any(|(name, _)| name == "archtype");
            }
        }
    }
    let hint = match known {
        Some(true) if in_group => None,
        Some(true) => Some((
            "ArchType 已安装，还没加进输入法列表",
            "打开应用菜单里的「ArchType 设置」，在第一页点「添加」。",
        )),
        Some(false) => Some((
            "ArchType 已安装，要重启一次输入法",
            "打开应用菜单里的「ArchType 设置」，在第一页点「重启并添加」。",
        )),
        None => Some((
            "ArchType 已安装，还差一步",
            "打开应用菜单里的「ArchType 设置」，在第一页点「一键设置」。",
        )),
    };
    if let Some((summary, body)) = hint {
        let _ = Command::new("notify-send")
            .args(["--app-name=ArchType", "--icon=archtype", summary, body])
            .status();
    }
    0
}
