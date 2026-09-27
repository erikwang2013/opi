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
  代价（2026-09-26 实测，**下述行数/体积已被 09-27 两次改动取代**，留作历史）：
  单字 20902 → 41289（+20387）、词组 21719 → 21759、
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

简体词组段（2026-09-27 加，用户真机反馈「常用词组无法拼音或模糊拼音显示」）：
  **根因是词源不是模糊层。** luna 自己的词组段只有繁体文言成语且拼音刻意空格分隔（见
  parse_rime 注释），实测「你好」「中国」「谢谢」「什么」「我们」在 luna.opid 里 **0 条**。
  先按「复用已有繁体词组表（trad_phrases.tsv）做繁→简」评估过，**不成立**，三条实测：
    1. terra 表里**根本没有「你好」**（`grep -c '你好\t'` = 0，只有长成语 任你好漢沒處躲）；
    2. 繁→简**不是单射**：Unihan kSimplifiedVariant 有 50 个字给多个简形，且首位未必对
       （瀋 → [沈, 渖]，取首位会把「瀋陽」写成「沈阳」）；未裁决时只能整条丢弃；
    3. 覆盖率：terra 繁→简后对现代常用词基准（phrase-pinyin-data 47111 条，仅作基准、
       **不入库**）只命中 **30.6%** —— terra 是文言/成语/地名表，缺 一些/一代/补助/能干/自发。
  改用 rime-pinyin-simp 的 pinyin_simp.dict.yaml：Apache-2.0（宽松，与 MIT 源码、LGPL-3.0
  的 luna 数据均相容）、**原生简体**（无需繁→简，整类歧义消失）、有真实词频（我们 322329）。
  它是 rime 自家仓库、与 luna 同一 dict.yaml 格式，来源是 AOSP PinyinIME（同样是 Apache-2.0）。
  实测 48078 条词组（2–4 字，无更长条目）：含 GB 级繁体专形的 **0 条**（见 trad_only 守卫）。

luna 繁体成语段退役（2026-09-27，同一任务的后半）：
  上面换词源的当天，上游自带的词组段（`parse_rime` 收的 21759 条）**整段删除**，不再进产物。
  理由：它存在的唯一理由写在旧注释里——「需换简体常用词源，另立任务」，那个任务已随简体段
  落地；于是同一个库里出现「一边有繁体字形守卫、一边收着 21759 条繁体词组」的半成品状态；
  且它们仍在候选中冒头，简体用户照旧看到繁体字形。
  **但「经常看到」不成立**（删之前量过）：空格拼音只有**首音节前缀**可达 ⇒ 21759 条的
  可达查询并集仅 484 个，其中进得了候选栏（8 槽）的**只有 2 个**（`zhuai` 第 6 位
  「拽布披麻」、`seng` 第 7 位「僧伽」），其余在第 425/596/2885 位。故**收益主体是体积与
  「半成品状态」，不是可见性**；「僧伽」也不是繁体字形，属本次已知代价。
  实测代价（新旧对照，门禁与逐条等价证据见 LICENSES.md「简体词组段」）：
  词组 48078 → 48078（只删旧段，简体段一条不动）、总行数 118092 → 96333、
  luna.opid 3099275 → 2370999 字节（-23.5%）；单字层 48255 条逐条同序不变。
  **单字层的繁体字形一个不动**（國/學/電/說 等仍在单字候选中）——那是 luna 既有行为，
  ranking_quality 的 NEAR 档就是按它设计的，本次只清词组段。

ü 记法归一 + 词组语料读音证据（2026-09-27，lead 全量抽检牵出，两条都已修）：
  **A 类 42 条 —— 本源的 üe 记法与本引擎不一致。** AOSP 写 `lue`/`nue`，引擎规范是 `v`
  （pinyin.rs 的 SYLLABLES 只有 `lv/lve/nv/nve`；单字层也全用 `v`）。不归一就**同一批键
  分两层两种写法**：`lve` 出得了单字「略」出不了词组「策略」，`lue` 反过来。按**音节**
  映射（见 U_FIX），不整串 replace（连写串里 `lue` 可能是 `lu|e` 跨音节）。
  **不做 `lu→lv`/`nu→nv`**：普通话里 luc≠lü 是两个音节，本源也确实分开写（实测 339 个
  `lu` + 29 个 `nu` 里没有一个是 ü 字，阳性对照 242 个 `lv` 全对得上）。
  **B 类 8 条 —— 单字层缺读音**，见 `corpus_readings` 与本文件「读音保留规则」处的长注释；
  顺带证伪：缺单字读音**不影响整串打词组**（`larou` 改前就已命中「腊肉」第 1 位），
  受影响的是**单音节**查询（`la` 出不出「腊」）。

用法（需要网络，下载 Unihan.zip + pinyin_simp.dict.yaml）：
  python3 scripts/gen_luna_dict.py <luna_pinyin.dict.yaml> > /tmp/luna_merged.tsv
产物不入库（luna.opid 本身 gitignore）；部署副本 android/app/src/main/assets/luna.opid。
"""
import hashlib
import sys
import urllib.request

from hanzi_freq import (
    FMAX,
    gb2312_levels,
    is_cjk,
    load_khanyupinlu,
    load_variants,
    segpos,
    unihan_member,
)

# ---- 简体词组上游 pin（2026-09-27）----
# 照 gen_trad_dict.py 的 terra 做法钉死到 commit + SHA-256：luna 自己至今没 pin（头部
# 有记录），再引一个浮动上游就是把同一个坑挖两次。
# 升级路径：换 commit + 换 SHA256（`curl -sSL <url> | sha256sum`），重跑本脚本与
# `cargo test -p opi-tools --test simplified_phrases`（词组缺失会红）、
# `ranking_quality`（排序变化会红）。
SIMP_COMMIT = "0c6861ef7420ee780270ca6d993d18d4101049d0"
SIMP_URL = (
    f"https://raw.githubusercontent.com/rime/rime-pinyin-simp/{SIMP_COMMIT}"
    "/pinyin_simp.dict.yaml"
)
SIMP_SHA256 = "e341598343a0f0f2035bb1aafc34a7f3bb7887deeecb3f60796262aaa2983e6b"
# 收录词长上限：上游最长就是 4 字（实测长度分布 (1,17047)(2,34072)(3,7379)(4,6627)），
# 写死一个上限是为了「上游哪天加长条目」时有个显式决定点，不是当前在过滤。
SIMP_MAX_LEN = 4
# ü 记法归一（2026-09-27）：本源（AOSP）把 üe 写成 lue/nue，引擎规范是 lve/nve —— 见
# parse_simp 文档。**不可盲目做 lu→lv / nu→nv**：普通话里 lu/nu 与 lü/nü 是不同音节
# （路 lù vs 吕 lǚ），本源也确实分开写（lu 339 次、lv 299 次），只映射 üe 这一对。
U_FIX = {"lue": "lve", "nue": "nve"}


def fetch(url: str, sha256: str) -> str:
    """下载并校验 pin 的摘要，返回 UTF-8 文本。摘要不符即报错 —— 宁可失败也不静默换数据。"""
    with urllib.request.urlopen(url, timeout=180) as r:
        data = r.read()
    got = hashlib.sha256(data).hexdigest()
    if got != sha256:
        raise RuntimeError(
            f"{url} 摘要与 pin 不符：期望 {sha256}，实得 {got}。"
            f"上游内容变了，请确认后更新 scripts/gen_luna_dict.py 的 pin 并重跑门禁。"
        )
    return data.decode("utf-8")


def parse_rime(path: str) -> dict[str, list[tuple[str, int | None]]]:
    """luna_pinyin.dict.yaml → 单字读音表。

    单字读音记 (pinyin, weight)：无第 3 列 → weight=None（默认读音，保留）；
    显式 0% → 该读音"从不使用"（如 冰 bing 100%/ning 0%），调用方剔除，
    避免低质量读音以单字段高词频泄漏进错误前缀（ni 查询出 冰）。

    **上游的词组段（2 字以上）整段不收**（2026-09-27 退役，见文件头同名小节）：
    它是繁体文言成语、拼音刻意空格分隔（中國內地 `zhong guo nei di`），引擎 buffer 是
    连续字母 → 整串查询永不命中，只在短前缀的候选尾巴上漏出繁体字形。原先留着是等一个
    简体词源，那个词源已到（rime-pinyin-simp），留着的唯一理由随之消失。
    """
    singles: dict[str, list[tuple[str, int | None]]] = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or line.startswith("-"):
                continue
            cols = line.split("\t")
            if len(cols) < 2 or len(cols) > 3:
                continue
            word, pinyin = cols[0].strip(), cols[1].strip().lower()
            if not word or not pinyin or not all(is_cjk(c) for c in word):
                continue
            if len(word) != 1:
                continue
            weight = parse_weight(cols[2]) if len(cols) == 3 else None
            if not any(p == pinyin for p, _ in singles.setdefault(word, [])):
                singles[word].append((pinyin, weight))
    return singles


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


def parse_simp(text: str, trad_only) -> list[tuple[str, list[str], int]]:
    """pinyin_simp.dict.yaml → [(简体词组, 逐字音节表, 词频)]（2..SIMP_MAX_LEN 字）。

    音节表按**空格切分原样保留**（不 join）：它同时是拼音与字形的**逐位对齐证据**
    （上游 `ni hao` ↔ 2 字词，实测 48078/48078 字数==音节数），`corpus_readings` 靠它
    反推单字读音。连写形态在产出时 join（`main` 的发射处）。

    拼音**连写**（`"ni hao"` → `"nihao"`）——这正是本次修复的要点：引擎 buffer 是连续
    字母，空格分隔的词组整串查询永远不命中（luna 旧词组段的病）。上游拼音是逐字空格
    分隔的小写无调形（`ni hao`），去空格即可。

    **ü 记法要归一**（2026-09-27 修）：本源的 AOSP 记法把 üe 写成 `lue`/`nue`（42+8 处），
    而引擎的规范是 `v` —— `pinyin.rs` 的 SYLLABLES 里只有 `lv/lve/nv/nve`，**没有** `lue/nue`；
    单字层也全用 `v`（略 → `lve`）。不归一的后果是**同一批键分两层两种写法**：打 `lve` 出得了
    单字「略」、出不了词组「策略」，打 `lue` 反过来。故按**音节**映射 `lue→lve`、`nue→nve`。
    逐音节替换而非在连写串上 `str.replace`：连写串里 `lue` 还可能是 `lu|e` 跨音节，
    逐音节取词则无此歧义（本源 `lu`/`nu` 与 `lv`/`nv` 是两个不同的音节，未混用 ——
    实测 339 个 `lu` + 29 个 `nu` 里没有一个是 ü 字，阳性对照 242 个 `lv` 全部对得上 ü 字）。

    trad_only 是 `main` 里的判据（字不在 GB2312 且 kSimplifiedVariant 指向 GB2312 字）：
    命中即**拒绝整条**并报错，而不是静默丢弃 —— 简体库里出现繁体字形正是用户的抱怨，
    悄悄少几条会让下一个人对着「0 条繁体」的门禁找不出原因。
    """
    out: list[tuple[str, str, int]] = []
    bad: list[str] = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or line.startswith("---") or line == "...":
            continue
        cols = line.split("\t")
        if len(cols) < 2:
            continue
        word, py = cols[0].strip(), cols[1].strip()
        if not word or not py or not all(is_cjk(c) for c in word):
            continue
        if not 2 <= len(word) <= SIMP_MAX_LEN:
            continue
        toks = [U_FIX.get(t, t) for t in py.split()]
        joined = "".join(toks)
        # 非 ASCII（带调/ü 冒号形）与超长直接跳过：compiler 会以同样的条件静默丢，
        # 在这里丢至少能计数报出来。
        if not joined.isascii() or not joined or len(joined) > 255:
            continue
        if any(trad_only(c) for c in word):
            bad.append(word)
            continue
        out.append((word, toks, parse_weight(cols[2]) if len(cols) > 2 else 0))
    if bad:
        raise RuntimeError(
            f"简体词源里混入繁体字形 {len(bad)} 条（首个：{bad[0]}）——上游换表了？"
            f"简体库不得收繁体字形，请先确认词源再改本守卫。"
        )
    return out


def corpus_readings(phrases: list[tuple[str, list[str], int]]) -> dict[str, set[str]]:
    """词组语料反推的单字读音：`{字: {读音}}`。

    词组的拼音是**逐字标注**的（字数 == 音节数，实测 48078/48078），故「词 W 的拼音是
    P」⇒ `W[i]` 有读音 `P[i]`。这是**数据自证**的第二条读音证据源，与「有 kHanyuPinlu
    词次证据就保留」是同一条原则、换一个证据来源 —— 而 kHanyuPinlu 只覆盖 3799 字，
    够不到的字就只剩上游第三列可依，于是上游标 0% 的**常用读音**被丢掉。

    只**取用**证据，不做删除：本函数只回「哪些读音被语料用过」，删不删既有读音不在
    这里决定（如「柠」上游标成 `chu` 是另一个独立问题，见 LICENSES.md）。
    """
    ev: dict[str, set[str]] = {}
    for word, toks, _ in phrases:
        if len(word) != len(toks):
            continue
        for ch, t in zip(word, toks):
            ev.setdefault(ch, set()).add(t)
    return ev


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: gen_luna_dict.py <luna_pinyin.dict.yaml> > luna_merged.tsv", file=sys.stderr)
        sys.exit(1)
    first, second = gb2312_levels()
    if len(first) != 3755 or len(second) != 3008:
        print(f"FATAL: GB2312 一级 {len(first)}/3755、二级 {len(second)}/3008 不符", file=sys.stderr)
        sys.exit(1)
    singles = parse_rime(sys.argv[1])

    kph = load_khanyupinlu(unihan_member("Unihan_Readings.txt"))
    trad_of, simp_of = load_variants(unihan_member("Unihan_Variants.txt"))
    gb = set(first) | set(second)

    def trad_only(ch: str) -> bool:
        """简体库判据：字**不在 GB2312**（=简体字符集）内，却有 kSimplifiedVariant 指向
        GB2312 字 —— 即「这是个繁体专形，它有对应的简体字」。

        不用 `simp_of[ch] 与 ch 不同` 判：那会把 著/覆/乾/藉/瑙/阪 判成繁体（它们有生僻
        异构简形 着/复/干/借/碯/坂，但本身在简体里照常用 —— 著名/覆盖/乾隆/慰藉）——
        实测误报 35 条。GB2312 归属才是「简体字形」的权威判据（國/學/電/說 都不在）。"""
        return ch not in gb and any(v in gb for v in simp_of.get(ch, []))

    simp_phrases = parse_simp(fetch(SIMP_URL, SIMP_SHA256), trad_only)

    # 词组语料反推的第二条读音证据（2026-09-27）—— 见 corpus_readings 文档。
    corpus = corpus_readings(simp_phrases)

    # 读音保留规则（2026-09-26 修，原为「显式 0% 即丢弃」；2026-09-27 加第三条证据）：
    #   有 kHanyuPinlu 词次证据、**或**被词组语料用过的读音 **一律保留**；
    #   0% 过滤器只对两条证据都没有的读音生效。
    # 依据：上游第三列是「该读音的份额」，**0% 的含义是份额为 0，不等于该读音不存在** ——
    # luna 单字表里 开 kai 0%（jian 95%）、备 bei 0%（yuan 100%）、广 guang 0%（yan 100%）
    # 等 38 字，常用读音被标 0%，旧规则整条丢弃 → 这些字只能由罕用读音打出来（开→jian）。
    # kHanyuPinlu 是逐读音语料词次（开 kai 3483），是「读音存在」的直接证据，优先级高于
    # 上游的份额标注。无证据且 0% = 上游自己声明不用，仍旧丢弃。
    #
    # **为什么还要第二条证据**：kHanyuPinlu 只 3799 字，够不到的字仍只剩上游第三列可依 ——
    # lead 的全量抽检抓到的正是这批：腊 la 0%/xi 100%、南 na 0%/nan 100%、「柠」上游干脆
    # 把 chu 当主读音。后果是打 `larou` 出不了「腊肉」、`xiangbin` 出不了「香槟」、
    # `daoheng` 出不了「道行」。词组语料是**数据自证**：它真用了那条读音。
    #
    # 注意语料证据要在**这里**并入，不是往 singles 里 append：显式 0% 的读音在 singles 里
    # **本来就存在**（腊 = [('la',0), ('xi',100000)]），按「在不在表里」判会把它们当成已有，
    # 而真正丢弃它们的是下面这个过滤器 —— 证据必须进过滤器。
    corpus_added = 0
    corpus_unfiltered = 0
    corpus_skipped = 0
    readings: dict[str, list[str]] = {}
    for ch, ps in singles.items():
        ev = corpus.get(ch, set())
        # ① 上游表里**有**这条读音（可能是显式 0%）：被语料用过就留下
        kept = [
            p
            for p, w in ps
            if w is None or w > 0 or (kph.get(ch) or {}).get(p, 0) > 0 or p in ev
        ]
        corpus_unfiltered += sum(
            1
            for p, w in ps
            if w == 0 and (kph.get(ch) or {}).get(p, 0) == 0 and p in ev
        )
        # ② 上游表里**根本没有**这条读音、但语料用过（行 heng / 挝 wo / 槟 bin / 琢 zuo）：
        #    补进来，这才是纯粹的「新增读音」
        extra = sorted(ev - {p for p, _ in ps})
        corpus_added += len(extra)
        readings[ch] = kept + extra
    for ch in corpus:
        if ch not in singles:
            # 语料里有、单字表里没有的字：补它 = **新增一个单字候选**（改排序面），
            # 不是本条要修的，跳过并计数（实测当前为 0）。
            corpus_skipped += 1

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

    # 段 4 词组：只剩简体常用词组（luna 的繁体成语段已于 2026-09-27 退役），
    # 按词频降序，文件序 tiebreak；词组整体排在全部单字之后（见 LICENSES.md）。
    simp_phrases.sort(key=lambda p: (-p[2], p[0]))
    all_phrases = simp_phrases

    n = len(ordered) + len(all_phrases)
    assert n < FMAX, "行数超过 FMAX，spacing 归零"
    spacing = FMAX // n
    out: list[str] = [
        f"{ch}\t{py}\t{FMAX - idx * spacing}" for idx, (ch, py) in enumerate(ordered)
    ]
    for idx, (word, toks, _) in enumerate(all_phrases):
        out.append(f"{word}\t{''.join(toks)}\t{FMAX - (len(ordered) + idx) * spacing}")

    sys.stdout.write("\n".join(out) + "\n")
    chs = {ch for ch, _ in ordered}
    print(f"单字: {len(chs)} (一级 {sum(1 for c in chs if c in first)}, "
          f"二级 {sum(1 for c in chs if c in second)}, 其余 {len(chs) - sum(1 for c in chs if c in first + second)})", file=sys.stderr)
    print(f"有词次单字: {sum(1 for c in chs if c in kph)}", file=sys.stderr)
    print(f"词组: {len(all_phrases)}（全部为简体常用，luna 繁体成语段已退役）", file=sys.stderr)
    print(
        f"词组语料读音证据: 救回被 0% 丢掉的 {corpus_unfiltered} 条 + 新增表外读音 "
        f"{corpus_added} 条 = +{corpus_unfiltered + corpus_added} 条；"
        f"语料里有、单字表没有的字跳过 {corpus_skipped} 个",
        file=sys.stderr,
    )
    print(f"总行数(含多读音展开): {len(out)}", file=sys.stderr)
    print(f"freq 范围: [{FMAX - (n - 1) * spacing}, {FMAX}] spacing={spacing}", file=sys.stderr)


if __name__ == "__main__":
    main()
