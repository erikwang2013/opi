#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""词库 freq 的位置编码：固定容量 + 固定层基址（gen_luna_dict / gen_trad_dict 共用）。

freq = FMAX - (base + idx) × (FMAX // CAPACITY)：间距只由容量决定，**与条目数无关**。
旧式 `FMAX // n` 的间距随条目数变化 —— n 差 7 行，全表 freq 全部重算（2026-10-28 实测
trad 两个 TSV 各 ~9.6 万行 diff）。层基址固定 ⇒ 单字层增长不顺延词组层（同次 B 类读音
补进单字层，旧式会让全部词组行 freq 变化）。

粒度如实：位置编码只对**层尾追加**稳定。按排序键插到层中间的新条目会让它之后的条目
整体顺延一个 spacing，diff 范围 = 插入点之后该层的全部行 —— 这是位置编码的固有代价，
本模块消不掉。
"""
from hanzi_freq import FMAX


def section_freqs(
    n_singles: int, n_phrases: int, capacity: int, phrase_base: int
) -> tuple[list[int], list[int]]:
    """单字层（基址 0）+ 词组层（基址 phrase_base）的 freq 列，顺序原样保留。

    越界即 `RuntimeError`（不是截断）：spacing 归零会让 freq 并列/下溢。容量/基址
    都是**显式上调**项 —— 上调会产生一次性全表 freq 变化，需要重编产物与跑排序门禁。
    """
    if n_singles > phrase_base:
        raise RuntimeError(
            f"单字层 {n_singles} 条超过词组层固定基址 {phrase_base}：两层会交叠，"
            "请上调 PHRASE_BASE 与容量后重跑。"
        )
    if phrase_base + n_phrases >= capacity:
        raise RuntimeError(
            f"词组层越界：基址 {phrase_base} + {n_phrases} 条 ≥ 固定容量 {capacity}，"
            "spacing 会归零/下溢。请上调 MAX_ENTRIES（会一次性重算全表 freq）后重跑。"
        )
    spacing = FMAX // capacity
    single = [FMAX - i * spacing for i in range(n_singles)]
    phrase = [FMAX - (phrase_base + i) * spacing for i in range(n_phrases)]
    return single, phrase
