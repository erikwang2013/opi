#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 把 `ios/` 的 Swift + Rust 静态库组装成宿主 App + 键盘扩展，并打成 **unsigned IPA**。
# 最终产物**只有一个**文件：
#
#   dist/apple/ios/opi-<版本>-ios-unsigned.ipa
#       Payload/Opi.app，内含宿主 App、键盘扩展、luna/trad/stroke/wubi86 词库、
#       rare_index.tsv 与图标。**未签名**。
#
# framework/xcframework、扩展目录、宿主 .app、临时 Payload 目录都只在本脚本的
# 临时构建目录里存在，脚本退出时清掉 —— dist 里不放任何中间产物。
#
# 用法（仓库根目录）：
#   scripts/apple/package-ios.sh <版本号> [产物目录]
#   版本号用 scripts/apple/version.sh 取（CI 里就是这么调的）。
#   档位由环境变量 OPI_IOS_TIER 选择：normal（默认，模型按需下载）/ full（内置模型）。
#
# ===== ⚠️ 未签名 IPA = 需要重签才能安装 =====
#
# 键盘扩展**不能**独立安装（Apple 强制要求内嵌在宿主 App 的 `PlugIns/` 里），
# 所以本脚本用 `ios/HostApp/OpiApp.swift` 编出一个最小宿主 App 来承载它。
# 但仓库没有 Apple 签名证书，因此：
#   · 生成的 `.ipa` 是**未签名**的；iOS 不会直接安装，需用 Sideloadly / AltStore
#     等工具以用户自己的 Apple ID 重签后再装（免费账号 7 天，付费开发者账号 1 年）；
#   · 想产出可直接安装/上架的签名 IPA，需要 Apple Developer 证书 + provisioning
#     profile，并在脚本/CI 里加签名步骤 —— 当前没有，也不假装有；
#   · 运行时行为（键盘在系统里出现、按键进引擎、候选上屏）**仍未验证**。
#
# 本脚本能证明的是：Swift 过编译器、链接上 Rust 静态库、无未定义 opi_ 符号，
# 且宿主 App / appex / IPA 的目录结构与 plist 自洽。
#
# ===== 语音输入（2026-11-02）=====
#
#   · 宿主 App 链接 opi_asr + sherpa-onnx/onnxruntime 静态库；采集、识别、回填都在
#     宿主。键盘扩展仍只链 opi_ffi，不申请麦克风、不创建 ASR 引擎。
#   · normal 档 IPA 不含模型，App 首次使用时从 opi.erik.xyz 逐文件下载到
#     Application Support/opi/asr。full 档把 `desktop/asr-bundle/asr` 复制进宿主
#     bundle 的 `asr/`，并补写两个 manifest.json（桌面 bundle 只有模型文件）。
#   · App Group（group.xyz.erik.opi）是签名构建的可选项：本脚本**不加** entitlements
#     （unsigned IPA 带了也没用）。没有 App Group 时语音结果经 UIPasteboard
#     （前缀 opi-voice:）回填，键盘需「允许完全访问」才能读取。
#
# ===== 前置：Rust 侧 iOS 静态库（本脚本**不**编 Rust，只读它的产物）=====
#
#   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
#   cargo build --release -p opi_ffi --target aarch64-apple-ios
#   cargo build --release -p opi_ffi --target aarch64-apple-ios-sim
#   cargo build --release -p opi-asr --target aarch64-apple-ios
#
# opi_asr 带 sherpa-rs 的 iOS 预编译静态库。本脚本按
# `target/aarch64-apple-ios/release/build/sherpa-rs-sys-*/output` 里的
# `cargo:rustc-link-search=` 找 libsherpa-onnx.a / libonnxruntime.a；也认 sherpa-rs
# 缓存（macOS: ~/Library/Caches/sherpa-rs/aarch64-apple-ios）与 SHERPA_LIB_PATH。
#
# 模拟器只做 arm64 切片：Intel Mac 上的模拟器要 x86_64-apple-ios —— 本轮不做，
# 要加就是在下面 `for spec` 里多一行，没有别的机关（如实记：现在缺这一片）。
set -euo pipefail

version="${1:?用法: scripts/apple/package-ios.sh <版本号> [产物目录]}"
out="${2:-dist/apple/ios}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

# ---- 本端常量 ----
FMWK="OPIKeyboard"                    # framework 名（也是里面的二进制名）
MODULE="OPIKeyboard"                  # Swift 模块名
# 部署目标：仓库**没有**声明 iOS 最低版本。15.0 覆盖键盘扩展用到的全部 API
# （`UIPress.key` 只要 13.4，取 15.0 留余量）。本脚本的选择，不是仓库约定。
IOS_MIN="15.0"
# 模型档位：normal=按需下载；full=内置模型。CI 的 tier 输入经环境变量传进来。
TIER="${OPI_IOS_TIER:-normal}"
case "$TIER" in
  normal|full) ;;
  *) echo "!! OPI_IOS_TIER 只能是 normal 或 full（现在是 '$TIER'）" >&2; exit 1 ;;
esac
# 宿主 App 与扩展的 bundle id。扩展 id **必须**以宿主 id 为前缀（Apple 强制），
# 所以这里由 HOST_BUNDLE_ID 派生，别分别手写两个容易漂的常量。
HOST_APP="Opi"
HOST_BUNDLE_ID="xyz.erik.opi"
KB_BUNDLE_ID="$HOST_BUNDLE_ID.keyboard"
# 设置/关于页英文表（由 shared/i18n/zh-en.tsv 生成）：缺了英文界面会整体回落中文。
[ -f ios/en.lproj/Localizable.strings ] || {
  echo "!! 缺少 ios/en.lproj/Localizable.strings（见 shared/i18n/zh-en.tsv）" >&2; exit 1; }

rm -rf "$out"

# 中间产物全部落在临时目录；EXIT 时清掉，dist 最终只由末尾的 mv 放进来一个 IPA。
build="$(mktemp -d "${TMPDIR:-/tmp}/opi-ios-build.XXXXXX")"
trap 'rm -rf "$build"' EXIT

# ---- 1) typecheck（两个平台各一遍）+ 真编成 framework ----
#
# 先 typecheck：它不产文件、错在哪一行直说，比链接期好读。设备与模拟器的 SDK 不同，
# 所以两遍都跑（可用性标注的差异就在这些地方，只有对应 SDK 才报）。
# spec = 平台:rust 三元组:sdk 名 —— Rust 静态库按**三元组**取，不给「暂存目录」另立一套命名。
for spec in "ios-arm64:aarch64-apple-ios:iphoneos" "ios-sim:aarch64-apple-ios-sim:iphonesimulator"; do
  plat="${spec%%:*}"; rest="${spec#*:}"; rustt="${rest%%:*}"; sdkname="${rest#*:}"
  sdk="$(xcrun --sdk "$sdkname" --show-sdk-path)"
  case "$plat" in
    ios-arm64) target="arm64-apple-ios$IOS_MIN" ;;
    *)         target="arm64-apple-ios$IOS_MIN-simulator" ;;
  esac
  rustlib="target/$rustt/release"
  [ -f "$rustlib/libopi_ffi.a" ] || {
    echo "!! 缺少 $rustlib/libopi_ffi.a —— 先 cargo build --release -p opi_ffi --target $rustt" >&2
    exit 1
  }
  # ⚠️ 链接**必须按显式路径喂 .a，不许走 `-L … -lopi_ffi`**：crate-type 里有
  # `cdylib`，所以同一目录里既有 `libopi_ffi.dylib` 又有 `libopi_ffi.a`，而 ld64 的
  # `-l` 搜索**优先挑 .dylib** —— 链接会「成功」，产出一个**动态依赖** libopi_ffi 的
  # framework：符号全部变成未定义导入，`nm -gU`（不列未定义）里一个 opi_ 符号都看不到。
  # 首轮实测正是这个形态（run 37933881578：链接过、但「没有定义 _opi_load」）。
  # iOS 上带第三方 dylib 的扩展本就不能装，所以 .a 是唯一正确的输入。
  echo "== $rustt 的产物（注意同目录还有 .dylib，我们**不用**它）："
  ls -la "$rustlib" | grep -i 'libopi' || true
  fw="$build/$plat/$FMWK.framework"
  mkdir -p "$fw"
  staticlib="$PWD/$rustlib/libopi_ffi.a"
  common=(-import-objc-header ios/OpiFFI.h -module-name "$MODULE" -sdk "$sdk" -target "$target")
  swiftc "${common[@]}" -typecheck ios/*.swift shared/apple/OpiClipRunner.swift
  # ⚠️ 2026-10-16 加的 `-force_load`：拉成员不再依赖归档索引按符号查找，把静态库整棵链进来。
  # 背景：iOS job 在 c639167 / 075a382 两连红，**签名逐字相同**（framework 里 `_opi_load`
  # 没有定义、链接本身不报错），而同一镜像跑旧提交（ae9c49a）是绿的（树相关、两次复现）。
  # 本地逐字节解析过归档：`__.SYMDEF` 在、索引里 `_opi_load` 在、定义成员在 —— 怀疑点落在
  # 「索引/查找→拉成员」这条路上（也可能是对象层，见下面这行注释的判据）。
  # 判据：若加 force_load 后**转绿** ⇒ 问题在索引/查找路径；若**同样红** ⇒ 在对象/ld64 层，
  # 把这条留不留另行裁决（它本身只增不减正确链接的成员）。
  swiftc "${common[@]}" -O -emit-library ios/*.swift shared/apple/OpiClipRunner.swift -o "$fw/$FMWK" \
    -Xlinker -force_load -Xlinker "$staticlib" \
    -Xlinker -install_name -Xlinker "@rpath/$FMWK.framework/$FMWK"
  # framework 的 Info.plist：`xcodebuild -create-xcframework` 只收**像 framework** 的目录，
  # 缺 plist / PackageType 不是 FMWK 会被拒。键值照 Xcode 的模板写。
  cat > "$fw/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
	<key>CFBundleExecutable</key><string>$FMWK</string>
	<key>CFBundleIdentifier</key><string>$KB_BUNDLE_ID.framework</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleName</key><string>$FMWK</string>
	<key>CFBundlePackageType</key><string>FMWK</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$version</string>
	<key>CFBundleSupportedPlatforms</key><array><string>$([ "$plat" = ios-arm64 ] && echo iPhoneOS || echo iPhoneSimulator)</string></array>
	<key>MinimumOSVersion</key><string>$IOS_MIN</string>
</dict>
</plist>
PLIST
  plutil -lint "$fw/Info.plist" >/dev/null
done

# ---- 2) 取证：真链上了、且**没有未定义的 opi_ 符号** ----
# 「链接过了」不等于「符号都齐了」：本仓库的教训是只有 `ldd -r` 才看得见未定义符号
# （`nm -u` 是同一件事在 Mach-O 上的做法）。iOS 上未定义符号默认是硬错，这条是补刀。
fwbin="$build/ios-arm64/$FMWK.framework/$FMWK"
# 三份输出**先落到变量再 grep**：`nm | grep -q` / `nm | head` 在 `pipefail` 下会因
# grep/head 提前退出、nm 收到 SIGPIPE 而把「符号明明在」判成失败（2026-10-18 实测：
# 符号数增长后 iOS job 连续两次红在 `LLVM ERROR: Broken pipe`，报错却指向未定义符号）。
nm_g=$(nm -g "$fwbin" 2>&1 || true)
nm_g_defined=$(nm -gU "$fwbin" 2>&1 || true)
nm_u=$(nm -u "$fwbin" 2>&1 || true)
otool_l=$(otool -L "$fwbin" 2>&1 || true)
# 失败时要能一眼看出是「没链进去」还是「链的是 dylib」，所以三条证据一起打。
# （首轮那条「没有定义 _opi_load」就是靠后面的 otool 才定性成动态链接的 —— 这次直接给。）
dump_evidence() {
  echo "-- file：" >&2; file "$fwbin" >&2
  echo "-- nm -g（已定义的外部符号，前 40 行）：" >&2; printf '%s\n' "$nm_g" | head -40 >&2
  echo "-- nm -u（未定义符号）：" >&2; printf '%s\n' "$nm_u" | head -40 >&2
  echo "-- otool -L（动态依赖）：" >&2; printf '%s\n' "$otool_l" >&2
}
for sym in _opi_load _opi_key_event _opi_ffi_free_string _opi_candidates_page; do
  printf '%s\n' "$nm_g_defined" | grep -q "$sym\$" || {
    echo "!! $FMWK 里没有定义 $sym —— Rust 静态库没链进去（链接线/crate-type 变了？）" >&2
    dump_evidence; exit 1
  }
done
if printf '%s\n' "$nm_u" | grep -q '_opi_'; then
  echo "!! $FMWK 里有未定义的 opi_ 符号 —— 链接不完整：" >&2
  printf '%s\n' "$nm_u" | grep '_opi_' >&2
  dump_evidence; exit 1
fi
# 静态库应当是**静态**链进去的：出现动态依赖就说明链接线变成了 `-l`（ld64 会优先挑
# 同目录的 .dylib）（iOS 上带第三方 dylib 的扩展装不上，而且那种链接会静默「成功」）。
if printf '%s\n' "$otool_l" | grep -q 'libopi_ffi'; then
  echo "!! $FMWK 动态依赖了 libopi_ffi —— 应为按路径直链 .a：" >&2
  dump_evidence; exit 1
fi

# ---- 3) xcframework ----
xcodebuild -create-xcframework \
  -framework "$build/ios-arm64/$FMWK.framework" \
  -framework "$build/ios-sim/$FMWK.framework" \
  -output "$build/$FMWK.xcframework"

# ---- 4) 扩展的 Info.plist（展开占位符，随后装进 appex）----
# 展开规则与 macOS 侧同一条：`$(PRODUCT_MODULE_NAME).KeyboardViewController` 写成**裸名**，
# 因为 `KeyboardViewController.swift` 上有 `@objc(KeyboardViewController)` ——
# 运行时名就是裸名，`NSClassFromString` 查得到的只有它。⚠️ 运行期未验证（要真装进系统）。
cp ios/Info.plist "$build/KeyboardExtension-Info.plist"
kb="$build/KeyboardExtension-Info.plist"
plutil -replace CFBundleExecutable -string "$FMWK" "$kb"
plutil -replace CFBundleIdentifier -string "$KB_BUNDLE_ID" "$kb"
plutil -replace CFBundleName -string "$FMWK" "$kb"
# 宿主与扩展的版本号必须一致；CFBundleVersion 允许点分整数串。
plutil -replace CFBundleShortVersionString -string "$version" "$kb"
plutil -replace CFBundleVersion -string "$version" "$kb"
plutil -replace MinimumOSVersion -string "$IOS_MIN" "$kb"
plutil -replace NSExtension.NSExtensionPrincipalClass -string "KeyboardViewController" "$kb"
plutil -lint "$kb" >/dev/null
# 判据落在**值**上（与 macOS 侧同一条理由）：`plutil -p` 只吐键值，注释里的 `$(` 例子不算。
kb_dump="$(plutil -p "$kb")"
[ -n "$kb_dump" ] || { echo "!! plutil -p 没输出，占位符门禁失效" >&2; exit 1; }
if grep -q '\$(' <<<"$kb_dump"; then
  echo "!! 扩展 plist 的**值**里还有没展开的 Xcode 占位符：" >&2
  grep '\$(' <<<"$kb_dump" >&2; exit 1
fi

# ---- 5) 扩展可执行（入口 _NSExtensionMain）----
#
# IPA 必须有扩展可执行，所以这一步**不再是可选项**：链接失败就退出。扩展二进制必须由
# 链接器入口 `_NSExtensionMain` 起头（Xcode 生成的链接行就是这么写的）；这条手写链接行
# 已在 CI 上实测通过。
if swiftc -parse-as-library -emit-executable ios/*.swift shared/apple/OpiClipRunner.swift -o "$build/.appex-bin" \
     -import-objc-header ios/OpiFFI.h -module-name "$MODULE" \
     -sdk "$(xcrun --sdk iphoneos --show-sdk-path)" -target "arm64-apple-ios$IOS_MIN" \
     -Xlinker "$PWD/target/aarch64-apple-ios/release/libopi_ffi.a" \
     -Xlinker -e -Xlinker _NSExtensionMain 2>&1 | tee "$build/appex-link.log"; then
  appex="$build/OpiKeyboard.appex"
  mkdir -p "$appex/en.lproj"
  cp ios/en.lproj/Localizable.strings "$appex/en.lproj/Localizable.strings"
  mv "$build/.appex-bin" "$appex/$FMWK"
  cp "$kb" "$appex/Info.plist"
  rm -f "$build/appex-link.log"
  # ⚠️ `${appex}` 的花括号**不能省**：`$appex（` 这种「变量后紧跟多字节字符」的写法
  # 在 macOS 的 /bin/bash（3.2）下会把 `（` 吞进变量名 → `appex…: unbound variable`
  # （首轮实测就是这么红的：run 37934619550，链接都过了、死在最后一行 echo 上）。
  # 本地 bash 5 复现不出来 —— 别用「本机跑过」当这条的判据。
  echo "appex 可执行链接成功：${appex}（**未签名**）"
else
  echo "!! .appex 可执行**未产出**（链接器原文见上），无法组装 IPA。" >&2
  echo "   中间产物只在临时构建目录，脚本退出后清理。" >&2
  exit 1
fi

# ---- 6) 宿主 App + unsigned IPA ----
# 键盘扩展不能独立安装：Apple 要求内嵌在宿主 App 的 PlugIns/ 里。
# 宿主源码是 ios/HostApp/OpiApp.swift，用 swiftc 直接编，不依赖 .xcodeproj。
app="$build/$HOST_APP.app"
hostbin="$app/$HOST_APP"
mkdir -p "$app/PlugIns" "$app/en.lproj"
cp ios/en.lproj/Localizable.strings "$app/en.lproj/Localizable.strings"
cp -R "$appex" "$app/PlugIns/OpiKeyboard.appex"
# 词库放进**扩展**的 bundle（扩展里 Bundle.main 指向 appex，不是宿主 App）。
# 取已入库的 Android 部署副本：CI checkout 里一定有；data/generated/luna.opid 被 gitignore。
for dict in luna trad stroke wubi86; do
  src="android/app/src/main/assets/$dict.opid"
  [ -f "$src" ] || { echo "!! 缺少词库 $src；不能产出可用 IPA" >&2; exit 1; }
  cp "$src" "$app/PlugIns/OpiKeyboard.appex/$dict.opid"
done
# 生僻字索引（`RarePanel` 从扩展 bundle 读，文件名必须是 `rare_index.tsv`）。
# 缺了面板只会显示「索引未打包」—— 那种 IPA 看着完整、功能残缺，所以这里硬失败。
rare="android/app/src/main/assets/rare_index.tsv"
[ -f "$rare" ] || { echo "!! 缺少生僻字索引 $rare；不能产出可用 IPA" >&2; exit 1; }
cp "$rare" "$app/PlugIns/OpiKeyboard.appex/rare_index.tsv"
# 动态皮肤插画（2026-10-31）：与 Android/桌面/鸿蒙同一份生成产物，iOS 只显示图片。
scenes_src="shared/scene/art"
[ -d "$scenes_src" ] || { echo "!! 缺少插画资产 $scenes_src" >&2; exit 1; }
mkdir -p "$app/PlugIns/OpiKeyboard.appex/scenes"
cp "$scenes_src"/scene-*.png "$app/PlugIns/OpiKeyboard.appex/scenes/"
# 图标用项目宠物图标；sips 生成 iOS 需要的尺寸（AppIcon60x60@2x/@3x、AppIcon76x76@2x）。
icon="desktop/icons/opi-pet.png"
[ -f "$icon" ] || { echo "!! 缺少图标 $icon" >&2; exit 1; }
sips -z 120 120 "$icon" --out "$app/AppIcon60x60@2x.png" >/dev/null
sips -z 180 180 "$icon" --out "$app/AppIcon60x60@3x.png" >/dev/null
sips -z 152 152 "$icon" --out "$app/AppIcon76x76@2x.png" >/dev/null

# 语音模型：full 复制内置，normal 不复制（IPA 里不能出现 asr/）。
if [ "$TIER" = full ]; then
  models_src="desktop/asr-bundle/asr"
  [ -d "$models_src" ] || {
    echo "!! full 档缺少 $models_src；先按 .github/workflows/apple.yml 下载模型" >&2
    exit 1
  }
  mkdir -p "$app/asr"
  cp -R "$models_src"/. "$app/asr/"
  for rel in sensevoice/model.int8.onnx sensevoice/tokens.txt \
    whisper-small.en/small.en-encoder.int8.onnx \
    whisper-small.en/small.en-decoder.int8.onnx \
    whisper-small.en/small.en-tokens.txt vad/silero_vad.onnx; do
    [ -f "$app/asr/$rel" ] || { echo "!! full 档模型缺 $rel" >&2; exit 1; }
  done
  # Rust 加载器无条件读 manifest.json；桌面 bundle 没有，按实际文件生成。
  python3 - "$app/asr" <<'PY'
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
fi

cat > "$app/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
	<key>CFBundleDisplayName</key><string>小欧输入法</string>
	<key>CFBundleExecutable</key><string>$HOST_APP</string>
	<key>CFBundleIdentifier</key><string>$HOST_BUNDLE_ID</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleName</key><string>$HOST_APP</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$version</string>
	<key>CFBundleSupportedPlatforms</key><array><string>iPhoneOS</string></array>
	<key>MinimumOSVersion</key><string>$IOS_MIN</string>
	<key>DTPlatformName</key><string>iphoneos</string>
	<key>UILaunchScreen</key><dict/>
	<key>UIRequiredDeviceCapabilities</key><array><string>arm64</string></array>
	<key>UISupportedInterfaceOrientations</key>
	<array><string>UIInterfaceOrientationPortrait</string><string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string></array>
	<key>UISupportedInterfaceOrientations~ipad</key>
	<array><string>UIInterfaceOrientationPortrait</string><string>UIInterfaceOrientationPortraitUpsideDown</string><string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string></array>
	<key>CFBundleIcons</key>
	<dict><key>CFBundlePrimaryIcon</key><dict><key>CFBundleIconFiles</key><array><string>AppIcon60x60</string></array></dict></dict>
	<key>CFBundleIcons~ipad</key>
	<dict><key>CFBundlePrimaryIcon</key><dict><key>CFBundleIconFiles</key><array><string>AppIcon76x76</string></array></dict></dict>
	<key>NSLocalNetworkUsageDescription</key><string>在同一局域网内发现并同步已配对设备之间的纯文本剪贴板内容。</string>
	<key>NSMicrophoneUsageDescription</key><string>用于录制语音并在本机识别，识别结果回填到键盘；录音不会上传或保存。</string>
	<key>CFBundleURLTypes</key>
	<array><dict>
		<key>CFBundleURLName</key><string>$HOST_BUNDLE_ID.voice</string>
		<key>CFBundleURLSchemes</key><array><string>opi-voice</string></array>
	</dict></array>
</dict>
</plist>
PLIST
plutil -lint "$app/Info.plist" >/dev/null
host_plist="$(plutil -p "$app/Info.plist")"
printf '%s\n' "$host_plist" | grep -q 'NSMicrophoneUsageDescription' || {
  echo "!! 宿主 Info.plist 缺 NSMicrophoneUsageDescription" >&2; exit 1; }
printf '%s\n' "$host_plist" | grep -q 'opi-voice' || {
  echo "!! 宿主 Info.plist 缺 opi-voice URL scheme" >&2; exit 1; }
# 宿主源码不止 OpiApp.swift：语音页/引擎/下载器都是独立文件。
host_sources=(ios/HostApp/*.swift ios/VoiceResultChannel.swift)
host_common=(-parse-as-library -import-objc-header ios/HostApp/OpiAsr.h \
  -sdk "$(xcrun --sdk iphoneos --show-sdk-path)" -target "arm64-apple-ios$IOS_MIN")
swiftc "${host_common[@]}" -typecheck "${host_sources[@]}"
asrlib="$PWD/target/aarch64-apple-ios/release/libopi_asr.a"
[ -f "$asrlib" ] || {
  echo "!! 缺少 $asrlib —— 先 cargo build --release -p opi-asr --target aarch64-apple-ios" >&2
  exit 1
}
# sherpa-rs 的 iOS 包是两个静态库；构建脚本把它们的目录写进 build output。
sherpa_dirs=("")
[ -n "${SHERPA_LIB_PATH:-}" ] && sherpa_dirs+=("$SHERPA_LIB_PATH" "$SHERPA_LIB_PATH/lib")
while IFS= read -r dir; do
  sherpa_dirs+=("$dir")
done < <(
  for sherpa_out in target/aarch64-apple-ios/release/build/sherpa-rs-sys-*/output; do
    [ -f "$sherpa_out" ] && sed -n \
      -e 's/^cargo:rustc-link-search=native=//p' \
      -e 's/^cargo:rustc-link-search=//p' "$sherpa_out"
  done
)
for sherpa_cache in "$HOME/Library/Caches/sherpa-rs/aarch64-apple-ios" \
  "$HOME/.cache/sherpa-rs/aarch64-apple-ios"; do
  [ -d "$sherpa_cache" ] || continue
  while IFS= read -r lib; do
    sherpa_dirs+=("$(dirname "$lib")")
  done < <(find "$sherpa_cache" \( -name libsherpa-onnx.a -o -name libonnxruntime.a \) 2>/dev/null)
done
find_archive() {
  name="$1"; shift
  for dir in "$@"; do
    [ -n "$dir" ] && [ -f "$dir/$name" ] && { printf '%s\n' "$dir/$name"; return 0; }
  done
  return 1
}
sherpa_lib="$(find_archive libsherpa-onnx.a "${sherpa_dirs[@]}")" || {
  echo "!! 找不到 libsherpa-onnx.a（先在本机编 opi_asr，或设 SHERPA_LIB_PATH）" >&2; exit 1; }
onnx_lib="$(find_archive libonnxruntime.a "${sherpa_dirs[@]}")" || {
  echo "!! 找不到 libonnxruntime.a（随 sherpa-rs 下载缓存）" >&2; exit 1; }
echo "宿主 ASR 静态库：$asrlib"
echo "  sherpa：$sherpa_lib"
echo "  onnxruntime：$onnx_lib"
# iOS 的 onnxruntime 静态 xcframework 可能把 absl 拆成独立归档；收集同目录的
# libabsl*.a 一并链接（找不到就交给链接器报真实缺符号）。
extra_archives=()
while IFS= read -r absl; do extra_archives+=("$absl"); done < <(
  find "$(dirname "$sherpa_lib")" "$(dirname "$onnx_lib")"     -name 'libabsl*.a' -o -name 'libabsl*.a' 2>/dev/null
)
if [ "${#extra_archives[@]}" -gt 0 ]; then
  echo "  absl：${extra_archives[*]}"
fi
for lib in "$sherpa_lib" "$onnx_lib"; do
  printf 'absl defs in %s: ' "$(basename "$lib")"
  xcrun nm -g "$lib" 2>/dev/null | grep -c 'AbslInternalSpinLock' || true
done
# 链接顺序：opi_asr → sherpa → onnxruntime。force_load 让归档成员稳定进宿主。
swiftc "${host_common[@]}" -emit-executable "${host_sources[@]}" -O -o "$hostbin" \
  -Xlinker -all_load \
  -Xlinker -force_load -Xlinker "$asrlib" \
  -Xlinker -force_load -Xlinker "$sherpa_lib" \
  -Xlinker -force_load -Xlinker "$onnx_lib" \
  ${extra_archives[@]+"${extra_archives[@]}"} \
  -Xlinker -lc++ -Xlinker -lz \
  -Xlinker -framework -Xlinker CoreML \
  -Xlinker -framework -Xlinker Foundation \
  -Xlinker -framework -Xlinker Accelerate \
  -Xlinker -framework -Xlinker CoreGraphics \
  -Xlinker -framework -Xlinker Security
chmod +x "$hostbin"
# 取证：宿主定义 opi_asr、无未定义 opi_asr、无动态 ASR 依赖。
host_nm="$(nm -gU "$hostbin" 2>&1 || true)"
host_nm_u="$(nm -u "$hostbin" 2>&1 || true)"
host_link="$(otool -L "$hostbin" 2>&1 || true)"
for sym in _opi_asr_create _opi_asr_load_model _opi_asr_poll_event _opi_asr_destroy; do
  printf '%s\n' "$host_nm" | grep -q "$sym\$" || {
    echo "!! 宿主没有定义 $sym —— opi_asr 静态库没链进去" >&2
    printf '%s\n' "$host_nm_u" >&2; exit 1; }
done
if printf '%s\n' "$host_nm_u" | grep -q '_opi_asr_'; then
  echo "!! 宿主有未定义 opi_asr 符号：" >&2
  printf '%s\n' "$host_nm_u" | grep '_opi_asr_' >&2; exit 1
fi
if printf '%s\n' "$host_link" | grep -qE 'libsherpa-onnx|libonnxruntime|libopi_asr'; then
  echo "!! 宿主动态依赖了 ASR 库（应为静态链接）：" >&2
  printf '%s\n' "$host_link" >&2; exit 1
fi
# IPA 就是 zip(Payload/Opi.app)。未签名，需重签后安装。
ipa="$build/opi-$version-ios-unsigned.ipa"
mkdir -p "$build/Payload"
rm -rf "$build/Payload/$HOST_APP.app"
cp -R "$app" "$build/Payload/$HOST_APP.app"
( cd "$build" && zip -q -r -y "$(basename "$ipa")" Payload )
rm -rf "$build/Payload"
[ -f "$ipa" ] || { echo "!! IPA 未产出" >&2; exit 1; }
# ---- 7) 最终产物自检：IPA 内容齐全，dist 里只有它 ----
ipa_list="$(unzip -l "$ipa")"
for member in \
  "Payload/Opi.app/Opi" \
  "Payload/Opi.app/en.lproj/Localizable.strings" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/en.lproj/Localizable.strings" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/OPIKeyboard" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/luna.opid" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/trad.opid" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/stroke.opid" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/wubi86.opid" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/rare_index.tsv" \
  "Payload/Opi.app/PlugIns/OpiKeyboard.appex/scenes/scene-meadow.png"; do
  if ! grep -qF "$member" <<<"$ipa_list"; then
    echo "!! IPA 缺少 $member" >&2
    exit 1
  fi
done

if [ "$TIER" = full ]; then
  for member in \
    "Payload/Opi.app/asr/sensevoice/model.int8.onnx" \
    "Payload/Opi.app/asr/sensevoice/tokens.txt" \
    "Payload/Opi.app/asr/sensevoice/manifest.json" \
    "Payload/Opi.app/asr/whisper-small.en/small.en-encoder.int8.onnx" \
    "Payload/Opi.app/asr/whisper-small.en/small.en-decoder.int8.onnx" \
    "Payload/Opi.app/asr/whisper-small.en/small.en-tokens.txt" \
    "Payload/Opi.app/asr/whisper-small.en/manifest.json" \
    "Payload/Opi.app/asr/vad/silero_vad.onnx"; do
    if ! grep -qF "$member" <<<"$ipa_list"; then
      echo "!! full 档 IPA 缺少 $member" >&2
      exit 1
    fi
  done
elif grep -qF "Payload/Opi.app/asr/" <<<"$ipa_list"; then
  echo "!! normal 档 IPA 不应包含 asr/ 模型" >&2
  exit 1
fi

mkdir -p "$out"
final_ipa="$out/$(basename "$ipa")"
mv "$ipa" "$final_ipa"
echo "IPA（未签名，需重签后安装）：$final_ipa"
stray="$(find "$out" -mindepth 1 -maxdepth 1 ! -name '*.ipa' -print -quit)"
if [ -n "$stray" ]; then
  echo "!! dist 里有非 IPA 产物：$stray" >&2
  exit 1
fi

echo "---- iOS 产物（unsigned IPA；未上机验证）----"
echo "  $final_ipa（dist 仅此一个文件）"
echo "  版本：${version}（取自 Cargo.toml）；语音模型档位：${TIER}"
echo "  语音模型：$([ "$TIER" = full ] && echo '已内置（无需下载）' || echo '按需下载')"
