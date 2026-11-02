#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 把 `macos/` 的 Swift + Rust 通用库组装成**可装**的 macOS 输入法：
#
#   dist/apple/macos/opi-<版本>-macos-universal.pkg   （**唯一产物**：内含完整 .app）
#
# `.app` / pkgroot / payload 清单只是构建期中间物，结尾清掉（用户 2026-10-21 裁决：
# macOS 端所有东西打进一个可安装包，不要额外文件）。
#
# 用法（仓库根目录）：
#   scripts/apple/package-macos.sh <版本号> [产物目录]
#   版本号用 scripts/apple/version.sh 取（CI 里就是这么调的）。
#   档位由环境变量 OPI_MACOS_TIER 选择：normal（默认，模型按需下载）/ full（内置模型）。
#
# ===== 语音输入（2026-11-02）=====
#
#   宿主进程（IMK）直接链 opi_asr + sherpa-onnx/onnxruntime 动态库。两个上游库由
#   sherpa-rs 的 macOS 预编译包提供（默认动态链接），脚本按 libopi_asr.dylib 的
#   `otool -L` 实际依赖收集，改 @rpath 后放进 Contents/Frameworks 并逐个签名。
#   full 档把 `desktop/asr-bundle/asr` 复制到 Contents/Resources/asr，normal 不复制。
#   宿主 Info.plist 需含 NSMicrophoneUsageDescription；本脚本不改 Swift 源码。
#
# ===== ⚠️ 未签名 / 未公证：产物是**开发用**的，不是可发布的 =====
#
# 本脚本**没有** Developer ID 证书可用（仓库不带 secrets，也不连任何签名服务），
# 只做 **ad-hoc 签名**（`codesign --sign -`）。后果如实写在这里：
#   · 本机装得上（`installer` 会警告「未签名」，要 `-allowUntrusted`）
#   · **不能**发给别人 —— 别人机器上 Gatekeeper 直接拦（「已损坏，无法打开」多半就是这个）
#   · 正式发布需要 Apple 开发者账号：Developer ID 证书 + `notarytool` 公证 + `stapler` 装订。
#     三件都不在本脚本范围内，缺了它们产物**只在本机有效**。
#
# ===== 前置：Rust 侧产物（本脚本**不**编 Rust，只读它的产物 + 自己 lipo）=====
#
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
#   cargo build --release -p opi_ffi --target aarch64-apple-darwin
#   cargo build --release -p opi_ffi --target x86_64-apple-darwin
#   cargo build --release -p opi-asr --target aarch64-apple-darwin
#   cargo build --release -p opi-asr --target x86_64-apple-darwin
#
# 库是**通用**的（两个 arch），所以下面 app 也编两个 arch 再 lipo —— 否则会得到
# 「通用 dylib + 单架构 app」这种半成品，而它在 `file` 输出里不显眼。

set -euo pipefail

version="${1:?用法: scripts/apple/package-macos.sh <版本号> [产物目录]}"
out="${2:-dist/apple/macos}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

# ---- 本端常量（只有一个地方写） ----
EXE="OpiInputMethod"                     # 可执行名 == Info.plist 的 CFBundleExecutable
MODULE="OpiInputMethod"                  # Swift 模块名（-module-name）
APP="$out/$EXE.app"
# bundle id：README 的示例只写了 `xyz.erik.opi`；这里加 `.inputmethod` 后缀，
# 理由是它要**唯一**（多个输入法撞 id 会让系统选错），且给将来可能的宿主 App 留位。
# 这是本脚本的选择，不是仓库约定。
BUNDLE_ID="xyz.erik.opi.inputmethod"
# 控制器类名：**裸名**，不带模块前缀。
# 理由（可复核，不是猜）：`InputController.swift` 上有 `@objc(OpiInputController)`，
# 这个属性把 ObjC 运行时名**逐字**定成裸名；而 `NSClassFromString` 查的就是运行时名
# ⇒ 写 `OpiInputMethod.OpiInputController` 是查不到的。plist 模板里那句
# `$(PRODUCT_MODULE_NAME).OpiInputController` 是「没有 @objc 改名时」的写法。
# ⚠️ 本判断只在**运行时**能最终确认（要真把输入法装进系统看它能不能实例化控制器）——
# 本机没有 macOS，故如实标注：**未经运行验证**。
CONTROLLER="OpiInputController"
# 部署目标：仓库**没有**声明 macOS 最低版本（`macos/README.md` 明写了这件事）。
# 11.0 = Apple Silicon 的第一个 macOS，且覆盖 InputMethodKit 全部用到的 API。本脚本的选择。
DEPLOY="11.0"
# 模型档位：normal=按需下载；full=内置模型。CI 的 tier 输入经环境变量传进来。
TIER="${OPI_MACOS_TIER:-normal}"
case "$TIER" in
  normal|full) ;;
  *) echo "!! OPI_MACOS_TIER 只能是 normal 或 full（现在是 '$TIER'）" >&2; exit 1 ;;
esac
# 设置/关于页英文表（由 shared/i18n/zh-en.tsv 生成）：缺了英文界面会整体回落中文。
[ -f macos/en.lproj/Localizable.strings ] || {
  echo "!! 缺少 macos/en.lproj/Localizable.strings（见 shared/i18n/zh-en.tsv）" >&2; exit 1; }

# 通用 dylib：从两个 arch 的 cargo 产物当场 lipo（缺哪边就**响**，不静默退化成单架构）。
# 落在 target/ 下而不是 $out 下 —— 后者会被上面的 rm -rf 清掉。
#
# ⚠️ 两个名字各管一段：clang/swiftc 认 `arm64`，cargo/rustc 认 `aarch64` —— **别互抄**。
# 首轮 CI（run 37933196475）就红在这里：脚本按 `arm64-apple-darwin` 去找产物，
# 而 cargo 落在 `target/aarch64-apple-darwin/`，于是「产物明明在，却报缺少」。
# spec = clang arch:cargo 三元组。
for spec in "arm64:aarch64-apple-darwin" "x86_64:x86_64-apple-darwin"; do
  rustt="${spec#*:}"
  f="target/$rustt/release/libopi_ffi.dylib"
  [ -f "$f" ] || {
    echo "!! 缺少 $f —— 先 cargo build --release -p opi_ffi --target $rustt" >&2
    exit 1
  }
done
mkdir -p target/apple/macos
lipo -create target/aarch64-apple-darwin/release/libopi_ffi.dylib \
            target/x86_64-apple-darwin/release/libopi_ffi.dylib \
  -output target/apple/macos/libopi_ffi.dylib
libdir="target/apple/macos"
dylib="$libdir/libopi_ffi.dylib"

# install name 必须在**链接 app 之前**改掉，且只改我们这份副本（cargo 产物不动）。
# 原因：链接器把 dylib 的 install name（LC_ID_DYLIB）原样抄进 app 的加载命令 ——
# 若那份写着**构建机上的绝对路径**（cargo cdylib 的常见形态），装到别人机器上就是
# `dyld: Library not loaded: /Users/runner/work/...`，且这个错在 CI 里**看不见**
# （本机路径存在、链接、签名、pkgbuild 全绿）。改成 @rpath/... 后由 app 的
# LC_RPATH（@executable_path/../Frameworks）解析到 bundle 内那份，下面第 5 节有断言钉住。
install_name_tool -id @rpath/libopi_ffi.dylib "$dylib"

# ---- 语音：通用 libopi_asr.dylib + sherpa-onnx/onnxruntime 动态依赖 ----
# sherpa-rs 默认在 macOS 动态链接，并把共享库复制到 target/<triple>/release/。
# 先 lipo 出通用 opi_asr，再按它的实际 otool 依赖名找同目录文件（不写死上游文件名）。
for rustt in aarch64-apple-darwin x86_64-apple-darwin; do
  f="target/$rustt/release/libopi_asr.dylib"
  [ -f "$f" ] || {
    echo "!! 缺少 $f —— 先 cargo build --release -p opi-asr --target $rustt" >&2
    exit 1
  }
done
lipo -create target/aarch64-apple-darwin/release/libopi_asr.dylib \
            target/x86_64-apple-darwin/release/libopi_asr.dylib \
  -output "$libdir/libopi_asr.dylib"
install_name_tool -id @rpath/libopi_asr.dylib "$libdir/libopi_asr.dylib"
asr_fw_libs=("$libdir/libopi_asr.dylib")
idx=0
while [ "$idx" -lt "${#asr_fw_libs[@]}" ]; do
  f="${asr_fw_libs[$idx]}"; idx=$((idx + 1))
  while IFS= read -r dep; do
    name="${dep##*/}"
    case "$dep" in
      /usr/lib/*|/System/Library/*|/Library/Frameworks/*) continue ;;
      /*|@*) ;;
      *) continue ;;
    esac
    case " ${asr_fw_libs[*]} " in *" $libdir/$name "*) continue ;; esac
    # sherpa-rs 把共享库放在 target/<triple>/release；按实际依赖名找，不写死版本号。
    src=""
    for d in target/aarch64-apple-darwin/release target/x86_64-apple-darwin/release \
             target/aarch64-apple-darwin/release/deps target/x86_64-apple-darwin/release/deps; do
      [ -f "$d/$name" ] && { src="$d/$name"; break; }
    done
    # sherpa-rs 把 osx-universal2 共享包放在下载缓存里，不一定复制进 target。
    [ -n "$src" ] || src="$(find "$HOME/Library/Caches/sherpa-rs" -type f -name "$name" 2>/dev/null | head -1)"
    [ -n "$src" ] || {
      echo "!! libopi_asr 的依赖 $dep 找不到对应动态库（先编 -p opi-asr）" >&2
      exit 1
    }
    cp "$src" "$libdir/$name"
    asr_fw_libs+=("$libdir/$name")
    # 上游是 osx-universal2 共享包；万一切片不全，用另一架构副本补齐。
    if ! lipo -archs "$libdir/$name" | grep -qw x86_64; then
      other="target/x86_64-apple-darwin/release/$name"
      [ -f "$other" ] || other="target/aarch64-apple-darwin/release/$name"
      [ -f "$other" ] && [ "$other" != "$src" ] || {
        echo "!! $name 缺 x86_64 切片" >&2; exit 1; }
      lipo -create "$src" "$other" -output "$libdir/.$name.universal"
      mv "$libdir/.$name.universal" "$libdir/$name"
    fi
    lipo -archs "$libdir/$name" | grep -qw arm64 || {
      echo "!! $name 缺 arm64 切片" >&2; exit 1; }
  done < <(otool -L "$f" | awk '/^[[:space:]]/ {print $1}')
done
[ "${#asr_fw_libs[@]}" -gt 1 ] || {
  echo "!! libopi_asr 没有 sherpa-onnx/onnxruntime 动态依赖；sherpa-rs 是否被编成静态？" >&2
  exit 1
}
# 把对内依赖与各自 install_name 改成 @rpath，避免把构建机绝对路径带进用户机器。
for f in "${asr_fw_libs[@]}"; do
  while IFS= read -r dep; do
    name="${dep##*/}"
    [ "$dep" = "@rpath/$name" ] && continue
    [ -f "$libdir/$name" ] && install_name_tool -change "$dep" "@rpath/$name" "$f"
  done < <(otool -L "$f" | awk '/^[[:space:]]/ {print $1}')
  install_name_tool -id "@rpath/${f##*/}" "$f"
done

rm -rf "$out"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"

sdk="$(xcrun --sdk macosx --show-sdk-path)"
# 桥接头就是仓库里那份声明面（不给 Xcode 留「第二份声明」的口子）。
flags=(-import-objc-header macos/OpiFFI.h -module-name "$MODULE" -sdk "$sdk" -O)

# ---- 1) 先 typecheck，再真编 ----
# 分两步是有用的：typecheck 不产生文件、结论直指源码错，比链接期报错好读得多。
# 两个 arch 的源码错是同一批，所以 typecheck 只跑一遍（宿主 arch）。
swiftc "${flags[@]}" -typecheck macos/*.swift shared/apple/OpiClipRunner.swift

for arch in arm64 x86_64; do
  swiftc "${flags[@]}" -target "$arch-apple-macosx$DEPLOY" macos/*.swift \
    shared/apple/OpiClipRunner.swift \
    -o "$out/.$EXE.$arch" \
    -L "$libdir" -lopi_ffi -lopi_asr \
    "${asr_fw_libs[@]:1}" \
    -framework Cocoa -framework InputMethodKit -framework AVFoundation \
    -Xlinker -rpath -Xlinker @executable_path/../Frameworks
done
lipo -create "$out/.$EXE.arm64" "$out/.$EXE.x86_64" -output "$APP/Contents/MacOS/$EXE"
rm -f "$out/.$EXE.arm64" "$out/.$EXE.x86_64"

# ---- 2) Info.plist：把 Xcode 占位符展开成字面值 ----
# 模板里的 `$(...)` 是非 Xcode 构件系统**不展开**的变量；不展开就装出一个
# CFBundleExecutable 字面写着 `$(PRODUCT_NAME)` 的包（系统连可执行文件都找不到）。
cp macos/Info.plist "$APP/Contents/Info.plist"
plist="$APP/Contents/Info.plist"
plutil -replace CFBundleExecutable -string "$EXE" "$plist"
plutil -replace CFBundleIdentifier -string "$BUNDLE_ID" "$plist"
plutil -replace InputMethodServerControllerClass -string "$CONTROLLER" "$plist"
plutil -lint "$plist" >/dev/null
# 兜底门禁：bundle 里**不许**剩任何未展开的占位符（上面漏替换一个键就是这条红）。
# ⚠️ 判据必须落在**值**上，不能直接 grep 原文：模板的 XML 注释里也写着 `$(` 例子
# （macos/Info.plist 的说明段），grep 原文会把注释当漏替换 → 假红。
# `plutil -p` 只吐解析后的键值，注释自然没了。
plist_dump="$(plutil -p "$plist")"
# 门禁自身的兜底：拿不到 dump 说明门禁失效 —— 红，别静默通过（本仓记过这类账）。
[ -n "$plist_dump" ] || { echo "!! plutil -p 没输出，占位符门禁失效" >&2; exit 1; }
if grep -q '\$(' <<<"$plist_dump"; then
  echo "!! bundle 的 Info.plist 的**值**里还有没展开的 Xcode 占位符：" >&2
  grep '\$(' <<<"$plist_dump" >&2
  exit 1
fi
# 版本号**不从命令行注入**：Info.plist 里的 CFBundleShortVersionString 是仓库的
# 另一个同步位（`macos/README.md` 记着「它曾经落后过仓库版本」）。这里只**对账**，
# 不改写 —— 改写会让「plist 漂了」这件事在打包时被静默抹掉。
plist_ver="$(plutil -extract CFBundleShortVersionString raw "$plist")"
[ "$plist_ver" = "$version" ] || echo "⚠️ Info.plist 版本 $plist_ver ≠ 工作区版本 ${version}（发布清单里要同步；本脚本按仓库的规矩**不动** plist）"

# ---- 3) 放 Rust dylib 与图标 ----
cp "$dylib" "$APP/Contents/Frameworks/"
for f in "${asr_fw_libs[@]}"; do
  cp "$f" "$APP/Contents/Frameworks/"
done
# tsInputMethodIconFileKey 指向 opi.tiff —— 键在 plist 里，文件得在 Resources 里。
cp macos/opi.tiff "$APP/Contents/Resources/opi.tiff"
# 设置/关于页的英文表（由 shared/i18n/zh-en.tsv 生成；Swift 按 en.lproj 读回）。
mkdir -p "$APP/Contents/Resources/en.lproj"
cp macos/en.lproj/Localizable.strings "$APP/Contents/Resources/en.lproj/Localizable.strings"
# 应用包图标（Finder 里看到的那张，浅底小欧），CFBundleIconFile 指向它；
# 与上面那张必须保持透明的输入源模板图标是两码事。
cp desktop/icons/opi-pet.icns "$APP/Contents/Resources/opi-pet.icns"
# 符号面板类目表（22 类）：与 Linux `/usr/share/opi/`、Windows 启动器旁同一份
# 真源文件。Swift 按 `Bundle.main.url(forResource:"symbol_categories")` 读它。
cp crates/fcitx5-opi/data/symbol_categories.tsv "$APP/Contents/Resources/symbol_categories.tsv"
# 表情/颜文字（`OpiEmojiData.loadEmoji/loadKaomoji`）与贴纸映射/资源。
# 与 Android 的 `assets/`、Linux 的共享数据同一份真源；缺文件时面板如实为空。
cp data/raw/emoji.tsv "$APP/Contents/Resources/emoji.tsv"
cp data/raw/kaomoji.tsv "$APP/Contents/Resources/kaomoji.tsv"
cp shared/pet/emoji-font/map.tsv "$APP/Contents/Resources/pet-map.tsv"
mkdir -p "$APP/Contents/Resources/pet/stickers" \
         "$APP/Contents/Resources/pet/stickers-gif"
cp shared/pet/stickers/*.png "$APP/Contents/Resources/pet/stickers/"
cp shared/pet/stickers-gif/*.gif "$APP/Contents/Resources/pet/stickers-gif/"
# 动态皮肤插画（2026-10-31）：与其它端同一份生成产物。
scenes_src="shared/scene/art"
[ -d "$scenes_src" ] || { echo "!! 缺少插画资产 $scenes_src" >&2; exit 1; }
mkdir -p "$APP/Contents/Resources/scenes"
cp "$scenes_src"/scene-*.png "$APP/Contents/Resources/scenes/"
# 词库：主库/繁体/笔画/五笔。`OpiEngine.resourceDictionaryPath` 优先用户目录，
# 否则读 `Resources/dicts/`。与 Android assets 同一份格式，不要再生成第二份。
mkdir -p "$APP/Contents/Resources/dicts"
for dict in luna trad stroke wubi86; do
  cp "android/app/src/main/assets/$dict.opid" "$APP/Contents/Resources/dicts/$dict.opid"
done

# 语音模型：full 复制到 Resources/asr（OpiVoiceModels 先读 Bundle）；normal 不复制。
if [ "$TIER" = full ]; then
  models_src="desktop/asr-bundle/asr"
  [ -d "$models_src" ] || {
    echo "!! full 档缺少 $models_src；先按 .github/workflows/apple.yml 下载模型" >&2
    exit 1
  }
  mkdir -p "$APP/Contents/Resources/asr"
  cp -R "$models_src"/. "$APP/Contents/Resources/asr/"
  for rel in sensevoice/model.int8.onnx sensevoice/tokens.txt \
    whisper-small.en/small.en-encoder.int8.onnx \
    whisper-small.en/small.en-decoder.int8.onnx \
    whisper-small.en/small.en-tokens.txt vad/silero_vad.onnx; do
    [ -f "$APP/Contents/Resources/asr/$rel" ] || {
      echo "!! full 档模型缺 $rel" >&2; exit 1; }
  done
  # Rust 加载器无条件读 manifest.json；桌面 bundle 没有，按实际文件生成。
  python3 - "$APP/Contents/Resources/asr" <<'PY'
import hashlib, json, os, sys
root = sys.argv[1]
roles = {
    "sensevoice/model.int8.onnx": "model",
    "sensevoice/tokens.txt": "tokens",
    "whisper-small.en/small.en-encoder.int8.onnx": "encoder",
    "whisper-small.en/small.en-decoder.int8.onnx": "decoder",
    "whisper-small.en/small.en-tokens.txt": "tokens",
}
for d in ("sensevoice", "whisper-small.en"):
    files = []
    for rel, role in roles.items():
        if not rel.startswith(d + "/"):
            continue
        path = os.path.join(root, rel)
        digest = hashlib.sha256()
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                digest.update(chunk)
        files.append({"path": os.path.basename(rel), "size": os.path.getsize(path),
                      "sha256": digest.hexdigest(), "role": role})
    with open(os.path.join(root, d, "manifest.json"), "w", encoding="utf-8") as out:
        json.dump({"version": 1, "files": files}, out, separators=(",", ":"))
PY
else
  [ ! -e "$APP/Contents/Resources/asr" ] || {
    echo "!! normal 档不应包含 Resources/asr" >&2; exit 1; }
fi

# ---- 4) 签名（ad-hoc）----
# 顺序：先签里面那份 dylib，再签外层 bundle。反过来的话外层签名封的是**旧**哈希，
# `--verify --deep` 立刻红（这条顺序不是讲究，是必须）。
for lib in "$APP/Contents/Frameworks"/*.dylib; do
  codesign --force --sign - "$lib"
done
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

# ---- 5) 取证：产物真的「通用 + 链上了 Rust + 能加载」 ----
bin="$APP/Contents/MacOS/$EXE"
info="$(lipo -info "$bin")"
for arch in arm64 x86_64; do
  grep -q "$arch" <<<"$info" || { echo "!! app 里没有 $arch 切片：$info" >&2; exit 1; }
done
# 「otool 里记了依赖」与「运行时找得到」是两件事：依赖名要靠 rpath 解析。
otool -L "$bin" | grep -q 'libopi_ffi.dylib' || {
  echo "!! app 没链上 libopi_ffi.dylib —— otool -L 全文：" >&2; otool -L "$bin" >&2; exit 1; }
# 记的必须是 @rpath/（能靠 bundle 内的 rpath 解析），**不能**是构建机上的绝对路径
# —— 绝对路径在本机全绿、在别人机器上直接崩（见上面 install_name_tool 那段）。
otool -L "$bin" | grep -q '^[[:space:]]*@rpath/libopi_ffi\.dylib' || {
  echo "!! app 对 libopi_ffi 的依赖不是 @rpath 形式（换台机器就加载不了）：" >&2
  otool -L "$bin" | grep 'libopi_ffi' >&2; exit 1; }
otool -L "$bin" | grep -q '^[[:space:]]*@rpath/libopi_asr\.dylib' || {
  echo "!! app 对 libopi_asr 的依赖不是 @rpath 形式：" >&2
  otool -L "$bin" | grep 'libopi_asr' >&2; exit 1; }
# Frameworks 里每个 dylib：自身 install_name 必须是 @rpath，所有非系统依赖必须能
# 按 @rpath 在同目录找到（否则装到别的机器就是 dyld: Library not loaded）。
for lib in "$APP/Contents/Frameworks"/*.dylib; do
  otool -D "$lib" | grep -q "^@rpath/${lib##*/}$" || {
    echo "!! $(basename "$lib") 的 install_name 不是 @rpath/${lib##*/}：" >&2
    otool -D "$lib" >&2; exit 1; }
  while IFS= read -r dep; do
    case "$dep" in
      @rpath/*) [ -f "$APP/Contents/Frameworks/${dep#@rpath/}" ] || {
        echo "!! $(basename "$lib") 的依赖 $dep 不在 Frameworks 里" >&2; exit 1; } ;;
      /usr/lib/*|/System/Library/*|/Library/Frameworks/*) ;;
      *) echo "!! $(basename "$lib") 有非 @rpath 依赖：$dep" >&2; exit 1 ;;
    esac
  done < <(otool -L "$lib" | awk '/^[[:space:]]/ {print $1}')
done
otool -l "$bin" | grep -q '@executable_path/../Frameworks' || {
  echo "!! app 里没有 @executable_path/../Frameworks 这条 rpath —— 装到系统里会找不到 dylib" >&2
  exit 1; }
# 真跑一次：dlopen bundle 里那份 dylib 并调真出口（不是只看链接记录）。
python3 - "$APP/Contents/Frameworks/libopi_ffi.dylib" <<'PY'
import ctypes, sys
lib = ctypes.CDLL(sys.argv[1])
lib.opi_load.restype = ctypes.c_bool
lib.opi_mode.restype = ctypes.c_int32
lib.opi_buffer.restype = None  # 只为确认符号在；真正读它要走 OpiString 释放，这里不碰
ok = lib.opi_load(None, 0)     # NULL + 0 = 内置回退词库
mode = lib.opi_mode()
print("dlopen 后 opi_load(内置回退库) =", ok, "/ opi_mode() =", mode)
sys.exit(0 if ok and mode == 0 else 1)
PY

# ---- 6) .pkg（未签名）----
# 落点 /Library/Input Methods/：TIS 文档里输入法包的两个合法位置之一（另一个是用户级
# ~/Library/Input Methods/，见 macos/README.md）；pkg 装系统级，用户级安装需自己构建 .app。
stage="$out/pkgroot"
mkdir -p "$stage/Library/Input Methods"
cp -R "$APP" "$stage/Library/Input Methods/"
pkg="$out/opi-$version-macos-universal.pkg"
# 装前停旧进程（2026-10-17 用户报障「装完还是旧版本」的 macOS 版）：脚本随包编入，
# 装/升级时由 installer 以 root 跑 preinstall（见 macos/scripts/preinstall 头注）。
# 构建期给一条语法门禁 —— 脚本坏在 pkgbuild 里只会得到一条晦涩的警告。
sh -n macos/scripts/preinstall || { echo "!! macos/scripts/preinstall 语法错" >&2; exit 1; }
pkgbuild --root "$stage" --identifier "$BUNDLE_ID.pkg" --version "$version" \
  --scripts macos/scripts \
  --install-location / "$pkg"
rm -rf "$stage"
# pkg 里「有没有东西」要真读一遍：pkgbuild 对空 root **不报错**，会产出一个
# 0 字节 payload 的包 —— 那种包在用户侧表现为「装完什么都没有」，且没有任何报错。
pkgutil --payload-files "$pkg" > "$out/.payload.txt"
grep -q "OpiInputMethod.app/Contents/MacOS/$EXE" "$out/.payload.txt" || {
  echo "!! .pkg 的 payload 里没有可执行文件 —— 空包或不完整包：" >&2
  cat "$out/.payload.txt" >&2; exit 1; }
grep -q 'OpiInputMethod.app/Contents/Info.plist' "$out/.payload.txt" || {
  echo "!! .pkg 的 payload 里没有 Info.plist" >&2; cat "$out/.payload.txt" >&2; exit 1; }
grep -q 'OpiInputMethod.app/Contents/Frameworks/libopi_ffi.dylib' "$out/.payload.txt" || {
  echo "!! .pkg 的 payload 里没有 Frameworks/libopi_ffi.dylib —— 装完 dyld 找不到库" >&2
  cat "$out/.payload.txt" >&2; exit 1; }
grep -q 'OpiInputMethod.app/Contents/Frameworks/libopi_asr.dylib' "$out/.payload.txt" || {
  echo "!! .pkg 的 payload 里没有 Frameworks/libopi_asr.dylib —— 语音引擎装不上" >&2
  cat "$out/.payload.txt" >&2; exit 1; }
grep -Eq 'OpiInputMethod\.app/Contents/Frameworks/libsherpa-onnx[^/]*\.dylib' "$out/.payload.txt" || {
  echo "!! .pkg 里没有 sherpa-onnx 动态库" >&2; cat "$out/.payload.txt" >&2; exit 1; }
grep -Eq 'OpiInputMethod\.app/Contents/Frameworks/libonnxruntime[^/]*\.dylib' "$out/.payload.txt" || {
  echo "!! .pkg 里没有 onnxruntime 动态库" >&2; cat "$out/.payload.txt" >&2; exit 1; }
if [ "$TIER" = full ]; then
  for res in \
    "asr/sensevoice/model.int8.onnx" "asr/sensevoice/tokens.txt" \
    "asr/sensevoice/manifest.json" \
    "asr/whisper-small.en/small.en-encoder.int8.onnx" \
    "asr/whisper-small.en/small.en-decoder.int8.onnx" \
    "asr/whisper-small.en/small.en-tokens.txt" \
    "asr/whisper-small.en/manifest.json" "asr/vad/silero_vad.onnx"; do
    grep -q "OpiInputMethod.app/Contents/Resources/$res" "$out/.payload.txt" || {
      echo "!! full 档 .pkg 缺少 Resources/$res" >&2
      cat "$out/.payload.txt" >&2; exit 1; }
  done
elif grep -q 'OpiInputMethod.app/Contents/Resources/asr/' "$out/.payload.txt"; then
  echo "!! normal 档 .pkg 不应包含 Resources/asr/" >&2
  cat "$out/.payload.txt" >&2; exit 1
fi
# 资源漏拷 = 对应面板/方案静默变空（还照常能开）—— 所以钉最终 payload，
# 而不只是钉打包前的 $APP。贴纸按源目录逐个列，避免「只查 idle 一个」漏掉别的。
sticker_res=()
for f in shared/pet/stickers/*.png; do sticker_res+=("pet/stickers/${f##*/}"); done
for f in shared/pet/stickers-gif/*.gif; do sticker_res+=("pet/stickers-gif/${f##*/}"); done
for res in "symbol_categories.tsv" "opi.tiff" "opi-pet.icns" \
           "en.lproj/Localizable.strings" \
           "emoji.tsv" "kaomoji.tsv" "pet-map.tsv" \
           "scenes/scene-meadow.png" \
           "${sticker_res[@]}" \
           "dicts/luna.opid" "dicts/trad.opid" "dicts/stroke.opid" "dicts/wubi86.opid"; do
  grep -q "OpiInputMethod.app/Contents/Resources/$res" "$out/.payload.txt" || {
    echo "!! .pkg 的 payload 里没有 Resources/$res" >&2
    cat "$out/.payload.txt" >&2; exit 1; }
done
rm -f "$out/.payload.txt"

# ---- 7) 收尾：dist 里只留这一个 .pkg ----
# `.app` 是构建中间物（payload 已在 pkg 里）；不再产出 .zip。
# 用户 2026-10-21 裁决：macOS 端所有东西打进一个可安装包，不要额外文件。
rm -rf "$APP"
stray="$(find "$out" -mindepth 1 -maxdepth 1 ! -name '*.pkg' -print -quit)"
if [ -n "$stray" ]; then
  echo "!! dist 里有非 PKG 产物：$stray" >&2
  exit 1
fi

echo "---- macOS 产物（未签名/未公证，开发用；dist 只此一个）----"
echo "  $pkg"
echo "  内含 /Library/Input Methods/$EXE.app（Frameworks + Resources 全量）"
echo "  语音模型档位：${TIER}（$([ "$TIER" = full ] && echo 'Resources/asr 内置' || echo '按需下载')）"
echo "  版本：${version}（取自 $(basename "$root")/Cargo.toml）；Info.plist 记的是 $plist_ver"
