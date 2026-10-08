#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""生成生僻字键盘的**部首/笔画索引表**（Unihan kRSUnicode + kTotalStrokes）。

产物：`android/app/src/main/assets/rare_index.tsv`（**入库**，Kotlin 侧解析）。
格式：`字<TAB>部首号(1..214)<TAB>总笔画数`，按码点升序 —— 面板流程
「选部首 → 选笔画数 → 列字」就是这张表上的两级筛选。

数据源与授权：Unicode Unihan（`Unihan_IRGSources.txt` 的 kRSUnicode /
kTotalStrokes；Unicode License，与 trad_hanzi.tsv 同源）。下载与 SHA-256 pin
复用 `scripts/hanzi_freq.py` 的 `unihan_member`（那里是 Unihan 的唯一入口）。

用法（需要网络）：`cd <repo 根> && python3 scripts/gen_rare_index.py`

为什么不用 stroke.opid 代替：stroke.opid 是**笔画序**（能按开头笔画查），本表是
**部首+笔画数**（Sogou 式生僻字键盘的浏览式查法）；两者互补，不是一份数据的两种
存法。两张表都覆盖扩展区（stroke 至 Ext J；本表随 Unihan 收录范围）。
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from hanzi_freq import unihan_member  # noqa: E402  （Unihan 唯一下载口，自带 sha256 pin）

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "android/app/src/main/assets/rare_index.tsv"

# Kanxi 部首数（kRSUnicode 的部首号范围）。
MAX_RADICAL = 214


def parse_irg(text: str):
    """→ {char: (radical, total_strokes)}，缺任一字段的字不收录。"""
    radical = {}
    strokes = {}
    for line in text.splitlines():
        if line.startswith("#") or "\t" not in line:
            continue
        parts = line.split("\t")
        if len(parts) < 3 or not parts[0].startswith("U+"):
            continue
        ch = chr(int(parts[0][2:], 16))
        field, value = parts[1], parts[2].strip()
        if field == "kRSUnicode":
            # 形态：`1.0` / `123.5`；撇号（' / ''）表示简体形态的部首，取前半段即可。
            num = value.split("'")[0]
            head = num.split(".")[0]
            if head.isdigit() and 1 <= int(head) <= MAX_RADICAL:
                radical[ch] = int(head)
        elif field == "kTotalStrokes":
            # 形如 `5` 或 `5 6`（简繁两形）——取第一个（该字自己的）。
            first = value.split()[0] if value.split() else ""
            if first.isdigit():
                strokes[ch] = int(first)
    return {ch: (radical[ch], strokes[ch]) for ch in radical if ch in strokes}


def main() -> int:
    table = parse_irg(unihan_member("Unihan_IRGSources.txt"))
    if len(table) < 20_000:
        # 覆盖量骤降 = 上游格式变了（列序/字段名），别写出一个残表还 exit 0 ——
        # 与 opi-tools 对「空壳词库」同一条原则。
        print(f"!! 只解析出 {len(table)} 个字，疑似 Unihan 格式变动，拒绝写出", file=sys.stderr)
        return 1
    rows = [f"{ch}\t{r}\t{s}" for ch, (r, s) in sorted(table.items())]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(rows) + "\n", encoding="utf-8")
    print(f"产物: {OUT.relative_to(REPO)}：{len(rows)} 字（{OUT.stat().st_size} 字节，已入库）")
    # 抽查：常用字的部首/笔画（人=9? 不 —— 人 部首 9、2 画；口 30、3 画）
    for probe, want in (("人", (9, 2)), ("口", (30, 3)), ("好", (38, 6))):
        got = table.get(probe)
        if got != want:
            print(f"!! 抽查不符：{probe} 期望 {want} 实得 {got}", file=sys.stderr)
            return 1
    print("抽查通过：人(9,2) 口(30,3) 好(38,6)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
