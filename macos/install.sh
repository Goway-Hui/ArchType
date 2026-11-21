#!/usr/bin/env bash
# 在 macOS 上安装 / 卸载 ArchType 输入法（装到当前用户的 ~/Library/Input Methods，不要管理员权限）。
#
#   macos/install.sh ArchType-<版本>-macos.zip     # 或者解压出来的 ArchType.app
#   macos/install.sh --uninstall
set -euo pipefail

[[ "$(uname)" == Darwin ]] || { echo "这个脚本只在 macOS 上用" >&2; exit 1; }
dest="$HOME/Library/Input Methods/ArchType.app"

stop_running() {
  # 按进程名精确匹配，只停 ArchType 自己
  pkill -x ArchType 2>/dev/null || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
  # 新版自己会卸（关掉输入源、停掉输入法、去掉登记、删掉程序）；老版本不认 --uninstall，照下面的老办法
  if [[ -x "$dest/Contents/MacOS/ArchType" ]] && "$dest/Contents/MacOS/ArchType" --uninstall; then
    exit 0
  fi
  [[ -x "$dest/Contents/MacOS/ArchType" ]] && "$dest/Contents/MacOS/ArchType" --disable || true
  stop_running
  rm -rf "$dest"
  echo "已卸载。用户数据还在 ~/Library/Application Support/ArchType，不要了可以手动删掉。"
  exit 0
fi

src=${1:?用法：macos/install.sh ArchType-<版本>-macos.zip | ArchType.app | --uninstall}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
if [[ "$src" == *.zip ]]; then
  ditto -x -k "$src" "$tmp"
  src="$tmp/ArchType.app"
fi
[[ -x "$src/Contents/MacOS/ArchType" ]] || { echo "$src 不是 ArchType.app" >&2; exit 1; }

stop_running
mkdir -p "$HOME/Library/Input Methods"
rm -rf "$dest"
ditto "$src" "$dest"
# 上面停掉以后、新文件落盘之前，系统可能已经把旧程序又拉起来了（在终端里按一次键就会）：再停一次
stop_running
# 从网上下载的没有公证，去掉隔离标记，不然系统不让输入法跑起来
xattr -dr com.apple.quarantine "$dest" 2>/dev/null || true

bin="$dest/Contents/MacOS/ArchType"
"$bin" --register
# 菜单栏的输入法菜单会缓存图标，升级换了图标时重启一下它才会刷新（系统会马上把它拉起来）
killall TextInputMenuAgent 2>/dev/null || true
# 刚登记完系统可能还没列出来，等一下再打开
for _ in 1 2 3 4 5; do
  if "$bin" --enable >/dev/null 2>&1; then
    "$bin" --enable
    "$bin" --select || true
    echo "装好了。菜单栏的输入法里选 ArchType 就能用。"
    exit 0
  fi
  sleep 1
done
cat <<'MSG'
已经装到 ~/Library/Input Methods/ArchType.app，但没能自动打开。
请注销重新登录一次，然后在「系统设置 → 键盘 → 输入法 → 编辑 → +」里，
在「简体中文」下面加上 ArchType。（已为你打开「系统设置 → 键盘」）
MSG
open "x-apple.systempreferences:com.apple.Keyboard-Settings.extension" 2>/dev/null || true
