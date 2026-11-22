//! 进程里只有一份的东西：设置、路径、引擎（后台载入）、数据目录。

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::SystemTime;

use objc2_foundation::{ns_string, NSBundle, NSString};

use crate::config::Config;
use crate::engine::{load_engine, EngineStore, Layout};
use crate::stats::Stats;
use crate::translate::{translate_config, TranslatorStore};

pub struct Shared {
    pub layout: Layout,
    config: Mutex<Config>,
    config_stamp: Mutex<Option<SystemTime>>,
    engine: OnceLock<EngineStore>,
    /// 进程里共用的翻译器（本地 llama-server 也只起一个）
    translator: TranslatorStore,
    /// 打字统计（进程共用；统计关着时它自己什么都不记）
    stats: Stats,
    /// 载入失败的原因（菜单里显示、自检打印）
    engine_error: Mutex<Option<String>>,
}

static SHARED: LazyLock<Shared> = LazyLock::new(Shared::new);

/// 安装包里的数据目录：`ArchType.app/Contents/Resources`。
pub fn resource_dir() -> PathBuf {
    let bundle = NSBundle::mainBundle();
    if let Some(path) = bundle.resourcePath() {
        return PathBuf::from(path.to_string());
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .map(|p| p.join("../Resources"))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 进程启动时调一次：读设置，后台载入引擎。
pub fn start() -> &'static Shared {
    let shared = &SHARED;
    shared.start_loading();
    shared
}

impl Shared {
    fn new() -> Self {
        let layout = Layout::new(resource_dir(), Layout::support_dir_standard());
        let config = Config::load(&layout.config_path().to_string_lossy());
        let stamp = modified(&layout.config_path());
        let translator = TranslatorStore::new();
        translator
            .get()
            .configure(translate_config(&config, &layout));
        Self {
            layout,
            config: Mutex::new(config),
            config_stamp: Mutex::new(stamp),
            engine: OnceLock::new(),
            translator,
            stats,
            engine_error: Mutex::new(None),
        }
    }

    fn start_loading(&self) {
        if self.engine.get().is_some() || self.engine_error.lock().unwrap().is_some() {
            return;
        }
        let layout = self.layout.clone();
        let config = self.config();
        std::thread::spawn(move || match load_engine(&layout, &config) {
            Ok(engine) => {
                let _ = SHARED.engine.set(EngineStore::new(engine));
            }
            Err(error) => {
                eprintln!("ArchType: {error}");
                *SHARED.engine_error.lock().unwrap() = Some(error);
            }
        });
    }

    pub fn engine(&self) -> Option<EngineStore> {
        self.engine.get().copied()
    }

    pub fn translator(&self) -> &'static crate::translate::Translator {
        self.translator.get()
    }

    /// 打字统计（统计关着也建，开着时才开始记）。
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn engine_error(&self) -> Option<String> {
        self.engine_error.lock().unwrap().clone()
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    /// 配置文件变了就重读；返回新的设置（没变返回 None）。
    pub fn reload_config_if_changed(&self) -> Option<Config> {
        let path = self.layout.config_path();
        let stamp = modified(&path);
        let mut known = self.config_stamp.lock().unwrap();
        if *known == stamp {
            return None;
        }
        *known = stamp;
        let config = Config::load(&path.to_string_lossy());
        self.translator
            .get()
            .configure(crate::translate::translate_config(&config, &self.layout));
        self.stats().set_enabled(config.typing_stats);
        *self.config.lock().unwrap() = config.clone();
        Some(config)
    }

    /// 写回设置（设置窗口改的是磁盘上的文件；外壳这边也跟上）。
    pub fn store_config(&self, config: Config) {
        let path = self.layout.config_path();
        let stamp = modified(&path);
        *self.config_stamp.lock().unwrap() = stamp;
        *self.config.lock().unwrap() = config;
    }
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// 输入法进程的名字要和 Info.plist 的 InputMethodConnectionName 一致；bundle id 也用它。
pub const CONNECTION_NAME: &str = "ArchType_Connection";
pub const BUNDLE_ID: &str = "io.github.gloridust.inputmethod.ArchType";

/// `ArchType --version` 用的版本号（Info.plist 的 CFBundleShortVersionString）。
pub fn version() -> String {
    let key = ns_string!("CFBundleShortVersionString");
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(key)
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}
