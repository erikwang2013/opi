# iOS 键盘扩展 —— 构建 / 打包 / 安装 / 验证

> ## 这份文档的性质：**起点与契约，不是可用实现**
>
> 教程里的每一步都要分清**两层**，它们的状态差得很远：
>
> | 层 | 状态 | 判据 |
> |---|---|---|
> | Rust 侧 C ABI（`crates/opi-ffi`） | **已实测**能为 iOS 目标编译、能产出 `libopi_ffi.a` | 见「产出 libopi_ffi.a」一节，本机可复现 |
> | `ios/` 下的 Swift（6 个文件） | **一行都没有被编译过** | 本机是 Linux，没有 Xcode / Apple SDK，`import UIKit` 就断 |
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
| 词库 | `data/generated/luna.opid`（必需）、`data/generated/trad.opid`（可选，缺了繁体模式回退简体库）。`fallback.opid` **不用**打进包 —— 内置回退库已编译进二进制 | `ios/README.md`「词库文件」 |
| 宿主 App | 键盘扩展**不能独立安装**，必须内嵌在一个 App 的 `PlugIns/` 里 | 这条很确定 |
| 工程文件 | **仓库里没有 `.xcodeproj`、没有 `Package.swift`、没有 `module.modulemap`** —— 工程要在 Mac 上用 Xcode 新建 | `macos/README.md` 解释了为什么故意不手写 `project.pbxproj` |

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
2. 把 `ios/` 下 **6 个 `.swift`** 加进**扩展** target：
   `OpiEngine.swift` · `KeyboardKeys.swift` · `KeyboardLayout.swift` · `KeyboardHardware.swift` ·
   `SymbolPanel.swift` · `KeyboardViewController.swift`。
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
| 显示名 | `CFBundleDisplayName` = `OPI 拼音` —— 设置里键盘列表显示的就是它 | —— |

`RequestsOpenAccess = false` 是**有意选的**，不是漏了：OPI 是离线优先输入法，不需要网络；
用户词也只需要写在扩展自己的容器里。代价见「已知限制」。

**先做格式检查，再做逐键比对：**
```bash
plutil -lint ios/Info.plist      # 只能在 Mac 上跑
```
⚠️ 本机（Linux）**没有** Apple 的 `plutil`；`/usr/bin/plutil` 是 **GNUstep** 的，
它对这份文件报 `non-NSData data argument passed to method`（exit=1）——**那是假红**，
不代表 plist 有问题。别把它当缺陷。

---

## 5. 安装与启用（真机）

1. 用 Xcode 把**宿主 App** 跑到真机上 —— 扩展随宿主 App 一起装进 `PlugIns/`。
   （模拟器对第三方键盘的支持不可靠，**未核实**，建议直接用真机。）
2. 真机上：**设置 → 通用 → 键盘 → 键盘 → 添加新键盘 → 第三方键盘 → OPI 拼音**。
   扩展**不能自己把自己启用**（无法引导、无法弹窗），这一步只能用户手动做。
3. 打开任意文本框，长按地球键切到 OPI。
4. **密码框里第三方键盘默认不可用**，系统会强制用内置键盘 —— 这是系统行为，不是缺陷。

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

1. **设置里能看到「OPI 拼音」** ⇒ 扩展被打包正确、被系统识别（`NSExtension*` 那组键对了）。
   —— 此时还不能说明任何代码能跑。
2. **切到 OPI，敲软键盘 `n` `i`** ⇒ **候选栏出现候选**（`ios/README.md`「第一件要做的事」第 4 步）。
   这一步是「Rust 引擎 + 词库 + FFI 桥 + 键盘 UI」整条链路的第一个可见成功点。
   候选内容**本文不预测** —— 引擎的排序行为由 Rust 侧决定，以实测为准。
3. **敲 `h` `a` `o` `空格` ⇒ 提交「好」**（跨端最小冒烟）。
   这条**逐字对齐 fcitx5 轨的实测表**（`crates/fcitx5-opi/cpp/README.md`「状态」一节：
   `h`/`a`/`o`/`SPACE` 的 action 是 1/1/1/2，提交 `好`），`macos/README.md` 要求 Apple 两端
   与它一致。**同一个引擎、同一套语义，两端的最小可见成功必须一致**；
   不一致就是桥接层的问题，不是引擎的。
4. 只有 1–3 都过之后，才谈功能（符号面板、繁体外词管理、⇧ 高亮……）。

> **这些验证的顺序不能反。** 在编译通过之前做的任何功能工作，都是在给一份可能整体重写的代码上加东西。

---

## 7. 已知限制（诚实清单）

**这些不是 bug，是「还没做」或「有意的取舍」** —— 交付时请如实转述，别当缺陷重报，
也别当没看见。

| # | 限制 | 说明 |
|---|---|---|
| L1 | **符号面板只拿得到 common 块** | `KeyboardViewController` 调的是 `layout.setSymbols(engine.commonSymbols())`，而 `commonSymbols()` 只遍历 `opi_symbol_blocks()` —— 那个出口**只回 common 块**，当前数据表里只有 CJK 符号块是 common ⇒ **非 common 块里的条目（含 emoji）在 iOS 上够不到**。缺口在 UI 上**没有症状**（面板照样有内容，只是少）。`searchSymbols(keyword:)`（包装 `opi_search_symbols`）**存在但零调用者**。判据：`grep -rn 'searchSymbols' ios/ --include=*.swift` 只命中定义那一行。**别把「包装已写」读成「面板能拿到全部符号」。** |
| L2 | **用户词不与宿主 App 同步** | `RequestsOpenAccess = false` ⇒ 拿不到 App Group 共享容器。App 里做的词库管理看不到键盘攒的词。要同步必须改成 `true`，且用户会在设置里看到「完全访问」隐私警告页 —— **这是产品决策，不是技术细节**。 |
| L3 | **无障碍未接** | 每个键都没有 `accessibilityLabel`（⇧ / ⌫ / 回车 / 🌐 这类符号键尤其需要），VoiceOver 读不出来。已知缺口。 |
| L4 | **键盘高度是拍的** | `viewWillAppear` 里写死 264pt（`priority = .defaultHigh`），**没按内容算、没处理旋转**。真机上要改。 |
| L5 | **⇧ 三态高亮 UI 未接** | 出口（`opi_shift_state`）与 Swift 包装（`OpiEngine.shiftState()`）都已就绪，**UI 没读**。所以也仍然没有会漂移的镜像。 |
| L6 | **外接键盘那一路风险最高** | `UIKey` / `UIKeyboardHIDUsage` 的成员名只做过**文档核对**（修掉一个：真名是 `.keyboardDeleteOrBackspace`，不是 `.keyboardBackspace`），**没有任何编译器看过**。`ios/README.md` 的建议是：**先把这一节整个注释掉**，让键盘在纯软键盘下跑通，再单独接硬件键盘。 |
| L7 | **REPEAT 位在 iOS 侧是预防性的** | `UIKeyModifierFlags` 里有没有 autorepeat 信号属于**记忆级**证据，未核对。`modifierStates(_:)` 里已写明「哪天真找到就加在这里，别在调用处手搓一个位」。 |
| L8 | **繁体库不放就回落简体** | `trad.opid` 是可选资源；缺了不报错，繁体模式静默用简体库。 |
| L9 | **长按 ⇧ → Lock 走的是另一套机制** | 软键盘长按 → `stateLongPressed`，与桌面轨的 `LONG_PRESSED` 键状态位不是同一条路。 |

---

## 8. 未验证清单（照抄现状，别读成已完成）

**本目录从未被任何 Apple 工具看过的东西：**

- `ios/` 下 **6 个 `.swift`**：没有编译、没有 lint、没有单测 —— **验证数是 0**。
  第一条针对 Swift 的可执行验证就是「在 Mac 上编译通过」。
- `ios/Info.plist`：本机只验过**格式**（`plistlib` 能解析）。**键名与取值全部凭记忆**。
- `ios/OpiFFI.h`：**转发头**，自身没有独立内容（唯一的一份声明在 `macos/OpiFFI.h`）。

**做过、但强度有限的检查（别当成「编译过了」）：**
- Swift 里 `OpiKey` / 状态位常量与 `crates/engine-core/src/keys.rs` 的**数值**逐项比对 ——
  这只证「常量抄对了」，不证能编译。
- `cabi.rs` ↔ `macos/OpiFFI.h` 的**名字 + 签名**集合比对（有门禁
  `crates/opi-ffi/tests/c_abi_contract.rs` 守着）。这只证**声明面**对齐，不证 Swift 能调。
- `KeyboardLayout.swift` / `KeyboardHardware.swift` 两次拆分做过的**定义集合 ↔ 引用集合**比对 ——
  只证「搬移没丢东西」，不证能编译。

**唯一真正过了工具的两个东西**（都在 Rust 侧，不在 Swift 侧）：
`cargo check` 的 iOS 目标（见「产出 `libopi_ffi.a`」）、以及那个含全部导出的 arm64 `.a`。

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
