#!/usr/bin/env bash
# 发版后让 Nix 包跟上这一版：词库换了就改 dataVersion/url/hash，设置界面包换了就改
# settingsAssets 里两个架构的 hash，一起推到 bot/nix-follow-<tag> 分支，开 PR 到 main。
# 同一个 tag 再跑只更新这个分支（PR 跟着变），不重复开。
#
# Actions 不能开 PR 时（仓库设置 Actions → General →「Allow GitHub Actions to create and approve pull
# requests」没开），退回开 issue，附上已经推好的分支的对比链接，点一下就能开 PR。
#
# 用法：nix-data-pr.sh <词库包> <设置界面 amd64 包> <设置界面 aarch64 包>；
# 要 GITHUB_REPOSITORY、GITHUB_REF_NAME、GH_TOKEN。
# NIX_DATA_PR_DRY_RUN=1 时只改文件、不推送不开 PR（本机试用）。
set -euo pipefail

tarball_path=$1
settings_amd64=$2
settings_arm64=$3
tarball=$(basename "$tarball_path")
built=${tarball#archtype-data-rime-ice-}
built=${built%.tar.gz}
nix=nix/package.nix
repo=$GITHUB_REPOSITORY
tag=$GITHUB_REF_NAME
dry=${NIX_DATA_PR_DRY_RUN:-}

git fetch -q origin +refs/heads/main:refs/remotes/origin/main
pinned=$(git show "origin/main:$nix" | sed -n 's/^ *dataVersion = "\(.*\)";/\1/p')

hex=$(sha256sum "$tarball_path" | cut -d' ' -f1)
url="https://github.com/${repo}/releases/download/${tag}/${tarball}"
sri=$(python3 -c 'import base64, sys; print("sha256-" + base64.b64encode(bytes.fromhex(sys.argv[1])).decode())' "$hex")

# 设置界面包：两个架构各一个 hash，URL 里是 ${version}，不用改
settings_sri() {
  local path=$1
  local h
  h=$(sha256sum "$path" | cut -d' ' -f1)
  if [ -z "$dry" ]; then
    local remote
    remote=$(curl -fsSL --retry 3 "https://github.com/${repo}/releases/download/${tag}/$(basename "$path")" | sha256sum | cut -d' ' -f1)
    if [ "$remote" != "$h" ]; then
      echo "::error title=Nix 设置界面::发布页上的 $(basename "$path")（${remote}）和本地算 hash 的（${h}）不一样"
      exit 1
    fi
  fi
  python3 -c 'import base64, sys; print("sha256-" + base64.b64encode(bytes.fromhex(sys.argv[1])).decode())' "$h"
}
sri_amd64=$(settings_sri "$settings_amd64")
sri_arm64=$(settings_sri "$settings_arm64")

if [ -z "$dry" ]; then
  # 词库也要和发布页上的那一份对一次
  remote=$(curl -fsSL --retry 3 "$url" | sha256sum | cut -d' ' -f1)
  if [ "$remote" != "$hex" ]; then
    echo "::error title=Nix 词库::发布页上的 ${tarball}（${remote}）和本地算 hash 的（${hex}）不一样"
    exit 1
  fi
fi

branch="bot/nix-follow-${tag}"
git switch -q -C "$branch" origin/main
python3 - "$nix" "$built" "$tag" "$sri" "$sri_amd64" "$sri_arm64" "$pinned" "$repo" <<'PY'
import re, sys
path, built, tag, sri, sri_amd64, sri_arm64, pinned, repo = sys.argv[1:]
s = open(path, encoding="utf-8").read()
if built != pinned:
    # 整行换成这一版的地址：仓库也统一到本仓库（原来的 Nix 包指着上流仓库的一个旧版本）
    url = f'https://github.com/{repo}/releases/download/{tag}/archtype-data-rime-ice-${{dataVersion}}.tar.gz'
    subs = [
        (r'(\n *dataVersion = )"[^"]*";', rf'\1"{built}";'),
        (r'(\n  data = fetchurl \{\n(?:    [^\n]*\n)*?    url = )"[^"]*";', rf'\1"{url}";'),
        # data = fetchurl { 里面那行 hash（网址里有 ${dataVersion}，不能按花括号找结尾，按缩进逐行找）
        (r'(\n  data = fetchurl \{(?:\n    [^\n]*)*?\n    hash = )"[^"]*";', rf'\1"{sri}";'),
    ]
    for pattern, repl in subs:
        s, n = re.subn(pattern, repl, s, count=1)
        if n != 1:
            sys.exit(f"nix/package.nix 里找不到要改的那行：{pattern}")
for arch, digest in (("x86_64-linux", sri_amd64), ("aarch64-linux", sri_arm64)):
    pattern = r'(\n    ' + arch + r' = \{\n(?:      [^\n]*\n)*?      hash = )"[^"]*";'
    s, n = re.subn(pattern, rf'\1"{digest}";', s, count=1)
    if n != 1:
        sys.exit(f"nix/package.nix 里找不到 settingsAssets 的 {arch}")
open(path, "w", encoding="utf-8").write(s)
PY
if [ -z "$(git diff --name-only -- "$nix")" ]; then
  echo "main 上 Nix 包的词库与设置界面都已经是 ${tag} 的"
  exit 0
fi
git diff -- "$nix"
[ -n "$dry" ] && exit 0

git -c user.name="github-actions[bot]" \
  -c user.email="41898282+github-actions[bot]@users.noreply.github.com" \
  commit -q -m "Nix 包跟上 ${tag}" -m "词库 ${built}，设置界面包按发布页上的两个架构更新 hash。" -- "$nix"
git push -q --force origin "$branch"

title="Nix 包跟上 ${tag}"
existing=$(gh pr list --repo "$repo" --head "$branch" --state open --json number -q '.[0].number // empty')
if [ -n "$existing" ]; then
  echo "PR #${existing} 已经开着，分支已更新到 ${tag} 的词库包"
  exit 0
fi

cat > nix-pr.md <<MD
${tag} 发了新词库和设置界面包，main 上的 nix/package.nix 还指向旧的，NixOS 用户拿到的还是上一版。

这个 PR 由发布流程自动生成，只改 nix/package.nix：

- 词库 \`${built}\`（\`dataVersion\`、URL、hash）
- 设置界面的 x86_64 与 aarch64 两个包的 hash

所有 hash 都是按上传到发布页的文件算的，并已重新下载核对过。合进 main 后，跟 release 分支的 NixOS 用户会拿到这一版的设置界面与词库。

Actions 开的 PR 不会自动跑 CI；要跑的话关掉再重新打开这个 PR。
MD
if gh pr create --repo "$repo" --base main --head "$branch" --title "$title" --body-file nix-pr.md; then
  exit 0
fi

echo "::warning title=Nix 词库::开不了 PR（仓库没允许 Actions 创建 PR），改开 issue"
if [ -n "$(gh issue list --repo "$repo" --state open --search "\"${title}\" in:title" --json number -q '.[0].number // empty')" ]; then
  echo "同名 issue 已经开着"
  exit 0
fi
{
  cat nix-pr.md
  echo
  echo "分支已经推好，点这里开 PR：https://github.com/${repo}/compare/main...${branch}?expand=1"
} > nix-issue.md
gh issue create --repo "$repo" --title "$title" --body-file nix-issue.md
