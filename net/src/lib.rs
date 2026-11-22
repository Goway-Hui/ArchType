//! 最小的下载与校验工具（curl + SHA256SUMS）：命令行、macOS 外壳和设置界面共用。

use sha2::{Digest, Sha256};
use std::process::Command;

/// 下载到 path（先写 .part）。失败返回原因。
pub fn download(url: &str, path: &str) -> Result<(), String> {
    let part = format!("{path}.part");
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let status = Command::new("curl")
        .args([
            "-sSL",
            "--fail",
            "--max-time",
            "1800",
            "-A",
            &format!("ArchType/{}", env!("CARGO_PKG_VERSION")),
            "-o",
            &part,
            url,
        ])
        .status()
        .map_err(|error| format!("跑不了 curl：{error}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err("下载失败".to_string());
    }
    std::fs::rename(&part, path).map_err(|error| format!("换不到位置：{error}"))
}

/// 取 URL 的文本内容。
pub fn fetch(url: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args([
            "-sSL",
            "--fail",
            "--max-time",
            "20",
            "-A",
            &format!("ArchType/{}", env!("CARGO_PKG_VERSION")),
            url,
        ])
        .output()
        .map_err(|error| format!("跑不了 curl：{error}"))?;
    if !output.status.success() {
        return Err("联网失败".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// 最新发布版的版本号（不带 v），只用网页：`releases/latest` 会跳到 `/releases/tag/vX.Y.Z`。
/// 不查 GitHub 接口，接口限流（403/429）也不影响检查更新。
/// 设置界面有一份同样的解析（settings-nsh/src/lib.rs 的 latest_tag_via_web），改这里时两处一起改。
pub fn latest_release_tag() -> Option<String> {
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L",
            "--max-time",
            "20",
            "-A",
            &format!("ArchType/{}", env!("CARGO_PKG_VERSION")),
            "-o",
            "/dev/null",
            "-w",
            "%{url_effective}",
            "https://github.com/Goway-Hui/ArchType/releases/latest",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let effective = String::from_utf8_lossy(&output.stdout);
    let (_, tail) = effective.rsplit_once("/tag/")?;
    let tag = tail.trim().trim_start_matches('v');
    if tag.is_empty() || !tag.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(tag.to_string())
}

/// 文件里某一行的 SHA256（`<摘要>  <文件名>`）。
pub fn expected_digest(sums: &str, name: &str) -> Option<String> {
    for line in sums.lines() {
        let mut fields = line.split_whitespace();
        // 空行、注释行、字段不全的行直接跳过；以前用 `?` 会让整个查找提前返回 None
        let (Some(digest), Some(file)) = (fields.next(), fields.next()) else {
            continue;
        };
        let file = file.trim_start_matches('*');
        if file == name && digest.len() == 64 {
            return Some(digest.to_lowercase());
        }
    }
    None
}

pub fn sha256(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// 像 `12.3 MB`。
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_skips_blank_and_malformed_lines() {
        let sums = "\n# comment\nabc\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  pkg.deb\n";
        assert_eq!(
            expected_digest(sums, "pkg.deb"),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string())
        );
        assert_eq!(expected_digest("", "pkg.deb"), None);
    }

    #[test]
    fn digest_accepts_binary_marker() {
        let sums = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef *pkg.deb\n";
        assert!(expected_digest(sums, "pkg.deb").is_some());
    }
}
