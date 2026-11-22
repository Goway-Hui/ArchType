//! 互译：复用核心的翻译器（`core::translate` + `core::llama`），外壳只负责把设置翻成它的配置。
//!
//! 翻译器是进程里共用的一份（本地模型的 llama-server 也只起一个），所有输入框共用；
//! 会话通过 `TranslateBackend` 借它。

use std::path::PathBuf;

use archtype_core::llama::LocalConfig;
use archtype_core::translate::TranslateConfig;
pub use archtype_core::translate::Translator;

use crate::config::Config;
use crate::engine::Layout;

/// 默认的本地翻译模型（腾讯 HY-MT，GGUF），和设置界面的「翻译」页一致。
pub const DEFAULT_LOCAL_MODEL: &str = "HY-MT1.5-1.8B-Q4_K_M.gguf";

/// 把设置翻成核心的翻译配置。
pub fn translate_config(config: &Config, layout: &Layout) -> TranslateConfig {
    let key_file = Some(layout.support_dir.join("translate.key"));
    let local = if config.translate_local {
        let model = if config.translate_local_model.trim().is_empty() {
            layout.support_dir.join("models").join(DEFAULT_LOCAL_MODEL)
        } else {
            expand_tilde(&config.translate_local_model)
        };
        Some(LocalConfig {
            model: if model.is_absolute() {
                model
            } else {
                layout.support_dir.join(model)
            },
            server: config.translate_llama_server.clone(),
            // 闲置几分钟后释放内存，0 表示一直开着
            idle_secs: config.translate_idle_minutes.max(0) as u64 * 60,
        })
    } else {
        None
    };
    TranslateConfig {
        base_url: config.translate_base_url.clone(),
        model: config.translate_model.clone(),
        target: config.translate_target.clone(),
        formal: config.translate_formal,
        key_file,
        local,
    }
}

/// 配好了翻译服务（有本地模型或填了接口和模型）才能进「译」。
pub fn translate_available(config: &Config, layout: &Layout) -> bool {
    config.translate && translate_config(config, layout).is_ready()
}

/// 进程里共用的翻译器（`Box::into_raw`，和引擎一样永不释放）。
pub struct TranslatorStore {
    ptr: *mut Translator,
}

// SAFETY: 翻译器内部全是 Arc<Mutex<..>>，可以在线程间共享；外壳只在主线程配置它，
// 后台工作由它自己的线程做。
unsafe impl Send for TranslatorStore {}
unsafe impl Sync for TranslatorStore {}

impl TranslatorStore {
    pub fn new() -> Self {
        Self {
            ptr: Box::into_raw(Box::new(Translator::new())),
        }
    }

    pub fn get(&self) -> &'static Translator {
        // SAFETY: 指针来自 Box::into_raw，没释放过。
        unsafe { &*self.ptr }
    }
}

impl Default for TranslatorStore {
    fn default() -> Self {
        Self::new()
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    archtype_core::llama::expand_home(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn layout() -> Layout {
        Layout::new("/tmp/archtype-test-res", "/tmp/archtype-test-data")
    }

    #[test]
    fn remote_service_needs_url_and_model() {
        let mut config = Config::default();
        config.translate = true;
        config.translate_local = false;
        assert!(!translate_available(&config, &layout()));
        config.translate_base_url = "https://api.example.com/v1".into();
        assert!(!translate_available(&config, &layout()));
        config.translate_model = "gpt-4o-mini".into();
        assert!(translate_available(&config, &layout()));
        let translated = translate_config(&config, &layout());
        assert_eq!(translated.target, "en");
        assert!(translated.local.is_none());
    }

    #[test]
    fn local_model_points_at_the_support_dir_and_respects_idle() {
        let mut config = Config::default();
        config.translate = true;
        config.translate_idle_minutes = 7;
        let translated = translate_config(&config, &layout());
        let local = translated.local.clone().expect("本地模型");
        assert!(local.model.ends_with(DEFAULT_LOCAL_MODEL));
        assert_eq!(local.idle_secs, 7 * 60);
        // 模型文件不在时不算配好（is_ready 看文件）
        assert!(!translated.is_ready());

        config.translate_local_model = "~/models/my.gguf".into();
        let translated = translate_config(&config, &layout());
        let model = translated.local.unwrap().model;
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(model, PathBuf::from(home).join("models/my.gguf"));
    }

    #[test]
    fn translate_is_off_until_enabled() {
        let config = Config::default();
        assert!(!translate_available(&config, &layout()));
    }
}
