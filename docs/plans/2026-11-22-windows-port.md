# Windows 平台适配方案

目标：在 Windows 10 1809+ / 11 上提供和 Linux、macOS 同一套 ArchType：同一份引擎、会话、设置契约，
外壳用 **TSF（Text Services Framework）** 实现；设置界面继续用 `settings-nsh`（nsh 支持 Windows 目标）。

## 1. 现状盘点

### 可直接复用（不改或小改）

| 库 | 内容 | 备注 |
|---|---|---|
| `core` | 引擎、会话（按键 → 候选 → 上屏 → 学习）、五笔、简繁、emoji、日期时间、计算器、剪贴板历史、统计、翻译、llama-server 管理、`archtype_*` C 接口 | 平台条件编译只有三处，见 §1.2 |
| `voice` | 热键状态机、VAD、ASR 抽象、微信 `wtlocal` 模型、上屏 | `cpal` 在 Windows 上走 WASAPI，无需改动 |
| `net` | curl 下载、SHA256SUMS、发布页解析 | 直接用系统 `curl.exe`（Windows 10 1803+ 自带） |
| `lexicon` | 万象词库包的下载/校验/安装/切换 | 路径由 `Env` 注入 |
| `settings-nsh` | 跨平台设置界面（Rust + NSH） | nsh 原生支持 `--target windows`；需要把配置路径改成 `%APPDATA%\ArchType` |
| `macos-rs` 的平台无关层 | 按键表、配置、引擎装载、会话驱动、统计、翻译配置、更新、语音句柄、窗口模式判定（约 1600 行实现） | 抽成共享 `shell` crate 后 macOS / Windows 共用 |

### 1.2 core 的 Windows 缺口（已核实）

| 位置 | 现状 | Windows 需要 |
|---|---|---|
| `session::system_time()` | `#[cfg(not(unix))]` 返回 `None` | 用 `GetLocalTime` 实现（日期时间候选） |
| `lexstore::Blob::open` | 非 unix 读进堆内存 | 可用，但词库快照每次读文件（几十 MB）；可后续加 `CreateFileMappingW` |
| `llama::spawn_server` | 非 linux 直接 `spawn` | 加 `CREATE_NO_WINDOW`（`creation_flags`）避免弹控制台，并用 Job Object 让 llama-server 随输入法退出 |
| `voice-daemon` | X11 专用 | Windows 另写全局热键（`WH_KEYBOARD_LL`） |

### 1.3 需要新写的平台层

- TSF 文本服务（COM in-proc DLL）
- 按键映射（VK/扫描码 → core 的 keysym）
- 编辑区（TSF composition）
- 候选窗口（Win32 分层窗口自绘）
- 注册与安装（COM + CTF 注册表、MSI）
- 全局语音热键、剪贴板采样、外部命令（`--mode` 等价物）

## 2. 架构决策

### 2.1 抽共享 `shell` crate

现状是 `macos-rs` 的外壳逻辑（`config/engine/input/stats/translate/update/voice/window_mode/keys`）
已经平台无关，只被 macOS 使用。Windows 若复制一份会立刻分叉。

新建 `shell/`（`archtype-shell`）：

```
shell/src/
  config.rs      # archtype.conf 解析/写回（键表、默认值、spec 契约测试）
  engine.rs      # 词库/语言模型/用户词库/历史/自定义词/nnlm 装载
  input.rs       # 会话驱动：按键 → Frame、上屏、学习保存
  stats.rs       # archtype_stats_* 封装
  translate.rs   # TranslateConfig 映射 + 进程级 TranslatorStore
  update.rs      # 版本比较、pkg/msi 下载校验
  voice.rs       # archtype_voice_* 封装
  window_mode.rs # 换应用/窗口判定
  keys/          # keysym/modifier/Shortcut + 平台键表
    macos.rs     # 现 keys.rs 的 macOS 部分（VK_pc 不同，分开）
    win.rs       # 新增：VK + 扫描码 → Key
  layout.rs      # 平台传入的路径（数据目录、资源目录、缓存目录）
```

`macos-rs` 改为 `macos-rs` + `shell` 依赖，只留 macOS 绑定（`macos/` 目录）。
`win-rs` 依赖同一个 `shell`。

验收：macOS 功能与现在一致（现有 41 项测试迁到 `shell`，CI 全绿）。

### 2.2 TSF 而不是 IMM32

- IMM32 只能覆盖传统 Win32 编辑控件，UWP/Edge/Electron 基本不工作；TSF 是现代输入法的唯一选择。
- `windows` crate 已有完整绑定（`windows::Win32::UI::TextServices`：`ITfTextInputProcessor`、
  `ITfKeyEventSink`、`ITfCandidateListUIElement`、`ITfLangBarItemButton` 等，并有 `*_Impl` 供实现）。
- 候选框采用**自绘窗口**而不是系统 `ITfCandidateListUIElement`：和 macOS/Linux 一致的样式、点击选词、
  横/竖排、展开面板、互译面板都在同一套绘制代码里演进。

### 2.3 进程模型

| 进程 | 作用 |
|---|---|
| `archtype-tsf.dll` | 由 `ctfmon.exe` 加载到**每个应用的进程**（32/64 位各一份），实现文本服务 |
| `ArchType.exe` | 控制台工具：`--register` / `--unregister` / `--selftest` / `--install-lexicon` / `--use-lexicon` / `--mode` / `--version` |
| `ArchType 设置.exe` | nsh 设置界面（每个用户一份） |

引擎在 DLL 进程里懒加载（和 fcitx5 插件、macOS 外壳一样），每个宿主进程一份；用户词库/输入习惯
由 `core::saver` 的后台线程写入 `%APPDATA%\ArchType`，多进程同时写用现有的原子改名策略。

## 3. Windows 平台层（`win-rs`）

### 3.1 COM / TSF 组件

```
CLSID_ArchTypeTextService = {6F2D3C1A-...}   （固定，写进 README 与注册表）
GUID_Profile              = {...}             （中文 zh-CN 的键盘 profile）
```

实现：

- `DllMain`（`DLL_PROCESS_ATTACH` 只做 `DisableThreadLibraryCalls`）
- `DllGetClassObject` / `DllCanUnloadNow` / `DllRegisterServer` / `DllUnregisterServer`
- `ITfTextInputProcessorEx`：`ActivateEx` / `Deactivate`
- `ITfThreadMgrEventSink`：`OnSetFocus` / `OnPushContext` / `OnPopContext`
- `ITfKeyEventSink`：`OnTestKeyDown` / `OnKeyDown` / `OnTestKeyUp` / `OnKeyUp` / `OnPreservedKey`
- `ITfCompositionSink`：`OnCompositionTerminated`（应用取消 composition）
- `ITfDisplayAttributeProvider`：preedit 下划线/高亮（可选，第一版可省略）
- `ITfLangBarItemButton`：模式图标（可选，第二期）

注册表（安装时写，卸载时删）：

```
HKCR\CLSID\{CLSID}\InprocServer32            = <安装目录>\archtype-tsf.dll
                                              ThreadingModel = Apartment
HKCR\CLSID\{CLSID}\Implemented Categories\{34745C63-B2F0-4784-8B67-5E12C8701A31}   # 文本服务
HKCR\CLSID\{CLSID}\Implemented Categories\{13A016DF-560B-46CD-947A-4C3AF1E0E35D}   # 键盘
HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x00000804\{GUID_Profile}
        Description = ArchType 拼音, IconFile/IconIndex
HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\Enable = 1
```

### 3.2 按键

`OnKeyDown`（同步）：

1. `GetKeyState(VK_SHIFT/CTRL/MENU/CAPITAL/LWIN/RWIN)` → core 的 modifier 位
2. 特殊键表：`VK_BACK/RETURN/ESCAPE/LEFT/RIGHT/UP/DOWN/PRIOR/NEXT/DELETE/SPACE` → keysym
3. 其他：`ToUnicodeEx(vk, scan, 256 字节 keyState, buf)` 取字符；拿不到字符（输入法正在组合、死键）时
   返回「不处理」，交给应用
4. 交给 `shell::input::Input::key()`；`Frame.handled` 为真则返回 `TRUE`（吃掉），否则 `FALSE`

与 macOS 的 `keys.rs` 一一对应；`shell/src/keys/win.rs` 带单测（VK → keysym、修饰键、大写）。

### 3.3 编辑区与上屏

所有对文本的改写都在 `ITfEditSession::DoEditSession` 里做（`TF_ES_SYNC | TF_ES_READWRITE`）：

- 开始组合：`ITfContextComposition::StartComposition` → 保存 `ITfComposition`
- 更新编辑区：`ITfComposition::GetRange` → `ITfRange::SetText(preedit)`
- 编辑区光标：`ITfRange::Collapse` 到字节偏移（UTF-8 → UTF-16 换算）或用 display attribute 画光标
- 上屏：`ITfRange::SetText(commit)`（替换 composition 范围）→ `ITfComposition::EndComposition`
- 取消：`ITfComposition::EndComposition`（不写文本）
- 读上文（自动空格、标点）：`ITfRange` 前后各取一个字（`GetText`），读不到就不带 `SURROUNDING` 标记
- 密码框：`GUID_PROP_INPUTSCOPE` 里有 `IS_PASSWORD` 时带 `NO_LEARN`/`NO_TRANSLATE`，且不做自动配对

同步失败（`TF_E_SYNCHRONOUS`/`TF_E_NOLOCK`）时排队到 `ITfContext::RequestEditSession` 再试，不要在主线程自旋。

### 3.4 候选窗口

- `CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TOPMOST, WS_POPUP)`
- 自绘：GDI+ 或 Direct2D；文字用 `DirectWrite`（中文渲染质量与系统一致）
- 定位：`ITfContextView::GetTextExt` 取光标矩形（`TF_ES_READ` edit session 里调用）；
  多屏/负坐标用 `MonitorFromRect` 兜底，屏幕下方放不下时画在上面（与 macOS 面板同一逻辑）
- DPI：`GetDpiForWindow` + `AdjustWindowRectExForDpi`
- 点击：`WM_LBUTTONDOWN` → 偏移 → 候选索引 → `candidateClicked`（与 macOS `panel.rs` 同一套
  `Page` 数据结构；绘制逻辑可抽出 `shell::panel_model` 供两端共用）
- 失焦/滚动/焦点变化时隐藏（`ITfThreadMgrEventSink::OnSetFocus`）

### 3.5 定时器（轮询）

TSF 没有 run loop API；建一个 `HWND_MESSAGE` 隐藏窗口，`SetTimer(50ms)` 驱动：

- 语音状态（`archtype_voice_poll`）
- 互译轮询（`Frame.wake_after_ms`）
- 双击 Shift 的等待（`Session::tick`）
- SIGTERM 的等价物：`SetConsoleCtrlHandler` / `WM_QUERYENDSESSION` 时保存学习数据

### 3.6 全局语音热键

- `voice` 的 `hotkey` 是状态机，不含平台绑定；Windows 侧用 `WH_KEYBOARD_LL` 低级钩子
  （`RegisterHotKey` 不支持单修饰键，语音键默认 `Alt_R`）
- 钩子回调只投递消息到隐藏窗口，不在钩子里做重活
- 麦克风：`cpal`（WASAPI）；第一次使用时 Windows 会弹「允许应用访问麦克风」

### 3.7 剪贴板与外部命令

- 剪贴板：`GetClipboardSequenceNumber` 变化时 `OpenClipboard` + `CF_UNICODETEXT` → `core::clipboard::remember`
- `--mode`：Windows 没有分布式通知，用命名管道（`\\.\pipe\archtype-mode`）或 `WM_COPYDATA` 到
  输入法进程；命令行发请求、等回复（2 秒超时），与 macOS 语义一致

### 3.8 数据与配置路径

| 内容 | 路径 |
|---|---|
| `archtype.conf`、`stats.json`、`user.tsv`、`history.tsv`、`custom.tsv`、`translate.key` | `%APPDATA%\ArchType\` |
| 词库包（万象）、语音模型、翻译模型、llama-server | `%LOCALAPPDATA%\ArchType\` |
| 安装包与 DLL | `%ProgramFiles%\ArchType\` |

`shell::layout::Layout` 由平台构造（macOS: `~/Library/Application Support`；Windows: `%APPDATA%`/`%LOCALAPPDATA%`）。

## 4. 设置界面

`settings-nsh` 已支持 Windows（nsh 平台列表包含 Windows，`nsh package --target windows`）。
需要改的只有平台差异（和 macOS 那次一样，UI 不做分支）：

- `Config::config_path()` → `%APPDATA%\ArchType\archtype.conf`
- `Config::data_dir()` → `%LOCALAPPDATA%\ArchType`
- `reload_config()` → 发一条消息给输入法（或空操作，输入法监听文件 mtime，与 macOS 相同）
- `classicui_*` → Windows 上存 `state.json`（同 macOS）
- 候选字号 → `CandidateFontSize`
- 「重启输入法」→ 空操作；「检查更新」→ MSI 更新
- 打开文件/链接用 `ShellExecuteW`（替代 `open`/`xdg-open`）

## 5. 打包与安装

- **MSI（WiX v4）**：`archtype-tsf-x64.dll`、`archtype-tsf-x86.dll`（32 位应用需要）、
  `ArchType.exe`、`ArchType 设置.exe`、`lexicon.tsv`/`words.lm`/`s2t.tsv`/`wubi86.tsv`/`nnlm.bin`
- 安装动作：复制文件 → 注册 COM（调用 `DllRegisterServer` 或直接写注册表）→ 添加语言 profile
- 卸载动作：移除 profile、删除 COM 注册、保留 `%APPDATA%`/`%LOCALAPPDATA%` 用户数据
- 升级：MSI 主版本升级（`MajorUpgrade`），安装前停掉宿主进程（`Restart Manager`）
- 代码签名：与 macOS 一样可选；没有签名时 SmartScreen 会拦一次

## 6. CI 与验证

| 步骤 | 内容 |
|---|---|
| 编译 | `cargo test --workspace`（core/voice/net/lexicon/shell，平台无关） |
| Windows 构建 | `windows-latest`：`cargo build --release --target x86_64-pc-windows-msvc -p archtype-tsf -p archtype-win`；`i686-pc-windows-msvc` 的 32 位 DLL |
| 自检 | `ArchType.exe --selftest`（载入安装包里的词库打 `nihao`，与 macOS 同一套） |
| 打包 | WiX 构建 MSI；`msiexec /i ... /qn` 安装；检查注册表键与输入法出现在 `Get-WinUserLanguageList` |
| 交互测试 | 无法自动化，列手动清单：记事本、VS Code、Chrome、Edge、Word、微信、UWP（设置页搜索框）、高 DPI、多屏、管理员窗口 |

## 7. 里程碑

| 里程碑 | 内容 | 预估 |
|---|---|---|
| M0 | 抽 `shell` crate，macOS 回归全绿（测试迁移） | ✅ 已完成 |
| M0.5 | 抽 `runtime`、拆 `keys`（`keys/{mod,macos,win}`）、抽 `panel`/`view` | ✅ 已完成 |
| M1 | `win-rs` 骨架：COM DLL、注册表注册、TSF 激活、按键 → 上屏 | ✅ 已落地（CI 编译 + 注册/卸载 + 自检） |
| M2a | TSF composition：编辑区显示拼音、上屏替换、外部终止清理 | ✅ 已落地 |
| M2b | 候选窗口：GDI 自绘、点击选词、DPI 缩放、光标上方/下方定位 | ✅ 已落地（真机验收待做） |
| M2 | TSF composition（编辑区）、候选窗口（自绘、翻页、点击、DPI/多屏） | 10–12 人日 |
| M3 | 设置界面 Windows 路径、配置热重载、`--selftest` | 3–4 人日 |
| M4 | 语音（cpal + 全局钩子）、互译、统计、更新、词库包、`--mode` | 8–10 人日 |
| M5 | MSI 打包、CI、文档（README/INSTALL） | 4–5 人日 |
| 合计 | | **35–44 人日** |

### M1–M2b 现状（已落地部分）

- `win-rs/`：COM in-proc DLL（`archtype-tsf.dll`）+ `archtype.exe`，独立 workspace，CI 的 `windows-latest` 任务编译。
- 已实现：`DllGetClassObject` / `DllRegisterServer` / `DllUnregisterServer`；TSF 激活/停用与
  `ITfKeyEventSink` 注册；`ToUnicodeEx` + VK 取字符；`archtype-shell` 的 `Input` 驱动会话；
  `ITfInsertAtSelection` 在 edit session 里上屏（只提交整词，没有编辑区）。
- M2a：`ITfContextComposition` 在光标处开组合，`SetText` 更新编辑区；上屏时替换组合并结束；`ITfCompositionSink`
  在 TSF 外部终止组合时清状态；`Esc`/切走结束并清空。
- M2b：Win32 弹出窗口（`WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`）+ GDI 双缓冲自绘；布局与命中复用
  `archtype_shell::panel`；点击 → `Input::select` → 重新应用一帧；`GetDpiForWindow` 缩放；放不下时画在光标上方。
- 切走应用（`Deactivate`）时异步保存用户词库/历史并 flush 统计。
- 还没做：密码框判定、语音热键（`WH_KEYBOARD_LL`）、MSI 安装包、真机交互验收。

## 8. 风险清单

| 风险 | 影响 | 应对 |
|---|---|---|
| TSF 编辑会话用错（在非 edit session 改文本） | 崩溃、卡死宿主应用 | 所有改写走 `RequestEditSession`；M1 先只做上屏，M2 再引入 composition |
| 32/64 位 | 32 位应用里输入法不出现 | 同时构建 x86/x64 DLL，MSI 两份都注册 |
| 管理员/UAC 进程 | 低权限输入法无法注入高权限窗口 | 与所有第三方输入法一致，文档说明 |
| 全屏独占游戏 | 候选窗口不可见 | 自绘窗口 `TOPMOST`，必要时隐藏候选只保留上屏 |
| Windows 10 旧版本 | 1809 以下不支持 TSF 的某些接口 | 文档写明 1809+；`GetVersionEx` 运行时检查 |
| 没有交互 CI | 回归靠手动 | 引擎/会话用 `cargo test` 覆盖；TSF 层保持薄，逻辑放 `shell` |
| 词库快照 | Windows 非 mmap，启动读几十 MB | 先接受；后续用 `CreateFileMappingW` |
