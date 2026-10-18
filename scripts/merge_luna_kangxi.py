#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""把 `data/raw/kangxi_luna.tsv` 并入已部署的 luna.opid，输出合并后的 TSV。

用途（CONTRIBUTING 的 luna 重编配方）：

    python3 scripts/merge_luna_kangxi.py > /tmp/luna.tsv
    cargo run --release -p opi-tools --bin opi-tools -- compile /tmp/luna.tsv \
        data/generated/luna.opid
    cp data/generated/luna.opid android/app/src/main/assets/luna.opid

为什么需要它：luna 的**词条源**是 `gen_luna_dict.py` 的 stdout（不落盘，且需要
上游下载）；已入库的部署副本是唯一现成的完整基线。这里按 .opid v1 布局还原
`word\tpinyin\tfreq`，再追加康熙差集（只追加，不改既有行序/词频），交给
opi-tools compile 重新排序序列化。可重复执行；若 luna 已含差集，输出与输入等价
（compile 会去重同一 (pinyin, word)）。

用法：python3 scripts/merge_luna_kangxi.py [--luna <opid>] [--extra <tsv>]
"""
import argparse
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_LUNA = REPO / "android/app/src/main/assets/luna.opid"
DEFAULT_EXTRA = REPO / "data/raw/kangxi_luna.tsv"


def read_opid(path: Path) -> list[tuple[str, str, int]]:
    data = path.read_bytes()
    if data[:4] != b"OPID":
        raise SystemExit(f"不是 OPID：{path}")
    if struct.unpack_from("<H", data, 4)[0] != 1:
        raise SystemExit(f"OPID 版本不是 1：{path}")
    count = struct.unpack_from("<I", data, 7)[0]
    table = 11
    pinyin_total = 0
    entries = []
    for i in range(count):
        off = table + i * 14
        po, pl, wo, wl = struct.unpack_from("<IBIB", data, off)
        freq = struct.unpack_from("<I", data, off + 10)[0]
        pinyin_total = max(pinyin_total, po + pl)
        entries.append((po, pl, wo, wl, freq))
    pinyin_at = table + count * 14
    word_at = pinyin_at + pinyin_total
    out = []
    for po, pl, wo, wl, freq in entries:
        py = data[pinyin_at + po : pinyin_at + po + pl].decode("utf-8")
        w = data[word_at + wo : word_at + wo + wl].decode("utf-8")
        out.append((w, py, freq))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--luna", type=Path, default=DEFAULT_LUNA)
    ap.add_argument("--extra", type=Path, default=DEFAULT_EXTRA)
    args = ap.parse_args()

    rows = read_opid(args.luna)
    extra_text = args.extra.read_text(encoding="utf-8") if args.extra.is_file() else ""
    extra = [line for line in extra_text.splitlines() if line.strip()]
    sys.stdout.write("\n".join(f"{w}\t{py}\t{f}" for w, py, f in rows))
    if rows:
        sys.stdout.write("\n")
    if extra:
        sys.stdout.write("\n".join(extra) + "\n")
    print(f"luna {len(rows)} 条 + 康熙差集 {len(extra)} 条", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
