#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""Rime `*.dict.yaml`（rime-wanxiang 等）→ OPI 导入词库 TSV（2026-10-26）。

上游示例：https://github.com/amzxyz/rime-wanxiang
许可：CC BY 4.0（**数据**；本脚本自身仍 MIT）—— 产物必须署名、标明改动；CC BY
      没有 share-alike 义务。详见 `docs/external-dictionaries.md`。

Pin（2026-10-26 实测）：仓库 commit `b7d4ee298c0c933ee5cfd2521f98cee8b9d4bde4`，
默认分支 `wanxiang`，许可证 CC-BY-4.0。仓库约 34 GB（含模型/音频），**不要整仓
clone**；只按 raw URL 取需要的表：

    dicts/zi.dict.yaml      616,559 B  1 字多读音（基础字表）
    dicts/jichu.dict.yaml    49,485,140 B  基础词库（本转换器的主力）
    dicts/duoyin.dict.yaml   137,474 B  多音兼容词
    dicts/cuoyin.dict.yaml     8,116 B  错音错字兼容
    dicts/diming.dict.yaml   3,307,073 B  地名
    dicts/mingren.dict.yaml    667,181 B  名人
    dicts/yiren.dict.yaml      423,957 B  艺人
（`wanxiang.dict.yaml` 的 `import_tables` 还列了 zi/jichu/cuoyin/duoyin/diming/
mingren/yiren；yixue/huaxue/... 在源里是注释掉的。）

## 转换口径（与 `engine-data/src/dict_text.rs` 的装载契约对齐）

- 跳过 `#` 注释、空行、rime front-matter（`---` … `...`/`---`；未闭合**报错**，
  与装载器一致，绝不静默吃掉后半份文件）。
- `词<TAB>code[<TAB>权重]`：两列 = 默认权重 1000；三列整数直取，`NN.NN%` 按
  `round(pct × 1000)`（与 Rust `parse_freq` 同舍入方向）。
- code 去声调（NFD 去组合符）、ü→v、小写、去音节空格与 `'` —— 本仓查询键是连写
  无调拼音（`nǐ hǎo` → `nihao`、`nǚ ér` → `nver`）。
- 词/拼音都守 ≤255 字节（超出装载器会丢）；词里含 `\\` 的条目跳过 —— 那是
  Rime 自定义短语的展开转义（如 `静夜思\\n...`），不是可输入的词条。
- 同一 `(词, 拼音)` 取**最大**权重（不同读音都保留，如 `行 háng/xíng`）。
- 非法权重（上游实测有 `6N`）**不丢词**：降级为 1000 并计数。

## 输出与上限

`词<TAB>拼音<TAB>词频`。默认 `--max-entries 200000 --max-output-mb 8`（Android/macOS
设置窗导入面的硬上限）；截断顺序 = 权重降序（保留 `sort: by_weight` 意图）。
完整 zi+jichu 约 1,543,055 条 / 39.2 MiB，只能走 fcitx5（Linux）/ TSF（Windows）的
词库目录直载——这两条宿主轨直接调 `load_extra_dict`，没有这两条上限。此时传
`--max-entries 0 --max-output-mb 0`。

用法：
    python3 scripts/convert_rime_dict.py dicts/zi.dict.yaml dicts/jichu.dict.yaml -o wx.tsv
    python3 scripts/convert_rime_dict.py dicts/jichu.dict.yaml --max-entries 0 --max-output-mb 0
"""
from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

from external_dict_common import (
    DEFAULT_FREQ,
    Entry,
    add_entry,
    header_lines,
    normalize_pinyin,
    parse_weight,
    sha256_file,
    valid_pinyin,
    valid_word,
    weight_first,
    write_tsv,
)

REPO = "https://github.com/amzxyz/rime-wanxiang"
PIN_COMMIT = "b7d4ee298c0c933ee5cfd2521f98cee8b9d4bde4"
PIN_NOTE = "default branch `wanxiang`"
LICENSE_NAME = "CC BY 4.0"
LICENSE_URL = "https://creativecommons.org/licenses/by/4.0/"
DEFAULT_MAX_ENTRIES = 200_000
DEFAULT_MAX_MB = 8


@dataclass
class Stats:
    parsed: int = 0
    duplicate: int = 0
    bad_line: int = 0
    bad_word: int = 0
    bad_pinyin: int = 0
    bad_weight: int = 0
    skipped: int = 0


@dataclass
class SourceFile:
    path: Path
    sha256: str


def parse_rime_file(path: Path, stats: Stats) -> list[tuple[str, str, str | None]]:
    """→ [(词, 原始 code, 原始权重或 None)]。front-matter 未闭合时抛 ValueError。"""
    rows: list[tuple[str, str, str | None]] = []
    state = "before"
    for raw in path.read_text(encoding="utf-8").splitlines():
        stripped = raw.strip()
        if state == "before":
            if stripped == "---":
                state = "front_matter"
                continue
            if not stripped or stripped.startswith("#"):
                continue
            state = "body"  # 没有 front-matter 也接受
        if state == "front_matter":
            if stripped in ("---", "..."):
                state = "body"
            continue
        if not stripped or stripped.startswith("#") or stripped.startswith("-"):
            continue
        cols = raw.split("\t")
        if len(cols) not in (2, 3):
            stats.bad_line += 1
            continue
        stats.parsed += 1
        rows.append((cols[0].strip(), cols[1].strip(), cols[2] if len(cols) == 3 else None))
    if state == "front_matter":
        raise ValueError(
            f"{path} 的 front-matter 到文件末尾仍未闭合（`---` … `...`）——"
            "拒绝把配置行当成词条；先修上游文件"
        )
    return rows


def convert(
    inputs: list[Path],
    output_path: Path,
    *,
    max_entries: int = DEFAULT_MAX_ENTRIES,
    max_output_mb: int = DEFAULT_MAX_MB,
    source_revision: str = PIN_COMMIT,
    source_note: str = PIN_NOTE,
) -> tuple[Stats, object]:
    stats = Stats()
    sources: list[SourceFile] = []
    raw: list[tuple[str, str, str | None]] = []
    for path in inputs:
        sources.append(SourceFile(path, sha256_file(path)))
        raw.extend(parse_rime_file(path, stats))

    best: dict[tuple[str, str], Entry] = {}
    for order, (word, code, raw_weight) in enumerate(raw):
        pinyin = normalize_pinyin(code)
        if not valid_word(word) or "\\" in word:
            stats.bad_word += 1
            continue
        if not valid_pinyin(pinyin):
            stats.bad_pinyin += 1
            continue
        if raw_weight is None:
            freq = DEFAULT_FREQ
        else:
            freq, ok = parse_weight(raw_weight)
            if not ok:
                stats.bad_weight += 1
        if not add_entry(best, Entry(word, pinyin, freq, order)):
            stats.duplicate += 1
    entries = list(best.values())
    if not entries:
        raise ValueError(
            f"{' / '.join(str(p) for p in inputs)} 没有一条可用词条"
            f"（坏行 {stats.bad_line}、坏词 {stats.bad_word}、坏拼音 {stats.bad_pinyin}）——"
            "拒绝写空词库"
        )

    def build_header(written: int, drop_entry: int, drop_bytes: int) -> list[str]:
        source_lines = [
            "Source: rime-wanxiang (Rime dictionary tables)",
            f"Source URL: {REPO}",
            f"Source revision: {source_revision} ({source_note})",
        ]
        source_lines += [
            f"Source file: {s.path.name} (sha256 {s.sha256})" for s in sources
        ]
        return header_lines(
            generator="scripts/convert_rime_dict.py",
            source_lines=source_lines,
            license_lines=[
                f"License: {LICENSE_NAME} — {LICENSE_URL}",
                "Attribution: rime-wanxiang data by amzxyz and contributors.",
                "Changes: tone marks removed; ü mapped to v; syllable spaces/apostrophes",
                "removed to produce concatenated toneless pinyin keys; duplicate (word,",
                "pinyin) merged with max weight; malformed weights coerced to 1000.",
            ],
            change_lines=[
                "Entries were deterministically truncated if the caps below applied",
                "(higher weights first; then shorter words; then key/word for stability).",
            ],
            stats_line=(
                f"Entries written: {written} / {len(entries)} unique; dropped: "
                f"{drop_entry} (entry cap), {drop_bytes} (byte cap); "
                f"malformed weights coerced: {stats.bad_weight}; "
                f"skipped bad lines/words/pinyin: {stats.bad_line}/"
                f"{stats.bad_word}/{stats.bad_pinyin}."
            ),
        )

    result = write_tsv(
        output_path,
        entries,
        build_header,
        max_entries=max_entries,
        max_bytes=0 if max_output_mb == 0 else max_output_mb * 1024 * 1024,
        priority=weight_first,
        three_columns=True,
    )
    return stats, result


def _num(n: int) -> str:
    return f"{n:,}"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description="Rime *.dict.yaml → OPI 导入词库 TSV（CC BY 4.0 数据）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__.split("用法：", 1)[-1],
    )
    ap.add_argument("inputs", nargs="+", type=Path, help="一个或多个 *.dict.yaml")
    ap.add_argument("-o", "--output", type=Path, help="输出 .tsv（缺省 = 第一个输入词干 + .tsv）")
    ap.add_argument(
        "--max-entries",
        type=int,
        default=DEFAULT_MAX_ENTRIES,
        help=f"条目上限（0 = 不设限；缺省 {DEFAULT_MAX_ENTRIES}）",
    )
    ap.add_argument(
        "--max-output-mb",
        type=int,
        default=DEFAULT_MAX_MB,
        help=f"输出 MiB 上限（0 = 不设限；缺省 {DEFAULT_MAX_MB}）",
    )
    ap.add_argument("--source-revision", default=PIN_COMMIT, help="写进表头的上游 revision")
    ap.add_argument("--source-note", default=PIN_NOTE, help="写进表头的上游版本说明")
    args = ap.parse_args(argv)
    if args.max_entries < 0 or args.max_output_mb < 0:
        ap.error("--max-entries / --max-output-mb 不能为负")
    out = args.output or args.inputs[0].with_suffix(".tsv")
    try:
        stats, result = convert(
            args.inputs,
            out,
            max_entries=args.max_entries,
            max_output_mb=args.max_output_mb,
            source_revision=args.source_revision,
            source_note=args.source_note,
        )
    except (ValueError, OSError, RuntimeError) as e:
        print(f"FATAL: {e}", file=sys.stderr)
        return 1
    print(
        f"RIME: parsed {_num(stats.parsed)}, unique {_num(result.total_unique)}, "
        f"duplicate {_num(stats.duplicate)}, bad weight {_num(stats.bad_weight)}, "
        f"bad line {_num(stats.bad_line)}, bad word {_num(stats.bad_word)}, "
        f"bad pinyin {_num(stats.bad_pinyin)}",
        file=sys.stderr,
    )
    print(
        f"RIME: wrote {out} — {_num(result.written)} entries, "
        f"dropped {_num(result.dropped_by_entries)} by entry cap / "
        f"{_num(result.dropped_by_bytes)} by byte cap, "
        f"{result.bytes_written / 1048576:.2f} MiB",
        file=sys.stderr,
    )
    if result.dropped_by_entries or result.dropped_by_bytes:
        print(
            "RIME: NOTE 截断是确定性丢弃；要完整词库请传 --max-entries 0 --max-output-mb 0，"
            "并用词库目录直载（fcitx5/TSF）；Android/macOS 的设置窗导入面上限是 200000 条 / 8 MiB。",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
