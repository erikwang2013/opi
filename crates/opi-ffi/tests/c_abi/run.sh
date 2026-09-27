#!/bin/bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 真 C 消费者验证：拿 **macos/OpiFFI.h** 当声明面，用 clang 编一个 C 程序，
# 链接 `cargo build -p opi_ffi --release` 产出的 libopi_ffi.so，真调几个导出。
#
# 复跑：bash crates/opi-ffi/tests/c_abi/run.sh
# 传词库路径可换用完整词库：bash crates/opi-ffi/tests/c_abi/run.sh data/generated/luna.opid
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
nm -D --defined-only "$LIB" | awk '{print $3}' | grep '^opi_' | sort -u > "$OUT/exported.txt"
grep -o 'opi_[a-z_]*' "$SRC" | sort -u > "$OUT/called.txt"
UNCALLED=$(comm -23 "$OUT/exported.txt" "$OUT/called.txt")
if [ -n "$UNCALLED" ]; then
  echo "[注意] 以下导出未被 C 消费者调用（不判失败 —— 可能是新加的出口）："
  echo "$UNCALLED" | sed 's/^/    /'
else
  echo "[覆盖] 库里 $(wc -l < "$OUT/exported.txt") 个导出全部被 C 消费者调用过"
fi

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
