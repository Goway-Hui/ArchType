#!/usr/bin/env bash
# 无网络、无 root、无真实包管理操作的安装器回归测试。
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/assets"

# 拦截所有可能修改系统/输入法配置的命令；curl 只读取本地夹具。
cat > "$work/bin/mock" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
name=${0##*/}
case "$name" in
  sudo) exec "$@" ;;
  curl)
    output= url=
    while (($#)); do
      case "$1" in
        -o) output=$2; shift 2 ;;
        *) url=$1; shift ;;
      esac
    done
    printf 'curl %s\n' "$url" >> "$FIXTURE_FETCH_LOG"
    case "$url" in
      file://*) cp -- "${url#file://}" "$output" ;;
      https://github.com/Goway-Hui/ArchType/releases/latest)
        printf '%s\n' https://github.com/Goway-Hui/ArchType/releases/tag/v0.3.8 ;;
      *) printf '测试禁止联网：%s\n' "$url" >&2; exit 1 ;;
    esac
    ;;
  pgrep) exit 1 ;;
  # GNOME 扩展：FIXTURE_EXTENSIONS / FIXTURE_ENABLED 是装了的、启用了的；FIXTURE_KIMPANEL=1 时软件源里有 kimpanel
  gnome-extensions)
    case "${1:-}" in
      list) if [[ "${2:-}" == --enabled ]]; then printf '%s' "${FIXTURE_ENABLED:-}"; else printf '%s' "${FIXTURE_EXTENSIONS:-}"; fi ;;
      *) printf 'gnome-extensions %s\n' "$*" >> "$FIXTURE_LOG"; exit "${FIXTURE_ENABLE_EXIT:-0}" ;;
    esac
    ;;
  apt-cache) [[ -n "${FIXTURE_KIMPANEL:-}" ]] ;;
  gsettings)
    printf 'gsettings %s\n' "$*" >> "$FIXTURE_LOG"
    [[ "${1:-}" != get ]] || printf '%s\n' "${FIXTURE_GSETTINGS:-@as []}"
    ;;
  im-config) [[ "${1:-}" != -m ]] || printf 'fcitx5\nfcitx5\n' ;;
  apt-get|pacman|dnf|archtype-config|installer)
    printf '%s' "$name" >> "$FIXTURE_LOG"
    printf ' %q' "$@" >> "$FIXTURE_LOG"
    printf '\n' >> "$FIXTURE_LOG"
    # archtype-config --setup 的退出码：0 好了、2 要注销重新登录、3 刚重启了 fcitx5、4 还差一步、1 出错
    [[ "$name $*" != "archtype-config --setup"* ]] || exit "${FIXTURE_SETUP_EXIT:-0}"
    ;;
  *) printf '意外命令：%s\n' "$name" >&2; exit 1 ;;
esac
MOCK
chmod +x "$work/bin/mock"
for name in sudo curl apt-get pacman dnf pgrep im-config archtype-config installer gnome-extensions apt-cache gsettings; do
  ln -s mock "$work/bin/$name"
done

test_arch=x86_64
test_version=v0.3.8
test_system=Linux
test_no_switch=1
test_desktop=
count=0
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
pass() { count=$((count + 1)); printf 'ok %s - %s\n' "$count" "$*"; }
# macOS 自带的 bash 3.2 在 UTF-8 下把变量名后面紧跟的全角字符（比如全角括号）当成变量名的一部分（set -u 时直接报
# unbound variable）。变量后面紧跟非 ASCII 字符的一律要写成 ${version}。CI 的 bash 5 复现不了，只能这样查。
bad=$(cd "$root" && git ls-files -z '*.sh' | xargs -0 perl -ne 'print "$ARGV:$.: $_" if /\$[A-Za-z_]\w*[^\x00-\x7f]/; close ARGV if eof' || true)
[[ -z "$bad" ]] || fail "变量后面紧跟中文要加花括号（macOS 的 bash 3.2 会报 unbound variable）：
$bad"

run() {
  env -u SUDO_USER PATH="$work/bin:$PATH" \
    FIXTURE_LOG="$work/commands" FIXTURE_FETCH_LOG="$work/fetches" \
    ARCHTYPE_SYSTEM="$test_system" ARCHTYPE_OS_RELEASE="$work/os-release" ARCHTYPE_ARCH="$test_arch" \
    ARCHTYPE_VERSION="$test_version" ARCHTYPE_REPO=Goway-Hui/ArchType \
    ARCHTYPE_BASE_URL="file://$work/assets" ARCHTYPE_YES=1 \
    ARCHTYPE_NO_IM_CONFIG="$test_no_switch" ARCHTYPE_NO_RESTART="$test_no_switch" XDG_CURRENT_DESKTOP="$test_desktop" \
    bash "$root/install.sh" "$@"
}
reset() {
  : > "$work/commands"
  : > "$work/fetches"
  rm -f "$work/assets/"*
  test_arch=x86_64
  test_version=v0.3.8
  test_system=Linux
  test_no_switch=1
  test_desktop=
  unset FIXTURE_SETUP_EXIT FIXTURE_EXTENSIONS FIXTURE_ENABLED FIXTURE_KIMPANEL FIXTURE_ENABLE_EXIT FIXTURE_GSETTINGS
  printf '%s\n' "$1" > "$work/os-release"
}
asset() {
  printf 'ArchType fixture for %s\n' "$1" > "$work/assets/$1"
  (cd "$work/assets" && sha256sum "$1") >> "$work/assets/SHA256SUMS"
}
plan() {
  reset "$1"
  local result
  result=$(run --print-plan) || fail "计划失败：$1"
  [[ "$result" == *"$2"* ]] || fail "包选择错误：$result"
  [[ "$result" == *"包管理器：$3"* ]] || fail "包管理器错误：$result"
  [[ ! -s "$work/commands" && ! -s "$work/fetches" ]] || fail '--print-plan 有副作用'
  pass "$4"
}
rejected() {
  reset "$1"
  if run --print-plan > "$work/output" 2>&1; then fail "不应支持：$1"; fi
  grep -Fq "$2" "$work/output" || fail "缺少有效指引：$(cat "$work/output")"
  [[ ! -s "$work/commands" && ! -s "$work/fetches" ]] || fail '拒绝平台时有副作用'
  pass "$3"
}
assert_log() { grep -Fq -- "$1" "$work/commands" || fail "缺少命令：$1"; }
assert_no_install() {
  if grep -Eq '^(apt-get|pacman|dnf|installer) ' "$work/commands"; then fail '校验失败仍运行包管理器'; fi
}

rejected $'ID=ubuntu\nVERSION_ID="22.04"' 'Ubuntu 22.04 已不再提供安装包' 'Ubuntu 22.04 is refused'
rejected $'ID=linuxmint\nVERSION_ID="21.3"\nID_LIKE="ubuntu debian"\nUBUNTU_CODENAME=jammy' '没有匹配的预编译包' 'Linux Mint 21 (jammy) is refused'
plan $'ID=ubuntu\nVERSION_ID="24.04"' archtype_0.3.8_ubuntu24.04_amd64.deb apt-get 'Ubuntu 24.04'
plan $'ID=deepin\nVERSION_ID="25"\nVERSION_CODENAME=crimson' archtype_0.3.8_deepin25_amd64.deb apt-get 'deepin 25'
plan $'ID=deepin\nVERSION_ID="25.0.9"' archtype_0.3.8_deepin25_amd64.deb apt-get 'deepin 25 point release'
rejected $'ID=deepin\nVERSION_ID="23"\nVERSION_CODENAME=beige' '只提供 deepin 25 的' 'deepin 23 is refused'
plan $'ID=ubuntu\nVERSION_ID="26.04"' archtype_0.3.8_ubuntu26.04_amd64.deb apt-get 'Ubuntu 26.04'
plan $'ID=debian\nVERSION_ID="13.1"' archtype_0.3.8_debian13_amd64.deb apt-get 'Debian 13 point release'
plan $'ID=linuxmint\nVERSION_ID="22.1"\nID_LIKE="ubuntu debian"\nUBUNTU_CODENAME=noble' ubuntu24.04 apt-get 'Linux Mint uses Ubuntu base'
plan $'ID=pop\nVERSION_ID="24.04"\nID_LIKE="ubuntu debian"\nVERSION_CODENAME=noble' ubuntu24.04 apt-get 'Pop!_OS uses explicit base'
plan $'ID=example\nID_LIKE=debian\nDEBIAN_CODENAME=trixie' debian13 apt-get 'Debian derivative with explicit trixie base'
plan 'ID=arch' archtype-0.3.8-1-x86_64.pkg.tar.zst pacman 'Arch Linux'
plan $'ID=endeavouros\nID_LIKE=arch' pkg.tar.zst pacman 'EndeavourOS'
plan $'ID=garuda\nID_LIKE=arch' pkg.tar.zst pacman 'Garuda'
plan $'ID=cachyos\nID_LIKE=arch' pkg.tar.zst pacman 'CachyOS'
plan $'ID=fedora\nVERSION_ID=43' archtype-0.3.8-1.fc43.x86_64.rpm dnf 'Fedora version-specific RPM'
rejected $'ID=ubuntu\nVERSION_ID=20.04' '源码' 'Ubuntu 20.04 is not silently upgraded to a newer binary'
rejected $'ID=Deepin\nVERSION_ID="20.8"\nVERSION_CODENAME="apricot"' '不再支持' 'deepin 20 rejected with reason'
rejected $'ID=linuxmint\nVERSION_ID=22\nID_LIKE="ubuntu debian"' '基线' 'Unknown derivative base rejected'
rejected $'ID=manjaro\nID_LIKE=arch' 'package-arch.sh' 'Manjaro uses native source package'
rejected $'ID=example\nID_LIKE="arch manjaro"' 'package-arch.sh' 'Manjaro derivative rejected'
rejected $'ID=fedora\nVERSION_ID=43\nVARIANT_ID=silverblue' 'rpm-ostree' 'Silverblue uses rpm-ostree'
rejected $'ID=fedora\nVERSION_ID=43\nVARIANT_ID=sway-atomic' 'rpm-ostree' 'Fedora Atomic uses rpm-ostree'
rejected 'ID=nixos' 'flake' 'NixOS uses flake'
rejected $'ID=opensuse-tumbleweed\nID_LIKE=suse' '源码' 'openSUSE source guidance'
rejected $'ID=rocky\nID_LIKE="rhel centos fedora"\nVERSION_ID=9' '源码' 'RHEL derivative is not treated as Fedora'
rejected 'ID=alpine' '源码' 'Alpine source guidance'

# arm64：deb 和 rpm 有预编译包，Arch 没有
arm_plan() { # <os-release> <uname -m> <期望包名> <说明>
  reset "$1"
  test_arch=$2
  local result
  result=$(run --print-plan) || fail "计划失败：$1 $2"
  [[ "$result" == *"$3"* ]] || fail "包选择错误：$result"
  pass "$4"
}
arm_plan $'ID=ubuntu\nVERSION_ID="24.04"' aarch64 archtype_0.3.8_ubuntu24.04_arm64.deb 'Ubuntu 24.04 arm64'
arm_plan $'ID=debian\nVERSION_ID="13"' arm64 archtype_0.3.8_debian13_arm64.deb 'Debian 13 arm64 (uname arm64)'
arm_plan $'ID=fedora\nVERSION_ID=44' aarch64 archtype-0.3.8-1.fc44.aarch64.rpm 'Fedora aarch64 RPM'

reset $'ID=deepin\nVERSION_ID="25"'
test_arch=aarch64
if run --print-plan > "$work/output" 2>&1; then fail 'Unexpected aarch64 deepin binary'; fi
grep -Fq 'x86_64' "$work/output" || fail 'Missing deepin arch guidance'
pass 'deepin aarch64 builds from source'

reset 'ID=arch'
test_arch=aarch64
if run --print-plan > "$work/output" 2>&1; then fail 'Unexpected aarch64 Arch binary'; fi
grep -Fq 'package-arch.sh' "$work/output" || fail 'Missing Arch aarch64 guidance'
pass 'Arch aarch64 builds locally'

reset $'ID=ubuntu\nVERSION_ID="24.04"'
test_arch=riscv64
if run --print-plan > "$work/output" 2>&1; then fail 'Unexpected riscv64 binary'; fi
grep -Fq '源码' "$work/output" || fail 'Missing architecture guidance'
pass 'Architecture mismatch rejected'

reset "ID=\$(touch $work/unexpected)"
if run --print-plan > "$work/output" 2>&1; then fail 'Unsafe os-release accepted'; fi
[[ ! -e "$work/unexpected" ]] || fail 'os-release shell code executed'
pass 'os-release is parsed as data'

reset 'ID=arch'
test_version=
result=$(run --print-plan)
[[ "$result" == *'archtype-<版本>-1-x86_64.pkg.tar.zst'* && ! -s "$work/fetches" ]] || fail 'Default plan needs network'
pass 'Plan with unspecified version stays offline'

reset $'ID=ubuntu\nVERSION_ID=24.04'
asset archtype_0.3.8_ubuntu24.04_amd64.deb
asset archtype-data-rime-ice_20260101_all.deb
run > "$work/output" 2>&1 || fail "apt install: $(cat "$work/output")"
assert_log 'apt-get update -qq'
assert_log 'apt-get install -y'
assert_log 'fcitx5-config-qt fcitx5-frontend-all im-config'
assert_log 'libgtk-4-1 libwebkitgtk-6.0-4 libgstreamer1.0-0 libgstreamer-plugins-base1.0-0'
assert_log 'archtype_0.3.8_ubuntu24.04_amd64.deb'
assert_log 'archtype-data-rime-ice_20260101_all.deb'
assert_log 'archtype-config --setup --yes --no-im-config --no-restart'
pass 'APT installs verified local packages and legacy data'

reset $'ID=deepin\nVERSION_ID=25\nVERSION_CODENAME=crimson'
asset archtype_0.3.8_deepin25_amd64.deb
run > "$work/output" 2>&1 || fail "apt install deepin: $(cat "$work/output")"
assert_log 'archtype_0.3.8_deepin25_amd64.deb'
assert_log 'fcitx5-frontend-all libgtk-4-1'
if grep -Eq '(^| )im-config( |$)|fcitx5-config-qt' "$work/commands"; then fail 'deepin 不该装 im-config、fcitx5-config-qt'; fi
assert_log 'archtype-config --setup --yes --no-im-config --no-restart'
pass 'deepin keeps its own IM framework and config tool'

# 输入法框架、输入法列表交给 archtype-config --setup：不设 ARCHTYPE_NO_* 时不带对应的选项，退出码决定最后的提示
reset $'ID=ubuntu\nVERSION_ID=24.04'
asset archtype_0.3.8_ubuntu24.04_amd64.deb
test_no_switch=
export FIXTURE_SETUP_EXIT=2
run > "$work/output" 2>&1 || fail "setup relogin: $(cat "$work/output")"
grep -Eq '^archtype-config --setup --yes$' "$work/commands" || fail "--setup 的选项不对：$(cat "$work/commands")"
grep -Fq '注销并重新登录' "$work/output" || fail "缺少注销提示：$(cat "$work/output")"
FIXTURE_SETUP_EXIT=3 run > "$work/output" 2>&1 || fail "setup restarted: $(cat "$work/output")"
grep -Fq 'Chromium/Electron' "$work/output" || fail "缺少重启提示：$(cat "$work/output")"
FIXTURE_SETUP_EXIT=1 run > "$work/output" 2>&1 || fail "setup failed 时安装不该失败：$(cat "$work/output")"
grep -Fq '一键设置' "$work/output" || fail "缺少一键设置提示：$(cat "$work/output")"
pass 'Setup is delegated to archtype-config --setup'

reset $'ID=deepin\nVERSION_ID=25\nVERSION_CODENAME=crimson'
asset archtype_0.3.8_deepin25_amd64.deb
test_no_switch=
run > "$work/output" 2>&1 || fail "deepin setup: $(cat "$work/output")"
grep -Eq '^archtype-config --setup --yes --no-im-config$' "$work/commands" || fail "deepin 不该切输入法框架：$(cat "$work/commands")"
pass 'deepin setup never switches the IM framework'

# GNOME：软件源里有 kimpanel 就装上并启用；没有的只给链接；装了没启用的只启用
reset $'ID=debian\nVERSION_ID=13'
asset archtype_0.3.8_debian13_amd64.deb
test_desktop=ubuntu:GNOME
export FIXTURE_KIMPANEL=1 FIXTURE_ENABLE_EXIT=1 FIXTURE_GSETTINGS="['ding@rastersoft.com']"
run > "$work/output" 2>&1 || fail "kimpanel: $(cat "$work/output")"
assert_log 'apt-get install -y gnome-shell-extension-kimpanel'
assert_log 'gnome-extensions enable kimpanel@kde.org'
assert_log "gsettings set org.gnome.shell enabled-extensions ['ding@rastersoft.com', 'kimpanel@kde.org']"
grep -Fq '注销并重新登录' "$work/output" || fail "装了扩展要提示重新登录：$(cat "$work/output")"
pass 'GNOME installs and enables kimpanel from the distro repository'

reset $'ID=ubuntu\nVERSION_ID=24.04'
asset archtype_0.3.8_ubuntu24.04_amd64.deb
test_desktop=ubuntu:GNOME
run > "$work/output" 2>&1 || fail "kimpanel link: $(cat "$work/output")"
if grep -q 'kimpanel' "$work/commands"; then fail "软件源里没有 kimpanel 时不该装：$(cat "$work/commands")"; fi
grep -Fq 'extensions.gnome.org/extension/261' "$work/output" || fail "缺少扩展链接：$(cat "$work/output")"
pass 'GNOME without a kimpanel package keeps the link'

reset $'ID=ubuntu\nVERSION_ID=24.04'
asset archtype_0.3.8_ubuntu24.04_amd64.deb
test_desktop=GNOME
export FIXTURE_EXTENSIONS=$'kimpanel@kde.org\n'
run > "$work/output" 2>&1 || fail "kimpanel enable: $(cat "$work/output")"
assert_log 'gnome-extensions enable kimpanel@kde.org'
if grep -q 'apt-get install -y gnome-shell' "$work/commands"; then fail '装了的扩展不该再装'; fi
export FIXTURE_ENABLED=$'kimpanel@kde.org\n'
: > "$work/commands"
run > "$work/output" 2>&1 || fail "kimpanel enabled: $(cat "$work/output")"
if grep -q 'gnome-extensions\|gsettings' "$work/commands"; then fail '已经启用了不该再动'; fi
pass 'GNOME only enables an installed kimpanel when needed'

reset 'ID=arch'
asset archtype-0.3.8-1-x86_64.pkg.tar.zst
asset archtype-data-rime-ice_20260101_all.deb
run > "$work/output" 2>&1 || fail "pacman install: $(cat "$work/output")"
assert_log 'pacman -S --needed --noconfirm'
if grep -q 'pacman -Syu' "$work/commands"; then fail 'installer ran a full system upgrade'; fi
assert_log 'gtk4 webkitgtk-6.0 gstreamer'
assert_log 'pacman -U --noconfirm'
if grep -q 'archtype-data-rime-ice' "$work/fetches"; then fail 'Arch fetched a Debian data package'; fi
pass 'Pacman installs settings dependencies and local package without -Syu'

reset $'ID=fedora\nVERSION_ID=43'
asset archtype-0.3.8-1.fc43.x86_64.rpm
run > "$work/output" 2>&1 || fail "dnf install: $(cat "$work/output")"
assert_log 'dnf install -y'
assert_log 'archtype-0.3.8-1.fc43.x86_64.rpm'
assert_log 'fcitx5-gtk3 fcitx5-gtk4 fcitx5-qt5 fcitx5-qt6'
pass 'DNF installs exact Fedora RPM and GTK/Qt frontends'

reset 'ID=arch'
asset archtype-0.3.8-1-x86_64.pkg.tar.zst
printf 'corruption\n' >> "$work/assets/archtype-0.3.8-1-x86_64.pkg.tar.zst"
if run > "$work/output" 2>&1; then fail 'Corrupted asset accepted'; fi
assert_no_install
pass 'Checksum mismatch prevents all package operations'

reset 'ID=arch'
asset archtype-0.3.8-1-x86_64.pkg.tar.zst
cat "$work/assets/SHA256SUMS" > "$work/duplicate"
cat "$work/duplicate" >> "$work/assets/SHA256SUMS"
if run > "$work/output" 2>&1; then fail 'Duplicate hashes accepted'; fi
assert_no_install
pass 'Duplicate manifest entries rejected'

reset 'ID=arch'
asset archtype-0.3.8-1-x86_64.pkg.tar.zst
printf 'bad  archtype-0.3.8-1-x86_64.pkg.tar.zst\n' > "$work/assets/SHA256SUMS"
if run > "$work/output" 2>&1; then fail 'Malformed hash accepted'; fi
assert_no_install
pass 'Malformed manifest hash rejected'

reset $'ID=fedora\nVERSION_ID=44'
asset archtype-0.3.8-1.fc43.x86_64.rpm
if run > "$work/output" 2>&1; then fail 'Wrong Fedora version accepted'; fi
assert_no_install
pass 'Missing target release asset fails before package operations'

reset 'ID=arch'
asset archtype-0.3.8-1-x86_64.pkg.tar.zst
rm "$work/assets/archtype-0.3.8-1-x86_64.pkg.tar.zst"
if run > "$work/output" 2>&1; then fail 'Missing asset accepted'; fi
assert_no_install
pass 'Failed asset download prevents package operations'

reset 'ID=arch'
asset archtype-0.3.8-1-x86_64.pkg.tar.zst
sed -i 's/  / */' "$work/assets/SHA256SUMS"
run > "$work/output" 2>&1 || fail 'Binary-mode checksum line rejected'
pass 'Binary-mode SHA256SUMS supported'

reset 'ID=arch'
asset archtype-0.3.8-1-x86_64.pkg.tar.zst
test_version=
run > "$work/output" 2>&1 || fail 'Latest release resolution failed'
grep -Fq 'https://github.com/Goway-Hui/ArchType/releases/latest' "$work/fetches" || fail 'Wrong release repository'
pass 'Latest release uses the upstream repository'

printf '%s installer checks passed\n' "$count"
