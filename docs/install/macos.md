# macOS 输入法（InputMethodKit）—— 构建 / 打包 / 安装 / 验证

> ## 这份文档的性质：**起点与契约，不是可用实现**
>
> 教程里的每一步都要分清**两层**，它们的状态差得很远：
>
> | 层 | 状态 | 判据 |
> |---|---|---|
> | Rust 侧 C ABI（`crates/opi-ffi`） | **已实测**能为 `aarch64-apple-darwin` 编译、能产出 `libopi_ffi.a`，且导出符号集合与 `cabi.rs` 完全一致 | 见「产出 `libopi_ffi.a`」，本机可复现 |
> | `macos/` 下的 Swift（3 个文件） | **一行都没有被编译过** | 本机是 Linux，没有 Xcode / Apple SDK，`import InputMethodKit` 就断 |
> | `macos/OpiFFI.h` | 过了 `clang -fsyntax-only`（C 与 C++ 两种模式），**但从未被链接过** | 「clang 说这堆声明自洽」与「ld 能把它们接到 Rust 的符号上」是两件事 |
>
> **别把「Rust 侧能编」读成「macOS 端能用」。** 本项目已经被「没被编译器看过的代码」
> 坑过两次，本目录的存在正是为了避免第三次：
>
> | 前两次 | 代价 |
> |---|---|
> | fcitx5 C++ 插件 | 写在仓库里、README 记着「待验收」，一编译发现 **7 处 API 误写**（连 `fcitx::InputMethod` 这个类都不存在） |
> | Windows TSF | README 记着「完成，待在 Windows 上验收」，实测 `DllGetClassObject` 恒返 `CLASS_E_CLASSNOTAVAILABLE`、**文本插入从未实现** → 整个端不可用 |
>
> 所以本教程的目标不是「照着做就能用」，而是**把第一轮编译要撞的墙提前标出来**
> （待修清单见 `macos/README.md` 的「最不确定的 API 清单」，
> 那一节是本目录最有价值的部分），**并说清哪些步骤走完之后才算真的接对了**。
> 更细的依据与全部实测记录在 `macos/README.md`。

---

## 1. 前提

| 项 | 要求 | 依据 / 状态 |
|---|---|---|
| Mac + Xcode | 必须有。**仓库没有声明最低 Xcode 版本，也没有声明 macOS 部署目标** | 未核实：仓库里没有任何 deployment target / Xcode 版本声明 |
| Rust | `Cargo.toml` 的 `[workspace.package] rust-version`（当前声明 1.88）。**这是声明**，实际下限由 CI 的 MSRV 执行点量出 | `Cargo.toml` · `.github/workflows/ci.yml` |
| Rust target | `aarch64-apple-darwin` + `x86_64-apple-darwin`（两者都编，再用 `lipo` 合成通用二进制） | CI 两个都在跑 |
| `lipo` | 合成通用库用，Xcode 自带 | —— |
| 词库 | 可选。**不放**就用内置的回退库（候选质量明显下降）。放的话路径是 `~/Library/Application Support/opi/luna.opid` | 这条 macOS 约定**未经核对**（`OpiEngine.defaultDictionaryPath`） |
| 工程文件 | **仓库里没有 `.xcodeproj`、没有 `Package.swift`、没有 `module.modulemap`** —— 工程要在 Mac 上用 Xcode 新建 | `macos/README.md`「构建集成」：手写 `project.pbxproj` 会是第三件「没被工具看过的产物」 |

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```
⚠️ **本机踩过的坑**：`rustup target add aarch64-apple-darwin` 可能以
`detected conflict: '.../libaddr2line-*.rlib'` 失败，且 `rustup target list --installed`
**不列出** darwin 目标 —— 但 `cargo check` 照样能过。**别拿 `rustup target list` 的空输出
当成「目标没装」。**

---

## 2. 产出 `libopi_ffi.a`

`crates/opi-ffi/Cargo.toml` 的 `crate-type` 已含 `staticlib`（实测：`grep -n 'crate-type' crates/opi-ffi/Cargo.toml`
→ `["cdylib", "staticlib", "lib"]`），**不需要再改 Cargo 配置**。

先在 Mac 上确认这台机器与 CI 看到的是同一个东西（**只在 Linux 上能跑的那几条 check，
本机实测 exit 0**）：

```
$ cargo check -p opi_ffi --target aarch64-apple-darwin
    Checking engine-core v1.3.1 (...)
    Checking engine-data v1.3.1 (...)
    Checking opi_ffi v1.3.1 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 39.43s      # exit 0
$ cargo check -p opi_ffi --target x86_64-apple-darwin                         # exit 0
$ cargo check --tests -p opi_ffi                                              # 宿主上的路由单测
```
（输出里的 `v1.3.1` 是**跑这些命令时**的工作区版本，不是当前版本 —— 当前版本以 `Cargo.toml` 为准。
本文照原样保留实测记录。）

真要出库（通用二进制）：

```bash
cargo rustc --release -p opi_ffi --target aarch64-apple-darwin --crate-type staticlib
cargo rustc --release -p opi_ffi --target x86_64-apple-darwin  --crate-type staticlib
lipo -create \
  target/aarch64-apple-darwin/release/libopi_ffi.a \
  target/x86_64-apple-darwin/release/libopi_ffi.a \
  -output build/libopi_ffi.a
```

本机实测（Linux，无 Apple 链接器也能**产出归档** —— `staticlib` 这一步不需要链接器；
下面是 `--target aarch64-apple-darwin` 那一条的实测）：

```
$ file target/aarch64-apple-darwin/release/libopi_ffi.a
    current ar archive
$ ar t libopi_ffi.a | grep -m1 '^opi_ffi-'      # 按**实际成员名**取（各目标的目标文件名不同）
$ ar x libopi_ffi.a <上面那个成员名> && file <那个 .o>
    Mach-O 64-bit arm64 object, flags:<|SUBSECTIONS_VIA_SYMBOLS>
```
⚠️ 取成员要用 `ar t` 列出来的**真实名字**：不同 target 的 `.rcgu.o` 文件名不同，
从别的 target 抄一个名字会让 `ar x` 什么都不解出来，而 `file` 那时读到的
可能是**上一次留下的旧文件** —— 探针自己骗自己。

**导出符号齐不齐**——别数个数，做**集合比对**：

```bash
grep -A1 'unsafe(no_mangle)' crates/opi-ffi/src/cabi.rs | grep -o 'fn opi_[a-z_]*' \
      | sed 's/fn /_/' | sort -u > /tmp/rs.txt
grep -a -o '_opi_[a-z_]*' build/libopi_ffi.a | sort -u > /tmp/a.txt
diff /tmp/rs.txt /tmp/a.txt        # 本机实测：无输出（集合相等）
```
**本文件不写导出条数** —— 它随每次扩容变（`cabi.rs` 已经扩容过多次，
`macos/README.md` 记了一份历史，但**那份历史的末尾同样会过期**）。
条数以 `crates/opi-ffi/tests/c_abi_contract.rs`（`macos_header_matches_cabi`）那道门禁为准。

⚠️ 方法是**降级**的：本机的 GNU `nm` / `objdump` **不认 Mach-O**
（`nm: __.SYMDEF: file format not recognized`），所以上面用的是 `grep -a` 读归档里的
符号名字符串。它证的是「归档里存在这些名字」，**不是**「链接器能解析它们」。
在 Mac 上要拿真正的符号表，用 `nm -gU libopi_ffi.a` 或 `xcrun nm`。

⚠️ `cargo build -p opi_ffi --target aarch64-apple-darwin`（不带 `--crate-type`）在 Linux 上
**会失败**，因为 `crate-type` 含 `cdylib`：
```
error: linking with `cc` failed: exit status: 1
  = note: cc: error: unrecognized command-line option '-arch'
          cc: error: unrecognized command-line option '-mmacosx-version-min=...'
```
判据：错在**链接**这一步、且是 `cc` 不认 Apple 参数 → 换台装了 Xcode 的 Mac 就好。
**这不是代码缺陷。** 顺带：`-mmacosx-version-min=...` 那个值是 **Rust 给该目标的默认**，
**不是**本项目的兼容性承诺 —— 要支持更老的系统得自己设。

⚠️ **未验证**：链接期是否需要额外系统库。`opi_ffi` 依赖 `jni` crate（为 JNI 出口），
它在 macOS 目标上能编译，且 `JNI_OnLoad` 是**导出**符号而非导入，
按此推断链接期不需要 libjvm —— **但这是推断，不是实测**。

---

## 3. 在 Xcode 里建工程

1. 新建 macOS App（**不是**命令行工具）：产物**必须**是 `OpiInputMethod.app` ——
   `main.swift` 靠 `Bundle.main` 取连接名与 bundle id，直接跑可执行文件会拿不到 bundle id。
2. 加 3 个 Swift 文件：`OpiEngine.swift` · `InputController.swift` · `main.swift`。
   ⚠️ **`main.swift` 这个名字不能改** —— Swift 只允许名为 `main.swift` 的文件放顶层语句
   （`IMKServer(...)` / `NSApplication.shared.run()` 都在顶层）。
3. 设置：

| 设置 | 值 |
|---|---|
| `SWIFT_OBJC_BRIDGING_HEADER` | `macos/OpiFFI.h`（Swift 看不见裸 C 符号，必须经一个头文件） |
| 链接库 | `libopi_ffi.a`（或上一步的通用库） |
| 链接框架 | `InputMethodKit`、`Cocoa` |
| `PRODUCT_BUNDLE_IDENTIFIER` | 例如 `xyz.erik.opi` —— 要进 Info.plist 的 `CFBundleIdentifier` |
| 产物 | `OpiInputMethod.app` |

**声明只有 `OpiFFI.h` 一份**，不要在 Swift 里再抄一遍 `@_silgen_name` 之类的第二份声明。
（要改用 SwiftPM 的话：把 `OpiFFI.h` + `module.modulemap` 放进一个 C target，
Swift target 依赖它 —— 但**仓库里没有** `module.modulemap`，得自己写。）

4. 首次编译的预期：**一堆 API 报错，这是正常的**。待修清单在 `macos/README.md`
   的「最不确定的 API 清单」（`client.insertText(_:replacementRange:)` 的
   `replacementRange` 取值、`setMarkedText` 签名、`IMKCandidates` 的构造与常量名、
   `NSEvent.keyCode` 的那张表、`IMKServer` 的启动顺序……）。
   **把编译错误当成这份草案的待办清单**：那个清单的长度本身就是本目录可信度的度量。

另一种产物形态（不想改 `crate-type` 时的替代方案）：直接用现成的 `cdylib`，
把 `libopi_ffi.dylib` 放进 `OpiInputMethod.app/Contents/Frameworks/`，
并把 rpath 设成 `@executable_path/../Frameworks`。fcitx5 轨就是这么做的（`$ORIGIN` rpath），
代价是多一个运行时依赖。

---

## 4. 打包

macOS 输入法是**一个 .app**，由 `Info.plist` 里的 IMK 键声明：

| 键 | 值 | 说明 |
|---|---|---|
| `InputMethodConnectionName` | `OpiInputMethod_Connection` | **连接名**。`main.swift` 的 `IMKServer(name:bundleIdentifier:)` 读的正是这个键（读不到时用同名字面量兜底），所以**两端必须逐字一致**。不一致的症状是「**进程在跑、系统输入法列表里也点得到，但按键永远进不来**」 |
| `InputMethodServerControllerClass` | `$(PRODUCT_MODULE_NAME).OpiInputController` | 现代 Swift 构建**必须带模块名前缀**，否则 IMK 实例化不出控制器，症状是「输入法能选、但完全无响应」。非 Xcode 的构件系统**不展开**这个变量，要写字面值。Swift 侧另有 `@objc(OpiInputController)` 作为裸名兜底 |
| `LSBackgroundOnly` / `LSUIElement` | `true` | 常驻后台、无 Dock 图标 |
| `tsInputMethodCharacterRepertoireKey` | `Latn` + `zh-Hans` + `zh-Hant` | ⚠️ **取值未核实**。资料说该键收 ISO 15924 **文字**代码（不是语言代码），而 `zh-Hans`/`zh-Hant` 是 BCP-47 **语言**标签 —— 对不上（真正的文字代码是 `Hans`/`Hant`）。**查不到权威取值表，故保留现状**；Mac 上请打开 `/System/Library/Input Methods/` 下某个系统中文输入法的 `Info.plist` 对照后再决定改不改 |
| `tsInputMethodIconFileKey` | `opi.tiff` | ⚠️ **仓库里没有这个文件**。缺文件时系统一般用默认图标；要正式发布就补一个，或在 plist 里去掉这个键 |
| `CFBundleShortVersionString` / `CFBundleVersion` | 手写字面值 | **不**由 `Cargo.toml` 自动生成 —— 发版时容易忘记改。维护方式二选一：发版清单加一条「同步 `macos/Info.plist`」，或让构建脚本从 `Cargo.toml` 注入 |

> 以上键名来自 InputMethodKit 的公开约定，**取值没被核对过**（`macos/README.md`
> 「最不确定的 API」#6）。打包完第一件事是拿系统输入法的 plist 做比对。

---

## 5. 安装与启用

> 以下步骤**除了 `~/Library/Input Methods/` 这个路径**（Apple 的 TIS 文档明确：
> 输入法包放 `~/Library/Input Methods/` 或 `/Library/Input Methods/`）**其余都是凭记忆写的，
> 请在 Mac 上逐条核对** —— macOS 各版本「系统设置」的改名/改路径很频繁。

```bash
# 1) 装（用户级，不需要 sudo）
mkdir -p ~/Library/Input\ Methods
cp -R OpiInputMethod.app ~/Library/Input\ Methods/

# 2) 若旧版本正在跑，先杀掉（否则系统仍在用旧二进制；改完必须重来这一步）
killall OpiInputMethod 2>/dev/null || true

# 3) 词库（可选；不放则用内置回退库，候选质量会明显下降）
mkdir -p ~/Library/Application\ Support/opi
cp luna.opid ~/Library/Application\ Support/opi/luna.opid
```

4. 打开 **系统设置 → 键盘 → 文本输入 → 输入法 → 编辑… → +**，在「中文（简体）」
   或「其他」下找到 **OPI 拼音** 加入（也可勾选「在菜单栏中显示输入法菜单」）。
5. 切到 OPI 拼音，在文本编辑器里敲 `h a o 空格`。

排障：

| 症状 | 先查 |
|---|---|
| 系统输入法列表里根本没有 | `CFBundleIdentifier` 是否唯一；app 是否真在 `~/Library/Input Methods/` |
| 能选、但按键永远进不来 | `InputMethodConnectionName` 与 `main.swift` 取到的连接名是否一致（**两个首要嫌疑之一**）；另一个是覆写没生效：`IMKInputController` 的选择器是 `handleEvent:client:`，Swift 侧可能是 `handle(_:client:)` 也可能要写成 `handleEvent(_:client:)`，用 `@objc(handleEvent:client:)` 钉住即可 |
| 进程直接退出 | run loop 起法（`main.swift` 的 `NSApplication.shared.run()`）；从命令行直接跑可执行文件会因拿不到 bundle id 而 `exit(1)` |
| 看不到输入法、又找不到别的原因 | `killall OpiInputMethod` + **注销重新登录**（最可靠） |

日志：`NSLog` 输出在 Console.app 里按进程名 `OpiInputMethod` 过滤
（本目录的装载失败告警都走 NSLog）。进程活没活可以直接 `pgrep OpiInputMethod`。

---

## 6. 验证 —— 怎么知道接对了

按顺序走，**每一步只证明一件事**：

1. **进程在跑**（`pgrep OpiInputMethod`）⇒ IMKServer 起来了。
   —— 注意这一条**不证明按键能进来**（连接名/控制器类错时，进程照样活着）。
2. **系统设置里能添加并切到「OPI 拼音」** ⇒ `Info.plist` 的键对了、控制器类实例化得出来。
3. **敲 `h` `a` `o` `空格` ⇒ 提交「好」**（`macos/README.md`「在 Mac 上第一件要做的事」第 4 步）。
   这条**逐字对齐 fcitx5 轨的实测表**（`crates/fcitx5-opi/cpp/README.md`「状态」一节：
   `h`/`a`/`o`/`SPACE` 的 action 是 1/1/1/2，提交 `好`）。
   **同一个引擎、同一套语义，两端的最小可见成功必须一致**；
   不一致就是桥接层的问题，不是引擎的。
4. **preedit（marked text）显示正常** ⇒ `setMarkedText` 那一路对了。
   若 preedit 不显示但提交正常，问题在 preedit；若提交也不正常，问题在提交
   （`client.insertText(_:replacementRange:)`，**整条链路里最不确定的一处**：
   本目录用的是 `NSRange(location: NSNotFound, length: NSNotFound)`，
   而 Apple 论坛示例用 `length: 0` —— 这处差异必须实测）。
5. 只有 1–4 都过之后，才谈功能（候选窗、模式切换入口、用户词管理 UI……）。

---

## 7. 已知限制（诚实清单）

**这些不是 bug，是「还没做」或有意的取舍** —— 交付时请如实转述，
别当缺陷重报，也别当没看见。

| # | 限制 | 说明 |
|---|---|---|
| G2 | **繁体模式在本端不可达** | 引擎与导出都支持（`Mode::Traditional` = 4），缺的只是入口。判据：`grep -n 'traditional' macos/InputController.swift` 零命中；`performHotkey` 里那处 `switchMode` 只在 English ⇄ Pinyin 之间来回，`Ctrl+\` 走的是 `opi_toggle_symbol`（**Pinyin ⇄ Symbol 来回切**，不是模式循环）。**与 iOS 的 `cycleMode()`（中→繁→英）不对称 —— 别以为两端一样。** 另外 macOS 端**没有三态循环**（那需要 IMK 菜单项），但有一个热键 `Ctrl+'` 切英文/回拼音 |
| G3 | **候选窗显示的是全局列表，不是 Rust 的当前页** | `InputController.swift` 的 `refresh()` 把 `opi_candidates(limit: 64)`（引擎级全局序、**不分页**）整份塞给候选窗，而 `router.rs` 内部维护 `page` 与 `PAGE_SIZE`。后果：翻页后数字键选中的候选与候选窗高亮的位置**可能不是同一个**。⚠️ 那个 `limit: 64` 是**本端独有的第三份常量**：既不是前端页大小、也不是 Rust 的抓取上限（`FETCH_LIMIT` 现在不设上限），而且它走的正是 `opi_candidates_page` 文档注释**点名禁止**的那条路径（原话：「前端显示当前页请用本出口、**不要**拿 `opi_candidates()` 自己按 8 切」）。**出口都已声明但本目录还没有调用点**：`opi_candidates_page` / `opi_select_page` / `opi_page` / `opi_page_count` / `opi_shift_state`。接的时候两条禁令：**不要**自己按 8 切；**不要**算 `opi_page() * 8 + k` 再喂 `opi_select()` |
| G4 | **无测试、无 CI、无工程文件** | `macos/` 里跑过的只有头文件（`clang -fsyntax-only` + 「导出名取地址」那个 TU）。**Swift 三个文件的验证数是 0。** 第一条针对 Swift 的可执行验证就是「在 Mac 上编译通过」 |
| G5 | **宠物「小欧」未接（有意）** | 候选窗用的是系统面板 `IMKCandidates`，**画不进去**。要摆小欧得先换成自绘 `NSPanel` —— 那是对未编译代码做结构改动。真要接时**别**新写一份 CoreGraphics 几何：几何真源是 `docs/opi-pet.svg`，同一张图的第三份拷贝必然漂移 |
| G6 | **符号库：包装有了，UI 没有** | `OpiEngine.swift` 有 `symbolBlocks()` / `symbolsInBlock(id:)` / `searchSymbols(keyword:)`，但 macOS 侧**没有任何调用点** —— 没有符号面板（候选窗是 `IMKCandidates`，摆不下网格）。三条契约：①`opi_symbol_blocks()` **只回 common 块**；②块 id 在 Rust 侧是 `u16`、在 C ABI 是 **`int16_t`**，只能对齐到 0..=32767，直接解成 `Int16` 会在 id > 32767 时抛异常，而本层对解析失败的处理是「返回空数组」⇒ 面板整块空掉且**无日志**；③`opi_search_symbols("")` 返回**全部**条目，不是「没搜到」 |
| L1 | **全角状态没有 UI 读** | `opi_fullwidth_state()` 出口有了、Swift 包装有了、**UI 没读**。所以也仍然没有会漂移的镜像。⚠️ 它**不是**在报错：「未装载」与「已装载且半角」共用 `false`，那时按键全部交系统，宿主拿到的本来就是半角 |
| L2 | **⇧ 长按 → Lock 不可达** | `NSEvent` 没有长按信号（同 fcitx5 轨的已知边界）。本层**不合成** `LONG_PRESSED` 位，所以 ⇧ 长按 = 单击。要接得自己定入口，别靠猜补一个信号 |
| L3 | **`keyUp` 大概率收不到** | `recognizedEvents:` 属于 `IMKStateSetting`，默认只返回 `NSKeyDownMask`；公开报告说即使加上 `NSKeyUpMask`，`NSKeyUp` **仍然**不会被投递给 `handleEvent:client:`（被指为 IMKit 已知问题）。**故本轮不动** `recognizedEvents`。影响：`RELEASED` 位收不到时，退格/回车的「按下放行、抬起也放行」对称性不成立，在依赖键状态的控件里可能表现为卡键 —— ⚠️ **但这个影响的前提本身也没被验证**（取决于 IMK 是否替我们转发了抬起）。代码里那个 `.keyUp` 分支很可能是死代码 |
| L4 | **autorepeat 认领但不执行** | 按住 `Shift+Space` 稍久，`NSEvent` 会发重复 keyDown；让它们执行会**反复切全角**，而用户的语义是「一次切换」。也**不能**在判定函数里返回「不是热键」—— 那一颗键会漏给 `keyEvent`，而 `Shift+Space` 的 SHIFT 不在直通掩码里 ⇒ 被当成**普通空格 = 选首候选**，比反复切更坏。所以判定函数故意不过滤，由调用处 `!event.isARepeat` 决定 |
| L5 | **⌘ 必须如实置 `META` 位** | 由 Rust 侧 `router.rs` 的直通掩码（`CTRL\|ALT\|META`）放行 —— 本层**故意没有**再拦一道（那会把规则抄成第二份）。⚠️ 这条依赖要跟着 Rust 侧复核：掩码一旦去掉 META，⌘A 会被当成普通 `'a'` 吃进缓冲 |

---

## 8. 未验证清单（照抄现状，别读成已完成）

- `macos/` 下 **3 个 `.swift`**（`OpiEngine.swift` / `InputController.swift` / `main.swift`）：
  **没有编译、没有 lint、没有单测**。
- `macos/Info.plist`：**没有被任何 macOS 构建系统读过**，也没有装进过任何系统的输入法列表。
- `macos/OpiFFI.h`：过了 `clang -fsyntax-only`（C 与 C++ 两种模式，`-Werror` 下 exit 0），
  也过了「把 `cabi.rs` 的每个导出名**取地址**」那个 TU。**但它从未被链接过**，
  且那条检查对**签名/类型是瞎的**（`&f` → `const void *` 对任何函数类型都成立）。
- **「最不确定的 API 清单」共 10 条**，逐条都标了不确定点（提交文本、
  `setMarkedText` 签名、`IMKCandidates` 构造与常量名、`candidateSelected` 怎么拿客户端、
  启动与 run loop、`Info.plist` 键、`NSEvent.keyCode` 数值表、
  `charactersIgnoringModifiers` 语义、修饰位与 keyUp、词库路径）。
  接手人从这里开始最省时间。
- `Info.plist` 的 `tsInputMethodCharacterRepertoireKey` 取值、`tsInputMethodIconFileKey`
  指向的文件、`~/Library/Application Support/opi/luna.opid` 这条约定 —— **全都没核对**。

**做完这些测试，上述若干条会被推翻 —— 推翻时就地改，别让「记忆级」在文档里冒充「已验证」。**

---

## 9. 下一步顺序（别跳）

```
1. 编译链接通过（Xcode 建工程，把 API 报错当待办清单消掉）—— 一行逻辑都不要改
2. 最小冒烟：装进 ~/Library/Input Methods/、添加输入源、h a o 空格 →「好」
3. 逐个消掉「最不确定的 API 清单」那 10 条
4. 只有 3 过了，才谈补功能（候选窗、模式入口、符号面板、宠物）
```

**顺序不能反。** 在编译链接通过之前做的任何功能工作，都是在给一份可能整体重写的代码上加东西。
更细的依据、每一项的证据与订正记录在 `macos/README.md`。
