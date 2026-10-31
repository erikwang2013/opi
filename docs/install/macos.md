# macOS 输入法（InputMethodKit）—— 构建 / 打包 / 安装 / 验证

> ### v1.26.0 功能注记（2026-10-26）
>
> - **用户词落盘**：`~/Library/Application Support/opi/opi_user_words.json`，选词后 800ms 防抖；损坏或 0 字节文件原样保留。这套持久化尚未在真机验收。
> - **外部词库**：设置窗导入与启动重放都走 `opi_import_dict`，受 8 MiB / 200,000 条限制；完整 34–39 MiB 词库只能在 Linux/Windows 目录直载，macOS 请用缺省截断包（见 [`../external-dictionaries.md`](../external-dictionaries.md)）。
> - **括号配对（2026-10-28 已接）**：IMK client 的 `insertText`/`replacementRange`，光标回移依赖宿主的 `doCommandBySelector`；宿主不支持时光标不回移、只插入。**未上机验证**。智能学习加权/撤销与英文频次是引擎语义，macOS 直接继承。

> ## 这份文档的性质：**起点与契约，不是可用实现**
>
> 教程里的每一步都要分清**两层**，它们的状态差得很远：
>
> | 层 | 状态 | 判据 |
> |---|---|---|
> | Rust 侧 C ABI（`crates/opi-ffi`） | **已实测**能为 `aarch64-apple-darwin` 编译、能产出 `libopi_ffi.a`，且导出符号集合与 `cabi.rs` 完全一致 | 见「产出 `libopi_ffi.a`」，本机可复现 |
> | `macos/` 下的 Swift（7 个文件） | **2026-10-20 的 7 个文件**已在 run `39425977787`（head 739df45）由 CI `apple.yml` 的 `swiftc macos/*.swift` 真编过（typecheck + 两架构 + 链接 + ad-hoc 签名打包）；运行时行为仍是 0 验证 | 本机仍是 Linux（`import InputMethodKit` 就断）；编译由 CI 的 macos-latest 补上，真机未装过；见 §4.5 |
> | `macos/OpiFFI.h` | 过了 `clang -fsyntax-only`（C 与 C++ 两种模式）。⏳ 2026-10-09 起**也被链接过了**：swiftc 真编时把它当桥接头、并链上 Rust 库（CI `apple.yml`，run 37935240583，见 §4.5） | 「clang 说这堆声明自洽」与「ld 能把它们接到 Rust 的符号上」是两件事——后者 2026-10-09 由 CI 补上；**没被 Swift 调到的声明**仍只过了语法检查 |
>
> **2026-10-21 对齐轮**：按 Linux 补齐悬浮状态栏（仅 opi 输入源激活时显示）、8 页设置
> （常用/按键/外观/状态栏/符号与表情/词库/五笔/关于）、表情/贴纸、宠物图标与自绘
> 虚拟键盘；Apple CI run `39486273199` 真编译通过。**运行期（IMK 加载、NSPanel、
> 状态栏激活时序、虚拟键盘焦点）仍未验证**；ENPRO、IMKCandidates 皮肤/字号/宠物等
> 平台不可达项按禁用/明确标注处理；Shift 轻按 2026-10-28 已接（`flagsChanged`，未上机验证）。
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
| 词库 | pkg 已随包 `Resources/dicts/{luna,trad,stroke,wubi86}.opid`；用户放在 `~/Library/Application Support/opi/<name>.opid` 的同名文件会覆盖包内副本。缺文件时才回退内置库 | 打包脚本 `scripts/apple/package-macos.sh`；覆盖顺序见 `macos/OpiEngine.swift` 的 `defaultDictionaryPath` |
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
2. 加 7 个 Swift 文件：`OpiEngine.swift` · `InputController.swift` · `OpiMenu.swift` ·
   `OpiSymbolPanel.swift` · `OpiSettings.swift` · `OpiSettingsWindow.swift` · `main.swift`。
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

   > ⏳ **2026-10-09 订正：首次编译已经发生**（CI `apple.yml`，见 §4.5）。撞出来的
   > 编译错误共三类，都已就地修好：① `IMKServer(name:bundleIdentifier:)` 是
   > **可失败** init（nil = 连接名被占用，改为 guard + 退出）；② `IMKCandidates` 的
   > 构造也是**可失败**的（候选窗建不起来时只记日志、不退出——它是显示层）；
   > ③ `updateCandidates()` 在 Swift 里叫 `update()`。清单里其余条目
   > （`insertText`/`setMarkedText` 的 replacementRange 取值、run loop、plist 键）
   > **依旧未验证** —— 编译过 ≠ 用起来对。

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
| `tsInputMethodIconFileKey` | `opi.tiff` | ✅ **仓库已带**（`macos/opi.tiff`，生成物 —— 重出命令见 `scripts/gen_pet_icons.py`；打包时须放进 bundle 的 `Contents/Resources/`，本仓库没有工程文件，那一步由你在 Xcode 里补）。⚠️ 这个键走**模板渲染**：系统只按 alpha 通道着色（与图里的颜色无关）⇒ 菜单里看到的是小欧的**剪影**（键帽 + 天线；眼/嘴是内部元素，模板化后不可见） |
| `CFBundleShortVersionString` / `CFBundleVersion` | 手写字面值 | **不**由 `Cargo.toml` 自动生成 —— 发版时容易忘记改。维护方式二选一：发版清单加一条「同步 `macos/Info.plist`」，或让构建脚本从 `Cargo.toml` 注入 |

> 以上键名来自 InputMethodKit 的公开约定，**取值没被核对过**（`macos/README.md`
> 「最不确定的 API」#6）。打包完第一件事是拿系统输入法的 plist 做比对。

---

## 4.5 CI 自动打包产物与 Release（未签名/未公证 —— 内测用，需自行签名）

`.github/workflows/apple.yml`（macos-latest，三个 job：`rust` / `macos` / `ios`）每次运行都把
`macos/` 的 Swift 真过一遍编译器（`swiftc -typecheck` → 真编两个架构 → `lipo` → 组装 IME
bundle → ad-hoc 签名 → 只产出一个 `.pkg`；`.app` 是构建中间物，脚本结尾清掉）。
产物挂在**该次 run 的 Artifacts** 里。取法：

```bash
gh run list --workflow apple.yml                  # 找 run id（或网页 Actions 页）
gh run download <run-id> -n "opi-macos-ime-v$(bash scripts/apple/version.sh)"
gh run download <run-id> -n "opi-apple-rust-v$(bash scripts/apple/version.sh)"   # 只要 Rust 库时
```

v1.26.0 起同一 pkg 也作为 `opi-<版本>-macos-universal.pkg` 附到 GitHub Release 与站点 `download/`；仍为 unsigned，需自行签名 / 公证。

| 产物名（版本号 = `Cargo.toml` 的 `[workspace.package] version`，唯一取法在 `scripts/apple/version.sh`） | 内容 |
|---|---|
| `opi-macos-ime-v<版本>` | 只有 `opi-<版本>-macos-universal.pkg` 一个文件（**唯一产物**；2026-10-21 起不再附 `.app`/`.zip`）：内含完整 `OpiInputMethod.app` —— **通用** arm64+x86_64、ad-hoc 签名、`Contents/Frameworks/libopi_ffi.dylib`、`Resources/` 下的符号类目表（22 类真源）/表情/颜文字/贴纸/图标与 `luna`/`trad`/`stroke`/`wubi86` 四部词库；装到 `/Library/Input Methods/`；2026-10-17 起 pkgbuild 带 `--scripts macos/scripts`：**装/升级前自动 killall 旧 IME 进程**，构建期有 `sh -n` 语法门禁；**未在真 macOS 上装过** |
| `opi-apple-rust-v<版本>`（rust job） | `macos/libopi_ffi.dylib` 与 `macos/libopi_ffi.a`（都是**通用**库）· `headers/OpiFFI.h` · `opi_ffi.xcframework`（iOS 用） |

**这次 run 真实证明了什么**（run 37935240583，证据都在 job 日志里，可复核）：

- `macos/` **当时那 3 个** `.swift` 过 typecheck + 真编两个架构 + `lipo` 成通用可执行
  （⚠️ 这是**旧版**的证据；2026-10-20 的 7 文件版本见下一段，已过编译）；
- `codesign --verify --deep --strict` 通过（原话：`valid on disk` /
  `satisfies its Designated Requirement`）；
- app 对 `libopi_ffi` 的依赖名是 `@rpath/libopi_ffi.dylib`，且 app 里带
  `@executable_path/../Frameworks` 的 rpath ⇒ **换台机器**（只要 dylib 在 bundle 里）能解析
  （构建机绝对路径那种「CI 全绿、别人机器上 dyld 崩」的地雷已排除）；
- **真跑过一次**：用 ctypes `dlopen` bundle 里的那份 dylib，并调真出口 ——
  `opi_load(内置回退库) = True / opi_mode() = 0`（在 CI 的 arm64 机器上）；
- `.pkg` 的 payload 真的含 `Library/Input Methods/OpiInputMethod.app` 的可执行文件、
  `Info.plist`、`Contents/Frameworks/libopi_ffi.dylib` 与 `Resources/` 下的符号/表情/
  贴纸/图标/词库（`pkgutil --payload-files` 逐项断言，防「空包也成功」与资源漏拷）。

**2026-10-20 的 7 文件版本也过了编译**（当前树：run `39425977787`，head 739df45）：macOS
job 的 `swiftc macos/*.swift` typecheck + 真编两架构 + 链接 + ad-hoc 签名打包全绿；此前
run `39420786257`（2945b31）红过一次，16bbd2a 修的是 Swift 协议一致性问题（`39421294529`
已绿），之后 About 外链等改动又由 `39425977787` 编过。⚠️ 这里证明的仍只是**编译/打包**，
真机安装与运行依旧 0 验证。

**怎么用**：

```bash
# 装法（系统级；未签名 ⇒ installer 会报未签名，需 -allowUntrusted）
sudo installer -pkg opi-<版本>-macos-universal.pkg -target / -allowUntrusted
# 用户级安装不在 CI 产物里：需自己构建 .app，再照 §5 复制到 ~/Library/Input Methods/
```

装完照 §5 走：`killall OpiInputMethod` → 注销重新登录 → 系统设置里添加。
⚠️ 上面**装法本身的细节（`-allowUntrusted` 的确切行为）未在 Mac 上核过** —— CI 只做到
「打包 + 签名校验 + 库能 dlopen」，**没有**装进系统这一步（runner 也不适合当这种验证点）。

**它不能做什么（现状，别读成「降级」）**：

- **未签名/未公证**：Gatekeeper 会拦；发给别人机器上很可能直接显示「已损坏」。
  正式发布要 Developer ID 证书 + `notarytool` 公证 + `stapler` 装订 —— 三件都**不在**
  本工作流范围内（仓库没有证书、不连签名服务）。
- **运行时 0 验证**：系统输入法列表里出不出现、按键进不进引擎、preedit 显不显示、
  候选窗对不对 —— 全都没测（§6 一步没走）。**编译过 ≠ 能用。**
- **2026-10-20 这批（7 文件）编译已过（run 39425977787），但没有上机**：符号面板、
  输入法菜单、设置窗、`UserDefaults` 持久化与词库重放全是静态改动。打包脚本的
  `bash -n`/`sh -n`、500 行门禁与 `symbol_categories.tsv` payload 断言**只证明脚本语法
  与文件存在**，不证明运行行为。
- `InputMethodServerControllerClass` 定成**裸名** `OpiInputController`：依据是源码里的
  `@objc(OpiInputController)`（它把 ObjC 运行时名逐字定成裸名）。**运行期未验证** ——
  装上后若「能选、但完全无响应」，先按 §4 的表查这个键。
- 版本号取自 `Cargo.toml`；`macos/Info.plist` 里记的版本**只对账、不改写**
  （漂了就漂着，由发版清单去同步 —— 见 §4 的表）。

想在本机复现（不用 CI）：Mac 上先 `rustup target add` 两个 darwin 目标、两条
`cargo build`（前置条件写在脚本头部），然后
`bash scripts/apple/package-macos.sh "$(bash scripts/apple/version.sh)"`。

⚠️ **自己写链接行时的一个坑**：cargo 产物目录里 `.a` 与 `.dylib` **并存**（`crate-type` 含
`cdylib`），`-L … -lopi_ffi` 会优先挑 **`.dylib`**；而 cargo 那份 `.dylib` 的 install name
常是**构建机上的绝对路径** —— 链接会成功、CI 全绿，装到别人机器上却是
`dyld: Library not loaded: /Users/...`。本工作流把库改成 `@rpath/libopi_ffi.dylib`
并断言 app 的依赖名就是 `@rpath/…`（见上面的证据清单），自己写链接行时**同样要改**
`install_name_tool -id @rpath/libopi_ffi.dylib`，并让 app 带上
`@executable_path/../Frameworks` 的 rpath。

---

## 5. 安装与启用

> CI 产物只有 `.pkg`（§4.5）；下面用户级安装用的 `OpiInputMethod.app` 指你自己按
> §3/§4 构建出来的那一个。
>
> 以下步骤**除了 `~/Library/Input Methods/` 这个路径**（Apple 的 TIS 文档明确：
> 输入法包放 `~/Library/Input Methods/` 或 `/Library/Input Methods/`）**其余都是凭记忆写的，
> 请在 Mac 上逐条核对** —— macOS 各版本「系统设置」的改名/改路径很频繁。

```bash
# 1) 装（用户级，不需要 sudo）
mkdir -p ~/Library/Input\ Methods
cp -R OpiInputMethod.app ~/Library/Input\ Methods/

# 2) 若旧版本正在跑，先杀掉（否则系统仍在用旧二进制；改完必须重来这一步）
#    ⚠️ 只有**手工 cp 这条用户级安装路**要手做（CI 只发 .pkg）：pkg 安装路自 2026-10-17 起由
#    macos/scripts/preinstall 自动完成（pkgbuild --scripts 随包编入；装/升级前
#    killall 旧 IME 进程 —— 与 Linux 的 deb/rpm 维护脚本、Windows MSI 的 taskkill
#    同一条语义）。杀掉之后由 imklaunchagent 按需拉起，下一次输入就是新版。
killall OpiInputMethod 2>/dev/null || true

# 3) 词库（可选；不放则用内置回退库，候选质量会明显下降）
mkdir -p ~/Library/Application\ Support/opi
cp luna.opid ~/Library/Application\ Support/opi/luna.opid
```

4. 打开 **系统设置 → 键盘 → 文本输入 → 输入法 → 编辑… → +**，在「中文（简体）」
   或「其他」下找到 **小欧输入法** 加入（也可勾选「在菜单栏中显示输入法菜单」）。
5. 切到 小欧输入法，在文本编辑器里敲 `h a o 空格`。

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
2. **系统设置里能添加并切到「小欧输入法」** ⇒ `Info.plist` 的键对了、控制器类实例化得出来。
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
5. 只有 1–4 都过之后，才验收已有功能（候选窗、菜单/热键、符号面板、设置与词库）。

### 6.5 设置窗「高级」页：自定义短语（2026-10-24，源码层）

设置窗新增 `高级` 分类（`OpiSettingsWindow.swift` 的 `OpiSettingsTab.advanced`，
内容由 `OpiSettingsPhrases.swift` 的 `buildAdvanced()` 建）。页内一个 `中文 | 英文`
下拉（`phraseKind`）切换两套**完全独立**的短语：各自开关、表、文件 ——
`~/Library/Application Support/opi/custom_phrases.json` / `custom_phrases_en.json`
（`OpiCustomPhraseStore.fileURL`；与 `luna.opid` 同目录）。宿主文件 schema =
引擎 JSON + `"enabled"`，宿主文件是唯一真源（引擎只持内存）。**开关缺省关
（2026-10-25 用户裁决）**：加短语后必须显式开启；每次开关/增删改都先写盘再推引擎，
立即生效，不需要重启或重新导入。

- 10 条/页、显示总数、增删改；导入 JSON / TSV / CSV / `字母=短语` / `字母 空白 短语`，
  **任何一条非法 ⇒ 整份拒收**；导出 JSON / TSV / CSV / `字母=短语`（比 Android/Linux
  多两种，见 `OpiCustomPhraseCodec` 的 `tsv` / `csv` / `lines`）；读文件上限 1 MiB。
  上限 **2000 条**、trigger **≤32 个 ASCII 字母**。
- 装载/推送：`main.swift` 启动时 `loadAndPush(.chinese)` / `loadAndPush(.english)`；
  打开「高级」页也 `loadAndPush` 一次；每次增删改/导入先写盘再推引擎
  （`OpiCustomPhraseStore.save` → `push`）。
- 匹配：中文套仅 `Mode::Pinyin`/`Traditional`（方案 Full/Nine/Fourteen；macOS 只有
  Full），英文套仅 `Mode::English`；精确整串、英文大小写不敏感；命中项前插候选。
  macOS 的按键**总是**先过引擎（`InputController` 的 `engine.keyEvent`），没有 fcitx5/
  TSF 那条「普通英文直出」分支 ⇒ 英文短语在普通英文态即可匹配，不需要 pro（macOS
  也没有 ENPRO 控制面，见 G7）。
- **未上机**：设置窗布局、文件路径、导入/导出、引擎推送都只在源码层成立。

---

## 7. 已知限制（诚实清单）

**这些不是 bug，是「还没做」或有意的取舍** —— 交付时请如实转述，
别当缺陷重报，也别当没看见。

| # | 限制 | 说明 |
|---|---|---|
| G2 | **模式入口：已补（2026-10-20，CI 已编译、未上机）** | `OpiMenu.swift` 的输入法菜单提供 英文模式 / 繁体模式 / 中文标点 / 全角 / 输入方案（全拼·双拼）/ 符号面板 / 设置；热键 `Ctrl+'`、`Ctrl+\`、`Shift+Space`、`Ctrl+Shift+F`（简繁）、`Ctrl+Shift+Z`（符号面板）。**仍未接**三态循环（iOS 的 `cycleMode()`）、数字/符号模式按钮、ENPRO（fcitx5 控制面专有，C ABI 无出口）。形态差异见 G7 |
| G3 | **候选分页：已补（2026-10-20，CI 已编译、未上机）** | `refresh()` 改喂 `opi_candidates_page()`，`candidateSelected` 调 `opi_select_page(k)`（页内索引）；不再用 `opi_candidates(limit: 64)` / `opi_select(index)`，也不再自己算 `page*PAGE_SIZE+k`。**未做**候选窗里的「第 N / M 页」文字 —— `IMKCandidates` 没有页标签控制面 |
| G4 | **无测试、无工程文件；本批 CI 已编译** | `macos/` 里跑过的只有头文件（`clang -fsyntax-only` + 「导出名取地址」那个 TU）；旧版 3 个 Swift 由 CI 在 2026-10-15 前编过。**2026-10-20 新增/改动的 7 个文件已由 CI 编过**（run 39425977787），本机只跑了 `bash -n`、`sh -n` 与 500 行门禁。下一步可执行验证是「装进系统跑最小冒烟」 |
| G5 | **宠物「小欧」未接（有意）** | 候选窗用的是系统面板 `IMKCandidates`，**画不进去**。要摆小欧得先换成自绘 `NSPanel` —— 那是对未上机验证的代码做结构改动。真要接时**别**新写一份 CoreGraphics 几何：几何真源是 `docs/opi-pet.svg`，同一张图的第三份拷贝必然漂移 |
| G6 | **符号面板：已补（2026-10-20，CI 已编译、未上机）** | `OpiSymbolPanel.swift`：左侧 22 类竖条、右侧 12×6=72/页网格、搜索框、翻页；数据是随包的 `symbol_categories.tsv`（打包脚本拷进 `Resources/`，`.pkg` payload 有存在性门禁），读不到才回退引擎 `opi_symbol_blocks()` 的 6 个 common 块并打日志。字形过滤用 CoreText 单码位判定；上屏直接 `insertText`，面板保持打开。**未接**表情页（引擎有 `opi_emoji_*` 出口，没有 UI）、贴纸/剪贴板 |
| G7 | **「状态栏」是菜单，不是常驻条** | InputMethodKit 没有 Linux 那种悬浮状态栏；`OpiMenu.swift` 用输入法菜单承载同组入口。皮肤与虚拟键盘已在 OPI 自绘面板落地（11 纯色 + 19 动态，2026-10-31）；LOGO 与 ENPRO 仍没有等价物 |
| L1 | **全角状态只在菜单里读** | `OpiMenu` 的「全角字符」勾选读 `opi_fullwidth_state()`（2026-10-20），但**没有常驻状态栏**可显示。⚠️ 它**不是**在报错：「未装载」与「已装载且半角」共用 `false`，那时按键全部交系统，宿主拿到的本来就是半角 |
| L2 | **⇧ 长按 → Lock 不可达** | `NSEvent` 没有长按信号（同 fcitx5 轨的已知边界）。本层**不合成** `LONG_PRESSED` 位，所以 ⇧ 长按 = 单击。要接得自己定入口，别靠猜补一个信号 |
| L3 | **`keyUp` 大概率收不到** | `recognizedEvents:` 属于 `IMKStateSetting`，默认只返回 `NSKeyDownMask`；公开报告说即使加上 `NSKeyUpMask`，`NSKeyUp` **仍然**不会被投递给 `handleEvent:client:`（被指为 IMKit 已知问题）。**故本轮不动** `recognizedEvents`。影响：`RELEASED` 位收不到时，退格/回车的「按下放行、抬起也放行」对称性不成立，在依赖键状态的控件里可能表现为卡键 —— ⚠️ **但这个影响的前提本身也没被验证**（取决于 IMK 是否替我们转发了抬起）。代码里那个 `.keyUp` 分支很可能是死代码 |
| L4 | **autorepeat 认领但不执行** | 按住 `Shift+Space` 稍久，`NSEvent` 会发重复 keyDown；让它们执行会**反复切全角**，而用户的语义是「一次切换」。也**不能**在判定函数里返回「不是热键」—— 那一颗键会漏给 `keyEvent`，而 `Shift+Space` 的 SHIFT 不在直通掩码里 ⇒ 被当成**普通空格 = 选首候选**，比反复切更坏。所以判定函数故意不过滤，由调用处 `!event.isARepeat` 决定 |
| L5 | **⌘ 必须如实置 `META` 位** | 由 Rust 侧 `router.rs` 的直通掩码（`CTRL\|ALT\|META`）放行 —— 本层**故意没有**再拦一道（那会把规则抄成第二份）。⚠️ 这条依赖要跟着 Rust 侧复核：掩码一旦去掉 META，⌘A 会被当成普通 `'a'` 吃进缓冲 |

---

## 8. 未验证清单（照抄现状，别读成已完成）

- `macos/` 下 **7 个 `.swift`**（`OpiEngine` / `InputController` / `OpiMenu` /
  `OpiSymbolPanel` / `OpiSettings` / `OpiSettingsWindow` / `main`）：
  没有 lint、没有单测。**旧版 3 个**在 2026-10-09 由 CI 编过（`apple.yml`，
  run 37935240583：typecheck + 真编 + 链接 + ad-hoc 签名，见 §4.5）；
  **2026-10-20 的 7 文件版本已由 run 39425977787 编过；仍未上机。**
  **运行时（装进系统之后）验证数依旧是 0。**
- **2026-10-24 订正文件面**：已不止 7 个 —— 新增 `OpiCustomPhrases.swift` /
  `OpiSettingsPhrases.swift`，`OpiSettingsWindow.swift` / `main.swift` 也有改动。
  这批改动还在工作区、**没有任何编译记录**，所以 §6.5 的自定义短语页连「编过」都
  谈不上；运行时仍是 0。
- `macos/Info.plist`：**没有被任何 macOS 构建系统读过**，也没有装进过任何系统的输入法列表。
- `macos/OpiFFI.h`：过了 `clang -fsyntax-only`（C 与 C++ 两种模式，`-Werror` 下 exit 0），
  也过了「把 `cabi.rs` 的每个导出名**取地址**」那个 TU。**2026-10-09 起它被链接过了**
  （CI 的 swiftc 真编拿它当桥接头并链上 Rust dylib，run 37935240583）。
  但「链接过」的分辨力有限：只有**被 Swift 代码实际调到**的声明过了 typecheck
  （参数/返回类型对得上了），没被调到的声明仍只有语法检查；而那个取地址 TU
  对**签名/类型是瞎的**（`&f` → `const void *` 对任何函数类型都成立）。
- **「最不确定的 API 清单」共 14 条**，逐条都标了不确定点（在旧 10 条之外，
  2026-10-20 新增：输入法菜单与 `doCommand` 分派、`LSBackgroundOnly` 进程里的
  `NSPanel`、随包资源与 `UserDefaults`）。旧 10 条是：提交文本、`setMarkedText` 签名、
  `IMKCandidates` 构造与常量名、`candidateSelected` 怎么拿客户端、启动与 run loop、
  `Info.plist` 键、`NSEvent.keyCode` 数值表、`charactersIgnoringModifiers` 语义、
  修饰位与 keyUp、词库路径。接手人从这里开始最省时间。
- **设置持久化与词库重放**（`UserDefaults` 键、`dicts/*.tsv` 启动重放、导入复制/
  删除语义）只在源码里成立，**没有在 macOS 上跑过一次**；`~/.config` 与
  `~/Library/Application Support` 的路径约定同样未核对。
- `Info.plist` 的 `tsInputMethodCharacterRepertoireKey` 取值、`tsInputMethodIconFileKey`
  指向的文件 —— **未核对**；pkg 随包四部词库与 `~/Library/Application Support/opi/<name>.opid` 的覆盖顺序已在源码核对，真机仍未运行。

**做完这些测试，上述若干条会被推翻 —— 推翻时就地改，别让「记忆级」在文档里冒充「已验证」。**

---

## 9. 下一步顺序（别跳）

```
1. （已完成）编译链接通过（7 个文件，run 39425977787）—— 下一步从 2 开始
2. 最小冒烟：装进 ~/Library/Input Methods/、添加输入源、h a o 空格 →「好」
3. 开一次输入法菜单与符号面板（先验 `LSBackgroundOnly` 下 NSPanel 能否出现）
4. 设置窗改一个开关、重启看是否生效；导入一个词库看是否重放
5. 逐个消掉「最不确定的 API 清单」那 14 条
6. 只有 1–5 过了，才谈表情/皮肤/宠物
```

**顺序不能反。** 在编译链接通过之前做的任何功能工作，都是在给一份可能整体重写的代码上加东西。
更细的依据、每一项的证据与订正记录在 `macos/README.md`。
