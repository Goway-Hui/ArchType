//! ArchType 设置界面的 Rust 侧：配置读写、fcitx5 重载、字体字号。
//! `App.tsx` 从 `archtype/native` 导入。

use nsh_core::prelude::*;
use std::fs;
use std::path::Path;

/// 应用自己的 UI 集合。
#[native_module("archtype/native")]
fn native(registry: &mut Registry) {
    registry.module::<Config>();
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn config_home() -> String {
    std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home()))
}

fn data_home() -> String {
    std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home()))
}

fn state_home() -> String {
    std::env::var("XDG_STATE_HOME").unwrap_or_else(|_| format!("{}/.local/state", home()))
}

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn atomic_write(path: &str, text: &str) -> bool {
    if let Some(dir) = Path::new(path).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let tmp = format!("{path}.tmp");
    if fs::write(&tmp, text).is_err() {
        return false;
    }
    fs::rename(&tmp, path).is_ok()
}

/// 更新 `key=value`，保留注释、未知键和分组行；没有就追加。
fn set_key(path: &str, key: &str, value: &str) -> bool {
    let text = read(path);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut found = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            if k.trim() == key {
                *line = format!("{key}={value}");
                found = true;
                break;
            }
        }
    }
    if !found {
        lines.push(format!("{key}={value}"));
    }
    let mut out = lines.join("\n");
    out.push('\n');
    atomic_write(path, &out)
}

fn get_key(path: &str, key: &str) -> String {
    for line in read(path).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim() == key {
                return v.trim().to_string();
            }
        }
    }
    String::new()
}

fn json_get(path: &str, key: &str) -> String {
    let text = read(path);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return String::new();
    };
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn json_set(path: &str, key: &str, value: &str) -> bool {
    let text = read(path);
    let mut object = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    object.insert(key.to_string(), serde_json::Value::String(value.to_string()));
    let Ok(text) = serde_json::to_string_pretty(&object) else {
        return false;
    };
    atomic_write(path, &text)
}

fn classicui_path() -> String {
    format!("{}/fcitx5/conf/classicui.conf", config_home())
}

fn state_path() -> String {
    format!("{}/archtype/state.json", state_home())
}

/// 字号在字体字符串末尾（`JetBrains Maple Mono 14` → `14`）。
fn font_size_of(font: &str) -> String {
    font.split_whitespace()
        .last()
        .filter(|part| part.parse::<f32>().is_ok())
        .unwrap_or("")
        .to_string()
}

/// 配置和数据文件。
#[module]
pub struct Config;

#[module]
impl Config {
    /// archtype.conf 的完整路径。
    pub fn config_path() -> String {
        format!("{}/fcitx5/conf/archtype.conf", config_home())
    }

    /// 用户数据目录（词库、模型、自定义词）。
    pub fn data_dir() -> String {
        format!("{}/archtype", data_home())
    }

    /// 读 archtype.conf 原文；读不到返回空串。
    pub fn read() -> String {
        read(&Config::config_path())
    }

    /// 读 archtype.conf 的一项。
    pub fn get(key: String) -> String {
        get_key(&Config::config_path(), &key)
    }

    /// 写 archtype.conf 的一项。
    pub fn set(key: String, value: String) -> bool {
        set_key(&Config::config_path(), &key, &value)
    }

    /// 读 state.json 的一项（界面主题偏好等）。
    pub fn state_get(key: String) -> String {
        json_get(&state_path(), &key)
    }

    /// 写 state.json 的一项。
    pub fn state_set(key: String, value: String) -> bool {
        json_set(&state_path(), &key, &value)
    }

    /// 读 classicui.conf（fcitx5 经典界面主题、字体）。
    pub fn classicui_get(key: String) -> String {
        get_key(&classicui_path(), &key)
    }

    /// 写 classicui.conf。
    pub fn classicui_set(key: String, value: String) -> bool {
        set_key(&classicui_path(), &key, &value)
    }

    /// 重启 fcitx5 让配置生效。
    pub fn restart_fcitx() -> bool {
        std::process::Command::new("fcitx5-remote")
            .arg("-r")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// 当前候选字号（classicui 的 Font 末尾数字）。
    pub fn font_size() -> String {
        font_size_of(&Config::classicui_get("Font".to_string()))
    }

    /// 改候选字号，保留字体名。
    pub fn set_font_size(size: String) -> bool {
        let font = Config::classicui_get("Font".to_string());
        if font.is_empty() {
            return Config::classicui_set("Font".to_string(), format!("Sans {size}"));
        }
        let mut parts: Vec<&str> = font.split_whitespace().collect();
        if parts.last().is_some_and(|p| p.parse::<f32>().is_ok()) {
            parts.pop();
        }
        let name = parts.join(" ");
        Config::classicui_set("Font".to_string(), format!("{name} {size}"))
    }

    /// 自定义词文件原文（每行「词<Tab>拼音」）。
    pub fn custom_words() -> String {
        read(&format!("{}/custom.tsv", Config::data_dir()))
    }

    /// 语音模型目录状态：有模型返回目录，没有返回空串。
    pub fn voice_model() -> String {
        let dir = format!("{}/wtlocal", Config::data_dir());
        let has_model = std::fs::read_dir(&dir)
            .map(|entries| entries.flatten().any(|e| e.file_name() != ".." && e.file_name() != "."))
            .unwrap_or(false);
        if has_model { dir } else { String::new() }
    }

    /// 统计文件原文。
    pub fn stats() -> String {
        read(&format!("{}/stats.json", Config::data_dir()))
    }

    /// 设置界面版本。
    pub fn version() -> String {
        "0.6.0".to_string()
    }
}
