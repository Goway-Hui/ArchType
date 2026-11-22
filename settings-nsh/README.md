# ArchType 设置（NSH）

ArchType 设置界面的 NSH 重写：`App.tsx` 是界面，`src/lib.rs` 是同目录 Rust crate（`nsh.json` 的 `"rust": "."`），
通过 `#[native_module("archtype/native")]` 把 `Config` 模块暴露给 TSX。

## 页面

| 页面 | 内容 |
|---|---|
| 常规 | 候选排列、每页个数、翻页键、界面主题、蓝色主题、候选字号 |
| 输入 | 输入方案、双拼方案、模糊音、按错键纠正、单按 Shift、标点自动配对、词库 |
| 表情 | 推荐 emoji、表情面板快捷键 |
| 语音 | 语音输入、语音键、复制结果、模型下载/删除（后台线程 + 进度轮询） |
| 翻译 | 互译、引擎（本地 / 哔哩哔哩 / 自定义）、目标语言、服务商、地址/模型/Key、正式语气、测试连接 |
| 自定义词 | 读取、添加、删除 `custom.tsv` |
| 统计 | 今日/累计字符、活跃时长、速度、按键、输入来源 |
| 更新 | 当前版本、检查 GitHub 最新发布、打开下载页 |

## 构建与检查

```sh
nsh run                      # 开发窗口（GTK）
nsh check                    # cargo check + App.tsx 类型检查
nsh test                     # cargo test + App.test.tsx（headless）
nsh package --target linux   # 打包：目录 + tar.gz（runner、bundle、.desktop）
```

## Rust 侧（`src/lib.rs`）

`Config` 模块负责所有文件与外部命令：

- 配置：`archtype.conf`（`key=value`，保留注释和未知键）、`classicui.conf`、`state.json`；
- 生效：改完写一条 `gdbus` 消息让 fcitx5 重载 archtype 插件（`ReloadAddonConfig`；`fcitx5-remote -r` 只重载输入法配置，插件不会重读）；
- 翻译测试：`curl` 调 OpenAI 格式接口（哔哩哔哩的提示词和默认模型与核心一致），错误信息不含 Key；
- 更新检查：GitHub Releases API；
- 语音模型：`curl` 后台下载微信离线语音包（102,920,639 字节，MD5 固定），`zip` 解出 `dict.decoder.utf8.txt` 和 `.xnet` 到 `wtlocal`；
- 自定义词：`custom.tsv`（`编码<TAB>词`，两行注释头）；
- 翻译 Key：`~/.config/archtype/translate.key`（0600，空则删除）。

## macOS

同一份 app 用 `nsh package --target macos` 打成 `ArchType 设置.app`，随输入法包放在
`ArchType.app/Contents/Resources/`，输入法菜单「设置…」打开（可带 `--page general|words|voice`）。
平台差异都在 Rust 侧，TSX 不做分支：

- `archtype.conf` / `state.json` / `translate.key` 在 `~/Library/Application Support/ArchType/`；
- `classicui.conf` 不存在，候选框主题这类键存进 `state.json`（`classicui.<键>`）；
- 「候选字号」读写 `CandidateFontSize`（archtype.conf），不是 fcitx 字体；
- 改完不需要通知（输入法自己监听 `archtype.conf` 的修改时间）；「重启输入法」是空操作；
- 打开文件/链接用 `open`；
- 「更新」页在 macOS 上提示下载新的 pkg（安装包更新，不走发行版包管理器）。

## GTK 要求

NSH 0.6 的 Linux runner 需要 **GTK 4.14+**：

- 支持：Arch Linux、Fedora 40+、Ubuntu 24.04+、Debian 13+、deepin 25。

Ubuntu 22.04、deepin 23 及更早的系统不再提供安装包与图形设置界面；`archtype-config` 只保留命令行部分（`--setup`、`--install-lexicon`、`--startup` 等）。

## 打包接入

```sh
scripts/package-settings-nsh.sh 0.6.0 dist/pkg   # 产出 archtype-settings-nsh-0.6.0-linux-<arch>.tar.gz
```

`scripts/package-deb.sh`、`package-rpm.sh`、`package-arch.sh` 在设置了 `ARCHTYPE_NSH_ARCHIVE=<tar.gz>` 时：

- 解压到 `/usr/lib/archtype/settings/`；
- 安装 `/usr/bin/archtype-settings` 启动脚本；
- 把 `io.github.GowayHui.ArchType.Config.desktop` 的 `Exec` 改成 `archtype-settings`。

没有设置时只装命令行 `archtype-config`（同样的界面在 `fcitx5-configtool` 里也能改）。

## 开发注意

- `nsh run` 的开发宿主是常驻进程，热更新会保留 signal 状态（例如当前页）；改页面初始值后要连宿主一起重启。
- 长任务（下载语音模型、翻译测试）在 Rust 侧同步/后台执行；UI 状态下轮询，避免阻塞渲染。
- 文件写入用「读旧文本、只改目标行、原子替换」，不要重写整个 `archtype.conf`。
