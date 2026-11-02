#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# HarmonyOS 语音输入的本机构建准备：
#   1. 交叉编译 opi-asr（cdylib）与 opi_ffi（staticlib），暂存到 HAP 会打包的位置；
#   2. 取 sherpa-onnx v1.12.9 的 OHOS arm64 预编译库（sha256 与 sherpa-rs-sys 0.6.8
#      checksum.txt 一致），连同 NDK 的 libc++_shared 一起放进 entry/libs/arm64-v8a；
#   3. normal 档清掉 rawfile/asr（模型按需下载）；full 档把 SenseVoice + VAD
#      放进 rawfile/asr（含 manifest.json），由首启拷贝到沙箱。
#
# 用法：scripts/prepare_harmony_asr.sh [normal|full]
# 环境：HARMONY_CLI_HOME（默认 /home/component/command-line-tools）
#       OPI_SHERPA_OHOS_DIR（已解压的 sherpa OHOS 目录，省一次下载）
#       OPI_ASR_MODELS_DIR（已备好的模型目录，结构同 rawfile/asr）
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIER="${1:-normal}"
case "$TIER" in
  normal|full) ;;
  *) echo "usage: $0 [normal|full]" >&2; exit 2 ;;
esac

CLI_HOME="${HARMONY_CLI_HOME:-/home/component/command-line-tools}"
NDK="$CLI_HOME/sdk/default/openharmony/native"
BIN="$NDK/llvm/bin"
SYSROOT="$NDK/sysroot"
TARGET="aarch64-unknown-linux-ohos"
CACHE="${OPI_ASR_CACHE:-$ROOT/target/harmony-asr}"
SHERPA_TAG="v1.12.9"
SHERPA_ARCHIVE="sherpa-onnx-$SHERPA_TAG-ohos-arm64-v8a.tar.bz2"
SHERPA_SHA="f899ef7fe027c0cd3c7559cd465273a4bef10dd301fc43626a2b2e78e7ae44b2"
SHERPA_DIR="${OPI_SHERPA_OHOS_DIR:-$CACHE/sherpa-onnx-$SHERPA_TAG-ohos-arm64-v8a}"

test -x "$BIN/aarch64-unknown-linux-ohos-clang" || {
  echo "[harmony-asr] 找不到 OHOS NDK：$NDK" >&2; exit 1
}

# ---- sherpa-onnx OHOS 预编译库 ------------------------------------------------
if [ ! -f "$SHERPA_DIR/lib/libsherpa-onnx-c-api.so" ]; then
  mkdir -p "$CACHE"
  if [ ! -f "$CACHE/$SHERPA_ARCHIVE" ]; then
    echo "[harmony-asr] 下载 $SHERPA_ARCHIVE"
    curl -fL --retry 5 --retry-delay 3 --connect-timeout 30 \
      "https://github.com/k2-fsa/sherpa-onnx/releases/download/$SHERPA_TAG/$SHERPA_ARCHIVE" \
      -o "$CACHE/$SHERPA_ARCHIVE"
  fi
  echo "$SHERPA_SHA  $CACHE/$SHERPA_ARCHIVE" | sha256sum -c -
  tar -xjf "$CACHE/$SHERPA_ARCHIVE" -C "$CACHE"
fi

# sherpa-rs-sys 对 target_os=linux 加 -lstdc++，OHOS 只带 libc++；补一个同内容
# 的 libstdc++.so（SONAME 仍是 libc++_shared.so，运行时依赖不会指错）。
if [ ! -f "$SHERPA_DIR/lib/libc++_shared.so" ]; then
  cp "$NDK/llvm/lib/aarch64-linux-ohos/libc++_shared.so" "$SHERPA_DIR/lib/libc++_shared.so"
fi
[ -e "$SHERPA_DIR/lib/libstdc++.so" ] || ln -s libc++_shared.so "$SHERPA_DIR/lib/libstdc++.so"

# ---- Rust 交叉编译 ------------------------------------------------------------
export CC_aarch64_unknown_linux_ohos="$BIN/aarch64-unknown-linux-ohos-clang"
export AR_aarch64_unknown_linux_ohos="$BIN/llvm-ar"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="$BIN/aarch64-unknown-linux-ohos-clang"
export BINDGEN_EXTRA_CLANG_ARGS="--target=aarch64-linux-ohos --sysroot=$SYSROOT -I$SYSROOT/usr/include -I$SYSROOT/usr/include/aarch64-linux-ohos"

cd "$ROOT"
cargo build -p opi_ffi --release --target "$TARGET"
# download-binaries 的 dist.json 没有 OHOS 目标，必须显式关掉后走 SHERPA_LIB_PATH。
# cdylib 自己没有 RUNPATH：加 $ORIGIN，让它和同目录的 sherpa/onnxruntime 互相解析，
# 不依赖加载器是否继承主库的搜索路径。
SHERPA_LIB_PATH="$SHERPA_DIR" \
  RUSTFLAGS='-C link-arg=-Wl,-rpath,$ORIGIN' \
  cargo build -p opi-asr --release --target "$TARGET" --no-default-features --features sherpa

CPP_LIB="$ROOT/harmony/entry/src/main/cpp/libs/arm64-v8a"
APP_LIB="$ROOT/harmony/entry/libs/arm64-v8a"
mkdir -p "$CPP_LIB" "$APP_LIB"
cp -f "$ROOT/target/$TARGET/release/libopi_ffi.a" "$CPP_LIB/libopi_ffi.a"
for lib in libopi_asr.so libsherpa-onnx-c-api.so libonnxruntime.so libc++_shared.so; do
  if [ "$lib" = "libopi_asr.so" ]; then
    cp -f "$ROOT/target/$TARGET/release/$lib" "$APP_LIB/$lib"
  else
    cp -f "$SHERPA_DIR/lib/$lib" "$APP_LIB/$lib"
  fi
done
ls -l "$CPP_LIB/libopi_ffi.a" "$APP_LIB"

# ---- 模型档位 ----------------------------------------------------------------
RAW="$ROOT/harmony/entry/src/main/resources/rawfile/asr"
if [ "$TIER" = "normal" ]; then
  rm -rf "$RAW"
  echo "[harmony-asr] normal：不随包模型（设置页按需下载）"
  exit 0
fi

SV="https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main"
VAD="https://gh-proxy.com/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx"
SRC="${OPI_ASR_MODELS_DIR:-}"

# 文件已存在且哈希正确就不再联网；源目录（OPI_ASR_MODELS_DIR）优先。
ensure() { # url rel sha
  local dest="$RAW/$2"
  mkdir -p "$(dirname "$dest")"
  if [ -n "$SRC" ] && [ -f "$SRC/$2" ]; then
    cp -f "$SRC/$2" "$dest"
  fi
  if [ -f "$dest" ] && echo "$3  $dest" | sha256sum -c --status; then
    return 0
  fi
  echo "[harmony-asr] 下载 $2"
  rm -f "$dest"
  curl -fL --retry 5 --retry-delay 3 --connect-timeout 30 "$1" -o "$dest"
  echo "$3  $dest" | sha256sum -c -
}

ensure "$SV/model.int8.onnx" sensevoice/model.int8.onnx \
  c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51
ensure "$SV/tokens.txt" sensevoice/tokens.txt \
  f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc
ensure "$VAD" vad/silero_vad.onnx \
  9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6

# 与 crates/opi-asr/src/manifest.rs 的 schema 逐字段一致；下载/内置共用。
cat > "$RAW/manifest.json" <<'JSON'
{"version":1,"files":[
{"path":"sensevoice/model.int8.onnx","size":239233841,"sha256":"c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51","role":"model"},
{"path":"sensevoice/tokens.txt","size":315894,"sha256":"f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc","role":"tokens"},
{"path":"vad/silero_vad.onnx","size":643854,"sha256":"9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"}
]}
JSON
du -sh "$RAW"
