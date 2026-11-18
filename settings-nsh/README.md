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
- 生效：`fcitx5-remote -r`；
- 翻译测试：`curl` 调 OpenAI 格式接口（哔哩哔哩的提示词和默认模型与核心一致），错误信息不含 Key；
- 更新检查：GitHub Releases API；
- 语音模型：`curl` 后台下载微信离线语音包（102,920,639 字节，MD5 固定），`zip` 解出 `dict.decoder.utf8.txt` 和 `.xnet` 到 `wtlocal`；
- 自定义词：`custom.tsv`（`编码<TAB>词`，两行注释头）；
- 翻译 Key：`~/.config/archtype/translate.key`（0600，空则删除）。

## GTK 要求

NSH 0.6 的 Linux runner 需要 **GTK 4.14+**：

- 支持：Arch Linux、Fedora 40+、Ubuntu 24.04+、Debian 13+；
- 不支持：Ubuntu 22.04、deepin 23（GTK 4.6/4.8）。这些系统继续用 Python 版 `archtype-config`（同一个 GUI 入口，功能覆盖旧版全部页面）。

打包脚本在支持的发行版里安装 NSH 设置并以它作为「ArchType 设置」的默认入口；旧发行版保持 Python 设置。

## 打包接入

```sh
scripts/package-settings-nsh.sh 0.6.0 dist/pkg   # 产出 archtype-settings-nsh-0.6.0-linux-<arch>.tar.gz
```

`scripts/package-deb.sh`、`package-rpm.sh`、`package-arch.sh` 在设置了 `ARCHTYPE_NSH_ARCHIVE=<tar.gz>` 时：

- 解压到 `/usr/lib/archtype/settings/`；
- 安装 `/usr/bin/archtype-settings` 启动脚本；
- 把 `io.github.GowayHui.ArchType.Config.desktop` 的 `Exec` 改成 `archtype-settings`。

没有设置时保持原行为（Python `archtype-config`）。

## 开发注意

- `nsh run` 的开发宿主是常驻进程，热更新会保留 signal 状态（例如当前页）；改页面初始值后要连宿主一起重启。
- 长任务（下载语音模型、翻译测试）在 Rust 侧同步/后台执行；UI 状态下轮询，避免阻塞渲染。
- 文件写入用「读旧文本、只改目标行、原子替换」，不要重写整个 `archtype.conf`。
