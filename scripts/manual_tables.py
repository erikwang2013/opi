#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""项目自撰的三列人工表（百家姓 / 网络流行语）→ gen_extra_phrases 的批量段尾。

两张表都是 `word\\tpinyin\\tfreq`，与 extra_manual/extra_phrases 同形。**拼音全部人工
标注**（不是按字注音）：多音字按姓氏读音或流行语实际读法写，生成器逐条用 pinyin.rs 的
SYLLABLES 做全切分校验，非法即 FATAL。freq 列只是层内档位（同键先后），最终频率仍由
gen_luna_dict 按整段行位置编码，不读这里的数值。

放置：两表都接在 extra_phrases.tsv（批量段）**队尾**，不是 extra_manual.tsv。理由：
extra_manual 段整段位置高于 simp 常用词，而本任务要求流行语「频次低于同长常用词」。
实测撞键：dazi 有 大字/打字、eren 有 恶人、shangtou 有 伤透/上头、youhua 有 有话/
油画/优化 —— 若进人工段会把常用词顶下去。放批量段尾还避免把几百个单姓键塞进
gen_luna_dict 的 top_keys（那会把 微信 等品牌词误降级）。

ASCII 字面键例外：yyds / 栓Q / City不City / i人 / 打call / 嗑CP 的键不是合法拼音
音节串，但引擎对全文精确匹配是字面比较（`yyds` 可直接命中）。仅当键为纯小写 ASCII
字母且词形含 ASCII 字母时放行；其余条目必须通过 SYLLABLES 全切分。
"""
from pathlib import Path
from typing import Callable

from hanzi_freq import is_cjk

ROOT = Path(__file__).resolve().parent.parent
SURNAMES_TSV = ROOT / "data" / "raw" / "surnames.tsv"
SLANG_TSV = ROOT / "data" / "raw" / "slang.tsv"

# 行数 pin（改表必须同改；0 或缺失 = FATAL，防「忘了填」静默跳过）
SURNAMES_ROWS = 508
SLANG_ROWS = 597
# 用户裁决的频次区间（只作档位校验；真实频率由行位置编码）
SURNAME_FREQ = (8000, 20000)
SLANG_FREQ = (2000, 8000)


def parse_manual3(path: Path, expect_n: int, freq_range: tuple[int, int]):
    """读三列表并校验形状；缺文件/列数/拼音/频次/重复 (词,拼音) 一律 FATAL。"""
    if not path.is_file():
        raise RuntimeError("缺少人工表 %s —— 先创建再跑本生成器" % path)
    lo, hi = freq_range
    rows: list[tuple[str, str, int]] = []
    seen: set[tuple[str, str]] = set()
    for i, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        cols = line.split("\t")
        if len(cols) != 3:
            raise RuntimeError("%s:%d 列数 %d != 3（期望 word\\tpinyin\\tfreq）"
                               % (path.name, i, len(cols)))
        word, key, freq_s = cols
        if not word or not key:
            raise RuntimeError("%s:%d 空词形或空拼音" % (path.name, i))
        if not key.isascii() or not key.isalpha() or not key.islower():
            raise RuntimeError("%s:%d 拼音必须是纯小写 ASCII 字母：%r"
                               % (path.name, i, key))
        if any(c.isspace() for c in word):
            raise RuntimeError("%s:%d 词形含空白：%r" % (path.name, i, word))
        try:
            freq = int(freq_s)
        except ValueError:
            raise RuntimeError("%s:%d 频次不是整数：%r" % (path.name, i, freq_s))
        if not lo <= freq <= hi:
            raise RuntimeError("%s:%d 频次 %d 超出约定区间 [%d, %d]"
                               % (path.name, i, freq, lo, hi))
        if (word, key) in seen:
            raise RuntimeError("%s:%d 重复 (词,拼音)：%r -> %r"
                               % (path.name, i, word, key))
        seen.add((word, key))
        rows.append((word, key, freq))
    if len(rows) != expect_n:
        raise RuntimeError("%s 行数 %d != pin %d（表被改过？同改本文件常量）"
                           % (path.name, len(rows), expect_n))
    return rows


def parse_surnames():
    return parse_manual3(SURNAMES_TSV, SURNAMES_ROWS, SURNAME_FREQ)


def parse_slang():
    return parse_manual3(SLANG_TSV, SLANG_ROWS, SLANG_FREQ)


def prep_curated(rows, max_len: int, syls: set, is_trad: Callable[[str], bool],
                 allow_ascii: bool, key_ok: Callable[[str, set], bool],
                 c) -> list[tuple[str, str, int]]:
    """校验并排序：词形只允许 CJK/ASCII 字母，读音走全切分或 ASCII 字面例外。"""
    out = []
    for word, key, freq in rows:
        c["raw"] += 1
        if len(word) > max_len:
            raise RuntimeError("人工表词长 %d > %d：%r" % (len(word), max_len, word))
        if not all(is_cjk(ch) or (ch.isascii() and ch.isalpha()) for ch in word):
            raise RuntimeError("人工表词形含不支持字符：%r" % word)
        if any(is_trad(ch) for ch in word):
            raise RuntimeError("人工表混入繁体字形：%r" % word)
        if not key_ok(key, syls):
            literal = (allow_ascii and key.isascii() and key.isalpha()
                       and key.islower()
                       and any(ch.isascii() and ch.isalpha() for ch in word))
            if not literal:
                raise RuntimeError("人工表读音不合法（不在 SYLLABLES 全切分内）："
                                   "%r -> %r" % (word, key))
            c["ascii_literal"] += 1
        out.append((word, key, freq))
    # 同键先后：高频档在前；同档按长度/词形定序（跨键序无产品影响）
    out.sort(key=lambda e: (-e[2], len(e[0]), e[0]))
    c["kept"] = len(out)
    return out
