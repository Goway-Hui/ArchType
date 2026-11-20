//! archtype.conf、state.json、classicui.conf 的读写。

use std::path::PathBuf;

pub fn version() -> String {
    "0.6.2".to_string()
}

pub fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
}

pub fn config_home() -> String {
    std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home()))
}

pub fn data_home() -> String {
    std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home()))
}

pub fn state_home() -> String {
    std::env::var("XDG_STATE_HOME").unwrap_or_else(|_| format!("{}/.local/state", home()))
}

pub fn config_path() -> String {
    format!("{}/fcitx5/conf/archtype.conf", config_home())
}

pub fn data_dir() -> String {
    format!("{}/archtype", data_home())
}

pub fn state_path() -> String {
    format!("{}/archtype/state.json", state_home())
}

pub fn profile_path() -> String {
    format!("{}/fcitx5/profile", config_home())
}

pub fn classicui_path() -> String {
    format!("{}/fcitx5/conf/classicui.conf", config_home())
}

pub fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn read_bytes(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

pub fn exists(path: &str) -> bool {
    PathBuf::from(path).exists()
}

/// archtype.conf 里的一项。
pub fn get(key: &str) -> String {
    get_key(&config_path(), key)
}

pub fn get_key(path: &str, key: &str) -> String {
    for line in read(path).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if let Some((name, value)) = trimmed.split_once('=') {
            if name.trim() == key {
                return value.trim().to_string();
            }
        }
    }
    String::new()
}

pub fn set(key: &str, value: &str) -> bool {
    set_key(&config_path(), key, value)
}

pub fn set_key(path: &str, key: &str, value: &str) -> bool {
    let text = read(path);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut found = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if let Some((name, _)) = trimmed.split_once('=') {
            if name.trim() == key {
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

pub fn classicui_get(key: &str) -> String {
    get_key(&classicui_path(), key)
}

pub fn classicui_set(key: &str, value: &str) -> bool {
    set_key(&classicui_path(), key, value)
}

pub fn state_get(key: &str) -> String {
    let text = read(&state_path());
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return String::new();
    };
    match &value[key] {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(flag) => flag.to_string(),
        _ => String::new(),
    }
}

pub fn state_set(key: &str, value: &str) -> bool {
    json_set(&state_path(), key, value)
}

pub fn json_set(path: &str, key: &str, value: &str) -> bool {
    let text = read(path);
    let mut object = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    object.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
    let Ok(text) = serde_json::to_string_pretty(&object) else {
        return false;
    };
    atomic_write(path, &text)
}

pub fn atomic_write(path: &str, text: &str) -> bool {
    let temporary = format!("{path}.tmp");
    if std::fs::write(&temporary, text).is_err() {
        return false;
    }
    if std::fs::rename(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return false;
    }
    true
}

pub fn ensure_parent(path: &str) -> bool {
    if let Some(parent) = std::path::Path::new(path).parent() {
        return std::fs::create_dir_all(parent).is_ok();
    }
    false
}
