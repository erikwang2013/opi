#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""康熙字典行 → 两张「并入默认词库」的补充表（产物提交入库）。

用户裁决（2026-10-18）：「增加…康熙字典的词语，在输入的时候匹配」。
康熙包（shunshi-kangxi-core，MIT）只有**单字条目**（`chars.json`：字 + 全部读音），
没有多字词；`gen_kangxi_dict.py` 已经生成 `data/raw/kangxi.tsv`（20939 条「字-读音」）。
本脚本把它与项目现有两张默认库对照，只把**库里还没有的 (字, 读音)** 抽出来，
且**只收非 GB2312 字**（常用字的读音以主库为准；康熙里的异读如 娘→nian 会把
「娘」塞进钉 `nian` 的九键候选，2026-10-18 被 `pin_reading_real` 当场红）：

  data/raw/kangxi_trad.tsv   → 并入 trad.opid（繁体模式；iOS/鸿蒙没有导入词库通道）
  data/raw/kangxi_luna.tsv   → 并入 luna.opid（简体模式；让「首启种子被删/未导入」也能查）

为什么不直接把 kangxi.tsv 整个追加：那会把已有的常用字读音重复一遍（同一 (字,拼音)
在 compile 时只留先出现者，词频取先到，等于白占体积），也会让导入层与内置层各存
一份。这里只补差集；数一下：luna 1279 条、trad 622 条（本次实测，生成器每次打印）。

词频常数 KX_FREQ = 1e9：实测（同进程、合并前后对照）把「乄 me」放在第 9 位、
「丆 o」第 6 位，常用字头（麼/哦）仍在第 1；4000 会掉到 64 窗之外，3.95e9 会把
常用字头顶掉。这个值是排序取舍，不是上游数据；改它先跑 `trad_modern_reach`
门禁与 docs/performance.md 的复量。

用法：python3 scripts/gen_kangxi_extra.py [--luna <合并前的 luna.opid>]
（离线；只读入库产物）。⚠️ 默认读部署副本；若它已经并过差集，脚本会算出 0 条 ——
重生成时请用合并前的基线（例如上一个 release 的 `android/app/src/main/assets/luna.opid`）。
"""
import argparse
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from hanzi_freq import gb2312_levels  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
KANGXI = REPO / "data/raw/kangxi.tsv"
TRAD = REPO / "data/raw/trad_hanzi.tsv"
LUNA = REPO / "android/app/src/main/assets/luna.opid"
OUT_TRAD = REPO / "data/raw/kangxi_trad.tsv"
OUT_LUNA = REPO / "data/raw/kangxi_luna.tsv"

KX_FREQ = 1_000_000_000


def tsv_pairs(path: Path) -> set[tuple[str, str]]:
    out = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        cols = line.split("\t")
        if len(cols) >= 2 and cols[0] and cols[1]:
            out.add((cols[0], cols[1]))
    return out


def opid_pairs(path: Path) -> set[tuple[str, str]]:
    data = path.read_bytes()
    if data[:4] != b"OPID":
        raise SystemExit(f"不是 OPID：{path}")
    count = struct.unpack_from("<I", data, 7)[0]
    table = 11
    pinyin_total = 0
    entries = []
    for i in range(count):
        off = table + i * 14
        po, pl, wo, wl = struct.unpack_from("<IBIB", data, off)
        pinyin_total = max(pinyin_total, po + pl)
        entries.append((po, pl, wo, wl))
    pinyin_at = table + count * 14
    word_at = pinyin_at + pinyin_total
    out = set()
    for po, pl, wo, wl in entries:
        py = data[pinyin_at + po : pinyin_at + po + pl].decode("utf-8")
        w = data[word_at + wo : word_at + wo + wl].decode("utf-8")
        out.add((w, py))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description="生成康熙差集（trad/luna）")
    ap.add_argument(
        "--luna",
        type=Path,
        default=LUNA,
        help="用于求差集的 luna.opid；部署副本已含差集时请指向合并前的基线",
    )
    args = ap.parse_args()

    kx = []
    for line in KANGXI.read_text(encoding="utf-8").splitlines():
        cols = line.split("\t")
        if len(cols) >= 2 and cols[0] and cols[1]:
            kx.append((cols[0], cols[1]))
    # 只给**非 GB2312 字**补新读音（2026-10-18 由 pin_reading_real 红出来）：
    # 康熙数据含少量与常用字冲突的异读（例：娘 → nian），把它当新读音塞进 luna 会让
    # 九键钉 `nian` 时冒出「娘」（本该是 niang）。GB2312 常用字的读音以主库为准；
    # 康熙只负责生僻字（乄/丆/馫…）。
    first, second = gb2312_levels()
    gb = set(first) | set(second)
    trad_have = tsv_pairs(TRAD)
    luna_have = opid_pairs(args.luna)

    trad_extra = [(w, py) for w, py in kx if w not in gb and (w, py) not in trad_have]
    luna_extra = [(w, py) for w, py in kx if w not in gb and (w, py) not in luna_have]
    # 去重（上游同字同音出现两次也不该写两行）
    trad_extra = list(dict.fromkeys(trad_extra))
    luna_extra = list(dict.fromkeys(luna_extra))

    OUT_TRAD.write_text(
        "".join(f"{w}\t{py}\t{KX_FREQ}\n" for w, py in trad_extra), encoding="utf-8"
    )
    OUT_LUNA.write_text(
        "".join(f"{w}\t{py}\t{KX_FREQ}\n" for w, py in luna_extra), encoding="utf-8"
    )
    print(f"康熙条目 {len(kx)}；新增 trad {len(trad_extra)} / luna {len(luna_extra)}")
    print(f"写入 {OUT_TRAD.relative_to(REPO)}、{OUT_LUNA.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
