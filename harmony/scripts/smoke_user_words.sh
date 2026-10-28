#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 鸿蒙用户词 import/export 的本机冒烟（不需要设备/模拟器/签名）：
#   1) 真实 OHOS NDK clang 对 NAPI 桥做语法检查（-Wall -Wextra -Wpedantic）；
#   2) 按 CMakeLists 同参数把 bridge + Rust 静态库链成 aarch64 .so，
#      断言 opi_* 无未解析符号（两个新导出真的链进来了）；
#   3) 断言 importUserWords / exportUserWords / pendingText / switchLanguage 三处同名：
#      模块 desc[]（strings 读产物）、
#      index.d.ts、OpiEngine.ets —— desc[] 少名字只会在运行期 undefined，编译期不报。
# 真机行为（沙箱写权限、rename 覆盖、fsync 语义）仍只能在设备上验。
set -euo pipefail

HARMONY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CPP="$HARMONY/entry/src/main/cpp"
NDK="${OHOS_NDK:-/home/component/command-line-tools/sdk/default/openharmony/native}"
CLANG="$NDK/llvm/bin/aarch64-unknown-linux-ohos-clang"
NM="$NDK/llvm/bin/llvm-nm"
LIB="$CPP/libs/arm64-v8a/libopi_ffi.a"

test -x "$CLANG" || { echo "[smoke] 缺 OHOS NDK clang：$CLANG（可用 OHOS_NDK=... 覆盖）" >&2; exit 1; }
test -s "$LIB" || { echo "[smoke] 缺 Rust 静态库：$LIB（先按 harmony/README.md §0.2 编）" >&2; exit 1; }
test -x "$NM" || NM=nm

FLAGS=(--target=aarch64-linux-ohos --sysroot="$NDK/sysroot" -std=c11)
"$CLANG" "${FLAGS[@]}" -Wall -Wextra -Wpedantic -fsyntax-only -I"$CPP" "$CPP/napi_bridge.c"
echo "[smoke] PASS 语法检查（真实 NDK N-API 头）"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
"$CLANG" "${FLAGS[@]}" -shared -o "$TMP/libopiime.so" "$CPP/napi_bridge.c" "$LIB" \
    -lace_napi.z -lhilog_ndk.z -Wl,--gc-sections -Wl,--allow-multiple-definition -Wl,-s
# 先落文件再 grep：`... | grep -q` 在 pipefail 下会被 SIGPIPE 判红（-q 提前关管道），
# 反向还会把「真有未解析符号」误判成干净 —— 两种方向都不能要。
"$NM" -D --undefined-only "$TMP/libopiime.so" > "$TMP/undef.txt"
if grep -q ' opi_' "$TMP/undef.txt"; then
    echo "[smoke] FAIL 链接产物仍有未解析的 opi_* 符号：" >&2
    grep ' opi_' "$TMP/undef.txt" >&2
    exit 1
fi
echo "[smoke] PASS 链接（bridge + libopi_ffi.a → aarch64 .so，opi_* 全解析）"

strings "$TMP/libopiime.so" > "$TMP/strings.txt"
for name in importUserWords exportUserWords pendingText switchLanguage; do
    grep -qx "$name" "$TMP/strings.txt" \
        || { echo "[smoke] FAIL 模块 desc[] 缺 $name（运行期 undefined）" >&2; exit 1; }
    grep -q "export const $name" "$CPP/types/libopiime/index.d.ts" \
        || { echo "[smoke] FAIL index.d.ts 缺 $name" >&2; exit 1; }
    grep -q "opiime.$name" "$HARMONY/entry/src/main/ets/InputMethodExtensionAbility/OpiEngine.ets" \
        || { echo "[smoke] FAIL OpiEngine.ets 未转出 $name" >&2; exit 1; }
done
echo "[smoke] PASS 名字三面一致（desc[] / index.d.ts / OpiEngine.ets）"
