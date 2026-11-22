//! macOS 的更新：下载 pkg（下载与校验的共用逻辑在 `archtype_shell::update`），
//! 再用系统的 `installer` 装给当前用户（不要管理员密码）。

use archtype_shell::engine::Layout;

/// 这一版 macOS 安装包的文件名。
pub fn package_name(version: &str) -> String {
    format!("ArchType-{}-macos.pkg", version.trim_start_matches('v'))
}

/// 下载、校验并安装（只装给当前用户）。
///
/// `base_url` 一般是 `https://github.com/Goway-Hui/ArchType/releases/download/v<版本>`。
pub fn install(version: &str, base_url: &str, layout: &Layout) -> Result<(), String> {
    let name = package_name(version);
    let dir = layout.support_dir.join("updates").join(version);
    let path = archtype_shell::update::download_verified(&name, base_url, &dir)?;
    let status = std::process::Command::new("/usr/sbin/installer")
        .arg("-pkg")
        .arg(&path)
        .arg("-target")
        .arg("CurrentUserHomeDirectory")
        .status()
        .map_err(|e| format!("跑不了 installer：{e}"))?;
    if !status.success() {
        return Err("安装失败（installer 返回非 0）".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_name_has_no_v() {
        assert_eq!(package_name("v0.8.1"), "ArchType-0.8.1-macos.pkg");
        assert_eq!(package_name("0.8.1"), "ArchType-0.8.1-macos.pkg");
    }
}
