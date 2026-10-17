#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成表情与颜文字数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，无表头，制表符分隔）：
  data/raw/emoji.tsv               text \\t name(英文小写) \\t 中文组名
  data/raw/kaomoji.tsv             text \\t 中文分类
  android/app/src/main/assets/kaomoji.tsv   同上，逐字节相同（同一次运行、同一内容）

`emoji.tsv` 的消费侧是 engine-core/src/symbols.rs 的 `builtin_emoji_texts` /
`builtin_emoji_groups`：按第 3 列**连续段**切组，所以同组条目必须连续；组序固定为
笑脸 → 人物 → 动物 → 植物 → 食物 → 旅行 → 活动 → 物品 → 符号（笑脸最前）。
`kaomoji.tsv` 同时写 `data/raw/`（真源）与 Android 资产（随包分发），两份逐字节相同。

## 数据来源 / provenance

* OpenMoji：https://github.com/hfg-gmuend/openmoji 的 `data/openmoji.json`，
  许可证 **CC BY-SA 4.0**（见 data/raw/LICENSES.md）。默认读取
  `/tmp/emoji-src/openmoji.json`（可用 `--openmoji` 或 `OPENMOJI_JSON` 覆盖）。
* 颜文字：https://github.com/aoguai/rime_kaomoji_dict @ commit
  `2f10ddca83f4cd87f41672442d27943f733b488f`（MIT，Copyright (c) 2023 aoguai），
  `data/` 下 5 份素材。默认读取 `/tmp/kaomoji-src2/data`（可用 `--kaomoji-dir`
  或 `KAOMOJI_DIR` 覆盖）。

两份源都钉了 SHA-256：拿到的文件不符就**大声失败**，绝不静默换数据。升级上游时
先核对内容再更新下面的常量。

## 筛选规则（为什么是这些条）

OpenMoji 全量 4565 条里，`flags`/`component`/`extras-*` 整组不要；肤色变体
（`skintone` 非空）不要；含 ZWJ（U+200D）的组合序列整类不要 —— 家庭/职业/情侣
这类多码位组合在输入法面板里格宽不一、字体覆盖也参差，正是「常用」之外的部分。
剩下的按 `subgroups` 逐个设上限（见 `EMOJI_SUBGROUP_CAP`），子类内按首码位
升序取前 N（早期码位 ≈ 更早进入 Unicode ≈ 更普及；这是**代理**）；常见但码位
靠后的条目由 `EMOJI_MUST_HAVE` 白名单兜住。上限是**人工裁剪**，不是词频 ——
没有可离线复算的常用度数据，宁可承认这一点，也不假装精确。全表目标 400–700 条。
子类上限/白名单/分类关键词表在 `scripts/emoji_kaomoji_rules.py`（纯数据；拆出去
的原因同 `symbol_keywords.py`：本文件贴着仓库的 500 行门禁）。

颜文字从 5 份素材合并、按清洗后的文本去重（源内大量重复），优先保留 lmeee /
sougou / A 岛的精选列表，再用 custom_phrase / Temreg 补足；分类先看素材自带的
中文描述，再看拼音（custom/Temreg 只有拼音），都匹配不上归「其他」。每类设上限，
目标 200–400 条。

用法：
  python3 scripts/gen_emoji_kaomoji.py
"""
import argparse
import hashlib
import html
import json
import os
import re
import sys
from collections import Counter
from pathlib import Path

from emoji_kaomoji_rules import (
    CN_RULES,
    EMOJI_GROUP_CN,
    EMOJI_MUST_HAVE,
    EMOJI_ORDER,
    EMOJI_SUBGROUP_CAP,
    KAOMOJI_CAP,
    KAOMOJI_ORDER,
    PY_RULES,
)


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OPENMOJI = Path("/tmp/emoji-src/openmoji.json")
DEFAULT_KAOMOJI_DIR = Path("/tmp/kaomoji-src2/data")

# 源文件钉（2026-10-18 取得）。不符 = 上游换了数据，先人工确认再改这里。
OPENMOJI_SHA256 = "b3a07c1c41b6b500423d01fb91c5d41e9c9337a53d217af20a3539d7df8438e4"
KAOMOJI_SHA256 = {
    "A_kaomoji_dict_data.txt": "d67207ab5e719f622f6d7b87a0146408ed776cca0680fcd37bb4fc87e3f8c23e",
    "custom_phrase_dict_data.txt": "b7c15690d6bda27c4654979d014eb38bcae186537ab4ed1a772321d54453facb",
    "lmeee_dict_data.txt": "06d6bef7c6d83f522e846729a15b46c9fd42367e498023845bebb5e5da2a9718",
    "sougou_dict_data.txt": "30153a4c5befcca01df7d8519dff3f6f7932f508cd06c773ff5f999bd761e842",
    "Temreg_dict_data.txt": "557cf8a32dd3e575f696f0af99fdc183bd74d0a31494f5ed4d515c4b536eb27e",
}

CJK = "㐀-鿿豈-﫿"
CJK_PUNCT = "　-〿！-／：-＠…—〜"
LABEL_RE = re.compile(r"^\s*[\[【][^\]】]{1,10}[\]】]\s*")
TAIL_LABEL_RE = re.compile(r"\s*[\[【][^\]】]{1,12}[\]】]\s*$")
# 行首描述：「无敌肉包拳！」「嗯~ 」「安安啦~~~ 」这类中文前缀到第一个颜文字字符为止
LEAD_DESC_RE = re.compile(
    r"^\s*[^()（）<>＜＞/\\]{1,14}?[" + CJK_PUNCT + r"\s~]+"
    r"(?=[()（）<>＜＞/\\~～oOヽノψφ╮╯┻▄])"
)
# 行尾描述：「吃药」「哎哟哟……」「 真开心」（要求空白分隔，避免吃掉颜文字里的中文）
TAIL_DESC_RE = re.compile(r"\s+[" + CJK + r"]{1,24}[…—～！？。，、；：!?.]*\s*$")
# 判定「像颜文字」：括号/分隔符/表情符号至少有一个，或两个以上假名/符号面部件。
KAO_CHARS = set("()（）[]【】{}<>＜＞/\\ノヽゝゞ~～_=^|｜@#")
WIDE_CHARS = set("・ω´｀∀≧≦▽△□°⊙﹏╯╰σΣ∪ᴗ꒳ᐛᗜ‸")
BAD_SUBSTRINGS = ("<span", "class=", "输入文字", "&amp;", "http", "www.", "&#")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(path: Path, expected: str, label: str) -> None:
    if not path.is_file():
        raise SystemExit(f"FATAL: 找不到{label}：{path}（见脚本头 provenance）")
    actual = sha256(path)
    if actual != expected:
        raise SystemExit(
            f"FATAL: {label} 的 sha256 变了\n  期望 {expected}\n  实得 {actual}\n"
            f"  上游换了数据 —— 人工确认筛选结果后再改脚本里的 pin。"
        )


def select_emoji(path: Path) -> list[tuple[str, str, str]]:
    """OpenMoji JSON → [(text, 英文名小写, 中文组)]，按 EMOJI_ORDER 连续分组。

    子类内按**码位升序**取前 N（早期码位 ≈ 更早进入 Unicode ≈ 更普及；这是代理，
    所以常见但码位靠后的条目由 EMOJI_MUST_HAVE 兜住）。子类顺序取 OpenMoji 文件序。
    """
    rows = json.loads(path.read_text(encoding="utf-8"))
    by_sub: dict[str, list[tuple[int, str, str, str]]] = {}
    sub_order: list[str] = []
    unknown: set[str] = set()
    for row in rows:
        group = row.get("group", "")
        sub = row.get("subgroups", "")
        if group not in EMOJI_GROUP_CN:
            continue  # flags / component / extras-*：整组不收
        if sub not in EMOJI_SUBGROUP_CAP:
            unknown.add(sub)
            continue
        if row.get("skintone") or not row.get("unicode"):
            continue
        text = row.get("emoji", "")
        # ZWJ 组合（家庭/职业/情侣…）整类不收；VS16 双码位形（❤️/⚽️）保留。
        if "‍" in text or not text or "\t" in text or "\n" in text:
            continue
        if sub not in by_sub:
            by_sub[sub] = []
            sub_order.append(sub)
        cat = EMOJI_GROUP_CN[group]
        if group == "animals-nature" and sub.startswith("plant"):
            cat = "植物"
        by_sub[sub].append((ord(text[0]), text, row.get("annotation", "").lower(), cat))
    if unknown:
        raise SystemExit(f"FATAL: OpenMoji 出现未归类的 subgroups {sorted(unknown)} —— 补表再跑")

    buckets: dict[str, list[tuple[str, str]]] = {c: [] for c in EMOJI_ORDER}
    seen_names: set[str] = set()
    seen_text: set[str] = set()
    for sub in sub_order:
        items = sorted(by_sub[sub])  # 码位升序
        picked = [it for it in items if it[2] in EMOJI_MUST_HAVE]
        seen_names.update(it[2] for it in picked)
        rest = [it for it in items if it[2] not in EMOJI_MUST_HAVE][:EMOJI_SUBGROUP_CAP[sub]]
        for _cp, text, name, cat in sorted(picked + rest):
            if text in seen_text:
                continue
            seen_text.add(text)
            buckets[cat].append((text, name))
    missing = EMOJI_MUST_HAVE - seen_names
    if missing:
        raise SystemExit(f"FATAL: EMOJI_MUST_HAVE 里的名字在上游找不到 {sorted(missing)}")
    empty = [c for c in EMOJI_ORDER if not buckets[c]]
    if empty:
        raise SystemExit(f"FATAL: 分类为空 {empty}")
    return [(t, n, cat) for cat in EMOJI_ORDER for t, n in buckets[cat]]


def extract_kaomoji(src: Path) -> list[tuple[str, str, str]]:
    """5 份素材 → [(原文, 描述或拼音, 源名)]，源序 = 常用度代理。"""
    out: list[tuple[str, str, str]] = []
    files = {name: (src / name).read_text(encoding="utf-8") for name in KAOMOJI_SHA256}
    for line in files["lmeee_dict_data.txt"].splitlines():
        m = re.search(r'data-clipboard-text="([^"]*)"', line)
        if m:
            d = re.search(r'data-desc="([^"]*)"', line)
            out.append((html.unescape(m.group(1)), html.unescape(d.group(1)) if d else "", "lmeee"))
    for line in files["sougou_dict_data.txt"].splitlines():
        m = re.match(
            r'<div class="ywz_content">(.*?)</div>\t'
            r'<div class="ywz_cont_name">输入文字：(.*?)</div>',
            line,
        )
        if m:
            out.append((html.unescape(m.group(1)), html.unescape(m.group(2)), "sougou"))
    for line in files["A_kaomoji_dict_data.txt"].splitlines():
        if line.strip():
            out.append((line.rstrip("\n"), "", "A"))
    for line in files["custom_phrase_dict_data.txt"].splitlines():
        m = re.match(r"^([a-z ]+?)\s{2,}(.+)$", line)
        if m:
            out.append((m.group(2), m.group(1), "custom"))
    for line in files["Temreg_dict_data.txt"].splitlines():
        parts = line.split("\t")
        if len(parts) >= 2:
            out.append((parts[0], parts[1], "Temreg"))
    return out


def clean_kaomoji(text: str) -> str:
    text = html.unescape(text).replace("​", "").strip()
    for _ in range(2):  # 可能先有 [标签] 再有中文前缀
        text = LABEL_RE.sub("", text)
        m = LEAD_DESC_RE.match(text)
        if m and re.search("[" + CJK + "]", m.group(0)):
            text = text[m.end():]
        else:
            break
    # 尾部 [标签]：只剥含中文的（[BINGO!] 这类是颜文字的一部分，保留）
    m = TAIL_LABEL_RE.search(text)
    if m and re.search("[" + CJK + "]", m.group(0)):
        text = text[:m.start()]
    text = TAIL_DESC_RE.sub("", text)
    return text.strip()


def kaomoji_ok(text: str) -> bool:
    if not (2 <= len(text) <= 32):
        return False
    if any(c in "\t\r\n" for c in text) or "�" in text:
        return False
    if any(s in text for s in BAD_SUBSTRINGS):
        return False
    if not (any(c in KAO_CHARS for c in text) or sum(c in WIDE_CHARS for c in text) >= 2):
        return False
    # 至少两个非中文可见字符：挡住「好的」「同意」这类漏出来的纯描述。
    return sum(1 for c in text if not (re.match("[" + CJK + "]", c) or re.match("[" + CJK_PUNCT + "]", c))) >= 2


def classify_kaomoji(desc: str) -> str:
    d = desc.lower()
    for cat, words in CN_RULES:
        if any(w in d for w in words.split()):
            return cat
    tokens = " " + " ".join(d.split()) + " "
    for cat, phrases in PY_RULES:
        if any(f" {p} " in tokens for p in phrases):
            return cat
    return "其他"


def select_kaomoji(entries: list[tuple[str, str, str]]) -> list[tuple[str, str]]:
    seen: set[str] = set()
    kept: list[tuple[str, str]] = []
    counts = Counter()
    for raw, desc, _src in entries:
        text = clean_kaomoji(raw)
        if text in seen or not kaomoji_ok(text):
            continue
        cat = classify_kaomoji(desc)
        if counts[cat] >= KAOMOJI_CAP[cat]:
            continue
        seen.add(text)
        counts[cat] += 1
        kept.append((text, cat))
    return sorted(kept, key=lambda r: KAOMOJI_ORDER.index(r[1]))  # 稳定排序 = 类内保持源序


def write_atomic(path: Path, text: str) -> None:
    tmp = path.with_name(f"{path.name}.tmp.{os.getpid()}")
    tmp.write_text(text, encoding="utf-8")
    os.replace(tmp, path)


def main() -> None:
    ap = argparse.ArgumentParser(description="生成 emoji.tsv / kaomoji.tsv")
    ap.add_argument("--openmoji", type=Path,
                    default=Path(os.environ.get("OPENMOJI_JSON", DEFAULT_OPENMOJI)))
    ap.add_argument("--kaomoji-dir", type=Path,
                    default=Path(os.environ.get("KAOMOJI_DIR", DEFAULT_KAOMOJI_DIR)))
    args = ap.parse_args()

    verify(args.openmoji, OPENMOJI_SHA256, "OpenMoji openmoji.json")
    for name, expected in KAOMOJI_SHA256.items():
        verify(args.kaomoji_dir / name, expected, f"rime_kaomoji_dict/{name}")

    emoji = select_emoji(args.openmoji)
    # 生成期门禁（与 engine-core/tests/emoji_coverage.rs 两道坎；这里拦得更早）。
    if not 400 <= len(emoji) <= 700:
        raise SystemExit(f"FATAL: emoji 选取 {len(emoji)} 条，超出 400–700")
    if len({t for t, _, _ in emoji}) != len(emoji):
        raise SystemExit("FATAL: emoji 有重复序列")
    for text, name, cat in emoji:
        if not text or not name or not cat or "\t" in name:
            raise SystemExit(f"FATAL: emoji 行字段损坏: {text!r} {name!r} {cat!r}")
    must = {"\U0001f600", "\U0001f602", "\U0001f970", "\U0001f62d", "\U0001f621",
            "\U0001f44d️", "\U0001f436", "\U0001f339", "\U0001f34e",
            "\U0001f697", "⚽️", "\U0001f4f1", "\U0001f389",
            "❤️", "\U0001f680", "\U0001f4a1", "\U0001f992", "\U0001f418",
            "\U0001f4b0️", "✅️", "\U0001f45f"}
    missing = must - {t for t, _, _ in emoji}
    if missing:
        raise SystemExit(f"FATAL: 常用表情代表缺失 {sorted(missing)} —— 检查子类上限")

    kaomoji = select_kaomoji(extract_kaomoji(args.kaomoji_dir))
    if not 200 <= len(kaomoji) <= 400:
        raise SystemExit(f"FATAL: kaomoji 选取 {len(kaomoji)} 条，超出 200–400")
    if len({t for t, _ in kaomoji}) != len(kaomoji):
        raise SystemExit("FATAL: kaomoji 有重复文本")
    for text, cat in kaomoji:
        if not text or not cat or any(c in "\t\r\n" for c in text):
            raise SystemExit(f"FATAL: kaomoji 行损坏: {text!r} {cat!r}")

    emoji_text = "\n".join(f"{t}\t{n}\t{c}" for t, n, c in emoji) + "\n"
    kaomoji_text = "\n".join(f"{t}\t{c}" for t, c in kaomoji) + "\n"

    write_atomic(ROOT / "data/raw/emoji.tsv", emoji_text)
    # 两处 kaomoji 用同一个字符串写（「同一次运行、同一内容」是副本一致性的前提）
    write_atomic(ROOT / "data/raw/kaomoji.tsv", kaomoji_text)
    write_atomic(ROOT / "android/app/src/main/assets/kaomoji.tsv", kaomoji_text)

    print(f"emoji.tsv {len(emoji)} 条 → data/raw/emoji.tsv")
    for cat in EMOJI_ORDER:
        print(f"  {cat} {sum(1 for _, _, c in emoji if c == cat)}")
    print(f"kaomoji.tsv {len(kaomoji)} 条 → data/raw/kaomoji.tsv + android assets")
    for cat in KAOMOJI_ORDER:
        print(f"  {cat} {sum(1 for _, c in kaomoji if c == cat)}")


if __name__ == "__main__":
    sys.exit(main())
