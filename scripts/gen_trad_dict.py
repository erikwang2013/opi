#!/usr/bin/env python3
"""生成简繁字库 TSV 数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，word\tpinyin\tfreq 三列）：
  data/raw/trad_hanzi.tsv     GB2312 一二级全量 6763 字 + terra 单字 + 其余 Unihan 单字
  data/raw/trad_phrases.tsv   人工常用词组 + terra 繁体词组

词频（统一常用度排序，spec 2026-08-15 验收偏差 #4）：
  freq = FMAX - idx * (FMAX // N)，idx 为排序键排完后的行序。排序键（同拼音前缀组内）：
  [有词次 > 无词次] → [词次降序] → [字形取向] → [变体同门位次] → [GB 段位] → [段内序]
  → [字码位] → [读音]。
  词次来自 Unihan kHanyuPinlu（逐读音语料词次，见 scripts/hanzi_freq.py）。
  字形取向：候选字形及其简繁变体中，terra 语料出现更多者优先——消解 只/戠（戠 经
  kSimplifiedVariant 继承 只 的词次）、发/髮、出/齣 这类同频不同形的选择；
  随后按 Unihan 变体列表位次（為 0 / 爲 1、裡 0 / 裏 1）定序。
  GB 段位（一级 3755 → 二级 3008 → 其余 Unihan）**保留**为同频/缺频兜底序——
  kHanyuPinlu 只覆盖 3799 字，其余字仍按「常用字先、生僻字后」排，只是不再当主键。
  拼音规范：kMandarin 去调号（NFD 剥离组合音调符）→ ü(冒号)→v → 小写。

收录范围（2026-09-26 放开扩展区）：
  此前单字与词组都按 `0x4E00..=0x9FFF` 过滤，扩展 A（U+3400–4DBF）与扩展 B
  （U+20000–2A6DF）整段丢弃 —— 词库根本没有这些字，翻多少页候选也翻不出来。
  现按 hanzi_freq.CJK_RANGES 收录到扩展 B 为止（扩展 C+ 有意不收，理由见该处注释）。
  代价（本次实测，见 LICENSES.md「收录范围」节）：单字 24070 → 46757 行（+22687）、
  词组 49645 → 49672 行（+27），trad.opid 73715 → 96429 条、2.10MB → 2.58MB（+22%）；
  `opi-tools verify` 装载 18.0 → 23.5ms（新旧交错各 9 次取最小；耗时与条目数成正比）。
  **局限：收录 ≠ 可见。** 扩展 B 依赖设备字体，各厂商覆盖差异很大（扩展 A 尚可，
  扩展 B 常见 tofu 空白框）。打出来是空白框，用户只会认为是输入法 bug ——
  这一点写在这里是为了让下一个人改数据范围前先想到字体。
  排序影响：扩展区字无 kHanyuPinlu 词次，全部落入「无词次」段（段位 2、码位序），
  仍排在 GB2312 与常用字之后；既有候选顺序不变，只有 freq 间距因行数增加而变密。

人工表退役记录（保留此节，勿静默删除——它记录的是当时的判断与代价）：
  * COMMON_TRAD（约 120 个繁体常用字白名单）：旧排序主体是 GB 码序，靠白名单把
    發/國/學/號/好 等抬到同音组首位。退役原因：kHanyuPinlu 逐读音词次直接给出了
    这个信息（好 hao 6317 本就是 "hao" 组最高，發 fa 1511 也是），白名单覆盖 120 字
    却让其余 9000+ 字继续按码序错排（wo→蜗、shi→匙、jian→見 第 243 位）。
  * COMMON_TRAD 末尾的 8 个 GB 常用字（好/号/豪/毫/发/法/乏/伐）：为修正 GB 码序内
    同音排序而追加的补丁，随词次排序一并退役（好 6317 > 号 490 > 毫 224 > 豪 165）。
  * SUPPLEMENT_PHRASES（人工常用词组）**保留**：terra 缺 臺灣/電話/謝謝 等常用词，
    且词组段没有词次数据可依，仍按「人工在前、terra 文件序在后」排列。

用法（需要网络：Unihan.zip + terra_pinyin.dict.yaml）：cd <repo 根> && python3 scripts/gen_trad_dict.py
"""
import hashlib
import os
import sys
import urllib.request

from hanzi_freq import (
    FMAX,
    gb2312_levels,
    is_cjk,
    load_kmandarin,
    load_khanyupinlu,
    load_variants,
    normalize,
    segpos,
    unihan_member,
)

# ---- 上游 pin（2026-09-26）----
# 钉死到 commit SHA，不再用 master：master 已经漂过一次（2026-08-15 → 09-26 之间
# 删了「怎麽」、把「怎麼搞的」的拼音 zemegaode 改成 zenmegaode），产物跟着变且不可复现。
# 升级路径（有意升级时唯一要改的地方）：取新的 commit SHA 换掉下面一行 +
# 换掉 SHA256（`curl -sSL <url> | sha256sum`），重跑
# `python3 scripts/gen_trad_dict.py` 与 `python3 scripts/gen_luna_dict.py > <tsv>`，
# 再用 `cargo test -p opi-tools --test ranking_quality` 与 trad_coverage 验收数据变化。
# 代价：不再自动获得上游修正（新词、拼音订正、删词都要手动升级才会进来）。
TERRA_COMMIT = "723e51bc266cf9464530c1ddedb856aa18e3da34"
TERRA_URL = (
    f"https://raw.githubusercontent.com/rime/rime-terra-pinyin/{TERRA_COMMIT}"
    "/terra_pinyin.dict.yaml"
)
# 本次 pin 时实际取到的 yaml 摘要（sha256）。校验失败 = URL 内容变了，宁可报错不静默换数据。
TERRA_SHA256 = "2f881a239e09a61e5993e79257655f98f81c3789dc8aeea5f33477f710345b27"

# 人工常用繁体词组（terra 缺失的常见词；拼音为全拼无空格连写、无撇号）。
SUPPLEMENT_PHRASES: dict[str, str] = {
    "臺灣": "taiwan", "中華民國": "zhonghuaminguo", "中國": "zhongguo",
    "香港": "xianggang", "澳門": "aomen", "中文": "zhongwen",
    "電腦": "diannao", "網路": "wanglu", "軟體": "ruanti", "資料": "ziliao",
    "電話": "dianhua", "手機": "shouji", "銀行": "yinhang", "問題": "wenti",
    "什麼": "shenme", "為什麼": "weishenme", "謝謝": "xiexie",
    "對不起": "duibuqi", "沒關係": "meiguanxi", "歡迎": "huanying",
    "再見": "zaijian", "早安": "zaoan", "學校": "xuexiao", "老師": "laoshi",
    "學生": "xuesheng", "工作": "gongzuo", "朋友": "pengyou", "家人": "jiaren",
    "結婚": "jiehun", "離婚": "lihun", "經濟": "jingji", "政治": "zhengzhi",
    "歷史": "lishi", "文化": "wenhua", "藝術": "yishu", "音樂": "yinyue",
    "電影": "dianying", "照片": "zhaopian", "風景": "fengjing", "天氣": "tianqi",
    "身體": "shenti", "健康": "jiankang", "醫院": "yiyuan", "醫生": "yisheng",
    "護士": "hushi", "購物": "gouwu", "商店": "shangdian", "市場": "shichang",
    "價格": "jiage", "便宜": "pianyi", "免費": "mianfei", "我們": "women",
    "你們": "nimen", "他們": "tamen", "已經": "yijing", "應該": "yinggai",
    "還有": "haiyou", "所有": "suoyou", "重要": "zhongyao", "知道": "zhidao",
    "喜歡": "xihuan", "愛情": "aiqing", "關係": "guanxi", "發展": "fazhan",
    "發現": "faxian", "發生": "fasheng", "出發": "chufa", "頭髮": "toufa",
    "汽車": "qiche", "火車": "huoche", "飛機": "feiji", "廁所": "cesuo",
}

# 缺 kMandarin 的码位人工补音（脚本报缺后在此补齐再重跑）
SUPPLEMENT: dict[str, list[str]] = {}


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=180) as r:
        return r.read()


def parse_terra(text: str) -> list[tuple[str, str]]:
    """terra_pinyin.dict.yaml → [(word, 无调全拼连写)]，保持文件序。"""
    out = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "\t" not in line:
            continue
        cols = line.split("\t")
        if len(cols) < 2 or len(cols) > 3:
            continue
        word, pinyin = cols[0].strip(), cols[1].strip()
        if not word or not all(is_cjk(c) for c in word):
            continue
        py = "".join(normalize(s) for s in pinyin.split())
        if not py.isascii() or len(py) > 255:
            continue
        out.append((word, py))
    return out


def write_atomic(path: str, lines: list[str]) -> None:
    """先写 .tmp 再 os.replace：中途崩溃不留半截 TSV（旧版直接覆盖写）。"""
    tmp = f"{path}.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    os.replace(tmp, path)


def main() -> None:
    first, second = gb2312_levels()
    if len(first) != 3755 or len(second) != 3008:
        print(f"FATAL: GB2312 一级 {len(first)}/3755、二级 {len(second)}/3008 不符", file=sys.stderr)
        sys.exit(1)
    gb = first + second

    readings_txt = unihan_member("Unihan_Readings.txt")
    km = load_kmandarin(readings_txt)
    kph = load_khanyupinlu(readings_txt)
    trad_of, simp_of = load_variants(unihan_member("Unihan_Variants.txt"))

    missing = [ch for ch in gb if ch not in km and ch not in SUPPLEMENT]
    if missing:
        print(f"FATAL: {len(missing)} 个 GB2312 码位缺 kMandarin：{' '.join(missing)}", file=sys.stderr)
        print("补进脚本 SUPPLEMENT 表后重跑。", file=sys.stderr)
        sys.exit(1)

    terra_raw = fetch(TERRA_URL)
    got = hashlib.sha256(terra_raw).hexdigest()
    if got != TERRA_SHA256:
        raise RuntimeError(
            f"terra_pinyin.dict.yaml 摘要与 pin 不符：期望 {TERRA_SHA256}，实得 {got}。"
            f"上游内容变了，请确认后更新 scripts/gen_trad_dict.py 的 pin 并重跑门禁。"
        )
    terra = parse_terra(terra_raw.decode("utf-8"))
    terra_single = [(w, py) for w, py in terra if len(w) == 1]
    terra_phrase = [(w, py) for w, py in terra if len(w) > 1]

    # terra 语料字形偏好：繁体字形在 terra 词条里出现越多，越可能是「实际在用」而非
    # 罕见专用变体（爲 601 vs 為 2、裏 185 vs 裡 1、髮 237 vs 發 164 都是 terra 习惯问题，
    # 故只在词次打平后生效，见 key_of）。
    terra_cnt: dict[str, int] = {}
    for w, _py in terra:
        for ch_in_w in w:
            terra_cnt[ch_in_w] = terra_cnt.get(ch_in_w, 0) + 1

    def cov(ch: str, py: str) -> int:
        """词次：自身 kHanyuPinlu，叠加其 kSimplifiedVariant 简体字的同读音词次。
        繁体专用字（裏/髮/為）自身常无 kHanyuPinlu，靠简体对应字（里/发/为）的词次继承——
        输入 "li/fa/wei" 的人要的就是这些繁体字形。"""
        best = (kph.get(ch) or {}).get(py, 0)
        for v in simp_of.get(ch, []):
            best = max(best, (kph.get(v) or {}).get(py, 0))
        return best

    def shape_pref(ch: str) -> int:
        """字形取向：1 = 该字形在其「简繁同门」（非自身变体）中最常用，0 = 否。
        同门无从比较（无变体）也为 1——不做无依据的贬低。"""
        rivals = {v for v in trad_of.get(ch, []) if v != ch}
        rivals |= {v for v in simp_of.get(ch, []) if v != ch}
        if not rivals:
            return 1
        return 1 if terra_cnt.get(ch, 0) > max(terra_cnt.get(r, 0) for r in rivals) else 0

    def sib_pos(ch: str) -> int:
        """该字形在其简体对应字的 kTraditionalVariant 列表中的位次（0 起，越小越优先）。
        Unihan 列表首位是权威写法（為 0/爲 1、裡 0/裏 1）；terra 是港式语料，
        会把 裏/爲 这种异体抬上来，故排在 terra 取向之后、"段位"之前。"""
        best = 99
        for s in simp_of.get(ch, []):
            if s == ch:
                continue
            lst = trad_of.get(s, [])
            if ch in lst:
                best = min(best, lst.index(ch))
        return best

    def key_of(ch: str, py: str) -> tuple[int, int, int, int, int, int, int, str]:
        freq = cov(ch, py)
        seg, pos = segpos(ch)
        return (
            0 if freq else 1,
            -freq,
            -shape_pref(ch),
            sib_pos(ch),
            seg,
            pos,
            ord(ch),
            py,
        )

    # 单字全集：GB2312 全量 ∪ terra 单字 ∪ 其余 Unihan（同旧版）；排序全交给 key_of，
    # 段位只在同频/缺频时兜底，不再按段拼装顺序。
    pairs: set[tuple[str, str]] = {(ch, py) for ch, pys in km.items() for py in pys}
    for w, py in terra_single:
        pairs.add((w, py))
    for ch in SUPPLEMENT:
        for py in SUPPLEMENT[ch]:
            pairs.add((ch, py))
    ordered: list[tuple[str, str]] = sorted(pairs, key=lambda cp: key_of(*cp))

    # 词组段：人工常用 → terra（文件序）；单字之后，无词次数据
    phrase_ordered: list[tuple[str, str]] = []
    phrase_seen: set[str] = set()
    for w, py in SUPPLEMENT_PHRASES.items():
        phrase_ordered.append((w, py))
        phrase_seen.add(w)
    for w, py in terra_phrase:
        if w in phrase_seen:
            continue
        phrase_ordered.append((w, py))
        phrase_seen.add(w)

    n = len(ordered) + len(phrase_ordered)
    assert n < FMAX, "行数超过 FMAX，spacing 归零，freq 全等"
    spacing = FMAX // n
    hanzi: list[str] = [
        f"{word}\t{py}\t{FMAX - idx * spacing}"
        for idx, (word, py) in enumerate(ordered)
    ]
    phrases_out: list[str] = [
        f"{word}\t{py}\t{FMAX - (len(ordered) + idx) * spacing}"
        for idx, (word, py) in enumerate(phrase_ordered)
    ]

    write_atomic("data/raw/trad_hanzi.tsv", hanzi)
    write_atomic("data/raw/trad_phrases.tsv", phrases_out)

    print(f"GB2312 单字: {len(gb)} (一级 {len(first)}, 二级 {len(second)})")
    print(f"繁体单字(含 terra 单字): {len({ch for ch, _ in ordered}) - len(gb)}")
    print(f"Unihan 词次表: {len(kph)} 字（其余字按段位序排）")
    print(f"词组(人工+terra): {len(phrase_ordered)}")
    print(f"trad_hanzi.tsv 行数: {len(hanzi)}")
    print(f"trad_phrases.tsv 行数: {len(phrases_out)}")
    print(f"freq 范围: [{FMAX - (n - 1) * spacing}, {FMAX}] spacing={spacing}")


if __name__ == "__main__":
    main()
