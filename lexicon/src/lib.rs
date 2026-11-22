//! 词库：下载万象词库并切换、或切回安装包自带的雾凇。
//!
//! 命令行、设置界面和 macOS 外壳共用这一份；路径和配置读写由调用方通过 [`Env`] 提供。

use archtype_net as net;
use std::process::Command;

const RELEASE_API: &str = "https://api.github.com/repos/Goway-Hui/ArchType/releases/latest";

/// 外壳提供的路径与配置读写（Linux 是 XDG + fcitx5 配置，macOS 是数据目录 + archtype.conf）。
pub trait Env {
    /// 用户数据目录（词库、模型、自定义词）。
    fn data_dir(&self) -> String;
    /// 读 archtype.conf 的一项。
    fn get(&self, key: &str) -> String;
    /// 写 archtype.conf 的一项。
    fn set(&self, key: &str, value: &str) -> bool;
    /// 让正在跑的输入法重新读配置；没在跑时返回 false。
    fn reload(&self) -> bool;
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn wanxiang_dir(env: &impl Env) -> String {
    format!("{}/dicts/wanxiang", env.data_dir())
}

fn wanxiang_lexicon(env: &impl Env) -> String {
    format!("{}/lexicon.tsv", wanxiang_dir(env))
}

/// 万象词库包的版本（包里的 VERSION 文件），没装返回空串。
pub fn installed_version(env: &impl Env) -> String {
    read(&format!("{}/VERSION", wanxiang_dir(env)))
        .trim()
        .to_string()
}

/// 文件名 -> 版本（不是词库包或版本不像样时 None）。
fn pack_version(name: &str) -> Option<String> {
    const PREFIX: &str = "archtype-data-wanxiang-";
    const SUFFIX: &str = ".tar.gz";
    let rest = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    let mut chars = rest.chars();
    let first = chars.next()?;
    // spec：版本整个符合 [0-9][0-9A-Za-z.]*
    if !first.is_ascii_digit() || !chars.all(|c| c.is_ascii_alphanumeric() || c == '.') {
        return None;
    }
    Some(rest.to_string())
}

/// 版本按数字段比大小（同 spec 的 pickAsset）。
fn version_key(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|segment| {
            segment
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// 读一次发布页（资产列表与下载地址都用它，别每次安装请求两遍）。
fn release() -> Result<serde_json::Value, String> {
    let text = net::fetch(RELEASE_API)?;
    serde_json::from_str(&text).map_err(|_| "看不懂 GitHub 的返回".to_string())
}

/// 发布页上最新的万象词库包：(文件名, 下载地址)。按 spec 取版本最大的，不是第一个匹配的。
fn latest_asset(value: &serde_json::Value) -> Result<(String, String), String> {
    let assets = value["assets"]
        .as_array()
        .ok_or_else(|| "发布页上没有资产".to_string())?;
    let mut best: Option<(Vec<u64>, String, String)> = None;
    for asset in assets {
        let Some(name) = asset["name"].as_str() else { continue };
        let Some(version) = pack_version(name) else { continue };
        let url = asset["browser_download_url"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            continue;
        }
        let key = version_key(&version);
        if best.as_ref().is_none_or(|(current, _, _)| key > *current) {
            best = Some((key, name.to_string(), url));
        }
    }
    best.map(|(_, name, url)| (name, url))
        .ok_or_else(|| "最新版的发布页上还没有万象词库包".to_string())
}

fn sums_url(value: &serde_json::Value) -> Result<String, String> {
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
pub fn install_wanxiang(env: &impl Env) -> Result<String, String> {
    let release = release()?;
    let (name, url) = latest_asset(&release)?;
    println!("下载 {name}");
    let work = format!("{}/updates/lexicon", env.data_dir());
    let _ = std::fs::create_dir_all(&work);
    let path = format!("{work}/{name}");
    net::download(&url, &path)?;
    let sums = net::fetch(&sums_url(&release)?)?;
    let expected = net::expected_digest(&sums, &name)
        .ok_or_else(|| format!("SHA256SUMS 里没有 {name}"))?;
    let actual = net::sha256(&path)?;
    if actual != expected {
        let _ = std::fs::remove_file(&path);
        return Err(format!("{name} 校验和不对，可能没下完整，请重试"));
    }
    let version = install_pack(env, &path)?;
    let _ = std::fs::remove_file(&path);
    Ok(version)
}

/// 当前用的是不是万象词库（看 archtype.conf 的 Lexicon 指向）。
pub fn using_wanxiang(env: &impl Env) -> bool {
    env.get("Lexicon") == wanxiang_lexicon(env)
}

/// 后台自动更新正在用的万象词库：发布页上有比本地 VERSION 新的包就换上。没在用万象时什么都不做。
pub fn auto_update(env: &impl Env) -> Result<bool, String> {
    if !using_wanxiang(env) {
        return Ok(false);
    }
    let installed = installed_version(env);
    if installed.is_empty() {
        return Ok(false);
    }
    let release = release()?;
    let (name, _url) = latest_asset(&release)?;
    let Some(latest) = pack_version(&name) else {
        return Ok(false);
    };
    if version_key(&latest) <= version_key(&installed) {
        return Ok(false);
    }
    install_wanxiang(env)?;
    Ok(true)
}

/// 列出 tar 成员（解包前校验用）。
fn tar_members(tarball: &str) -> Result<Vec<String>, String> {
    let output = Command::new("tar")
        .args(["-tzf", tarball])
        .output()
        .map_err(|error| format!("跑不了 tar：{error}"))?;
    if !output.status.success() {
        return Err("读不了词库包".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim_end_matches('/').to_string())
        .filter(|line| !line.is_empty() && line != ".")
        .collect())
}

/// 从 tar.gz 装上词库（只认可白名单里的文件），返回版本号。
pub fn install_pack(env: &impl Env, tarball: &str) -> Result<String, String> {
    const PACK_FILES: [&str; 4] = ["lexicon.tsv", "LICENSE", "SOURCE.txt", "VERSION"];
    let staging = format!("{}.staging", wanxiang_dir(env));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    // 先看成员再解包：`../`、绝对路径、链接在 read_dir 校验之前就会写到 staging 外面
    for name in tar_members(tarball)? {
        let normalized = name.trim_start_matches("./");
        if !PACK_FILES.contains(&normalized) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("词库包里有不安全的文件 {name}，不安装"));
        }
    }
    let status = Command::new("tar")
        .args(["-xzf", tarball, "-C", &staging])
        .status()
        .map_err(|error| format!("跑不了 tar：{error}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("解压词库包失败".to_string());
    }
    for entry in std::fs::read_dir(&staging).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !PACK_FILES.contains(&name.as_str()) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("词库包里有不认识的文件 {name}，不安装"));
        }
    }
    if !std::path::Path::new(&format!("{staging}/lexicon.tsv")).is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("词库包里没有 lexicon.tsv，不安装".to_string());
    }
    let version = read(&format!("{staging}/VERSION")).trim().to_string();
    // spec：VERSION 缺失或为空算「?」，有 lexicon.tsv 就算装好
    let version = if version.is_empty() {
        "?".to_string()
    } else {
        version
    };
    let target = wanxiang_dir(env);
    if let Some(parent) = std::path::Path::new(&target).parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    // 先把旧目录挪到 .old，新目录进去以后才删：中途失败能回滚，不会两个都没有
    let old = format!("{target}.old");
    let _ = std::fs::remove_dir_all(&old);
    if std::path::Path::new(&target).exists() {
        std::fs::rename(&target, &old).map_err(|error| format!("挪不开旧词库：{error}"))?;
    }
    if let Err(error) = std::fs::rename(&staging, &target) {
        if std::path::Path::new(&old).exists() {
            let _ = std::fs::rename(&old, &target);
        }
        return Err(format!("换不到位置：{error}"));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(version)
}

/// `--use-lexicon rime-ice|wanxiang`
pub fn use_lexicon(env: &impl Env, kind: &str) -> i32 {
    match kind {
        "rime-ice" => {
            env.set("Lexicon", "");
        }
        "wanxiang" => {
            if installed_version(env).is_empty() {
                println!("还没装万象词库，先运行 --install-lexicon wanxiang");
                return 1;
            }
            env.set("Lexicon", &wanxiang_lexicon(env));
        }
        _ => {
            println!("用法：--use-lexicon rime-ice|wanxiang");
            return 2;
        }
    }
    let running = env.reload();
    println!(
        "已切到{}{}",
        if kind == "wanxiang" { "万象拼音" } else { "雾凇拼音（默认）" },
        if running {
            "，输入法正在后台换词库，过几秒生效"
        } else {
            "，输入法下次启动时生效"
        }
    );
    0
}

/// `--install-lexicon [wanxiang|文件.tar.gz]`
pub fn install_lexicon(env: &impl Env, source: &str) -> i32 {
    let result = if source == "wanxiang" || source.is_empty() {
        install_wanxiang(env)
    } else {
        install_pack(env, source)
    };
    let version = match result {
        Ok(version) => version,
        Err(error) => {
            println!("没装好：{error}");
            return 1;
        }
    };
    env.set("Lexicon", &wanxiang_lexicon(env));
    let running = env.reload();
    println!("万象词库 {version} 装好了：{}", wanxiang_dir(env));
    println!(
        "已切到万象拼音{}",
        if running {
            "，输入法正在后台换词库，过几秒生效"
        } else {
            "，输入法下次启动时生效"
        }
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_versions_and_ordering() {
        assert_eq!(
            pack_version("archtype-data-wanxiang-2026.1.1.1.tar.gz").as_deref(),
            Some("2026.1.1.1")
        );
        assert_eq!(pack_version("archtype-data-rime-ice-2026.1.tar.gz"), None);
        assert_eq!(pack_version("archtype-data-wanxiang-x.tar.gz"), None);
        assert!(version_key("2026.1.1.2") > version_key("2026.1.1.1"));
        assert!(version_key("2026.2") > version_key("2026.1.9"));
    }
}
