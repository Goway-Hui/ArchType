# ArchType：fcitx5 插件 + 设置界面 + 自检工具 + 词库。
# 用法见 flake.nix 和 docs/INSTALL.md 的「NixOS / Nix」一节。
{
  lib,
  stdenv,
  fetchurl,
  rustPlatform,
  cargo,
  rustc,
  cmake,
  kdePackages,
  pkg-config,
  gobject-introspection,
  wrapGAppsHook4,
  fcitx5,
  alsa-lib,
  gtk4,
  libadwaita,
  python3,
}:

let
  # 和 core/Cargo.toml 保持一致
  version = (lib.importTOML ../core/Cargo.toml).package.version;

  # 词库不在 git 里：用发布页上的词库压缩包（scripts/package-deb.sh data 打的），按哈希固定。
  # 换了词库的版本发版后，发布流程会自动开 PR 改这三行（.github/scripts/nix-data-pr.sh）；
  # 手动改的话 hash 在发布页 SHA256SUMS 里，nix hash convert --hash-algo sha256 <十六进制> 转成 SRI 格式。
  dataVersion = "2026.11.11.3";
  data = fetchurl {
    url = "https://github.com/Gloridust/WowType/releases/download/v0.3.12/wowtype-data-rime-ice-${dataVersion}.tar.gz";
    hash = "sha256-6gfD6M9my/fQ68I6uVzJY4Tm84CP0M3uH4eQUyLlHXw=";
  };

  pythonEnv = python3.withPackages (ps: [ ps.pygobject3 ]);
in
stdenv.mkDerivation {
  pname = "archtype";
  inherit version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../core
      ../voice
      ../voice-daemon
      ../fcitx5-addon
      ../icons
      ../themes
      ../settings
      ../README.md
    ];
  };

  cargoDeps = rustPlatform.importCargoLock { lockFile = ../Cargo.lock; };

  nativeBuildInputs = [
    rustPlatform.cargoSetupHook
    cargo
    rustc
    cmake
    kdePackages.extra-cmake-modules
    pkg-config
    gobject-introspection
    wrapGAppsHook4
  ];

  buildInputs = [
    fcitx5
    alsa-lib
    gtk4
    libadwaita
    pythonEnv
  ];

  # 自检工具默认去 /usr/share/archtype 找词库，改成本包的目录
  postPatch = ''
    substituteInPlace core/src/bin/check.rs \
      --replace-fail /usr/share/archtype "$out/share/archtype"
  '';

  # 插件静态链接 Rust 核心：CMake 配置时 target/release/libarchtype_core.a 必须已经在
  preConfigure = ''
    cargo build --release --frozen --offline -p archtype-core
  '';

  cmakeDir = "../fcitx5-addon";
  cmakeFlags = [
    (lib.cmakeFeature "ARCHTYPE_VERSION" version)
    # fcitx5 的 CMake 配置里这个目录指向 fcitx5 自己的 store 路径，要装进本包
    (lib.cmakeFeature "FCITX_INSTALL_PKGDATADIR" "${placeholder "out"}/share/fcitx5")
  ];

  postInstall = ''
    rm -rf "$out/include"
    for bin in archtype-check archtype-show; do
      install -Dm755 "../target/release/$bin" "$out/bin/$bin"
    done

    mkdir -p data
    tar -xzf ${data} -C data
    install -Dm644 -t "$out/share/archtype" data/lexicon.tsv data/english.txt data/words.lm
    # 数据清单（docs/DATA.md「数据版本标记」）：REV=6 起的词库包里才有，钉的还是旧包时没有
    if [ -f data/DATA-MANIFEST ]; then
      install -Dm644 -t "$out/share/archtype" data/DATA-MANIFEST
    fi
    # 整句增强模型 nnlm.bin（ArchType 自己训的，CC BY-SA 4.0）：词库包是 package-deb.sh data 打的，
    # make-data.sh 下载到了模型的那次发版起包里才有它；钉的还是更早的包时没有，插件只是不做整句增强
    # （用户可以自己导入微信输入法的模型）。发版后 nix-data-pr.sh 换成新包，就自动带上
    if [ -f data/nnlm.bin ]; then
      install -Dm644 -t "$out/share/archtype" data/nnlm.bin
    fi
    install -Dm644 ../README.md "$out/share/doc/archtype/README.md"
    install -Dm644 data/SOURCE.txt "$out/share/doc/archtype/DATA-SOURCE.txt"
    echo ${dataVersion} > "$out/share/doc/archtype/DATA-VERSION"
    install -Dm644 data/LICENSE "$out/share/licenses/archtype/LICENSE.rime-ice"
    # 五笔码表（rime-wubi，LGPL-3.0）：make-data.sh 的 REV=5 起，词库包里才有它。上面钉的还是旧包时没有它，
    # 设置里五笔灰掉；发版后 nix-data-pr.sh 换成新包，这个判断就总是成立
    if [ -f data/wubi86.tsv ]; then
      install -Dm644 -t "$out/share/archtype" data/wubi86.tsv
      install -Dm644 -t "$out/share/doc/archtype" data/WUBI-SOURCE.txt data/WUBI-AUTHORS
      install -Dm644 data/WUBI-LICENSE "$out/share/licenses/archtype/LICENSE.rime-wubi"
    fi

    # 设置界面：用带 PyGObject 的 Python，词库路径指到本包
    substituteInPlace "$out/bin/archtype-config" \
      --replace-fail '#!/usr/bin/python3' '#!${pythonEnv.interpreter}' \
      --replace-fail /usr/share/archtype "$out/share/archtype" \
      --replace-fail /usr/share/doc/archtype "$out/share/doc/archtype"
  '';

  # 只给设置界面套 GTK 环境（GI_TYPELIB_PATH 等），别的 ELF 不用
  dontWrapGApps = true;
  postFixup = ''
    wrapGApp "$out/bin/archtype-config"
  '';

  passthru = {
    inherit data dataVersion;
  };

  meta = {
    description = "WeChat-style pinyin input method for fcitx5, with bundled dictionaries and language model";
    homepage = "https://github.com/Goway-Hui/ArchType";
    license = with lib.licenses; [
      mit
      asl20
      unicode-30
      gpl3Only # 词库（rime-ice）
      cc-by-sa-40 # 词级语言模型 words.lm、整句增强模型 nnlm.bin
      lgpl3Only # 五笔码表 wubi86.tsv（rime-wubi）
    ];
    platforms = lib.platforms.linux;
    mainProgram = "archtype-config";
  };
}
