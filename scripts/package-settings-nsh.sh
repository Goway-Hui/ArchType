#!/usr/bin/env bash
# 打 NSH 版设置界面的 Linux tar.gz（deb/rpm/Arch 打包脚本用 ARCHTYPE_NSH_ARCHIVE 接入）。
#
#   scripts/package-settings-nsh.sh <版本> [输出目录]
#
# 需要 `nsh`（crates.io 的 nsh 0.6.x）和 GTK 4.14+ 的构建环境；
# 产物是 runner + bundle + .desktop 的 tar.gz，解压到 /usr/lib/archtype/settings 即可用。
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=${1:?用法: package-settings-nsh.sh <版本> [输出目录]}
if [[ ! "$VERSION" =~ ^[0-9][0-9A-Za-z.+~_-]*$ ]]; then
  echo "版本号格式不对：$VERSION" >&2
  exit 1
fi
OUT=$(realpath -m "${2:-dist/pkg}")
mkdir -p "$OUT"

command -v nsh >/dev/null || { echo "需要 nsh 0.6.x（curl -fsSL https://nsh.dev/install.sh | NSH_VERSION=v0.6.0 sh）" >&2; exit 1; }

rm -rf settings-nsh/.nsh/out
(cd settings-nsh && nsh package --target linux --app-version "$VERSION" -o .nsh/out >/dev/null)

archive=$(find settings-nsh/.nsh/out -maxdepth 1 -name '*.tar.gz' | head -1)
[[ -f "$archive" ]] || { echo "nsh package 没有生成 tar.gz" >&2; exit 1; }

final="$OUT/archtype-settings-nsh-${VERSION}-linux-$(uname -m).tar.gz"
cp "$archive" "$final"
printf '%s\n' "$final"
