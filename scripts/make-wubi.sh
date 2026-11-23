#!/usr/bin/env bash
# 从 rime-wubi（Rime 五笔 86，LGPL-3.0）生成五笔码表。
#
#   scripts/make-wubi.sh [rime-wubi 目录] [输出目录]
#
# 不给 rime-wubi 目录时按下面钉死的提交下载三个文件（wubi86.dict.yaml、LICENSE、AUTHORS），
# 给了目录也一样核对 sha256，对不上就退出。输出（同样的输入逐字节相同）：
#   wubi86.tsv        code<TAB>词<TAB>频次，转换规则见 tools/wubi2tsv.py
#   WUBI-LICENSE      rime-wubi 的许可证（LGPL-3.0），随码表一起分发
#   WUBI-AUTHORS      rime-wubi 的作者与来源说明
#   WUBI-SOURCE.txt   来源、钉死的提交、转换方式
#   DATA-MANIFEST     数据清单（tools/datamark.py manifest），和 make-data.sh 输出到同一个目录时一起列上词库
# wubi86.tsv 第一行是版本标记 `# archtype-data: wubi86.tsv <版本> <来源>`（docs/DATA.md「数据版本标记」），
# 版本 = rime-wubi 提交日期 + 修订号 WUBI_REV。
# 文件名带 WUBI- 前缀，跟 make-data.sh 输出到同一个目录时不会盖掉 rime-ice 的 LICENSE、SOURCE.txt。
set -euo pipefail
cd "$(dirname "$0")/.."

# 换版本时三处一起改：提交、提交日期、各文件的 sha256（WUBI_REV 归 0）。
# 只改了转换方式（tools/wubi2tsv.py）、码表内容变了时 WUBI_REV 加一，插件才分得出新旧。
WUBI_REV=0
WUBI_REPO=https://github.com/rime/rime-wubi
WUBI_COMMIT=152a0d3f3efe40cae216d1e3b338242446848d07
WUBI_DATE=2023-10-25
WUBI_DICT_SHA256=f833d86b72341fe82e069a425b6625f29ef85f1bc0f34f6fb7975fe514888b5a
WUBI_LICENSE_SHA256=da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768
WUBI_AUTHORS_SHA256=4e4ca0e3ae6763a7cd5913f92c3b28091a7ec97d5cf4fc96c8f1a294e43f2839

SRC=${1:-}
OUT=${2:-dist/data}
mkdir -p "$OUT"

tmp=""
cleanup() { [[ -z "$tmp" ]] || rm -rf "$tmp"; }
trap cleanup EXIT

if [[ -z "$SRC" ]]; then
  tmp=$(mktemp -d)
  SRC=$tmp
  for name in wubi86.dict.yaml LICENSE AUTHORS; do
    curl -fsSL --retry 3 -o "$SRC/$name" \
      "https://raw.githubusercontent.com/rime/rime-wubi/${WUBI_COMMIT}/${name}"
  done
fi

# 不用 sha256sum -c：macOS 的 sha256sum 是 BSD 版；python3 两边都有
sha256() {
  python3 -c 'import hashlib, sys
h = hashlib.sha256()
with open(sys.argv[1], "rb") as f:
    for block in iter(lambda: f.read(1 << 20), b""):
        h.update(block)
print(h.hexdigest())' "$1"
}
check() {
  local file=$1 want=$2 got
  [[ -f "$file" ]] || { echo "没有 ${file}" >&2; exit 1; }
  got=$(sha256 "$file")
  if [[ "$got" != "$want" ]]; then
    echo "${file} 的 sha256 对不上（应为 ${want}，实际 ${got}）：不是钉死的 rime-wubi ${WUBI_COMMIT}" >&2
    exit 1
  fi
}
check "$SRC/wubi86.dict.yaml" "$WUBI_DICT_SHA256"
check "$SRC/LICENSE" "$WUBI_LICENSE_SHA256"
check "$SRC/AUTHORS" "$WUBI_AUTHORS_SHA256"

python3 tools/wubi2tsv.py "$SRC/wubi86.dict.yaml" -o "$OUT/wubi86.tsv"
[[ -s "$OUT/wubi86.tsv" ]] || { echo "wubi86.tsv 是空的" >&2; exit 1; }
python3 tools/datamark.py stamp "$OUT/wubi86.tsv" wubi86.tsv "${WUBI_DATE//-/.}.${WUBI_REV}" \
  --source "rime-wubi ${WUBI_COMMIT:0:12} (${WUBI_DATE})"

cp "$SRC/LICENSE" "$OUT/WUBI-LICENSE"
cp "$SRC/AUTHORS" "$OUT/WUBI-AUTHORS"
cat > "$OUT/WUBI-SOURCE.txt" <<EOF
wubi86.tsv（五笔 86 码表）
数据来源：Rime 五笔字型 rime-wubi（${WUBI_REPO}），码表源自 ibus-table / 极点五笔，
原始编码方案为王永民先生的五笔字型 86 版；作者与来源见 WUBI-AUTHORS。
许可证：LGPL-3.0（GNU Lesser General Public License v3），全文见 WUBI-LICENSE；
LGPL-3.0 是在 GPL-3.0 之上的附加许可，GPL-3.0 全文见同目录（或软件包）里的 LICENSE / COPYING。
版本：rime-wubi 提交 ${WUBI_COMMIT}（${WUBI_DATE}）
源文件：wubi86.dict.yaml，sha256 ${WUBI_DICT_SHA256}
下载：https://raw.githubusercontent.com/rime/rime-wubi/${WUBI_COMMIT}/wubi86.dict.yaml

wubi86.tsv 由 ArchType 的 tools/wubi2tsv.py 转换，scripts/make-wubi.sh 可重新生成（逐字节相同）：
第一行是版本标记（# archtype-data: wubi86.tsv ${WUBI_DATE//-/.}.${WUBI_REV} ...），
之后每行 code<TAB>词<TAB>频次，按码升序、频次降序；去掉了 z 开头的符号码、权重为 0 的扩展区生僻字与部首、
注释掉的条目和 stem（构词码）列；没有权重的《通用规范汉字表》字频次记 1。
ArchType 的源代码（含上面两个脚本）：https://github.com/Goway-Hui/ArchType
EOF

# 数据清单：同一目录里 make-data.sh 生成的词库、语言模型一起列上（它们要带版本标记）
python3 tools/datamark.py manifest "$OUT" core/data/s2t.tsv >/dev/null

echo "wubi86.tsv: $(grep -vc '^#' "$OUT/wubi86.tsv") 条"
echo "输出目录: $OUT"
