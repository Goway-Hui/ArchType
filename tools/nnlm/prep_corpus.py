#!/usr/bin/env python3
"""把训练语料整理成一行一句的简体纯文本，给 tools/nnlm/train.py 用。

    python3 tools/nnlm/prep_corpus.py --s2t core/data/s2t.tsv --out DIR \
        --wiki wiki-*.parquet --lccc lccc_base_train.jsonl.gz

- 维基百科（Hugging Face wikimedia/wikipedia 的 zh 分片，CC BY-SA 4.0）：按句号、问号、叹号、分号、换行切句，
  只留汉字占一半以上、4–120 个字的句子；去掉「参考文献」之类的段落标题行（太短，自然被滤掉）。
- LCCC（thu-coai，MIT）：对话里每一句去掉分词空格，一句一行。
- 繁体按 s2t.tsv 反过来逐字转成简体（一对多的取第一个；词组表不用，逐字够训练用）。

输出 DIR/wiki.txt、DIR/lccc.txt。确定性：同样的输入得到同样的输出。
"""

import argparse
import gzip
import json
import os
import re

CJK = re.compile(r"[㐀-鿿]")
SPLIT = re.compile(r"(?<=[。！？；!?;])|\n")


def t2s_table(path):
    table = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#"):
                continue
            parts = line.rstrip("\n").split("\t")
            if len(parts) != 2 or len(parts[0]) != 1:
                continue
            simple = parts[0]
            for trad in parts[1].split():
                if len(trad) == 1 and trad != simple and ord(trad) not in table:
                    table[ord(trad)] = simple
    return table


def good(sentence):
    n = len(sentence)
    if n < 4 or n > 120:
        return False
    return len(CJK.findall(sentence)) * 2 >= n


def prep_wiki(paths, table, out):
    import pyarrow.parquet as pq
    kept = 0
    with open(out, "w", encoding="utf-8") as f:
        for path in paths:
            pf = pq.ParquetFile(path)
            for batch in pf.iter_batches(columns=["text"], batch_size=2000):
                for text in batch.column(0).to_pylist():
                    text = text.translate(table)
                    for s in SPLIT.split(text):
                        s = s.strip()
                        if good(s):
                            f.write(s + "\n")
                            kept += 1
    return kept


def prep_lccc(path, table, out):
    kept = 0
    with gzip.open(path, "rt", encoding="utf-8") as src, open(out, "w", encoding="utf-8") as f:
        for line in src:
            for utt in json.loads(line):
                s = utt.replace(" ", "").translate(table).strip()
                if good(s):
                    f.write(s + "\n")
                    kept += 1
    return kept


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--s2t", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--wiki", nargs="*", default=[])
    ap.add_argument("--lccc")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    table = t2s_table(args.s2t)
    print(f"繁 -> 简 {len(table)} 个字")
    if args.lccc:
        print("lccc", prep_lccc(args.lccc, table, os.path.join(args.out, "lccc.txt")), "句")
    if args.wiki:
        print("wiki", prep_wiki(sorted(args.wiki), table, os.path.join(args.out, "wiki.txt")), "句")


if __name__ == "__main__":
    main()
