#!/usr/bin/env python3
"""训练 ArchType 自己的字级神经语言模型（整句增强模型），导出成 core/src/nnlm.rs 读的 WTNN 格式。

    python3 tools/nnlm/train.py --text DIR --out nnlm.bin [--steps 20000] [--version 2026.11.17.0]

DIR 里是 tools/nnlm/prep_corpus.py 生成的 wiki.txt、lccc.txt（一行一句的简体纯文本）。要 PyTorch（有 CUDA 更好）。

结构和 nnlm.rs 的推理代码一一对应（和微信输入法自带的那个同样大小，所以每步推理的耗时也一样）：
3 层 GPT-2，宽 256、4 个头、前馈 1024、最多 512 个位置，LayerNorm 在前，GELU 用 tanh 近似，
输出层和字向量共用（logit = wte[字]·y）。编号 0 = 填充，1 = <unk>，2 = <s>（和 nnlm.rs 的约定一样）。

WTNN 格式（小端）：
    b"WTNN"、u32 版本 1、u32 说明长度 + UTF-8 说明、u32 词表大小 V、u32 字数 N、N 组 (u32 码位, u32 编号)、
    float16 权重：wte[V*D]、wpe[512*D]、每层 ln1_g ln1_b attn_w[3D*D] attn_b[3D] proj_w[D*D] proj_b
    ln2_g ln2_b fc_w[FF*D] fc_b[FF] out_w[D*FF] out_b、最后 lnf_g lnf_b。
说明的第一行是数据版本标记 `archtype-data: nnlm.bin <版本> <来源>`（在文件前 4096 字节里，插件和设置界面
按它认用户目录里的旧副本，见 docs/DATA.md），后面是 JSON（名字、版本、语料、许可证）。
矩阵都按 [输出, 输入] 行存（和 PyTorch 的 Linear.weight 一样）。
"""

import argparse
import collections
import json
import math
import os
import struct
import time

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

D, HEADS, FF, LAYERS, MAX_POS = 256, 4, 1024, 3, 512
PAD, UNK, BOS = 0, 1, 2
SPECIAL = 3


class Block(nn.Module):
    def __init__(self):
        super().__init__()
        self.ln1 = nn.LayerNorm(D, eps=1e-5)
        self.attn = nn.Linear(D, 3 * D)
        self.proj = nn.Linear(D, D)
        self.ln2 = nn.LayerNorm(D, eps=1e-5)
        self.fc = nn.Linear(D, FF)
        self.out = nn.Linear(FF, D)

    def forward(self, x):
        b, t, _ = x.shape
        q, k, v = self.attn(self.ln1(x)).split(D, dim=2)
        shape = (b, t, HEADS, D // HEADS)
        q, k, v = (z.view(shape).transpose(1, 2) for z in (q, k, v))
        o = F.scaled_dot_product_attention(q, k, v, is_causal=True)
        x = x + self.proj(o.transpose(1, 2).reshape(b, t, D))
        return x + self.out(F.gelu(self.fc(self.ln2(x)), approximate="tanh"))


class Model(nn.Module):
    def __init__(self, vocab):
        super().__init__()
        self.wte = nn.Embedding(vocab, D)
        self.wpe = nn.Embedding(MAX_POS, D)
        self.blocks = nn.ModuleList(Block() for _ in range(LAYERS))
        self.lnf = nn.LayerNorm(D, eps=1e-5)
        nn.init.normal_(self.wte.weight, std=0.02)
        nn.init.normal_(self.wpe.weight, std=0.01)

    def forward(self, idx):
        pos = torch.arange(idx.shape[1], device=idx.device)
        x = self.wte(idx) + self.wpe(pos)
        for block in self.blocks:
            x = block(x)
        return self.lnf(x) @ self.wte.weight.T


def build_vocab(paths, max_chars, min_count):
    counts = collections.Counter()
    for path in paths:
        with open(path, encoding="utf-8") as f:
            for line in f:
                counts.update(line.rstrip("\n"))
    chars = [c for c, n in counts.most_common(max_chars) if n >= min_count and not c.isspace()]
    total = sum(counts.values())
    covered = sum(counts[c] for c in chars)
    print(f"字表 {len(chars)} 个字，覆盖 {covered / total:.4%}")
    return {c: SPECIAL + i for i, c in enumerate(chars)}


def encode(path, vocab, out):
    """一行一句 -> <s> 字 字 ... 的 uint16 流；返回每句开头（<s>）的位置。"""
    ids, starts = [], []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line:
                continue
            starts.append(len(ids))
            ids.append(BOS)
            ids.extend(vocab.get(c, UNK) for c in line)
    arr = np.asarray(ids, dtype=np.uint16)
    arr.tofile(out)
    return np.asarray(starts, dtype=np.int64), len(arr)


class Stream:
    """从某句的 <s> 开始取一段（可以跨到后面几句）。"""

    def __init__(self, path, starts, length, seq, holdout):
        self.data = np.memmap(path, dtype=np.uint16, mode="r", shape=(length,))
        cut = int(len(starts) * (1 - holdout))
        self.train = starts[:cut][starts[:cut] + seq + 1 <= length]
        self.valid = starts[cut:][starts[cut:] + seq + 1 <= length]
        self.seq = seq
        if len(self.train) == 0:
            raise SystemExit(
                f"训练池为空：语料太短（starts={len(starts)}，seq={seq}，holdout={holdout}）"
            )

    def batch(self, n, rng, valid=False):
        pool = self.valid if valid else self.train
        if len(pool) == 0:
            raise SystemExit("验证池为空：语料太短，调小 --holdout 或换更长的语料")
        picks = pool[rng.integers(0, len(pool), n)]
        rows = np.stack([self.data[p:p + self.seq + 1] for p in picks]).astype(np.int64)
        return torch.from_numpy(rows)


def export(model, vocab, path, meta):
    sd = {k: v.detach().float().cpu().numpy() for k, v in model.state_dict().items()}
    parts = [sd["wte.weight"], sd["wpe.weight"]]
    for l in range(LAYERS):
        p = f"blocks.{l}."
        parts += [sd[p + "ln1.weight"], sd[p + "ln1.bias"], sd[p + "attn.weight"], sd[p + "attn.bias"],
                  sd[p + "proj.weight"], sd[p + "proj.bias"], sd[p + "ln2.weight"], sd[p + "ln2.bias"],
                  sd[p + "fc.weight"], sd[p + "fc.bias"], sd[p + "out.weight"], sd[p + "out.bias"]]
    parts += [sd["lnf.weight"], sd["lnf.bias"]]
    vocab_size = sd["wte.weight"].shape[0]
    marker = f"archtype-data: nnlm.bin {meta['version']} archtype-nnlm LCCC+zhwiki"
    text = (marker + "\n" + json.dumps(meta, ensure_ascii=False)).encode("utf-8")
    blob = bytearray(b"WTNN")
    blob += struct.pack("<II", 1, len(text)) + text
    blob += struct.pack("<II", vocab_size, len(vocab))
    for ch, i in sorted(vocab.items(), key=lambda kv: kv[1]):
        blob += struct.pack("<II", ord(ch), i)
    for p in parts:
        blob += np.ascontiguousarray(p, dtype="<f2").tobytes()
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(blob)
    os.replace(tmp, path)
    print(f"导出 {path}：{len(blob) / 1e6:.1f} MB")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--text", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--work", help="放编码后的语料和检查点的目录，默认 --text 目录")
    ap.add_argument("--steps", type=int, default=20000)
    ap.add_argument("--batch", type=int, default=512)
    ap.add_argument("--seq", type=int, default=96)
    ap.add_argument("--lr", type=float, default=2e-3)
    ap.add_argument("--max-chars", type=int, default=7000)
    ap.add_argument("--min-count", type=int, default=50)
    ap.add_argument("--lccc-weight", type=float, default=0.6, help="每批里对话语料占的比例")
    ap.add_argument("--version", default=time.strftime("%Y.%m.%d.0"))
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--export-only", action="store_true", help="不训练，把 --work 里的 checkpoint.pt 导出成 --out")
    args = ap.parse_args()
    work = args.work or args.text
    os.makedirs(work, exist_ok=True)
    torch.manual_seed(args.seed)
    rng = np.random.default_rng(args.seed)
    device = "cuda" if torch.cuda.is_available() else "cpu"

    sources = {name: os.path.join(args.text, name + ".txt") for name in ("lccc", "wiki")}
    vocab_path = os.path.join(work, "vocab.json")
    if os.path.exists(vocab_path):
        with open(vocab_path, encoding="utf-8") as f:
            vocab = json.load(f)
    else:
        vocab = build_vocab(sources.values(), args.max_chars, args.min_count)
        with open(vocab_path, "w", encoding="utf-8") as f:
            json.dump(vocab, f, ensure_ascii=False)
    streams = {}
    for name, path in sources.items():
        bin_path = os.path.join(work, name + ".u16")
        idx_path = os.path.join(work, name + ".starts.npy")
        if os.path.exists(idx_path):
            starts = np.load(idx_path)
            length = os.path.getsize(bin_path) // 2
        else:
            starts, length = encode(path, vocab, bin_path)
            np.save(idx_path, starts)
        streams[name] = Stream(bin_path, starts, length, args.seq, holdout=0.002)
        print(f"{name}: {length / 1e6:.0f}M 个字符，{len(starts) / 1e6:.1f}M 句")

    vocab_size = SPECIAL + len(vocab)
    model = Model(vocab_size).to(device)
    meta = {
        "name": "archtype-nnlm",
        "version": args.version,
        "corpus": "LCCC (thu-coai, MIT) + 中文维基百科 (wikimedia/wikipedia 20231101.zh, CC BY-SA 4.0)",
        "license": "CC BY-SA 4.0",
        "steps": args.steps,
    }
    if args.export_only:
        model.load_state_dict(torch.load(os.path.join(work, "checkpoint.pt"), map_location=device))
        export(model, vocab, args.out, meta)
        return
    print(f"参数 {sum(p.numel() for p in model.parameters()) / 1e6:.2f}M")
    decay = [p for n, p in model.named_parameters() if p.dim() >= 2]
    no_decay = [p for n, p in model.named_parameters() if p.dim() < 2]
    opt = torch.optim.AdamW([{"params": decay, "weight_decay": 0.05},
                             {"params": no_decay, "weight_decay": 0.0}],
                            lr=args.lr, betas=(0.9, 0.98), fused=device == "cuda")
    warmup = 500

    def lr_at(step):
        if step < warmup:
            return args.lr * step / warmup
        progress = (step - warmup) / max(1, args.steps - warmup)
        return args.lr * (0.05 + 0.95 * 0.5 * (1 + math.cos(math.pi * progress)))

    n_lccc = int(args.batch * args.lccc_weight)

    def get_batch(valid=False):
        rows = torch.cat([streams["lccc"].batch(n_lccc, rng, valid),
                          streams["wiki"].batch(args.batch - n_lccc, rng, valid)])
        rows = rows.to(device, non_blocking=True)
        return rows[:, :-1], rows[:, 1:]

    def loss_of(x, y):
        with torch.autocast(device_type=device, dtype=torch.bfloat16, enabled=device == "cuda"):
            logits = model(x)
        return F.cross_entropy(logits.float().reshape(-1, vocab_size), y.reshape(-1), ignore_index=PAD)

    started = time.time()
    for step in range(1, args.steps + 1):
        for g in opt.param_groups:
            g["lr"] = lr_at(step)
        x, y = get_batch()
        loss = loss_of(x, y)
        opt.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        opt.step()
        if step % 500 == 0 or step == args.steps:
            model.eval()
            with torch.no_grad():
                vl = sum(loss_of(*get_batch(valid=True)).item() for _ in range(8)) / 8
            model.train()
            print(f"step {step} train {loss.item():.3f} valid {vl:.3f} "
                  f"({(time.time() - started) / 60:.1f} 分钟)", flush=True)
            torch.save(model.state_dict(), os.path.join(work, "checkpoint.pt"))

    export(model, vocab, args.out, meta)


if __name__ == "__main__":
    main()
