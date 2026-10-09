#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 把 `macos/` 的 Swift + Rust 通用库组装成**可装**的 macOS 输入法：
#
#   dist/apple/macos/OpiInputMethod.app                                  （ad-hoc 签名的 IME bundle）
#   dist/apple/macos/opi-inputmethod-<版本>-unsigned.pkg                 （pkgbuild 出的安装包）
#   dist/apple/macos/opi-inputmethod-<版本>-unsigned.zip                 （不想走 pkg 的人用这个）
#
# 用法（仓库根目录）：
#   scripts/apple/package-macos.sh <版本号> [产物目录]
#   版本号用 scripts/apple/version.sh 取（CI 里就是这么调的）。
#
# ===== ⚠️ 未签名 / 未公证：产物是**开发用**的，不是可发布的 =====
#
# 本脚本**没有** Developer ID 证书可用（仓库不带 secrets，也不连任何签名服务），
# 只做 **ad-hoc 签名**（`codesign --sign -`）。后果如实写在这里：
#   · 本机装得上（`installer` 会警告「未签名」，要 `-allowUntrusted`，或对 .app 右键打开）
#   · **不能**发给别人 —— 别人机器上 Gatekeeper 直接拦（「已损坏，无法打开」多半就是这个）
#   · 正式发布需要 Apple 开发者账号：Developer ID 证书 + `notarytool` 公证 + `stapler` 装订。
#     三件都不在本脚本范围内，缺了它们产物**只在本机有效**。
#
# ===== 前置：Rust 侧产物（本脚本**不**编 Rust，只读它的产物 + 自己 lipo）=====
#
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
#   cargo build --release -p opi_ffi --target aarch64-apple-darwin
#   cargo build --release -p opi_ffi --target x86_64-apple-darwin
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

rm -rf "$out"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"

sdk="$(xcrun --sdk macosx --show-sdk-path)"
# 桥接头就是仓库里那份声明面（不给 Xcode 留「第二份声明」的口子）。
flags=(-import-objc-header macos/OpiFFI.h -module-name "$MODULE" -sdk "$sdk" -O)

# ---- 1) 先 typecheck，再真编 ----
# 分两步是有用的：typecheck 不产生文件、结论直指源码错，比链接期报错好读得多。
# 两个 arch 的源码错是同一批，所以 typecheck 只跑一遍（宿主 arch）。
swiftc "${flags[@]}" -typecheck macos/*.swift

for arch in arm64 x86_64; do
  swiftc "${flags[@]}" -target "$arch-apple-macosx$DEPLOY" macos/*.swift \
    -o "$out/.$EXE.$arch" \
    -L "$libdir" -lopi_ffi \
    -framework Cocoa -framework InputMethodKit \
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
if grep -q '\$(' "$plist"; then
  echo "!! bundle 的 Info.plist 里还有没展开的 Xcode 占位符：" >&2
  grep -n '\$(' "$plist" >&2
  exit 1
fi
# 版本号**不从命令行注入**：Info.plist 里的 CFBundleShortVersionString 是仓库的
# 另一个同步位（`macos/README.md` 记着「它曾经落后过仓库版本」）。这里只**对账**，
# 不改写 —— 改写会让「plist 漂了」这件事在打包时被静默抹掉。
plist_ver="$(plutil -extract CFBundleShortVersionString raw "$plist")"
[ "$plist_ver" = "$version" ] || echo "⚠️ Info.plist 版本 $plist_ver ≠ 工作区版本 $version（发布清单里要同步；本脚本按仓库的规矩**不动** plist）"

# ---- 3) 放 Rust dylib 与图标 ----
cp "$dylib" "$APP/Contents/Frameworks/"
# tsInputMethodIconFileKey 指向 opi.tiff —— 键在 plist 里，文件得在 Resources 里。
cp macos/opi.tiff "$APP/Contents/Resources/opi.tiff"

# ---- 4) 签名（ad-hoc）----
# 顺序：先签里面那份 dylib，再签外层 bundle。反过来的话外层签名封的是**旧**哈希，
# `--verify --deep` 立刻红（这条顺序不是讲究，是必须）。
codesign --force --sign - "$APP/Contents/Frameworks/libopi_ffi.dylib"
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
# ~/Library/Input Methods/，见 macos/README.md）；pkg 装系统级，用户级请用上面的 .zip。
stage="$out/pkgroot"
mkdir -p "$stage/Library/Input Methods"
cp -R "$APP" "$stage/Library/Input Methods/"
pkg="$out/opi-inputmethod-$version-unsigned.pkg"
pkgbuild --root "$stage" --identifier "$BUNDLE_ID.pkg" --version "$version" \
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
rm -f "$out/.payload.txt"

# ---- 7) .zip（不想走 pkg 的人用；保留 .app 的结构与符号链接）----
ditto -c -k --keepParent "$APP" "$out/opi-inputmethod-$version-unsigned.zip"

echo "---- macOS 产物（未签名/未公证，开发用）----"
echo "  $APP"
echo "  $pkg"
echo "  $out/opi-inputmethod-$version-unsigned.zip"
echo "  版本：$version（取自 $(basename "$root")/Cargo.toml）；Info.plist 记的是 $plist_ver"
