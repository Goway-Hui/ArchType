//! fcitx5 的少量系统集成：重启、输入法列表（profile 与 D-Bus）、正在跑的插件版本。

use crate::config;
use std::process::Command;

const DEST: &str = "org.fcitx.Fcitx5";
const PATH: &str = "/controller";
const IFACE: &str = "org.fcitx.Fcitx.Controller1";

/// 调 fcitx5 的控制器；fcitx5 没在跑或方法不存在时返回 None。
fn call(method: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new("gdbus");
    command.args([
        "call",
        "--session",
        "--dest",
        DEST,
        "--object-path",
        PATH,
        "--method",
        &format!("{IFACE}.{method}"),
    ]);
    command.args(args);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 重启 fcitx5。KDE Wayland 下由 KWin 拉起时 D-Bus 的 Restart 不生效，交给 KWin。
pub fn restart() -> bool {
    call("Restart", &[]).is_some()
}

/// fcitx5 在跑并且认得 ArchType：`Some(true)`；在跑但不认得：`Some(false)`；没在跑：`None`。
pub fn knows_archtype() -> Option<bool> {
    let reply = call("AvailableInputMethods", &[])?;
    Some(reply.contains("'archtype'"))
}

/// 输入法分组的形状：(组名, 布局, [(输入法名, 布局)])。
pub type GroupInfo = (String, String, Vec<(String, String)>);

/// 当前输入法分组。
pub fn group_info(group: &str) -> Option<GroupInfo> {
    let reply = call("InputMethodGroupInfo", &[group])?;
    Some(parse_group_info(&reply))
}

/// `SetInputMethodGroupInfo` 的回复；失败返回 false。
pub fn set_group_info(group: &str, layout: &str, items: &[(String, String)]) -> bool {
    let list = items
        .iter()
        .map(|(name, layout)| format!("('{name}', '{layout}')"))
        .collect::<Vec<_>>()
        .join(", ");
    let argument = format!("('{group}', '{layout}', [{list}])");
    call("SetInputMethodGroupInfo", &[&argument]).is_some()
}

/// 当前输入法分组的名字。
pub fn current_group() -> Option<String> {
    let reply = call("CurrentInputMethodGroup", &[])?;
    parse_first_string(&reply)
}

/// 从 `('Default', 'us', [('keyboard-us', ''), ('archtype', '')])` 里取出组名、布局和条目。
fn parse_group_info(reply: &str) -> (String, String, Vec<(String, String)>) {
    let strings = all_strings(reply);
    let group = strings.first().cloned().unwrap_or_default();
    let layout = strings.get(1).cloned().unwrap_or_default();
    (group, layout, parse_items(reply))
}

/// 只取 `[` 与 `]` 之间的 `('name', 'layout')` 对。以前从 reply 开头找 `('`，命中的是外层
/// `('Default', 'us', ...)`，解析出脏数据，回写时 `SetInputMethodGroupInfo` 必然失败。
fn parse_items(reply: &str) -> Vec<(String, String)> {
    let Some(open) = reply.find('[') else { return Vec::new() };
    let Some(close) = reply[open..].find(']') else { return Vec::new() };
    let inner = &reply[open + 1..open + close];
    let mut items = Vec::new();
    let mut at = 0;
    while let Some(start) = inner[at..].find("('") {
        let start = at + start;
        let Some((name, after_name)) = read_quoted(inner, start + 1) else { break };
        // 跳过 `,` 与空白，读布局字符串
        let mut j = after_name;
        while inner.as_bytes().get(j).is_some_and(|b| *b == b' ' || *b == b',') {
            j += 1;
        }
        let Some((item_layout, after_layout)) = read_quoted(inner, j) else { break };
        if !name.is_empty() {
            items.push((name, item_layout));
        }
        at = after_layout;
    }
    items
}

/// 从 `at`（指向单引号）起读一个 GVariant 字符串，处理 `\\` 与 `\'` 转义。
fn read_quoted(s: &str, at: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.get(at) != Some(&b'\'') {
        return None;
    }
    let mut out = String::new();
    let mut i = at + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => return Some((out, i + 1)),
            b'\\' if i + 1 < bytes.len() => {
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            _ => {
                let ch = s[i..].chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

fn all_strings(reply: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = reply;
    while let Some(start) = rest.find('\'') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('\'') else { break };
        out.push(rest[..end].to_string());
        rest = &rest[end + 1..];
    }
    out
}

fn parse_first_string(reply: &str) -> Option<String> {
    all_strings(reply).into_iter().next()
}

/// 系统键盘布局名（`us`、`de-nodeadkeys`……），读不到用 `us`。
pub fn system_keyboard_layout() -> String {
    let output = Command::new("localectl").arg("status").output();
    let Ok(output) = output else {
        return "us".to_string();
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut layout = String::new();
    let mut variant = String::new();
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = value.trim().split(',').next().unwrap_or("").trim().to_string();
        match key {
            "X11 Layout" => layout = value,
            "X11 Variant" => variant = value,
            _ => {}
        }
    }
    if layout.is_empty() {
        layout = "us".to_string();
    }
    if variant.is_empty() {
        layout
    } else {
        format!("{layout}-{variant}")
    }
}

/// fcitx5 还没运行过：输入法列表和全局配置都不存在。
pub fn first_time() -> bool {
    !config::exists(&config::profile_path())
        && !config::exists(&format!("{}/fcitx5/config", config::config_home()))
}

/// 把 ArchType 加进 fcitx5 的 profile（fcitx5 没在跑时用）。
pub fn profile_add_archtype() -> Result<String, String> {
    let path = config::profile_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let layout = system_keyboard_layout();
            config::ensure_parent(&path);
            let body = format!(
                "[Groups/0]\nName=Default\nDefault Layout={layout}\nDefaultIM=archtype\n\n\
                 [Groups/0/Items/0]\nName=keyboard-{layout}\nLayout=\n\n\
                 [Groups/0/Items/1]\nName=archtype\nLayout=\n\n\
                 [GroupOrder]\n0=Default\n"
            );
            std::fs::write(&path, body).map_err(|error| format!("建不了 {path}：{error}"))?;
            return Ok(format!("新建了 fcitx5 输入法列表：键盘 {layout} + ArchType"));
        }
        Err(error) => return Err(format!("读不了 {path}：{error}")),
    };
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut current: Option<usize> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            sections.push((line[1..line.len() - 1].to_string(), Vec::new()));
            current = Some(sections.len() - 1);
        } else if let Some(index) = current {
            if !line.is_empty() && !line.starts_with('#') {
                if let Some((key, value)) = line.split_once('=') {
                    sections[index]
                        .1
                        .push((key.trim().to_string(), value.trim().to_string()));
                }
            }
        }
    }
    let lookup = |name: &str, key: &str| -> String {
        sections
            .iter()
            .find(|(section, _)| section == name)
            .and_then(|(_, entries)| entries.iter().find(|(k, _)| k == key))
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let first = lookup("GroupOrder", "0");
    if first.is_empty() {
        return Err(format!("看不懂 {path}，请在 fcitx5 配置里手动添加 ArchType"));
    }
    let index = sections
        .iter()
        .find_map(|(section, entries)| {
            let number = section.strip_prefix("Groups/")?;
            if number.chars().all(|c| c.is_ascii_digit())
                && entries
                    .iter()
                    .any(|(key, value)| key == "Name" && value == &first)
            {
                Some(number.to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| format!("看不懂 {path}，请在 fcitx5 配置里手动添加 ArchType"))?;
    let prefix = format!("Groups/{index}/Items/");
    let items: Vec<String> = sections
        .iter()
        .filter_map(|(section, entries)| {
            let number = section.strip_prefix(&prefix)?;
            if !number.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            entries
                .iter()
                .find(|(key, _)| key == "Name")
                .map(|(_, value)| value.clone())
        })
        .collect();
    if items.iter().any(|name| name == "archtype") {
        return Ok(format!("ArchType 已经在输入法列表「{first}」里"));
    }
    let backup = format!("{path}.bak-archtype");
    let _ = std::fs::write(&backup, &text);
    let mut updated = text.trim_end().to_string();
    updated.push_str(&format!(
        "\n\n[Groups/{index}/Items/{}]\nName=archtype\nLayout=\n",
        items.len()
    ));
    std::fs::write(&path, updated).map_err(|error| format!("写不进 {path}：{error}"))?;
    Ok(format!(
        "已把 ArchType 加进输入法列表「{first}」（原来的存成 profile.bak-archtype）"
    ))
}

/// 正在跑的插件版本：`None` 表示说不清，`Some("")` 是没有 Version 方法的旧插件。
pub fn running_addon_version() -> Option<String> {
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.archtype.Input",
            "--object-path",
            "/org/archtype/Input",
            "--method",
            "org.archtype.Input1.Version",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let old = error.contains("UnknownMethod")
            || error.contains("UnknownInterface")
            || error.contains("UnknownObject");
        return if old { Some(String::new()) } else { None };
    }
    parse_first_string(&String::from_utf8_lossy(&output.stdout))
}

/// 正在跑的插件和装好的不是同一版：要重启输入法。
pub fn restart_needed(running: &Option<String>, installed: &str) -> bool {
    match running {
        None => false,
        Some(version) => !installed.starts_with('@') && version != installed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_group_info_from_the_entry_list() {
        let reply = "('Default', 'us', [('keyboard-us', ''), ('archtype', '')])";
        let (group, layout, items) = parse_group_info(reply);
        assert_eq!(group, "Default");
        assert_eq!(layout, "us");
        assert_eq!(
            items,
            vec![
                ("keyboard-us".to_string(), String::new()),
                ("archtype".to_string(), String::new()),
            ]
        );
    }

    #[test]
    fn keeps_item_layouts_and_escapes() {
        let reply = "('Default', 'us', [('keyboard-us', 'us'), ('archtype', 'de-nodeadkeys')])";
        let (_, _, items) = parse_group_info(reply);
        assert_eq!(items[1], ("archtype".to_string(), "de-nodeadkeys".to_string()));

        let reply = "('Default', 'us', [('it\\'s', '')])";
        let (_, _, items) = parse_group_info(reply);
        assert_eq!(items[0].0, "it's");
    }

    #[test]
    fn missing_entry_list_yields_no_items() {
        let (group, layout, items) = parse_group_info("('Default', 'us', [])");
        assert_eq!(group, "Default");
        assert_eq!(layout, "us");
        assert!(items.is_empty());
    }
}
