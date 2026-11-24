//! 输入法外壳的共用逻辑，macOS 与 Windows 用同一份：
//!
//! - `config`：archtype.conf 的键表、默认值、解析/写回（对照 spec/config.json）
//! - `engine`：词库、语言模型、用户词库、输入习惯、自定义词、整句模型的装载
//! - `keys`：平台键码（macOS 的 VK；Windows 的键表在 M1 加）与快捷键解析
//! - `input`：会话驱动（按键 → 候选 → 上屏 → 学习保存）
//! - `stats`：打字统计（复用核心的 `archtype_stats_*`）
//! - `translate`：互译配置与进程级翻译器
//! - `voice`：语音句柄（复用核心的 `archtype_voice_*`）
//! - `update`：版本比较、下载与校验（安装动作由平台做）
//! - `panel`：候选框的数据与布局（尺寸、位置、点击命中），平台只提供文字测量与绘制
//! - `view`：会话的一帧 → 候选框的一页（横/竖排、展开网格、联想、互译、语音提示）
//! - `window_mode`：换应用/窗口判定

pub mod config;
pub mod engine;
pub mod input;
pub mod keys;
pub mod panel;
pub mod runtime;
pub mod stats;
pub mod translate;
pub mod update;
pub mod view;
pub mod voice;
pub mod window_mode;
