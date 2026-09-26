#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""luna 拼音词库排序（真实字频，修复 rime 权重缺陷 + 布局序缺陷）。

背景（spec 2026-08-15 验收偏差 #4）：
1. luna_pinyin.dict.yaml 第 3 列是读音概率份额（多读音字按 P(读音|字) 分派），非词频；
2. 旧脚本因此拿 GB2312 一二级码序当常用度替身——但码序是汉字**编码顺序**，同音字
   谁在前纯属历史巧合：wo→蜗、ni→呢、hao→镐、de→澄、shi→匙、fa→樊（26/30 首位错）。
现改用 Unihan kHanyuPinlu（逐读音语料词次，见 scripts/hanzi_freq.py）排序。

排序键（同拼音前缀组内，依次比较）：
  有词次 > 无词次 → 词次降序 → 简体专形优先 → GB 段位（一级/二级/其余）→ 段内序 → 字码位。
GB 段位**保留**（同频/缺频字的兜底序：常用字仍在生僻字前），只是不再当主键；
freq = FMAX - idx × (FMAX // N) 的间距编码不变，词组仍排在全部单字之后。

kHanyuPinlu 只覆盖 3799 字，其余字按「无词次」落入段位序（旧行为），不会被丢掉。
「有繁体专形优先」的由来：Unihan 变体镜像字（如 戠 经 kSimplifiedVariant 继承 只 的
词次）在同音同频时会把真简形 只 顶下去，简体库取有繁体专形的字形可消解。
读音保留规则（2026-09-26 修）：**有 kHanyuPinlu 词次证据的读音一律保留**，0% 过滤器
只对无证据读音生效——上游第三列的 0% 是「该读音份额为 0」，**不等于该读音不存在**：
开 kai 0%（jian 95%）、备 bei 0%（yuan 100%）等 38 字的常用读音被标 0%，旧规则整条
丢弃后这些字只能由罕用读音打出（开→jian，见 ranking_quality.rs REACH 档）。
反过来「上游标 0% 且无证据」仍旧丢弃，如 冰 bing 100%/ning 0%（kHanyuPinlu 冰 只有
bing 167）——留着会把 冰 泄漏进 ni 查询（ni 是 ning 的前缀）。
旧脚本的「次读音搬组尾 + 锚点表 + 读音概率表」整段退役——真实词次本身
就表达了「都 dou 常用、du 少用」，不再需要按段位猜。

收录范围（2026-09-26 放开扩展区）：
  原 `0x4E00..=0x9FFF` 过滤把 luna 表里的扩展 A/B 字整段丢弃（上游表里其实有：
  2 万多条词条含 BMP 外字符）。现按 hanzi_freq.CJK_RANGES 收录到扩展 B 为止。
  代价（本次实测）：单字 20902 → 41289（+20387）、词组 21719 → 21759、
  总行数 47422 → 70014，luna.opid 1.19MB → 1.71MB（+38%）；
  `opi-tools verify` 装载 10.9ms → 15.6ms（新旧交错各 9 次取最小；耗时与条目数成正比）。
  扩展区字无 kHanyuPinlu 词次 → 落「无词次」段
  （段位 2、码位序），仍排在常用字之后，既有候选顺序不变。
  注音符号 ㄓㄔㄕ（U+3113+）与 〇（U+3007）**不在 CJK_RANGES 内，仍旧不收** ——
  本次只放开 CJK 扩展区，不顺带改收录范围；它们此前也被同一条过滤丢掉，行为不变。
  **局限：收录 ≠ 可见。** 扩展 B 依赖设备字体，各厂商覆盖差异很大（常见 tofu 空白框）。

注意 luna_pinyin.dict.yaml **没有 pin**（gen_trad_dict.py 的 terra 有）：上游取
rime/rime-luna-pinyin master。本次用的版本 sha256 =
75bcf6eb3ff62b129882ed89cc22b2d80a5347aa72bcfa2ccc839bac298e7314（889896 字节，
70771 行，与 LICENSES.md 记录的「889KB ~70771 行」一致），用它跑本脚本产出的
luna.opid 与入库副本 sha256 逐字节相同 —— 即当前产物确实来自这个版本。
**升级应照 terra 的做法补 pin**（未做：不在本次任务范围，且当前 master 恰好可复现）。

用法（需要网络，下载 Unihan.zip）：
  python3 scripts/gen_luna_dict.py <luna_pinyin.dict.yaml> > /tmp/luna_merged.tsv
产物不入库（luna.opid 本身 gitignore）；部署副本 android/app/src/main/assets/luna.opid。
"""
import sys

from hanzi_freq import (
    FMAX,
    gb2312_levels,
    is_cjk,
    load_khanyupinlu,
    load_variants,
    segpos,
    unihan_member,
)


def parse_rime(path: str) -> tuple[dict[str, list[tuple[str, int | None]]], list[tuple[str, str, int]]]:
    """luna_pinyin.dict.yaml → (单字读音表, 词组)。

    单字读音记 (pinyin, weight)：无第 3 列 → weight=None（默认读音，保留）；
    显式 0% → 该读音"从不使用"（如 冰 bing 100%/ning 0%），调用方剔除，
    避免低质量读音以单字段高词频泄漏进错误前缀（ni 查询出 冰）。"""
    singles: dict[str, list[tuple[str, int | None]]] = {}
    phrases: list[tuple[str, str, int]] = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or line.startswith("-"):
                continue
            cols = line.split("\t")
            if len(cols) < 2 or len(cols) > 3:
                continue
            word, pinyin = cols[0].strip(), cols[1].strip().lower()
            # 注意：词组拼音保持 rime 的空格分隔（"ni hao"）——引擎 buffer 是
            # 连续字母，整串查询不命中词组，靠逐音节 fallback 出单字。不要为
            # "激活词组"去空格：luna_pinyin 词组段是繁体文言成语（中國內地、
            # 一不拗衆），激活后简体候选栏被繁体占满，比逐字更糟。词组治理
            # 需换简体常用词源（terrapinyin/cc-cedict），另立任务。
            if not word or not pinyin or not all(is_cjk(c) for c in word):
                continue
            weight = parse_weight(cols[2]) if len(cols) == 3 else None
            if len(word) == 1:
                if not any(p == pinyin for p, _ in singles.setdefault(word, [])):
                    singles[word].append((pinyin, weight))
            else:
                phrases.append((word, pinyin, weight or 0))
    return singles, phrases


def parse_weight(s: str) -> int:
    """`NN.NN%` → round(percent × 1000)（与 compiler.rs parse_freq 一致）；整数直取。"""
    s = s.strip()
    if not s:
        return 0
    if s.endswith("%"):
        return round(float(s[:-1]) * 1000)
    try:
        return int(s)
    except ValueError:
        return 0


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: gen_luna_dict.py <luna_pinyin.dict.yaml> > luna_merged.tsv", file=sys.stderr)
        sys.exit(1)
    first, second = gb2312_levels()
    if len(first) != 3755 or len(second) != 3008:
        print(f"FATAL: GB2312 一级 {len(first)}/3755、二级 {len(second)}/3008 不符", file=sys.stderr)
        sys.exit(1)
    singles, phrases = parse_rime(sys.argv[1])

    kph = load_khanyupinlu(unihan_member("Unihan_Readings.txt"))
    trad_of, _simp_of = load_variants(unihan_member("Unihan_Variants.txt"))

    # 读音保留规则（2026-09-26 修，原为「显式 0% 即丢弃」）：
    #   有 kHanyuPinlu 词次证据的读音 **一律保留**；0% 过滤器只对无证据读音生效。
    # 依据：上游第三列是「该读音的份额」，**0% 的含义是份额为 0，不等于该读音不存在** ——
    # luna 单字表里 开 kai 0%（jian 95%）、备 bei 0%（yuan 100%）、广 guang 0%（yan 100%）
    # 等 38 字，常用读音被标 0%，旧规则整条丢弃 → 这些字只能由罕用读音打出来（开→jian）。
    # kHanyuPinlu 是逐读音语料词次（开 kai 3483），是「读音存在」的直接证据，优先级高于
    # 上游的份额标注。无证据且 0% = 上游自己声明不用，仍旧丢弃。
    readings = {
        ch: [
            p
            for p, w in ps
            if w is None or w > 0 or (kph.get(ch) or {}).get(p, 0) > 0
        ]
        for ch, ps in singles.items()
    }

    def key_of(ch: str, py: str) -> tuple[int, int, int, int, int, str, str]:
        cov = (kph.get(ch) or {}).get(py, 0)
        s, p = segpos(ch)
        # 同频时简体专形优先（0 排在 1 前）：发（有繁体 發/髮）先于 髮，
        # 也压住靠 kSimplifiedVariant 继承词次的变形字。
        only_simp = 0 if any(v != ch for v in trad_of.get(ch, [])) else 1
        return (0 if cov else 1, -cov, only_simp, s, p, ch, py)

    ordered: list[tuple[str, str]] = sorted(
        ((ch, py) for ch, pys in readings.items() for py in pys),
        key=lambda cp: key_of(*cp),
    )

    # 段 4 词组：权重降序，文件序 tiebreak（luna 词组第三列是读音概率，几乎全 0，
    # 故此序≈文件序；词组整体排在单字之后，见 LICENSES.md）
    phrases.sort(key=lambda p: (-p[2], p[0]))

    n = len(ordered) + len(phrases)
    assert n < FMAX, "行数超过 FMAX，spacing 归零"
    spacing = FMAX // n
    out: list[str] = [
        f"{ch}\t{py}\t{FMAX - idx * spacing}" for idx, (ch, py) in enumerate(ordered)
    ]
    for idx, (word, py, _) in enumerate(phrases):
        out.append(f"{word}\t{py}\t{FMAX - (len(ordered) + idx) * spacing}")

    sys.stdout.write("\n".join(out) + "\n")
    chs = {ch for ch, _ in ordered}
    print(f"单字: {len(chs)} (一级 {sum(1 for c in chs if c in first)}, "
          f"二级 {sum(1 for c in chs if c in second)}, 其余 {len(chs) - sum(1 for c in chs if c in first + second)})", file=sys.stderr)
    print(f"有词次单字: {sum(1 for c in chs if c in kph)}", file=sys.stderr)
    print(f"词组: {len(phrases)}", file=sys.stderr)
    print(f"总行数(含多读音展开): {len(out)}", file=sys.stderr)
    print(f"freq 范围: [{FMAX - (n - 1) * spacing}, {FMAX}] spacing={spacing}", file=sys.stderr)


if __name__ == "__main__":
    main()
