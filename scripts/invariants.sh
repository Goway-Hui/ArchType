#!/usr/bin/env bash
# 排序不变量：把 tools/eval/ 下全部用例逐键打一遍，检查「不管排序怎么调都不许出现」的结果
# （照着打的拼音被纠错的抢了第一、第一个音节的单字没了、候选只剩几个……）。一条都不许违反，
# tools/eval/invariants-allow.tsv 里写明了理由的除外。规则见 core/src/bin/common/invariants.rs、
# docs/ENGINE-INVARIANTS.md。CI 的「排序不变量」任务跑的就是它。
#
#   scripts/invariants.sh <数据目录> [--nnlm <nnlm.bin>] [wowtype-eval 的其它参数，比如 --inv-limit 0]
#
# 每个前缀没有上文查一次、带用例自带的上文和下面 CONTEXTS 里的上文各查一次；给了 --nnlm 时每种再带上
# 神经语言模型查一次（慢一倍多）。
#
# 有 $GITHUB_STEP_SUMMARY 时把结果追加进去。WOWTYPE_EVAL_BIN 指定现成的 wowtype-eval，不给就现编一个。
# 数据目录不可写时（比如 /usr/share/wowtype）先复制到临时目录：载入词库、模型会在旁边写缓存。
set -euo pipefail
cd "$(dirname "$0")/.."

DATA=${1:?用法：scripts/invariants.sh <数据目录> [--nnlm <nnlm.bin>] [wowtype-eval 的参数]}
shift
EVAL_DIR=tools/eval
ALLOW=$EVAL_DIR/invariants-allow.tsv
SETS=(regress common mid rare jianpin trunc trunc_exact homophone2 homophone3 daily context)
# 每个前缀都带着查一次的上文：72bf8e2 修的 cehaohou 就是接着「我本地」打时整个没了「测好」
CONTEXTS=(我本地)

for f in lexicon.tsv words.lm english.txt; do
  if [[ ! -s "$DATA/$f" ]]; then
    echo "数据目录 ${DATA} 里没有 ${f}（要 scripts/make-data.sh 的输出）" >&2
    exit 1
  fi
done

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
if [[ ! -w "$DATA" ]]; then
  mkdir "$tmp/data"
  for f in lexicon.tsv words.lm english.txt VERSION SOURCE.txt; do
    [[ -f "$DATA/$f" ]] && cp "$DATA/$f" "$tmp/data/"
  done
  DATA=$tmp/data
fi

extra=()
while (($#)); do
  if [[ "$1" == "--nnlm" ]]; then
    model=${2:?--nnlm 要一个文件}
    # 模型旁边也会写缓存，不可写时一样复制
    if [[ ! -w "$(dirname "$model")" ]]; then
      cp "$model" "$tmp/nnlm.bin"
      model=$tmp/nnlm.bin
    fi
    extra+=(--nnlm "$model")
    shift 2
  else
    extra+=("$1")
    shift
  fi
done

BIN=${WOWTYPE_EVAL_BIN:-}
if [[ -z "$BIN" ]]; then
  cargo build --release --locked -p wowtype-core --no-default-features --bin wowtype-eval >&2
  BIN=${CARGO_TARGET_DIR:-target}/release/wowtype-eval
fi

args=(--data "$DATA" --invariants --inv-out "$tmp/violations.tsv")
for name in "${SETS[@]}"; do
  args+=(--eval "$EVAL_DIR/$name.tsv")
done
for c in "${CONTEXTS[@]}"; do
  args+=(--inv-context "$c")
done
[[ -f "$ALLOW" ]] && args+=(--inv-allow "$ALLOW")

status=0
"$BIN" "${args[@]}" ${extra[@]+"${extra[@]}"} | tee "$tmp/report.txt" || status=$?

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "## 排序不变量"
    echo
    echo '```'
    sed -n '/^== 不变量/,$p' "$tmp/report.txt"
    echo '```'
    if ((status)); then
      echo
      echo "**有不变量被违反了。** 本地复现：\`scripts/invariants.sh <数据目录> --inv-limit 0\`"
    fi
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit "$status"
