#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 把 `ios/` 的 Swift + Rust 静态库组装成宿主 App + 键盘扩展，并打成 **unsigned IPA**：
#
#   dist/apple/ios/OPIKeyboard.xcframework                 （device arm64 + simulator arm64，**保证有**）
#   dist/apple/ios/KeyboardExtension-Info.plist            （占位符已展开的扩展 plist，集成起点）
#   dist/apple/ios/OpiKeyboard.appex/                      （真扩展可执行，入口 _NSExtensionMain）
#   dist/apple/ios/Opi.app/                               （宿主 App，内嵌 PlugIns/OpiKeyboard.appex + 词库）
#   dist/apple/ios/opi-ios-keyboard-<版本>-unsigned.ipa    （Payload/Opi.app 的 IPA，**未签名**）
#   dist/apple/ios/opi-ios-keyboard-<版本>-unsigned.zip    （xcframework + 扩展 plist + IPA）
#
# 用法（仓库根目录）：
#   scripts/apple/package-ios.sh <版本号> [产物目录]
#   版本号用 scripts/apple/version.sh 取（CI 里就是这么调的）。
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
# 宿主 App 与扩展的 bundle id。扩展 id **必须**以宿主 id 为前缀（Apple 强制），
# 所以这里由 HOST_BUNDLE_ID 派生，别分别手写两个容易漂的常量。
HOST_APP="Opi"
HOST_BUNDLE_ID="xyz.erik.opi"
KB_BUNDLE_ID="$HOST_BUNDLE_ID.keyboard"

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
  # ⚠️ 2026-10-16 加的 `-force_load`：拉成员不再依赖归档索引按符号查找，把静态库整棵链进来。
  # 背景：iOS job 在 c639167 / 075a382 两连红，**签名逐字相同**（framework 里 `_opi_load`
  # 没有定义、链接本身不报错），而同一镜像跑旧提交（ae9c49a）是绿的（树相关、两次复现）。
  # 本地逐字节解析过归档：`__.SYMDEF` 在、索引里 `_opi_load` 在、定义成员在 —— 怀疑点落在
  # 「索引/查找→拉成员」这条路上（也可能是对象层，见下面这行注释的判据）。
  # 判据：若加 force_load 后**转绿** ⇒ 问题在索引/查找路径；若**同样红** ⇒ 在对象/ld64 层，
  # 把这条留不留另行裁决（它本身只增不减正确链接的成员）。
  swiftc "${common[@]}" -O -emit-library ios/*.swift -o "$fw/$FMWK" \
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
fwbin="$out/ios-arm64/$FMWK.framework/$FMWK"
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
# IPA 必须有扩展可执行，所以这一步**不再是可选项**：链接失败就退出（framework 与
# xcframework 已落盘，仍可供排查/集成）。扩展二进制必须由链接器入口 `_NSExtensionMain`
# 起头（Xcode 生成的链接行就是这么写的）；这条手写链接行已在 CI 上实测通过。
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
  # ⚠️ `${appex}` 的花括号**不能省**：`$appex（` 这种「变量后紧跟多字节字符」的写法
  # 在 macOS 的 /bin/bash（3.2）下会把 `（` 吞进变量名 → `appex…: unbound variable`
  # （首轮实测就是这么红的：run 37934619550，链接都过了、死在最后一行 echo 上）。
  # 本地 bash 5 复现不出来 —— 别用「本机跑过」当这条的判据。
  echo "appex 可执行链接成功：${appex}（**未签名**）"
else
  echo "!! .appex 可执行**未产出**（链接器原文见上），无法组装 IPA。" >&2
  echo "   framework/xcframework 仍在 $out，appex-link.log 也在，供排查。" >&2
  exit 1
fi

# ---- 6) 宿主 App + unsigned IPA ----
# 键盘扩展不能独立安装：Apple 要求内嵌在宿主 App 的 PlugIns/ 里。
# 宿主源码是 ios/HostApp/OpiApp.swift，用 swiftc 直接编，不依赖 .xcodeproj。
app="$out/$HOST_APP.app"
hostbin="$app/$HOST_APP"
mkdir -p "$app/PlugIns"
cp -R "$appex" "$app/PlugIns/OpiKeyboard.appex"
# 词库放进**扩展**的 bundle（扩展里 Bundle.main 指向 appex，不是宿主 App）。
# 取已入库的 Android 部署副本：CI checkout 里一定有；data/generated/luna.opid 被 gitignore。
for dict in luna trad stroke wubi86; do
  src="android/app/src/main/assets/$dict.opid"
  [ -f "$src" ] || { echo "!! 缺少词库 $src；不能产出可用 IPA" >&2; exit 1; }
  cp "$src" "$app/PlugIns/OpiKeyboard.appex/$dict.opid"
done
# 图标用项目宠物图标；sips 生成 iOS 需要的尺寸（AppIcon60x60@2x/@3x、AppIcon76x76@2x）。
icon="desktop/icons/opi-pet.png"
[ -f "$icon" ] || { echo "!! 缺少图标 $icon" >&2; exit 1; }
sips -z 120 120 "$icon" --out "$app/AppIcon60x60@2x.png" >/dev/null
sips -z 180 180 "$icon" --out "$app/AppIcon60x60@3x.png" >/dev/null
sips -z 152 152 "$icon" --out "$app/AppIcon76x76@2x.png" >/dev/null

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
</dict>
</plist>
PLIST
plutil -lint "$app/Info.plist" >/dev/null

swiftc -parse-as-library -emit-executable ios/HostApp/OpiApp.swift \
  -sdk "$(xcrun --sdk iphoneos --show-sdk-path)" \
  -target "arm64-apple-ios$IOS_MIN" -O -o "$hostbin"
chmod +x "$hostbin"

# IPA 就是 zip(Payload/Opi.app)。未签名，需重签后安装。
ipa="$out/opi-ios-keyboard-$version-unsigned.ipa"
mkdir -p "$out/Payload"
rm -rf "$out/Payload/$HOST_APP.app"
cp -R "$app" "$out/Payload/$HOST_APP.app"
( cd "$out" && zip -q -r -y "$(basename "$ipa")" Payload )
rm -rf "$out/Payload"
[ -f "$ipa" ] || { echo "!! IPA 未产出" >&2; exit 1; }
echo "IPA（未签名，需重签后安装）：$ipa"

# ---- 7) zip ----
# 只列**明确成员**（不用 `zip -r .`）：那样会把中间产物一起卷进去，也会把 zip 自己套进来。
# appex 已包含在 IPA 里，不再重复放进外层 zip。
members=("$FMWK.xcframework" "KeyboardExtension-Info.plist" "$(basename "$ipa")")
zip="$out/opi-ios-keyboard-$version-unsigned.zip"
( cd "$out" && zip -q -r "$(basename "$zip")" "${members[@]}" )

echo "---- iOS 产物（unsigned IPA / xcframework；未上机验证）----"
echo "  $out/$FMWK.xcframework"
echo "  $kb"
echo "  $appex（未签名扩展目录）"
echo "  $app（宿主 App）"
echo "  $ipa"
echo "  $zip"
echo "  版本：${version}（取自 Cargo.toml）"
