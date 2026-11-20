#!/usr/bin/env bash
# ArchType 一键安装：Ubuntu / Debian、Fedora（x86_64、arm64），Arch Linux、deepin（x86_64）。
#
#   curl -fsSL https://raw.githubusercontent.com/Goway-Hui/ArchType/main/install.sh | bash
#
# 做这几件事：
#   1. 从 GitHub Release 下载和你系统对应的最新安装包（词库已经在包里），按 SHA256SUMS 校验
#   2. 用 apt-get / pacman / dnf 安装（含 fcitx5、GTK 4、WebKitGTK、GStreamer）
#   3. 输入法框架不是 fcitx5 时，用 im-config 切到 fcitx5（之后要注销重新登录一次）
#   4. 把 ArchType 加进 fcitx5 的输入法列表（fcitx5 正在运行时会先重启它一次）
#   3、4 两步是 archtype-config --setup 做的（直接装包的用户在「ArchType 设置」第一页点「一键设置」也是它）
#
# 再跑一遍就是升级到最新版。发行版和源码安装说明见 docs/INSTALL.md。
#
# 环境变量：
#   ARCHTYPE_VERSION=v0.3.8   装指定版本（默认最新）
#   ARCHTYPE_REPO=owner/repo  指定发布仓库（默认 Goway-Hui/ArchType）
#   ARCHTYPE_NO_IM_CONFIG=1   不改输入法框架
#   ARCHTYPE_NO_RESTART=1     不重启 fcitx5（之后自己重启或注销再登录，再到设置里添加 ArchType）
#   ARCHTYPE_YES=1            不问，直接按默认做
set -euo pipefail

REPO=${ARCHTYPE_REPO:-Goway-Hui/ArchType}

say() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m注意：\033[0m%s\n' "$*" >&2; }
die() { printf '\033[1;31m出错：\033[0m%s\n' "$*" >&2; exit 1; }

# 问一句，默认「是」；curl | bash 时从终端读
confirm() {
  [[ -n "${ARCHTYPE_YES:-}" ]] && return 0
  local answer=""
  if [[ -r /dev/tty ]] && { true </dev/tty; } 2>/dev/null; then
    read -r -p "$1 [Y/n] " answer </dev/tty || answer=""
  else
    return 0
  fi
  [[ -z "$answer" || "$answer" =~ ^[Yy是] ]]
}

fetch() { # <地址> <保存到>
  if command -v curl >/dev/null; then
    curl -fL --retry 3 --progress-bar -o "$2" "$1"
  else
    wget -q --show-progress -O "$2" "$1"
  fi
}

latest_tag() {
  local url
  if command -v curl >/dev/null; then
    url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")
  else
    url=$(wget -q --max-redirect=5 -S --spider "https://github.com/$REPO/releases/latest" 2>&1 |
          sed -n 's/^ *[Ll]ocation: *//p' | tail -1)
  fi
  url=${url%$'\r'}
  [[ "$url" == */tag/* ]] || die "查不到最新版本（GitHub 连不上？）。可以到 https://github.com/$REPO/releases 手动下载"
  echo "${url##*/tag/}"
}

usage() {
  cat <<'EOF'
用法：bash install.sh [--print-plan | --help]

默认下载、校验并安装匹配当前发行版的 ArchType 包。
--print-plan  只显示发行版、包管理器和包名，不联网、不安装、不更改配置。

支持 Ubuntu 24.04 / 26.04、Debian 13 及有明确兼容基线的衍生版、deepin 25，
Arch Linux / EndeavourOS / Garuda / CachyOS，Fedora；预编译包有 x86_64，
Ubuntu、Debian 的 deb 和 rpm 另有 arm64（aarch64）。
Manjaro 请在本机用 scripts/package-arch.sh 构建；NixOS 使用 flake。
Fedora Atomic 桌面使用 rpm-ostree。其他发行版见 docs/INSTALL.md 的源码构建。

环境变量：ARCHTYPE_VERSION、ARCHTYPE_REPO、ARCHTYPE_YES、ARCHTYPE_NO_IM_CONFIG、
          ARCHTYPE_NO_RESTART。
测试夹具：ARCHTYPE_SYSTEM 代替 uname -s（固定 Linux），
          ARCHTYPE_OS_RELEASE 指定 os-release 文件，ARCHTYPE_ARCH 指定机器架构，
          ARCHTYPE_BASE_URL 指定资产目录（可用 file:///...）。
EOF
}

# os-release 的已知字段仅作数据读取，不执行文件里的 shell 代码。
read_os_release() {
  local release=${ARCHTYPE_OS_RELEASE:-/etc/os-release} key value
  [[ -r "$release" ]] || {
    [[ -z "${ARCHTYPE_OS_RELEASE:-}" ]] && release=/usr/lib/os-release
    [[ -r "$release" ]] || die "找不到 os-release，无法选择安装包"
  }
  ID= ID_LIKE= VERSION_ID= VERSION_CODENAME= UBUNTU_CODENAME= DEBIAN_CODENAME= VARIANT_ID=
  while IFS='=' read -r key value || [[ -n "$key" ]]; do
    case "$key" in
      ID|ID_LIKE|VERSION_ID|VERSION_CODENAME|UBUNTU_CODENAME|DEBIAN_CODENAME|VARIANT_ID)
        case "$value" in
          \"*\") value=${value:1:${#value}-2} ;;
          \'*\') value=${value:1:${#value}-2} ;;
        esac
        [[ "$value" =~ ^[a-zA-Z0-9._[:space:]-]*$ ]] ||
          die "os-release 的 $key 字段格式无效"
        printf -v "$key" '%s' "$value"
        ;;
    esac
  done < "$release"
}

source_install() {
  die "$1。安装与源码构建说明：https://github.com/$REPO/blob/main/docs/INSTALL.md"
}

select_platform() {
  [[ "${ARCHTYPE_SYSTEM:-$(uname -s)}" == Linux ]] || source_install "ArchType 的 fcitx5 插件需要 Linux"
  read_os_release
  machine=${ARCHTYPE_ARCH:-$(uname -m)}
  # deb 和 rpm 对同一种架构叫法不同：amd64 / x86_64、arm64 / aarch64
  case "$machine" in
    x86_64|amd64) deb_arch=amd64; rpm_arch=x86_64 ;;
    aarch64|arm64) deb_arch=arm64; rpm_arch=aarch64 ;;
    *) source_install "当前预编译包只有 x86_64 和 arm64；$machine 请从源码构建" ;;
  esac
  local family=" $ID $ID_LIKE " codename
  # deepin 25 的磐石系统基于 OSTree，但 apt-get install 照常能用（系统转给 deepin-immutable-ctl），
  # 所以要在下面「不可变系统」那条之前认出来。deepin 20 的 ID 是大写的 Deepin。
  if [[ "$ID" == deepin || "$ID" == Deepin ]]; then
    case "$VERSION_CODENAME:$VERSION_ID" in
      crimson:*|*:25|*:25.*) distro=deepin25 ;;
      *) source_install "deepin ${VERSION_ID:-$VERSION_CODENAME} 没有匹配的预编译包（只提供 deepin 25 的；23 及更早、UOS 20 不再支持）" ;;
    esac
    [[ "$deb_arch" == amd64 ]] || source_install "deepin 的预编译包只有 x86_64；$machine 请从源码构建"
    manager=apt-get
    return
  fi
  if [[ -e /run/ostree-booted || "$VARIANT_ID" == *atomic* ]]; then
    source_install "不可变系统请按说明使用 rpm-ostree 或系统自身的包配置"
  fi
  case "$VARIANT_ID" in
    silverblue|kinoite|sericea|onyx|coreos)
      source_install "Fedora $VARIANT_ID 请按说明使用 rpm-ostree" ;;
  esac
  case "$ID" in
    nixos) source_install "NixOS 请使用仓库 flake 和 i18n.inputMethod.fcitx5.addons" ;;
    ubuntu)
      case "$VERSION_ID" in
        24.04|26.04) distro="ubuntu$VERSION_ID"; manager=apt-get ;;
        22.04) source_install "Ubuntu 22.04 已不再提供安装包（它的 GTK 4.6 太旧）；请升级到 24.04，或按安装说明从源码构建" ;;
        *) source_install "Ubuntu $VERSION_ID 没有匹配的预编译包" ;;
      esac
      ;;
    debian)
      case "$VERSION_ID" in
        13|13.*) distro=debian13; manager=apt-get ;;
        *) source_install "Debian ${VERSION_ID:-$VERSION_CODENAME} 没有匹配的预编译包" ;;
      esac
      ;;
    fedora)
      [[ "$VERSION_ID" =~ ^[0-9]+$ ]] || source_install "无法识别 Fedora 版本"
      distro="fedora$VERSION_ID"; manager=dnf
      ;;
    *)
      if [[ "$family" == *" manjaro "* ]]; then
        source_install "Manjaro 使用独立软件源，请在本机运行 scripts/package-arch.sh 构建，避免 Arch 二进制 ABI 不匹配"
      elif [[ "$family" == *" arch "* || "$ID" == archlinux || "$ID" == endeavouros || "$ID" == garuda || "$ID" == cachyos ]]; then
        [[ "$rpm_arch" == x86_64 ]] ||
          source_install "Arch Linux 的预编译包只有 x86_64；$machine 请按说明用 scripts/package-arch.sh 在本机构建"
        distro=archlinux; manager=pacman
      elif [[ -n "$UBUNTU_CODENAME" || "$family" == *" ubuntu "* ]]; then
        codename=${UBUNTU_CODENAME:-$VERSION_CODENAME}
        case "$codename" in
          noble) distro=ubuntu24.04; manager=apt-get ;;
          resolute) distro=ubuntu26.04; manager=apt-get ;;
          *) source_install "$ID 的 Ubuntu 基线 ${codename:-未知} 没有匹配的预编译包" ;;
        esac
      elif [[ -n "$DEBIAN_CODENAME" || "$family" == *" debian "* ]]; then
        codename=${DEBIAN_CODENAME:-$VERSION_CODENAME}
        [[ "$codename" == trixie ]] || source_install "$ID 的 Debian 基线 ${codename:-未知} 没有匹配的预编译包"
        distro=debian13; manager=apt-get
      else
        source_install "$ID 暂无匹配的预编译包，请使用发行版对应的源码构建命令"
      fi
      ;;
  esac
}

package_filename() {
  case "$manager" in
    apt-get) printf 'archtype_%s_%s_%s.deb\n' "$1" "$distro" "$deb_arch" ;;
    pacman) printf 'archtype-%s-1-x86_64.pkg.tar.zst\n' "$1" ;;
    dnf) printf 'archtype-%s-1.fc%s.%s.rpm\n' "$1" "$VERSION_ID" "$rpm_arch" ;;
  esac
}

# 只提取选定文件的校验值；重复、缺失或无效条目一律拒绝。
manifest_hash() {
  local wanted=$1 manifest=$2 hash filename found= count=0
  while read -r hash filename || [[ -n "$hash" ]]; do
    filename=${filename#\*}
    [[ "$filename" == "$wanted" ]] || continue
    [[ "$hash" =~ ^[[:xdigit:]]{64}$ ]] || return 1
    found=$hash
    ((count += 1))
  done < "$manifest"
  [[ "$count" == 1 ]] || return 1
  printf '%s\n' "$found"
}

# 要用户确认的命令从终端读输入（curl | bash 时标准输入是脚本本身）；没有终端就照常
from_tty() {
  if [[ -z "${ARCHTYPE_YES:-}" ]] && { : < /dev/tty; } 2>/dev/null; then
    "$@" < /dev/tty
  else
    "$@"
  fi
}

install_packages() {
  local -a paths=("${files[@]/#/$tmp/}") yes=()
  [[ -n "${ARCHTYPE_YES:-}" ]] && yes=(--noconfirm)
  case "$manager" in
    apt-get)
      local -a extra
      case "$distro" in
        # deepin 自带 fcitx5 和控制中心里的输入法设置，不装 im-config、fcitx5-config-qt
        deepin*) extra=(fcitx5-frontend-all) ;;
        *) extra=(fcitx5-config-qt fcitx5-frontend-all im-config) ;;
      esac
      "${SUDO[@]}" apt-get update -qq || warn "apt-get update 失败，接着用现有的软件源列表安装"
      "${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y "${paths[@]}" \
        "${extra[@]}" libgtk-4-1 libwebkitgtk-6.0-4 libgstreamer1.0-0 libgstreamer-plugins-base1.0-0
      ;;
    pacman)
      # 不替用户 -Syu（整机升级该由用户自己决定）；-S 不带 y 不会造成部分升级。
      # 依赖装不上多半是软件源太旧：先 sudo pacman -Syu 再重跑。
      # curl | bash 时标准输入是脚本本身，pacman 的确认要从终端读。
      from_tty "${SUDO[@]}" pacman -S --needed "${yes[@]}" fcitx5 fcitx5-gtk fcitx5-qt \
        fcitx5-configtool gtk4 webkitgtk-6.0 gstreamer gst-plugins-base ||
        die "依赖装不上：先运行 sudo pacman -Syu 升级系统，再重新安装"
      from_tty "${SUDO[@]}" pacman -U "${yes[@]}" "${paths[@]}"
      ;;
    dnf)
      "${SUDO[@]}" dnf install -y "${paths[@]}" \
        fcitx5-configtool fcitx5-gtk3 fcitx5-gtk4 fcitx5-qt5 fcitx5-qt6 fcitx5-autostart \
        gtk4 webkitgtk6.0 gstreamer1 gstreamer1-plugins-base
      ;;
  esac
}

# GNOME 的候选框靠扩展 Input Method Panel（kimpanel@kde.org）画，活动概览的搜索框里才看得到。
# 发行版仓库里有它的（Debian 12 / 13 的 gnome-shell-extension-kimpanel，Fedora 45 起）就问一句装上并启用；
# 仓库里没有的（Ubuntu、Arch 官方仓库、更早的 Fedora）照旧只给链接。有没有按仓库实际查，不按发行版猜。
KIMPANEL_UUID=kimpanel@kde.org
KIMPANEL_PKG=gnome-shell-extension-kimpanel
KIMPANEL_URL=https://extensions.gnome.org/extension/261/kimpanel/
kimpanel_in_repo() {
  case "$manager" in
    apt-get) apt-cache show "$KIMPANEL_PKG" >/dev/null 2>&1 ;;
    dnf) dnf -q info "$KIMPANEL_PKG" >/dev/null 2>&1 ;;
    *) return 1 ;;
  esac
}
enable_kimpanel() {
  gnome-extensions enable "$KIMPANEL_UUID" 2>/dev/null && return 0
  # 刚装的扩展 GNOME Shell 还没扫到（Wayland 下要重新登录），enable 会失败：直接记进启用列表，登录后生效
  command -v gsettings >/dev/null || return 1
  local current
  current=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null) || return 1
  [[ "$current" == *"'$KIMPANEL_UUID'"* ]] && return 0
  if [[ "$current" == "@as []" || "$current" == "[]" ]]; then
    current="['$KIMPANEL_UUID']"
  else
    current="${current%]}, '$KIMPANEL_UUID']"
  fi
  gsettings set org.gnome.shell enabled-extensions "$current"
}
gnome_kimpanel() {
  [[ "${XDG_CURRENT_DESKTOP:-}" == *GNOME* ]] && command -v gnome-extensions >/dev/null || return 0
  local installed
  installed=$(gnome-extensions list 2>/dev/null) || return 0
  if grep -qx "$KIMPANEL_UUID" <<<"$installed"; then
    gnome-extensions list --enabled 2>/dev/null | grep -qx "$KIMPANEL_UUID" && return 0
    if confirm "GNOME 扩展 Input Method Panel 装了但没启用（候选框要靠它显示），现在启用吗？"; then
      enable_kimpanel && say "已启用 GNOME 扩展 Input Method Panel" ||
        warn "没启用成功，请在「扩展」应用里打开 Input Method Panel"
    fi
    return 0
  fi
  if kimpanel_in_repo &&
     confirm "GNOME 下候选框要靠扩展 Input Method Panel 显示，现在装上（${KIMPANEL_PKG}）并启用吗？"; then
    local ok=""
    case "$manager" in
      apt-get) "${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y "$KIMPANEL_PKG" && ok=1 ;;
      dnf) "${SUDO[@]}" dnf install -y "$KIMPANEL_PKG" && ok=1 ;;
    esac
    if [[ -n "$ok" ]] && enable_kimpanel; then
      say "已装上并启用 GNOME 扩展 Input Method Panel（注销重新登录后生效）"
      relogin=1
      return 0
    fi
    warn "Input Method Panel 没装好，可以从 ${KIMPANEL_URL} 安装"
    return 0
  fi
  warn "GNOME 下建议装扩展 Input Method Panel（${KIMPANEL_URL}），活动概览的搜索框里才能显示候选框"
}

main() {
  local plan=0
  case "${1:-}" in
    --help|-h) usage; return ;;
    --print-plan) plan=1 ;;
    '') ;;
    *) die "未知参数：$1（用 --help 查看用法）" ;;
  esac
  [[ $# -le 1 ]] || die "参数过多（用 --help 查看用法）"
  [[ "$REPO" =~ ^[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+$ ]] || die "ARCHTYPE_REPO 必须是 owner/repo"
  select_platform
  if ((plan)); then
    local requested=${ARCHTYPE_VERSION:-<版本>}
    printf '系统：%s %s\n包基线：%s\n包管理器：%s\n仓库：%s\n包文件：%s\n' \
      "$ID" "$VERSION_ID" "$distro" "$manager" "$REPO" \
      "$(package_filename "${requested#v}")"
    return
  fi
  if [[ $EUID -eq 0 ]]; then
    [[ -n "${SUDO_USER:-}" ]] && die "请直接用普通用户运行，不要加 sudo（需要时脚本会自己调 sudo）"
    SUDO=()
  else
    command -v sudo >/dev/null || die "需要 sudo 来安装软件包"
    SUDO=(sudo)
  fi
  command -v "$manager" >/dev/null || die "找不到当前发行版的包管理器 $manager"
  command -v curl >/dev/null || command -v wget >/dev/null || die "需要 curl 或 wget，请先用发行版的包管理器安装"
  command -v sha256sum >/dev/null || die "需要 sha256sum（coreutils）校验安装包"

  local tag version package base
  tag=${ARCHTYPE_VERSION:-$(latest_tag)}
  [[ "$tag" == v* ]] || tag="v$tag"
  version=${tag#v}
  [[ "$version" =~ ^[a-zA-Z0-9][a-zA-Z0-9._+~^-]*$ ]] || die "无效的版本号：$version"
  base=${ARCHTYPE_BASE_URL:-"https://github.com/$REPO/releases/download/$tag"}
  package=$(package_filename "$version")

  tmp=$(mktemp -d)   # 退出时的 trap 还要用它
  trap 'rm -rf "${tmp:-}"' EXIT
  chmod 755 "$tmp"   # apt 用 _apt 用户读本地包

  say "ArchType ${version}（$ID → ${distro}）"
  fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" 2>/dev/null ||
    die "下载 $base/SHA256SUMS 失败，请检查版本和网络；发布页：https://github.com/$REPO/releases"
  manifest_hash "$package" "$tmp/SHA256SUMS" >/dev/null ||
    source_install "ArchType $version 没有 $distro 的有效安装包（${package}）"
  files=("$package")
  # 0.3.3 及以前的 deb 词库是单独的包；其他格式不安装 deb 数据包。
  local hash filename data=
  if [[ "$manager" == apt-get ]]; then
    while read -r hash filename || [[ -n "$hash" ]]; do
      filename=${filename#\*}
      if [[ "$filename" =~ ^archtype-data-rime-ice_[a-zA-Z0-9._+~^-]+_all\.deb$ ]]; then
        [[ -z "$data" ]] || die "SHA256SUMS 包含多个词库包"
        data=$filename
      fi
    done < "$tmp/SHA256SUMS"
    [[ -z "$data" ]] || files+=("$data")
  fi
  local f expected actual
  for f in "${files[@]}"; do
    expected=$(manifest_hash "$f" "$tmp/SHA256SUMS") || die "$f 缺少唯一且有效的 SHA256"
    say "下载并校验 $f"
    fetch "$base/$f" "$tmp/$f"
    actual=$(sha256sum "$tmp/$f")
    [[ "${actual%% *}" == "${expected,,}" ]] || die "$f 校验和对不上，请重新下载"
    chmod 644 "$tmp/$f"
  done

  say "安装（需要时会要求输入密码）"
  install_packages

  # 旧品牌 ArchType 的包如果还装着，输入法列表里会多一个用不了的 ArchType；只提示，不替用户卸
  if [[ -e /usr/share/fcitx5/addon/archtype.conf || -e /usr/lib/fcitx5/libarchtype.so ]]; then
    warn "检测到旧品牌 ArchType 的安装文件。建议卸载旧包（pacman -R archtype / apt remove archtype / dnf remove archtype），用户词库和设置会自动迁移到 ArchType"
  fi

  # 输入法框架、输入法列表交给 archtype-config --setup（直接装包的用户在设置第一页点「一键设置」是同一套）：
  #   fcitx5 没在跑：输入法框架不是 fcitx5 时问一句再 im-config -n fcitx5，写好 fcitx5 的输入法列表，要注销重新登录
  #   在跑但还没载入 ArchType：问一句再重启 fcitx5，加进输入法列表；在跑也认得：直接加
  # 问话从终端读（from_tty）；ARCHTYPE_YES / ARCHTYPE_NO_IM_CONFIG / ARCHTYPE_NO_RESTART 照旧。
  # deepin 默认就是 fcitx5（经 deepin-im），不去动输入法框架
  local relogin="" restarted="" code=0
  local -a setup=(--setup)
  [[ -n "${ARCHTYPE_YES:-}" ]] && setup+=(--yes)
  [[ -n "${ARCHTYPE_NO_IM_CONFIG:-}" || "$distro" == deepin* ]] && setup+=(--no-im-config)
  [[ -n "${ARCHTYPE_NO_RESTART:-}" ]] && setup+=(--no-restart)
  if ! command -v archtype-config >/dev/null || ! archtype-config --self-test >/dev/null 2>&1; then
    warn "命令行工具 archtype-config 跑不起来，请在「Fcitx 5 配置」里手动添加 ArchType"
  else
    from_tty archtype-config "${setup[@]}" || code=$?
    case "$code" in
      0) ;;
      2) relogin=1 ;;   # 切了输入法框架或 fcitx5 没在跑
      3) restarted=1 ;; # 刚重启了 fcitx5
      4) ;;             # 用户没答应或不让重启，archtype-config 已经说了还差哪一步
      *) warn "没设置好，请在「ArchType 设置」第一页点「一键设置」" ;;
    esac
  fi

  gnome_kimpanel

  echo
  say "ArchType $version 装好了"
  if [[ -n "$relogin" ]]; then
    echo "    注销并重新登录，然后按 Ctrl+空格 切换到 ArchType，打 nihao 试试。"
  else
    echo "    按 Ctrl+空格 切换到 ArchType，打 nihao 试试。"
  fi
  [[ -n "$restarted" ]] &&
    echo "    刚重启了 fcitx5：Chrome、VS Code、微信等 Chromium/Electron 程序要彻底退出再打开才能打中文。"
  echo "    设置在应用菜单「ArchType 设置」里；以后有新版本会自动提醒。"
}

main "$@"
