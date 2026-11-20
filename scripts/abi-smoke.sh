#!/usr/bin/env bash
# 构建并运行 C ABI 冒烟测试（模拟 fcitx5 插件的调用序列）。
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p archtype-core -p archtype-config-cli

LEXICON=${1:-/tmp/archtype_abi.tsv}
# 自己生成的词库可以重建；调用方给的文件不覆盖，缺条目时说清楚
if [[ ! -f "$LEXICON" ]]; then
  printf '# abi smoke\nni hao\t你好\t100000\ntian\t天\t5000\ntian\t田\t2000\nzhong guo\t中国\t8000\nzhi dao\t知道\t6000\n' > "$LEXICON"
elif ! grep -q 'zhi dao' "$LEXICON" 2>/dev/null; then
  echo "词库 $LEXICON 里没有 zhi dao；冒烟测试需要它（不覆盖调用方给的文件）" >&2
  exit 1
fi

BIGRAM=${2:-/tmp/archtype_abi_bigram.tsv}
if [[ ! -f "$BIGRAM" ]]; then
  printf '# prev<TAB>cur<TAB>count\n今天\t天\t8\n今天\t田\t1\n' > "$BIGRAM"
fi

INCLUDE=core/include
LIBDIR=target/release
case "$(uname -s)" in
  Darwin) LIB=libarchtype_core.dylib; RUNENV="DYLD_LIBRARY_PATH=$LIBDIR" ;;
  *)      LIB=libarchtype_core.so;    RUNENV="LD_LIBRARY_PATH=$LIBDIR" ;;
esac

# 库要放在源文件后面：Ubuntu 默认 --as-needed，放前面会被丢掉
cc -O2 -I"$INCLUDE" -o /tmp/archtype_abi_smoke tools/abi_smoke.c -L"$LIBDIR" -larchtype_core
S2T="$PWD/core/data/s2t.tsv"
env $RUNENV /tmp/archtype_abi_smoke "$LEXICON" "$BIGRAM" "$S2T"
