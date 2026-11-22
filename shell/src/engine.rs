//! 引擎和文件路径：词库、语言模型、用户词库、输入习惯、自定义词、整句增强模型。
//!
//! 平台无关：Linux 上也能编、能测（macOS 层把 NSBundle 里的资源目录传进来）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use archtype_core::lm::{load_lm, BigramLm, LanguageModel};
use archtype_core::{Engine, Lexicon, MemoryLexicon};

use crate::config::Config;

/// 进程里共用的一份引擎：外壳在启动时（后台线程）载入，之后所有输入框共用。
///
/// 指针来自 `Box::into_raw`，永不释放（进程活多久它活多久），所以借出来的引用是 `'static`。
/// 输入法进程里所有改动都在主线程上串行做，没有并发访问。
#[derive(Clone, Copy)]
pub struct EngineStore {
    ptr: *mut Engine,
}

// SAFETY: EngineStore 只在输入法进程里主线程上读写（配置变了、载入学习数据）；
// 后台线程只把它放进 OnceLock，之后不再碰引擎。
unsafe impl Send for EngineStore {}
unsafe impl Sync for EngineStore {}

impl EngineStore {
    pub fn new(engine: Engine) -> Self {
        Self {
            ptr: Box::into_raw(Box::new(engine)),
        }
    }

    pub fn get(&self) -> &'static Engine {
        // SAFETY: 指针来自 Box::into_raw，没释放过（见类型文档）。
        unsafe { &*self.ptr }
    }

    pub fn get_mut(&self) -> &'static mut Engine {
        // SAFETY: 同 `get`，调用方保证主线程串行访问。
        unsafe { &mut *self.ptr }
    }
}

/// 安装包里的数据和用户数据在哪儿。
#[derive(Clone, Debug)]
pub struct Layout {
    /// 安装包里的数据（词库、英文词表、语言模型、五笔码表、简繁表）。macOS 上是
    /// `ArchType.app/Contents/Resources`（由 NSBundle 给出）。
    pub resource_dir: PathBuf,
    /// 用户数据：`~/Library/Application Support/ArchType`（`ARCHTYPE_DATA_DIR` 可以换，
    /// 自检和测试用）。
    pub support_dir: PathBuf,
}

impl Layout {
    pub fn new(resource_dir: impl Into<PathBuf>, support_dir: impl Into<PathBuf>) -> Self {
        Self {
            resource_dir: resource_dir.into(),
            support_dir: support_dir.into(),
        }
    }

    pub fn ensure_support_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.support_dir)
    }

    pub fn resource(&self, name: &str) -> Option<PathBuf> {
        let path = self.resource_dir.join(name);
        path.is_file().then_some(path)
    }

    pub fn config_path(&self) -> PathBuf {
        self.support_dir.join("archtype.conf")
    }

    pub fn user_dict_path(&self) -> PathBuf {
        self.support_dir.join("user.tsv")
    }

    pub fn history_path(&self) -> PathBuf {
        self.support_dir.join("history.tsv")
    }

    pub fn custom_words_path(&self) -> PathBuf {
        self.support_dir.join("custom.tsv")
    }

    /// 用户导入的整句增强模型（微信输入法的 nlrgpt2_nnlm.bin）
    pub fn imported_nnlm_path(&self) -> PathBuf {
        self.support_dir.join("wetype").join("nlrgpt2_nnlm.bin")
    }

    /// 实际用哪个词库：设置里的 Lexicon（文件在才用）> 安装包里的雾凇词库。
    ///
    /// 安装包里的词库旁边会写一份快照（`.wcache`，下次启动快很多）；安装包是签过名的，
    /// 不能往里写，所以在数据目录放一个指向安装包里词库的链接，快照就落在数据目录。
    pub fn lexicon_path(&self, config: &Config) -> Option<PathBuf> {
        if !config.lexicon.is_empty() {
            let configured = expand_tilde(&config.lexicon);
            if configured.is_file() {
                return Some(configured);
            }
        }
        let bundled = self.resource_dir.join("lexicon.tsv");
        if !bundled.is_file() {
            return None;
        }
        if self.ensure_support_dir().is_err() {
            return Some(bundled);
        }
        let link = self.support_dir.join("lexicon.tsv");
        match std::fs::read_link(&link) {
            Ok(target) if target == bundled => Some(link),
            _ => {
                let _ = std::fs::remove_file(&link);
                match std::os::unix::fs::symlink(&bundled, &link) {
                    Ok(()) => Some(link),
                    Err(_) => Some(bundled),
                }
            }
        }
    }

    /// 整句增强模型：设置窗口导入的微信输入法模型优先，没有再用安装包里的 nnlm.bin
    /// （ArchType 自己训的，随包带上）；都没有是 None。
    pub fn nnlm_path(&self) -> Option<PathBuf> {
        let imported = self.imported_nnlm_path();
        if imported.is_file() {
            return Some(imported);
        }
        self.resource("nnlm.bin")
    }
}

/// 载入引擎：词库、语言模型、英文词表、领域词级模型、整句增强模型、用户词库、输入习惯、自定义词。
///
/// 返回的错误是一整句话，外壳记日志用。词库读不进来才算失败；用户词库、输入习惯、
/// 自定义词缺失是正常的（第一次用还没有）。
pub fn load_engine(layout: &Layout, config: &Config) -> Result<Engine, String> {
    let _ = layout.ensure_support_dir();
    let lexicon_path = layout
        .lexicon_path(config)
        .ok_or_else(|| "安装包里没有 lexicon.tsv".to_string())?;
    let lexicon = MemoryLexicon::from_tsv_path_cached(&lexicon_path)
        .map_err(|e| format!("载入词库失败 {}：{e:#}", lexicon_path.display()))?;
    let lexicon: Arc<dyn Lexicon> = Arc::new(lexicon);

    let lm: Arc<dyn LanguageModel> = match layout.resource("words.lm") {
        Some(path) => match load_lm(&path) {
            Ok(lm) => lm,
            Err(e) => {
                eprintln!(
                    "ArchType: 语言模型载入失败 {}：{e:#}，按默认权重排序",
                    path.display()
                );
                Arc::new(BigramLm::default())
            }
        },
        None => Arc::new(BigramLm::default()),
    };

    let mut engine = Engine::new(lexicon, lm);
    // 模糊音索引（兜底默认开）也在这时建好，不留到第一次按键
    engine.prepare_fuzzy();

    if let Some(english) = layout.resource("english.txt") {
        let _ = engine.load_english(&english);
    }

    // 用户自己训的领域词级模型（可选），和 words.lm 混着用
    let domain = layout.support_dir.join("words.domain.lm");
    if domain.is_file() {
        match load_lm(&domain) {
            Ok(model) => engine.set_domain_lm(Some(model)),
            Err(e) => eprintln!("ArchType: 领域词级模型载入失败 {}：{e:#}", domain.display()),
        }
    }

    if let Some(nnlm) = layout.nnlm_path() {
        match engine.load_nnlm(&nnlm) {
            Ok(vocab) => {
                if vocab > 0 {
                    eprintln!(
                        "ArchType: 载入整句增强模型 {}（字表 {vocab}）",
                        nnlm.display()
                    );
                }
            }
            Err(e) => eprintln!("ArchType: 整句增强模型载入失败 {}：{e:#}", nnlm.display()),
        }
    }

    let _ = engine.load_user_dict(layout.user_dict_path());
    let _ = engine.load_history(layout.history_path());
    let _ = engine.load_custom_words(layout.custom_words_path());
    Ok(engine)
}

/// 设置里管引擎的部分（模糊音、纠错、双拼、生僻字、繁体、五笔码表），和 Linux 的 applyCoreSettings 一样。
///
/// 返回要记日志的问题（不认识的双拼方案、简繁表或五笔码表载入失败）。
pub fn apply_engine_settings(engine: &mut Engine, config: &Config, layout: &Layout) -> Vec<String> {
    let mut problems = Vec::new();
    engine.set_fuzzy(config.fuzzy);
    engine.set_fuzzy_groups(config.fuzzy_groups());
    engine.set_shuangpin(config.shuangpin());
    match archtype_core::shuangpin::Scheme::from_name(&config.shuangpin_scheme) {
        Some(scheme) => engine.set_shuangpin_scheme(scheme),
        None => {
            problems.push(format!(
                "不认识双拼方案 {}，用自然码",
                config.shuangpin_scheme
            ));
            engine.set_shuangpin_scheme(archtype_core::shuangpin::Scheme::Ziranma);
        }
    }
    engine.set_shuangpin_abbrev(config.shuangpin_abbrev);
    engine.set_typo_correction(config.typo_correction);
    engine.set_rare_ext_chars(config.rare_ext_chars);

    let mut conversion_loaded = engine.conversion_len() > 0;
    if config.traditional && !conversion_loaded {
        match layout.resource("s2t.tsv") {
            Some(path) => match engine.load_conversion(&path) {
                Ok(len) if len > 0 => conversion_loaded = true,
                Ok(_) => problems.push(format!("简繁表是空的 {}", path.display())),
                Err(e) => problems.push(format!("简繁表载入失败 {}：{e:#}", path.display())),
            },
            None => problems.push("简繁表载入失败（没有 s2t.tsv）".to_string()),
        }
    }
    engine.set_traditional(config.traditional && conversion_loaded);

    if config.wubi() {
        match layout.resource("wubi86.tsv") {
            Some(path) => {
                if let Err(e) = engine.load_wubi(&path) {
                    problems.push(format!(
                        "五笔码表载入失败 {}：{e:#}，先按全拼打",
                        path.display()
                    ));
                }
            }
            None => problems.push("五笔码表载入失败（没有 wubi86.tsv），先按全拼打".to_string()),
        }
    }
    problems
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        return Path::new(&home).join(rest);
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("archtype-macos-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const LEXICON: &str = "ni hao\t你好\t900000\nhao\t好\t8000\nshi jian\t时间\t7000\n";

    #[test]
    fn loads_a_lexicon_and_applies_settings() {
        let resource = temp_dir("resource");
        let support = temp_dir("support");
        std::fs::write(resource.join("lexicon.tsv"), LEXICON).unwrap();
        let layout = Layout::new(&resource, &support);
        let config = Config::default();
        let mut engine = load_engine(&layout, &config).expect("载入引擎");

        assert!(layout.support_dir.join("lexicon.tsv").is_symlink());

        let problems = apply_engine_settings(&mut engine, &config, &layout);
        assert!(problems.is_empty(), "{problems:?}");
        let _ = std::fs::remove_dir_all(&resource);
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn configured_lexicon_wins_when_it_exists() {
        let resource = temp_dir("resource2");
        let support = temp_dir("support2");
        std::fs::write(resource.join("lexicon.tsv"), LEXICON).unwrap();
        let custom = support.join("my-lexicon.tsv");
        std::fs::write(&custom, "ni hao\t拟好\t1\n").unwrap();
        let mut config = Config::default();
        config.lexicon = custom.to_string_lossy().to_string();
        let layout = Layout::new(&resource, &support);
        assert!(load_engine(&layout, &config).is_ok());
        let _ = std::fs::remove_dir_all(&resource);
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn missing_lexicon_is_an_error() {
        let resource = temp_dir("resource3");
        let support = temp_dir("support3");
        let layout = Layout::new(&resource, &support);
        assert!(load_engine(&layout, &Config::default()).is_err());
        let _ = std::fs::remove_dir_all(&resource);
        let _ = std::fs::remove_dir_all(&support);
    }
}
