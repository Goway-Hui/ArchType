//! 词库：下载万象词库并切换、或切回安装包自带的雾凇。

use crate::config;
use crate::net;
use std::process::Command;

const RELEASE_API: &str = "https://api.github.com/repos/Goway-Hui/ArchType/releases/latest";

fn wanxiang_dir() -> String {
    format!("{}/dicts/wanxiang", config::data_dir())
}

fn wanxiang_lexicon() -> String {
    format!("{}/lexicon.tsv", wanxiang_dir())
}

/// 万象词库包的版本（包里的 VERSION 文件），没装返回空串。
pub fn installed_version() -> String {
    config::read(&format!("{}/VERSION", wanxiang_dir()))
        .trim()
        .to_string()
}

/// 发布页上最新的万象词库包：(文件名, 下载地址)。
fn latest_asset() -> Result<(String, String), String> {
    let text = net::fetch(RELEASE_API)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "看不懂 GitHub 的返回".to_string())?;
    let assets = value["assets"]
        .as_array()
        .ok_or_else(|| "发布页上没有资产".to_string())?;
    for asset in assets {
        let Some(name) = asset["name"].as_str() else { continue };
        if name.starts_with("archtype-data-wanxiang-") && name.ends_with(".tar.gz") {
            let url = asset["browser_download_url"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if !url.is_empty() {
                return Ok((name.to_string(), url));
            }
        }
    }
    Err("最新版的发布页上还没有万象词库包".to_string())
}

fn sums_url() -> Result<String, String> {
    let text = net::fetch(RELEASE_API)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "看不懂 GitHub 的返回".to_string())?;
    value["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find_map(|asset| {
                (asset["name"].as_str() == Some("SHA256SUMS"))
                    .then(|| asset["browser_download_url"].as_str().map(str::to_string))
                    .flatten()
            })
        })
        .ok_or_else(|| "发布页缺少 SHA256SUMS".to_string())
}

/// 下载并安装万象词库，返回版本号。
pub fn install_wanxiang() -> Result<String, String> {
    let (name, url) = latest_asset()?;
    println!("下载 {name}");
    let work = format!("{}/updates/lexicon", config::data_dir());
    let _ = std::fs::create_dir_all(&work);
    let path = format!("{work}/{name}");
    net::download(&url, &path)?;
    let sums = net::fetch(&sums_url()?)?;
    let expected = net::expected_digest(&sums, &name)
        .ok_or_else(|| format!("SHA256SUMS 里没有 {name}"))?;
    let actual = net::sha256(&path)?;
    if actual != expected {
        let _ = std::fs::remove_file(&path);
        return Err(format!("{name} 校验和不对，可能没下完整，请重试"));
    }
    let version = install_pack(&path)?;
    let _ = std::fs::remove_file(&path);
    Ok(version)
}

/// 从 tar.gz 装上词库（只认可白名单里的文件），返回版本号。
pub fn install_pack(tarball: &str) -> Result<String, String> {
    let staging = format!("{}.staging", wanxiang_dir());
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let status = Command::new("tar")
        .args(["-xzf", tarball, "-C", &staging, "--strip-components=1"])
        .status()
        .map_err(|error| format!("跑不了 tar：{error}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("解压词库包失败".to_string());
    }
    for entry in std::fs::read_dir(&staging).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !matches!(
            name.as_str(),
            "lexicon.tsv" | "VERSION" | "LICENSE" | "SOURCE.txt"
        ) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("词库包里有不认识的文件 {name}，不安装"));
        }
    }
    if !std::path::Path::new(&format!("{staging}/lexicon.tsv")).is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("词库包里没有 lexicon.tsv，不安装".to_string());
    }
    let version = config::read(&format!("{staging}/VERSION")).trim().to_string();
    let target = wanxiang_dir();
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&staging, &target).map_err(|error| format!("换不到位置：{error}"))?;
    Ok(version)
}

fn reload() -> bool {
    Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.fcitx.Fcitx5",
            "--object-path",
            "/controller",
            "--method",
            "org.fcitx.Fcitx.Controller1.ReloadAddonConfig",
            "archtype",
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// `--use-lexicon rime-ice|wanxiang`
pub fn use_lexicon(kind: &str) -> i32 {
    match kind {
        "rime-ice" => {
            config::set("Lexicon", "");
        }
        "wanxiang" => {
            if installed_version().is_empty() {
                println!("还没装万象词库，先运行 archtype-config --install-lexicon wanxiang");
                return 1;
            }
            config::set("Lexicon", &wanxiang_lexicon());
        }
        _ => {
            println!("用法：archtype-config --use-lexicon rime-ice|wanxiang");
            return 2;
        }
    }
    let running = reload();
    println!(
        "已切到{}{}",
        if kind == "wanxiang" { "万象拼音" } else { "雾凇拼音（默认）" },
        if running {
            "，fcitx5 正在后台换词库，过几秒生效"
        } else {
            "，fcitx5 下次启动时生效"
        }
    );
    0
}

/// `--install-lexicon [wanxiang|文件.tar.gz]`
pub fn install_lexicon(source: &str) -> i32 {
    let result = if source == "wanxiang" || source.is_empty() {
        install_wanxiang()
    } else {
        install_pack(source)
    };
    let version = match result {
        Ok(version) => version,
        Err(error) => {
            println!("没装好：{error}");
            return 1;
        }
    };
    config::set("Lexicon", &wanxiang_lexicon());
    let running = reload();
    println!(
        "万象词库 {version} 装好了：{}",
        wanxiang_dir()
    );
    println!(
        "已切到万象拼音{}",
        if running {
            "，fcitx5 正在后台换词库，过几秒生效"
        } else {
            "，fcitx5 下次启动时生效"
        }
    );
    0
}
