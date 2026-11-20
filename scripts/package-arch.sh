#!/usr/bin/env bash
# 用 makepkg 从当前源码打 Arch Linux 安装包（不要以 root 运行）。
#   scripts/package-arch.sh <版本> [输出目录]
# 需要 base-devel rust cmake extra-cmake-modules fcitx5 alsa-lib；
# 词库用 scripts/make-data.sh 的产物，默认 dist/data。
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=${1:?用法: package-arch.sh <版本> [输出目录]}
# Arch 的 pkgver 不能有空白、冒号、斜杠或连字符；不要静默改写标签。
[[ "$VERSION" =~ ^[[:alnum:]][[:alnum:]._+~]*$ ]] || {
  echo "无效的 Arch 版本号: $VERSION" >&2
  exit 1
}
(( EUID != 0 )) || { echo "makepkg 不能以 root 运行，请用普通用户运行此脚本" >&2; exit 1; }
command -v makepkg >/dev/null || { echo "需要 Arch Linux 的 makepkg（pacman/base-devel）" >&2; exit 1; }
OUT=$(realpath -m "${2:-dist}")
DATA=$(realpath -m "${ARCHTYPE_DATA_DIR:-dist/data}")
for f in lexicon.tsv english.txt words.lm LICENSE SOURCE.txt; do
  [[ -s "$DATA/$f" ]] || { echo "缺 $DATA/${f}，先跑 scripts/make-data.sh" >&2; exit 1; }
done
for f in wubi86.tsv WUBI-LICENSE WUBI-SOURCE.txt WUBI-AUTHORS; do
  [[ -s "$DATA/$f" ]] || { echo "缺 $DATA/${f}，先跑 scripts/make-wubi.sh" >&2; exit 1; }
done
mkdir -p "$OUT"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# 只打包构建输入，不带现有 target/、CMake 缓存或 .git。makepkg 在独立目录重建，
# 不依赖按键测试留下的二进制，也不往 checkout 写 root 所有的构建文件。
mkdir -p "$work/source/dist/data"
tar --exclude=target --exclude='build*' -cf - \
  Cargo.toml Cargo.lock core voice voice-daemon cli fcitx5-addon icons themes settings README.md |
  tar -xf - -C "$work/source"
cp "$DATA"/{lexicon.tsv,english.txt,words.lm,LICENSE,SOURCE.txt} "$work/source/dist/data/"
cp "$DATA"/{wubi86.tsv,WUBI-LICENSE,WUBI-SOURCE.txt,WUBI-AUTHORS} "$work/source/dist/data/"
if [[ -f "$DATA/DATA-MANIFEST" ]]; then
  cp "$DATA/DATA-MANIFEST" "$work/source/dist/data/"
fi
# 整句增强模型（ArchType 自己训的 nnlm.bin，CC BY-SA 4.0，已在 license 里）：make-data.sh 下载到了才有
if [[ -f "$DATA/nnlm.bin" ]]; then
  cp "$DATA/nnlm.bin" "$work/source/dist/data/"
fi
if [[ -f "$DATA/VERSION" ]]; then
  cp "$DATA/VERSION" "$work/source/dist/data/"
fi
tar -cf "$work/archtype-src.tar" -C "$work/source" .
rm -rf "$work/source"
checksum=$(sha256sum "$work/archtype-src.tar")
{
  printf 'pkgver=%q\n' "$VERSION"
  printf "sha256sums=('%s')\n" "${checksum%% *}"
  cat <<'PKGBUILD'
pkgname=archtype
pkgrel=1
pkgdesc='WeChat-style pinyin input method for fcitx5, with bundled dictionaries and language model'
arch=('x86_64')
url='https://github.com/Goway-Hui/ArchType'
license=('MIT' 'Apache-2.0' 'GPL-3.0-only' 'CC-BY-SA-4.0' 'Unicode-3.0' 'LGPL-3.0-only')
# 设置界面（gtk4、webkitgtk-6.0、gstreamer）和 GTK / Qt 程序里打字要的 fcitx5-gtk、fcitx5-qt 都是硬依赖
depends=('fcitx5' 'fcitx5-gtk' 'fcitx5-qt' 'alsa-lib' 'gcc-libs' 'glibc'
         'gtk4' 'webkitgtk-6.0' 'gstreamer' 'gst-plugins-base')
makedepends=('rust' 'cmake' 'extra-cmake-modules' 'pkgconf')
optdepends=('noto-fonts-emoji: color emoji in candidates' 'curl: translate mode')
# makepkg handles ELF stripping and byte-sized .PKGINFO/.MTREE/.BUILDINFO.
# Avoid a second debug package in the CI release artifacts.
options=('!debug')
source=('archtype-src.tar')
install=archtype.install

prepare() {
  cd "$srcdir" || return
  cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
  cd "$srcdir" || return
  cargo build --release --frozen -p archtype-core --lib -p archtype-config-cli \
    --bin archtype-check --bin archtype-show
  cmake -S fcitx5-addon -B build -DCMAKE_BUILD_TYPE=None \
    -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_INSTALL_LIBDIR=lib -DARCHTYPE_VERSION="$pkgver"
  cmake --build build -j"$(nproc)"
}

check() {
  cd "$srcdir" || return
  cargo test --release --frozen -p archtype-core --lib
}

package() {
  cd "$srcdir" || return
  DESTDIR="$pkgdir" cmake --install build
  rm -r "$pkgdir/usr/include/archtype"
  rmdir --ignore-fail-on-non-empty "$pkgdir/usr/include"
  for bin in archtype-check archtype-show; do
    install -Dm755 "target/release/$bin" "$pkgdir/usr/bin/$bin"
  done
  install -m644 dist/data/{lexicon.tsv,english.txt,words.lm,wubi86.tsv} "$pkgdir/usr/share/archtype/"
  # 数据清单：各数据文件的名字、版本、来源（docs/DATA.md「数据版本标记」）
  if [[ -f dist/data/DATA-MANIFEST ]]; then
    install -m644 dist/data/DATA-MANIFEST "$pkgdir/usr/share/archtype/"
  fi
  # 整句增强模型：源码包里有才装
  if [[ -f dist/data/nnlm.bin ]]; then
    install -m644 dist/data/nnlm.bin "$pkgdir/usr/share/archtype/"
  fi
  install -Dm644 README.md "$pkgdir/usr/share/doc/archtype/README.md"
  install -Dm644 dist/data/SOURCE.txt "$pkgdir/usr/share/doc/archtype/DATA-SOURCE.txt"
  install -Dm644 dist/data/WUBI-SOURCE.txt "$pkgdir/usr/share/doc/archtype/WUBI-SOURCE.txt"
  install -Dm644 dist/data/WUBI-AUTHORS "$pkgdir/usr/share/doc/archtype/WUBI-AUTHORS"
  if [[ -f dist/data/VERSION ]]; then
    install -Dm644 dist/data/VERSION "$pkgdir/usr/share/doc/archtype/DATA-VERSION"
  fi
  install -Dm644 dist/data/LICENSE "$pkgdir/usr/share/licenses/archtype/LICENSE.rime-ice"
  install -Dm644 dist/data/WUBI-LICENSE "$pkgdir/usr/share/licenses/archtype/LICENSE.rime-wubi"
  # NSH 版设置界面（可选）：ARCHTYPE_NSH_ARCHIVE 给了就装，并把桌面入口换成 archtype-settings
  if [[ -f archtype-settings.tar.gz ]]; then
    install -d "$pkgdir/usr/lib/archtype/settings"
    tar -xzf archtype-settings.tar.gz -C "$pkgdir/usr/lib/archtype/settings" --strip-components=1
    runner=$(find "$pkgdir/usr/lib/archtype/settings" -maxdepth 1 -type f -perm -u+x | head -1)
    printf '#!/bin/sh\nexec /usr/lib/archtype/settings/%s "$@"\n' "$(basename "$runner")" > "$pkgdir/usr/bin/archtype-settings"
    chmod 755 "$pkgdir/usr/bin/archtype-settings"
    sed -i 's|^Exec=.*|Exec=archtype-settings|' "$pkgdir/usr/share/applications/io.github.GowayHui.ArchType.Config.desktop"
  fi
}
PKGBUILD
} > "$work/PKGBUILD"
if [[ -n "${ARCHTYPE_NSH_ARCHIVE:-}" ]]; then
  cp "$ARCHTYPE_NSH_ARCHIVE" "$work/archtype-settings.tar.gz"
  nsh_sum=$(sha256sum "$work/archtype-settings.tar.gz")
  sed -i \
    -e "s|^source=('archtype-src.tar')$|source=('archtype-src.tar' 'archtype-settings.tar.gz')|" \
    -e "s|^sha256sums=('.*')$|sha256sums=('${checksum%% *}' '${nsh_sum%% *}')|" \
    "$work/PKGBUILD"
fi
cat > "$work/archtype.install" <<'INSTALL'
post_install() {
  cat <<'EOF'
==> ArchType 已安装。
  用普通用户运行 archtype-config --setup（或打开应用菜单的「ArchType 设置」点第一页的「一键设置」），
  把 ArchType 加进 fcitx5 的输入法列表，必要时重启 fcitx5。
  Arch 上没有 im-config，别照 Ubuntu 教程跑它；fcitx5 还没在跑的话，在桌面设置里让它自启动。
EOF
}
post_upgrade() {
  cat <<'EOF'
==> ArchType 已更新：重启 fcitx5（或注销重新登录）后生效，ArchType 也会发通知提醒。
EOF
}
INSTALL

cd "$work"
# 不安装依赖、不提权。CI 先装好依赖，再用普通用户执行这里。
PKGDEST="$OUT" PKGEXT=.pkg.tar.zst BUILDDIR="$work" SRCDEST="$work" \
  SRCPKGDEST="$work" LOGDEST="$work" makepkg --noconfirm --force --nosign
file="$OUT/archtype-$VERSION-1-$(uname -m).pkg.tar.zst"
[[ -s "$file" ]] || { echo "makepkg 没有生成 $file" >&2; exit 1; }
printf '%s\n' "$file"
