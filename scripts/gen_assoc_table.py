#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""从 .opid 词库与短语表生成语义联想小表（Spec §3.8）。

输出 `root<TAB>suffix<TAB>rank`，无表头；root 字节升序，同 root 按
`(rank desc, len asc, bytes asc)`，与运行时的读取顺序一致。

两遍扫描：先按短语出现次数选根，再对选中根收集紧随其后的 2/3/4 字后缀。
不按 `roots × phrases` 做查找 —— 短语表有几万行、根有几千个。

用法：
    python3 scripts/gen_assoc_table.py --opid data/generated/luna.opid \\
        --phrases data/raw/extra_phrases.tsv --out data/generated/luna.assoc.tsv
"""
from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from merge_luna_kangxi import read_opid  # noqa: E402

# 后缀以这些虚词开头时不成搭配（Spec §3.8 第 4 条）。
PARTICLES = "的了呢吗啊啦吧得地"


@dataclass(frozen=True)
class Row:
    root: str
    suffix: str
    rank: int


def is_cjk(ch: str) -> bool:
    return "一" <= ch <= "鿿"


def all_cjk(text: str) -> bool:
    return bool(text) and all(is_cjk(c) for c in text)


def load_phrases(path: Path) -> tuple[list[str], int]:
    """短语表是三列 `word<TAB>pinyin<TAB>freq`：只取第一列，非空且全 CJK 才留。"""
    phrases: list[str] = []
    skipped = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        word = line.split("\t", 1)[0].strip()
        if not all_cjk(word):
            skipped += 1
            continue
        phrases.append(word)
    return phrases, skipped


def select_roots(words: dict[str, int], phrases: list[str], roots: int) -> list[str]:
    """2–3 字 CJK 词按短语出现次数降序（同次数按字节序）取前 `roots` 个。"""
    candidates = {w for w in words if 2 <= len(w) <= 3 and all_cjk(w)}
    counts = dict.fromkeys(candidates, 0)
    for phrase in phrases:
        for i in range(len(phrase)):
            for length in (2, 3):
                sub = phrase[i : i + length]
                if len(sub) == length and sub in counts:
                    counts[sub] += 1
    ordered = sorted(counts, key=lambda r: (-counts[r], r.encode("utf-8")))
    return ordered[:roots]


def collect_rows(
    roots: list[str], words: dict[str, int], phrases: list[str], cap: int
) -> list[Row]:
    """每个根取紧随其后的 2/3/4 字后缀，按 Spec §3.8 第 4–6 条过滤并取前 `cap`。"""
    root_set = set(roots)
    counts: dict[str, dict[str, int]] = {root: {} for root in root_set}
    for phrase in phrases:
        for i in range(len(phrase)):
            for root_len in (2, 3):
                root = phrase[i : i + root_len]
                if len(root) != root_len or root not in root_set:
                    continue
                rest = phrase[i + root_len :]
                for length in (2, 3, 4):
                    suffix = rest[:length]
                    if len(suffix) < length:
                        continue
                    if suffix[0] in PARTICLES:
                        continue
                    if root + suffix in words:  # 已是词库词：前缀索引负责
                        continue
                    counts[root][suffix] = counts[root].get(suffix, 0) + 1

    rows: list[Row] = []
    for root, suffixes in counts.items():
        ranked = sorted(
            suffixes,
            key=lambda s: (
                -words.get(root + s, 0),
                -suffixes[s],
                len(s),
                s.encode("utf-8"),
            ),
        )
        for suffix in ranked[:cap]:
            rows.append(Row(root, suffix, words.get(root + suffix, 0)))
    return rows


def format_rows(rows: list[Row]) -> list[str]:
    ordered = sorted(
        rows,
        key=lambda r: (r.root.encode("utf-8"), -r.rank, len(r.suffix), r.suffix.encode("utf-8")),
    )
    return [f"{r.root}\t{r.suffix}\t{r.rank}" for r in ordered]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--opid", type=Path, required=True)
    ap.add_argument("--phrases", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--roots", type=int, default=5000)
    ap.add_argument("--cap", type=int, default=3)
    args = ap.parse_args()

    words: dict[str, int] = {}
    for word, _pinyin, freq in read_opid(args.opid):
        words[word] = max(words.get(word, 0), freq)
    phrases, skipped = load_phrases(args.phrases)
    roots = select_roots(words, phrases, args.roots)
    rows = collect_rows(roots, words, phrases, args.cap)
    lines = format_rows(rows)
    args.out.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")
    print(
        f"roots {len(roots)}，pairs {len(rows)}，跳过短语 {skipped}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
