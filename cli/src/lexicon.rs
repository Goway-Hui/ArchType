//! 词库命令（`--install-lexicon` / `--use-lexicon`）：路径和配置读写接到命令行这一侧，
//! 下载、校验、安装的逻辑在 `archtype-lexicon`，和 macOS 外壳共用。

use crate::config;
use archtype_lexicon::Env;

pub struct CliEnv;

impl Env for CliEnv {
    fn data_dir(&self) -> String {
        config::data_dir()
    }

    fn get(&self, key: &str) -> String {
        config::get(key)
    }

    fn set(&self, key: &str, value: &str) -> bool {
        config::set(key, value)
    }

    fn reload(&self) -> bool {
        std::process::Command::new("gdbus")
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
}

pub fn auto_update() -> Result<bool, String> {
    archtype_lexicon::auto_update(&CliEnv)
}

pub fn use_lexicon(kind: &str) -> i32 {
    archtype_lexicon::use_lexicon(&CliEnv, kind)
}

pub fn install_lexicon(source: &str) -> i32 {
    archtype_lexicon::install_lexicon(&CliEnv, source)
}
