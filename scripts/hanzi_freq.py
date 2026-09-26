#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""单字读音词频（Unihan kHanyuPinlu）+ 变体关系 + GB2312 段位，供两个生成器共用。

背景（spec 2026-08-15 验收偏差 #4）：候选首位此前由 GB2312 码序（布局序）决定
（wo→蜗、ni→呢、hao→镐、shi→匙、fa→樊）。码序是汉字编码顺序，不是使用频率，
同音字谁在前纯属历史巧合。此处提供**逐读音的语料词次**作为排序键的真实来源。

数据源：Unihan.zip 的 Unihan_Readings.txt `kHanyuPinlu` 字段——《现代汉语频率
词典》（北京语言学院）逐读音词次统计，3799 字，含繁体字形（繁體字形各自成条）。
Unihan.zip 本就是 gen_trad_dict.py 的记录上游，许可证 Unicode License
（data/raw/LICENSES.md），**不引入新来源、不新增许可证**。

同源附带：Unihan_Variants.txt 的 kTraditionalVariant / kSimplifiedVariant
（简体字形 vs 繁体字形互为对手），用于同频字形的取向判定（见各生成器 key_of）。

注意：kHanyuPinlu 覆盖 3799 字以外的字没有词次 → 键里退化为「无词次」类，
由生成器的段位（GB 一级/二级/其余）继续兜底，GB 段位设计未被推翻，只是降为
同频/缺频时的 tiebreak。

收录范围（CJK_RANGES）：本模块同时是**码位过滤口径**的唯一来源（2026-09-26 起）。
扩展 A/B 的字没有 kHanyuPinlu 词次 → 全部落在「无词次」段（段位 2、按码位序），
排在 GB2312 与常用字之后 —— 生僻字仍然后置，只是不再被整段丢掉。
"""
import codecs
import hashlib
import io
import re
import unicodedata
import urllib.request
import zipfile

# 4e9 < u32::MAX；parse_dict 接受 u32 freq，引擎按 u64 比较（同 trad 既有约定）
FMAX = 4_000_000_000

# ---- CJK 收录范围（2026-09-26 放开扩展区）----
# 此前四处 `0x4E00 <= cp <= 0x9FFF` 把扩展 A/B 整段丢弃，词库里根本没有这些字：
# README 缘起第一条「想输入一个生僻字，翻了三页候选词都找不到」说的就是这个 ——
# 翻不出来不是排序问题，是**数据不存在**。
#
# 收录到扩展 B 为止（U+2A6DF）。扩展 C 及以后（U+2A700+，另有约 3024 个带 kMandarin
# 的码位）有意**不**收：
#   * 扩展 B 已覆盖「生僻但有出处」的绝大多数用例（Unihan kMandarin 14,618 字），
#     再加 C–I 只多 ~3000 字、体积再涨 ~15%，边际收益小；
#   * 设备字体覆盖是真实瓶颈：扩展 A 在主流手机字体（Noto Sans CJK / 思源）里覆盖
#     尚可，扩展 B 已明显依赖厂商字体，扩展 C+ 基本是 tofu 重灾区。**收录≠可见**，
#     打出来是空白框时用户只会认为是输入法的 bug。
#   升级路径：往 CJK_RANGES 里加 (0x2A700, 0x2B73F) 等区间即可，两道门禁会跟着验收。
#
# 注意：收录范围只影响**词库收录**。Composer / Pinyin 切分层不涉及码位范围，
# `.opid` 与 Trie 均以 char（码位）为键，扩展 B 的四字节 UTF-8 / 代理对对其无感。
CJK_RANGES: tuple[tuple[int, int], ...] = (
    (0x3400, 0x4DBF),    # CJK 扩展 A
    (0x4E00, 0x9FFF),    # CJK 基本区
    (0x20000, 0x2A6DF),  # CJK 扩展 B
)


def is_cjk(ch: str) -> bool:
    """字符是否在收录范围内。按码位判定：Python str 是码位序列，代理对不成问题。"""
    cp = ord(ch)
    return any(lo <= cp <= hi for lo, hi in CJK_RANGES)

# ---- 上游 pin（2026-09-26）----
# 钉死不动的版本，不再用 UCD/latest：latest 会在 Unicode 发布新版时悄悄换掉整份
# Unihan（2026-09 当下 latest = 18.0.0），产物随之无声变化、又无法复现。
# 升级路径（有意升级时唯一要改的地方）：把下面两行改到新的版本目录 + 新的 zip SHA，
# 然后 `python3 scripts/gen_trad_dict.py && python3 scripts/gen_luna_dict.py > ...`
# 重编产物并重跑门禁 `cargo test -p opi-tools --test ranking_quality`（排序变化会红）。
# 代价：不再自动获得上游修正（新增字、词次修订、读音订正都要手动升级才会进来）。
UNIHAN_VERSION = "18.0.0"
UNIHAN_ZIP_URL = f"https://www.unicode.org/Public/{UNIHAN_VERSION}/ucd/Unihan.zip"
# 本次 pin 时实际取到的 zip 摘要（sha256）。校验失败 = URL 内容变了，宁可报错也不静默换数据。
UNIHAN_ZIP_SHA256 = "4c93ea9c1f636451729a840978f1667a53886af37ba854fdcce109721c63d43e"

_ZIP: dict[str, str] = {}


def unihan_member(member: str) -> str:
    """下载 Unihan.zip（进程内缓存，校验 pin 的 SHA-256）并解出成员文本。"""
    if member not in _ZIP:
        with urllib.request.urlopen(UNIHAN_ZIP_URL, timeout=180) as r:
            data = r.read()
        got = hashlib.sha256(data).hexdigest()
        if got != UNIHAN_ZIP_SHA256:
            raise RuntimeError(
                f"Unihan.zip 摘要与 pin 不符：期望 {UNIHAN_ZIP_SHA256}，实得 {got}。"
                f"上游内容变了，请确认后更新 scripts/hanzi_freq.py 的 pin 并重跑门禁。"
            )
        with zipfile.ZipFile(io.BytesIO(data)) as zf:
            _ZIP[member] = zf.read(member).decode("utf-8")
    return _ZIP[member]


def normalize(py: str) -> str:
    """带调/数字调/ü 冒号拼音 → 无调、ü→v、小写。
    关键顺序：ü 家族（ü/ǖ/ǘ/ǚ/ǜ）与 "u:" 必须在 NFD 之前映射 v——
    NFD 后组合音调符已被剥离，无法再区分 u/ü（nǚ→nu 而非 nv）。"""
    py = py.replace("u:", "v")
    py = re.sub(r"[üǖǘǚǜ]", "v", py)
    py = unicodedata.normalize("NFD", py)
    py = "".join(c for c in py if not unicodedata.combining(c))
    return re.sub(r"[0-9]", "", py).lower()


def load_khanyupinlu(text: str) -> dict[str, dict[str, int]]:
    """Unihan_Readings.txt → {字: {读音: 词次}}，如 好 → {"hao": 6317}。
    值形如 `hǎo(6060) hāo(142) hào(115)`；**同一声母韵母的不同声调相加**——
    输入法查询是无调拼音（好 hao 下三个声调都算 "hao" 的候选），故按无调音节汇总，
    如 往 wǎng(856)+wàng(427)=1283。仅收 CJK_RANGES 内的码位。
    注：kHanyuPinlu 只覆盖 3799 字，**全部在基本区**（扩展 A/B 一字无词次），
    故放宽范围对本表无实际影响——放宽是为了两侧口径一致，避免以后再踩同一个坑。"""
    out: dict[str, dict[str, int]] = {}
    for line in text.splitlines():
        if not line.startswith("U+"):
            continue
        parts = line.split("\t")
        if len(parts) < 3 or parts[1] != "kHanyuPinlu":
            continue
        cp = int(parts[0][2:], 16)
        if not any(lo <= cp <= hi for lo, hi in CJK_RANGES):
            continue
        counts = out.setdefault(chr(cp), {})
        for py, n in re.findall(r"([^\s()]+)\((\d+)\)", parts[2]):
            norm = normalize(py)
            if norm:
                counts[norm] = counts.get(norm, 0) + int(n)
    return out


def load_kmandarin(text: str) -> dict[str, list[str]]:
    """Unihan_Readings.txt → {字: [规范化无调拼音...]}（kMandarin 多音空格分隔，去重）。
    kMandarin 覆盖全部收录范围（基本区 20924 + 扩展 A 5787 + 扩展 B 14618）。"""
    km: dict[str, list[str]] = {}
    for line in text.splitlines():
        if not line.startswith("U+"):
            continue
        parts = line.split("\t")
        if len(parts) < 3 or parts[1] != "kMandarin":
            continue
        cp = int(parts[0][2:], 16)
        if not any(lo <= cp <= hi for lo, hi in CJK_RANGES):
            continue
        readings: list[str] = []
        for py in parts[2].strip().split():
            norm = normalize(py)
            if norm and norm not in readings:
                readings.append(norm)
        if readings:
            km[chr(cp)] = readings
    return km


def load_variants(text: str) -> tuple[dict[str, list[str]], dict[str, list[str]]]:
    """Unihan_Variants.txt → (trad_of, simp_of)：
    trad_of[简] = [繁...]（kTraditionalVariant），simp_of[繁] = [简...]（kSimplifiedVariant）。
    值里可能带 `<kMeyerWempe` 来源后缀，按 `<` 截断；列表含自身条目（Unihan 记录
    「也可按原样使用」），调用方需自行 `!= ch` 过滤。"""
    trad_of: dict[str, list[str]] = {}
    simp_of: dict[str, list[str]] = {}
    for line in text.splitlines():
        if not line.startswith("U+"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        ch = chr(int(parts[0][2:], 16))
        vals = [
            chr(int(x.split("<")[0][2:], 16))
            for x in parts[2].split()
            if x.split("<")[0].startswith("U+")
        ]
        if parts[1] == "kTraditionalVariant":
            trad_of[ch] = vals
        elif parts[1] == "kSimplifiedVariant":
            simp_of[ch] = vals
    return trad_of, simp_of


_LEVELS: tuple[list[str], list[str]] | None = None


def gb2312_levels() -> tuple[list[str], list[str]]:
    """GB2312 汉字区 B0–F7 行 × A1–FE 列 → (一级, 二级)。
    row ≤ 0xD7 为一级（拼音序）、其余二级（部首序）；D7FA–D7FE 未定义，显式跳过。"""
    global _LEVELS
    if _LEVELS is None:
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
        _LEVELS = (first, second)
    return _LEVELS


_SEGPOS: dict[str, tuple[int, int]] = {}


def segpos(ch: str) -> tuple[int, int]:
    """段位 (段, 段内序)：0 = GB 一级（拼音序）、1 = GB 二级（部首序）、
    2 = 其余（码位序）。段序保留原设计（常用字先于生僻字），但降为同频 tiebreak。"""
    if not _SEGPOS:
        first, second = gb2312_levels()
        _SEGPOS.update({c: (0, i) for i, c in enumerate(first)})
        _SEGPOS.update({c: (1, i) for i, c in enumerate(second)})
    return _SEGPOS.get(ch, (2, ord(ch)))
