//! 检查更新和装新版：下载、校验、SHA256SUMS 都复用 `archtype-net`（和 Linux 命令行同一套）。
//!
//! 检查版本走网页（releases/latest 会跳到最新 tag 页，不算 GitHub 接口次数），
//! 和 Linux 的 `archtype-config`、设置界面一样。

use archtype_net::{download, expected_digest, fetch, latest_release_tag, sha256};

/// 新版本的 tag（不带 v），比如 `0.8.1`。
pub fn latest_version() -> Option<String> {
    latest_release_tag()
}

/// `latest` 比 `current` 新吗。两边都可以带 v。不是版本号（dev、空）时不算新。
pub fn is_newer(latest: &str, current: &str) -> bool {
    let Some(latest) = parse_version(latest) else {
        return false;
    };
    let Some(current) = parse_version(current) else {
        return false;
    };
    latest > current
}

fn parse_version(text: &str) -> Option<Vec<u64>> {
    let text = text.trim().trim_start_matches('v');
    if text.is_empty() || text == "dev" {
        return None;
    }
    let mut parts = Vec::new();
    for part in text.split('.') {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        parts.push(digits.parse().ok()?);
    }
    Some(parts)
}

/// 下载一个发布资产并按 SHA256SUMS 校验，返回本地路径。安装动作由平台做。
///
/// `base_url` 一般是 `https://github.com/<owner>/<repo>/releases/download/v<版本>`。
pub fn download_verified(
    name: &str,
    base_url: &str,
    dir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let sums = fetch(&format!("{base_url}/SHA256SUMS"))?;
    let expected =
        expected_digest(&sums, name).ok_or_else(|| format!("SHA256SUMS 里没有 {name}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("建目录失败：{e}"))?;
    let path = dir.join(name);
    download(&format!("{base_url}/{name}"), &path.to_string_lossy())?;
    let actual = sha256(&path.to_string_lossy())?;
    if actual.to_lowercase() != expected.to_lowercase() {
        let _ = std::fs::remove_file(&path);
        return Err(format!("{name} 校验和对不上，请重新下载"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.8.1", "0.8.0"));
        assert!(is_newer("v0.9.0", "0.8.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.8.0", "0.8.0"));
        assert!(!is_newer("0.7.9", "0.8.0"));
        assert!(!is_newer("dev", "0.8.0"));
        assert!(!is_newer("", "0.8.0"));
        assert!(!is_newer("0.8.0", "dev"));
        // 带后缀的 tag（0.8.1-beta1）按数字段比
        assert!(is_newer("0.8.1-beta1", "0.8.0"));
    }

}
