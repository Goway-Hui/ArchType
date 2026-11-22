# 平台外壳的移植契约

引擎、会话、候选、翻译、统计、语音都在 `core`（Rust），设置契约在 `spec/config.json`。
一个新平台（现在是 macOS，计划中的 Windows）只需要实现**平台绑定**；其余逻辑在
`archtype-shell` 里，不按平台分叉。

## 分层

```
core/            引擎、会话、候选、学习、翻译、统计、剪贴板、语音 FFI（C ABI 也给 fcitx5 插件）
voice/           语音：热键状态机、采集、VAD、离线识别（core 的 voice_ffi 包装它）
net/ lexicon/    下载校验、万象词库包
shell/           外壳共用：config、engine、input、keys、stats、translate、voice、update、panel、view、runtime
macos-rs/        macOS 绑定（InputMethodKit）
win-rs/（计划）  Windows 绑定（TSF）
```

平台外壳负责：

| 职责 | 共用部分 | 平台要做的事 |
|---|---|---|
| 按键 | `shell::keys`（keysym、修饰键、Shortcut） | 平台键码 → `KeyInput`（`keys/macos.rs`、`keys/win.rs`） |
| 会话 | `shell::input::Input` | 把 `Key` 交给它，照 `Frame` 做事 |
| 编辑区 / 上屏 | `Frame.preedit` / `commit` / `cursor_delta` / `delete_next` | 平台文本 API（IMK composition、TSF composition） |
| 候选框 | `shell::panel`（Page、布局、点击命中）、`shell::view`（Frame → Page） | 文字测量（`TextMeasurer`）与绘制、窗口定位 |
| 配置 | `shell::config`（键表、默认值、解析/写回） | 配置文件路径（`Layout`） |
| 引擎/翻译/统计 | `shell::runtime::Runtime` | 启动时 `Layout`，后台载引擎已完成 |
| 翻译 | `shell::translate`（`core::translate` + `core::llama`） | 无（进程共用） |
| 语音 | `shell::voice`（`archtype_voice_*`） | 全局热键（平台钩子）、麦克风权限提示 |
| 更新 | `shell::update`（版本比较、下载校验） | 安装动作（pkg / MSI） |
| 词库包 | `archtype-lexicon` | 配置读写适配（`Env` 实现） |
| 输入法注册 | 无 | TIS / TSF 注册表与 COM |

## 键表的接口

```rust
// shell::keys::KeyInput
pub struct KeyInput { pub keysym: u32, pub modifiers: Modifiers, pub release: bool }

// 每个平台模块导出同名函数与 VoiceKey
pub fn translate_key_down(...) -> Option<KeyInput>;
pub struct VoiceKey { ... }  // parse(name) / is_modifier() / PRESETS
```

`shell::keys` 按 `cfg(target_os)` 导出对应模块（现在 `macos`；Windows 加 `win`）。
`keysym`、`Modifiers`、`Shortcut`、`voice_wave` 平台无关。

## 面板的接口

```rust
pub struct Page { texts, comments, cursor, has_prev, has_next, vertical, columns,
                  aux_top, aux_bottom, labels, font_size }
pub trait TextMeasurer { fn measure(&self, text: &str, font_size: f64) -> (f64, f64); }
pub fn layout(page: &Page, measurer: &impl TextMeasurer) -> Layout;
pub fn clicked_index(page: &Page, measurer: &impl TextMeasurer, x: f64, y: f64) -> Option<usize>;
```

布局（横/竖排、6 列网格、序号/注释宽度、上下提示行）与点击命中都在共用代码里；
平台只按 `Layout` 画，并在 `WM_LBUTTONDOWN` / `mouseDown:` 时回一个候选序号。

## 平台要验证的清单

- 按键：字母、数字选词、方向键、翻页键、退格、回车、Esc、Shift 单按、语音键、快捷键
- 编辑区与上屏：整句、分段选词、Esc 放弃、切换应用时上屏
- 候选框：横/竖排、展开面板、表情/工具栏、联想、互译上下行、点击选词、高 DPI、多屏
- 配置：改 `archtype.conf` 后立刻生效（平台监听或通知）
- 语音：热键、录音、识别、上屏、权限提示
- 更新与词库包：检查更新、下载校验、安装
