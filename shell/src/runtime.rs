//! 进程级共用状态：配置热重载、后台载引擎、引擎/翻译器/统计单例、退出时保存。
//!
//! 平台外壳只提供资源目录和数据目录（`Layout`），其余逻辑 macOS 与 Windows 共用。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use archtype_core::translate::Translator;

use crate::config::Config;
use crate::engine::{load_engine, EngineStore, Layout};
use crate::stats::Stats;
use crate::translate::{translate_config, TranslatorStore};

/// 收到 SIGTERM / 关机一类的终止请求：主线程看到后保存再退。
static TERMINATE: AtomicBool = AtomicBool::new(false);

pub fn request_terminate() {
    TERMINATE.store(true, Ordering::SeqCst);
}

pub fn terminate_requested() -> bool {
    TERMINATE.load(Ordering::SeqCst)
}

pub struct Runtime {
    pub layout: Layout,
    config: Mutex<Config>,
    config_stamp: Mutex<Option<SystemTime>>,
    engine: OnceLock<EngineStore>,
    translator: TranslatorStore,
    stats: Stats,
    engine_error: Mutex<Option<String>>,
}

impl Runtime {
    pub fn new(layout: Layout) -> Self {
        let config = Config::load(&layout.config_path().to_string_lossy());
        let stamp = modified(&layout.config_path());
        let translator = TranslatorStore::new();
        translator.get().configure(translate_config(&config, &layout));
        let stats = Stats::new(
            &crate::stats::stats_path(&layout.support_dir),
            config.typing_stats,
        )
        .expect("建统计记录器");
        archtype_core::recent_emoji::set_path(
            &layout
                .support_dir
                .join("emoji-recent.txt")
                .to_string_lossy(),
        );
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

    /// 后台载入词库和引擎；载入完成前输入控制器不接按键。
    pub fn start_loading(&'static self) {
        if self.engine.get().is_some() || self.engine_error.lock().unwrap().is_some() {
            return;
        }
        let layout = self.layout.clone();
        let config = self.config();
        std::thread::spawn(move || match load_engine(&layout, &config) {
            Ok(engine) => {
                let _ = self.engine.set(EngineStore::new(engine));
            }
            Err(error) => {
                eprintln!("ArchType: {error}");
                *self.engine_error.lock().unwrap() = Some(error);
            }
        });
    }

    pub fn engine(&self) -> Option<EngineStore> {
        self.engine.get().copied()
    }

    pub fn translator(&self) -> &'static Translator {
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
            .configure(translate_config(&config, &self.layout));
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

/// 退出前把学习数据交出去（系统可能随时杀掉输入法进程）。
pub fn shutdown(runtime: &Runtime) {
    if let Some(engine) = runtime.engine() {
        let engine = engine.get();
        let _ = engine.save_user_dict_async(runtime.layout.user_dict_path());
        let _ = engine.save_history_async(runtime.layout.history_path());
        let _ = engine.save_wait(Some(Duration::from_secs(3)));
    }
    runtime.stats().flush();
    runtime.stats().wait(3000);
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}
