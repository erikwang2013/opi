#!/bin/bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 真 C 消费者验证：拿 **macos/OpiFFI.h** 当声明面，用 clang 编一个 C 程序，
# 链接 `cargo build -p opi_ffi --release` 产出的 libopi_ffi.so，真调几个导出。
#
# 复跑：bash crates/opi-ffi/tests/c_abi/run.sh
# 传词库路径可换用完整词库：bash crates/opi-ffi/tests/c_abi/run.sh android/app/src/main/assets/luna.opid
# （`data/generated/luna.opid` 是本地重编产物、**没入库**：在全新 clone 上这条
#   命令会以「词库不存在」退出 2。CI 传的是上面那份入库副本。）
#
# 为什么值得跑：`cargo test` 里的 ABI 测试是 **Rust 调 extern "C" fn** —— 那验的是
# 「Rust 以为的 ABI」，头文件写成什么样都照样绿。只有本脚本会**编译头文件**。
set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)
HDR_DIR="$ROOT/macos"
SRC="$ROOT/crates/opi-ffi/tests/c_abi/consumer.c"
OUT=$(mktemp -d)
trap 'rm -rf "$OUT"' EXIT

DICT=""
if [ -n "${1:-}" ]; then
  DICT="$ROOT/$1"
  [ -f "$DICT" ] || { echo "词库不存在：$DICT"; exit 2; }
fi

echo "=== [1/4] 构建 libopi_ffi.so（release）==="
cargo build -p opi_ffi --release --manifest-path "$ROOT/Cargo.toml" > "$OUT/build.log" 2>&1
rc=$?
if [ $rc -ne 0 ]; then
  echo "cargo build 失败（退出码 $rc）："; tail -30 "$OUT/build.log"; exit $rc
fi
LIB=$(ls "$ROOT"/target/release/libopi_ffi.so)
echo "ok: $LIB"

echo
echo "=== [2/4] clang -fsyntax-only 头文件（C 与 C++ 两种模式）==="
clang -fsyntax-only -Wall -Wextra -I "$HDR_DIR" "$SRC"; echo "C   : 退出码 $?"
clang++ -x c++ -fsyntax-only -Wall -Wextra -I "$HDR_DIR" "$SRC"; echo "C++ : 退出码 $?"

echo
echo "=== [3/4] 编译 + 链接消费者（-I 指向 macos/，即 Swift bridging header 那份）==="
clang -std=c11 -Wall -Wextra -Werror -I "$HDR_DIR" -o "$OUT/consumer" "$SRC" \
  -L "$ROOT/target/release" -lopi_ffi -lpthread -ldl -lm
rc=$?
if [ $rc -ne 0 ]; then echo "编译/链接失败（退出码 $rc）"; exit $rc; fi
echo "ok: $OUT/consumer"
echo
echo "链接到的导出（nm -D --defined-only）："
nm -D --defined-only "$LIB" | awk '{print $3}' | grep '^opi_' | sort | wc -l
nm -D --defined-only "$LIB" | awk '{print $3}' | grep '^opi_' | sort | tr '\n' ' '
echo
# 覆盖核对：库里导出的每个 opi_* 是否都在 consumer.c 里被**真的调用**过。
# 声明有了、C 侧也编得过，但没人调 = 链接期问题要等 Swift 那边才暴露。
#
# 这里**会判失败**。此前是打印一句「不判失败 —— 可能是新加的出口」就过去，
# 等于这条门禁永远不会红：新出口加完不补调用，CI 一路绿到用户那头。
nm -D --defined-only "$LIB" | awk '{print $3}' | grep '^opi_' | sort -u > "$OUT/exported.txt"
# 按**编译器眼里的引用**取（AST 里的 DeclRefExpr），不按文本匹配。
# 来历（别改回文本正则）：原先用 `grep -oE 'opi_[a-z_]+[[:space:]]*\('`，它能被
# 两种「最像改坏的改法」骗过去 —— ① 把调用**注释掉**（`// opi_foo();` 文本上照样
# 命中）；② printf 的说明串里出现 `opi_foo(`（本文件真有几处，见 `[6]` 那行）。
# 实测：把 [11c] 整段注释掉后文本法仍数出 34 条 ⇒ 门禁照绿。AST 里注释不产生
# 结点、字符串不产生 DeclRefExpr，且跨行调用也不再漏。
clang -Xclang -ast-dump -fsyntax-only -I "$HDR_DIR" "$SRC" 2> "$OUT/ast.err" \
  | grep -oE "DeclRefExpr.*Function 0x[0-9a-f]+ 'opi_[a-z_]+'" \
  | grep -oE "'opi_[a-z_]+'" | tr -d "'" | sort -u > "$OUT/called.txt"
# 抽不到就是命令坏了（clang 改了 dump 格式 / 解析失败），不是「没有调用」——
# 空名单会让下面把 34 条全报成未调用。这里先把它自己变成一条明确的红。
if [ ! -s "$OUT/called.txt" ]; then
  echo "!! 从 AST 里一条 opi_* 引用都没抽到 —— 抽取命令或 clang 的 ast-dump 格式变了："
  head -5 "$OUT/ast.err"
  exit 1
fi
UNCALLED=$(comm -23 "$OUT/exported.txt" "$OUT/called.txt")
# **零豁免**：库里的每一条导出都必须在 consumer.c 里有真调用点。
#
# 来历（别改回去）：这里原本是「打印一句『不判失败 —— 可能是新加的出口』就过去」，
# 等于这条门禁**永远不会红**；随后收成「显式豁免名单」，2026-09-28 三条标点导出
# 在 C 侧补上真调用后名单清空。以后若确需长期豁免，**不要**恢复成「不判失败」——
# 写明名单与理由，并让名单以外的一切照样红。
if [ -n "$UNCALLED" ]; then
  echo "[覆盖] 库里 $(wc -l < "$OUT/exported.txt") 条导出，以下未被 C 消费者调用："
  echo "$UNCALLED" | sed 's/^/    /'
  echo "  补法：在 crates/opi-ffi/tests/c_abi/consumer.c 里真调一次（只声明不算调用）。"
  exit 1
fi
echo "[覆盖] 库里 $(wc -l < "$OUT/exported.txt") 条导出全部被 C 消费者调用过"

echo
echo "=== [4/4] 运行 ==="
if [ -n "$DICT" ]; then
  LD_LIBRARY_PATH="$ROOT/target/release" "$OUT/consumer" "$DICT"
else
  LD_LIBRARY_PATH="$ROOT/target/release" "$OUT/consumer"
fi
rc=$?
echo "消费者退出码：$rc"
exit $rc
