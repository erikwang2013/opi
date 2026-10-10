#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""外部开源词库 → data/raw/extra_phrases.tsv（luna.opid 的追加词组层，2026-10-11）。

用户裁决（2026-10-11）：把主流开源输入法词库**去重整合**进 luna.opid（补 词组 /
品牌 / 成语 / 歇后语 / 网络用语 / 专业术语 / 新词），接受 GPL-3.0 数据 —— 合并产物
luna.opid 按 GPL-3.0 分发，源码仍 MIT。逐源声明与许可义务见 data/raw/LICENSES.md。

## 上游（全部 pin commit + sha256 + 行数，三者不符即 FATAL —— 绝不静默换数据）

    base   rime-ice cn_dicts/base       GPL-3.0-only 543118 拼音✓真词频✓
    ext    rime-ice cn_dicts/ext        GPL-3.0-only 339186 拼音✓权重全100
    essay  rime-essay essay.txt         LGPL-3.0     442688 无拼音（按字注音）
    idiom  chinese-xinhua idiom.json    MIT           30895 拼音✓（带调）
    xie    chinese-xinhua xiehouyu.json MIT           14032 无拼音（按字注音）
    THUOCL IT/医疗/法律/财经             MIT           见常量 无拼音（按字注音）
    OpenCC TSCharacters（判繁，非词源）  Apache-2.0    4113   字表
    品牌/新词/覆盖表 = data/raw/ 三张人工表（MIT，本项目自撰）

不收（逐条取证见 LICENSES.md）：rime-ice 的 tencent/8105/41448/others、jieba（来源
不明）、zhwiki 系（BY-SA+GFDL）、libpinyin、SunPinyin、白霜、万象（上游无从署名）、
phrase-pinyin-data（汉典抓取 + CEDICT）。

## 关键设计（改之前先读）

- **层序 = 优先级**：品牌 → 新词 → base → ext → essay → 成语 → 术语 → 歇后语；整段
  由 gen_luna_dict 追加在**既有单字层与既有 simp 词组之后** ⇒ 既有排序门禁（13 个
  首位钉 / ranking_quality 三档 / m2 硬位置）零位移。
- **层内排序**（只影响同拼音键内先后）：base = 权重>1 先、再 -权重,len,word（权重=1
  的 6.2 万条沉底）；ext = len,word；essay = freq>0 先、再 -freq,len,word；成语/歇后语
  = len,word；术语 = -DF,word；人工表 = 文件序。**不做跨源词频校准**：候选竞争只在
  同一拼音键内发生，不同量纲跨层无害。
- **注音（无拼音源）**：词级人工覆盖 → kHanyuPinlu 词次 argmax → kMandarin 首读；
  内建自测拿 base/ext 真拼音剥掉重注比对（下限 ACCURACY_MIN）+ 打印错字榜。
- **繁简：丢弃不转换**（繁→简非单射是先例）；判据 = OpenCC TSCharacters（剔除自靶行）
  ∪ 旧 Unihan 判据，见 `is_trad` —— 单用 Unihan 会漏 線/喫/遊 一整类（实测 essay 层
  7,644 条）。**去重两段**：① 被注音层的词若已在有拼音层出现（word 层面）⇒ 整条抑制
  （猜的键只会多一份可能错的）；② 其余按 (pinyin, word) 层序优先去重；同词不同拼音都保留。
- **音节校验读单源**：解析 pinyin.rs 的 SYLLABLES（断言 410）做全切分，不复制音节表
  （跨语言镜像常量会静默漂，本仓已知反模式）。word 允许非 CJK 标点（`·`/`-`/`——`），
  拼音键只由 CJK 字生成；字面过滤 = 无 ASCII/空白 + CJK 字数 ≥2。

## 用法（在仓库根运行；需要网络）：python3 scripts/gen_extra_phrases.py

产物两份（均 `word\tpinyin\torder`，order 严格递减 = 文件序）：
- `data/raw/extra_manual.tsv` —— 品牌 + 新词两张人工表（千条量级：609 + 391）；
- `data/raw/extra_phrases.tsv` —— 其余批量层。
gen_luna_dict 把它们分别排在既有 simp 词组**之前**与**之后**（三段序：
人工 → simp → 批量）—— 人工表前置是为了品牌词在同键上压过既有常用词
（实测：`威信` 在 simp 层权重 3.71e9，微信在批量段 3.61e9 会被压到第 3）。
计数以本脚本每次打印的 stderr 报告为准，别把数字抄进文档。
"""
import hashlib
import random
import re
import sys
import urllib.request
from collections import Counter
from functools import partial
from pathlib import Path

from extra_sources import (parse_essay, parse_ice, parse_idiom, parse_manual,
                           parse_thuocl, parse_ts, parse_xie)
from gen_trad_dict import write_atomic
from hanzi_freq import (gb2312_levels, is_cjk, load_khanyupinlu, load_kmandarin,
                        load_variants, normalize, trad_only, unihan_member)

ROOT = Path(__file__).resolve().parent.parent
EXTRA_TSV = ROOT / "data" / "raw" / "extra_phrases.tsv"
EXTRA_MANUAL_TSV = ROOT / "data" / "raw" / "extra_manual.tsv"
BRAND_TSV = ROOT / "data" / "raw" / "brand_terms.tsv"
NEWWORDS_TSV = ROOT / "data" / "raw" / "new_words.tsv"
OVERRIDES_TSV = ROOT / "data" / "raw" / "reading_overrides.tsv"
PINYIN_RS = ROOT / "crates" / "engine-core" / "src" / "pinyin.rs"

# ---- 上游 pin（2026-10-11 实测；升级 = 换 commit+sha256+行数三件套并重跑门禁）----
ICE = "da1fbe602e38f26db846fa10120ee64c2b0324c0"
ESSAY_C = "054920de4f54c9e5994276a96a4fc2a35cb51aa3"
XINHUA = "fe6d6c2e8baa82187f4c96bbe042e43f96c05666"
THU_C = "a30ce79d895d01ab5132a5c74c29703ff7efb4cc"
_ice = "https://raw.githubusercontent.com/iDvel/rime-ice/" + ICE + "/cn_dicts/%s"
_xh = "https://raw.githubusercontent.com/pwxcoo/chinese-xinhua/" + XINHUA + "/data/%s"
_thu = "https://raw.githubusercontent.com/thunlp/THUOCL/" + THU_C + "/data/%s"

BASE = (_ice % "base.dict.yaml", "0418d5103d8e40c8fff4b686a873ad03e9836cd0419ecae0b7e1e2526312dba0", 543118)
EXT = (_ice % "ext.dict.yaml", "8e49a0294c0139e63c815cc33a2d438cc144eb7889297d30cec899a368697c06", 339186)
ESSAY = ("https://raw.githubusercontent.com/rime/rime-essay/" + ESSAY_C + "/essay.txt", "3e8512ebaa6657e35961b1f5762060717c647e3bf471d5e3b86e783b4e934ffb", 442688)
IDIOM = (_xh % "idiom.json", "1d4b4f454ce1c416d6a1ab2369d6e66c0ff99e04390172eef70790499e21ce19", 30895)
XIE = (_xh % "xiehouyu.json", "22e3c4aa31c7d93765eaa842fe91983e6a109b29aec25956e1efacd9f71418d9", 14032)
THU = {
    "IT": (_thu % "THUOCL_IT.txt", "e8cd42c9f5559735fa3bfb91515cbe051f991efd4a803038a1d5c0da85af3526", 16000),
    "医疗": (_thu % "THUOCL_medical.txt", "a01f39f1677af0b6a620bbc68bf293e8e436f06248b74adb625cd61d9ad7bbcd", 18749),
    "法律": (_thu % "THUOCL_law.txt", "8ef1a5f41052ac95dedde88b167c5031a43b3e0211bf128b6125736cc76ef7b2", 9896),
    "财经": (_thu % "THUOCL_caijing.txt", "68e63d2009e59c6f388eeaa789d19b6a681657de3e8e56cc3479b21cd8548542", 3830),
}
# OpenCC TSCharacters（繁→简字表）——判繁的第二判据。为什么要它：Unihan 的
# kSimplifiedVariant 不是「繁→简」表（線→[缐]、喫/遊/錄 干脆没有），essay 层实测
# 7,644 条繁体词形从旧判据下漏过（字频榜前 30 全是 線/喫/遊/羣…）。Apache-2.0，
# pin 到 release tag ver.1.1.9（解析见 extra_sources.parse_ts，自靶行由调用方排除）。
OPENCC = ("https://raw.githubusercontent.com/BYVoid/OpenCC/ver.1.1.9/data/dictionary/TSCharacters.txt",
          "6b5a0a799bea2bb22c001f635eaa3fc2904310f0c08addbff275477a80ecf09a", 4113)

# 人工表行数（改表必须同改常量；0 = FATAL，防「忘了填」静默跳过）
BRAND_ROWS, NEW_WORDS_ROWS, OVERRIDES_ROWS = 609, 391, 422

BULK_MAX_LEN = 6
IDIOM_MAX_LEN = 8
MANUAL_MAX_LEN = 12
XIE_MAX_LEN = 24
ACCURACY_MIN = 0.90
SAMPLE_N = 5000


def fetch(url: str, sha256: str) -> str:
    """下载并校验 pin 的摘要，返回 UTF-8 文本；不符即 FATAL（同 gen_luna_dict.fetch）。"""
    with urllib.request.urlopen(url, timeout=180) as r:
        data = r.read()
    got = hashlib.sha256(data).hexdigest()
    if got != sha256:
        raise RuntimeError("%s 摘要与 pin 不符：期望 %s，实得 %s。"
                           "上游变了，确认后更新本脚本 pin 并重跑门禁。" % (url, sha256, got))
    return data.decode("utf-8")


def expect(rows: list, n: int, name: str) -> None:
    if len(rows) != n:
        raise RuntimeError("%s 行数 %d != pin %d（上游漂移？）" % (name, len(rows), n))


# ---- 音节校验（单源 = pinyin.rs）----

def load_syllables() -> set:
    m = re.search(r"pub const SYLLABLES[^=]*= &\[(.*?)\];",
                  PINYIN_RS.read_text(encoding="utf-8"), re.S)
    if not m:
        raise RuntimeError("解析 %s 的 SYLLABLES 失败（声明形状变了？）" % PINYIN_RS)
    syls = re.findall(r'"([a-z]+)"', m.group(1))
    if len(syls) != 410:
        raise RuntimeError("SYLLABLES 抽出 %d 条 != 410 —— 先核对再放行" % len(syls))
    return set(syls)


def key_ok(key: str, syls: set) -> bool:
    """全切分：key 能否由 SYLLABLES 的音节恰好拼成（DP，窗口 6 同引擎 segment）。"""
    n = len(key)
    dp = [False] * (n + 1)
    dp[0] = True
    for i in range(n):
        if not dp[i]:
            continue
        for ln in range(1, min(6, n - i) + 1):
            if key[i : i + ln] in syls:
                dp[i + ln] = True
    return dp[n]


def cjk_n(word: str) -> int:
    return sum(1 for c in word if is_cjk(c))


def chars_ok(word: str) -> bool:
    """字面过滤：无 ASCII/空白，CJK 字数 ≥2（非 CJK 标点放行）。"""
    if any(c.isascii() or c.isspace() for c in word):
        return False
    return cjk_n(word) >= 2


# ---- 各源解析：已拆到 extra_sources.py（本文件贴 500 行门禁；理由见该文件头）----


# ---- 注音（无拼音源）----

def make_annotator(kph: dict, km: dict, syls: set, overrides: dict):
    """→ note(word) -> (key, conf)；key=None 时 conf ∈ {miss, sylbad}。
    非 CJK 字符不出音节；conf：override > kph（词次 argmax）> kmandarin（首读）。"""

    def readings(word: str):
        parts, low = [], False
        for c in word:
            if not is_cjk(c):
                continue
            r = kph.get(c)
            if r:
                parts.append(max(r, key=r.get))
            elif c in km:
                parts.append(km[c][0])
                low = True
            else:
                return None
        return "".join(normalize(p) for p in parts), ("kmandarin" if low else "kph")

    def note(word: str):
        if word in overrides:
            return overrides[word], "override"
        got = readings(word)
        if got is None:
            return None, "miss"
        key, conf = got
        if not key_ok(key, syls):
            return None, "sylbad"
        return key, conf

    return note, readings


def self_test(note, readings, probe: list, rng: random.Random) -> None:
    """拿 base/ext 的真拼音当标准答案：剥掉重注逐词比对；打印错字榜。"""
    sample = rng.sample(probe, min(SAMPLE_N, len(probe)))
    ok = 0
    char_err: Counter = Counter()
    examples: dict = {}
    for word, toks, _wt in sample:
        ours_true = "".join(normalize(t) for t in toks)
        our, _conf = note(word)
        if our == ours_true:
            ok += 1
            continue
        got = readings(word)
        if got is None:
            continue
        ours, _ = got
        if len(ours) == len(ours_true):
            cjk = [c for c in word if is_cjk(c)]
            i = 0
            for ch, tok in zip(cjk, toks):
                ln = len(normalize(tok))
                if ours[i : i + ln] != normalize(tok):
                    char_err[ch] += 1
                    examples.setdefault(ch, word)
                i += ln
    acc = ok / len(sample)
    print("注音自测（vs base/ext 真拼音，n=%d）：逐词一致 %.1f%%（下限 %.0f%%）"
          % (len(sample), 100 * acc, 100 * ACCURACY_MIN), file=sys.stderr)
    if acc < ACCURACY_MIN:
        raise RuntimeError("注音准确率 %.3f < 下限 %.2f —— 先补错字榜/覆盖表"
                           % (acc, ACCURACY_MIN))
    if char_err:
        print("错字榜（供补 reading_overrides.tsv）：" + "、".join(
            "%s(%d，如%s)" % (c, n, examples[c]) for c, n in char_err.most_common(20)),
            file=sys.stderr)


def main() -> None:
    syls = load_syllables()
    first, second = gb2312_levels()
    if len(first) != 3755 or len(second) != 3008:
        print("FATAL: GB2312 段位不符（%d/%d）" % (len(first), len(second)), file=sys.stderr)
        sys.exit(1)
    kph = load_khanyupinlu(unihan_member("Unihan_Readings.txt"))
    km = load_kmandarin(unihan_member("Unihan_Readings.txt"))
    _trad_of, simp_of = load_variants(unihan_member("Unihan_Variants.txt"))
    gb_set = set(first) | set(second)
    to = partial(trad_only, gb=gb_set, simp_of=simp_of)
    ts = parse_ts(fetch(OPENCC[0], OPENCC[1]))
    if len(ts) != OPENCC[2]:
        raise RuntimeError("TSCharacters 解析出 %d 条 != pin %d（上游变体？）" % (len(ts), OPENCC[2]))

    def is_trad(ch: str) -> bool:
        """繁形判据（2026-10-11 升级，两个信号取并、共用一道 GB2312 门）：
        **GB2312 成员 = 简体字形**（既有权威判据，`gen_luna_dict` 头注释写明）——
        後/於/吒 都在 GB2312 里，故 然後/哪吒 不被下面的 OpenCC 信号误杀
        （OpenCC 的 `吒→咤` 没有自靶，无门会整词杀掉 哪吒）。门外的字：
        ① OpenCC TSCharacters 命中且**自靶不在目标里**（`乾→干 乾` 这类另有简体
           语境用法，不判繁 —— 乾隆/覆盖 不受影响）；② 旧 Unihan 判据（它漏
           線/喫/遊 这一大类 —— 那正是 OpenCC 信号的来由）。"""
        if ch in gb_set:
            return False
        targets = ts.get(ch)
        if targets is not None and ch not in targets:
            return True
        return to(ch)

    # ---- 人工表（含覆盖表自检）----
    overrides = {}
    truth: dict = {}
    brand_rows = parse_manual(BRAND_TSV, BRAND_ROWS)
    newword_rows = parse_manual(NEWWORDS_TSV, NEW_WORDS_ROWS)
    for w, k in parse_manual(OVERRIDES_TSV, OVERRIDES_ROWS):
        if not chars_ok(w) or not key_ok(k, syls):
            raise RuntimeError("覆盖表条目不合法：%r -> %r" % (w, k))
        if w in overrides:
            raise RuntimeError("覆盖表重复词：%r" % w)
        overrides[w] = k
    note, readings = make_annotator(kph, km, syls, overrides)

    # ---- 下载 + 解析（行数核对）----
    base_rows = parse_ice(fetch(BASE[0], BASE[1]))
    ext_rows = parse_ice(fetch(EXT[0], EXT[1]))
    expect(base_rows, BASE[2], "base")
    expect(ext_rows, EXT[2], "ext")
    essay_rows = parse_essay(fetch(ESSAY[0], ESSAY[1]))
    expect(essay_rows, ESSAY[2], "essay")
    idiom_rows = parse_idiom(fetch(IDIOM[0], IDIOM[1]))
    expect(idiom_rows, IDIOM[2], "idiom")
    xie_rows = parse_xie(fetch(XIE[0], XIE[1]))
    expect(xie_rows, XIE[2], "xie")
    thu_rows = {}
    for name, spec in THU.items():
        thu_rows[name] = parse_thuocl(fetch(spec[0], spec[1]))
        expect(thu_rows[name], spec[2], "THUOCL-" + name)

    # ---- 覆盖表对真值自检（词在 base/ext 出现时，覆盖读音必须是其中之一）----
    for w, toks, _wt in base_rows + ext_rows:
        if len(toks) == cjk_n(w):
            truth.setdefault(w, set()).add("".join(normalize(t) for t in toks))
    for w, k in overrides.items():
        if w in truth and k not in truth[w]:
            raise RuntimeError("覆盖表 %r -> %r 与上游真读音 %s 全部不符 —— 复核"
                               % (w, k, sorted(truth[w])))

    # ---- 注音自测 ----
    probe = [(w, t, wt) for (w, t, wt) in base_rows + ext_rows
             if 2 <= len(w) <= BULK_MAX_LEN and chars_ok(w) and len(t) == cjk_n(w)]
    self_test(note, readings, probe, random.Random(20261011))

    # ---- 逐层准备（基础过滤 + 层内排序；键在 emit 段定）----
    def base_filters(word: str, max_len: int, c: Counter) -> bool:
        if len(word) > max_len:
            c["len"] += 1
            return False
        if not chars_ok(word):
            c["chars"] += 1
            return False
        if any(is_trad(ch) for ch in word):
            c["trad"] += 1
            return False
        return True

    def prep_ice(rows, c):
        out = []
        for word, toks, wt in rows:
            c["raw"] += 1
            if not base_filters(word, BULK_MAX_LEN, c):
                continue
            if len(toks) != cjk_n(word):
                c["align"] += 1
                continue
            key = "".join(normalize(t) for t in toks)
            if not key_ok(key, syls):
                c["sylbad"] += 1
                continue
            out.append((word, key, wt))
        # 权重=1 沉底桶；层内序 = (-权重, len, word)
        out.sort(key=lambda e: (e[2] == 1, -e[2], len(e[0]), e[0]))
        return out

    def prep_annot(words, max_len, c):
        out = []
        for word in words:
            c["raw"] += 1
            if base_filters(word, max_len, c):
                out.append(word)
        return out

    def prep_thu(rows, c):
        out = []
        for word, df in rows:
            c["raw"] += 1
            if base_filters(word, BULK_MAX_LEN, c):
                out.append((word, df))
        out.sort(key=lambda e: (-e[1], len(e[0]), e[0]))
        return out

    def prep_idiom(rows, c):
        out = []
        for word, toks, abbr in rows:
            c["raw"] += 1
            if not base_filters(word, IDIOM_MAX_LEN, c):
                continue
            if len(toks) != cjk_n(word):
                c["align"] += 1
                continue
            key = "".join(normalize(t) for t in toks)
            if not key_ok(key, syls):
                c["sylbad"] += 1
                continue
            if abbr and abbr != "".join(t[0] for t in toks):
                c["abbr_mismatch"] += 1
            out.append((word, key))
        out.sort(key=lambda e: (len(e[0]), e[0]))
        return out

    def prep_manual(rows, c):
        out = []
        for word, key in rows:
            c["raw"] += 1
            if not base_filters(word, MANUAL_MAX_LEN, c):
                continue
            if not key_ok(key, syls):
                raise RuntimeError("人工表条目音节非法：%r -> %r" % (word, key))
            out.append((word, key))
        return out

    cnt: dict = {}
    lanes = []  # (名称, kind, items)
    cnt["品牌"] = Counter()
    lanes.append(("品牌", "manual", prep_manual(brand_rows, cnt["品牌"])))
    cnt["新词"] = Counter()
    lanes.append(("新词", "manual", prep_manual(newword_rows, cnt["新词"])))
    cnt["base"] = Counter()
    lanes.append(("base", "ice", prep_ice(base_rows, cnt["base"])))
    cnt["ext"] = Counter()
    lanes.append(("ext", "ice", prep_ice(ext_rows, cnt["ext"])))
    cnt["essay"] = Counter()
    lanes.append(("essay", "annot", prep_annot([w for w, _ in essay_rows], BULK_MAX_LEN,
                                               cnt["essay"])))
    cnt["成语"] = Counter()
    lanes.append(("成语", "ice", prep_idiom(idiom_rows, cnt["成语"])))
    for name in THU:
        cnt[name] = Counter()
        lanes.append(("术语-" + name, "thu", prep_thu(thu_rows[name], cnt[name])))
    cnt["歇后语"] = Counter()
    lanes.append(("歇后语", "annot", prep_annot(xie_rows, XIE_MAX_LEN, cnt["歇后语"])))

    # 有拼音层（manual/ice）：其 word 集合 → 抑制被注音层（annot/thu）的同词条目
    primary = set()
    for _name, kind, items in lanes:
        if kind in ("manual", "ice"):
            primary.update(e[0] for e in items)

    # ---- emit：抑制 → 注音 → (key,word) 去重（人工段与批量段分别落盘）----
    seen = set()
    final_manual: list = []
    final_bulk: list = []
    conf: Counter = Counter()
    for name, kind, items in lanes:
        c = cnt.get(name)
        kept = 0
        for e in items:
            if kind == "manual":
                word, key = e  # 音节合法性已在 prep_manual 校验
            elif kind == "ice":
                word, key = e[0], e[1]
            else:  # annot / thu：无拼音源 → 先抑制、再注音
                word = e if kind == "annot" else e[0]
                if word in primary:
                    conf["suppressed"] += 1
                    continue
                key, kconf = note(word)
                if key is None:
                    conf[kconf] += 1
                    continue
                conf[kconf] += 1
            if (key, word) in seen:
                if c is not None:
                    c["dup"] += 1
                continue
            seen.add((key, word))
            (final_manual if kind == "manual" else final_bulk).append((word, key))
            kept += 1
        if c is None:
            cnt[name] = Counter()
            c = cnt[name]
        c["kept"] = kept
        print("层 %-10s %s → 最终 %d" % (name, dict(c), kept), file=sys.stderr)

    # ---- 出口断言 + 写出（每份文件各自验 order 与繁形残留）----
    def emit(path: Path, rows: list) -> None:
        total = len(rows)
        lines = ["%s\t%s\t%d" % (w, k, total - i) for i, (w, k) in enumerate(rows)]
        for i in range(1, len(lines)):
            prev = int(lines[i - 1].rsplit("\t", 1)[1])
            cur = int(lines[i].rsplit("\t", 1)[1])
            if cur >= prev:
                raise RuntimeError("%s order 列非严格递减：第 %d 行" % (path.name, i))
        residual = sum(1 for w, _ in rows if any(is_trad(c) for c in w))
        if residual:
            raise RuntimeError("%s trad 字形残留 %d 条（简体库不得混繁体）" % (path.name, residual))
        write_atomic(str(path), lines)
        print("总条目 %d → %s" % (total, path), file=sys.stderr)

    emit(EXTRA_MANUAL_TSV, final_manual)
    emit(EXTRA_TSV, final_bulk)
    print("注音计数 %s；覆盖表 %d 条" % (dict(conf), len(overrides)), file=sys.stderr)


if __name__ == "__main__":
    main()
