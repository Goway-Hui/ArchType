{
  description = "ArchType: WeChat-style pinyin input method for fcitx5";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      # CI 用本轮 CI 打好的设置界面包（不下载发布页上的旧资产）：
      #   ARCHTYPE_SETTINGS_ARCHIVE=$PWD/dist/settings/xxx.tar.gz nix build .#default --impure
      # 普通构建走纯求值，getEnv 恒为空，仍然按 package.nix 的 settingsAssets 下载。
      localSettings =
        let
          path = builtins.getEnv "ARCHTYPE_SETTINGS_ARCHIVE";
        in
        if path == "" then null else builtins.path { path = path; };
    in
    {
      # NixOS：i18n.inputMethod.fcitx5.addons = [ inputs.archtype.packages.${system}.default ];
      packages = forAllSystems (pkgs: rec {
        archtype = pkgs.callPackage ./nix/package.nix { settingsArchive = localSettings; };
        default = archtype;
      });

      # 想用 pkgs.archtype：nixpkgs.overlays = [ inputs.archtype.overlays.default ];
      overlays.default = final: _prev: {
        archtype = final.callPackage ./nix/package.nix { };
      };

      # nix flake check：装好的自检工具能用真实词库打出「你好」，设置界面的自测能过
      checks = forAllSystems (
        pkgs:
        let
          archtype = self.packages.${pkgs.stdenv.hostPlatform.system}.archtype;
        in
        {
          inherit archtype;
          smoke = pkgs.runCommand "archtype-smoke" { nativeBuildInputs = [ archtype ]; } ''
            export HOME=$TMPDIR
            test -f ${archtype}/lib/fcitx5/libarchtype.so
            test -f ${archtype}/share/fcitx5/addon/archtype.conf
            test -f ${archtype}/share/fcitx5/inputmethod/archtype.conf
            test -s ${archtype}/share/archtype/words.lm
            archtype-check
            archtype-show --lexicon ${archtype}/share/archtype/lexicon.tsv nihao > show.txt
            cat show.txt
            grep -q '1\.你好' show.txt
            archtype-config --self-test
            # 图形设置界面（NSH）：包装脚本和它解出来的运行时都在
            test -x ${archtype}/bin/archtype-settings
            test -d ${archtype}/lib/archtype/settings
            touch $out
          '';
        }
      );
    };
}
