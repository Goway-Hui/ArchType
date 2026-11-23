# Linux 安装与更新

ArchType 是 Fcitx 5 的输入法插件。安装时需要让插件、Fcitx 5 和系统动态库使用兼容的版本；
安装包因此按发行版构建。源码构建会使用本机的 Fcitx 5 开发文件。

最低要求：Fcitx 5.0、C++20 编译器、CMake 3.16。构建使用当前稳定版 Rust 和仓库的 `Cargo.lock`。
默认启用的语音功能需要 ALSA。设置界面是 Rust + NSH 写的原生程序：构建需要 GTK 4.14+、libadwaita、WebKitGTK 6、GStreamer 的开发包和 `nsh` CLI，运行时只需要这些库（不需要 Python）；
蓝色候选框主题和语音浮条（GNOME 扩展）需要 GNOME 45+。

## 选择安装方式

| 系统 | 方式 | 说明 |
|---|---|---|
| Arch Linux | 一键安装或本机 `makepkg` | 预编译包只有 x86_64；依赖不满足时先 `sudo pacman -Syu` |
| EndeavourOS / Garuda / CachyOS | Arch 安装路径或本机打包 | 系统必须与所用 Arch 动态库兼容；加载失败时本机重编译 |
| Manjaro | 本机 `makepkg` | 使用自己的软件源，脚本不会自动套用 Arch 包 |
| Ubuntu 24.04 / 26.04 | 对应版本 deb | amd64、arm64；不跨 Ubuntu 版本混装 |
| Debian 13 / 13.x | Debian 13 deb | amd64、arm64 |
| deepin 25 | 对应版本 deb | 只有 amd64；不改输入法框架（deepin 默认就是 fcitx5） |
| Mint / Pop!_OS 等衍生版 | 明确声明兼容基线时使用 deb | `noble → ubuntu24.04`、`resolute → ubuntu26.04`、`trixie → debian13` |
| Fedora 43 / 44 / 45 | 对应 `.fc<版本>` RPM | x86_64、aarch64；一键安装只匹配当前系统版本 |
| openSUSE Tumbleweed | 本机源码安装 | 独立的 zypper 依赖和 Fcitx 插件路径 |
| NixOS / Home Manager | flake | x86_64、aarch64；与系统共用 nixpkgs，注册到 Fcitx 插件列表 |
| Fedora Atomic | 宿主 `rpm-ostree` 分层 | 普通 dnf 安装脚本会给出手动说明 |

[Releases](https://github.com/Goway-Hui/ArchType/releases) 中存在对应资产后才能使用二进制安装。
没有发布包、其他 CPU 架构或其他发行版，先使用下面的源码构建方法。
Alpine 使用 musl，需要单独完成构建与运行验证；这里不把 glibc 安装包用于 Alpine。
openSUSE Leap、RHEL / Rocky / AlmaLinux 也需要先核对 Fcitx 5 和 libadwaita 的可用版本。

## 一键安装与升级

用普通用户运行；脚本会在安装软件包时调用 sudo：

```bash
curl -fsSL https://raw.githubusercontent.com/Goway-Hui/ArchType/main/install.sh | bash
```

下载目标来自当前仓库，按系统和 CPU 架构选择 `.deb`、`.pkg.tar.zst` 或 `.rpm`，每个文件都校验 SHA256。
缺少匹配包、校验记录重复、哈希不符或下载失败时，脚本在安装前停止。
Arch 不会替你升级整个系统；提示依赖不满足时，先运行 `sudo pacman -Syu` 再重新安装。

查看安装目标、固定版本或保留当前桌面设置：

```bash
curl -fsSL https://raw.githubusercontent.com/Goway-Hui/ArchType/main/install.sh -o install.sh
bash install.sh --help
bash install.sh --print-plan
ARCHTYPE_VERSION=v0.3.8 bash install.sh
ARCHTYPE_NO_IM_CONFIG=1 ARCHTYPE_NO_RESTART=1 bash install.sh
```

`--print-plan` 不查询网络、不安装软件、不修改配置。`ARCHTYPE_YES=1` 可用于非交互安装。
`ARCHTYPE_REPO=owner/repo` 可选择其他发布仓库；系统和版本的匹配规则仍然生效。
对应发行版的 Release 资产不存在时，请从本仓库源码构建。

## 从源码构建

### 1. 安装本发行版的依赖

#### Debian 13 / Ubuntu 24.04、26.04 / 兼容衍生版

```bash
sudo apt update
sudo apt install build-essential pkg-config cmake extra-cmake-modules git curl xz-utils \
  libasound2-dev libfcitx5core-dev libfcitx5config-dev libfcitx5utils-dev \
  fcitx5 fcitx5-frontend-all fcitx5-config-qt im-config \
  libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev libjavascriptcoregtk-6.0-dev \
  libsoup-3.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev clang
```

已有可用的 Rust 工具链可以直接使用；未安装时按 [Rust 官方说明](https://www.rust-lang.org/tools/install)安装：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile minimal
. "$HOME/.cargo/env"
```

#### Arch / EndeavourOS / Garuda / CachyOS / Manjaro

```bash
sudo pacman -Syu --needed base-devel rust cmake extra-cmake-modules pkgconf git curl xz \
  fcitx5 fcitx5-gtk fcitx5-qt fcitx5-configtool alsa-lib \
  gtk4 libadwaita webkitgtk-6.0 gstreamer gst-plugins-base clang
```

Arch 的 `fcitx5` 包已经包含头文件和 CMake 配置，不需要另找 `fcitx5-devel`。
使用 rustup 管理工具链的用户保留现有 rustup 安装，不要再安装与它冲突的 `rust` 包。
Manjaro 在自己的仓库内完整升级后，本机编译下面的安装包。

#### Fedora

```bash
sudo dnf install rust cargo gcc-c++ make cmake extra-cmake-modules pkgconf-pkg-config git curl xz \
  fcitx5 fcitx5-devel alsa-lib-devel \
  fcitx5-configtool fcitx5-gtk3 fcitx5-gtk4 fcitx5-qt5 fcitx5-qt6 \
  gtk4-devel libadwaita-devel webkitgtk6.0-devel javascriptcoregtk6.0-devel \
  libsoup3-devel gstreamer1-devel gstreamer1-plugins-base-devel clang
```

GTK 和 Qt 前端显式安装各自的版本子包，适用于禁用弱依赖的系统。
需要制作本机 RPM 时，再安装 `rpm-build`。

#### openSUSE Tumbleweed

```bash
sudo zypper refresh
sudo zypper install rust cargo gcc-c++ make cmake extra-cmake-modules pkgconf-pkg-config git curl xz \
  fcitx5 fcitx5-devel fcitx5-configtool fcitx5-gtk3 fcitx5-gtk4 alsa-devel \
  gtk4-devel libadwaita-devel webkit2gtk6.0-devel libsoup-3_0-devel \
  gstreamer-devel gstreamer-plugins-base-devel clang
```

Qt 应用还需要与应用 Qt 版本对应的 Fcitx 前端，可用 `zypper search fcitx5-qt` 查看仓库提供的子包。
WebKitGTK / libsoup 的 -devel 包名会随 Tumbleweed 调整；改设置界面时需要它们（`nsh` 的 Linux host 用 WebKit 画网页界面）。

### 2. 获取源码与词库

```bash
git clone https://github.com/Goway-Hui/ArchType.git
cd ArchType
scripts/make-data.sh "" dist/data
scripts/make-wubi.sh "" dist/data
```

词库脚本下载并转换 rime-ice，合并项目热词，下载经过固定 SHA256 校验的语言模型和整句增强模型。
输出包含 `lexicon.tsv`、`english.txt`、`words.lm`、`nnlm.bin` 和许可证、来源信息
（整句增强模型下载不到时只警告、不带 `nnlm.bin`，见 [DATA.md §3.5](DATA.md#35-archtype-自己的整句增强模型-nnlmbin)）。
五笔脚本按钉死的提交下载 rime-wubi（LGPL-3.0），核对 SHA256 后生成五笔码表 `wubi86.tsv`
和 `WUBI-LICENSE`、`WUBI-AUTHORS`、`WUBI-SOURCE.txt`；各发行版的打包脚本都要这几个文件。
已有 rime-ice checkout 时可传入其路径，避免重复下载。
默认还会融进仓库里的语料词频表 `data/tatoeba-freq.tsv`（不联网），得到的词库和发布的一样，
见 [CONFIG.md](CONFIG.md) 的「语料词频」。

### 3. Arch / Manjaro：制作原生安装包

安装好第 1 节依赖并准备数据后，用普通用户执行：

```bash
version=$(sed -n 's/^version = "\(.*\)"/\1/p' core/Cargo.toml)
scripts/package-arch.sh "$version" dist/pkg
sudo pacman -U "dist/pkg/archtype-$version-1-$(uname -m).pkg.tar.zst"
```

`makepkg` 会在独立目录编译并把插件、设置界面、自检工具和完整词库一起打包。
之后可用 `pacman -Q archtype` 查询安装版本、`pacman -Qkk archtype` 检查已安装文件。
无需继续执行通用源码安装步骤，直接进入“启用 Fcitx 5”。

### 4. 通用源码编译与安装

其他发行版或不需要原生安装包时，在仓库根目录执行：

```bash
cargo build --release --locked -p archtype-core
cargo test --locked -p archtype-core
./scripts/abi-smoke.sh
cmake -S fcitx5-addon -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr
cmake --build build -j"$(nproc)"
sudo cmake --install build
```

CMake 使用 Fcitx 5 导出的安装路径：Debian / Ubuntu 通常为 `/usr/lib/<multiarch>/fcitx5`，
Arch 为 `/usr/lib/fcitx5`，Fedora / openSUSE 为 `/usr/lib64/fcitx5`。
可用 `pkg-config --variable=libdir Fcitx5Core` 查看本机库目录。
输入法注册文件、简繁映射表、自检工具（`archtype-check`、`archtype-show`）、`archtype-config` 命令行和第 2 节生成的
`dist/data` 里的词库、英文词表、语言模型、五笔码表都由 `cmake --install` 装好（装在 `/usr/share/archtype/`），
不用再往用户目录拷文件。图形设置界面是 NSH 的独立产物：需要 `nsh` 时先
`curl -fsSL https://nsh.dev/install.sh | NSH_VERSION=v0.6.0 sh`，再
`scripts/package-settings-nsh.sh "$version" dist/pkg` 打出 tar.gz，解压到 `/usr/lib/archtype/settings/`
并装好 `archtype-settings` 启动脚本（各发行版的包脚本会自动接上）；不装图形界面时用 `fcitx5-configtool` 添加输入法。以后更新数据：重新跑第 2 节的脚本，再 `sudo cmake --install build`。
数据放在别处时加 `-DARCHTYPE_DATA_DIR=<目录>`。
以前按旧说明拷进 `~/.local/share/archtype/` 的那几个文件不用管，插件认得出来、会改用新装的（见
[CONFIG.md](CONFIG.md) 的「数据文件」）。

需要可由包管理器卸载的包时，Debian / Ubuntu 可用
`scripts/package-deb.sh engine "$version" dist/pkg`，Fedora 可用
`scripts/package-rpm.sh "$version" dist/pkg`，其中 `version` 按上一节从 `core/Cargo.toml` 读取。
Debian 打包还需要 `dpkg-dev`，Fedora 打包需要 `rpm-build`；两者都需要第 2 节的数据。

只保留文字输入时，Rust 和 CMake 两侧一起关闭语音：

```bash
cargo build --release --locked -p archtype-core --no-default-features
cmake -S fcitx5-addon -B build-text -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX=/usr -DARCHTYPE_VOICE=OFF
cmake --build build-text -j"$(nproc)"
sudo cmake --install build-text
```

这条构建路径不需要 ALSA 开发包；词库和自检工具同样由 `cmake --install` 装好。
如果不需要设置界面，再添加 `-DARCHTYPE_INSTALL_SETTINGS=OFF`，使用 `fcitx5-configtool` 添加输入法。
维护开发环境时可用 `scripts/build-linux.sh --addon` 统一构建与测试。
真实词库存在时会运行按键测试，测试失败会直接返回非零状态。

## 启用 Fcitx 5

先让桌面启动 Fcitx 5，再添加 ArchType。修改会话配置后注销并重新登录。

| 桌面 / 系统 | 设置 |
|---|---|
| Debian / Ubuntu | `im-config -n fcitx5`，然后重新登录 |
| KDE Plasma Wayland | 系统设置 → 虚拟键盘 → Fcitx 5，由 KWin 启动输入法 |
| Fedora 的其他桌面 | 可安装 `fcitx5-autostart`，或使用 `im-chooser` 选择 Fcitx 5 |
| Arch / openSUSE 的 X11 桌面 | 在桌面自启动项中启用 Fcitx 5，并按下面设置 X11 环境变量 |
| GNOME / 其他 Wayland 合成器 | 按合成器的启动方式配置 Fcitx 5；GNOME 可安装 Input Method Panel 扩展 |

X11 会话通常需要在桌面读取的登录配置文件中设置：

```bash
export XMODIFIERS=@im=fcitx
export GTK_IM_MODULE=fcitx
export QT_IM_MODULE=fcitx
```

Wayland 的客户端支持不同：KDE / KWin 的原生 Qt 通道通常应让 `QT_IM_MODULE` 保持未设置，
支持 text-input-v3 的 GTK 应用也可使用原生通道。按 [Fcitx 官方 Wayland 指南](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland/en)
配置自己的桌面，不要把上面的 X11 变量统一写入所有 Wayland 会话。
例外是 sway 1.9（Ubuntu 24.04 带的版本）：它不画输入法的弹出窗口，GTK 应用走 text-input-v3 时看不到候选框（fcitx5 自己的 Unicode 选字框也看不到）；保留上面的 `GTK_IM_MODULE=fcitx`、`QT_IM_MODULE=fcitx`（im-config 默认就是这样）一切正常。sway 1.10 两种都行。
需要自启动文件的 XDG 桌面可将 `/usr/share/applications/org.fcitx.Fcitx5.desktop` 复制到 `~/.config/autostart/`；
KDE Wayland 使用其虚拟键盘设置，避免重复启动。

添加输入法：

```bash
archtype-config --setup
```

（或者打开「ArchType 设置」，点第一页的「一键设置」。fcitx5 还没在跑时它会在有 im-config 的系统上问你要不要切到 fcitx5，
fcitx5 在跑但还没载入 ArchType 时问你要不要重启它。）

或打开 `fcitx5-configtool`，取消“仅显示当前语言”，搜索并添加 ArchType。
按 `Ctrl+Space` 切换，输入 `nihao`。重启 Fcitx 5 后，Chromium / Electron 程序可能需要退出后重新打开。

## 检查与排错

```bash
archtype-config --self-test
# 安装包自带的系统词库：
archtype-check --lexicon /usr/share/archtype/lexicon.tsv
archtype-show --lexicon /usr/share/archtype/lexicon.tsv nihao
# 源码安装到当前用户目录的词库：
archtype-check --lexicon "${XDG_DATA_HOME:-$HOME/.local/share}/archtype/lexicon.tsv"
fcitx5-diagnose
```

| 现象 | 检查方式 |
|---|---|
| CMake 找不到 Fcitx5Core 或版本过旧 | `pkg-config --modversion Fcitx5Core`；安装第 1 节的开发包 |
| 找不到 `libarchtype_core.a` | 先构建 Rust 核心；自定义 target 目录时用 `-DARCHTYPE_CORE_LIB=/绝对路径/libarchtype_core.a` |
| 输入法列表没有 ArchType | 检查 `/usr/share/fcitx5/addon/archtype.conf`、`/usr/share/fcitx5/inputmethod/archtype.conf` 和插件库目录 |
| 插件报 undefined symbol | 按本机发行版安装匹配包；Arch 完整升级，Manjaro 本机重编译 |
| 设置界面打不开 | 缺 GTK 4 / libadwaita / WebKitGTK 6 的运行时库（设置界面是 NSH 的原生程序，不用 Python）；按第 1 节补齐，或看 `archtype-settings` 的报错 |
| 只有少量候选 | 真实词库未加载，程序退回演示词典；检查 `lexicon.tsv` 路径 |
| 只在部分应用里无法输入 | 检查应用所用 GTK / Qt 前端、X11 / Wayland 配置，并重启应用 |
| emoji 显示方框 | Debian / Ubuntu 安装 `fonts-noto-color-emoji`；Arch 安装 `noto-fonts-emoji`；Fedora 安装 `google-noto-color-emoji-fonts` |

用户数据优先于系统词库；环境变量优先于设置界面配置。完整变量说明见 [CONFIG.md](CONFIG.md)，语音说明见 [VOICE.md](VOICE.md)。
[CI](../.github/workflows/ci.yml) 定义了安装与更新回归、Debian / Ubuntu / Arch / Fedora 打包安装、openSUSE 源码安装和 Nix 检查。
桌面候选框、真实麦克风和具体应用兼容性需要在对应桌面会话中确认。

## NixOS / Nix

构建或检查 flake：

```bash
nix build github:Goway-Hui/ArchType
nix flake check github:Goway-Hui/ArchType
```

系统 flake 配置：

```nix
# flake.nix 的 inputs
archtype = {
  url = "github:Goway-Hui/ArchType/release";
  inputs.nixpkgs.follows = "nixpkgs";
};

# configuration.nix，inputs 通过 specialArgs 传入
# 与系统使用同一份 Fcitx 5，避免插件 ABI 不匹配。
i18n.inputMethod = {
  enable = true;
  type = "fcitx5";
  fcitx5.addons = [ inputs.archtype.packages.${pkgs.stdenv.hostPlatform.system}.default ];
};
```

运行 `sudo nixos-rebuild switch` 后重新登录。Home Manager 将同一个包加入自己的 `i18n.inputMethod.fcitx5.addons`。
也可以使用 `nixpkgs.overlays = [ inputs.archtype.overlays.default ];` 后引用 `pkgs.archtype`。
插件作为系统 Fcitx 的 addon 注册；单独把包放进 Nix profile 不会自动完成插件注册。
词库使用 `nix/package.nix` 中固定 URL 和哈希的上游数据资产。

flake 输入选哪个：

| 写法 | 拿到的是 |
|---|---|
| `github:Goway-Hui/ArchType/release` | 最新正式版（推荐）；每次发版时 CI 把 `release` 分支移到新版本的标签 |
| `github:Goway-Hui/ArchType/v0.3.10` | 锁定这一版，升级时手动改版本号 |
| `github:Goway-Hui/ArchType` | main 分支最新提交，可能包含还没发布的改动 |

更新时先在自己的 flake 中执行 `nix flake update archtype`，再重建系统或 Home Manager 配置。
CI 在 nixos-unstable（x86_64、aarch64）和当前稳定版 nixos-26.05（x86_64）上构建并自检，
发布要等这些检查都通过。设置界面的更新提醒在 NixOS 上只提示新版本，不会去调用其他包管理器。

### 单按 Shift 在 NixOS 上

fcitx5 默认用左 Shift「临时切换输入法」，它先拿到这一下，ArchType 收不到。设置界面的「Shift 只给 ArchType 用」
是让正在跑的 fcitx5 改掉这个快捷键，再由 fcitx5 存进 `~/.config/fcitx5/config`。
用 Home Manager 的 `i18n.inputMethod.fcitx5.settings` 时，整个 `~/.config/fcitx5` 是指向 `/nix/store` 的只读目录，
fcitx5 存不进去，重新登录后左 Shift 又被拿走（设置界面会提示「只在这次有效」）；ArchType 自己的设置也存不了，
要写在 `settings.addons.archtype.globalSection` 里。这种情况把快捷键写进 Nix 配置：

```nix
# Home Manager
i18n.inputMethod.fcitx5.settings.globalOptions = {
  Hotkey.AltTriggerKeys = "";   # 空值 = 不用 Shift「临时切换输入法」；切输入法还是 Ctrl+空格
};
```

NixOS 模块的同名选项（`i18n.inputMethod.fcitx5.settings.globalOptions`，写法相同）写的是 `/etc/xdg/fcitx5/config`，
只在没有 `~/.config/fcitx5/config` 时才生效；已经有这个文件时，用设置界面的开关就行（它能写这个文件）。
fcitx5 其他全局快捷键（`EnumerateForwardKeys` 等）里也不要放单按的 Shift。

### 维护：先发预发布

`v*` 的 tag 打在 main 上的提交才发成正式版（成为 Latest）；打在别处（比如 pre-release 分支的提交）时，
发布 job 发成 GitHub 的 pre-release，不发 Telegram、不动 `release` 分支、不开 Nix 词库 PR。
设置界面和 `install.sh` 的检查更新都只认 `releases/latest`，普通用户拿不到预发布；
自己测时指定版本装（Release 说明的开头有这条命令）：

```bash
curl -fsSL https://raw.githubusercontent.com/Goway-Hui/ArchType/pre-release/install.sh | ARCHTYPE_VERSION=v<版本> bash
```

测完转正：把 pre-release 合进 main，运行 `gh release edit v<版本> --prerelease=false --latest`，
再照下面一节手动更新 `release` 分支；这一版换了词库时，下载这一版 Release 里的 `archtype-data-rime-ice-*.tar.gz`，
在本机运行 `GITHUB_REPOSITORY=Goway-Hui/ArchType GITHUB_REF_NAME=v<版本> GH_TOKEN=$(gh auth token) .github/scripts/nix-data-pr.sh <那个文件>`
开 Nix 词库的 PR。
要改代码就打下一个版本号，别挪已经发出去的 tag。

### 维护：发版时的 Nix 相关步骤

- `release` 分支由发布 job 自动更新。`GITHUB_TOKEN` 不能把分支移到改过 `.github/workflows` 的提交上，
  两次发版之间改过 CI 时会失败并在 Actions 里留下提醒，此时在本机运行
  `git push --force origin v<版本>^{commit}:refs/heads/release`。
  想全自动，在仓库 Secrets 里加 `RELEASE_BRANCH_TOKEN`：细粒度 PAT，只给本仓库，权限 Contents 和 Workflows 读写。
- 词库换了版本（rime-ice 更新，或 `scripts/make-data.sh` 的 REV 加一）时，新词库包要等发版后才有下载地址，
  发布 job 会自动改好 `nix/package.nix` 里的 `dataVersion`、`url`、`hash`，推到 `bot/nix-data-<词库版本>` 分支并开 PR 到 main，
  合并就行。这要在仓库设置 Actions → General 里勾上「Allow GitHub Actions to create and approve pull requests」；
  没勾时退回开 issue，issue 里有开 PR 的链接。Actions 开的 PR 不会自动跑 CI，要跑就关掉再重新打开。

## macOS

需要 macOS 12 或更新（Apple 芯片和 Intel 都行；安装包是通用二进制）。

- 下载 `ArchType-<版本>-macos.pkg` 双击安装：只装给当前用户，不要管理员密码；系统会问要不要打开这个没公证的安装包，
  到「系统设置 → 隐私与安全性」点「仍要打开」。
- 或者解压 `ArchType-<版本>-macos.zip`，运行发布页里的 `install.sh`（或仓库里的 `macos/install.sh`）：
  装到 `~/Library/Input Methods/`、去掉隔离标记，再 `--register` / `--enable` 登记并打开。

设置界面是同一个跨平台 app（`ArchType 设置.app`，随输入法包一起安装）：菜单栏的输入法菜单「设置…」打开，
也可以从 `ArchType --settings` 打开，`--settings voice` 直接到「语音」页。

语音输入要先在设置「语音」页下载模型；第一次按语音键时系统会问麦克风权限，在「系统设置 → 隐私与安全性 → 麦克风」里允许。

## Fedora Atomic

Silverblue / Kinoite 等桌面在宿主上使用匹配当前 Fedora 版本的 RPM。
下载并按 Release 的 `SHA256SUMS` 核对后，使用宿主终端：

```bash
rpm-ostree install ./archtype-*.rpm fcitx5 fcitx5-configtool \
  fcitx5-gtk3 fcitx5-gtk4 fcitx5-qt5 fcitx5-qt6 \
  gtk4 libadwaita webkitgtk6.0 libsoup3 gstreamer1 gstreamer1-plugins-base
systemctl reboot
```

重启进入新部署后按桌面说明启用 Fcitx 5。Toolbox 内构建的插件需要打包并安装到宿主，
只在容器里执行 `dnf install` 不会为宿主桌面注册输入法。

## 更新与卸载

deb、Arch 和 Fedora 原生安装包可使用设置界面的更新页；它只选择与本机匹配并通过校验的资产。
源码安装运行 `git pull --ff-only` 后重复原有构建安装步骤。Manjaro 重做本机包，NixOS 更新自己的 flake。

| 安装方式 | 卸载命令 |
|---|---|
| Debian / Ubuntu | `sudo apt remove archtype` |
| Arch / Manjaro | `sudo pacman -R archtype` |
| Fedora | `sudo dnf remove archtype` |
| Fedora Atomic | `rpm-ostree uninstall archtype`，然后重启 |
| NixOS / Home Manager | 从 `fcitx5.addons` 移除 ArchType，再重建配置 |
| 通用源码 | 在原仓库中检查 `build/install_manifest.txt` 后运行下方命令 |

源码安装的卸载：

```bash
sudo xargs -d '\n' -r rm -v -- < build/install_manifest.txt
sudo rm -f /usr/bin/archtype-check /usr/bin/archtype-show
```

如果使用 `build-text` 等其他构建目录，应使用对应的 `install_manifest.txt`。
卸载保留用户词库、学习记录和配置。随后在 Fcitx 5 配置中移除 ArchType，再重启输入法。

## 官方参考

- [Arch 的 Fcitx 5 包与开发文件](https://archlinux.org/packages/extra/x86_64/fcitx5/)
- [Manjaro 软件源分支](https://wiki.manjaro.org/index.php/Switching_Branches)
- [Fedora 的 Fcitx 5 开发包](https://packages.fedoraproject.org/pkgs/fcitx5/fcitx5-devel/)与 [GTK 前端](https://packages.fedoraproject.org/pkgs/fcitx5-gtk/fcitx5-gtk3/)
- [NSH（设置界面框架）](https://nsh.dev/)
- libadwaita 1.5 起提供的 [AlertDialog](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/class.AlertDialog.html) 和 [AboutDialog](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/class.AboutDialog.html)
- [Fcitx 5 启动与会话设置](https://fcitx-im.org/wiki/Setup_Fcitx_5)和 [Wayland 配置](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland/en)
