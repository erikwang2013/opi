#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成符号面板数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，无表头，制表符分隔）：
  data/raw/symbol_blocks.tsv   id \\t start(十六进制) \\t end \\t name \\t common(0/1)
  data/raw/symbols.tsv         text \\t name \\t keywords(逗号分隔,无空格) \\t block_id \\t emoji(0/1)

落在 `data/raw/` 而非任务书写的 `data/generated/`：消费侧
`engine-core/src/symbols.rs::builtin()` 用 `include_str!("../../../data/raw/symbols.tsv")`
**编译期**嵌入这两份表，路径写死；且 `data/generated/` 在本仓库只放 .opid 二进制，
脚本生成的 TSV（trad_hanzi.tsv / trad_phrases.tsv）一向在 `data/raw/`。
两处必须同改才编得过 —— 改目录时同时改本行与 symbols.rs，否则 cargo 直接编译失败。

数据来源：**UnicodeData.txt** + **emoji-data.txt**（同属 UCD 18.0.0，与
scripts/hanzi_freq.py 同一版本目录、同一份 Unicode License，见 data/raw/LICENSES.md
—— 零新增许可证）。
区块范围与字符名系统性取自前者，**emoji 列取自后者的 `Emoji` 属性**，
两份都不手工维护码位表：升级 UCD 只需换版本号 + SHA。

## 区块
沿用引擎内置声明（engine-core/src/symbols.rs `builtin()`）6 个区块中的 **5 个**，
id 原样保留（1/2/3/4/6，见 BLOCKS 注释为什么不重编号）。**id 5「CJK 扩展 A」
（U+3400–4DBF）被移除** —— **扩展 A 与生僻字任务重叠，故不单列符号区块**（扩展 A
已按拼音进词库，见任务一：打 qiu 出 㐀）。详细理由：
  1. 扩展 A 的 Unicode 名一律是 `CJK UNIFIED IDEOGRAPH-3400` 这种码位标签，分词后
     keywords 只能是 cjk/unified/ideograph/3400 —— 6592 条彼此不可区分，
     面板的**搜索**（SymbolEngine::search 是关键字前缀匹配）对它完全失效；
  2. 面板的**浏览**同样失效：8 列网格里 6592 个无标签汉字，用户不可能"看到才想起"；
     且 Android 的「全部」页取 `search_symbols("")` 全量，等于每次开面板多传
     6592 个字符串过 JNI；
  3. 它与本次任务一重叠且任务一做得更好：扩展 A 已按拼音进词库（打 qiu 出 㐀），
     而**不知道读音**正是扩展 A 的常态 —— 塞进符号面板只是把"翻三页候选"
     换成"翻一千屏网格"。
要恢复：往 BLOCKS 加回 `(5, 0x3400, 0x4DBF, "CJK 扩展 A", 0)`，本脚本照常产出
（约 +6592 条），但请先解决上面第 1、2 条，否则只是把空声明换成噪声。

## 关键字（搜索靠它）
* `MANUAL` 表**人工写**（拼音 + 英文），覆盖常用符号：CJK 符号全量、
  1F600–1F64F 那 80 个表情全量、几何图形与杂项符号的常用形、平假名全量
  （训令式与黑本式罗马字都给：si/shi、ti/chi、tu/tsu、hu/fu、zi/ji）。共约 280 条。
* 未人工写的条目按 Unicode 名兜底分词：`BLACK UP-POINTING TRIANGLE` →
  black,up,pointing,triangle（按非字母数字切分，去 WITH/AND/OF 等虚词）。
  **局限：兜底只保证英文可搜，拼音搜不到** —— 生僻条目（几何图形的象限填充、
  杂项符号里的交通标志/占星符号）体验取决于用户是否碰巧输英文词。
  这是兜底不是等价替代；要提升就得继续往 MANUAL 里补。
  ⚠️ 2026-09-28 扩块后**这条局限成了主路径**：新增的约 2750 条里绝大多数只有
  英文名分词关键字 —— 打 `xiao` 出 😄 仍成立（那 80 条在 MANUAL 里），
  但打 `huojian` 出 🚀 **不成立**，得打 `rocket`。中文/拼音侧是 **CLDR 名称
  搜索**那个任务（spec 第 60/86 行），本轮有意不做：它要么新引一份上游
  （`cldr-json` 的 annotations/zh，~1MB，另有 LICENSE 与版本 pin 要接），
  要么手写三千条中文名（不可复核）。两者都超出「扩覆盖」这一半。
* 旧内置样例的首字母缩写约定（上三角 sjx）**不再机械生成**：机械生成要配一张
  几千字的拼音首字母表，而引擎是前缀匹配 —— 输 "s" 已经能命中 "sanjiao"，
  缩写只在用户恰好输全缩写时有用。需要的那几条（▲ sjx）手写进 MANUAL 即可。
* emoji 列 = **UTS#51 `Emoji` 属性**（emoji-data.txt，同一版本目录，见下 pin），
  生成期断言，`engine-core/tests/symbol_coverage.rs` 用同一份属性的快照再锁一遍。
  **旧口径「码位 > 0xFFFF」是代理指标，2026-09-28 废弃** —— 它与 Android
  `SymbolCatalog.isEmoji()` 的「含代理对」逐条同构，两头一起错：
    - 假 emoji：1F780–1F7FF（几何图形扩展）120 条方框/斜叉/三角全被当成表情，
      单键反馈前 8 槽实测是 🖾🗙🗴🗵🗶🗷 + 2 个真笑；全表 **265 条**没有 Emoji 属性；
    - 漏 emoji：BMP 区的真 emoji（☺ U+263A、♥ U+2665、⌚ U+231A、⭐ U+2B50、
      ✂ U+2702）emoji=0，不进「表情」页，只能从「全部」页/关键字搜到（**164 条**）。
  改判据后：265 条降级为 emoji=0 但**条目保留在表内**（几何图形是合法符号，
  用户仍能在「全部」页/按关键字搜到，只是不再冒充表情），164 条 BMP 真 emoji 转正。
* **修饰符/组件不入库**（`Emoji_Modifier ∪ Emoji_Component` 属性，本范围内 9 条：
  肤色 1F3FB–1F3FF、发色 1F9B0–1F9B3）：它们不是 emoji 图形本身，单独占一格渲染
  不完整、插入也无意义 —— 与 `EXCLUDE_CATEGORIES` 排除 Mn/Me 同理。判据用属性行，
  不写死码位；不进表的 9 条因此连 emoji=0 的格子都不会占。
* ⚠️ **判据换掉后单键反馈的前 8 槽变了**（如实记，不是新 bug）：`candidates.rs`
  单键走 `search_emoji`，命中按 text（码位）升序平局，BMP 真 emoji 现在也进通道，
  低码位优先，于是打 a 从 🌆🌉🌍🌎🌏🍆🍎🍏 变成 ↔↕↖↗↘↙↩↪（箭头）、
  打 x 从 🖾🗙🗴🗵🗶🗷😂😄 变成 ▪▫☃☪☺♥⛄✖。那些字**确实有** Emoji 属性，
  判据是对的；要让前 8 槽优先出默认彩色的字，得改引擎侧的平局规则
  （按 `Emoji_Presentation` 分档，动 `candidates.rs`/`symbols.rs`），不在本脚本范围内。

用法（需要网络下载 UnicodeData.txt 与 emoji-data.txt）：
  python3 scripts/gen_symbols.py
"""
import hashlib
import os
import re
import sys
import urllib.request

from symbol_keywords import KANA_EXTRA, KANA_ROMAJI, MANUAL

# ---- 上游 pin（与 hanzi_freq.py 同版本目录）----
UNIHAN_VERSION = "18.0.0"
UNICODE_DATA_URL = f"https://www.unicode.org/Public/{UNIHAN_VERSION}/ucd/UnicodeData.txt"
UNICODE_DATA_SHA256 = "0736451de439ae7baf1425136617da495e09ee5afbe6e394374db7009ea08950"
# emoji-data.txt（UTS#51 属性，文件头 `Date: 2026-01-30`，108719 字节）——
# emoji 列的唯一判据来源。与 UnicodeData.txt 同版本目录、同一份 Unicode License。
EMOJI_DATA_URL = f"https://www.unicode.org/Public/{UNIHAN_VERSION}/ucd/emoji/emoji-data.txt"
EMOJI_DATA_SHA256 = "80d00f8e616a0ef27fd6b8de3b758c06383b5d917e2977709578e68baf733bf1"

# (id, start, end, name, common)。id 与 engine-core `builtin()` 逐一对应，
# **id 5 空出不重编号**：平假名若改成 5，任何仍认为「5 = 扩展 A」的旧代码会
# **静默**取到平假名（错得看不出来）；留空则它拿到空列表，错得响。
#
# 1–6 是原有块，**顺序与内容不许动**（决定 symbols.tsv 行序 = 面板内条目序，
# 且被 symbol_mode.rs 等用例钉着）；7 起为 2026-09-28 补的 emoji 覆盖块，
# 一律追加在尾部（新区块只 append，不改写前缀 —— 注：2026-09-28 换 emoji 判据
# 那一轮**破了**这条保证：emoji 列在原行上改写、且删了 9 条修饰符/组件行，
# 见模块头「修饰符/组件不入库」）。
#
# 新增块取自 UTS#51 §2.1 的 emoji 区块表（Unicode 15.1），与既有块 2/3 同一口径：
# 一个 Unicode 区块进表就是整块，不按「这块里有多少个是 emoji」筛
# —— emoji 与否是**逐码位**由属性判定的，与块无关（块只决定浏览分组）。
#
# ⚠️ 规范里的「~3700」是 emoji-test.txt 的完全限定**序列**数（15.1 实测 3776），
# **不是码位数**，本表到不了：条目 text 必须单字符（rare_coverage.rs
# `symbol_data_wellformed`）。本表给的是这些区块内全部已分配、非组合类码位。
#
# ⚠️ 非 BMP 的「非图形」块（麻将 1F000 / 多米诺 1F030 / 扑克 1F0A0 / 带圈字母数字
# 补充 1F100 / 带圈表意文字补充 1F200）有意不收：`candidates.rs` 单键走
# `search_emoji` 且命中按码位升序平局排序，这几块里的 emoji 码位**低**
# （🀄 1F004、🃏 1F0CF、🅰 1F170…），必然占满每个字母键的前 8 槽（实测打 s 出
# 麻将牌而非 😀）。代价：那 58 条带 Emoji 属性的字符不在表内；要恢复先解决
# 上面那条平局排序。换判据后 BMP 的 emoji 块也进 emoji 通道了（＝7–13 整块收，
# 其中 ☺♥⌚⭐ 等直接转正），同一个平局问题对它们同样成立，见模块头的 ⚠️。
BLOCKS = [
    (1, 0x3000, 0x303F, "CJK 符号", 1),
    (2, 0x25A0, 0x25FF, "几何图形", 0),
    (3, 0x2600, 0x26FF, "杂项符号", 0),
    (4, 0x1F600, 0x1F64F, "表情符号", 0),
    (6, 0x3040, 0x309F, "平假名", 0),
    # ---- 2026-09-28 追加：UTS#51 emoji 区块（BMP 段；emoji 列逐码位由属性定）----
    (7, 0x2190, 0x21FF, "箭头", 0),
    (8, 0x2300, 0x23FF, "杂项技术符号", 0),
    (9, 0x2460, 0x24FF, "带圈字母数字", 0),
    (10, 0x2700, 0x27BF, "装饰符号", 0),
    (11, 0x2900, 0x297F, "补充箭头-B", 0),
    (12, 0x2B00, 0x2BFF, "杂项符号与箭头", 0),
    (13, 0x3200, 0x32FF, "带圈中日韩字母", 0),
    # ---- 2026-09-28 追加：UTS#51 emoji 区块（非 BMP 图形段）----
    (14, 0x1F300, 0x1F5FF, "杂项符号与图形", 0),
    (15, 0x1F680, 0x1F6FF, "交通与地图符号", 0),
    (16, 0x1F780, 0x1F7FF, "几何图形扩展", 0),
    (17, 0x1F900, 0x1F9FF, "补充符号与图形", 0),
    (18, 0x1FA70, 0x1FAFF, "符号与图形扩展-A", 0),
]

# 旧硬编码符号表的 8 条 —— 生成器是「在现有基础上扩充」，不是整表替换：
# 这些字与其 keywords **一条都不能少**。钉的是**用户可见行为**，不是数据美观：
# opi-ffi 的 `cabi_symbols_blocks_and_search` 与 api `symbol_search_and_blocks`
# 都断言「搜索 he 命中 ♥」。丢关键字不会编译失败，只会让远处的 ffi 测试红。
# 码位 → 必须保留的 keywords（旧表原样 + ♥ 的 he）。
LEGACY: dict[int, tuple[str, ...]] = {
    0x3001: ("dun", "comma"),
    0x3002: ("ju", "period"),
    0x3008: ("shu",),
    0x25B2: ("sjx", "triangle"),
    0x2665: ("heart", "ai", "xin", "he"),
    0x2605: ("star", "xing"),
    0x1F604: ("xiao", "smile", "laugh"),
    0x3042: ("a",),
}

# 不入库的字符类别：Cc/Cf/Cs/Co 不可见；Mn/Me 是**组合**记号 ——
# 在 8 列网格里它是附在上一个字上的浮点，单独插入也只会粘住前一个字符，
# 在符号面板里没有独立意义。本范围内命中 6 个：302A–302D（声调符）、3099–309A（浊点）。
# Zs 保留（U+3000 全角空格是常用符号）。
# 同理排除的还有 UTS#51 的修饰符/组件（肤色/发色，见 `component_prop`）——
# 它们的 gc 是 Sk 不是 Mn，类别这条拦不住，得另按属性拦。
EXCLUDE_CATEGORIES = {"Cc", "Cf", "Cs", "Co", "Mn", "Me", "Zl", "Zp"}

# 人工关键字表（MANUAL）与平假名罗马字例外表（KANA_ROMAJI / KANA_EXTRA）
# 在 scripts/symbol_keywords.py —— 数据侧，不是脚本。拆出去的原因：本文件贴着
# 500 行门禁（crates/opi-ffi/tests/line_limit.rs，按文件数行），加注释就超。

def fetch_pinned(url: str, want_sha: str, what: str) -> str:
    """下载并校验 SHA-256；不符即报错（宁可失败也不静默换数据）。"""
    with urllib.request.urlopen(url, timeout=180) as r:
        raw = r.read()
    got = hashlib.sha256(raw).hexdigest()
    if got != want_sha:
        raise RuntimeError(
            f"{what} 摘要与 pin 不符：期望 {want_sha}，实得 {got}。"
            f"上游内容变了，请确认后更新 scripts/gen_symbols.py 的 pin 并重跑门禁。"
        )
    return raw.decode("utf-8")


def parse_emoji_data(text: str) -> dict[str, list[tuple[int, int]]]:
    """emoji-data.txt → {属性名: [(start, end), ...]}。

    行格式 `1F3FB..1F3FF  ; Emoji_Modifier   # 注释`；`#` 后是注释，区间写法 A..B
    （单码位就是 A）。**整份文件照抄入库的只有 `Emoji` 属性**，其余属性只用于排除。
    """
    out: dict[str, list[tuple[int, int]]] = {}
    for line in text.splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        rng, prop = (f.strip() for f in line.split(";"))
        first, _, last = rng.partition("..")
        out.setdefault(prop, []).append((int(first, 16), int(last or first, 16)))
    return out


def in_ranges(ranges: list[tuple[int, int]], cp: int) -> bool:
    return any(a <= cp <= b for a, b in ranges)


def parse_unicode_data(text: str) -> dict[int, tuple[str, str]]:
    """UnicodeData.txt → {码位: (字符名, 总类)}。First/Last 区间标记按区间展开。"""
    out: dict[int, tuple[str, str]] = {}
    pending: tuple[int, str, str] | None = None
    for line in text.splitlines():
        fields = line.split(";")
        if len(fields) < 3:
            continue
        cp = int(fields[0], 16)
        name, cat = fields[1], fields[2]
        if name.endswith(", First>"):
            pending = (cp, cat, name[: -len(", First>")])
        elif name.endswith(", Last>"):
            assert pending is not None, "Last 无对应 First"
            start, cat0, base = pending
            for c in range(start, cp + 1):
                out[c] = (base, cat0)
            pending = None
        else:
            out[cp] = (name, cat)
    return out


_STOPWORDS = {"with", "and", "or", "for", "the", "of", "a", "an"}


def name_keywords(uname: str) -> list[str]:
    """Unicode 名 → 小写词元（'BLACK UP-POINTING TRIANGLE' → black,up,pointing,triangle）。"""
    words = re.split(r"[^a-z0-9]+", uname.lower())
    return [w for w in words if w and w not in _STOPWORDS]


def kana_row(cp: int, uname: str) -> tuple[str, list[str]]:
    """平假名条目：中文名沿用旧内置样例 `平假名a` 的形式（小写假名加「小」），
    关键字 = 罗马字（含小写的 x/l 输入法写法）+ 全名 + 块名。"""
    tail = uname.split("HIRAGANA LETTER ", 1)[-1].lower()
    small = tail.startswith("small ")
    letter = tail[len("small "):] if small else tail
    romaji = KANA_ROMAJI.get(cp, letter)
    keys = [romaji, "hiragana", "pingjia", "riyu"] + name_keywords(uname)
    if small:
        keys += [f"x{romaji}", f"l{romaji}"]
    keys += KANA_EXTRA.get(cp, [])
    return f"平假名{'小' if small else ''}{romaji}", keys


def rows_for_block(block: tuple[int, int, int, str, int], chars: dict[int, tuple[str, str]],
                   emoji_prop: list[tuple[int, int]],
                   component_prop: list[tuple[int, int]]):
    """区块内逐码位产出 (text, name, keywords, block_id, emoji)。"""
    bid, start, end, _name, _common = block
    for cp in range(start, end + 1):
        if cp not in chars:
            continue  # 未分配码位
        uname, cat = chars[cp]
        if cat in EXCLUDE_CATEGORIES:
            continue
        # 修饰符/组件（肤色/发色）：不是 emoji 图形本身，单独一格渲染不完整，
        # 与 Mn/Me 同理**不入库**（不入库连 emoji=0 的格子都不占）。
        if in_ranges(component_prop, cp):
            continue
        if cp in MANUAL:
            name, kw = MANUAL[cp]
            keys = kw.split(",") + name_keywords(uname)
        elif bid == 6:
            name, keys = kana_row(cp, uname)
        else:
            name, keys = uname, name_keywords(uname)
        # emoji 标记 = **UTS#51 `Emoji` 属性**，既不是「区块 4」也不是「非 BMP」：
        # 判据只在属性表里，与码位高低、区块归属都无关。生成期下面的断言与
        # engine-core/tests/symbol_coverage.rs 的 UTS51_EMOJI 快照各锁一遍。
        yield chr(cp), name, list(dict.fromkeys(keys)), bid, 1 if in_ranges(emoji_prop, cp) else 0


def write_atomic(path: str, lines: list[str]) -> None:
    tmp = f"{path}.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    os.replace(tmp, path)


def main() -> None:
    chars = parse_unicode_data(fetch_pinned(UNICODE_DATA_URL, UNICODE_DATA_SHA256, "UnicodeData.txt"))
    eprops = parse_emoji_data(fetch_pinned(EMOJI_DATA_URL, EMOJI_DATA_SHA256, "emoji-data.txt"))
    emoji_prop = eprops["Emoji"]
    # 修饰符/组件：肤色（Emoji_Modifier）与发色/ZWJ/VS16/区域指示符（Emoji_Component）。
    # 两个属性都可能与 `Emoji` 重叠 —— 重叠的部分按「不入库」处理，故这里取并集。
    component_prop = eprops["Emoji_Modifier"] + eprops["Emoji_Component"]

    per_block = {b[0]: list(rows_for_block(b, chars, emoji_prop, component_prop)) for b in BLOCKS}

    # 自检（生成期门禁，与 crates/opi-tools/tests/rare_coverage.rs 的产物门禁同义）：
    # 1) 每个 common=1 区块 ≥ 32 条（8 列面板 4 行）：防「声明了区块却几乎没内容」——
    #    改动前 CJK 扩展 A 就是这么白声明的。32 是下限而非目标，实测最小块 60 条。
    # 2) emoji=1 ⟺ UTS#51 `Emoji` 属性：判据就是查这份属性表，别处（引擎/Android）
    #    再用别的口径判一遍就会各说各话 —— 这条拦住生成期，symbol_coverage.rs 拦产物。
    for bid, _s, _e, name, common in BLOCKS:
        rows = per_block[bid]
        if common and len(rows) < 32:
            raise SystemExit(f"FATAL: 常用区块 {bid}「{name}」只有 {len(rows)} 条（下限 32）")
        for text, _n, keys, _b, emoji in rows:
            cp = ord(text)
            if emoji != in_ranges(emoji_prop, cp):
                raise SystemExit(
                    f"FATAL: U+{cp:04X} emoji={emoji} 与 UTS#51 Emoji 属性不符"
                )
            if in_ranges(component_prop, cp):
                raise SystemExit(f"FATAL: U+{cp:04X} 是修饰符/组件，不该入库")
            # 关键字必须是小写 ASCII 字母数字：SymbolEngine::search 把查询转小写后
            # 按**字节前缀**比较，含大写/空格/非 ASCII 的关键字永远搜不到。
            # （这条抓到过真错：手写表里的 fangkUang / yinFu / DIE FACE-1 的 "1"。）
            for k in keys:
                if not re.fullmatch(r"[a-z0-9]+", k):
                    raise SystemExit(f"FATAL: U+{ord(text):04X} 关键字 {k!r} 非小写 ASCII 字母数字")
            assert "\t" not in text and " " not in text, f"U+{ord(text):04X} text 含空白"

    # 旧表 8 条必须原样活着（丢了只会让 opi-ffi 的断言在远处红，这里当场拦下）
    have = {t: set(ks) for rows in per_block.values() for t, _n, ks, _b, _e in rows}
    for cp, need in LEGACY.items():
        ch = chr(cp)
        if ch not in have:
            raise SystemExit(f"FATAL: 旧内置条目 U+{cp:04X} {ch} 在新数据里消失了")
        lost = [k for k in need if k not in have[ch]]
        if lost:
            raise SystemExit(f"FATAL: U+{cp:04X} {ch} 丢了旧关键字 {lost}（opi-ffi 断言依赖）")

    blocks_tsv = [
        f"{bid}\t{start:04X}\t{end:04X}\t{name}\t{common}"
        for bid, start, end, name, common in BLOCKS
    ]
    symbols_tsv = [
        f"{text}\t{name}\t{','.join(keys)}\t{bid}\t{emoji}"
        for bid in per_block
        for text, name, keys, _b, emoji in per_block[bid]
    ]
    write_atomic("data/raw/symbol_blocks.tsv", blocks_tsv)
    write_atomic("data/raw/symbols.tsv", symbols_tsv)

    print(f"UCD {UNIHAN_VERSION} UnicodeData.txt + emoji-data.txt sha256 校验通过"
          f"（emoji 属性 {len(emoji_prop)} 段，排除的修饰符/组件 {len(component_prop)} 段）")
    total = 0
    for bid, start, end, name, common in BLOCKS:
        n = len(per_block[bid])
        total += n
        hand = sum(1 for cp in range(start, end + 1) if cp in MANUAL)
        derived = n - hand if bid == 6 else 0  # 平假名：罗马字由字符名推导，非手写
        print(f"  区块 {bid} {name:10s} U+{start:04X}-{end:04X} {n:4d} 条"
              f"（手写 {hand}，罗马字推导 {derived}，名字分词兜底 {n - hand - derived}）"
              f"common={common}")
    print(f"合计 {total} 条 → data/raw/symbols.tsv")


if __name__ == "__main__":
    main()
