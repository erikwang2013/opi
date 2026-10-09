#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 把 `ios/` 的 Swift + Rust 静态库组装成键盘扩展产物（**未签名**）：
#
#   dist/apple/ios/OPIKeyboard.xcframework                 （device arm64 + simulator arm64，**保证有**）
#   dist/apple/ios/KeyboardExtension-Info.plist            （占位符已展开的扩展 plist，集成起点）
#   dist/apple/ios/OpiKeyboard.appex/                      （尽力而为：真扩展可执行，见第 5 节）
#   dist/apple/ios/opi-ios-keyboard-<版本>-unsigned.zip    （上面全部）
#
# 用法（仓库根目录）：
#   scripts/apple/package-ios.sh <版本号> [产物目录]
#   版本号用 scripts/apple/version.sh 取（CI 里就是这么调的）。
#
# ===== ⚠️ 未签名 = **不能装真机** =====
#
# 本产物的定位是「**可编译 + 可下载**」的证据与集成起点，**不是**可用安装包：
#   · 装真机要 Apple 开发者账号签名（仓库没有证书，也不连签名服务）；
#   · 键盘扩展还**必须**内嵌在一个宿主 App 里（Apple 规定扩展不能独立安装）——
#     仓库里没有宿主 App，故产物里没有.
#   · 因此这里能给出的最强结论是：`ios/` 那批 Swift 能过编译器、能链接上 Rust 静态库，
#     且链接后**没有未定义的 opi_ 符号**。运行时行为（键盘起来之后的事）**未验证**。
#
# ===== 前置：Rust 侧 iOS 静态库（本脚本**不**编 Rust，只读它的产物）=====
#
#   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
#   cargo build --release -p opi_ffi --target aarch64-apple-ios
#   cargo build --release -p opi_ffi --target aarch64-apple-ios-sim
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
# 扩展的 bundle id：真实发布时它**必须**以宿主 App 的 id 为前缀（Apple 强制）。
# 仓库没有宿主 App，这里用 `xyz.erik.opi.keyboard` 占位；做宿主时按
# `<宿主 id>.keyboard` 改（改一处：这里）。
KB_BUNDLE_ID="xyz.erik.opi.keyboard"

rm -rf "$out"
mkdir -p "$out"

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
  fw="$out/$plat/$FMWK.framework"
  mkdir -p "$fw"
  staticlib="$PWD/$rustlib/libopi_ffi.a"
  common=(-import-objc-header ios/OpiFFI.h -module-name "$MODULE" -sdk "$sdk" -target "$target")
  swiftc "${common[@]}" -typecheck ios/*.swift
  swiftc "${common[@]}" -O -emit-library ios/*.swift -o "$fw/$FMWK" \
    -Xlinker "$staticlib" \
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
fwbin="$out/ios-arm64/$FMWK.framework/$FMWK"
# 失败时要能一眼看出是「没链进去」还是「链的是 dylib」，所以三条证据一起打。
# （首轮那条「没有定义 _opi_load」就是靠后面的 otool 才定性成动态链接的 —— 这次直接给。）
dump_evidence() {
  echo "-- file：" >&2; file "$fwbin" >&2
  echo "-- nm -g（已定义的外部符号，前 40 行）：" >&2; nm -g "$fwbin" | head -40 >&2
  echo "-- nm -u（未定义符号）：" >&2; nm -u "$fwbin" | head -40 >&2
  echo "-- otool -L（动态依赖）：" >&2; otool -L "$fwbin" >&2
}
for sym in _opi_load _opi_key_event _opi_ffi_free_string _opi_candidates_page; do
  nm -gU "$fwbin" | grep -q "$sym\$" || {
    echo "!! $FMWK 里没有定义 $sym —— Rust 静态库没链进去（链接线/crate-type 变了？）" >&2
    dump_evidence; exit 1
  }
done
if nm -u "$fwbin" | grep -q '_opi_'; then
  echo "!! $FMWK 里有未定义的 opi_ 符号 —— 链接不完整：" >&2
  nm -u "$fwbin" | grep '_opi_' >&2
  dump_evidence; exit 1
fi
# 静态库应当是**静态**链进去的：出现动态依赖就说明链接线变成了 `-l`（ld64 会优先挑
# 同目录的 .dylib）（iOS 上带第三方 dylib 的扩展装不上，而且那种链接会静默「成功」）。
if otool -L "$fwbin" | grep -q 'libopi_ffi'; then
  echo "!! $FMWK 动态依赖了 libopi_ffi —— 应为按路径直链 .a：" >&2
  dump_evidence; exit 1
fi

# ---- 3) xcframework ----
xcodebuild -create-xcframework \
  -framework "$out/ios-arm64/$FMWK.framework" \
  -framework "$out/ios-sim/$FMWK.framework" \
  -output "$out/$FMWK.xcframework"

# ---- 4) 扩展的 Info.plist（展开占位符；集成起点）----
# 展开规则与 macOS 侧同一条：`$(PRODUCT_MODULE_NAME).KeyboardViewController` 写成**裸名**，
# 因为 `KeyboardViewController.swift` 上有 `@objc(KeyboardViewController)` ——
# 运行时名就是裸名，`NSClassFromString` 查得到的只有它。⚠️ 运行期未验证（要真装进系统）。
cp ios/Info.plist "$out/KeyboardExtension-Info.plist"
kb="$out/KeyboardExtension-Info.plist"
plutil -replace CFBundleExecutable -string "$FMWK" "$kb"
plutil -replace CFBundleIdentifier -string "$KB_BUNDLE_ID" "$kb"
plutil -replace CFBundleName -string "$FMWK" "$kb"
plutil -replace NSExtension.NSExtensionPrincipalClass -string "KeyboardViewController" "$kb"
plutil -lint "$kb" >/dev/null
if grep -q '\$(' "$kb"; then
  echo "!! 扩展 plist 里还有没展开的 Xcode 占位符：" >&2; grep -n '\$(' "$kb" >&2; exit 1
fi

# ---- 5) 尽力而为：真 .appex 可执行（入口 _NSExtensionMain）----
#
# framework **保证有**；这一段是加分项。扩展二进制必须由链接器入口 `_NSExtensionMain`
# 起头（Xcode 生成的链接行就是这么写的），而手写这条链接行**从未被验证过**。
# 失败不影响上面的产物，但**必须响** —— 原样打印链接器的话，别读成「appex 也做出来了」。
if swiftc -parse-as-library -emit-executable ios/*.swift -o "$out/.appex-bin" \
     -import-objc-header ios/OpiFFI.h -module-name "$MODULE" \
     -sdk "$(xcrun --sdk iphoneos --show-sdk-path)" -target "arm64-apple-ios$IOS_MIN" \
     -Xlinker "$PWD/target/aarch64-apple-ios/release/libopi_ffi.a" \
     -Xlinker -e -Xlinker _NSExtensionMain 2>&1 | tee "$out/appex-link.log"; then
  appex="$out/OpiKeyboard.appex"
  mkdir -p "$appex"
  mv "$out/.appex-bin" "$appex/$FMWK"
  cp "$kb" "$appex/Info.plist"
  rm -f "$out/appex-link.log"
  echo "appex 可执行链接成功：$appex（**未签名**，装不上真机）"
else
  echo "!! .appex 可执行**未产出**（链接器原文见上）。本产物里只有 framework/xcframework。"
  echo "   这不是静默降级：上面那几行链接器输出就是原因，appex-link.log 也在产物里。"
fi

# ---- 6) zip ----
# 只列**明确成员**（不用 `zip -r .`）：那样会把中间产物一起卷进去，也会把 zip 自己套进来。
members=("$FMWK.xcframework" "KeyboardExtension-Info.plist")
if [ -d "$out/OpiKeyboard.appex" ]; then members+=("OpiKeyboard.appex"); fi
zip="$out/opi-ios-keyboard-$version-unsigned.zip"
( cd "$out" && zip -q -r "$(basename "$zip")" "${members[@]}" )

echo "---- iOS 产物（未签名，**不能装真机**；可编译 + 可下载的证据与集成起点）----"
echo "  $out/$FMWK.xcframework"
echo "  $kb"
if [ -d "$out/OpiKeyboard.appex" ]; then echo "  $out/OpiKeyboard.appex（未签名扩展目录）"; fi
echo "  $zip"
echo "  版本：$version（取自 Cargo.toml）"
