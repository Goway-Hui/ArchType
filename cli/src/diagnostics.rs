//! 诊断信息：版本、系统、配置、插件和日志摘要，导出给用户贴到 issue 里。

use crate::config;
use std::process::Command;

pub fn collect() -> String {
    let mut out = String::new();
    out.push_str(&format!("ArchType {}\n", config::version()));
    out.push_str(&format!("系统：{}\n", os_release_name()));
    out.push_str(&format!(
        "桌面：{} / {}\n",
        std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "-".to_string()),
        std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "-".to_string())
    ));
    out.push_str(&format!("fcitx5：{}\n", run(&["fcitx5", "--version"])));
    out.push_str(&format!(
        "fctix5 在跑：{}\n",
        if process_running("fcitx5") { "是" } else { "否" }
    ));
    out.push_str(&format!("插件版本：{}\n", config::version()));
    out.push_str(&format!(
        "在跑的插件版本：{}\n",
        match crate::fcitx::running_addon_version() {
            None => "说不清".to_string(),
            Some(version) if version.is_empty() => "0.4.0 及以前".to_string(),
            Some(version) => version,
        }
    ));
    out.push_str(&format!("配置：{}\n", config::config_path()));
    out.push_str(&format!("数据目录：{}\n", config::data_dir()));
    let config_text = config::read(&config::config_path());
    if config_text.trim().is_empty() {
        out.push_str("\narchtype.conf：没有或为空\n");
    } else {
        out.push_str("\narchtype.conf：\n");
        for line in config_text.lines().take(200) {
            out.push_str(&format!("  {line}\n"));
        }
    }
    let log_path = format!("{}/archtype.log", log_dir());
    if let Ok(text) = std::fs::read_to_string(&log_path) {
        out.push_str(&format!("\n{log_path} 最后 100 行：\n"));
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.len().saturating_sub(100);
        for line in &lines[start..] {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out
}

pub fn export_to(target: &str, text: &str) -> Result<String, String> {
    let path = std::path::Path::new(target);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    if let Some(parent) = absolute.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&absolute, text).map_err(|error| error.to_string())?;
    Ok(absolute.display().to_string())
}

fn log_dir() -> String {
    std::env::var("XDG_STATE_HOME").unwrap_or_else(|_| format!("{}/.local/state", config::home()))
        + "/archtype"
}

fn os_release_name() -> String {
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
            return rest.trim().trim_matches('"').to_string();
        }
    }
    "未知".to_string()
}

fn run(command: &[&str]) -> String {
    let Ok(output) = Command::new(command[0]).args(&command[1..]).output() else {
        return format!("（跑不了 {}）", command[0]);
    };
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let text = text.trim().to_string();
    if text.is_empty() {
        "（无输出）".to_string()
    } else {
        text
    }
}

fn process_running(name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
