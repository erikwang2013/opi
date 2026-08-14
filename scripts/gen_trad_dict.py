#!/usr/bin/env python3
"""生成简繁字库 TSV 数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，word\tpinyin\tfreq 三列）：
  data/raw/trad_hanzi.tsv     GB2312 一二级全量 6763 字（GB 码序）+ 常用繁体单字
  data/raw/trad_phrases.tsv   rime terra-pinyin 繁体词组 + 人工常用词组

词频（统一常用度排序，见 plan Task 1 偏差注记 #2）：
  freq = FMAX - idx * (FMAX // N)，idx 为全部行按优先段排列的序号：
  [COMMON_TRAD 常用繁体单字] → [GB 一级（GB 码序）] → [GB 二级]
  → [terra 单字（文件序，去重）] → [其余 Unihan 单字（码位序）]
  → [SUPPLEMENT_PHRASES 人工词组] → [terra 词组（文件序，去重）]。
  GB 字默认在 GB 段；少数 GB 常用字（好/号/豪/毫、发/法/乏/伐）经 COMMON_TRAD
  前置修正 GB 码序内同音排序；常用字必然排在同音生僻字前（fa→發/髮、hao→號/好）。
拼音规范：kMandarin 去调号（NFD 剥离组合音调符）→ ü(冒号)→v → 小写。

用法（需要网络）：cd <repo 根> && python3 scripts/gen_trad_dict.py
"""
import codecs
import io
import re
import sys
import unicodedata
import urllib.request
import zipfile

# Unihan 自 2025 起以 Unihan.zip 发布（单文件 URL 404）
UNIHAN_ZIP_URL = "https://www.unicode.org/Public/UCD/latest/ucd/Unihan.zip"
TERRA_URL = "https://raw.githubusercontent.com/rime/rime-terra-pinyin/master/terra_pinyin.dict.yaml"

# 4e9 < u32::MAX；parse_dict 接受 u32 freq，引擎按 u64 比较
FMAX = 4_000_000_000

# 人工常用单字（按常用度降序；GB 码序/terra 文件序都非字内常用度序——
# 末尾追加的 GB 常用字 好/号/豪/毫、发/法/乏/伐 用于修正同音排序：
# 实测 GB 码序 hao 组为 壕嚎豪毫郝好耗号浩（好第 7），terra 序为 号嚎壕好…（好第 4），
# 均不满足验收「hao→好前二」，故人工前置。GB 段循环对已见字跳过，不重复）。
COMMON_TRAD: list[str] = [
    "發", "髮", "臺", "灣", "國", "學", "個", "們", "時", "間",
    "現", "這", "點", "鐘", "機", "話", "電", "腦",
    "網", "軟", "體", "資", "郵", "銀", "錢", "愛",
    "說", "問", "題", "聽", "覺", "錯", "誤", "對", "謝", "請",
    "幫", "應", "該", "經", "濟", "歷", "實", "際", "場",
    "邊", "處", "樓", "橋", "車", "輪", "數", "據", "檢", "測",
    "試", "驗", "認", "識", "讓", "讀", "寫", "講", "館", "報",
    "紙", "雜", "誌", "書", "習", "醫", "藥", "樂", "圖", "視",
    "劇", "幣", "鈔", "島", "縣", "鎮", "鄉", "區", "號", "碼",
    "頁", "項", "條", "費", "價", "減", "漲", "虧", "賺", "還",
    "買", "賣", "貴", "賤", "舊", "壞", "髒", "亂", "悶", "熱",
    "溫", "濕", "淨", "藍", "綠", "黃", "紅", "遠", "淺",
    "細", "長", "寬", "圓", "狀", "樣", "類",
    # 以下为 GB 常用字（修正 GB 码序内同音排序；见段注释）
    "好", "号", "豪", "毫", "发", "法", "乏", "伐",
]

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


def fetch_unihan_readings() -> str:
    """Unihan.zip → 内存解压 Unihan_Readings.txt（zip 根目录内该单文件）。"""
    data = fetch(UNIHAN_ZIP_URL)
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        return zf.read("Unihan_Readings.txt").decode("utf-8")


def normalize(py: str) -> str:
    """kMandarin/terra 拼音 → 无调、ü→v、小写。
    当前 UCD kMandarin 用调号（ā/fā/nǚ），NFD 剥离组合音调符；
    旧式数字调号（fa1）与 ü 冒号（lu:4→lv）一并处理。"""
    py = unicodedata.normalize("NFD", py)
    py = "".join(c for c in py if not unicodedata.combining(c))
    return re.sub(r"[0-9]", "", py).replace(":", "v").lower()


def load_kmandarin(text: str) -> dict[str, list[str]]:
    """Unihan_Readings.txt → {char: [规范化无调拼音...]}（kMandarin 多音空格分隔，去重）。"""
    km: dict[str, list[str]] = {}
    for line in text.splitlines():
        if not line.startswith("U+"):
            continue
        parts = line.split("\t")
        if len(parts) < 3 or parts[1] != "kMandarin":
            continue
        cp = int(parts[0][2:], 16)
        if not (0x4E00 <= cp <= 0x9FFF):
            continue
        readings = []
        for py in parts[2].strip().split():
            norm = normalize(py)
            if norm and norm not in readings:
                readings.append(norm)
        if readings:
            km[chr(cp)] = readings
    return km


def gb2312_rows() -> list[tuple[int, str]]:
    """GB2312 汉字区 B0–F7 行 × A1–FE 列，GB 码序；row ≤ 0xD7 为一级（拼音序），其余二级（部首序）。
    D7FA–D7FE 在 GB2312-80 未定义（GBK 才定义），显式跳过。"""
    out = []
    for row in range(0xB0, 0xF8):
        for col in range(0xA1, 0xFF):
            if row == 0xD7 and col >= 0xFA:
                continue
            try:
                ch = codecs.decode(bytes([row, col]), "gb2312")
            except UnicodeDecodeError:
                continue
            out.append((row, ch))
    return out


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
        if not word or not all(0x4E00 <= ord(c) <= 0x9FFF for c in word):
            continue
        py = "".join(normalize(s) for s in pinyin.split())
        if not py.isascii() or len(py) > 255:
            continue
        out.append((word, py))
    return out


def main() -> None:
    km = load_kmandarin(fetch_unihan_readings())
    rows = gb2312_rows()
    if len(rows) != 6763:
        print(f"FATAL: GB2312 单字 {len(rows)} ≠ 6763（Python gb2312 codec 覆盖与预期不符）", file=sys.stderr)
        sys.exit(1)

    missing = [ch for _, ch in rows if ch not in km and ch not in SUPPLEMENT]
    if missing:
        print(f"FATAL: {len(missing)} 个 GB2312 码位缺 kMandarin：{' '.join(missing)}", file=sys.stderr)
        print("补进脚本 SUPPLEMENT 表后重跑。", file=sys.stderr)
        sys.exit(1)

    gb_set = {ch for _, ch in rows}
    gb_in_common = [ch for ch in COMMON_TRAD if ch in gb_set]
    if gb_in_common:
        # GB 字允许进 COMMON_TRAD（用于修正 GB 码序内同音排序）；GB 段循环对已见字
        # 跳过，每字只出现一次。此处仅提示，不阻断。
        print(f"NOTE: COMMON_TRAD 含 {len(gb_in_common)} 个 GB2312 字（置常用段、GB 段跳过）：{' '.join(gb_in_common)}")
    no_km = [ch for ch in COMMON_TRAD if ch not in km and ch not in SUPPLEMENT]
    if no_km:
        print(f"FATAL: COMMON_TRAD 缺 kMandarin（补进 SUPPLEMENT）：{' '.join(no_km)}", file=sys.stderr)
        sys.exit(1)

    terra = parse_terra(fetch(TERRA_URL).decode("utf-8"))
    terra_single = [(w, py) for w, py in terra if len(w) == 1]
    terra_phrase = [(w, py) for w, py in terra if len(w) > 1]

    # 段 1–5 单字：常用繁体 → GB 一二级（GB 码序）→ terra 单字（文件序）→ 其余 Unihan（码位序）
    ordered: list[tuple[str, list[str]]] = []
    seen: set[str] = set()
    for ch in COMMON_TRAD:
        readings = SUPPLEMENT.get(ch) or km[ch]
        ordered.append((ch, readings))
        seen.add(ch)
    for _, ch in rows:
        if ch in seen:
            continue
        readings = SUPPLEMENT.get(ch) or km[ch]
        ordered.append((ch, readings))
        seen.add(ch)
    for w, py in terra_single:
        if w in seen:
            continue
        seen.add(w)
        readings = [py] + [p for p in (km.get(w) or []) if p != py]
        ordered.append((w, readings))
    for ch in sorted(km):
        if ch in seen:
            continue
        seen.add(ch)
        ordered.append((ch, km[ch]))

    # 段 6–7 词组：人工常用 → terra（文件序）
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
    spacing = FMAX // n
    hanzi: list[str] = []
    for idx, (word, readings) in enumerate(ordered):
        freq = FMAX - idx * spacing
        for py in readings:
            hanzi.append(f"{word}\t{py}\t{freq}")
    phrases_out: list[str] = []
    for idx, (word, py) in enumerate(phrase_ordered):
        freq = FMAX - (len(ordered) + idx) * spacing
        phrases_out.append(f"{word}\t{py}\t{freq}")

    with open("data/raw/trad_hanzi.tsv", "w", encoding="utf-8") as f:
        f.write("\n".join(hanzi) + "\n")
    with open("data/raw/trad_phrases.tsv", "w", encoding="utf-8") as f:
        f.write("\n".join(phrases_out) + "\n")

    print(f"GB2312 单字: {len(rows)} (一级 {sum(1 for r, _ in rows if r <= 0xD7)}, 二级 {len(rows) - sum(1 for r, _ in rows if r <= 0xD7)})")
    print(f"繁体单字(含 terra 单字): {len(ordered) - len(rows)}")
    print(f"词组(人工+terra): {len(phrase_ordered)}")
    print(f"trad_hanzi.tsv 行数: {len(hanzi)}")
    print(f"trad_phrases.tsv 行数: {len(phrases_out)}")
    print(f"freq 范围: [{FMAX - (n - 1) * spacing}, {FMAX}] spacing={spacing}")


if __name__ == "__main__":
    main()
