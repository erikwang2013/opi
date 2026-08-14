#!/usr/bin/env python3
"""生成简繁字库 TSV 数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，word\tpinyin\tfreq 三列）：
  data/raw/trad_hanzi.tsv     GB2312 一二级全量 6763 字（GB 码序）+ 常用繁体单字（Unihan 码序）
  data/raw/trad_phrases.tsv   rime terra-pinyin 繁体词组（单字词组并入 hanzi 带）

词频带：繁体单字 [9900,9001]、GB 一级 [9000,5246]、GB 二级 [6000,2993]、
多字词组 [8800,8001]。拼音规范：kMandarin 去调号、ü(冒号)→v。

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

    现行 Unihan kMandarin 为带调元音（ā 等，无调号数字），先做 NFD 拆字后删除组合
    符号实现去调号；ü（含带调 ü：ǖǘǚǜ）与历史 "u:" 写法统一映射为 v。"""
    py = py.replace(":", "v")
    for u in "üǖǘǚǜ":
        py = py.replace(u, "v")
    py = "".join(c for c in unicodedata.normalize("NFD", py) if not unicodedata.combining(c))
    return re.sub(r"[0-9]", "", py).lower()


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


def band_freq(total: int, idx: int, top: int, bottom: int) -> int:
    """序号 → 带内 freq：idx∈[0,total) 线性压缩到 [bottom, top]，正数且带内有序。"""
    return top - idx * (top - bottom) // total


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

    level1_count = sum(1 for r, _ in rows if r <= 0xD7)
    hanzi: list[str] = []
    for i, (row, ch) in enumerate(rows):
        readings = SUPPLEMENT.get(ch) or km.get(ch)
        assert readings
        seq = i if row <= 0xD7 else i - level1_count
        freq = 9000 - seq if row <= 0xD7 else 6000 - seq
        for py in readings:
            hanzi.append(f"{ch}\t{py}\t{freq}")

    gb_set = {ch for _, ch in rows}
    trad = []
    singles = sorted((ch, r) for ch, r in km.items() if ch not in gb_set)
    for i, (ch, readings) in enumerate(singles):
        freq = band_freq(len(singles), i, 9900, 9001)
        for py in readings:
            trad.append(f"{ch}\t{py}\t{freq}")

    phrases = []
    for i, line in enumerate(fetch(TERRA_URL).decode("utf-8").splitlines()):
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
        phrases.append((word, py))

    # 单字词组（len==1）并入繁体单字带；多字词组用 8800 带，按文件序压缩
    single_phrase = [p for p in phrases if len(p[0]) == 1]
    multi_phrase = [p for p in phrases if len(p[0]) > 1]
    for idx, (word, py) in enumerate(single_phrase):
        trad.append(f"{word}\t{py}\t{band_freq(max(len(single_phrase), 1), idx, 9900, 9001)}")
    phrases_out = []
    for idx, (word, py) in enumerate(multi_phrase):
        phrases_out.append(f"{word}\t{py}\t{band_freq(len(multi_phrase), idx, 8800, 8001)}")

    with open("data/raw/trad_phrases.tsv", "w", encoding="utf-8") as f:
        f.write("\n".join(phrases_out) + "\n")
    with open("data/raw/trad_hanzi.tsv", "w", encoding="utf-8") as f:
        f.write("\n".join(hanzi + trad) + "\n")

    print(f"GB2312 单字: {len(rows)} (一级 {level1_count}, 二级 {len(rows) - level1_count})")
    print(f"繁体单字(含单字词组): {len(trad)}")
    print(f"多字词组: {len(phrases_out)}")
    print(f"trad_hanzi.tsv 行数: {len(hanzi) + len(trad)}")
    print(f"trad_phrases.tsv 行数: {len(phrases_out)}")


if __name__ == "__main__":
    main()
