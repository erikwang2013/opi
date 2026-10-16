#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成「康熙字典」词库 TSV（2026-10-16 用户裁决：「把 shunshi-kangxi-core 中康熙字典
提取词库，作项目适配词库」；分发面「全发」）。

产物：`data/raw/kangxi.tsv`  `字\\t拼音\\t词频`（与 fallback / 导入词库同一格式）。
消费面：四端共用的**导入词库契约**（`*.tsv` 放词库目录即被装载，见
desktop 的 `Dicts.kt`、fcitx5 的 `cpp/opi_dicts.h`、Android 的 `DictStore`）——
本脚本同时把部署副本写进 `android/app/src/main/assets/` 与 `crates/fcitx5-opi/data/`，
各端首启种子/导入取它。`.opid` 由 `opi-tools compile` 另出（入库，供引擎级门禁
`crates/engine-data/tests/kangxi_real.rs` 直接装载核字；**不进包** —— 用户在 Linux
上吃的是上面那条 TSV 导入路，别发没有消费方的二进制）。

数据源 pin：npm `shunshi-kangxi-core@0.1.1`（MIT，Shunshi.AI；数据同上游仓库
https://github.com/shunshi-ai/kangxi-mcp）。tarball 是不可变的（npm 重传会 410），
SHA-256 钉住 —— 换版本 = 换两个常量 + 重跑 + 过 `kangxi_real` 门禁。

## 适配口径（改之前先读）
* **每字取全部读音**：`chars[字].py` 是空格分隔的读音串（多音字如 `乄` = `shī  mē`，
  空串 = 无读音跳过）——全部展开成多条（字, 读音），不是只取第一个。
* **读音规范化**：NFC → `ü` 系（ǖǘǚǜ）→ `v`（本仓词库惯例 lv/nv/lve/nve，见
  `gen_luna_dict.py` 的 U_FIX 注）→ NFD 去声调组合符 → 少数 IPA 字母落位
  （`ɑ`→`a`、`ɡ`→`g`，上游偶用）→ 小写；**再过 `pinyin.rs::SYLLABLES` 音节表**
  （唯一真源，从源文件抽，别手抄）——不在表的读音丢弃并计数（上游数据的问题，
  不许悄悄进库）。
* **词频**：一个低位常数 [FREQ]。Kangxi 字绝大多数**不在主库**（进了也是「查得到」
  的补充），常数只决定「同读音时的稳定次序」与「与主库撞键时的取大合并」——
  取低不抢主库名次；口径与调法照 `polyphone.rs` 的缩放先例，要动先量。
* **许可**：MIT（保留版权声明）——登记在 `data/raw/LICENSES.md`。

用法：`python3 scripts/gen_kangxi_dict.py [--tarball <已下载的.tgz>]`
（不带 --tarball 时从 npm registry 下载并校验 SHA-256；确定性输出，重跑字节相同。）
"""

import argparse
import gzip
import hashlib
import io
import json
import re
import sys
import tarfile
import unicodedata
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# ---- 上游 pin（勿改；升级要连 LICENSES.md 与门禁一起走）----
PKG = "shunshi-kangxi-core"
VERSION = "0.1.1"
TARBALL_URL = f"https://registry.npmjs.org/{PKG}/-/{PKG}-{VERSION}.tgz"
TARBALL_SHA256 = "196e4de9c5447e5e2b456069441582fd72624225afea872c7e0fcd5431cc3434"

# ---- 适配常量 ----
# 低位常数词频 [FREQ]。Kangxi 字绝大多数**不在主库**（进了也是「查得到」的补充），
# 常数只决定「同读音时的稳定次序」与「与主库撞键时的取大合并」——
# **抢位不再靠数据**（见下「沿革」）。
#
# 沿革（2026-10-17 同日改定两次，末条为准）：
# · 曾经：4000 → 3.98e9。起因用户报「打不出 馫/蘒」——4000 档让康熙字沉在 luna
#   同音字的精确加成带之下（馫 xin 第 124 位、蘒 qiu 第 108 位，手机只抓 64）。
#   当时用「数据里写大词频」硬顶；代价如实记过：luna 3.95–3.98e9 段的中晚期单字
#   会被压到康熙字之后。
# · 现在：**回退 4000**。引擎侧落地了**导入层独有条目的机制性加成**
#   （`engine-core/src/dict_layers.rs::boosted_import_freq`，同日用户裁决
#   「是词库就要抢位」）：独有条目自动落在「主库量纲之顶 − 0.8%」的窄带里，馫蘒一族
#   不再依赖任何数据 hack；主库已有的重复项维持原「取大」语义、不受影响。
# 门禁：`crates/opi-ffi/tests/kangxi_reach.rs`——C ABI 逐字同构 Android 调用序列
# （load→import→逐键→candidates(64)），钉「抓取窗内可达」与「常用字头不受影响」。
FREQ = 4_000

# 两处部署副本（与 gen_symbol_categories.py 同一条「同一次运行、同一内容」纪律）
DEPLOY = (
    "android/app/src/main/assets/kangxi.tsv",
    "crates/fcitx5-opi/data/kangxi.tsv",
)

OUT_DEFAULT = REPO / "data/raw/kangxi.tsv"

# IPA 落位（上游偶用之；UnicodeData 里这些不是 Mn 组合符，NFD 去不掉）
IPA_FIX = {"ɑ": "a", "ɡ": "g", "ɛ": "e", "ɔ": "o", "ʅ": "i", "ʮ": "ü"}
# üe 记法归一：上游把 lüe/nüe 写成 lue/nue，本仓引擎规范是 lve/nve —— 与
# gen_luna_dict.py 的 U_FIX **同一对**（那边注了「不可盲目 lu→lv」：路/吕 是不同音节，
# 上游也确实分开写；只映射 üe 这一对）。
UE_FIX = {"lue": "lve", "nue": "nve"}
# ü 系（含声调形）→ v —— **必须在 NFD 之前**：分解后 u 上的分音符与声调都是
# 组合符，按「去组合符」一刀切会把 lü 的 ü 去成 u（路/吕 就撞了，真错）。
V_FIX = {c: "v" for c in "üǖǘǚǜ"}


def syllables() -> set[str]:
    """合法音节集 = 引擎侧那张表的**唯一真源**（从 pinyin.rs 抽，别手抄）。
    与 gen_polyphone.py 同一手法（两处都跟着源文件走，改源必两边同醒）。"""
    src = (REPO / "crates/engine-core/src/pinyin.rs").read_text(encoding="utf-8")
    body = src.split("pub const SYLLABLES", 1)[1].split("];", 1)[0]
    return set(re.findall(r'"([a-z]+)"', body))


def normalize_reading(py: str) -> str:
    """上游读音 → 本仓词库拼音键（小写、无调、ü=v）。"""
    s = unicodedata.normalize("NFC", py).lower()
    s = "".join(V_FIX.get(c, c) for c in s)
    s = "".join(IPA_FIX.get(c, c) for c in s)
    s = unicodedata.normalize("NFD", s)
    s = "".join(c for c in s if not unicodedata.combining(c))
    s = unicodedata.normalize("NFC", s)
    return UE_FIX.get(s, s)


def fetch_tarball(path: str | None) -> bytes:
    if path:
        data = Path(path).read_bytes()
    else:
        with urllib.request.urlopen(TARBALL_URL, timeout=60) as r:  # noqa: S310（pin 校验在后）
            data = r.read()
    got = hashlib.sha256(data).hexdigest()
    if got != TARBALL_SHA256:
        sys.exit(f"!! {PKG}@{VERSION} tarball SHA-256 不符：{got}（表里 {TARBALL_SHA256}）")
    return data


def load_chars(tgz: bytes) -> dict:
    with tarfile.open(fileobj=io.BytesIO(tgz)) as t:
        f = t.extractfile("package/data/chars.json.gz")
        assert f is not None, "包里没有 data/chars.json.gz（上游结构变了）"
        return json.load(gzip.open(f))["chars"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(OUT_DEFAULT))
    ap.add_argument("--tarball", default=None, help="已下载的 .tgz（离线/慢网时用）")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    syl = syllables()
    chars = load_chars(fetch_tarball(args.tarball))

    rows: set[tuple[str, str]] = set()
    no_py = illegal = 0
    illegal_samples: list[str] = []
    for ch, rec in chars.items():
        if len(ch) != 1:
            continue  # 理论不会有；上游字形键按单字约束
        py = (rec or {}).get("py", "").strip()
        if not py:
            no_py += 1
            continue
        for raw in py.split():
            r = normalize_reading(raw)
            if r not in syl:
                illegal += 1
                if len(illegal_samples) < 10:
                    illegal_samples.append(f"{ch}={raw}→{r}")
                continue
            rows.add((ch, r))

    lines = [f"{ch}\t{r}\t{FREQ}" for ch, r in sorted(rows, key=lambda x: (ord(x[0]), x[1]))]
    out = "\n".join(lines) + "\n"
    Path(args.out).write_text(out, encoding="utf-8")
    for dest in DEPLOY:
        (REPO / dest).write_text(out, encoding="utf-8")

    print(f"写入 {args.out} + 两处部署副本（同一次运行、同一内容）")
    print(f"  字 {len(chars)} → 条 {len(rows)}（无读音跳过 {no_py}，读音不合法丢弃 {illegal}）")
    if illegal_samples:
        print(f"  非法读音样例：{illegal_samples}")
    if args.verbose:
        print(f"  音节表 {len(syl)} 条；样例：",
              [l for l in lines[:5]])
    return 0


if __name__ == "__main__":
    sys.exit(main())
