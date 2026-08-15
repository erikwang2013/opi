#!/usr/bin/env python3
"""luna 拼音词库统一常用度排序（修复 rime 权重缺陷）。

背景（spec 2026-08-15 验收偏差 #4）：luna_pinyin.dict.yaml 第 3 列是读音概率
份额（多读音字按 P(读音|字) 分派，单读音字 100%），非词频。引擎前缀匹配 +
纯 freq 排序下，100% 字（蒿/樊/泛/倭/妳）压过无权重常用字（好/发/我/你），
fa → 樊 非 发。本脚本以 GB2312 一二级（一级拼音序、二级部首序，即常用度序）
为单字段主体，词组按权重降序排在全部单字之后——单音节输入出常用字，
多音节输入才出词组。

段序：GB2312 一级 → GB2312 二级 → 其余单字（码位序）→ 词组（权重降序，文件序 tiebreak）。
freq = FMAX - idx × (FMAX // N)，FMAX = 4e9（同 trad：u32 上限内、段内可区分；
引擎 user_boost 按 max_freq×2 缩放，learner 一次选词仍压过全部静态词）。

用法：python3 scripts/gen_luna_dict.py <luna_pinyin.dict.yaml> > /tmp/luna_merged.tsv
产物不入库（luna.opid 本身 gitignore）；部署副本 android/app/src/main/assets/luna.opid。
"""
import codecs
import sys

FMAX = 4_000_000_000


def gb2312_rows() -> tuple[list[str], list[str]]:
    """GB2312 汉字区 B0–F7 行 × A1–FE 列（GB 码序）；row ≤ 0xD7 为一级（拼音序），
    其余二级（部首序）。D7FA–D7FE 未定义，显式跳过。"""
    first: list[str] = []
    second: list[str] = []
    for row in range(0xB0, 0xF8):
        for col in range(0xA1, 0xFF):
            if row == 0xD7 and col >= 0xFA:
                continue
            try:
                ch = codecs.decode(bytes([row, col]), "gb2312")
            except UnicodeDecodeError:
                continue
            (first if row <= 0xD7 else second).append(ch)
    return first, second


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
            if not word or not pinyin or not all(0x4E00 <= ord(c) <= 0x9FFF for c in word):
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
    first, second = gb2312_rows()
    if len(first) != 3755 or len(second) != 3008:
        print(f"FATAL: GB2312 一级 {len(first)}/3755、二级 {len(second)}/3008 不符", file=sys.stderr)
        sys.exit(1)
    singles, phrases = parse_rime(sys.argv[1])

    # 读音过滤（显式 0% = 从不使用）+ 概率表（非表音读音组内排序用）
    readings = {ch: [p for p, w in ps if w is None or w > 0] for ch, ps in singles.items()}
    probs = {ch: {p: (w or 0) for p, w in ps} for ch, ps in singles.items()}

    # GB 一级表段归属：连续同读音运行，段读音 = 该字在表中的主读音。
    # 次读音（rime 权重低于表音，如 镐 gao/hao、都 dou/du）排目标读音组尾，
    # 避免"hao"查询被 镐(gao 段位置) 压过 好——字频仅由其主读音位置决定。
    # 健壮性：段内孤字（rime 缺该段读音，如 茧 chong 100%/jian 0%）按前
    # 后字前瞻归段；新段起始取权重最高读音（夯 ben 2.88%/hang 97.12% → hang）。
    biaoyin: dict[str, str] = {}
    cur: str | None = None
    for i, ch in enumerate(first):
        rs = readings.get(ch)
        nxt = readings.get(first[i + 1]) if i + 1 < len(first) else None
        if rs and (cur in rs or (nxt and cur in nxt)):
            biaoyin[ch] = cur
        else:
            cur = max(rs, key=lambda p: probs[ch].get(p, 0)) if rs else None
            biaoyin[ch] = cur
    pos = {ch: i for i, ch in enumerate(first)}
    anchor: dict[str, int] = {}
    for ch, by in biaoyin.items():
        if by:
            anchor[by] = max(anchor.get(by, 0), pos[ch])
    LARGE = len(first)

    def key_of(ch: str, py: str) -> tuple[int, int, int, str]:
        if biaoyin.get(ch) == py:
            return (pos[ch], 0, 0, ch)
        # 非表音读音排组尾：组内按 rime 读音概率降序（重 chong 41.55% 靠前，
        # rime 乱挂的蛊 chong/涌 chong 无权重 0 靠后；无权重生僻字 0 最后）
        return (anchor.get(py, LARGE), 1, -probs[ch].get(py, 0), ch)

    ordered: list[tuple[str, str]] = sorted(
        ((ch, py) for ch, pys in readings.items() for py in pys),
        key=lambda cp: key_of(*cp),
    )

    # 段 4 词组：权重降序，文件序 tiebreak
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
          f"二级 {sum(1 for c in chs if c in second)}, 其余 {len(chs) - sum(1 for c in chs if c in first + second)})")
    print(f"词组: {len(phrases)}")
    print(f"总行数(含多读音展开): {len(out)}")
    print(f"freq 范围: [{FMAX - (n - 1) * spacing}, {FMAX}] spacing={spacing}", file=sys.stderr)


if __name__ == "__main__":
    main()
