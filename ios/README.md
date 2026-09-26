# OPI iOS 端 —— **草案，从未编译，未验证**

> ## ⚠️ 先读这段
>
> 本目录的所有 Swift 代码**一个字符都没有编译过**。它是在 Linux 上写的：
> 本机没有 macOS、没有 Xcode、没有 Apple SDK，`import UIKit` 就断，
> 连语法检查都做不到。**不要把这里当成「iOS 端已完成」。**
>
> 这份东西的定位是：**给 Mac 开发者的起点 + 精确契约**。
> 它替你省掉「从零读 C ABI、从零搭键盘骨架」的时间，
> **不**替你做「代码能跑」这件事。
>
> 本项目已经被「没被编译器看过的代码」坑过两次，不要让它成为第三次：
> - **fcitx5 C++**：写在仓库里、README 记着「待验收」，一编译发现 7 处 API 误写
>   （连 `fcitx::InputMethod` 这个类都不存在）
> - **Windows TSF**：README 记着「完成，待在 Windows 上验收」，实测
>   `DllGetClassObject` 恒返 `CLASS_E_CLASSNOTAVAILABLE`、无 CLSID、
>   文本插入从未实现 → 整个端不可用
>
> 所以：**本目录里唯一经过验证的东西是「Rust 侧能为 iOS 目标编译」**（见下方实测记录），
> 其余全部是待验证的草案。

---

## 在 Mac 上第一件要做的事

**顺序不能反：先编译，再补功能。** 不要一上来就改 UI、也不要先看功能对不对 ——
在 Swift 代码从未过过编译器的情况下，「写得对不对」这个问题还没有意义。

```
第 1 步：建 target，让它编译过
  在 Xcode 里新建 Keyboard Extension target（不要用本目录的 Info.plist 当权威，
  用 Xcode 模板生成的做逐键比对），把 ios/*.swift 加进去，
  SWIFT_OBJC_BRIDGING_HEADER = ios/OpiFFI.h。
  → 目标：编译通过（大概率要修一堆 API 名字，见下方「最不确定的 API」）。
  → 此时功能仍然一个都不能用，这是正常的。

第 2 步：plutil -lint ios/Info.plist，并与 Xcode 模板逐键比对。

第 3 步：按下方「构建集成」把 libopi_ffi.a 弄出来，链接进去，
  确认 28 个符号都能解析（全称见 crates/opi-ffi/src/cabi.rs）。
  → 目标：链接通过 + 真机装上后能在设置里看到这个键盘。

第 4 步：在真机上打开、打一个 "ni" 看有没有候选。
  → 到这一步才算「iOS 端能用」的第一天。

第 5 步之后才是：补 UI、补符号面板、补用户词持久化。
```

**不要跳过第 1 步去「顺手重构一下分层」。** 这份代码的分层是按
「Rust 侧是唯一逻辑真源」设计的，改之前先读
`crates/engine-core/src/router.rs` 的模块头注释（那里解释了为什么
路由表不能再抄第四份）。

---

## 文件清单

| 文件 | 作用 |
|------|------|
| `OpiFFI.h` | **转发头**，`#include "../macos/OpiFFI.h"`。C ABI 的声明只有那一份，这里故意不重抄（本项目已被「同一语义抄三份」坑过） |
| `OpiEngine.swift` | C ABI 薄桥：`OpiString` 所有权、`opi_key_event` → Swift 枚举、JSON 候选解码 |
| `KeyboardLayout.swift` | 键盘 UI 最小骨架：候选栏（含翻页）+ QWERTY/数字层 + 按下抬起接线 |
| `KeyboardViewController.swift` | `UIInputViewController` 入口：生命周期、词库装载、按键路由、`textDocumentProxy`、外接键盘 |
| `Info.plist` | 键盘扩展的 `NSExtension` 配置（**键名未核对**） |
| `README.md` | 本文件 |

---

## 实测记录（唯一经过验证的部分）

### 1. Rust 侧为 iOS 目标编译 —— **通过**

C ABI 在 iOS 两个目标上都过了类型检查。`cargo check` 不链接，所以 Linux 上不需要 Apple SDK。

```
$ rustup target add aarch64-apple-ios aarch64-apple-ios-sim
（exit 0）

$ cargo check -p opi_ffi --target aarch64-apple-ios
    Checking engine-core v1.0.13 (/home/wwwroot/bag/opi/crates/engine-core)
    Checking engine-data v1.0.13 (/home/wwwroot/bag/opi/crates/engine-data)
    Checking opi_ffi v1.0.13 (/home/wwwroot/bag/opi/crates/opi-ffi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.87s
（exit 0）

$ cargo check -p opi_ffi --target aarch64-apple-ios-sim
    Checking opi_ffi v1.0.13 (/home/wwwroot/bag/opi/crates/opi-ffi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.57s
（exit 0）
```

**这说明什么**：`opi-ffi` 及其依赖（`engine-core`、`engine-data`、`jni`、`serde_json` 等）
在 `aarch64-apple-ios` / `-ios-sim` 上**没有平台相关代码编译不过**。
**不说明什么**：没链接、没运行、没验证任何行为。

### 2. 静态库真的能产出，且是 arm64 Mach-O —— **通过**

（`AR_aarch64_apple_ios=ar` 是因为本机没有 Apple 工具链，让 rustc 用 GNU `ar`。）

```
$ AR_aarch64_apple_ios=ar cargo rustc -p opi_ffi --target aarch64-apple-ios \
      --release --crate-type staticlib --target-dir /tmp/opi_ios_v3
    Finished `release` profile [optimized] target(s) in 29.93s
（exit 0）

$ ls -la /tmp/opi_ios_v3/aarch64-apple-ios/release/libopi_ffi.a
-rw-rw-r-- 1 erik erik 6547520 9月27日 01:00 .../libopi_ffi.a

$ ar t ... | head -2
__.SYMDEF
opi_ffi-42e7b89bd5c359af.opi_ffi.562ff5d747b105b9-cgu.3.rcgu.o
$ ar t ... | wc -l          → 374

$ od -An -tx1 -N 16 <解出来的 .o>
 cf fa ed fe 0c 00 00 01 00 00 00 00 01 00 00 00
 └─ MH_MAGIC_64 (LE)  └─ CPU_TYPE_ARM64 (0x0100000C)

$ 逐个核对 cabi.rs 的 28 个 no_mangle 导出在 .a 里是否存在
  缺失: 0 / 28        （28 个全部命中，含 _opi_key_event / _opi_select_page）
```

即：**374 个成员、BSD 风格的 `__.SYMDEF` 索引、对象是 arm64 Mach-O、
C 符号带 Mach-O 的下划线前缀、28 个导出一个不少**。这不只是「编译过」，
是真的产出了一个长得像 iOS 静态库的文件。

**另核过一项：`cabi.rs` 的导出、`macos/OpiFFI.h` 的声明、`.a` 里的真实符号
三者各 28 个、两两差集为空**（本轮 Rust 侧仍在改，`opi_select_page` 就是这一轮加的
—— 头文件一度落后于 `cabi.rs`，现已对齐）。

#### ⚠️ 查符号时踩到的坑：Linux 上的 `nm` 读不了 Mach-O

第一次查符号数时得到的是 **0**，看着像「导出一个都没有」，实际是**工具的锅**：

```
$ nm --defined-only libopi_ffi.a | grep -c ' T _opi_'
0
$ nm --defined-only libopi_ffi.a 2>&1 | head -2
nm: __.SYMDEF: file format not recognized
nm: opi_ffi-...rcgu.o: file format not recognized
```

GNU binutils 的 `nm` **不认 Mach-O**，逐个成员报「file format not recognized」
后一个符号都不输出 —— 不加 `2>&1` 就会把这读成「符号缺失」。`go tool nm`
同样报 `unrecognized object file`。本机也没有 `llvm-nm` / `llvm-objdump`。

本轮用的是**自己写的一个 Mach-O 符号表解析脚本**（读 `LC_SYMTAB`，筛
`N_EXT | N_SECT`；临时脚本，未入库）：

```
$ python3 /tmp/macho_syms.py target/aarch64-apple-ios/release/libopi_ffi.a
CPU_TYPE 集合: {'0x100000c'}        ← CPU_TYPE_ARM64 | CPU_ARCH_ABI64
导出 opi_ 符号数: 28
   _opi_backspace  _opi_buffer  _opi_candidates  _opi_candidates_page  ...
```

**别只信 `nm` 的计数**：正确做法是「解析出符号表**再与 `grep -c 'no_mangle' cabi.rs`
对账**」—— 两个数不相等就说明方法有问题，而不是符号有问题。

**Mac 上不需要这么麻烦** —— 那边的 `nm` / `otool -L` 原生认 Mach-O。
记这一条是因为：在本机复现这套验证的人，**很容易被 `nm` 的 `0` 骗过去**
（一个缺失的符号和一个读不了的文件，在 `grep -c` 里长得一模一样）。

#### 复验时间戳（Rust 侧在并行改动，数字要对得上）

Rust 侧由别的分身在同时改。本轮记录的数字全部来自 **01:00** 的重跑
（两个 `cargo check` 于 `01:00:12`–`01:00:13` 各 exit 0；`.a` 于 `01:00` 无缓存重建，
29.93s）。此前 00:51 的一次 `cargo rustc` 曾以 `E0624` 失败，原因是 `cabi.rs` 恰好在
那一分钟被写（`cabi.rs` mtime `00:51:56`、`api/mod.rs` `00:51:50`、
`router.rs` `00:51:48`）—— **是并行编辑撞上了半写的文件，不是代码本身编译不过**：
紧接着用同一命令重跑即 `Finished`（exit 0）。

**导出数这一晚变过三次：25 → 27 → 28。** 新增的是 `opi_candidates_page` /
`opi_page_count`（第 26、27 个）与 `opi_select_page`（第 28 个）。第 26、27 个落地时
`macos/OpiFFI.h` **落后过** —— 那时本目录的 Swift **不敢调**它们，只记录用法；
现在（28 个）头文件已对齐，Swift 也已真的调用。**`ios/OpiFFI.h` 是转发头，
所以每次都是自动跟上**，iOS 侧没有同步动作 —— 但「自动跟上」的前提是那个头也在
同一个仓库里，这也是当初不在这里重抄一份 C 声明的理由。

### 2b. 无缓存复验 —— **通过**

上面的 `cargo check` 会命中 cargo 缓存，不足以证明「当前源码」能过。
所以又用**全新的 target-dir**（`--target-dir /tmp/...`，零缓存）各跑了一遍，
依赖从零编译：

```
$ cargo check -p opi_ffi --target aarch64-apple-ios     --target-dir /tmp/opi_ios_fresh
    Checking jni-sys v0.4.1
    Checking engine-core v1.0.13 ...
    Checking engine-data v1.0.13 ...
    Checking opi_ffi v1.0.13 ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.17s   (exit 0)

$ cargo check -p opi_ffi --target aarch64-apple-ios-sim --target-dir /tmp/opi_ios_fresh
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.84s    (exit 0)
```

**但仍有未验证项**（Mac 上要确认）：
- 这个 `.a` 是 **GNU ar** 打的包，不是 Apple 的 `libtool`。
  索引名是 `__.SYMDEF`（不是 `__.SYMDEF SORTED`）—— 按 BSD 约定两者都合法，
  但**没有用 ld64 验过**。建议 Mac 上第一次就以 Xcode 自己产的为准。
- 只验过 device 目标；模拟器目标只跑了 `cargo check`，没产 `.a`。
- Intel Mac 的模拟器目标是 `x86_64-apple-ios`，本次**没装也没验**。

### 3. `Info.plist` 结构能解析 —— **仅限格式**

```
$ python3 -c "import plistlib; plistlib.load(open('ios/Info.plist','rb'))"
→ 解析 OK
```
这只证明 XML 和 plist 语法没问题。**键名对不对、值对不对，完全没验证** ——
本机没有 `plutil` 也没有 Apple SDK。Mac 上第一件事是 `plutil -lint` + 比对 Xcode 模板。

### 4. Swift 里的键码常量与 `keys.rs` 逐项比对 —— **通过**

`OpiEngine.swift` 的 `OpiKey` 是从 `crates/engine-core/src/keys.rs` 手抄的，
所以值得单独查一遍「抄漏了没有」。用一个临时脚本（解析两侧常量再比对，未入库）：

```
比对 22 项常量：一致 22，不一致 0
Rust 特殊键 12 个，与 ASCII 段重叠: 无
Swift 特殊键 12 个，与 ASCII 段重叠: 无
```

即：**22 个 `KEY_*` / `KEY_STATE_*` 常量与 `SPECIAL_BASE` 两边数值完全一致**，
且「特殊键码全在补充平面、与 ASCII 不相交」这条不变式在 Swift 侧同样成立
（这条不变式是 TSF 轨被真 bug 逼出来的，见 `keys.rs` 模块头）。

**这只证明常量抄对了，不证明 Swift 能编译。**

⚠️ 顺带记一条容易踩的：`keys.rs` 与 `router.rs` 在本轮**分了家** ——
**编码常量在 `keys.rs`，路由逻辑在 `router.rs`**（`KeyRouter` / `KeyAction` /
`ShiftState` / `PAGE_SIZE` 都还在 `router.rs`）。改常量改前者，改行为改后者。

**两者是同一张表，不是两张**：`router.rs:14` 是 `use crate::keys::*;`
（glob 导入），`router.rs` 里 `pub const KEY_` 数量为 0，所以
「以 router.rs 的常量为准」与「以 keys.rs 的常量为准」**指向同一组数值**。
接口冻结时口述的那张表若与此不符，以下面的实际值为准：

| 键 | 实际值（`keys.rs`，`router.rs` 经 glob 用同一份） |
|---|---|
| 空格 `KEY_SPACE` | `0x20` —— **可打印段，不是特殊键**。写成 `0x1_0020` 会落进非 ASCII 分支被直通，后果是**拼音打一半按空格不提交候选** |
| ⇧ `KEY_SHIFT` | `0x1_0083` |
| PageUp / PageDown | `0x1_0080` / `0x1_0081` |
| Delete `KEY_DELETE` | `0x1_0082` |

（`keys.rs` 内有测试 `assert_eq!(KEY_SPACE, 0x20)` 且断言它不在 `SPECIAL_KEYS` 里，
这个坑在上游已被钉住。）**本目录的 `OpiKey` 已按上述实际值写，并脚本比对过 22/22。**

#### 同类陷阱：模式表 `OpiMode` 与 `composer::Mode` 的**声明顺序不同**

`opi_mode()` / `opi_switch_mode()` 的 int 编码**不是** Rust 枚举的声明顺序：

| | 声明顺序 | **C ABI 实际编码**（`crates/opi-ffi/src/api/mod.rs` 的 `mode_to_int`） |
|---|---|---|
| Pinyin | 0 | 0 |
| Traditional | **1** | **4** |
| English | 2 | 1 |
| Number | 3 | 2 |
| Symbol | 4 | 3 |

若照声明顺序编号，繁体就会被编成 1。**这个坑真实发生过**：
`OpiMode` 曾漏掉 `traditional = 4`，于是 `opi_mode()` 返回 4 时
`OpiMode(rawValue:)` 得到 nil、再被 `?? .pinyin` 兜成拼音 ——
**繁体模式被静默显示成拼音**，不崩溃、不打日志，最难查的那种。

两处已按此修好：①`OpiMode` 每个 case **显式写死数值**（不依赖声明顺序推导）；
②`mode()` 的兜底**不静默**，遇到不认识的编码会 `NSLog` 报「两端模式表已漂移」。

⚠️ 另一处**上游注释漂移**（Rust 侧文档笔误，不影响行为）：
`cabi.rs` 里 `opi_switch_mode` 的注释写「0=Pinyin 1=English 2=Number 3=Symbol」，
**漏了 4=Traditional**；实际收 4。以 `mode_from_int` 为准，别照那句注释写。

⚠️ 已知缺口：键盘 UI **没有进入繁体模式的入口**（`toggleLanguage` 只在
拼音 ⇄ 英文之间切）。引擎支持、UI 未接 —— 接法参照 Android 的繁简开关，
不要在 Swift 侧自创判定。

### 5. Swift 代码 —— **零验证**

一行都没编译过。别问「有没有 bug」，问就是「不知道」。
上面第 4 条只是「常量的**数值**抄对了」，与「代码能编译」是两件事。

---

## 构建集成：Rust 静态库怎么来

iOS 需要的是**静态库**（`.a`），而 `crates/opi-ffi/Cargo.toml` 现在写的是：

```toml
[lib]
crate-type = ["cdylib", "lib"]      # ← 没有 staticlib
```

所以有两条路：

**路子 A（推荐，一行改动）**：给 crate-type 加 `staticlib`。

```toml
crate-type = ["cdylib", "lib", "staticlib"]
```
之后 `cargo build -p opi_ffi --release --target aarch64-apple-ios` 直接产出
`libopi_ffi.a`，跟 Xcode 的 script phase 好配合。
⚠️ 这一行**我没有改** —— 它在本任务范围（只新建 `ios/`）之外，且会影响
Android 侧的构建产物。请由负责构建的人确认后再加。

**路子 B（不改 Cargo.toml，本轮实测用的就是这个）**：
```
AR_aarch64_apple_ios=ar cargo rustc -p opi_ffi --target aarch64-apple-ios \
    --release --crate-type staticlib
```
在 Mac 上不需要 `AR_...` 那一段（那里有真的 `ar`）。

### 三个 slice 与 XCFramework

键盘扩展要能在**真机**和**模拟器**上跑，需要两个 slice：

| 用途 | target |
|------|--------|
| 真机（arm64） | `aarch64-apple-ios` |
| Apple Silicon 模拟器 | `aarch64-apple-ios-sim` |
| Intel 模拟器 | `x86_64-apple-ios` |

然后把它们合成一个 XCFramework（`xcodebuild -create-xcframework`，只能在 macOS 上跑）：
```
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libopi_ffi.a \
  -library target/aarch64-apple-ios-sim/release/libopi_ffi.a \
  -output OpiFFI.xcframework
```
⚠️ 上面这条命令是**凭记忆写的**，本机无法执行。Mac 上以 `xcodebuild -help` 为准。

### 词库文件

宿主 App / 扩展的 bundle 里要放词库，和 Android 放 `assets/` 同一思路：

| 文件 | 来源 | 必需? |
|------|------|-------|
| `luna.opid`（约 1.7 MB） | `data/generated/luna.opid` | 是 |
| `trad.opid`（约 2.6 MB） | `data/generated/trad.opid` | 否（失败则繁体模式回退简体库） |
| `fallback.opid` | `data/generated/fallback.opid` | 不用放 —— 内置回退词库已编译进二进制 |

`KeyboardViewController.loadDictionaries()` 用 `Bundle.main.path(forResource:ofType:)` 找；
**扩展的 `Bundle.main` 是扩展自己**，不是宿主 App。找不到时退化为内置回退词库并提示。

⚠️ 目录里现有的 `android/rust_builder/ios/`（cargokit 残留：`Classes/dummy_file.c`
+ `rust_lib_app.podspec`）是 Flutter/cargokit 的产物，指向 `crates/opi-ffi`。
本目录**没有**复用它 —— 那条路要把 cargokit 的 Dart 构建工具也带上，
而 iOS 侧现在只需要一个 `.a`。要不要走 cargokit 由 Mac 上的人决定。

---

## Apple 平台限制清单

> **整节的每一行都是「我凭记忆写的，需核对」。** 本机查不了 Apple 文档，
> 也没有 SDK。请当作**检查清单**用，不要当作事实依据。
> 括号里标了我不确定的具体点。

### 部署与启用

1. **用户必须手动启用**：设置 → 通用 → 键盘 → 键盘 → 添加新键盘 → 第三方键盘。
   扩展不能自己把自己启用（无法引导、无法弹窗）。
   （不确定：宿主 App 是否必须先被打开过一次才会出现在列表里 —— 我记的是「是」。）
2. **必须有宿主 App**：键盘扩展不能独立安装，必须内嵌在一个 App 的
   `PlugIns/` 里。（这条我很确定。）
3. **第三方键盘在密码框里默认不可用**，系统会强制用内置键盘。
   （不确定：`IsASCIICapable` 是否会影响这个行为 —— 我记的是「不影响，
   安全输入框一律用系统键盘」。）

### 完全访问（RequestsOpenAccess）与本项目「离线优先」的关系

4. `RequestsOpenAccess = false`（**本草案选的就是这个**）时：
   - 拿不到网络访问 —— OPI 不需要，无影响。
   - 拿不到与宿主 App 共享的 App Group 容器 —— 意味着**用户词无法与宿主 App 同步**。
   - 用户在设置里不会看到「完全访问」的隐私警告。
5. `= true` 时才有网络 + 共享容器，代价是用户会看到一个明确的隐私警告页。
   **这是产品决策**：若将来要做「App 内词库管理与键盘同步」，必须改成 true。
6. 无完全访问时可能受限的还有：剪贴板读取、`UIDevice.playInputClick()`
   按键音、`openURL`。（这几条我**都不确定**，需核对；「离线优先 + 无完全访问」
   是可行组合，但上面这些边角要逐条试。）

### 资源与运行时

7. **扩展有内存上限，超了会被系统杀**（表现为键盘突然消失/切回系统键盘）。
   具体数值我记不准（几十 MB 量级，随设备/系统版本变化），**必须实测**。
   - 有利因素：词库走 `mmap`（`engine-data` 依赖 `memmap2`），
     RSS 不会等于文件大小；`luna.opid` 1.7 MB / `trad.opid` 2.6 MB 本身不大。
   - 危险因素：`opi_export_user_words` / `opi_import_user_words` 是全量 JSON
     **字符串**，用户词表大时会在内存里留两份。做导入导出时要注意。
8. **键盘没有听写（dictation）**。系统键盘的麦克风按钮，第三方键盘拿不到。
9. **键盘不能盖住宿主界面**：只能提供 `inputView`（+ `inputAccessoryView`），
   不能弹自己的全屏 UI。（不确定：能否用 `present` 弹一个 `UIAlertController` ——
   我记的是「基本不行」。）
10. **键盘高度由扩展自己给**。本草案在 `viewWillAppear` 里写死 264pt
    （`priority = .defaultHigh`）。真机上要按实际内容算，且要处理旋转。
11. **`UIApplication.shared` 不可用**（扩展里拿不到），别写。
12. **首次加载慢**：系统会在需要时才加载扩展进程，冷启动有可感延迟。
    别在 `viewDidLoad` 里做重活（本草案只装词库 + 建 UI）。
    （不确定：系统预加载策略，我记的是「会缓存一段时间，闲置后回收」。）
13. **地球键要自己提供**：自定义键盘不会自动获得系统的地球键。
    本草案在功能行放了 🌐 → `advanceToNextInputMode()`。
    （不确定：`needsInputModeSwitchKey` 为 false 时是否该隐藏 —— 我按「始终显示」写。）

### 无障碍

14. **每个键都要有 `accessibilityLabel`**（尤其 ⇧ / ⌫ / 回车 / 🌐 这类符号键），
    否则 VoiceOver 读不出来。**本草案没做** —— 这是已知缺口，不是疏漏。
    （不确定：iOS 会不会从 `UIButton.title` 自动推断出可读标签 ——
    即使会，符号键也需要显式标签。）

---

## 我最不确定的 API 用法

**这一节是给接手人最省时间的东西。** 因为无法编译，下面这些「我凭记忆写的」
地方在 Mac 上很可能要改。按不确定程度从高到低排。

### 1. `UIKey` / `UIKeyboardHIDUsage`（最不确定，改起来最费时）

`KeyboardViewController.swift` 的硬件键盘一节全靠记忆：

- `UIPress.key` 属性名、`UIKey.keyCode` 类型为 `UIKeyboardHIDUsage`
- 枚举成员名：`.keyboardBackspace` `.keyboardDeleteForward` `.keyboardReturnOrEnter`
  `.keyboardTab` `.keyboardEscape` `.keyboardPageUp` `.keyboardPageDown`
  `.keyboardLeftShift` `.keyboardRightShift` `.keyboardUpArrow` `.keyboardDownArrow`
  `.keyboardLeftArrow` `.keyboardRightArrow` `.keyboardSpacebar`
- `UIKey.characters` / `UIKey.modifierFlags` 属性名
- `override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?)`
  的签名（`UIPressesEvent?` 的 Optional 性）
- **HID usage 的语义**：USB HID Usage Tables 里 0x28=Return、0x29=Esc、0x2A=Backspace、
  0x2B=Tab、0x2C=Space、0x4B=PageUp、0x4C=DeleteForward、0x4E=PageDown、
  0x4F..0x52=方向键、0xE1/0xE5=左右 Shift —— 这套**数值**我比较有把握
  （是通用标准，不是 Apple 私有的），但**枚举成员名**把握不大。

**建议**：这一节先整个注释掉，让键盘在纯软键盘下跑通，再单独接硬件键盘。

### 2. `textWillChange` / `textDidChange` 的用法

我按「**系统回调，重写即可**」写（`textDidChange` 里调 `refresh()`）。
不确定的是：
- 两个方法的参数类型是 `UITextInput?` 还是别的
- **插入文本前后要不要主动调一次**通知宿主 —— 我**没有**主动调。
  若 Mac 上发现宿主 App 的文本状态不同步（撤销栈错乱、字数统计不动），
  这里是第一嫌疑点。

### 3. C 类型 → Swift 类型的导入

| C 声明 | Swift 里应该是什么 | 我的把握 |
|--------|-------------------|---------|
| `size_t` 参数（`opi_select` / `opi_candidates`） | 导入为 `Int`（我按这个写） | 中 —— 也可能是 `UInt`，改起来是编译错误级别的显式报错，好改 |
| `bool` 返回值（`opi_load` / `opi_load_trad` / `opi_learner_enabled`） | `Bool` | 高 |
| `OpiString.ptr`（`const uint16_t *`） | `UnsafePointer<UInt16>?` | 中 |
| `OpiKeyEventResult.action`（`int32_t`） | `Int32` | 高 |

### 4. `OpiString` 的持有与释放

契约在 `crates/opi-ffi/src/cabi.rs` 里写得很死，我按它实现了
`OpiEngine.takeString(_:)`（值先拷进 String，再 `defer` 释放）。
不确定的只有 Swift 侧的类型细节（上表），**所有权逻辑本身没有猜测成分** ——
这是本目录里我最有把握的一部分。

**容易漏的点**（已写进注释）：`opi_key_event` 返回的 `text` 字段**无论 action 取值都要释放**。
`action != 2` 时它是空句柄，`opi_ffi_free_string` 对空句柄是 no-op，
所以「无条件释放」是安全的、也是要求。

### 5. 约束与布局细节

- `UIScrollView` 的 `contentLayoutGuide` / `frameLayoutGuide` 写法
- 候选栏 `UIStackView` 嵌在 `UIScrollView` 里的约束（横向滚动常见的坑）
- 键宽比例约束：我用
  `b.width == row.width × frac + (-gaps × frac)` 让「各键之和 + 空隙 = 行宽」恰好成立。
  数学是对的，但**没跑过**，上机第一件事是看有没有约束冲突日志。
- `UIButton.contentEdgeInsets` 在 iOS 15 起废弃（换成 `UIButton.Configuration`）——
  能用，但会有废弃警告。
- 键盘背景推荐用 `UIInputView(style: .keyboard)` 承载（能拿到系统键盘外观），
  本草案用的是普通 `UIView` + `systemBackground`。

### 6. `Info.plist`

`NSExtensionAttributes` 的四个键名（`IsASCIICapable` / `PrefersRightToLeft` /
`PrimaryLanguage` / `RequestsOpenAccess`）、`CFBundlePackageType = XPC!`、
`NSExtensionPrincipalClass = $(PRODUCT_MODULE_NAME).KeyboardViewController`
**全部凭记忆**。

`@objc(KeyboardViewController)` 加模块前缀的拼法是这里最容易崩的地方 ——
**启动即崩先查这个**。

### 7. 桥接头

`ios/OpiFFI.h` 用 `#include "../macos/OpiFFI.h"` 跨目录引用。
- 编译期相对路径按**本文件所在目录**解析 —— 只在整个仓库一起 checkout 时成立。
- 不确定 Xcode 对 bridging header 里跨目录 `#include` 是否有额外限制。
- 更干净的做法：把声明头挪到中立的 `include/opi_ffi.h`，macOS / iOS 各自转发。
  这属于仓库结构调整，**本轮没做**（超出「只新建 ios/」的范围）。

---

## 设计约定（改代码前先读）

### 1. Swift 要薄，逻辑在 Rust

键路由的唯一真源是 `crates/engine-core/src/router.rs`（经 `opi_key_event` 暴露）。
**不要**在 Swift 里加「哪个键干什么」的判断 —— 这张表已经被抄了三份
（fcitx5 / TSF / Android），router.rs 的模块头专门解释了为什么不能再抄。

Swift 侧只允许有两类「判断」：
- **键码映射**：平台键码 → `opi_key_event` 的 keyval 编码
  （`OpiKey`，与 `crates/engine-core/src/keys.rs` 的 `KEY_*` / `KEY_STATE_*` 逐条对应）。
  这部分**必须**在胶水层，`opi_key_event` 只认一种编码。
  ⚠️ 注意 `keys.rs` 与 `router.rs` 分家：**编码常量在 `keys.rs`，
  路由逻辑（`KeyRouter` / `KeyAction` / `ShiftState` / `PAGE_SIZE`）在 `router.rs`**。
  改常量改前者，改行为改后者。
- **fallback**：引擎说「这键我不管」（`action = 0`）时客户端做什么
  （`KeyFallback`）。Rust 判「管不管」，Swift 判「不管时怎么办」——
  后者是平台语义（软键盘上回车要自己 `insertText("\n")`，
  macOS 上是交给 TextView）。

### 2. 按下与抬起都要发

`keyEvent` 在 `didPress` 和 `didRelease` 里各调一次（抬起的 states 带
`stateReleased`）。原因写在 router.rs 里：可打印键「抬起按按下的结论回复」，
退格/回车「抬起与按下同判」；只发按下会让引擎的 ⇧ 状态机与键盘状态不完整。
抬起的返回**永远不会**是 commit，所以不会重复上屏。

### 3. 没接管的硬件键必须交回系统

`pressesBegan` 里若引擎返回 `.passThrough`，那个 `UIPress` 必须传给
`super.pressesBegan`，否则宿主 App 收不到方向键 / ⌘ 组合键 ——
用户的键盘就像坏了一样。软键盘则走 `KeyFallback`（没有「系统」可交）。

### 4. Swift 侧不要拦修饰键，直接送进来

`router.rs:239` 的直通掩码是 `KEY_STATE_CTRL | KEY_STATE_ALT | KEY_STATE_META`，
**META 已在里面**。所以 Swift 侧**不需要**自己判断「Command 按下了要放行」——
`modifierStates(_:)` 只负责把 `UIKeyModifierFlags` 翻成位，剩下交给引擎。
Swift 侧若再拦一次，就是又抄了一份分流逻辑（而且很容易漏掉 ⌥ 或 ⌘ 中的一个）。

### 5. 不要静默失败

`opi_load` 返回 false 时 Rust 侧所有出口都会退化成空操作。
`KeyboardViewController` 因此有 `dictionaryLoaded` 标志，失败时候选栏显示
「词库未装载」并 `NSLog`。
（fcitx5 轨的教训：词库损坏时插件静默全失效，用户只看到「键盘没反应」。）

### 6. 宠物几何不在 Swift 里复制

项目宠物「小欧」的几何真源是 `docs/opi-pet.svg`；Android 与 Windows 候选窗共用
`shared/pet/OpiPet.kt` 这一份实现（本项目原则：各端 UI 不共享代码，但宠物是一张图）。
iOS 侧**故意没画**：候选栏空态（`KeyboardLayout.swift` 的 `updateCandidates`，
空数组直接 `return`）现在是一片空白。要接的话**别**新写一份 CoreGraphics 几何
—— 同一张图的第三份拷贝必然漂移。顺序照旧：先让 Swift 过编译器（见「在 Mac 上
第一件要做的事」），再谈接哪一份。

---

## 缺的出口：**已落地**（本节从「建议」更新为「现状」）

草案最初有四处**因为缺出口**而做了次优处理。四个出口**现已由 Rust 侧实现**
（`cabi.rs` 现为 **28** 个 `#[no_mangle]`，本轮已在 arm64 `.a` 里逐个核对过）：

```c
OpiString opi_candidates_page(void);   /* 当前页候选，JSON 文本数组；未装载 → [] */
uint32_t  opi_page(void);              /* 当前候选页（0 起，末页由路由钳制）；未装载 → 0 */
uint32_t  opi_page_count(void);        /* 候选总页数；无候选 → 0，未装载 → 0 */
OpiString opi_select_page(uint32_t k); /* 当前页第 k 个（页内索引，0 起）；越界/未装载 → 空串 */
int32_t   opi_shift_state(void);       /* 0=OFF 1=SINGLE 2=LOCK；未装载 → 0 */
```

五个都在 `catch_unwind` 内。注意 `opi_page` / `opi_page_count` / `opi_select_page` 的参数
与返回**都是 `uint32_t`**（`opi_page` 不是我最初建议的 `i32`），Swift 侧读出来是 `UInt32`
—— 所以**负数要在 Swift 侧先判掉**（见下）。
`opi_candidates_page` 与 `opi_candidates(limit)` 一样返回 `OpiString` 的 JSON 文本数组。

⚠️ **声明头落后过 `cabi.rs` 两次**（`opi_candidates_page`/`opi_page_count` 落地时
`macos/OpiFFI.h` 还停在 25 个声明）。`ios/OpiFFI.h` 是**转发头**，会自动跟上 ——
这正是当初不在这里重抄一份 C 声明的理由。**但转发也意味着：声明头补上之前，
这里调用新函数一律编译不过。**（本轮已对齐：各 28 个、差集为空。）

**怎么用（四条，前三条**本草案已接线**，第四条仍留白）：**

1. ✅ **候选列表读 `opi_candidates_page()`，不自己按 8 切。**
   `refresh()` 现在调 `engine.candidatesPage()`，`OpiEngine` 里那个「取一批、
   自己按 8 切片」的 `candidates(limit:)` **已删除**。
   原写法就是出口文档点名的反模式：**页大小是引擎侧常量**（`router.rs` 的
   `PAGE_SIZE`），UI 再抄一份，引擎改一次就**静默错位**（高亮的页 ≠ 选词所在的页）。
2. ✅ **页码读 `opi_page()`，不本地累加。**
   `didRequestPage` 不再 `page += delta`，翻完页直接 `refresh()` 读引擎。
   **页码钳制在 `router.rs`**：到末页再按 PageDown 引擎会把页码钳在最后一页，
   本地累加正是漂移的来源。「共 N 页」用 `opi_page_count()`（**无候选是 0，不是 1**）。
3. ✅ **选词走 `opi_select_page(k)`（页内索引），不算全局下标。**
   这是最后消灭掉 `PAGE_SIZE` 副本的那一刀 —— 详见下一节。
4. ⬜ **⇧ 高亮读 `opi_shift_state()`** —— **仍故意没接**。
   ⇧ 的真源是 `KeyRouter::ShiftState` 三态（Off/Single/Lock），且引擎会在提交后
   **自动复位 Single**。注意它和 `opi_set_shift` **不是一回事**：后者打的是引擎侧
   shift 位，三态是前端状态、决定英文直传路径的大小写 —— 引擎位看不出来。
   见 `KeyboardLayout.swift` 里那段注释；接线时读出口即可，不必自己镜像状态。

### `PAGE_SIZE` 副本是怎么被清掉的（记一笔，别再抄回来）

**曾经的问题**：`opi_select(index)` 收的是**全局**下标 ——
`api::select` 转发的是 `Engine::select`，而路由内部那个**页内**语义的
`Router::select`（自己 `page * PAGE_SIZE + index` 换算）当时是**私有**的、不在这条链上。
于是前端拿到的只有「当前页」，点第 `i` 个仍得自己算 `page * pageSize + i`
—— **`PAGE_SIZE` 在 UI 侧的第二份拷贝**；引擎改页大小，前端**静默选错候选**。

（当时查过 Android：`EngineController.kt` 的 `selectFromPage(i) = select(page * pageSize + i)`，
且它自己也有 `const val pageSize = 8` —— 说明这是**既有约定**，不是 iOS 单方面抄错。）

**现在**：`Router::select` 已 `pub`，注释写明「**页内换算只此一份**：数字选词、
回车提交与 `opi_select_page` 都走这里」，并新增了页内索引出口：

```c
OpiString opi_select_page(uint32_t k);  /* 当前页第 k 个；越界/未装载 → 空串 */
```

于是本目录**删掉了最后一处常量副本**：`OpiEngine.pageSize` 已不存在，
`KeyboardLayout.updateCandidates` 不再需要 `page` 参数（回调只传页内序号），
`KeyboardViewController` 也不再持有 `page` 状态。

⚠️ **负数是越界语义，不是「从末尾数」** —— 但 `UInt32(-1)` 在 Swift 里是
**运行时陷阱（直接崩）**，不像 C 那样回绕。所以 `selectPage(_:)` 里显式
`guard k >= 0 else { return "" }`；JNI 侧对同一场景也是「负索引按越界处理」。

顺序仍然是：**先在 Mac 上过编译，再接。**

### 已修：`api::select` / `input_space` / `clear` 绕过路由却没重置页码

`router.rs` 写死了契约：「绕过路由改引擎，**改完必须调用
`reset_page_if_buffer_changed`** —— 否则页码停在过期值上……」。

本轮先发现 `api::select` / `input_space` / `clear` 三个出口**没调**（`input_key` /
`backspace` 调了），而这三个都会改 buffer。**我当时判断「构造不出必然出错的时序」，
这个判断偏窄了** —— Rust 侧补上后给的症状是**纯显示层就能复现**的：

> `opi_select` 清空缓冲后 `opi_page()` 停在 1、而 `opi_page_count()` 变成 0
> → UI 显示「**第 2 页 / 共 0 页**」。

我当时的推理是「本草案的候选栏在候选为空时不绘制，所以看不出来」—— 那是**拿自己
草案的实现去替 ABI 契约开脱**，站不住：契约说页码必须对齐，与某个前端碰巧不画无关。
现在三处已补（`api/mod.rs` 的 `select`/`input_space`/`clear` 各自对齐页码），
本层 `select(index:)` 注释里那句「选完词别立刻信任 `page()`」**已随之撤掉**。

---

## 另一处已知的静默失败：`opi_switch_mode` 越界

`opi_switch_mode` 对越界值是**静默不动作**：`cabi.rs` 走 `mode_from_int(mode)`，
返 `None` 就什么都不做，而且**函数返回 `void`，调用方查不到失败**。症状是「切了模式
但界面毫无变化，且无任何日志」。

`OpiEngine.swift` 的 `switchMode(_:)` 收的是 `OpiMode` 枚举、被 `rawValue` 封死在
0..=4，**当前安全**。但**将来若从菜单/配置/持久化读进裸整数**（`Int32` /
`UserDefaults` / JSON），**必须先自校验再传**（macOS 侧已同样注明）。

---

## CI 建议

本仓库已有的 `ci.yml` 里已经有同构的先例（Windows 目标）：
```yaml
- name: cargo check -p tsf_opi (x86_64-pc-windows-msvc)
  run: cargo check -p tsf_opi --target x86_64-pc-windows-msvc
```
理由注释写的是「`cargo check` 不链接，所以 Linux 上无需 msvc 链接器即可跑通」。

**iOS 完全可以照抄这一条**（本轮已在 Linux 上实测通过）：
```yaml
- name: cargo check -p opi_ffi (iOS targets)
  run: |
    rustup target add aarch64-apple-ios aarch64-apple-ios-sim
    cargo check -p opi_ffi --target aarch64-apple-ios
    cargo check -p opi_ffi --target aarch64-apple-ios-sim
```
价值：有人在 Rust 侧改了 C ABI 或加了 `cfg` 分支时，**CI 会立刻发现
「iOS 目标编译不过」**，而不是等到 Mac 上才发现。
Swift 侧则**无法**进这个 CI（需要 macOS runner），只能靠人。
