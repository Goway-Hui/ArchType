//! 插件切到 ArchType 时在后台跑（`--startup` / `--check-update`）。

use crate::config;
use crate::fcitx;
use archtype_net as net;
use std::process::Command;

/// 首次使用时给 classicui 写 ArchType 的主题（浅色/深色各一套）。原来设过的就不动。
pub fn init_default_theme() {
    if !config::classicui_get("Theme").is_empty() {
        return;
    }
    config::classicui_set("Theme", "archtype");
    config::classicui_set("DarkTheme", "archtype-dark");
    config::classicui_set("UseDarkTheme", "True");
}

fn notify(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["--app-name=ArchType", "--icon=archtype", summary, body])
        .status();
}

/// 升级以后还在跑旧插件：提醒一次重启输入法。
fn restart_reminder() {
    let running = fcitx::running_addon_version();
    if !fcitx::restart_needed(&running, &config::version()) {
        return;
    }
    let installed = config::version();
    if config::state_get("restart_reminded") == installed {
        return;
    }
    notify(
        &format!("ArchType 已更新到 {installed}，重启输入法后生效"),
        "重启后，Chrome、VS Code、微信等 Chromium / Electron 程序要彻底退出再打开才能打中文。",
    );
    let _ = config::state_set("restart_reminded", &installed);
}

/// 最多一小时查一次；有新版本就发一条通知（同一版本只提醒一次）。
fn check_update() {
    if config::get("CheckUpdate") == "False" {
        return;
    }
    let last = config::state_get("update_checked")
        .parse::<u64>()
        .unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(last) < 3600 {
        return;
    }
    let _ = config::state_set("update_checked", &now.to_string());
    let Some(tag) = net::latest_release_tag() else {
        return;
    };
    if config::state_get("update_notified") == tag {
        return;
    }
    if !newer_version(&tag, &config::version()) {
        return;
    }
    notify(
        &format!("ArchType {tag} 可以更新了"),
        "打开「ArchType 设置」的「更新」页查看这一版的变化。",
    );
    let _ = config::state_set("update_notified", &tag);
}

/// 后台自动更新正在用的万象词库：最多一小时查一次，有新包就换上（失败不打扰用户）。
fn auto_update_lexicon() {
    if config::get("CheckUpdate") == "False" {
        return;
    }
    let last = config::state_get("lexicon_checked")
        .parse::<u64>()
        .unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    if now.saturating_sub(last) < 3600 {
        return;
    }
    let _ = config::state_set("lexicon_checked", &now.to_string());
    if let Ok(true) = crate::lexicon::auto_update() {
        notify(
            "万象词库已更新",
            "正在用的词库换成了新版本，重新切到 ArchType（或重启输入法）后用上新词。",
        );
    }
}

/// `--startup`：默认主题 + 重启提醒 + 检查更新。
pub fn tasks() -> i32 {
    init_default_theme();
    restart_reminder();
    check_update();
    auto_update_lexicon();
    0
}

fn version_tuple(text: &str) -> Vec<u64> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn newer_version(candidate: &str, current: &str) -> bool {
    version_tuple(candidate) > version_tuple(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer_version("0.6.2", "0.6.1"));
        assert!(!newer_version("0.6.1", "0.6.1"));
        assert!(!newer_version("0.5.9", "0.6.1"));
    }
}
