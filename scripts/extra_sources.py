#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""`gen_extra_phrases.py` 的各上游解析器（2026-10-11 拆出）。

拆出去的原因：`gen_extra_phrases.py` 贴着 500 行门禁，加 OpenCC 繁简判据就超
（照 `symbol_keywords.py` 从 `gen_symbols.py` 拆出的先例）。内容是纯解析/读表：
上游文本 → 内存行，不含任何过滤或排序决策 —— 那些留在生成器里。
"""
import json
from pathlib import Path


def parse_ice(text: str) -> list:
    """rime dict.yaml 正文 → [(word, [音节...], weight)]（front-matter 至 '...' 止）。"""
    rows, started = [], False
    for line in text.splitlines():
        if not started:
            if line.strip() == "...":
                started = True
            continue
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        rows.append((parts[0], parts[1].split(), int(parts[2])))
    return rows


def parse_essay(text: str) -> list:
    out = []
    for line in text.splitlines():
        if not line:
            continue
        w, _, f = line.partition("\t")
        out.append((w, int(f) if f.strip().isdigit() else 0))
    return out


def parse_thuocl(text: str) -> list:
    out = []
    for line in text.splitlines():
        if not line.strip():
            continue
        w, _, d = line.partition("\t")
        out.append((w.strip(), int(d.strip()) if d.strip().isdigit() else 0))
    return out


def parse_manual(path: Path, expect_n: int) -> list:
    """人工表 `word\t拼音`（拼音人工写死）；行数不符即 FATAL（防手滑）。"""
    if expect_n == 0:
        raise RuntimeError("先填 %s 的实测行数（常量 0 是防呆，不是可用值）" % path.name)
    lines = [l for l in path.read_text(encoding="utf-8").splitlines() if l.strip()]
    if len(lines) != expect_n:
        raise RuntimeError("%s 行数 %d != 常量 %d：改表必须同改常量"
                           % (path.name, len(lines), expect_n))
    out = []
    for l in lines:
        w, _, k = l.partition("\t")
        out.append((w.strip(), k.strip()))
    return out


def parse_idiom(text: str) -> list:
    return [(d["word"], d["pinyin"].split(), d.get("abbreviation", "")) for d in json.loads(text)]


def parse_xie(text: str) -> list:
    return [d.get("riddle", "") + "——" + d.get("answer", "") for d in json.loads(text)]


def parse_ts(text: str) -> dict:
    """OpenCC TSCharacters.txt → {繁形字: {候选简形字...}}。

    ⚠️ **含自靶的行不是纯繁形**：`乾 → 干 乾`、`覆 → 覆 复` 这类把原字也列进目标
    （该字形在简体语境里另有合法读音/用法，如 乾隆/覆盖）—— 判繁时必须先排除自靶，
    否则会把 乾隆 这类词整批误杀（旧的 Unihan 判据正是为规避这类误报才加 GB 门）。
    本函数的调用方（生成器的 `is_trad`）负责 `ch not in targets` 这条判断。"""
    ts: dict = {}
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 2 or not parts[0] or not parts[1].strip():
            continue
        ts.setdefault(parts[0], set()).update(parts[1].split())
    return ts
