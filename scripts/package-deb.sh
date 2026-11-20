#!/usr/bin/env bash
# 打 .deb 包（CI 发版用，本地也能跑）。
#
#   scripts/package-deb.sh engine <版本> [输出目录]   安装包：fcitx5 插件 + 设置界面 + 自检工具 + 词库
#   scripts/package-deb.sh data <数据目录> [输出目录]  不用 deb 的系统用的词库压缩包
#
# 词库（scripts/make-data.sh 的产物）直接打进安装包，用户只装一个文件。
# 数据目录默认 dist/data，ARCHTYPE_DATA_DIR=<目录> 可以改。
# 以前词库是单独的 archtype-data-rime-ice 包；新包 Conflicts/Replaces 它，apt 装新包时会顺手把它删掉。
#
# 安装包和当前系统的 fcitx5 版本绑定，要在目标发行版上（或同版本容器里）打。
# 包的版本号带发行版（0.1.0-1~ubuntu24.04）；文件名里不用 ~（GitHub Release 会把它改成 .）：
# archtype_0.1.0_ubuntu24.04_amd64.deb。发行版默认取 os-release 的 ${ID}${VERSION_ID}，
# ARCHTYPE_DISTRO=<名字> 可以指定（CI 传矩阵里的名字，免得镜像的版本号变成 25.0.x 时和一键安装对不上）。
set -euo pipefail
cd "$(dirname "$0")/.."

MODE=${1:?用法: package-deb.sh engine <版本> | data <数据目录>}
ARG=${2:?缺少参数}
OUT=$(realpath -m "${3:-dist}")
mkdir -p "$OUT"
HOMEPAGE=https://github.com/Goway-Hui/ArchType
MAINTAINER="Gloridust <gloridust@users.noreply.github.com>"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
stage="$work/stage"

build_deb() { # <包名> <版本> <架构> <依赖> <简介> <详细说明>
  local name=$1 version=$2 arch=$3 depends=$4 summary=$5 body=$6
  mkdir -p "$stage/DEBIAN"
  {
    echo "Package: $name"
    echo "Version: $version"
    echo "Architecture: $arch"
    echo "Maintainer: $MAINTAINER"
    echo "Installed-Size: $(du -sk --exclude=DEBIAN "$stage" | cut -f1)"
    [[ -n "$depends" ]] && echo "Depends: $depends"
    [[ -n "${RECOMMENDS:-}" ]] && echo "Recommends: $RECOMMENDS"
    [[ -n "${REPLACES:-}" ]] && printf 'Conflicts: %s\nReplaces: %s\nProvides: %s\n' \
      "$REPLACES" "$REPLACES" "$REPLACES"
    echo "Section: utils"
    echo "Priority: optional"
    echo "Homepage: $HOMEPAGE"
    echo "Description: $summary"
    sed 's/^/ /' <<<"$body"
  } > "$stage/DEBIAN/control"
  local file="$OUT/${DEB_FILE:-${name}_${version}_${arch}.deb}"
  dpkg-deb --build --root-owner-group -Zxz "$stage" "$file" >/dev/null
  echo "$file"
}

data_copyright() { # <数据目录>：copyright 里词库那几段
  echo "Files: usr/share/archtype/lexicon.tsv usr/share/archtype/english.txt"
  echo "Comment: 转换自雾凇拼音 rime-ice（https://github.com/iDvel/rime-ice），见 DATA-SOURCE.txt"
  # 语料词频（默认融进 data/tatoeba-freq.tsv）的来源、许可证、署名都在 SOURCE.txt 里
  if grep -q 'tatoeba-freq.tsv' "$1/SOURCE.txt"; then
    echo " 词频融进了 Tatoeba（https://tatoeba.org）中文句子统计出的词频，"
    echo " 句子许可证 CC BY 2.0 FR，Tatoeba contributors；见 DATA-SOURCE.txt。"
  fi
  echo "License: GPL-3.0"
  sed 's/^$/./; s/^/ /' "$1/LICENSE"
  echo
  echo "Files: usr/share/archtype/words.lm"
  echo "Copyright: ArchType contributors; statistics derived from LCCC (thu-coai, MIT)"
  echo " and Chinese Wikipedia (Wikipedia contributors, CC BY-SA 4.0)"
  echo "License: CC-BY-SA-4.0"
  echo " Word-level language model built by archtype-lmbuild from the LCCC corpus"
  echo " (https://huggingface.co/datasets/thu-coai/lccc) and Chinese Wikipedia"
  echo " (https://zh.wikipedia.org/). Licensed under the Creative Commons"
  echo " Attribution-ShareAlike 4.0 International License:"
  echo " https://creativecommons.org/licenses/by-sa/4.0/legalcode"
  echo
  # 整句增强模型 nnlm.bin（make-data.sh 下载到了才有）
  if [[ -f "$1/nnlm.bin" ]]; then
    echo "Files: usr/share/archtype/nnlm.bin"
    echo "Copyright: ArchType contributors; trained on LCCC (thu-coai, MIT)"
    echo " and Chinese Wikipedia (Wikipedia contributors, CC BY-SA 4.0)"
    echo "License: CC-BY-SA-4.0"
    echo " Character-level neural language model (WTNN) trained by ArchType's"
    echo " tools/nnlm/train.py on the LCCC corpus"
    echo " (https://huggingface.co/datasets/thu-coai/lccc) and Chinese Wikipedia"
    echo " (https://zh.wikipedia.org/). Licensed under the Creative Commons"
    echo " Attribution-ShareAlike 4.0 International License:"
    echo " https://creativecommons.org/licenses/by-sa/4.0/legalcode"
    echo
  fi
  echo "Files: usr/share/archtype/wubi86.tsv"
  echo "Comment: 五笔 86 码表，转换自 Rime 五笔字型 rime-wubi（https://github.com/rime/rime-wubi），"
  echo " 作者见 WUBI-AUTHORS，来源和钉死的提交见 WUBI-SOURCE.txt"
  echo "License: LGPL-3.0"
  sed 's/^$/./; s/^/ /' "$1/WUBI-LICENSE"
}

# 安装包的 postinst：第一次装（不是升级）时，给每个已登录图形会话的用户跑一次 archtype-config --notify-setup，
# 还差一步（没切到 fcitx5、fcitx5 还没载入 ArchType、没加进输入法列表）就发一条通知，告诉他到「ArchType 设置」
# 点「一键设置」。只发通知、不改任何用户设置；哪一步不行就跳过，不让安装失败。
# 升级时不发：正在跑的插件每小时叫一次 archtype-config --startup，它会提醒重启输入法。
write_postinst() { # <路径>
  mkdir -p "$(dirname "$1")"
  cat > "$1" <<'POSTINST'
#!/bin/sh
set -e
if [ "$1" = configure ] && [ -z "$2" ] && command -v loginctl >/dev/null 2>&1 &&
   command -v runuser >/dev/null 2>&1; then
  seen=" "
  for session in $(loginctl list-sessions --no-legend 2>/dev/null | awk '{print $1}'); do
    type=$(loginctl show-session "$session" -p Type --value 2>/dev/null || true)
    case "$type" in x11|wayland) ;; *) continue ;; esac
    uid=$(loginctl show-session "$session" -p User --value 2>/dev/null || true)
    case "$uid" in ''|0|*[!0-9]*) continue ;; esac
    case "$seen" in *" $uid "*) continue ;; esac
    seen="$seen$uid "
    bus="/run/user/$uid/bus"
    user=$(getent passwd "$uid" | cut -d: -f1)
    home=$(getent passwd "$uid" | cut -d: -f6)
    [ -S "$bus" ] && [ -n "$user" ] && [ -n "$home" ] || continue
    timeout 20 runuser -u "$user" -- env HOME="$home" XDG_RUNTIME_DIR="/run/user/$uid" \
      DBUS_SESSION_BUS_ADDRESS="unix:path=$bus" /usr/bin/archtype-config --notify-setup \
      </dev/null >/dev/null 2>&1 || true
  done
fi
exit 0
POSTINST
  chmod 755 "$1"
}

# NSH 版设置界面（可选）：ARCHTYPE_NSH_ARCHIVE 指向 scripts/package-settings-nsh.sh 的产物时安装它，
# 并把「ArchType 设置」的桌面入口换成 archtype-settings。只有 GTK 4.14+ 的发行版才该设置
# （Python 版 archtype-config 保留命令行：--setup、--install-lexicon、--startup 等）。
install_nsh_settings() { # <root>
  local root=$1
  [[ -n "${ARCHTYPE_NSH_ARCHIVE:-}" ]] || return 0
  local dest="$root/usr/lib/archtype/settings"
  mkdir -p "$root/usr/bin" "$dest"
  tar -xzf "$ARCHTYPE_NSH_ARCHIVE" -C "$dest" --strip-components=1
  local runner
  runner=$(find "$dest" -maxdepth 1 -type f -perm -u+x | head -1)
  [[ -n "$runner" ]] || { echo "NSH 设置产物里没有可执行文件" >&2; exit 1; }
  {
    echo '#!/bin/sh'
    echo "exec /usr/lib/archtype/settings/$(basename "$runner") \"\$@\""
  } > "$root/usr/bin/archtype-settings"
  chmod 755 "$root/usr/bin/archtype-settings"
  local desktop
  desktop=$(find "$root/usr/share/applications" -name '*.desktop' 2>/dev/null | head -1)
  [[ -n "$desktop" ]] || { echo "没找到设置界面的 desktop 文件" >&2; exit 1; }
  sed -i 's|^Exec=.*|Exec=archtype-settings|' "$desktop"
}

check_data() { # <数据目录>
  for f in lexicon.tsv english.txt words.lm LICENSE SOURCE.txt VERSION DATA-MANIFEST; do
    [[ -f "$1/$f" ]] || { echo "缺 $1/${f}，先跑 scripts/make-data.sh" >&2; exit 1; }
  done
  for f in wubi86.tsv WUBI-LICENSE WUBI-SOURCE.txt WUBI-AUTHORS; do
    [[ -f "$1/$f" ]] || { echo "缺 $1/${f}，先跑 scripts/make-wubi.sh" >&2; exit 1; }
  done
}

case "$MODE" in
engine)
  version=$ARG
  data=${ARCHTYPE_DATA_DIR:-dist/data}
  check_data "$data"
  . /etc/os-release
  distro="${ARCHTYPE_DISTRO:-${ID}${VERSION_ID:-}}"
  debver="${version}-1~${distro}"

  cargo build --release --locked -p archtype-core
  cmake -S fcitx5-addon -B "$work/build" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr \
    -DARCHTYPE_VERSION="$version" >/dev/null
  cmake --build "$work/build" -j"$(nproc)"
  DESTDIR="$stage" cmake --install "$work/build" >/dev/null
  rm -rf "$stage/usr/include"          # 头文件只给开发用
  install_nsh_settings "$stage"
  for bin in archtype-check archtype-show; do
    install -Dm755 "target/release/$bin" "$stage/usr/bin/$bin"
  done
  # 只处理 ELF（/usr/bin 里还有 Python 写的设置界面 archtype-config）
  is_elf() { [[ "$(head -c4 "$1" | od -An -c | tr -d ' ')" == '177ELF' ]]; }
  mapfile -t elfs < <(find "$stage/usr" -type f \( -name '*.so' -o -path '*/bin/*' \) |
                      while read -r f; do is_elf "$f" && echo "$f"; done)
  strip --strip-unneeded "${elfs[@]}"

  # 词库：雾凇拼音（GPL-3.0）+ ArchType 的词级语言模型（CC BY-SA 4.0）+ 五笔码表（rime-wubi，LGPL-3.0）
  for f in lexicon.tsv english.txt words.lm wubi86.tsv; do
    install -Dm644 "$data/$f" "$stage/usr/share/archtype/$f"
  done
  # 整句增强模型（ArchType 自己训的 nnlm.bin，CC BY-SA 4.0）：make-data.sh 下载到了才有，没有就不带
  if [[ -f "$data/nnlm.bin" ]]; then
    install -Dm644 "$data/nnlm.bin" "$stage/usr/share/archtype/nnlm.bin"
  fi
  # 数据清单：各数据文件的名字、版本、来源（tools/datamark.py manifest，docs/DATA.md「数据版本标记」）
  install -Dm644 "$data/DATA-MANIFEST" "$stage/usr/share/archtype/DATA-MANIFEST"
  doc="$stage/usr/share/doc/archtype"
  install -Dm644 README.md "$doc/README.md"
  install -Dm644 "$data/SOURCE.txt" "$doc/DATA-SOURCE.txt"
  install -Dm644 "$data/VERSION" "$doc/DATA-VERSION"
  install -Dm644 "$data/WUBI-SOURCE.txt" "$doc/WUBI-SOURCE.txt"
  install -Dm644 "$data/WUBI-AUTHORS" "$doc/WUBI-AUTHORS"
  {
  cat <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: ArchType
Source: $HOMEPAGE

Files: *
License: MIT or Apache-2.0

Files: usr/share/archtype/s2t.tsv
Comment: 由 OpenCC 的字符表和词组表（STCharacters、STPhrases）生成（tools/opencc2tsv.py），
 版本见文件头，https://github.com/BYVoid/OpenCC
License: Apache-2.0

Files: usr/lib/*/fcitx5/libarchtype.so
Comment: 编进去的 emoji 关键词（core/src/emoji/cldr.tsv）来自 Unicode CLDR 和 emoji-test.txt
Copyright: 1991-2026 Unicode, Inc.
License: Unicode-3.0
 https://www.unicode.org/license.txt

EOF
  data_copyright "$data"
  } > "$doc/copyright"

  write_postinst "$stage/DEBIAN/postinst"

  # 运行时依赖交给 dpkg-shlibdeps 从 .so 里算（libfcitx5core、libstdc++ 等）
  mkdir -p "$work/debian"
  printf 'Source: archtype\n\nPackage: archtype\nArchitecture: any\n' > "$work/debian/control"
  shlibs=$(cd "$work" && dpkg-shlibdeps -O "${elfs[@]}" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
  # 设置界面（PyGObject + GTK 4 + libadwaita）是硬依赖：缺了的话应用菜单里点「ArchType 设置」没反应，
  # 插件在后台叫的 archtype-config --startup（检查更新、默认主题、提醒重启输入法）也跑不了
  depends="${shlibs:+$shlibs, }fcitx5, python3-gi, python3-gi-cairo, gir1.2-gtk-4.0, gir1.2-adw-1"

  # emoji 要彩色 emoji 字体才画得出来（桌面版系统一般都装了）；互译用 curl 发请求
  RECOMMENDS="fonts-noto-color-emoji, curl" \
  REPLACES="archtype-data-rime-ice" \
  DEB_FILE="archtype_${version}_${distro}_$(dpkg --print-architecture).deb" \
  build_deb archtype "$debver" "$(dpkg --print-architecture)" "$depends" \
    "WeChat-style pinyin input method for fcitx5" \
"拼音输入法：长句、半成品输入、按错键纠错、按用户习惯学习。
自带雾凇拼音（rime-ice）词库、词级语言模型、整句增强模型和五笔 86 码表（rime-wubi）。
装完打开应用菜单的「ArchType 设置」点第一页的「一键设置」（或运行 archtype-config --setup）。"
  ;;

data)
  data=$ARG
  check_data "$data"
  version=$(<"$data/VERSION")
  version=${version//-/.}
  # 不用 deb 的系统：解压到 ~/.local/share/archtype/ 即可
  tarball="$OUT/archtype-data-rime-ice-$version.tar.gz"
  # 可复现：固定文件顺序、时间和属主，gzip 不写时间戳。内容不变时每次打出来的包
  # sha256 都一样，打包脚本（如 AUR）钉住的校验和不会因为重新发版而失效。
  # 整句增强模型 nnlm.bin 有就一起打进去（Nix 包从这个压缩包取数据）
  files=(lexicon.tsv english.txt words.lm LICENSE SOURCE.txt DATA-MANIFEST
         wubi86.tsv WUBI-LICENSE WUBI-SOURCE.txt WUBI-AUTHORS)
  [[ -f "$data/nnlm.bin" ]] && files+=(nnlm.bin)
  tar -C "$data" --sort=name --mtime="@${SOURCE_DATE_EPOCH:-0}" \
    --owner=0 --group=0 --numeric-owner -cf - \
    "${files[@]}" | gzip -n >"$tarball"
  echo "$tarball"
  ;;

*)
  echo "未知模式 $MODE" >&2
  exit 2
  ;;
esac
