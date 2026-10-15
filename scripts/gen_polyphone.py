#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""多音字**补读音**表（`data/raw/polyphone.tsv`）—— 修「打某个合法读音一个字不出」。

背景（2026-10-15 用户报障「为何单独打『么』候选词没有？」）：`么` 在 luna 词库里只挂了
`ma`/`me`/`yao` 三个键，**没有 `mo`** —— 打 `mo` 出来的是 `模 摸 默 …`，`么` 一个字不出。
同类：`绿` 缺 `lu`（绿林）、`咯` 缺 `ka`（咯血）、`薄` 缺 `bo`… 这一类是**词库缺读音**，
不是排序问题（引擎侧实测：26 键与 9 键、新旧两版引擎，`me`/`63` 的 `么` 都在第 0 位）。

修法：从 Unihan（**与 `scripts/hanzi_freq.py` 同一份 pin**）取每个字的**全部读音**，
滤成合法音节，落成一张小表交给引擎做**纠偏层**（`crates/engine-core/src/polyphone.rs`，
include_str! 进二进制 —— 与 `trad_chars.rs` 同一条先例）。引擎在候选层把它当
「读音精确匹配」的补充候选加入，**去重交给既有的 seen 集合**（已在主查询结果里的不重复加）。

为什么不直接重生成 luna.opid：上游 `luna_pinyin.dict.yaml` 的读音列表本身就缺这些
（`么` 的上游读音里没有 `mo`），改数据要动生成链与上游 pin；而这一类是**有限、可枚举、
与读音有关**的补丁，做成表更诚实（表里每一行都能被 Unihan 的字段原文复核）。

数据源：Unihan.zip（版本与 SHA-256 都 pin 在 `scripts/hanzi_freq.py`，本脚本复用）。

读音来源（**只要现代词典口径**）：`kXHC1983`（现代汉语词典）· `kTGHZ2013`（通用规范
汉字表）· `kMandarin`（普通话读音）· `kHanyuPinlu`（语料词次，算证据）。
⚠️ **`kHanyuPinyin` 全量历史读音表故意不收**：它太宽 —— 实测按它会补出 `百(mò)`、
`貌(mò)` 这类只有古读、现代词典一字不载的读音（打 `mo` 会给出一串不相干的字）。
**变体孪生字**：一个字自己的现代条目可能不含某个读音，而它的繁/简孪生字含 ——
`么` 就是这种（`mó` 只在 `麽` 的 kXHC1983/kTGHZ2013 里，`幺麽` 的 mó）。
故读音 = `modern(字) ∪ ⋃ modern(孪生字)`（孪生关系取 Unihan_Variants 一跳）。
`百`/`貌` 没有孪生字带着 `mò` ⇒ 仍然收不进来 ✓ 这正是要的分界。

词频：优先取**词库侧**该字的最大词频（转储第三列，同量纲）；词库不认识这个字
（罕见）才退该读音的 `kHanyuPinlu` 词次，再没有则 0（引擎会排在同类候选尾部）。

过滤（**只留词库真缺的**）：拿 `opi-tools dump-keys <luna.opid>` 的字→键转储做差集 ——
表里已有的读音不再重复收录（否则表会涨到上万行，且引擎侧每次都要去重）。
转储命令可复现（`.opid` 是二进制、python 读不了，故取键转储放在 opi-tools 里）。

用法：
  opi-tools dump-keys android/app/src/main/assets/luna.opid > /tmp/luna_keys.tsv
  python3 scripts/gen_polyphone.py --dump /tmp/luna_keys.tsv
  python3 scripts/gen_polyphone.py --dump … --out /tmp/x    # 写到别处（对拍用）
幂等：同输入同输出（排序确定）。
"""

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import hanzi_freq as hf  # noqa: E402  （复用它的 Unihan 下载 + pin 校验）

REPO = Path(__file__).resolve().parent.parent
OUT_DEFAULT = REPO / "data/raw/polyphone.tsv"

def syllables() -> set[str]:
    """合法音节集 = 引擎侧那张表的**唯一真源**（从 pinyin.rs 抽，别手抄）。"""
    src = (REPO / "crates/engine-core/src/pinyin.rs").read_text(encoding="utf-8")
    body = src.split("pub const SYLLABLES", 1)[1].split("];", 1)[0]
    return set(re.findall(r'"([a-z]+)"', body))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(OUT_DEFAULT))
    ap.add_argument(
        "--dump",
        required=True,
        help="opi-tools dump-keys <luna.opid> 的输出路径（字<TAB>键,键…）",
    )
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    # Unihan 的下载 + pin 校验 + 成员解包都由 hanzi_freq 提供（同一份 pin，别复制）
    readings = hf.unihan_member("Unihan_Readings.txt")
    zones_variants = hf.unihan_member("Unihan_Variants.txt")
    syl = syllables()
    lv1, lv2 = hf.gb2312_levels()
    in_gb = set(lv1) | set(lv2)

    pinlu: dict[str, dict[str, int]] = {}   # 字 → {读音: 词次}
    modern: dict[str, set[str]] = {}        # 字 → {现代词典口径的读音}
    for line in readings.splitlines():
        if line.startswith("#") or not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        cp, field, value = parts[0], parts[1], parts[2]
        if not cp.startswith("U+"):
            continue
        ch = chr(int(cp[2:], 16))
        if field == "kHanyuPinlu":
            for m in re.finditer(r"([^\s()]+)\((\d+)\)", value):
                r = hf.normalize(m.group(1))
                if r in syl:
                    pinlu.setdefault(ch, {})[r] = pinlu.setdefault(ch, {}).get(r, 0) + int(m.group(2))
                    modern.setdefault(ch, set()).add(r)
        elif field in ("kXHC1983", "kTGHZ2013"):
            # 形如 `0740.100:lù 0743.010:lǜ`（同一字多条 = 多个读音）
            for chunk in value.split():
                _, _, rs = chunk.partition(":")
                for r in rs.split(","):
                    r = hf.normalize(r.strip())
                    if r in syl:
                        modern.setdefault(ch, set()).add(r)
        elif field == "kMandarin":
            for r in value.split():
                r = hf.normalize(r.strip())
                if r in syl:
                    modern.setdefault(ch, set()).add(r)

    # 变体孪生字（Unihan_Variants 一跳）：读音 = modern(字) ∪ ⋃ modern(孪生)
    trad_of, simp_of = hf.load_variants(zones_variants)
    all_read: dict[str, set[str]] = {ch: set(rs) for ch, rs in modern.items()}
    for ch in list(modern):
        for v in list(trad_of.get(ch, [])) + list(simp_of.get(ch, [])):
            all_read.setdefault(ch, set()).update(modern.get(v, set()))
    # 只留「有真实语料证据」或「GB2312 常用字」的字（否则表会涨到几万行：
    # 扩展区生僻字也带一堆读音，而报障的是常用字打不出来）
    # 词库现有键与**该字在词库里的最大词频**：字 → ({键}, 词频)
    have: dict[str, set[str]] = {}
    dict_freq: dict[str, int] = {}
    for line in Path(args.dump).read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) >= 2 and parts[0]:
            have[parts[0]] = set(parts[1].split(","))
            if len(parts) >= 3 and parts[2].isdigit():
                dict_freq[parts[0]] = int(parts[2])

    rows = []
    for ch, rs in all_read.items():
        if not (ch in pinlu or ch in in_gb):
            continue
        known = have.get(ch, set())
        top = max(pinlu.get(ch, {}).values(), default=0)
        for r in sorted(rs):
            if r in known:
                continue  # 词库已经能打出来 ⇒ 不是缺口，不收
            # 落位词频 = **该字在词库里的最大词频 × 该读音的语料份额**：
            #   份额 = kHanyuPinlu 里这个读音的词次 ÷ 该字最大词次（缺证据 ⇒ 0）。
            # 为什么不是「该字最大词频」整份给：那会让**冷读音**按整字的常用度落位 ——
            # 实测踩过：`儿/ní`（儿 的罕读）凭 儿 的常用度排到 `ni` 候选第 2，压住 `你`。
            # 份额制下：`儿/ní` 份额 0 ⇒ 沉到精确字组尾部（页 20 之后，实际看不见）；
            # 有证据的读音（如 `行/háng` 这类带 kHanyuPinlu 的）则按真实份额落位。
            # `么/mó`、`绿/lù` 同样是 0 ⇒ 落在各自音节的精确字组尾部 —— **可达**（翻页能找到），
            # 不再像补这条之前那样「一个字都打不出来」。
            share_num = pinlu.get(ch, {}).get(r, 0)
            share_den = top  # 该字最大词次（0 = 该字无语料证据）
            base = dict_freq.get(ch, 0)
            freq = base * share_num // share_den if share_den else 0
            rows.append((ch, r, freq))
    rows.sort(key=lambda t: (t[1], t[0]))  # 读音 → 字（与查表方向同序）

    # 参考量纲：词库里**所有单字条目**的最大词频 —— 表里的词频就是按它写的。
    # 引擎侧拿活跃词库的 max_freq 相对它缩放（见 `polyphone.rs` 头注「量纲」那条：
    # 夹具词库量纲小的时候，不缩放会让补进来的条目凭真库量级的词频霸榜）。
    dict_max = max(dict_freq.values(), default=0)

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as f:
        f.write("# 多音字补读音（生成器 scripts/gen_polyphone.py，**勿手改**）\n")
        # ⚠️ 数字后面**不要紧跟别的字符**：引擎侧按 `# dict_max <N>` 取值（见
        # `polyphone.rs::reference_max`）—— 写成 `4000000000（说明…）` 会让它解析失败、
        # 静默退化成「不适用」（2026-10-15 实测踩过：补读音整层不生效）。
        f.write(f"# dict_max {dict_max}\n")
        f.write("#↑ 表内词频的参考量纲 = 取键转储里单字最大词频（引擎按活跃词库缩放）\n")
        f.write("# 列：字\t读音\t词频（词库同量纲：该字的词库最大词频 × 该读音的语料份额；缺证据 = 0）\n")
        for ch, r, freq in rows:
            f.write(f"{ch}\t{r}\t{freq}\n")
    print(f"写出 {out}：{len(rows)} 行（字 {len({c for c, _, _ in rows})} 个）")
    if args.verbose:
        for t in rows[:10]:
            print("  ", t)
    return 0


if __name__ == "__main__":
    sys.exit(main())
