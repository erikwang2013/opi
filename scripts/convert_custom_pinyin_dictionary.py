#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""CustomPinyinDictionary → OPI 导入词库 TSV（2026-10-26）。

上游：https://github.com/wuhgit/CustomPinyinDictionary
许可：CC BY-SA 4.0（**数据**；本脚本自身仍 MIT）—— 产物是改编数据，必须署名、
      标明改动、并以 CC BY-SA 4.0 再分发。详见 `docs/external-dictionaries.md`。
Pin：仓库 commit `cf17f96af885cb818c2fad87184f383a52482351`，release tag `assets`，
     数据构建 v20260101（`status.json` 的 updateDate=2026-01-01、totalWords=1,498,781）。

## 上游实际格式（2026-10-26 实测；仓库树里**没有**词库文本）

- `CustomPinyinDictionary_Fcitx.dict`（release 资产，29,688,969 B）：
  libime 二进制词典（开头偏移 8 起是 zstd 帧），**无可用词频**（dump 第三列恒 0）。
  用 `libime_pinyindict -d <in> <out>`（libime-bin 包）转出的文本是：
  `词<空格>音节'音节<空格>0`，实测 1,498,781 行、全部 3 字段、code 只含 `[a-z']`。
- `CustomPinyinDictionary_Gboard_Magisk_*.zip` 里的 `dict/db`：SQLite
  `entry(_id, word, shortcut, locale)`，同样 1,498,781 行，**无频次列**。
  本脚本可直接读 SQLite（纯标准库），不需要 libime。
- 两个 Magisk zip 的 `dict/data` 仍是 libime 二进制；脚本会自动挑 `dict/db`。

## 输出

`词<TAB>拼音`（**两列**）—— 上游没有可用词频，写第三列只会伪造一个数字；装载器
`engine-data::dict_text::load_extra_dict` 对两列默认 freq=1000。拼音去掉了 `'`
音节分隔（本仓查询键是连写，如 `nihao`），同 (词, 拼音) 去重。

默认 `--max-entries 200000 --max-output-mb 8`：这是 `opi-ffi` 导入面
（Android/macOS 设置窗）的硬上限；完整 1,498,781 条约 34.4 MiB，**不能**走该导入面，
只能用 fcitx5（Linux）/ TSF（Windows）的词库目录直载——这两条宿主轨直接调
`load_extra_dict`，没有 8 MiB/200k 限制。此时传 `--max-entries 0 --max-output-mb 0`。
无权重源的截断顺序 = 短词优先（稳定、可复现）。

用法：
    python3 scripts/convert_custom_pinyin_dictionary.py CustomPinyinDictionary_Fcitx.dict
    python3 scripts/convert_custom_pinyin_dictionary.py CustomPinyinDictionary_Gboard_Magisk_20260101.zip -o cpd.tsv
    python3 scripts/convert_custom_pinyin_dictionary.py dump.txt --max-entries 0 --max-output-mb 0
"""
from __future__ import annotations

import argparse
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

from external_dict_common import (
    DEFAULT_FREQ,
    Entry,
    add_entry,
    header_lines,
    normalize_pinyin,
    sha256_file,
    shortest_first,
    valid_pinyin,
    valid_word,
    write_tsv,
)

REPO = "https://github.com/wuhgit/CustomPinyinDictionary"
PIN_COMMIT = "cf17f96af885cb818c2fad87184f383a52482351"
PIN_NOTE = "release tag `assets`; data build v20260101 (status.json updateDate=2026-01-01, totalWords=1,498,781)"
LICENSE_NAME = "CC BY-SA 4.0"
LICENSE_URL = "https://creativecommons.org/licenses/by-sa/4.0/"
DEFAULT_MAX_ENTRIES = 200_000
DEFAULT_MAX_MB = 8


@dataclass
class Stats:
    parsed: int = 0
    duplicate: int = 0
    bad_line: int = 0
    bad_word: int = 0
    bad_pinyin: int = 0
    notes: list[str] = field(default_factory=list)


def _collect(
    rows,
    stats: Stats,
    best: dict[tuple[str, str], Entry],
) -> None:
    for order, (word, code) in enumerate(rows):
        stats.parsed += 1
        word = str(word).strip()
        code = str(code)
        pinyin = normalize_pinyin(code)
        if not valid_word(word):
            stats.bad_word += 1
            continue
        if not valid_pinyin(pinyin):
            stats.bad_pinyin += 1
            continue
        if not add_entry(best, Entry(word, pinyin, DEFAULT_FREQ, order)):
            stats.duplicate += 1


def _read_dump(path: Path, stats: Stats) -> list[tuple[str, str]]:
    """libime_pinyindict -d 的文本：`词 音节'音节 0`（单空格分隔）。"""
    rows: list[tuple[str, str]] = []
    text = path.read_text(encoding="utf-8")
    if text.startswith("﻿"):
        text = text[1:]
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        # rsplit：词里若真有空格也不会把 code 切坏；示例格式固定 3 段。
        parts = line.rsplit(" ", 2)
        if len(parts) != 3:
            stats.bad_line += 1
            continue
        word, code, _freq = parts
        rows.append((word, code))
    return rows


def _read_sqlite(path: Path) -> list[tuple[str, str]]:
    con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        try:
            cur = con.execute("SELECT word, shortcut FROM entry ORDER BY _id")
        except sqlite3.DatabaseError as e:
            raise ValueError(
                f"{path} 不是预期的 Gboard 词库库（entry(word, shortcut, locale)）：{e}"
            ) from e
        return [(w, s) for w, s in cur]
    finally:
        con.close()


def _is_sqlite(path: Path) -> bool:
    with open(path, "rb") as f:
        return f.read(16) == b"SQLite format 3\x00"


def _is_libime(path: Path) -> bool:
    with open(path, "rb") as f:
        head = f.read(64)
    # 实测头 8 字节后紧跟 zstd 帧魔数；按魔数判、不按扩展名判（dumped 文本也可能叫 .dict）。
    return b"\x28\xb5\x2f\xfd" in head


def _dump_libime(path: Path, tool: str) -> Path:
    exe = shutil.which(tool)
    if exe is None:
        raise ValueError(
            f"输入是 libime 二进制词典，需要 `{tool}`（Debian/Ubuntu: apt install libime-bin；"
            "Arch: libime）；或改用 Gboard zip 里的 dict/db（纯 Python 可读）"
        )
    fd, tmp = tempfile.mkstemp(prefix="opi-cpd-dump-", suffix=".txt")
    os.close(fd)
    out = Path(tmp)
    try:
        proc = subprocess.run(
            [exe, "-d", str(path), str(out)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if proc.returncode != 0 or out.stat().st_size == 0:
            raise ValueError(
                f"{tool} -d 失败（exit {proc.returncode}）："
                f"{proc.stderr.decode('utf-8', 'replace').strip()[:300]}"
            )
        return out
    except BaseException:
        out.unlink(missing_ok=True)
        raise


def _unpack_zip(path: Path) -> tuple[Path, str, tempfile.TemporaryDirectory]:
    tmpdir = tempfile.TemporaryDirectory(prefix="opi-cpd-zip-")
    with zipfile.ZipFile(path) as zf:
        names = set(zf.namelist())
        # Gboard 的 db 是 SQLite（纯标准库）；Fcitx zip 的 dict/data 是 libime 二进制。
        for inner in ("dict/db", "dict/data"):
            if inner in names:
                target = Path(tmpdir.name) / Path(inner).name
                with zf.open(inner) as src, open(target, "wb") as dst:
                    shutil.copyfileobj(src, dst)
                return target, inner, tmpdir
    tmpdir.cleanup()
    raise ValueError(
        f"{path} 里没有 dict/db（Gboard zip）或 dict/data（Fcitx zip）"
    )


def convert(
    input_path: Path,
    output_path: Path,
    *,
    max_entries: int = DEFAULT_MAX_ENTRIES,
    max_output_mb: int = DEFAULT_MAX_MB,
    libime_tool: str = "libime_pinyindict",
    source_revision: str = PIN_COMMIT,
    source_note: str = PIN_NOTE,
) -> tuple[Stats, object]:
    stats = Stats()
    tmpdir: tempfile.TemporaryDirectory | None = None
    dump_tmp: Path | None = None
    try:
        artifact_name = input_path.name
        artifact_sha = sha256_file(input_path)
        work = input_path
        if zipfile.is_zipfile(input_path):
            work, inner, tmpdir = _unpack_zip(input_path)
            artifact_name = f"{input_path.name}!{inner}"
            artifact_sha = sha256_file(work)
        if _is_sqlite(work):
            rows = _read_sqlite(work)
            stats.notes.append(f"read SQLite entry table ({work.name})")
        else:
            if _is_libime(work):
                dump_tmp = _dump_libime(work, libime_tool)
                rows = _read_dump(dump_tmp, stats)
                stats.notes.append(f"dumped libime binary with `{libime_tool} -d`")
            else:
                rows = _read_dump(work, stats)
                stats.notes.append("read libime dump text")
        best: dict[tuple[str, str], Entry] = {}
        _collect(rows, stats, best)
        entries = list(best.values())

        def build_header(written: int, drop_entry: int, drop_bytes: int) -> list[str]:
            return header_lines(
                generator="scripts/convert_custom_pinyin_dictionary.py",
                source_lines=[
                    "Source: CustomPinyinDictionary",
                    f"Source URL: {REPO}",
                    f"Source revision: {source_revision} ({source_note})",
                    f"Source artifact: {artifact_name} (sha256 {artifact_sha})",
                ],
                license_lines=[
                    f"License: {LICENSE_NAME} — {LICENSE_URL}",
                    "Attribution: dictionary data by wuhgit and contributors.",
                    "Derived-data notice: this file is a CC BY-SA 4.0 adaptation. Redistribution",
                    "must keep attribution and be licensed under CC BY-SA 4.0 (share-alike).",
                    "Upstream README credits third-party sources (FREEMDICT, administrative",
                    "divisions, customs/statistics catalogues); review those before reuse.",
                ],
                change_lines=[
                    "Changes: syllable separators (apostrophes) removed to produce concatenated",
                    "pinyin keys; duplicates by (word, pinyin) merged; upstream data has no",
                    "usable frequency values (libime dump's third field is always 0), so output",
                    "is 2-column and OPI assigns its default 1000; entries were deterministically",
                    "truncated if the caps below applied (shortest words first).",
                ],
                stats_line=(
                    f"Entries written: {written} / {len(entries)} unique; dropped: "
                    f"{drop_entry} (entry cap), {drop_bytes} (byte cap)."
                ),
            )

        result = write_tsv(
            output_path,
            entries,
            build_header,
            max_entries=max_entries,
            max_bytes=0 if max_output_mb == 0 else max_output_mb * 1024 * 1024,
            priority=shortest_first,
            three_columns=False,
        )
        return stats, result
    finally:
        if dump_tmp is not None:
            dump_tmp.unlink(missing_ok=True)
        if tmpdir is not None:
            tmpdir.cleanup()


def _num(n: int) -> str:
    return f"{n:,}"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description="CustomPinyinDictionary → OPI 导入词库 TSV（CC BY-SA 4.0 数据）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__.split("用法：", 1)[-1],
    )
    ap.add_argument("input", type=Path, help=".dict / .zip / .db / libime dump .txt")
    ap.add_argument("-o", "--output", type=Path, help="输出 .tsv（缺省 = 输入名词干 + .tsv）")
    ap.add_argument(
        "--max-entries",
        type=int,
        default=DEFAULT_MAX_ENTRIES,
        help=f"条目上限（0 = 不设限；缺省 {DEFAULT_MAX_ENTRIES}，对应 opi-ffi 导入面）",
    )
    ap.add_argument(
        "--max-output-mb",
        type=int,
        default=DEFAULT_MAX_MB,
        help=f"输出 MiB 上限（0 = 不设限；缺省 {DEFAULT_MAX_MB}，对应 opi-ffi 导入面）",
    )
    ap.add_argument("--libime-tool", default="libime_pinyindict", help="libime 反编译工具名/路径")
    ap.add_argument("--source-revision", default=PIN_COMMIT, help="写进表头的上游 revision")
    ap.add_argument("--source-note", default=PIN_NOTE, help="写进表头的上游版本说明")
    args = ap.parse_args(argv)
    if args.max_entries < 0 or args.max_output_mb < 0:
        ap.error("--max-entries / --max-output-mb 不能为负")
    out = args.output or args.input.with_suffix(".tsv")
    try:
        stats, result = convert(
            args.input,
            out,
            max_entries=args.max_entries,
            max_output_mb=args.max_output_mb,
            libime_tool=args.libime_tool,
            source_revision=args.source_revision,
            source_note=args.source_note,
        )
    except (ValueError, OSError, RuntimeError) as e:
        print(f"FATAL: {e}", file=sys.stderr)
        return 1
    print(
        f"CPD: parsed {_num(stats.parsed)}, unique {_num(result.total_unique)}, "
        f"duplicate {_num(stats.duplicate)}, bad lines {_num(stats.bad_line)}, "
        f"bad word {_num(stats.bad_word)}, bad pinyin {_num(stats.bad_pinyin)}",
        file=sys.stderr,
    )
    for note in stats.notes:
        print(f"CPD: {note}", file=sys.stderr)
    print(
        f"CPD: wrote {out} — {_num(result.written)} entries, "
        f"dropped {_num(result.dropped_by_entries)} by entry cap / "
        f"{_num(result.dropped_by_bytes)} by byte cap, "
        f"{result.bytes_written / 1048576:.2f} MiB",
        file=sys.stderr,
    )
    if result.dropped_by_entries or result.dropped_by_bytes:
        print(
            "CPD: NOTE 截断是确定性丢弃；要完整词库请传 --max-entries 0 --max-output-mb 0，"
            "并用词库目录直载（fcitx5/TSF）；Android/macOS 的设置窗导入面上限是 200000 条 / 8 MiB。",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
