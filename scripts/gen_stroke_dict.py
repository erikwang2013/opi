#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""生成笔画词典 stroke.opid（移动端「笔画键盘」+「生僻字键盘」的按笔画查字）。

数据源：`rime/rime-stroke` 的 `stroke.dict.yaml`（五笔画：h 横 / s 竖 / p 撇 /
n 捺 / z 折，主码表源自 CNS11643 全字库，超集扩充至扩展 J）。**LGPL-3.0** ——
授权记录进 `data/raw/LICENSES.md`，随包分发时与 luna/trad 同一条规矩。

为什么能直接喂给 `opi-tools compile`：那个解析器按「第 0 列 = word、第 1 列 =
pinyin」的 rime 通用形状读（compiler.rs 的 parse_dict_report）—— stroke.dict.yaml
的每行恰好是 `字<TAB>笔画串`，于是产出 (key=笔画串, word=字) 的 .opid，正是引擎
`Scheme::Stroke` 要的形状。**不用任何改写**（改写一份就是多一个会漂的真源）。

产物：android/app/src/main/assets/stroke.opid（**入库**，随 Android 资产分发；
Linux 端不发它 —— 桌面三轨本轮没有笔画入口，见 docs/install/linux.md）。

用法（需要网络 + 本机能编 Rust）：
    cd <repo 根> && python3 scripts/gen_stroke_dict.py

升级路径：换 STROKE_COMMIT + 重算 SHA256（`curl -sSL <url> | sha256sum`）→ 重跑本
脚本 → 抽查几个字的笔画串（`opi-tools` 无查询子命令，用引擎的
`tests/scheme_keys.rs` 那套加载即可）。
"""

import hashlib
import subprocess
import sys
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "android/app/src/main/assets/stroke.opid"
RAW = REPO / "data/generated/stroke.dict.yaml"  # 未入库的本地下载副本

# 上游 pin（2026-10-09）。rime-stroke 的 master 也在动，钉 commit 的理由与
# gen_trad_dict.py 的 terra 逐字相同：浮动上游 = 产物不可复现。
STROKE_COMMIT = "1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48"
STROKE_URL = (
    f"https://raw.githubusercontent.com/rime/rime-stroke/{STROKE_COMMIT}"
    "/stroke.dict.yaml"
)
STROKE_SHA256 = "b3e93dce89c185f45c3d6e189b86b3a8626913352cc85e1094c786579a665791"


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=180) as r:
        return r.read()


def main() -> int:
    print(f"上游: {STROKE_URL}")
    data = fetch(STROKE_URL)
    got = hashlib.sha256(data).hexdigest()
    if got != STROKE_SHA256:
        print(
            f"!! 摘要与 pin 不符：期望 {STROKE_SHA256}，实得 {got}。\n"
            "   上游内容变了 —— 确认后更新本文件的 pin 再重跑。",
            file=sys.stderr,
        )
        return 1
    RAW.parent.mkdir(parents=True, exist_ok=True)
    RAW.write_bytes(data)
    print(f"已下载并校验：{RAW.relative_to(REPO)}（{len(data)} 字节）")

    # 编译（与音频词库同一条 CLI：opi-tools compile <输入> <输出>）。
    for step, args in (
        ("compile", ["compile", str(RAW), str(OUT)]),
        ("verify", ["verify", str(OUT)]),
    ):
        proc = subprocess.run(
            ["cargo", "run", "--release", "-p", "opi-tools", "--", *args],
            cwd=REPO,
            capture_output=True,
            text=True,
        )
        tail = "\n".join(
            line for line in proc.stdout.strip().splitlines() if not line.startswith(" ")
        )
        print(f"opi-tools {step}: exit={proc.returncode}\n{tail[-600:]}")
        if proc.returncode != 0:
            print(proc.stderr[-1200:], file=sys.stderr)
            return 1

    print(f"\n产物: {OUT.relative_to(REPO)}（{OUT.stat().st_size} 字节，已入库）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
