#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 构建期把鸿蒙随包资产从 Android 资产目录复制到 rawfile（复制品不入库，见 .gitignore）。
# 鸿蒙桥真能装载的三张词库：
#   · luna.opid    完整拼音词库（opi_load）
#   · stroke.opid  笔画表（opi_load_stroke）
#   · wubi86.opid  五笔 86 表（opi_load_wubi；与 98 二选一，本端固定 86）
# 不复制 trad.opid（`opi_load_trad` 未进 Harmony 桥）与 wubi98.opid（同上二选一）。
# 宿主侧自己解析的两张文本资产（符号页 / 生僻字面板，不过引擎）：
#   · symbol_categories.tsv  22 类符号
#   · rare_index.tsv         Unihan 部首 + 笔画索引（1.1MB，面板首次打开时才读）
#
# rawfile 必须放在 `entry/src/main/resources/rawfile/`（**不是** `resources/base/rawfile`，
# 那个位置 hvigor 报 `Invalid resource directory name 'rawfile'`）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/android/app/src/main/assets"
DST="$ROOT/harmony/entry/src/main/resources/rawfile"
FILES=(luna.opid stroke.opid wubi86.opid symbol_categories.tsv rare_index.tsv)

mkdir -p "$DST"
for f in "${FILES[@]}"; do
  test -s "$SRC/$f" || { echo "[harmony-dict] 缺少源资产：$SRC/$f" >&2; exit 1; }
  if cmp -s "$SRC/$f" "$DST/$f"; then
    continue
  fi
  cp -f "$SRC/$f" "$DST/$f"
  cmp -s "$SRC/$f" "$DST/$f" || { echo "[harmony-dict] 复制后不一致：$f" >&2; exit 1; }
done
ls -l "$DST"
