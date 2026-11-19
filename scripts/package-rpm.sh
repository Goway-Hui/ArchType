#!/usr/bin/env bash
# 用 rpmbuild 从当前源码打 Fedora 安装包（.rpm）。
#   scripts/package-rpm.sh <版本> [输出目录]
# 需要 rpm-build rust cargo cmake extra-cmake-modules gcc-c++ fcitx5-devel alsa-lib-devel；
# 词库用 scripts/make-data.sh 的产物，默认 dist/data（ARCHTYPE_DATA_DIR=<目录> 可以改）。
#
# 版本号可以带 ~（0.3.6~git20261110.abc1234），RPM 把 ~ 排在正式版之前。
# 安装包和打包机器上的 fcitx5 版本绑定，要在目标 Fedora 版本上（或同版本容器里）打。
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=${1:?用法: package-rpm.sh <版本> [输出目录]}
# RPM 的 Version 不能有连字符、空白或斜杠；不要静默改写标签。
[[ "$VERSION" =~ ^[[:alnum:]][[:alnum:]._+~^]*$ ]] || {
  echo "无效的 RPM 版本号: $VERSION" >&2
  exit 1
}
command -v rpmbuild >/dev/null || { echo "需要 rpmbuild（dnf install rpm-build）" >&2; exit 1; }
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
mkdir -p "$work"/{SOURCES,SPECS,BUILD,BUILDROOT,SRPMS}

# 只打包构建输入，不带现有 target/、CMake 缓存或 .git：rpmbuild 在独立目录从头编，
# 不依赖按键测试留下的二进制。
src="$work/source"
mkdir -p "$src/dist/data" "$src/packaging"
tar --exclude=target --exclude='build*' -cf - \
  Cargo.toml Cargo.lock core voice voice-daemon fcitx5-addon icons themes settings README.md |
  tar -xf - -C "$src"
cp "$DATA"/{lexicon.tsv,english.txt,words.lm,LICENSE,SOURCE.txt} "$src/dist/data/"
cp "$DATA"/{wubi86.tsv,WUBI-LICENSE,WUBI-SOURCE.txt,WUBI-AUTHORS} "$src/dist/data/"
if [[ -f "$DATA/VERSION" ]]; then
  cp "$DATA/VERSION" "$src/dist/data/"
fi
if [[ -f "$DATA/DATA-MANIFEST" ]]; then
  cp "$DATA/DATA-MANIFEST" "$src/dist/data/"
fi
# 整句增强模型（ArchType 自己训的 nnlm.bin，CC BY-SA 4.0）：make-data.sh 下载到了才有，没有就不带
if [[ -f "$DATA/nnlm.bin" ]]; then
  cp "$DATA/nnlm.bin" "$src/dist/data/"
fi

# 各部分的许可证（和 deb 包的 copyright 内容一致）
cat > "$src/packaging/COPYRIGHT" <<'EOF'
ArchType — https://github.com/Goway-Hui/ArchType

程序（fcitx5 插件、设置界面、archtype-check、archtype-show、主题、图标）：
  MIT 或 Apache-2.0，二选一。

/usr/share/archtype/s2t.tsv：
  由 OpenCC 的字符表和词组表（STCharacters、STPhrases）生成（tools/opencc2tsv.py），
  版本见文件头，Apache-2.0，https://github.com/BYVoid/OpenCC。

插件里编进去的 emoji 关键词（core/src/emoji/cldr.tsv）：
  来自 Unicode CLDR 和 emoji-test.txt，Copyright 1991-2026 Unicode, Inc.，
  Unicode-3.0（https://www.unicode.org/license.txt）。

/usr/share/archtype/lexicon.tsv、/usr/share/archtype/english.txt：
  转换自雾凇拼音 rime-ice（https://github.com/iDvel/rime-ice），GPL-3.0，
  全文见同目录 LICENSE.rime-ice，来源见 /usr/share/doc/archtype/DATA-SOURCE.txt。

/usr/share/archtype/words.lm：
  ArchType 的词级语言模型，由 archtype-lmbuild 从 LCCC 语料（thu-coai，MIT，
  https://huggingface.co/datasets/thu-coai/lccc）和中文维基百科（Wikipedia
  contributors，CC BY-SA 4.0，https://zh.wikipedia.org/）统计得到。
  按知识共享 署名-相同方式共享 4.0 国际许可协议授权：
  https://creativecommons.org/licenses/by-sa/4.0/legalcode

/usr/share/archtype/wubi86.tsv：
  五笔 86 码表，转换自 Rime 五笔字型 rime-wubi（https://github.com/rime/rime-wubi），LGPL-3.0，
  全文见同目录 LICENSE.rime-wubi（LGPL-3.0 是 GPL-3.0 的附加许可，GPL-3.0 全文见 LICENSE.rime-ice），
  作者见 /usr/share/doc/archtype/WUBI-AUTHORS，来源和钉死的提交见 /usr/share/doc/archtype/WUBI-SOURCE.txt。
EOF
if [[ -f "$DATA/nnlm.bin" ]]; then
  cat >> "$src/packaging/COPYRIGHT" <<'EOF'

/usr/share/archtype/nnlm.bin：
  ArchType 的整句增强模型（字级神经语言模型，WTNN 格式），由 ArchType 的 tools/nnlm/train.py
  从 LCCC 语料（thu-coai，MIT，https://huggingface.co/datasets/thu-coai/lccc）和中文维基百科
  （Wikipedia contributors，CC BY-SA 4.0，https://zh.wikipedia.org/）训练得到。
  按知识共享 署名-相同方式共享 4.0 国际许可协议授权：
  https://creativecommons.org/licenses/by-sa/4.0/legalcode
EOF
fi

tar -czf "$work/SOURCES/archtype-src.tar.gz" -C "$src" .
rm -rf "$src"

{
  printf '%%global wt_version %s\n' "$VERSION"
  cat <<'SPEC'
# 插件静态链接 Rust 核心，不单独出 debuginfo 包
%global debug_package %{nil}

Name:           archtype
Version:        %{wt_version}
Release:        1%{?dist}
Summary:        WeChat-style pinyin input method for fcitx5
License:        (MIT OR Apache-2.0) AND Apache-2.0 AND Unicode-3.0 AND GPL-3.0-only AND CC-BY-SA-4.0 AND LGPL-3.0-only
URL:            https://github.com/Goway-Hui/ArchType
Source0:        archtype-src.tar.gz
ExclusiveArch:  x86_64 aarch64

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  cmake
BuildRequires:  extra-cmake-modules
BuildRequires:  gcc-c++
BuildRequires:  fcitx5-devel
BuildRequires:  alsa-lib-devel
Requires:       fcitx5
# 设置界面要 PyGObject + GTK 4 + libadwaita：缺了的话应用菜单里点「ArchType 设置」没反应，
# 插件在后台叫的 archtype-config --startup（检查更新、默认主题、提醒重启输入法）也跑不了
Requires:       python3-gobject
Requires:       gtk4
Requires:       libadwaita
# PyGObject 载入 Gtk 时要 cairo-1.0.typelib，在这个包里
Requires:       gobject-introspection
# emoji 候选要彩色 emoji 字体才画得出来
Recommends:     google-noto-color-emoji-fonts
# 互译用 curl 发请求
Recommends:     curl

%description
拼音输入法：长句、半成品输入、按错键纠错、按用户习惯学习。
自带雾凇拼音（rime-ice）词库、词级语言模型、整句增强模型和五笔 86 码表（rime-wubi）。
装完运行 archtype-config --setup（或打开应用菜单的「ArchType 设置」点「一键设置」），
把 ArchType 加进 fcitx5 的输入法列表。

%prep
%setup -q -c -n archtype-src

%build
cargo build --release --locked -p archtype-core
cmake -S fcitx5-addon -B build -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX=%{_prefix} -DCMAKE_INSTALL_LIBDIR=%{_lib} \
  -DARCHTYPE_VERSION=%{version}
cmake --build build -j%{?_smp_build_ncpus}%{!?_smp_build_ncpus:2}

%install
DESTDIR=%{buildroot} cmake --install build
rm -rf %{buildroot}%{_includedir}
for bin in archtype-check archtype-show; do
  install -Dm755 target/release/$bin %{buildroot}%{_bindir}/$bin
done
install -m644 dist/data/lexicon.tsv dist/data/english.txt dist/data/words.lm dist/data/wubi86.tsv \
  %{buildroot}%{_datadir}/archtype/
# 数据清单：各数据文件的名字、版本、来源（docs/DATA.md「数据版本标记」）
if [ -f dist/data/DATA-MANIFEST ]; then
  install -m644 dist/data/DATA-MANIFEST %{buildroot}%{_datadir}/archtype/
fi
# 整句增强模型：源码包里有才装
if [ -f dist/data/nnlm.bin ]; then
  install -m644 dist/data/nnlm.bin %{buildroot}%{_datadir}/archtype/
fi
doc=%{buildroot}%{_docdir}/archtype
install -Dm644 README.md $doc/README.md
install -Dm644 dist/data/SOURCE.txt $doc/DATA-SOURCE.txt
install -Dm644 dist/data/WUBI-SOURCE.txt $doc/WUBI-SOURCE.txt
install -Dm644 dist/data/WUBI-AUTHORS $doc/WUBI-AUTHORS
if [ -f dist/data/VERSION ]; then
  install -Dm644 dist/data/VERSION $doc/DATA-VERSION
fi
lic=%{buildroot}%{_licensedir}/archtype
install -Dm644 packaging/COPYRIGHT $lic/COPYRIGHT
install -Dm644 dist/data/LICENSE $lic/LICENSE.rime-ice
install -Dm644 dist/data/WUBI-LICENSE $lic/LICENSE.rime-wubi

# NSH 版设置界面（可选）：rpmbuild 带 --define "archtype_nsh_archive <tar.gz>" 时安装
%if 0%{?archtype_nsh_archive:1}
mkdir -p %{buildroot}%{_prefix}/lib/archtype/settings
tar -xzf %{archtype_nsh_archive} -C %{buildroot}%{_prefix}/lib/archtype/settings --strip-components=1
runner=$(find %{buildroot}%{_prefix}/lib/archtype/settings -maxdepth 1 -type f -perm -u+x | head -1)
printf '#!/bin/sh\nexec %{_prefix}/lib/archtype/settings/%s "$@"\n' "$(basename "$runner")" > %{buildroot}%{_bindir}/archtype-settings
chmod 755 %{buildroot}%{_bindir}/archtype-settings
sed -i 's|^Exec=.*|Exec=archtype-settings|' %{buildroot}%{_datadir}/applications/io.github.GowayHui.ArchType.Config.desktop
%endif

%files
%dir %{_licensedir}/archtype
%license %{_licensedir}/archtype/COPYRIGHT
%license %{_licensedir}/archtype/LICENSE.rime-ice
%license %{_licensedir}/archtype/LICENSE.rime-wubi
%{_docdir}/archtype/
%{_bindir}/archtype-config
%if 0%{?archtype_nsh_archive:1}
%{_prefix}/lib/archtype/settings
%{_bindir}/archtype-settings
%endif
%{_bindir}/archtype-check
%{_bindir}/archtype-show
%{_libdir}/fcitx5/libarchtype.so
%{_datadir}/fcitx5/addon/archtype.conf
%{_datadir}/fcitx5/inputmethod/archtype.conf
%{_datadir}/fcitx5/themes/archtype/
%{_datadir}/fcitx5/themes/archtype-dark/
%{_datadir}/gnome-shell/extensions/archtype-theme@goway-hui.github.io/
%{_datadir}/applications/io.github.GowayHui.ArchType.Config.desktop
%{_datadir}/icons/hicolor/*/apps/archtype.*
%{_datadir}/icons/hicolor/scalable/status/archtype-stats-symbolic.svg
%{_datadir}/archtype/

%post
if [ "$1" -eq 1 ]; then
  echo "ArchType 已安装。用普通用户运行 archtype-config --setup（或打开「ArchType 设置」点第一页的「一键设置」），"
  echo "它会把 ArchType 加进 fcitx5 的输入法列表，必要时重启 fcitx5。"
fi

%changelog
SPEC
  # 构建日期写进 changelog（rpmbuild 用它定 SOURCE_DATE_EPOCH）
  printf '* %s ArchType <gloridust@users.noreply.github.com> - %s-1\n- 从源码自动打包\n' \
    "$(LC_ALL=C date -u '+%a %b %d %Y')" "$VERSION"
} > "$work/SPECS/archtype.spec"

nsh_define=()
if [[ -n "${ARCHTYPE_NSH_ARCHIVE:-}" ]]; then
  # rpmbuild 的 %install 在 BUILDROOT 里跑，相对路径会失效：先转成绝对路径。
  [[ -f "$ARCHTYPE_NSH_ARCHIVE" ]] || {
    echo "找不到 NSH 设置界面产物：$ARCHTYPE_NSH_ARCHIVE" >&2
    exit 1
  }
  nsh_define+=(--define "archtype_nsh_archive $(realpath "$ARCHTYPE_NSH_ARCHIVE")")
fi
rpmbuild -bb "$work/SPECS/archtype.spec" \
  --define "_topdir $work" \
  --define "_rpmdir $OUT" \
  --define '_rpmfilename %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm' \
  "${nsh_define[@]}"
shopt -s nullglob
files=("$OUT"/archtype-"$VERSION"-1*."$(uname -m)".rpm)
(( ${#files[@]} )) || { echo "rpmbuild 没有生成 archtype-$VERSION 的 rpm" >&2; exit 1; }
printf '%s\n' "${files[@]}"
