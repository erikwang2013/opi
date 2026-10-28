#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""单字读音覆盖表（`data/raw/single_reading_overrides.tsv`）的解析。

格式：`字<TAB>读音<TAB>词典`（词典 = `luna` / `trad`），一行一个 (字, 读音)；
同一字可有多个读音，两个词典各写各的行。表由 OPI 项目人工撰写（MIT），
用途与理由见 `data/raw/LICENSES.md` 的 B 类读音节。
"""
from pathlib import Path

TSV = Path(__file__).resolve().parent.parent / "data" / "raw" / "single_reading_overrides.tsv"


def load(target: str, known: set[str] | None = None) -> dict[str, list[str]]:
    """返回 `{字: [读音, ...]}`（只含 `target` 词典的行）。坏行/重复行直接报错；
    给了 `known` 时，表里出现单字表没有的字也报错。"""
    out: dict[str, list[str]] = {}
    seen: set[tuple[str, str, str]] = set()
    for line in TSV.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) != 3:
            raise RuntimeError(f"{TSV}: 每行必须是「字<TAB>读音<TAB>词典」，实得 {line!r}")
        ch, py, tgt = parts
        if tgt not in ("luna", "trad"):
            raise RuntimeError(f"{TSV}: 词典必须是 luna/trad，实得 {tgt!r}")
        if (ch, py, tgt) in seen:
            raise RuntimeError(f"{TSV}: 重复行 {line!r}")
        seen.add((ch, py, tgt))
        if tgt == target:
            out.setdefault(ch, []).append(py)
    if known is not None:
        unknown = sorted(set(out) - known)
        if unknown:
            raise RuntimeError(f"{TSV}: 字不在 {target} 单字表：{''.join(unknown)}")
    return out


def merge(
    ch: str,
    kept: list[str],
    extra: set[str],
    overrides: dict[str, list[str]],
    tail: set[tuple[str, str]],
) -> tuple[list[str], int]:
    """把覆盖表的读音并入 kept/extra；**覆盖表新补**的对记进 tail（排序时沉底，
    避免非音节形 / 低频读音把常用字与康熙导入层挤出手机抓取窗）。返回 (读音列表, 新补对数)。"""
    ov = set(overrides.get(ch, []))
    new = ov - set(kept) - extra
    tail.update((ch, py) for py in new)
    return sorted(set(kept) | extra | ov), len(new)
