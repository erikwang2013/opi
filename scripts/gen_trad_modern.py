#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""现代汉语常用词 → 繁体词组层（`data/raw/trad_modern.tsv`，产物提交入库）。

用户裁决（2026-10-18）：「增加繁体…的词语，在输入的时候匹配」。
`trad_phrases.tsv` 的 terra 段是文言/成语/地名表（gen_trad_dict.py 头注已记），
现代词（計算機/瀏覽器/人工智能…）覆盖不足。本脚本把 **rime-pinyin-simp 的常用简体
词组**（Apache-2.0，与 gen_luna_dict 同一 commit + SHA-256 pin）与项目人工表
`extra_manual.tsv`（MIT）用 OpenCC 做**词组级**简→繁转换（`s2t.json`；不是逐字
映射 —— 头发/出发 这类同形异繁必须靠词组表），再剔除 `trad_phrases.tsv` 已有的
(词形, 拼音)，产出可直接被 `opi-tools compile` 消费的三列 TSV（固定词频
MODERN_FREQ，低于既有 terra 词组段）。

为什么不用 luna.opid 的前 10 万高频词组：luna 合并了 rime-ice（GPL-3.0-only）
等层，取它的词会让 trad.opid 从 LGPL-3.0 变成 GPL-3.0；pinyin_simp 是 Apache-2.0，
与 trad 现有许可相容。代价：pinyin_simp 只有 2–4 字常用词（约 4.9 万行），
比 luna 的 88 万行小，但覆盖的正是现代常用词；长尾专名不进本层。

依赖：OpenCC CLI（Debian/Ubuntu: `apt-get install opencc`；本产物用 1.1.5 生成）。
      换 OpenCC 版本会改变个别异体字（麵/麪 等），重跑后必须过门禁并如实记录。
用法：python3 scripts/gen_trad_modern.py [--simp <pinyin_simp.dict.yaml>] [--opencc opencc]
      缺省从 pin 的 rime-pinyin-simp 下载；离线时用 --simp 指本地文件。
"""
import argparse
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gen_luna_dict import SIMP_SHA256, SIMP_URL, fetch  # noqa: E402  （pin 单一来源）

REPO = Path(__file__).resolve().parent.parent
TRAD_PHRASES = REPO / "data/raw/trad_phrases.tsv"
EXTRA_MANUAL = REPO / "data/raw/extra_manual.tsv"
OUT = REPO / "data/raw/trad_modern.tsv"

MODERN_FREQ = 50_000
MAX_LEN = 8

# CJK 统一表意文字（基本区 + 扩展 A/B；与 hanzi_freq.CJK_RANGES 同范围）。
CJK = ((0x3400, 0x4DBF), (0x4E00, 0x9FFF), (0x20000, 0x2A6DF))


def is_cjk(ch: str) -> bool:
    o = ord(ch)
    return any(lo <= o <= hi for lo, hi in CJK)


def read_simp(text: str) -> list[tuple[str, str]]:
    """pinyin_simp.dict.yaml → [(词, 连写拼音)]，保持文件序、按词去重。"""
    out = []
    seen = set()
    for line in text.splitlines():
        if not line or line.startswith("#") or line.startswith("-") or "\t" not in line:
            continue
        cols = line.split("\t")
        if len(cols) < 2:
            continue
        word = cols[0].strip()
        pinyin = cols[1].replace(" ", "").strip()
        if not word or not pinyin or word in seen:
            continue
        if not 2 <= len(word) <= 4 or not all(is_cjk(c) for c in word):
            continue
        seen.add(word)
        out.append((word, pinyin))
    return out


def read_manual(path: Path) -> list[tuple[str, str]]:
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        cols = line.split("\t")
        if len(cols) < 2:
            continue
        word, pinyin = cols[0].strip(), cols[1].strip()
        if word and pinyin and len(word) <= MAX_LEN and all(is_cjk(c) for c in word):
            out.append((word, pinyin))
    return out


def existing_keys(path: Path) -> set[tuple[str, str]]:
    keys = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        cols = line.split("\t")
        if len(cols) >= 2 and cols[0] and cols[1]:
            keys.add((cols[0], cols[1]))
    return keys


def convert(words: list[str], opencc: str) -> list[str]:
    """一次进程转换整批词；行数必须一一对应（OpenCC 不改行数）。"""
    proc = subprocess.run(
        [opencc, "-c", "s2t.json"],
        input="\n".join(words) + "\n",
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit(f"opencc 失败（{proc.returncode}）：{proc.stderr}")
    out = proc.stdout.splitlines()
    if len(out) != len(words):
        raise SystemExit(f"OpenCC 行数变了：{len(words)} → {len(out)}")
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--simp", default=None, help="本地 pinyin_simp.dict.yaml（缺省按 pin 下载）")
    ap.add_argument("--opencc", default="opencc")
    args = ap.parse_args()

    simp_text = (
        Path(args.simp).read_text(encoding="utf-8")
        if args.simp
        else fetch(SIMP_URL, SIMP_SHA256)
    )
    source = read_simp(simp_text)
    # 人工表接在后面；同词保留先出现者（pinyin_simp 的注音已经过 gen_luna_dict 校验）。
    seen_words = {w for w, _ in source}
    for w, py in read_manual(EXTRA_MANUAL):
        if w not in seen_words:
            seen_words.add(w)
            source.append((w, py))

    converted = convert([w for w, _ in source], args.opencc)
    existing = existing_keys(TRAD_PHRASES)
    out: list[str] = []
    seen: set[tuple[str, str]] = set()
    skipped = bad = 0
    for (_, py), t in zip(source, converted):
        t = t.strip()
        if not t or len(t) > MAX_LEN or any(not is_cjk(c) for c in t):
            bad += 1
            continue
        key = (t, py)
        if key in existing or key in seen:
            skipped += 1
            continue
        seen.add(key)
        out.append(f"{t}\t{py}\t{MODERN_FREQ}")

    OUT.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"源词组 {len(source)}（pinyin_simp + 人工表）；保留 {len(out)} "
          f"（与 trad_phrases 重复 {skipped}，过滤 {bad}）")
    print(f"写入 {OUT.relative_to(REPO)}（词频常数 {MODERN_FREQ}）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
