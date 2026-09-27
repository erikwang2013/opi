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
  确认符号全部能解析 —— **不要照抄任何写死的个数**（本仓到过 19 → 20 → 22 → 28 → 31；
  从没有过 25，别再把它当历史抄进去），全称与当前数量以 `crates/opi-ffi/src/cabi.rs`
  与 `crates/opi-ffi/tests/c_abi_contract.rs` 那道门禁为准。
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
| `OpiEngine.swift` | C ABI 薄桥：`OpiString` 所有权、`opi_key_event` → Swift 枚举、JSON 候选解码、符号库三个出口 |
| `KeyboardKeys.swift` | 「一个键是什么」：`KeySpec` / `KeyFallback` / `KeyboardFunctionKey` / `KeyButton`。不含布局 |
| `KeyboardLayout.swift` | 键盘 UI 骨架的视图装配：候选栏（含翻页）+ 字母/数字层 + 触摸接线 + 外部刷新入口 |
| `SymbolPanel.swift` | 符号层的网格（竖向滚动）。数据来自 `OpiEngine.commonSymbols()`，点击直提 |
| `KeyboardViewController.swift` | `UIInputViewController` 入口：生命周期、词库装载、**软键盘**路由、`textDocumentProxy`，外加外接键盘的两个入口 `pressesBegan/pressesEnded`（逻辑在下一行那个文件） |
| `KeyboardHardware.swift` | **外接键盘那一路**：`UIKey`/`UIPress` → 引擎键码、修饰位，以及**引擎之前**判掉的三个热键（`Ctrl+'` / `Ctrl+\` / `Shift+Space`）。不含触摸与布局 |
| `Info.plist` | 键盘扩展的 `NSExtension` 配置（**键名未核对**） |
| `README.md` | 本文件 |

⚠️ 为什么 `KeyboardLayout.swift` 拆成了三个文件：它到了 **543 行**，撞上本仓
「源码文件 500 行」的规矩。拆法按**概念**而不是按行数切齐：键的定义是编译期常量、
布局是视图树装配、符号面板是运行时数据 —— 本来就是三件事。
（`CLAUDE.md:12` 写的是「Keep files under 500 lines」，**没写适用范围**；
「只管源码、文档不受限」是 team-lead 的裁决 —— 本仓 `docs/superpowers/plans/` 下
三个 plan 文件是 2079 / 1698 / 1315 行。所以别拿这条去拆 README。）
搬移本身做了**声明集合比对**（没搬丢、没搬重、旧名零残留），见「实测记录」5d ——
**但那只证搬移，不证能编译。**

⚠️ **第二次拆分（2026-09-27，`KeyboardViewController.swift` → 加一个 `KeyboardHardware.swift`）：**
它到了 **478 行**。478 **是合规的**（<500），拆的理由不是违规而是**余量**：只剩 22 行，
而本仓被「一次全库格式化就把贴线文件顶破」咬过。
拆法是**软键盘 / 硬件键盘**这一刀（触摸与层 vs HID 键码与修饰位），行数 **478 → 318 + 214**。
两条**必须记住的副作用**：

1. **`pressesBegan`/`pressesEnded` 两个 `override` 故意留在主文件**，没跟着搬。
   覆写 ObjC 方法能否写在**跨文件**的扩展里，本机没有编译器可问 ——
   这是**未验证的取舍**（不是「已验证不能写」），故取最保守的写法。
2. 搬过去的扩展要用 `engine` / `layout` / `insert` / `refresh` / `modeLabel`，
   Swift 的 `private` **只跨「同文件的扩展」可见** ⇒ 这 5 个成员被放开成 internal。
   **不要为了整洁改回 `private`** —— 那会让 `KeyboardHardware.swift` 编译不过。
   该风险有一条能红的检查看着，见「实测记录」5e。

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

> ⚠️ **上面这段已被 2026-09-27 推翻，原样留着当记录。** `cabi.rs` 现在是 **31** 个导出
> （新增 `opi_toggle_fullwidth` / `opi_fullwidth_state` / `opi_toggle_symbol`），
> 而 `macos/OpiFFI.h` **仍是 28 个声明**（指纹 `e3b37085…`、mtime `23:26:15` 未动）。
> 所以三者**不再相等**，差集就是那三个名字 —— 这是「声明头落后于 `cabi.rs`」**第三次**
> 出现。头文件归 `ffi-contract`，本节不动它；详见下面「全角 / 符号开关的出口」一节。
>
> ⚠️ **同日更晚再订正（2026-09-27）**：`ffi-contract` 已把 `macos/OpiFFI.h` 补齐，
> `cargo test -p opi_ffi --test c_abi_contract` = **14 passed / 0 failed**。
> ⇒ 上面那句「`macos/OpiFFI.h` **仍是 28 个声明**」**当时成立、现已不成立**。
> **「三次滞后」的历史仍然有效**（它记的是发生过什么），但**具体数字不要再抄** ——
> 判据是那道门禁。

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

> ⚠️ 上面几段输出里的 `v1.0.13` 是**跑那些命令时**的工作区版本，不是当前版本
> （仓库已到 v1.0.15）。**记录照原样保留** —— 改成本轮的版本号就是把一份实测
> 记录改成没跑过的样子。要今天的新数字就重跑一遍。

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
本机没有 Apple SDK。Mac 上第一件事是 `plutil -lint` + 比对 Xcode 模板。

⚠️ **订正一条早先的说法**：本机**有** `/usr/bin/plutil`，但它是 **GNUstep** 的
（`dpkg -S` → `gnustep-base-runtime`），**不是 Apple 的**。它不但不能替代
`plutil -lint`，还会给出**假红**：

```
$ plutil -lint ios/Info.plist
Loading 'ios/Info.plist' - non-NSData data argument passed to method
（exit=1）
$ plutil                       # 不带参数
plutil: no files given.
```

「exit=1」在这里**不代表 plist 有问题** —— 是 GNUstep 的 plutil 读不了这个文件。
别把这条红当成 Info.plist 的缺陷（红色的另一种错法：红也可能红错）。

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

#### 繁体模式的入口 —— **已写，未编译**

（**订正**：本节早先写的是「键盘 UI **没有**进入繁体模式的入口」。现在有了。）

`KeyboardViewController.cycleMode()` 做三态循环 **中 → 繁 → 英 → 中**，
**逐字对齐 Android `ImeScreen.kt` 的 `toggleMode()`**（含前两跳先 `clear()`：

```kotlin
EngineMode.PINYIN      -> { controller.clear(); controller.switchMode(EngineMode.TRADITIONAL) }
EngineMode.TRADITIONAL -> { controller.clear(); controller.switchMode(EngineMode.ENGLISH) }
else                   -> controller.switchMode(EngineMode.PINYIN)
```

前两跳清缓冲是**必须的**：拼音打一半切走，残留缓冲会被下一个空格/回车意外提交。
模式键的键面字跟着模式走（`modeLabel(of:)` → 中/繁/英，对齐 Android `modeLabelOf`）。

引擎侧**没有**为此加什么东西：`opi_switch_mode(4)` 本来就走得通，缺的只是 UI 入口。

### 5. Swift 代码 —— **仍然零编译，只多了三项文本级检查**

**一行都没编译过。别问「有没有 bug」，问就是「不知道」。**
上面第 4 条只是「常量的**数值**抄对了」，与「代码能编译」是两件事。

本机**确实没有**编译器（`which swiftc xcodebuild` → exit 1，两个都找不到）。
所以本节列的都是**文本级**检查，看的是「名字对不对得上」，不是「能不能编」。

#### 5a. `cabi.rs` ↔ `macos/OpiFFI.h` 签名一致性 —— 当时 28/28，其后一度 31 vs 28，**现已补回一致**

比的是 **名称 + 元数 + 参数类型序列 + 返回类型**（不是只比名字）：

```
$ python3 /tmp/abi_parity.py
Rust 导出 = 28   头文件声明 = 28
名字差集 Rust−头: []
名字差集 头−Rust: []
签名不一致: 0
（exit=0）
```

被验的头文件指纹：`md5sum macos/OpiFFI.h` = `e3b3708547b0192b88281aec23fa4399`，
mtime `2026-09-27 23:26:15`。**该文件归另一个分身所有、正在被它改动** ——
上表的时效**只到上面那个 mtime**。它一改，本节就过期，请重跑。

> ⚠️ **已经过期了，就在同一天晚些时候**：`cabi.rs` 新增了三个导出，而这**三条原始输出
> 一个字都没改**（原文照留）。现在跑同一脚本得到的是 3 个名字的差集、`exit=1`
> —— 完整输出与**第二条独立证据**（`ffi-contract` 自己的门禁 `c_abi_contract.rs` 也是红的、
> 报同样 3 个名字）见下面「全角 / 符号开关的出口」一节。**别只读到本节就打住。**

⚠️ **这个检查器是我自己写的，本次任务里它先后报出 16 个、5 个、6 个、19 个
「不一致」，全部是它自己的 bug**（参数名没剥、`bool`/`_Bool` 映射不对称、
`const uint16_t *path` 里指针与名字粘连、`[a-z0-9_]` 与 UTF-8 相邻时的怪异行为）。
**「我的脚本说 0」比「编译器说 0」弱得多** —— 这条要连着上一段的哈希一起读。

#### 5b. Swift 侧没有调用未声明的出口 —— **通过**（本轮重验）

对 `ios/*.swift` + `macos/*.swift` 里出现的全部 `opi_*` 标识符逐个查表：

- **非注释行**里出现、却不在头文件声明的：**0 个**。
- 查不到的标识符**全部只出现在注释散文里**：
  `opi_fcitx5`（是 C++ **文件名** `crates/fcitx5-opi/cpp/opi_fcitx5.cpp`）。

**这条检查的经历值得记一笔，因为它变过一次含义**：

| 时间 | 非注释调用点 | 这条检查在证什么 |
|---|---|---|
| 头文件补齐**之前** | **0** | 「纯注释、零调用」—— 三个声明缺口今天不会造成编译错误；**谁先写调用点谁先把树弄红** |
| 头文件补齐**之后**（现在） | **3**（全在 `OpiEngine.swift`，就是那三个薄包装） | 调用点**有且仅有**这三个包装 → 路由层没有散落的直接 C 调用 |

```
$ python3 - <<'PY'   # 去 // 与 /* */ 后匹配 \bopi_toggle_symbol\s*\( 等
OpiEngine.swift:363  opi_toggle_fullwidth(
OpiEngine.swift:375  opi_fullwidth_state(
OpiEngine.swift:386  opi_toggle_symbol(
```

⚠️ **同一条命令，前后含义相反**：补齐前「0」是**好事**（不能调），补齐后「0」会是**坏事**
（说明包装白写了）。所以这条检查**不能只看数字，要看它当时该是什么** ——
和门禁里「一条红一条绿」那次是同一类陷阱（绿的那条只是覆盖范围不同）。

#### 5c. 括号/引号配平 —— **通过，但这条几乎不算证据**

用一个剥掉注释与字符串的临时脚本数 `()` `{}` `[]` 的净值，8 个 Swift 文件全为 `+0`。
**它只能发现「少了一个大括号」，发现不了任何语义错误**（少一个 `import`、
写错枚举成员名、类型不匹配，它一律看不见）。列在这里只是为了说明
「连这种检查都只有这一种」，**不要**把它当成绿灯。

#### 5d. `KeyboardLayout.swift` 拆分的**定义集合 ↔ 引用集合比对** —— **通过**（本轮补的）

5c 那条弱在「只数括号」。`KeyboardKeys.swift` 是从 `KeyboardLayout.swift`
里**机械搬出去的**（124 行），搬移最典型的失效是**搬丢一条**或**搬重一条** ——
两者都不一定破坏括号配平，所以 5c 看不见。补一条集合比对：

```
$ git show HEAD:ios/KeyboardLayout.swift > /tmp/kl_before.swift     # 拆分前 416 行
$ python3 /tmp/swift_decls.py                                       # 抽 类型/方法/枚举 case 名
类型/方法/枚举 case 条目： HEAD 单文件 = 32   拆分后 = 42
丢失 (HEAD计数, 现计数): {'case toggleLayer': (1,0), 'case toggleLanguage': (1,0),
                          'func setLetterLayer': (1,0)}
重复（拆分后同名 >1）  : {'func keyboardLayout': 5}
HEAD 里就重复的        : {'func keyboardLayout': 5}
```

三条「丢失」**全部是同一轮的改名**，不是搬丢（逐个核过）：

| HEAD | 现在 |
|---|---|
| `case toggleLayer` | `case setLayer(_ new: Layer)` + 新增 `enum Layer`（`.letters`/`.numbers`/`.symbols`） |
| `case toggleLanguage` | `case cycleMode` |
| `func setLetterLayer(_ letter: Bool)` | `func setLayer(_ new: Layer)` |

```
$ grep -rn "toggleLayer\|toggleLanguage\|setLetterLayer" ios/ macos/     # 旧名残留
残留计数 = 0
```

`func keyboardLayout` 出现 5 次**不是拆分引入的**：HEAD 里同样是 5 次
（不同层参数的重载），脚本把这一列一并打出来就是为了让这个区分**看得见**。

**引用侧**（上面比的是「定义」，这一半比「谁在引用」）。逐枚举抽 case 名，
再在剥掉注释与字符串的全树里数 `.name`：

```
$ python3 /tmp/swift_enumrefs.py
声明 case 名 = 15   全树 .xxx 引用去重 = 221 处名字
① 声明了但**全树零引用**（改名漏删 / 路由断掉）: 无
② 被引用但无枚举声明的名字 = 206 个（绝大多数是 UIKit/Foundation 成员）
```

② 那 206 个**不是发现，是噪声**（脚本只认 enum，所以我们的 `setLayer`/
`onPick`/`modeLabel` 这类 struct/class 成员也会落进去）。**但它恰好能证明脚本没瞎**：

```
$ .setLayer        = 3 次      ← 新名字：被引用了，且出现在 ② 的名单里（脚本对自家成员可见）
$ .setLetterLayer  = 0 次      ← 旧名字：零引用
$ .toggleLayer     = 0 次
$ .toggleLanguage  = 0 次
```

**「脚本看得见我们的成员」+「旧名为 0」两条一起读，才排得掉「脚本根本没看见」这个解释**
—— 同 `c_abi_contract.rs` 里那条 `parser_is_not_blind` 的思路。

**顺带补一条真能咬人的**：`KeyboardFunctionKey` 现在是 **6 个 case**
（`showLetters`/`showNumbers`/`showSymbols`/`cycleMode`/`nextInputMode`/`commitText`），
而 `KeyboardViewController.swift:205` 那个 `switch` 有 **6 个 arm、没有 `default`**
—— 少一个 arm 就是编译错误（Swift 的枚举穷尽性）。这条**是**靠人眼数的，
但它是有限且确定的：**加 case 时必须同时加 arm**。

##### 证据强度：能抓什么、抓不到什么

| | |
|---|---|
| ✅ **能抓** | 搬移**搬丢**一条定义；**搬重**一条（同名 >1）；改名后**旧名残留**在调用点；声明了却**全树无人引用**的 case（路由断掉/改名漏删）；加 case 没加 switch arm（靠人眼数，6 vs 6） |
| ❌ **抓不到** | **类型是否匹配**（`setLayer(.letters)` 传错枚举、`Int` 当 `Int32`）；**语义是否走样**（搬过去的方法体改了逻辑）；`UIKit` 用法对不对；`import` 少没少；**任何**只有编译器能判的东西 |
| ❌ **完全没碰** | 运行时行为、布局与约束、内存与 `OpiString` 释放次数 |

**所以这三条合起来是一个"搬移完整性"证明，不是"能编译"证明。**
它值钱的地方在于：`KeyboardKeys.swift` 是**本轮唯一一段我改了却没给它任何检查的代码**，
现在它有了 —— 而「搬移看起来不需要检查」正是它危险的原因（它会搬丢，且不弄坏括号配平）。

#### 5e. `KeyboardHardware.swift` 拆分的同类比对 + **一条能红的跨文件 `private` 检查**

第二次拆分（478 → 318 + 214）跑了同一套，另补一条 5d **没有**的检查 ——
因为这次搬移有一个 5d 没有的失效模式：**Swift 的 `private` 只跨「同文件的扩展」可见**，
搬到别的文件后，主文件里那些 `private` 成员会**看不见**。见②。

**① 声明集合 ↔ 引用集合（`swift_decls.py`，本轮改了判定口径）**

```
HEAD 单文件声明条目 = 27 (21 个名字)
拆分后多文件条目   = 35 (29 个名字)

丢失（HEAD 有、拆分后不够）: 无
跨文件重复（= 搬成了复制）  : 无
新增（拆分后才有）          : ['case toggleEnglish', 'case toggleFullwidth', 'case toggleSymbol',
                             'enum OpiHotkey', 'func cycleMode', 'func hotkey',
                             'func modeLabel', 'func performHotkey']
exit=0
```

「新增」那 8 条**不是这次搬移引入的**，是**本会话早先**加的热键代码（`git HEAD` 是本次会话之前的提交）。

⚠️ **口径改了，理由值得记**：原来的判定是「同一名字出现 >1 次就报重复」，
于是干净跑也永远红 —— `func keyboardLayout` 出现 5 次是**协议重载**（签名各不相同），
`var states` 出现 2 次是**两个局部变量**。**一个永远红的检查等于没有检查**（人人学会无视它）。
改判为**跨文件同名**才抓到真信号：「搬移」的典型事故是**搬成了复制**（两个文件各留一份 ⇒ 重复定义）。
配套两条：`extension X` 天然会出现在多个文件，排除；`var|let` 只认缩进 ≤4 的类体成员，避免再收局部变量。
**变异证明它仍会红**（把 `routeHardware` 复制一份留在主文件）：

```
跨文件重复（= 搬成了复制）  : {'func routeHardware': ['/tmp/mut_kvc.swift', 'ios/KeyboardHardware.swift']}
exit=1
```

**② 跨文件 `private` 泄漏检查（本轮新补 —— 这次拆分最要命的一条）**

搬过去的扩展要用 `engine` / `layout` / `insert` / `refresh` / `modeLabel`。
若我**漏放宽任何一条**，它仍是 `private`、跨文件不可见 ⇒ **编译不过**。
这条检查把「主文件里仍然 `private` 的名字」与「新文件里出现过的名字」求交：

```
$ python3 /tmp/leak_check.py ios/KeyboardViewController.swift ios/KeyboardHardware.swift
  主文件 private: ['applyFallback', 'cycleMode', 'dictionaryLoaded',
                   'heightConstraint', 'loadDictionaries', 'route']
  ❌ 被新文件引用（会编译不过）: 无
  exit=0
```

**变异证明**（把 `refresh` 改回 `private`，模拟「漏放宽一个成员」）：

```
  主文件 private: [..., 'loadDictionaries', 'refresh', 'route']
  ❌ 被新文件引用（会编译不过）: ['refresh']
  exit=1
```

**③ `OpiHotkey` 的定义集 ↔ 引用集（两个方向，搬移后重跑）**

枚举有 3 个 case，两个文件都是「生产 1 次、消费 1 次」，反查无未定义名：

```
toggleEnglish    return处=1  switch分支=1
toggleFullwidth  return处=1  switch分支=1
toggleSymbol     return处=1  switch分支=1
反向（.toggleX 未定义）: （空）
```

**变异证明**（把 macos 的 `case .toggleSymbol:` 改名，行数不变）：正向报「switch 分支 =0」、
反向报「`.toggleSymboX` 未定义」—— **两个方向都会红**：漏分支 = 生产了没人消费，写错名 = 消费了没人生产。

**④ 括号配平（同 5c，弱）**：`KeyboardHardware.swift` 25/25 · 29/29；
`KeyboardViewController.swift` 38/38 · 89/89；`macos/InputController.swift` 45/45 · 75/75。

##### 证据强度：能抓什么、抓不到什么

| | |
|---|---|
| ✅ **能抓**（且每条都有变异证明） | 搬移**搬丢**；搬成**复制**（跨文件同名）；**漏放宽 `private`**（跨文件引用不可见成员）；热键枚举**漏 switch 分支**或**名字打错** |
| ❌ **抓不到** | **类型**对不对（`UInt32` 写成 `UInt`、`Set<UIPress>` 参数写错）；**访问级别放过头**（我把 `refresh` 放成 internal，脚本不会说「其实可以更窄」）；`import UIKit` 少没少；**任何**只有编译器能判的东西 |
| ❌ **完全没碰** | 运行时行为、热键在真键盘上是否命中、autorepeat 的真实表现 |

⚠️ ②③ 的价值**恰恰在于它们是缺陷驱动、且能红的** —— 5c 那种「形状检查」永远是绿的，
而这三条都是「不这么写就一定红」。但它们**全部是文本级**：**没有一条碰得到 Swift 的语义。**

---

#### 本机**做不到**的（Mac 上必须逐条做）

| 做不了的事 | 为什么 |
|---|---|
| `swiftc` 编译任何一个 `.swift` | 本机没装 Swift 工具链 |
| `xcodebuild` / 建 target | 没有 Xcode |
| `import UIKit` / `import InputMethodKit` | 没有 Apple SDK，import 就断 |
| 链接 `libopi_ffi.a`（ld64） | 没有 Apple 链接器；.a 是 GNU `ar` 打的 |
| `plutil -lint Info.plist` | 本机只有 GNUstep 的 plutil（且会给假红，见第 3 条） |
| 真机 / 模拟器上装这个键盘 | 同上，全链路都没有 |
| 无障碍（VoiceOver）实际读数 | 需要真机 |
| 符号面板在真实键盘高度下是否放得下 | 需要真机；行高 40pt、键盘高 264 都还是拍的值 |

**所以：本目录里的 Swift 代码，从「能不能编译」到「行为对不对」，
没有一项在本机被验证过。** 它们是**更精确的起点 + 契约**，不是「做好的端」。

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
- 枚举成员名 —— ✅ **已按 Apple 公开文档逐个核对（本轮），只错了一个，已改**：
  `UIKeyboardHIDUsage` **没有** `.keyboardBackspace`，真名是
  **`.keyboardDeleteOrBackspace`**（HID 0x2A）。其余全部正确：
  `.keyboardDeleteForward` `.keyboardReturnOrEnter` `.keyboardTab` `.keyboardEscape`
  `.keyboardPageUp` `.keyboardPageDown` `.keyboardLeftShift` `.keyboardRightShift`
  `.keyboardUpArrow` `.keyboardDownArrow` `.keyboardLeftArrow` `.keyboardRightArrow`
  `.keyboardSpacebar`。
  ⚠️ 这一条**只是文档核对，不是编译验证** —— 仍然没有编译器看过这个文件。
  顺带：成员名写错属**编译错误**（不是静默失效），所以这类错在 Mac 上第一轮编译
  就会全部暴露，改起来不费时。
- `UIKey.characters` / `UIKey.modifierFlags` 属性名（**仍未核对**）
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

⚠️ **声明头落后过 `cabi.rs`**（**落后过几次本机核不出来**：`git log -- macos/OpiFFI.h`
只有**一个**提交态，中间那些滞后全发生在未提交的工作区里 —— 见下面那条订正）。
`ios/OpiFFI.h` 是**转发头**，
会自动跟上 —— 这正是当初不在这里重抄一份 C 声明的理由。**但转发也意味着：声明头补上之前，
这里调用新函数一律编译不过。**

> **订正（2026-09-28）**：本段原先写着「落后过两次（`opi_candidates_page`/`opi_page_count`
> 落地时 `macos/OpiFFI.h` **还停在 25 个声明**）… **现在第三次，还没补**：`cabi.rs` 到 31、
> 声明头仍 28」。**两句都作废，而且是两种不同的错**：
> 1. **「25」是错的，且错在「哪个文件」** —— `git log -- macos/OpiFFI.h` 显示它**只有一个
>    提交态**（`2dc1fa2`，**28 条**），从未到过 25。**25 是 `cabi.rs` 当时的导出数**
>    （`opi_candidates_page`/`opi_page_count` 是第 26、27 个），我把 **Rust 侧的计数
>    安到了头文件头上**。⚠️ 本仓已经栽过同一个坑（「18 个方法」实为 21，18 是**另一份文件**的数）。
> 2. **「第三次，还没补」已不成立** —— `ffi-contract` 已补齐，现测 `cabi.rs` 31 ↔
>    `OpiFFI.h` 31、名字差集双向为空、签名不一致 0（`abi_parity.py` exit 0）。
>
> ⇒ **判据：本节的数字一律以那道门禁与 `git log -- <文件>` 为准，不要抄这里的任何计数。**

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
4. ⬜ **⇧ 高亮读 `opi_shift_state()`** —— **半步已做，UI 仍未接**。
   两步里的 ① **已写**：`OpiEngine.shiftState() -> OpiShiftState`
   （`.off` / `.single` / `.lock`，与 C 的 0/1/2 一一对应，未装载 → `.off`）。
   ② **没做**：`KeyboardLayout` 里**仍然没有** ⇧ 高亮（那段注释已同步订正）。
   所以现状是「出口有了、包装有了、UI 没读」—— 也就仍然**没有会漂移的镜像**。
   ⇧ 的真源是 `KeyRouter::ShiftState` 三态（Off/Single/Lock），且引擎会在提交后
   **自动复位 Single**。注意它和 `opi_set_shift` **不是一回事**：后者打的是引擎侧
   shift 位，三态是前端状态、决定英文直传路径的大小写 —— 引擎位看不出来。
   接线时读 `shiftState()` 即可，**别**拿 `KeyButton.longPressFired` 推状态
   （那只是防「长按后又补一次 tap」的去抖标志，不是状态机）。

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

## 本轮补的功能面：符号面板（**已写，未编译**）

### 加了什么

| 层 | 新增 | Android 侧的对应物（证据） |
|---|---|---|
| 桥 | `OpiEngine.symbolBlocks()` / `symbolsInBlock(id:)` / `searchSymbols(keyword:)` / `commonSymbols()` | `SymbolCatalog`（`android/.../keyboard/SymbolCatalog.kt`） |
| UI | `SymbolPanel.swift`（竖向滚动网格）+ 键盘第三层 `Layer.symbols` | `SymbolPanel.kt`（`@Composable fun SymbolPanel`）的网格 |
| 直提 | `KeyboardFunctionKey.commitText(String)` | `router::commitText`（Android 符号键绕过引擎） |
| 模式 | `cycleMode()`：中→繁→英→中 | `ImeScreen.kt` 的 `toggleMode()` |

### 三个必须记住的契约

1. **`opi_symbol_blocks()` 只回 common 块。** `Engine::symbol_blocks()` 走的是
   `symbols.common_blocks()`，非 common 块**根本不在这个列表里**——
   所以 JSON 里的 `common` 字段在该出口上**恒为 true**，别拿它当「全部块」用。
   当前数据表 `data/raw/symbol_blocks.tsv` 只有 id=1（CJK 符号）是 common，
   于是「常用」= **60 个**符号；`docs`/Android 若给出别的数字，那是另一套口径。

   ⚠️ **由此产生的实际后果，本目录如实记下（2026-09-28 复核）**：iOS 的符号面板
   **只有这一个块的条目** —— `KeyboardViewController.swift` 调的是
   `layout.setSymbols(engine.commonSymbols())`，而 `commonSymbols()` 只遍历
   `symbolBlocks()`（= common 块）。也就是说**非 common 块里的 emoji 等条目在 iOS 上
   够不到**，而这个缺口在 UI 上**没有症状**（面板照样有内容，只是少）。
   `searchSymbols(keyword:)`（包装 `opi_search_symbols`）**存在但零调用者** ——
   判据：`grep -rn 'searchSymbols' ios/ --include=*.swift` 只命中定义那**一行**
   （`OpiEngine.swift` 的 `func searchSymbols`）。⚠️ 别省掉 `--include`：本 README 里
   这几行也含该词，不带过滤会把「文档提到」当成「代码调用」。
   要接的话 Android 的 `SymbolCatalog` 是现成参考：它用 `opi_search_symbols("")`
   拿全部条目（契约 ③）。**别把「包装已写」读成「面板能拿到全部符号」。**
2. **块 id 的类型在两端不一样。** Rust 侧是 `BlockId(pub u16)`（0..=65535），
   C ABI 的 `opi_symbols_in_block(int16_t)` 收**有符号** 16 位 —— 只能对齐到
   0..=32767。所以 Swift 侧按 `Int` 解码、**显式收窄并 NSLog 越界**，
   **不要**直接把 JSON 解成 `Int16`：那会在 id > 32767 时抛异常，
   而本层对解析失败的处理是「返回空数组」—— 后果是**符号面板整块空掉、没有任何日志**。
3. **`opi_search_symbols("")` 返回全部条目**，不是「什么都没搜到」。
   Android 的 `SymbolCatalog.all` 正是靠这个语义拿「全部」。

### 两条纪律

- **符号键直提，不过引擎**：引擎的标点表会把 `,` 改写成中文标点/全角
  （那是**文本**模式该做的事），而用户在符号面板上点的 `,` 就是要 `,`。
- **空结果不缓存**：`commonSymbols()` 每次开面板现取。空结果是「没拿到」
  而不是「真的没有」，缓存它会让面板从此永远空白 ——
  Android `SymbolCatalog` **真的踩过这个坑**（见 commit `00cfea4`
  「「常用」符号不是常用标点 + 空结果被永久缓存」）。本草案在 `SymbolPanel`
  里把空列表画成一行可见的「（符号库未就绪）」，不留白。

---

## 全角 / 符号开关的出口：**已落地（Rust 31 / 头文件 31）**

### 状态（2026-09-27 尾，实测；**本节当天经历了「差 3 个 → 补齐」**）

| | 数量 | 证据 |
|---|---|---|
| `crates/opi-ffi/src/cabi.rs` 的 `#[unsafe(no_mangle)]` | **31** | `opi_toggle_fullwidth` / `opi_fullwidth_state` / `opi_toggle_symbol` 已在 |
| `macos/OpiFFI.h` 的声明 | **31** | 指纹 `98aa35de7343647fda15967fc42e1c23`、mtime `23:42:53`（**已补齐**） |

**曾经差 3 个**（头文件停在指纹 `e3b37085…` / mtime `23:26:15`，**第三次**出现本 README
记过的那个滞后模式：`opi_candidates_page`/`opi_page_count` 一次、`opi_select_page` 一次）。
当时的后果是硬性的：`ios/OpiFFI.h` 是转发头，**Swift 侧看不见这三个函数，调了就是编译不过**
—— 所以那段时间本目录是「纯注释、零调用」。**`ffi-contract` 补齐后此约束消失。**

现在跑：
```
$ python3 /tmp/abi_parity.py
Rust 导出 = 31   头文件声明 = 31
名字差集 Rust−头: []
名字差集 头−Rust: []
签名不一致: 0
exit=0
```
⚠️ **上面这段是"声明面对齐"的证明，不是"Swift 能调"的证明** ——
Swift 那边仍然一行都没编译过（见第 5 节），本机永远证明不了。

⚠️ 头文件归 `ffi-contract`，**我全程没改它**。

**两条独立证据给出同一组名字**（我写 `abi_parity.py` 之前它出过好几次假阳性，
所以这次特意找了第二条）：

```
$ python3 /tmp/abi_parity.py
Rust 导出 = 31   头文件声明 = 28
名字差集 Rust−头: ['opi_fullwidth_state', 'opi_toggle_fullwidth', 'opi_toggle_symbol']
签名不一致: 0                                     （exit=1）

$ cargo test -p opi_ffi --test c_abi_contract     # ffi-contract 自己的门禁，我只跑不改
test macos_header_matches_cabi ... FAILED
crates/opi-ffi/src/cabi.rs 与 macos/OpiFFI.h 漂移（共 3 条）：
  头文件缺少 Rust 导出 `opi_fullwidth_state`
  头文件缺少 Rust 导出 `opi_toggle_fullwidth`
  头文件缺少 Rust 导出 `opi_toggle_symbol`
test result: FAILED. 13 passed; 1 failed
```

⚠️ 同一跑里 `header_compiles_and_consumer_builds ... ok` **不是反证** ——
那个 `c_abi/consumer.c` 只调**已声明的那些**，所以它过得去；
它证的是「旧 28 个仍自洽」，不是「头文件没事」。**别只读这一行。**

### 三条契约（**违反了都是静默失效**，接线前必读）

⚠️ **先说证据强度：这三条契约的强度只有「注释」，没有任何机器检查。**
`crates/opi-ffi/tests/c_abi_contract.rs` 管的是**名字与签名**（集合相等 + 类型），
`tests/jni/mode.rs` 管的是**值编码**（`mode_to_int` ↔ Kotlin `EngineMode`）。
「调完必须重读」「不许自己推导映射」属于**调用方的时序/值契约**，
两处门禁都**看不见** —— 现有门禁全绿也不代表这三条被遵守了。
能钉住它的模板是 `tests/c_abi/consumer.c`（真链接真调用），
但**那要人先写进去**；在那之前，读到这里的人**只能靠自己遵守**。

1. **重读时机**：`opi_switch_mode` **与** `opi_toggle_symbol` 是仅有的两个
   「调用后必须重读 `opi_fullwidth_state()`」的出口。后者**内部调了 `switch_mode`**，
   而 `switch_mode` 无条件执行 `fullwidth = mode.default_fullwidth()`
   （`composer.rs:33`：`matches!(self, Mode::Pinyin | Mode::Traditional)`）
   —— Symbol 的默认是**半角**，所以**按一下符号键会把全角指示灯悄悄弄灭**。
   调完 `opi_toggle_symbol` 要重读的不止全角，是 **mode / buffer / candidates / fullwidth 四个**。
2. **客户端不许自己推导映射**：全角是否映射**不是 `(mode, fullwidth)` 的纯函数** ——
   `engine.rs:129` 写着撇号在拼音/繁体是**音节分隔符**（`xi'an`），**只有缓冲空时**
   才当引号。同一个键、同一个模式，结果随缓冲状态不同。所以**别写**
   「拼音 + 全角 + `'` ⇒ `‘`」这类推导，**一律把键交给引擎**。
   （`opi_fullwidth_state()` 是给**状态栏**用的，不是给你算映射的。）
3. **全角跨模式不粘**（已裁定的产品语义）：Pinyin/Traditional → 全角，
   English/Number/Symbol → 半角；**任何模式切换都重置**（含 `opi_toggle_symbol` 内部的）。
   硬理由：做成全局 sticky 会让拼音下开的全角带到英文模式 ⇒ `,` 变 `，` ⇒
   直接违背「英文/数字半角直通」。**这是引擎语义，不是实现层能补的。**

签名（`cabi.rs` 实际值，与 lead 给的形状一致）：

```c
bool      opi_toggle_fullwidth(void);  /* 返回切换后的新状态 */
bool      opi_fullwidth_state(void);   /* 读侧；未装载 → false */
OpiString opi_toggle_symbol(void);     /* 返回**待上屏的文本**，空句柄 = 无提交 */
```

⚠️ `opi_toggle_symbol` 返回的**不是「刚切出来的那个符号」**，是**切模式前那截缓冲的
待提交文本**（有候选→首候选；乱码如 `zzz`→清掉不上屏）。**空句柄可无条件 free。**

### 这两件事的键位其实**已经裁决了**（Rust 侧注释是旧的）

`Engine::toggle_symbol` 的文档注释说「**键位不在本层**（计划 B5 仍未定，TSF 侧还要动
`vk.rs` 的映射表）」，`fcitx5-opi/src/candidate.rs` 与 `tsf-opi/src/logic.rs` 也各有一句
「各端键位未定，B0 未裁决」。**但这三句注释与隔壁的代码不一致**：

- `crates/tsf-opi/src/vk.rs` 的 `mode_hotkey`：**Ctrl + `'`** → 切英文、
  **Ctrl + `\`** → 切 Symbol；`crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 的
  `handleModeHotkey` 是**同一张表**（`FcitxKey_apostrophe` / `FcitxKey_backslash`）。
- `crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 的 `handleFullwidthHotkey`：**Shift+Space**
  （那份注释还附了键位占用调查：`strings` 扫过 fcitx5 全部库，Shift+Space 没被占）。
  TSF 的 `fullwidth_hotkey` 同判。

**这不是我该改的**（`crates/**` 各有其主），只报告：**注释说「未裁决」，代码已经裁决了，
而且是两轨一致**。接手 iOS/macOS 硬件键盘的人照 `vk.rs` 那两张表接即可，
别照注释里的「未定」去自创键位。

⚠️ **接的时候还有一条必须知道的（否则静默失效）**：这三个热键**不能送进
`opi_key_event`**，必须在**客户端侧、调引擎之前**判掉 ——
`router.rs` 的 `key_event` 直通分支（`router.rs:245`）对 `CTRL|ALT|META` **在 `key_event` 最前面就 `PassThrough`**
（这是有意的：⌘A/⌘C 不能被吃进拼音缓冲），所以 `Ctrl+'`/`Ctrl+\` 送进去等于交回宿主 App，
**模式不动、也没有任何日志**；而 `Shift+Space` 的 SHIFT **不在**直通掩码里，会一路走到
`KEY_SPACE` 分支，又因为「空格分支不看 Shift 位」而变成**选首候选**。
桌面两轨正是这么做的（`tsf-opi/src/tsf.rs` 把 `mode_hotkey`/`fullwidth_hotkey`
判在引擎之前，注释写明「送进引擎就是普通空格，会被当成选首候选」）。
（`ios/KeyboardHardware.swift` 的硬件键盘一节已把这条写成注释；
**已按这条接线**：`hotkey(for:states:)` 判在 `routeHardware` 里、`engine.keyEvent` 之前，
`performHotkey(_:)` 执行。⚠️ 抬起也**认出来但只消费不执行** —— 只拦按下会给宿主一个
**没有 keydown 的 keyup**（桌面两轨都注了这条）。）

⚠️ **autorepeat 同判「认出来、但不执行」**（2026-09-27 裁决）。按住 `Shift+Space` 稍久一点，
系统会发重复 keyDown；让它们执行就会**反复切全角**，而用户的语义是「一次切换」。
**但不能简单地在判定函数里返回「不是热键」** —— 那一颗键会漏给 `keyEvent`，
而 `Shift+Space` 的 SHIFT 不在直通掩码里 ⇒ 会被当成**普通空格＝选首候选**，比反复切更坏。
所以 `hotkey(...)` **故意不过滤** repeat / 抬起，由**调用处**决定「消费但不动作」。

- macOS 侧这半句是**活代码**（`NSEvent.isARepeat`）。
- **iOS 侧这半句是预防性的**：`UIKeyModifierFlags` 里没有 repeat 位，
  `UIKey`/`UIPress` 上也没有别的来源（凭记忆，本机无从核对）。
  **不要为了「四轨对称」去编一个位出来** —— 全引擎只有一个消费者
  （`router.rs` 的 `handle_shift` 里那句 `let repeat = states & KEY_STATE_REPEAT != 0;`：按住 ⇧ 时不要反复切 shift 状态机），
  而 iOS 的 ⇧ 走的是**另一种**机制（软键盘长按 → `stateLongPressed` = Lock）。

> ### 🔲 **待 Mac 侧核实**：iOS 到底有没有 autorepeat 信号（2026-09-28）
>
> 上面那句「`UIKeyModifierFlags` 里没有 repeat 位」是**记忆级**证据，**不是核对级** ——
> 核它需要 Apple SDK，本机没有（team-lead 同样没有，所以这条**谁也替谁背不了书**）。
> 按它**写下的翻案条件**来：
>
> **翻案条件**：若 UIKit 其实**提供了** autorepeat 信号（`UIKey`/`UIPress` 上的任何成员、
> 或 `UIKeyModifierFlags` 的某个位），则
> 1. 「**不置 REPEAT**」这个结论**作废**；
> 2. `KeyboardHardware.swift` 里那句 `states & OpiKey.stateRepeat == 0` **从「预防性」变成活代码**；
> 3. 该信号的接入点是**唯一一处**：`modifierStates(_:)`（那里已写明「哪天真找到就加在这里，
>    别在调用处手搓一个位」）。
>
> 怎么核：在 Mac 上写三行探针 —— 按住 `Shift+Space` 不放，把每个 `UIPress` 的
> `type` / `key.modifierFlags.rawValue` / `key.keyCode` 打出来，看**第二次及以后**的
> 那几颗与第一次**有没有任何可区分的字段**。全等 ⇒ 没有信号，结论维持。
> **查到就回来改这一段**，别让「记忆级」在文档里冒充「已验证」。

⚠️ **另两轨不在本分身的域内**（`crates/**` 各有其主）。**接之前先 grep 这两个名字确认**，
别信任何一侧的转述 —— 下面是**判据与位置**，不是状态断言：

> ⚠️ **表里的 `crates/...` 路径是「写这一节时」的位置，不是承诺 —— 以符号名为准。**
> 2026-09-28 当场量到这件事的实况：`crates/tsf-opi/src/vk.rs` 先是 **520 行**（超 500，
> `ime-platform` 在拆），**几分钟后我复核时已是 222 行**（拆分落地），而三个符号
> **都还在 `vk.rs` 里、行号也没变**（`hotkey_should_act` :127、`mode_hotkey` :161、
> `fullwidth_hotkey` :209）。走掉的那 298 行是**测试模块** —— 搬到了
> `crates/tsf-opi/src/vk_tests.rs`（由 `vk.rs:221` 的 `#[path = …]` 引入，
> 与本目录已有的 7 个 `*_tests.rs` 同惯例），**不是**语义拆分。
> ⇒ **别照抄这里的路径或行号**（上面那三个行号也只是**那一次**的观察值），找符号用
> `grep -rn 'fn hotkey_should_act' crates/tsf-opi/`。这正是本节**只写「文件 + 符号」、
> 不写行号**的同一个理由，只不过「文件」这半边同样会漂。

| 轨 | 判定函数 | 「认领但不动作」写在哪 |
|---|---|---|
| TSF | `crates/tsf-opi/src/vk.rs` 的 `hotkey_should_act` | `states & (KEY_STATE_REPEAT \| KEY_STATE_RELEASED) == 0`；`tsf.rs` 的两个调用点各自先过它再动作 |
| fcitx5 | `crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 的 `isRepeatEvent` | `!keyEvent.isRelease() && !isRepeatEvent(keyEvent)`（⚠️ 重复位取自 `keyEvent.rawKey()`，见那份文件里 `KeyState::Repeat` 的掩码说明） |
| iOS / macOS | `hotkey(...)` + 调用处 | 见本节上一段 |

> **2026-09-27 观察**：那时**四轨都已有这条**，而且是**同一个设计** ——
> 判定函数照常返回「是热键」（好让调用处**认领**这颗键），由调用处决定要不要动作。
> ⚠️ 但另两轨那两处当时在**别人的工作区里、未提交**（`git status` 里是 `M`，且
> `cargo fmt --check` 正报 `vk.rs` 的 import 折行未格式化 = 编辑途中），
> **所以上面这段是观察，不是承诺** —— 以 `grep` 结果为准。

⚠️ 上面这张表**故意不写行号**，改用「文件 + 符号」—— 这是团队约定（多分身并发下裸行号十分钟就漂），
**不是** `CLAUDE.md` 的成文规矩。
写这一节时当场量到漂移：`vk.rs` 的 `mode_hotkey` 从 `:139` 漂到 `:161`、
`fullwidth_hotkey` 从 `:185` 漂到 `:209`，`opi_fcitx5.cpp` 的 `handleModeHotkey`
从 `:232` 漂到 `:244`、`handleFullwidthHotkey` 从 `:285` 漂到 `:302`，
`tsf.rs` 那条「刷了是白推」的注释从 `:213` 漂到 `:227`。
**它们是并行分身在改的文件，行号十分钟就漂；函数名不会。**
（仍然准确的那几个 —— `router.rs:245`、`router.rs` 的 `handle_shift`、`engine.rs:228` ——
保留行号是因为那句就在原处，且旁边都写了符号名。）

### 本草案因此怎么做

- **`opi_toggle_symbol` 不是 `cycleMode()` 的替代品 —— 它俩是两件事。**
  `cycleMode()`（键面上的中/繁/英）走的是「离开拼音族先 `clear()`」，与 Android
  `ImeScreen.kt` 的 `toggleMode()` 对齐；而 `Engine::toggle_symbol`
  （`engine.rs:228`）第一句就是
  `let target = if mode == Mode::Symbol { Mode::Pinyin } else { Mode::Symbol };`
  —— 它是 **Pinyin ⇄ Symbol 的来回切**，对应的是热键 `Ctrl+\`，不是模式循环。
  ⚠️ 我一度在本 README 里把它写成 `cycleMode()` 的升级版，**那是错的**（读到 `engine.rs` 才发现）。
  两者唯一重叠的地方是**收尾缓冲**：`toggle_symbol` 有候选就提交首候选、乱码缓冲才丢弃，
  而 `clear()` 那条会**丢掉打了一半的拼音**（Android 也这样）。所以 `cycleMode()` 保持原样，
  `Ctrl+\` 走 `opi_toggle_symbol`。
- 全角开关现在**可以做了**（声明面已齐）。键位 `Shift+Space` 见上。
  注意读侧 `opi_fullwidth_state()` 的语义：**「未装载」与「已装载且半角」共用 `false`**，
  所以它**不是**在报错 —— 那时按键全部交系统，宿主拿到的本来就是半角。
  另外全角只影响标点，且**非中文模式是机械全角**：`.` → `．`(U+FF0E)，**不是** `。`(U+3002)。
- （`Mode::Symbol` 本身**到得了**：`opi_switch_mode(3)` 是通的。iOS 的符号面板是
  **数据驱动**的，压根不经模式，与 Android 同一条路。）

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
