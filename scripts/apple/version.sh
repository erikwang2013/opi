#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 打印工作区版本号（`Cargo.toml` 的 `[workspace.package] version`）。
#
# 为什么单独一个脚本：Apple 打包这条链上**三处**要用它（Rust 产物命名、macOS bundle/包、
# iOS 产物），而版本号只能有**一个**真源。取法写三遍就是三份会各自漂的实现 ——
# 本仓库在「同一语义抄多份」上栽过多次（键路由表抄三份那条老账）。
#
# 取不到时**响的红**（非零退出 + stderr），绝不打印空值：空版本会让产物名变成
# `opi-apple-rust-v.zip` 这种缺一段的东西，而「名字里少一截」在 artifact 列表里不显眼。
#
# 用法：
#   scripts/apple/version.sh          # → 1.7.4
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
f="$root/Cargo.toml"

# 只认**顶层**那一行。成员 crate 里写的是 `version.workspace = true`（匹配不上 `^version = "`），
# 所以顶层恰好一条就是判据；条数不对说明键被改名/挪走了，取法已失效 —— 红，别猜。
n="$(grep -c '^version = "' "$f" || true)"
[ "$n" = 1 ] || {
  echo "!! $f 顶层匹配到 $n 条 'version = \"'（期望恰好 1 条）—— 取法已失效" >&2
  exit 1
}
v="$(sed -n 's/^version = "\(.*\)"/\1/p' "$f")"
# 形态当场认：产物名里出现别的形态（区间/带后缀/空）都是没料到的情况。
grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' <<<"$v" || {
  echo "!! 版本号形态不认识: '$v'（期望 x.y.z）" >&2
  exit 1
}
printf '%s\n' "$v"
