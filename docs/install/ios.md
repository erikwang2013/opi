# iOS 键盘扩展 —— 构建 / 打包 / 安装 / 验证

> ## 这份文档的性质：**起点与契约，不是可用实现**
>
> 教程里的每一步都要分清**两层**，它们的状态差得很远：
>
> | 层 | 状态 | 判据 |
> |---|---|---|
> | Rust 侧 C ABI（`crates/opi-ffi`） | **已实测**能为 iOS 目标编译、能产出 `libopi_ffi.a` | 见「产出 libopi_ffi.a」一节，本机可复现 |
> | `ios/` 下的 Swift（`ios/*.swift`，数量不写死） | **2026-10-09 起：过了真编译器**（CI `apple.yml` 对 `ios/*.swift` glob 跑 `swiftc -typecheck` + 真编 + 与 Rust 静态库链接；2026-10-21 的 run 39486273199 对当前全部文件绿（含宿主 App 与键盘内设置面板））。但**运行时行为仍是 0 验证** | 本机仍是 Linux（`import UIKit` 就断），编译这一层由 CI 的 macos-latest 补上；见 §4.5 |
>
> **别把「Rust 侧能编」读成「iOS 端能用」。** 本项目已经被「没被编译器看过的代码」
> 坑过两次，本目录的存在正是为了避免第三次：
>
> | 前两次 | 代价 |
> |---|---|
> | fcitx5 C++ 插件 | 写在仓库里、README 记着「待验收」，一编译发现 **7 处 API 误写**（连 `fcitx::InputMethod` 这个类都不存在） |
> | Windows TSF | README 记着「完成，待在 Windows 上验收」，实测 `DllGetClassObject` 恒返 `CLASS_E_CLASSNOTAVAILABLE`、**文本插入从未实现** → 整个端不可用 |
>
> 所以本教程的目标不是「照着做就能用」，而是**把第一轮编译要撞的墙提前标出来**
> （Swift 待修项见 `ios/README.md` 的「我最不确定的 API 用法」），
> **并说清哪些步骤走完之后才算真的接对了**（见「验证」一节）。
> 更细的依据与全部实测记录在 `ios/README.md`；C ABI 声明面在 `crates/opi-ffi/src/cabi.rs`。

---

## 1. 前提

| 项 | 要求 | 依据 / 状态 |
|---|---|---|
| Mac + Xcode | 必须有。**仓库没有声明最低 Xcode 版本**，也没有声明 iOS 部署目标 —— 以 Xcode 新建工程时的模板默认值为准 | 未核实：仓库里没有任何 deployment target / Xcode 版本的声明 |
| Rust | `Cargo.toml` 的 `[workspace.package] rust-version`（当前声明 1.88）。**这是声明**，实际下限是 CI 里那个 MSRV 执行点量出来的 | `Cargo.toml` · `.github/workflows/ci.yml` |
| Rust target | `aarch64-apple-ios`（真机 arm64）、`aarch64-apple-ios-sim`（Apple Silicon 模拟器）。Intel Mac 的模拟器才需要 `x86_64-apple-ios` | CI 只跑前两个 + 两个 darwin 目标 |
| 词库 | `data/generated/luna.opid`（必需，937,780 条）、`data/generated/trad.opid`（可选，缺了繁体模式回退简体库；139,355 条，2026-10-18 起含现代繁体词与康熙差集）。`fallback.opid` **不用**打进包 —— 内置回退库已编译进二进制 | `ios/README.md`「词库文件」 |
| 宿主 App | 键盘扩展**不能独立安装**，必须内嵌在一个 App 的 `PlugIns/` 里。仓库现在提供最小宿主 App：`ios/HostApp/OpiApp.swift`（无 storyboard），由 `package-ios.sh` 用 `swiftc` 直接编成 `Opi.app` | 这条很确定；2026-10-20 起 CI 会产出 `Payload/Opi.app` 的 unsigned IPA |
| 工程文件 | **仓库里没有 `.xcodeproj`、没有 `Package.swift`、没有 `module.modulemap`** —— 用 Xcode 集成要在 Mac 上新建工程；CI 打包走 `package-ios.sh` 的 `swiftc` 直编路线 | `macos/README.md` 解释了为什么故意不手写 `project.pbxproj` |

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
```

---

## 2. 产出 `libopi_ffi.a`（本教程唯一在本机实测过的部分）

`crates/opi-ffi/Cargo.toml` 的 `crate-type` 已含 `staticlib`（实测：`grep -n 'crate-type' crates/opi-ffi/Cargo.toml`
→ `["cdylib", "staticlib", "lib"]`），**不需要再改 Cargo 配置**。
> ⚠️ `ios/README.md` 的「构建集成」一节还写着「没有 staticlib / 请由构建负责人确认后再加」——
> **那句已过期**，以 `crates/opi-ffi/Cargo.toml` 为准。

先在 Mac 上确认这台机器与 CI 看到的是同一个东西（**只在 Linux 上能跑的那几条 check，
本机实测 exit 0**；在 Mac 上重跑一遍是为了排除环境差异）：

```
$ cargo check -p opi_ffi --target aarch64-apple-ios
    Checking engine-core v1.3.1 (...)
    Checking engine-data v1.3.1 (...)
    Checking opi_ffi v1.3.1 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.97s      # exit 0
$ cargo check -p opi_ffi --target aarch64-apple-ios-sim                       # exit 0
$ cargo check -p opi_ffi --target aarch64-apple-darwin                        # exit 0
```
（输出里的 `v1.3.1` 是**跑这些命令时**的工作区版本，不是当前版本 —— 当前版本以 `Cargo.toml` 为准。
本文照原样保留实测记录，不把记录改写成「看起来是最新的」。）

真要出库：

```bash
# 真机 slice
cargo rustc -p opi_ffi --target aarch64-apple-ios --release --crate-type staticlib
# Apple Silicon 模拟器 slice
cargo rustc -p opi_ffi --target aarch64-apple-ios-sim --release --crate-type staticlib
```

本机实测（Linux，无 Apple 链接器也能出 —— `staticlib` 这一步不需要链接器）：

```
$ file target/aarch64-apple-ios/release/libopi_ffi.a
    current ar archive
$ ar x libopi_ffi.a <其中一个成员> && file <那个 .o>
    Mach-O 64-bit arm64 object, flags:<|SUBSECTIONS_VIA_SYMBOLS>
```

**导出符号齐不齐**——别数个数，做**集合比对**（两边名字集合完全相等才算数）：

```bash
grep -A1 'unsafe(no_mangle)' crates/opi-ffi/src/cabi.rs | grep -o 'fn opi_[a-z_]*' \
      | sed 's/fn /_/' | sort -u > /tmp/rs.txt
grep -a -o '_opi_[a-z_]*' target/aarch64-apple-ios/release/libopi_ffi.a | sort -u > /tmp/a.txt
diff /tmp/rs.txt /tmp/a.txt        # 本机实测：无输出（集合相等）
```
**本文件不写导出条数** —— 它随每次扩容变（`cabi.rs` 已经扩容过多次；
`ios/README.md` 记了一份历史，但**那份历史的末尾同样会过期**），
写死的数字只是把下一次假留给下一个人。条数以 `crates/opi-ffi/tests/c_abi_contract.rs`
那道门禁为准（`cargo test -p opi_ffi --test c_abi_contract`）。

⚠️ 方法是**降级**的：本机的 GNU `nm` / `objdump` **不认 Mach-O**，所以上面用的是
`grep -a` 读归档里的符号名字符串。它证的是「归档里存在这些名字」，
**不是**「链接器能解析它们」。在 Mac 上要拿真正的符号表，用 `nm -gU libopi_ffi.a`
或 `xcrun nm`。

⚠️ `cargo build`（不带 `--crate-type`）在 Linux 上**会失败**，因为 `crate-type` 含 `cdylib`：
```
error: linking with `cc` failed: exit status: 1
  = note: cc: error: unrecognized command-line option '-arch'
```
判据：错在**链接**这一步、且是 `cc` 不认 Apple 参数 → 换台装了 Xcode 的 Mac 就好。
**这不是代码缺陷**，别照着它去改 Cargo 配置。

### 两个 slice 合成 XCFramework（可选，但真机 + 模拟器都跑就需要）

```
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libopi_ffi.a \
  -library target/aarch64-apple-ios-sim/release/libopi_ffi.a \
  -output OpiFFI.xcframework
```
⚠️ 这条命令**凭记忆写的，本机无法执行**（`xcodebuild` 不存在）。Mac 上以 `xcodebuild -help` 为准。

---

## 3. 在 Xcode 里建 target

**Xcode 模板是权威，`ios/Info.plist` 是按记忆写的** —— 建完 target 之后拿模板 plist
与本目录的做**逐键比对**，而不是反过来。

1. 新建 iOS App（宿主，名字随意）→ 再加一个 **Keyboard Extension** target。
2. 把 `ios/` 下**全部** `.swift`（`ios/*.swift`，以 `ls` 为准）加进**扩展** target；
   `ios/HostApp/OpiApp.swift` 归宿主 App，不进扩展。
   （`OpiFFI.h` **不是**编译单元，它是桥接头，见下一步。）
3. 设置 `SWIFT_OBJC_BRIDGING_HEADER = ios/OpiFFI.h`。
   `ios/OpiFFI.h` 是**转发头**（`#include "../macos/OpiFFI.h"`），相对路径按**它自己所在目录**
   解析 ⇒ 只在**整个仓库一起 checkout** 时成立；单独把 `ios/` 拷出去会断（这是有意的：
   断了比悄悄用一份陈旧副本好）。这里只准有一份 C 声明，**不要在 Swift 里再抄一遍**
   `@silgen_name` 之类的第二份。
4. 链接 `libopi_ffi.a`（Build Settings → Other Linker Flags / 或把 `.xcframework` 拖进 target）。
5. 把 `data/generated/luna.opid`（+ 可选 `trad.opid`）加进**扩展** target 的资源里。
   注意 `KeyboardViewController.loadDictionaries()` 用的是 `Bundle.main.path(forResource:ofType:)`，
   **扩展的 `Bundle.main` 是扩展自己**，不是宿主 App。
6. 首次编译的预期：**一堆 API 报错，这是正常的**。待修清单按不确定程度排好了，
   在 `ios/README.md` 的「我最不确定的 API 用法」（`UIKey` / `UIKeyboardHIDUsage` 成员名、
   `textWillChange/textDidChange` 的签名、`size_t` 导入成 `Int` 还是 `UInt`）。
   **把编译错误当成这份草案的待办清单，而不要当成本目录作者写错了什么** ——
   那个清单的长度本身就是这份草案可信度的度量。

   > ⏳ **2026-10-09 订正：首次编译已经发生**（CI `apple.yml`，见 §4.5）。实际撞出来的
   > 编译错误**只有一类**：`KeyboardLayout.layer` 与 `UIView.layer` 同名冲突
   > （改名为 `keyLayer`）。上面点名的几个「凭记忆」项 —— `UIKey` / `UIKeyboardHIDUsage`
   > 成员名、`textWillChange/textDidChange` 签名、`size_t` 的导入形态 —— **都过了编译器**
   > （只剩两条警告：`contentEdgeInsets` 已废弃、`toggleFullwidth()` 返回值未用）。
   > 编译过 ≠ 能用：**运行时仍然一步没走**。

> 仓库里**没有** Xcode 工程文件是有意的：手写一个从未被 Xcode 打开过的 `project.pbxproj`
> 会是第三件「没被工具看过的产物」，且比 Swift 更难人工核对。同理**不用** nib/storyboard。

---

## 4. 打包

产物是**键盘扩展**，包类型与宿主 App 不同：

| 项 | 值 | 状态 |
|---|---|---|
| `CFBundlePackageType` | `XPC!` | ⚠️ **凭记忆写的，Mac 上核对**（`ios/Info.plist` 里就带着这条注释） |
| `NSExtensionPointIdentifier` | `com.apple.keyboard-service` | 凭记忆 |
| `NSExtensionPrincipalClass` | `$(PRODUCT_MODULE_NAME).KeyboardViewController` | 必须与 `KeyboardViewController.swift` 的 `@objc(KeyboardViewController)` **一致**；**启动即崩先查这个** |
| `NSExtensionAttributes` | `IsASCIICapable` / `PrefersRightToLeft` / `PrimaryLanguage` / `RequestsOpenAccess` | **四个键名全部凭记忆**，Mac 上以 Xcode 模板为准 |
| 显示名 | `CFBundleDisplayName` = `小欧输入法` —— 设置里键盘列表显示的就是它 | —— |

`RequestsOpenAccess = true`（2026-10-20 起）：自有贴纸要写 `UIPasteboard` 才能被宿主粘贴，
无完全访问时这一步会被系统拒绝；用户须在设置里手动允许，系统会展示隐私警告页。
OPI 仍然不联网，用户词仍只写扩展自己的容器。

**先做格式检查，再做逐键比对：**
```bash
plutil -lint ios/Info.plist      # 只能在 Mac 上跑
```
⚠️ 本机（Linux）**没有** Apple 的 `plutil`；`/usr/bin/plutil` 是 **GNUstep** 的，
它对这份文件报 `non-NSData data argument passed to method`（exit=1）——**那是假红**，
不代表 plist 有问题。别把它当缺陷。

---

## 4.5 CI 自动打包产物（unsigned IPA —— 需重签后安装）

`.github/workflows/apple.yml`（macos-latest，三个 job：`rust` / `macos` / `ios`）每次运行都把
`ios/` 的 Swift 真过一遍编译器（`swiftc -typecheck` → 真编 → 与 Rust 静态库链接），
并组装宿主 App + 扩展、产出 **unsigned IPA**。iOS artifact 只上传这一个 IPA；
xcframework/扩展目录/宿主 `.app`/临时 Payload 等中间产物只在脚本的临时构建目录里，退出时清理。
产物挂在**该次 run 的 Artifacts** 里。取法：

```bash
gh run list --workflow apple.yml                    # 找 run id（或网页 Actions 页）
gh run download <run-id> -n "opi-ios-keyboard-v$(bash scripts/apple/version.sh)"
```

| 产物名（版本号 = `Cargo.toml` 的 `[workspace.package] version`，唯一取法在 `scripts/apple/version.sh`） | 内容 |
|---|---|
| `opi-ios-keyboard-v<版本>` | `opi-ios-keyboard-<版本>-unsigned.ipa`（**唯一产物**；`Payload/Opi.app`，内含 `Opi` 可执行、内嵌 `PlugIns/OpiKeyboard.appex`、`luna/trad/stroke/wubi86` 词库、`rare_index.tsv` 与宠物图标）。xcframework/扩展目录/宿主 `.app`/临时 Payload 只在构建期存在，脚本退出时清掉 |
| `opi-apple-rust-v<版本>`（rust job） | `opi_ffi.xcframework`（同上的静态库版）+ `headers/OpiFFI.h` + macOS 用的通用库 |

**IPA 的目录结构**（CI 会逐条校验）：

```
Payload/Opi.app/
  Opi                         # 宿主 App 可执行（ios/HostApp/OpiApp.swift）
  Info.plist                  # CFBundleIdentifier = xyz.erik.opi
  AppIcon60x60@2x.png …
  PlugIns/OpiKeyboard.appex/
    OPIKeyboard               # 扩展可执行（入口 _NSExtensionMain）
    Info.plist                # CFBundleIdentifier = xyz.erik.opi.keyboard
    luna.opid trad.opid stroke.opid wubi86.opid rare_index.tsv
```

**这次 run 真实证明了什么**（历史证据 run 37935240583，以及 2026-10-20 起的 IPA 结构校验）：

- `ios/` 的 Swift 过 `swiftc -typecheck` + 真编译（device 与模拟器两个 SDK 各一遍）；
- framework 的 `nm -gU` 里有 `_opi_load` 等导出、`nm -u` 里**没有**未定义的 `opi_` 符号
  ⇒ Rust 静态库**真的链进去了**（不是「链接命令没报错」就算）；
- `otool -L` 里**没有** `libopi_ffi` ⇒ 是静态链接（iOS 扩展带第三方 dylib 装不上）；
- `xcodebuild -create-xcframework` 成功（仅构建期校验，产物不随 IPA 发布）；
  扩展可执行（`.appex`）也链出来了（入口 `_NSExtensionMain`，链接期实测可过）；
- IPA 里 `Payload/Opi.app/Opi`、`PlugIns/OpiKeyboard.appex/OPIKeyboard`、`luna.opid`
  都在，宿主/扩展 plist 通过 `plutil -lint`，扩展 bundle id 以宿主 id 为前缀。

**它不能做什么（现状，别读成「降级」）**：

- **不能直接安装**：仓库没有 Apple 签名证书，IPA 是**未签名**的。iOS 不会接受未签名包；
  需用 Sideloadly / AltStore 等工具以用户自己的 Apple ID 重签后再装（免费账号签的
  App 7 天过期，付费开发者账号 1 年）。想产出可直接安装/上架的签名 IPA，需要 Apple
  Developer 证书 + provisioning profile，并给 CI 配签名步骤 —— 当前没有。
- **运行时 0 验证**：键盘在系统里出不出现、按键进不进引擎、候选栏有没有字、上屏对不对
  —— 全部未测（§6 的五步一步都没走）。**编译过、打包结构对 ≠ 能用。**
- 模拟器只有 arm64 切片：Intel Mac 的模拟器要 `x86_64-apple-ios`，本工作流没构建
  （要加就是 `scripts/apple/package-ios.sh` 里 `for spec` 多一行，没有别的机关）。
- Artifact 默认 90 天过期、随每次 run 重出 —— 它是 **CI 证据**，不是发布渠道。

想在本机复现（不用 CI）：Mac 上先装两个 Rust target、两条 `cargo build`（前置条件写在脚本头部），
然后 `bash scripts/apple/package-ios.sh "$(bash scripts/apple/version.sh)"`。

⚠️ **自己写链接行时的一个坑**（CI 首轮实测撞出来的）：`crate-type` 含 `cdylib`
⇒ `target/<iOS 三元组>/release/` 里 `libopi_ffi.dylib` 与 `libopi_ffi.a` **并存**，
而 `-L … -lopi_ffi` 的搜索**优先挑 `.dylib`** —— 链接会「成功」，但符号全变成未定义导入
（症状是 `nm -gU` 里一个 `opi_` 都看不到）。本工作流按**显式路径**直链 `.a`。

---

## 5. 安装与启用（真机）

有两条路：用 Sideloadly/AltStore 重签 CI 的 unsigned IPA，或用 Xcode 自己签。

### 5.1 用 unsigned IPA（需要重签工具）

1. 下载 CI 产物里的 `opi-ios-keyboard-<版本>-unsigned.ipa`。
2. 在电脑上用 [Sideloadly](https://sideloadly.io/) 或 AltStore 打开它，登录你自己的
   Apple ID，让工具重签并安装到 iPhone/iPad。工具会同时重签宿主 App 和内嵌的
   `OpiKeyboard.appex`；不要只签宿主、漏掉扩展。
3. 首次安装后若被系统拦（未受信任的开发者），到
   **设置 → 通用 → VPN 与设备管理** 里信任你的开发者证书。
4. 真机上：**设置 → 通用 → 键盘 → 键盘 → 添加新键盘 → 第三方键盘 → 小欧输入法**。
   扩展**不能自己把自己启用**（无法引导、无法弹窗），这一步只能用户手动做。
5. 打开任意文本框，长按地球键切到 OPI。
6. **密码框里第三方键盘默认不可用**，系统会强制用内置键盘 —— 这是系统行为，不是缺陷。

⚠️ unsigned IPA **不能**用 AirDrop / Finder / `ideviceinstaller` 直接装 —— 那些工具不做重签。
免费 Apple ID 签出的 App 每 7 天要重签一次；付费开发者账号是 1 年。

### 5.2 用 Xcode

1. 用 Xcode 把**宿主 App** 跑到真机上 —— 扩展随宿主 App 一起装进 `PlugIns/`。
   （模拟器对第三方键盘的支持不可靠，**未核实**，建议直接用真机。）
2. 之后同 §5.1 的第 4–6 步。

排障顺序（从最可能到最不可能）：

| 症状 | 先查 |
|---|---|
| 设置里根本看不到这个键盘 | 扩展是否真的进了宿主的 `PlugIns/`；`NSExtensionPointIdentifier`；宿主 App 是否至少被打开过一次（**未核实**，我记的是「要先打开过一次」） |
| 能看到、选中就闪退 | `NSExtensionPrincipalClass` 的模块名前缀是否与 `@objc(KeyboardViewController)` 对得上 |
| 键盘能显示、按键无反应 | 桥接头是否生效（`SWIFT_OBJC_BRIDGING_HEADER`）；链接是否真的接上了 `libopi_ffi.a` |
| 键盘有键、无候选 | 词库是否打进**扩展**的资源（不是宿主 App 的） |
| 用一会儿键键盘消失 | 大概率是**扩展内存超限被系统杀**（见「已知限制」） |

日志：所有 `NSLog` 都带 `[OPI]` 前缀，用 Console.app 或 Xcode 的设备日志按它过滤。

---

## 6. 验证 —— 怎么知道接对了

按顺序走，**每一步只证明一件事**，别跳：

1. **设置里能看到「小欧输入法」** ⇒ 扩展被打包正确、被系统识别（`NSExtension*` 那组键对了）。
   —— 此时还不能说明任何代码能跑。
2. **切到 OPI，敲软键盘 `n` `i`** ⇒ **候选栏出现候选**（`ios/README.md`「第一件要做的事」第 4 步）。
   这一步是「Rust 引擎 + 词库 + FFI 桥 + 键盘 UI」整条链路的第一个可见成功点。
   候选内容**本文不预测** —— 引擎的排序行为由 Rust 侧决定，以实测为准。
3. **敲 `h` `a` `o` `空格` ⇒ 提交「好」**（跨端最小冒烟）。
   这条**逐字对齐 fcitx5 轨的实测表**（`crates/fcitx5-opi/cpp/README.md`「状态」一节：
   `h`/`a`/`o`/`SPACE` 的 action 是 1/1/1/2，提交 `好`），`macos/README.md` 要求 Apple 两端
   与它一致。**同一个引擎、同一套语义，两端的最小可见成功必须一致**；
   不一致就是桥接层的问题，不是引擎的。
4. **输入时顶部 38pt 拼音行显示 `ni'hao`**；9 键读数只覆盖前缀时，未覆盖的键按
   **键面首字母**补全（完整串，不出现「·」/省略号），超出屏幕可横向滑动。
   9 键输入 `926248694364`（yanchunzheng 的 T9 串）应出现「厌蠢症」；点 123 后
   2–9 直接进输入框（半角）。
   **长按 ⌫ = 按住连续删除**；点工具条任一功能页再返回，键盘仍是 OPI（不跳系统键盘）。
   **🌐 短按在 OPI 内循环语言，长按才切系统输入法**；9 键底行繁/123/英文都应留在 OPI
   且**不再闪退**（1.24.5 按真机 .ips 修复了键面重建时的 Auto Layout 崩溃）。
   拼音行空闲时顶部应出现**随机多形态宠物条**（1–8 枚，约 2.8s 换一组）。
   这些 2026-10-23 改过，真机必验。
5. 只有 1–4 都过之后，才谈功能（符号面板、繁体外词管理、⇧ 高亮……）。

> **这些验证的顺序不能反。** 在编译通过之前做的任何功能工作，都是在给一份可能整体重写的代码上加东西。

### 6.5 键盘版式（2026-10-24 参考图改版，源码层）

与 Android / 鸿蒙同一套参考图（Android 落点见 `android.md` §4.5b）。**这轮改动还在
工作区、没有提交，因此没有被任何 CI 编译过；本机（Linux）也编不了 UIKit。以下全是
源码核对，不是真机结论。** 协议值不变：普通点击仍发原来的 T9 数字 / 14 键首字母 /
26 键小写码点。

- **9 键**（`InputModes.nineKeyRows`）：主区 3×3（第一格分词键），每格右上角序号 1–9
  （`digitKey` 的 `cornerHint`，分词格固定 `"1"`）；长按/上滑弹层 = 字母（字号递减）+
  **末尾数字**，`popupDefaultIndex = items.count - 1`。**大 9 键只隐藏角标**
  （`big ? nil : digit`），弹层照旧。
- **9 键左列**：`.scrollingColumn` 六格（五标点 + 自定义），格内按下 `defersPress`
  （否则滚动会先打出字符）。自定义格未选时点开 `KeyPopup.swift` 的 `PunctuationPicker`，
  数据是 `KeyboardViewController` 传入的 `engine.commonSymbols()`（仍然只有 common 块，
  见 L1）；选中值存 `PunctuationPreference.custom`（`InputModePreference.swift`，
  UserDefaults），已选后点按直提、长按重开。
- **14 键**（`InputModes.fourteenKeyRows`）：`qw er ty ui op` / `as df gh jk l` /
  `⇧ zx(~) cv(!) bn(@) m(?) ⌫`；前两行数字提示 1–5 / 6–0，第三行符号提示 ~ ! @ ?。
  弹层只在第三行四枚符号键上（`ModeKeySymbols.fourteenPair`）：`[大写对, 符号, 小写对]`、
  默认中间项；提交值是**协议首字母**。底行 `🌐 符 123 ， 空格 。 中英 换行`
  （`🌐` 是 iOS 系统切换入口，保留；模式键走三态循环 —— 本盘没有独立英文键）。
- **26 键**（`ModeKeyTables26.letterRows`）：拼音 26 无数字行、键面大写、Q–P 角标 1–0；
  英文 26 有 0–9 数字行、键面小写；字母弹层 `[大写, 符号, 小写]`、默认中间项
  （`ModeKeySymbols.letterSpec`，`popupDefaultIndex = 1`）。**五笔**：键面同拼音 26，
  第三行最左换成「拼」（`.selectInputMode(.pinyin26)` → 既有 `InputModeApply.applyInputMode`，
  不是只改 UI）、空格键面写「五笔」（发送仍是空格）。
- **9 键读数（2026-10-24 修复）**：`KeyboardCandidates.refresh` 先读
  `engine.displayBuffer()`；为空且 `layout.wantsReadings` 时用 `readings.first` 兜底，
  仍空就**留空**，绝不回退 `buffer` 的数字串（`hou` → `468` 的成因）。非读数方案
  （26/14/笔画）仍回退字母缓冲。

**2026-10-25 键面/底行订正（源码层）**：9/大 9 底行「英文」改「英」，左标点列窄一档
（`ModeCell.scrollingColumn.widthUnits = 0.8`、17pt）；右列空格换 **0**（`KeySpec.char("0")`），
空格移到底行正中并加宽（`widthUnits: 2.4`）。26 键底行模式键按目标语言写「英/中」，新增
`.toggleChineseEnglish` 一次点击直达（中文 → `english26`，英文 → `lastChineseInputMode`）；
14 键底行「中英」改「英」，且**所有 14 键字母键**的弹层都改为 `[大写对, 数字/符号, 小写对]`、默认中间项并走同一条。顶部宠物条槽位 24→38pt，多枚按条宽铺满；根视图铺主题底色以适配顶部圆角。弹层保持横排。9/14/26 键弹层统一**大写在前**、小写随后、符号/数字末位：小写进引擎匹配词库，大写/数字/符号**直接原样上屏**；大 9 键没有数字/符号的格（分词）不弹层。

**未验**：这些键位的实际手感、弹层方向/裁切、左列滚动与选择器在真机上的表现；§8
的「运行时验证数 = 0」不变。

---

## 7. 已知限制（诚实清单）

**这些不是 bug，是「还没做」或「有意的取舍」** —— 交付时请如实转述，别当缺陷重报，
也别当没看见。

| # | 限制 | 说明 |
|---|---|---|
| L1 | **符号面板只拿得到 common 块** | `KeyboardViewController` 调的是 `layout.setSymbols(engine.commonSymbols())`，而 `commonSymbols()` 只遍历 `opi_symbol_blocks()` —— 那个出口**只回 common 块**，当前数据表里只有 CJK 符号块是 common ⇒ **非 common 块里的条目（含 emoji）在 iOS 上够不到**。缺口在 UI 上**没有症状**（面板照样有内容，只是少）。`searchSymbols(keyword:)`（包装 `opi_search_symbols`）现在只被**表情搜索**调用（`KeyboardViewController.runEmojiSearch`），符号面板本身仍只拿 common 块。**别把「表情能搜全表」读成「符号面板能拿到全部符号」。** |
| L2 | **用户词不与宿主 App 同步** | 当前 `RequestsOpenAccess = true`（为贴纸剪贴板），但键盘仍只写扩展自己的容器；App 里做的词库管理看不到键盘攒的词。要同步还要显式接 App Group 共享容器。 |
| L3 | **无障碍未接** | 每个键都没有 `accessibilityLabel`（⇧ / ⌫ / 回车 / 🌐 这类符号键尤其需要），VoiceOver 读不出来。已知缺口。 |
| L4 | **键盘高度是拍的** | `viewWillAppear` 里写死 264pt（`priority = .defaultHigh`），**没按内容算、没处理旋转**。真机上要改。 |
| L5 | **⇧ 三态高亮 UI 未接** | 出口（`opi_shift_state`）与 Swift 包装（`OpiEngine.shiftState()`）都已就绪，**UI 没读**。所以也仍然没有会漂移的镜像。 |
| L6 | **外接键盘那一路风险最高** | `UIKey` / `UIKeyboardHIDUsage` 的成员名只做过**文档核对**（修掉一个：真名是 `.keyboardDeleteOrBackspace`，不是 `.keyboardBackspace`）。⏳ 2026-10-09：这一节**已过编译器**（CI 真编，见 §4.5）—— 但**运行时**从未接过硬件键盘，风险仍在（`ios/README.md` 的原建议：先整节注释掉、纯软键盘跑通后再单独接，作为「万一运行时炸」的退路仍然有效）。 |
| L7 | **REPEAT 位在 iOS 侧是预防性的** | `UIKeyModifierFlags` 里有没有 autorepeat 信号属于**记忆级**证据，未核对。`modifierStates(_:)` 里已写明「哪天真找到就加在这里，别在调用处手搓一个位」。 |
| L8 | **繁体库不放就回落简体** | `trad.opid` 是可选资源；缺了不报错，繁体模式静默用简体库。 |
| L9 | **长按 ⇧ → Lock 走的是另一套机制** | 软键盘长按 → `stateLongPressed`，与桌面轨的 `LONG_PRESSED` 键状态位不是同一条路。 |
| L10 | **候选栏空态没有小欧** | 其他端（Android 候选栏、Windows 候选窗、鸿蒙）在「打不出候选」时会显示小欧 + 提示；iOS 的 `KeyboardLayout.updateCandidates(_:)` 在空数组时先清空子视图再 `return` ⇒ **纯空白**。接它只能走位图（`ios/README.md` §6 明文禁止第三份 CoreGraphics 几何），而 iOS 现在连 `.xcassets` 都没有。**本版没做。** |
| L11 | **自定义短语：没有 UI（2026-10-24）** | C ABI 有中文套出口，`ios/OpiEngine.swift` 也包了 `customPhrasesEnabled` / `exportCustomPhrases` / `setCustomPhrases`，但**没有英文套包装、没有设置页、没有宿主文件装载**（`ios/` 下没有任何 `*Phrase*` 文件）⇒ 用户在 iOS 上配不了，重启也没有表可装。别把 Android / macOS 的「自定义短语」入口读成 iOS 也有。 |

---

## 8. 未验证清单（照抄现状，别读成已完成）

**本目录的验证现状**（2026-10-09 更新，别读成已完成）：

- `ios/` 下**全部** `.swift`（`ios/*.swift`）：没有 lint、没有单测；**编译**这一条 2026-10-09 起
  由 CI 补上（`apple.yml`，run 37935240583 编的是当时 6 个：typecheck + 真编 + 静态链接 +
  `nm` 未定义符号检查；2026-10-20 run 39425977787 编过当前全部文件，见 §4.5）。
  **运行时（装进系统之后）验证数依旧是 0。**
- `ios/Info.plist`：本机只验过**格式**（`plistlib` 能解析）；CI 里也只得过
  `plutil -lint` / `plutil -replace`（同样是格式层，`-replace` 对不存在的键照样成功）。
  **键名与取值的语义全部凭记忆**。
- `ios/OpiFFI.h`：**转发头**，自身没有独立内容（唯一的一份声明在 `macos/OpiFFI.h`）。
- **2026-10-24 键盘改版与 9 键读数修复**（`InputModes.swift` / `ModeKeyTables26.swift` /
  `ModeKeySymbols.swift` / `KeyPopup.swift` / `KeyboardCandidates.swift` /
  `KeyboardPanelHost.swift` 等）：工作区未提交 ⇒ **不在任何 CI run 里**，本机也编不了。
  §6.5 的每条都只是源码核对；运行时验证数仍是 0。

**做过、但强度有限的检查（别当成「编译过了」）：**
- Swift 里 `OpiKey` / 状态位常量与 `crates/engine-core/src/keys.rs` 的**数值**逐项比对 ——
  这只证「常量抄对了」，不证能编译。
- `cabi.rs` ↔ `macos/OpiFFI.h` 的**名字 + 签名**集合比对（有门禁
  `crates/opi-ffi/tests/c_abi_contract.rs` 守着）。这只证**声明面**对齐，不证 Swift 能调。
- `KeyboardLayout.swift` / `KeyboardHardware.swift` 两次拆分做过的**定义集合 ↔ 引用集合**比对 ——
  只证「搬移没丢东西」，不证能编译。

**真正过了工具的东西**（截至 2026-10-09）：Rust 侧 —— `cargo check` 的 iOS 目标
（见「产出 `libopi_ffi.a`」）、含全部导出的 arm64 `.a`；**Swift 侧** —— 当时 `ios/` 的
6 个 `.swift` 过了 typecheck + 真编 + 静态链接 + `nm` 无未定义符号
（CI run 37935240583，见 §4.5）。**还没过任何工具的**：运行时行为 ——
一段代码都没在 iOS 上真正跑过。

---

## 9. 下一步顺序（别跳）

```
1. 编译通过（Xcode 建 target，把 API 报错当待办清单消掉）
2. plutil -lint + 与 Xcode 模板逐键比对 Info.plist
3. 链接 libopi_ffi.a，符号全部解析；真机装上、设置里能看到键盘
4. 最小冒烟：n i 出候选；h a o 空格 →「好」
5. 只有 4 过了，才谈补功能（符号面板扩容、⇧ 高亮、用户词、无障碍标签）
```

**顺序不能反。** 更细的依据、每一项的证据与订正记录在 `ios/README.md`。
