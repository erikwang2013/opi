# OPI macOS 端（草案）

> ## ⚠️ 草案 · 从未编译 · 未验证
>
> **本目录的 Swift 代码一行都没有编译过。** 没有 macOS、没有 Xcode、没有 Apple SDK；
> InputMethodKit / AppKit 是 Apple 独有框架，在 Linux 上 `import` 就断 —— 连语法检查
> 都做不到。Info.plist 没有被任何 macOS 构建系统读过，也没有装进过任何系统的输入法列表。
>
> 所以本目录的交付物**不是「可用的 macOS 输入法」，而是「一份起点 + 一份精确契约」**。
> 不要在任何地方把它写成「完成」「已实现」—— 本项目的 README 刚花了一整轮
> （v1.0.12）把「写成现状的愿景」降级，不要再制造一个。
>
> 这个项目已经被「没被编译器看过的代码」坑过两次，本目录的存在正是为了避免第三次：
>
> | 前两次 | 代价 |
> |---|---|
> | fcitx5 C++ 插件 | 写在仓库里、README 记着「待验收」，一编译发现 **7 处 API 误写**（连 `fcitx::InputMethod` 这个类都不存在） |
> | Windows TSF | README 记着「完成，待在 Windows 上验收」，实测 `DllGetClassObject` 恒返 `CLASS_E_CLASSNOTAVAILABLE`、无 CLSID、**文本插入从未实现** → 整个端不可用 |

## 交付物

| 文件 | 内容 |
|---|---|
| `OpiFFI.h` | C ABI 声明面（**28 个导出** = 27 个 `opi_*` 函数 + `opi_ffi_free_string`），逐条对照 `crates/opi-ffi/src/cabi.rs` 抄写；`ios/OpiFFI.h` 是指向本文件的转发头 |
| `OpiEngine.swift` | C ABI 桥：`OpiString` 的持有/释放、键码/状态位常量、词库装载与回退 |
| `InputController.swift` | `IMKInputController` 子类：NSEvent → `opi_key_event` 入参、提交文本、preedit、候选窗 |
| `main.swift` | `IMKServer` 启动（无 nib） |
| `Info.plist` | 输入法组件声明 |
| `README.md` | 本文件 |

**没有 Xcode 工程文件（`.xcodeproj`）是有意的**：手写一个从未被 Xcode 打开过的
`project.pbxproj` 会是第三件「没被工具看过的产物」，而且它比 Swift 更难人工核对。
工程请在 Mac 上由 Xcode 新建，然后把本目录的 4 个源文件与 Info.plist 加进去。

## 已验证 / 未验证

**已验证（本机 Linux，可复现）**：

```bash
$ rustup target add aarch64-apple-darwin x86_64-apple-darwin
$ cargo check -p opi_ffi --target aarch64-apple-darwin
    Checking engine-core v1.0.13 (/home/wwwroot/bag/opi/crates/engine-core)
    Checking engine-data v1.0.13 (/home/wwwroot/bag/opi/crates/engine-data)
    Checking opi_ffi v1.0.13 (/home/wwwroot/bag/opi/crates/opi-ffi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.00s   # exit 0

$ cargo check -p opi_ffi --target x86_64-apple-darwin      # 同样 exit 0

# 更强的一条：真的产出了 macOS 归档（staticlib 不需要链接器，所以在 Linux 上也能出）
$ cargo rustc --release -p opi_ffi --target aarch64-apple-darwin \
      --crate-type staticlib --lib
    Finished `release` profile [optimized] target(s) in 20.21s              # exit 0
$ file target/aarch64-apple-darwin/release/libopi_ffi.a
    current ar archive
$ grep -a -o '_opi_[a-z_]*' …/libopi_ffi.a | sort -u | wc -l
    28        # 与 OpiFFI.h 的声明数一致（Mach-O 的 C 符号带前导下划线）

# 不止数个数：与 cabi.rs 的导出名逐个 diff（空 diff = 集合完全一致）
$ grep -A1 'unsafe(no_mangle)' crates/opi-ffi/src/cabi.rs | grep -o 'fn opi_[a-z_]*' \
      | sed 's/fn /_/' | sort -u > /tmp/rs.txt
$ grep -a -o '_opi_[a-z_]*' …/libopi_ffi.a | sort -u > /tmp/a.txt
$ diff /tmp/rs.txt /tmp/a.txt        # 无输出 → 28 个符号两边一致

# 头文件本身的语法检查（本目录唯一能在本机真跑的一步，clang 与 Xcode 同族）
$ clang -std=c11 -Wall -Wextra -Wpedantic -fsyntax-only -x c OpiFFI.h
    OpiFFI.h:24:9: warning: #pragma once in main file     # 直接编头文件的必然告警
    # exit 0，无其它诊断（C++ 模式 g++ -x c++ 同样 exit 0，走的是 extern "C" 分支）
$ grep -A1 'unsafe(no_mangle)' crates/opi-ffi/src/cabi.rs | grep -o 'fn opi_[a-z_]*' \
      | sed 's/fn //' | awk '{printf "    (const void *)&%s,\n", $0}' > /tmp/refs.h
# 拼成一个小 TU（含 #include "OpiFFI.h" + 上面 28 行取地址表）后再编 —— 名字少一个声明就断
$ clang -std=c11 -Wall -Wextra -Imacos -fsyntax-only -x c /tmp/check_opi.c
    # exit 0 → 28 个导出名在头文件里全部有声明（这个检查抓到过真问题，见下）
```

> ⚠️ 上面输出里的 `v1.0.13` 是**跑这些命令时**的工作区版本，不是当前版本
> （仓库已到 v1.0.15）。**记录照原样保留** —— 改成本轮的版本号就是把一份实测
> 记录改成没跑过的样子。要今天的新数字就重跑一遍。

**符号数是怎么数出来的（方法本身也要交代）**：本机的 GNU `nm` / `objdump` **不认
Mach-O**（`nm: __.SYMDEF: file format not recognized`），所以上面用的是 `grep -a`
读归档里的符号名字符串 —— 它证的是「归档里存在这些名字」，**不是**「链接器能解析
它们」。在 Mac 上要拿真正的符号表，用 `nm -gU libopi_ffi.a` 或
`xcrun nm`。
注意 `strings -a … | grep -o '^_opi_[a-z_]*$'` 与 `grep -a -o '_opi_[a-z_]*'` 两种
写法都数出 **28**（前者靠 `strings` 换行、后者不锚定，两者一致才算数）。

**上面那条「取地址」检查是有价值的、不是形式**：它当场抓到 `opi_select_page` 已在
`cabi.rs` 落地、而头文件还没声明（`error: use of undeclared identifier
'opi_select_page'`）—— 也就是本目录最容易犯的「头文件落后 Rust 侧」。修完 28/28 通过。
但它只证「名字都在、声明自洽」，**不**比对类型是否与 Rust 一致（那要等 Mac 上链一次）。

这两条证明的是 **C ABI 能为 macOS 目标编译，并真能 codegen + 归档出一个含全部 28 个
`opi_*` 符号的 `.a`；且本头文件自身能被 clang 解析**。
它们**不**证明：这个 `.a` 能在 macOS 上被 ld 链接、能被 IMK 加载、Swift 侧任何一行是对的。

**已知的失败（环境，不是代码）**：整体 `cargo build -p opi_ffi --target aarch64-apple-darwin`
在本机**失败**，因为 `crate-type` 含 `cdylib` —— 链接动态库需要 Apple 的链接器，本机
Rust 调的是 Linux 的 `cc`：

```
error: linking with `cc` failed: exit status: 1
  = note: cc: error: unrecognized command-line option '-arch'
          cc: error: unrecognized command-line option '-mmacosx-version-min=11.0.0'
```

判据：错在**链接**这一步、且是 `cc` 不认 Apple 参数 → 换台装了 Xcode 的 Mac 就好。
`staticlib` 那条路径不需要链接器，因此本机可产出（上面的 `.a` 就是它）。

这两条**已经进 CI 了**：`.github/workflows/ci.yml` 的 rust job 现在把
`aarch64-apple-darwin` / `x86_64-apple-darwin` 与 `aarch64-apple-ios*` 一起列进
`targets`，并按同一个循环逐个 `cargo check` —— 与既有的 Windows 目标检查同级
（那条的注释解释了原理：「`cargo check` 不链接，所以 Linux 上无需 msvc 链接器即可跑通」）。

**未验证（全部）**：本目录所有 Swift 与 plist。一条都没跑过。
唯一的例外是 `OpiFFI.h` —— 它过了 clang 的语法检查（见上），但**也从未被链接过**：
「clang 说这堆声明自洽」与「ld 能把它们接到 Rust 的符号上」是两件事。

## 在 Mac 上第一件要做的事

**顺序不能反：先让它编译链接通过，再谈功能。** 具体地：

1. **先跑通 `cargo check` 的三个目标**（确认这台机器与 CI 看到的是同一个东西）：
   ```bash
   rustup target add aarch64-apple-darwin x86_64-apple-darwin
   cargo check -p opi_ffi --target aarch64-apple-darwin
   cargo check --tests -p opi_ffi          # 宿主上的路由单测也要绿
   ```
   注：`cabi.rs` 由别的轨在持续加导出，某一条**可能暂时红**。判据很简单 ——
   错误若是 `unresolved import opi_ffi::cabi::opi_xxx`，那是「测试先写、导出后落地」
   的半成品状态（本目录交付时就撞上过两次：先是 `opi_key_event`，后是
   `opi_page`/`opi_shift_state`），不是你的环境问题；`--target aarch64-apple-darwin`
   那条不含测试目标，不受影响。
2. **编静态库**（`staticlib` **已经加进** `crates/opi-ffi/Cargo.toml` 的 `crate-type`，
   不需要你再改 Cargo 配置，直接编即可 —— 见「构建集成」）。
3. **建 Xcode 工程，把 4 个源文件与 Info.plist 加进去，只求 `xcodebuild` 通过** ——
   **一行逻辑都不要改**。这一步会一次性暴露本目录里所有「凭记忆写的」API 用法
   （见下方「最不确定的 API」）。把编译错误当成这份草案的**待办清单**，而不是当成本目录
   作者写错了什么：那个清单的长度本身就是本目录可信度的度量。
4. **跑最小冒烟**：`~/Library/Input Methods/` 装上、系统设置里添加、打开文本编辑器
   敲 `h` `a` `o` `空格` —— 期望提交「好」。这条**逐字对齐 fcitx5 轨的实测表**
   （`crates/fcitx5-opi/cpp/README.md`「状态」一节：`h`/`a`/`o`/`SPACE` 的 action 是
   1/1/1/2，提交 `好`）。**同一个引擎、同一套语义，两端的最小可见成功必须一致**；
   不一致就是桥接层的问题，不是引擎的。
5. 只有 4 通过之后，才谈「补功能」（候选窗美化、模式切换入口、用户词管理 UI 等）。
   在第 3 步之前做的任何功能工作，都是在给一份可能整体重写的代码上加东西。

## 构建集成

### 1. Rust 静态库（Cargo 配置已就绪）

`crates/opi-ffi/Cargo.toml` 里 `staticlib` 已经在 `crate-type` 里了
（`["cdylib", "staticlib", "lib"]`，本目录交付后补的），不需要再改 Cargo 配置。
直接编通用二进制（两种 Mac 都能跑）：

```bash
cargo build --release -p opi_ffi --target aarch64-apple-darwin
cargo build --release -p opi_ffi --target x86_64-apple-darwin
lipo -create \
  target/aarch64-apple-darwin/release/libopi_ffi.a \
  target/x86_64-apple-darwin/release/libopi_ffi.a \
  -output build/libopi_ffi.a
```

⚠️ 未验证：链接期是否需要额外系统库。`opi_ffi` 依赖 `jni` crate（为 JNI 出口），
它在 macOS 目标上能编译（上面 `cargo check` 过了），且 `JNI_OnLoad` 是**导出**符号
而非导入，按此推断链接期不需要 libjvm —— 但这是推断，不是实测。

替代方案（不想改 `crate-type`）：直接用现成的 `cdylib`，把 `libopi_ffi.dylib` 放进
`OpiInputMethod.app/Contents/Frameworks/`，并把 rpath 设成
`@executable_path/../Frameworks`。fcitx5 轨就是这么做的（`$ORIGIN` rpath），
代价是多一个运行时依赖。

### 2. Swift 侧导入 C 符号

Swift 看不见裸 C 符号，必须经一个头文件。两种接法，任选：

- **Xcode**：`SWIFT_OBJC_BRIDGING_HEADER = macos/OpiFFI.h`
- **SwiftPM**：把 `OpiFFI.h` + `module.modulemap` 放进一个 C target，Swift target 依赖它

声明只在 `OpiFFI.h` 一份 —— 不要在 Swift 里再抄一遍 `@_silgen_name` 之类的第二份声明。

### 3. 工程设置

| 设置 | 值 |
|---|---|
| 链接库 | `libopi_ffi.a` |
| 链接框架 | `InputMethodKit`、`Cocoa`（`-framework InputMethodKit -framework Cocoa`） |
| `SWIFT_OBJC_BRIDGING_HEADER` | `macos/OpiFFI.h` |
| `PRODUCT_BUNDLE_IDENTIFIER` | 例如 `io.opi.inputmethod`（要进 Info.plist 的 `CFBundleIdentifier`） |
| 产物 | `OpiInputMethod.app`（**必须**是 .app，`main.swift` 靠 `Bundle.main` 取连接名与 bundle id） |

## 安装与启用

> 以下步骤**除了 `~/Library/Input Methods/` 这个路径**（Apple 的 TIS 文档明确：
> 输入法包放 `~/Library/Input Methods/` 或 `/Library/Input Methods/`）**其余都是凭记忆写的，
> 请在 Mac 上逐条核对**。macOS 各版本的「系统设置」面板改名/改路径很频繁。

```bash
# 1) 装（用户级，不需要 sudo）
mkdir -p ~/Library/Input\ Methods
cp -R OpiInputMethod.app ~/Library/Input\ Methods/

# 2) 若旧版本正在跑，先杀掉（否则系统仍在用旧二进制；改完必须重来这一步）
killall OpiInputMethod 2>/dev/null || true

# 3) 词库（可选；不放则用内置 35 词回退库，候选质量会明显下降）
mkdir -p ~/Library/Application\ Support/opi
cp luna.opid ~/Library/Application\ Support/opi/luna.opid
```

4. 打开 **系统设置 → 键盘 → 文本输入 → 输入法 → 编辑… → +**，在「中文（简体）」
   或「其他」下找到 **OPI 拼音** 加入（也可勾选「在菜单栏中显示输入法菜单」）。
5. 切到 OPI 拼音，在文本编辑器里敲 `h a o 空格`。
6. **看不到输入法时**：`killall OpiInputMethod` + 注销重新登录（最可靠）；
   仍看不到就检查 `CFBundleIdentifier` 是否唯一、`Info.plist` 里的
   `InputMethodConnectionName` 是否与 `main.swift` 里的字符串逐字一致。
7. **日志**：`NSLog` 输出在 Console.app 里按进程名 `OpiInputMethod` 过滤
   （本目录的装载失败告警都走 NSLog）。

## 最不确定的 API 清单

按「错了会怎样」排序。**这一节是本目录最有价值的部分** —— 它把「凭记忆写的、
Mac 上很可能要改」的地方标出来了，接手人从这里开始最省时间。

### 1. 文本到底怎么提交进应用 —— `client.insertText(_:replacementRange:)`

`InputController.swift` 的 `commit(_:to:)`。这是 IMK 侧对应 fcitx5
`ic->commitString(s)` 的那一步，也是**整条链路里最不确定的一处**。

- 现在的写法：`sender as? IMKTextInput` → 先 `setMarkedText("")` 清 preedit →
  `insertText(text, replacementRange: NSRange(location: NSNotFound, length: NSNotFound))`。
- **没被验证的**：① 客户端对象就是 `handle(_:client:)` 的 `sender`；② 「先清 marked
  text 再 insert」的顺序；③ `replacementRange` 用 `NSNotFound` 表示「插在光标处、不替换
  任何范围」（Apple 论坛示例代码用的是 `NSRange(location: NSNotFound, length: 0)`，
  与本目录的 `length: NSNotFound` **不一致** —— 这处差异必须实测）。
- 若提交没反应，按顺序试：`NSRange(location: NSNotFound, length: 0)` →
  用 `markedRange()` 作为 replacementRange（替换掉 preedit）→ 检查是否必须先
  `setMarkedText("")`。
- 参考：`crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 的 `case 2:` 分支（那边
  `ic->commitString(...)` 已被真实 fcitx5 验证过语义，但 API 是 fcitx5 的）。
- **如果连 `handle` 都没被调用**（按键完全没反应、Console 里没有任何日志），那不是
  提交的问题而是覆写没生效：`IMKInputController` 的选择器是 `handleEvent:client:`，
  Swift 侧的名字可能是 `handle(_:client:)` 也可能是 `handleEvent(_:client:)`。
  用 `@objc(handleEvent:client:)` 显式钉住即可（`InputController.swift` 的事件入口
  处有同样的注释）。**这是「按键进不来」的两个首要嫌疑之一**，另一个是连接名
  （见 #5）。

### 2. preedit（marked text）—— `setMarkedText(_:selectionRange:replacementRange:)`

同一个函数里的另一个调用。不确定：签名（`selectionRange` / `replacementRange` 的
类型与语义）、`selectionRange` 是否该是「缓冲长度, 0」（把光标放末尾）、空串是否等于
「取消 preedit」。**症状判断**：如果 preedit 不显示但提交正常，问题在这里；如果提交也
不正常，问题在 #1。

### 3. 候选窗 —— `IMKCandidates`

- `IMKCandidates(server:panelType:styleType:)` 的 Swift 签名、以及
  `kIMKSingleRowSteppingCandidatePanel` / `kIMKMain` / `kIMKLocateCandidatesBelowHint`
  这些常量名（它们是老式 C 枚举，Swift 里**保留 `k` 前缀**；`IMKCandidates` 类名本身
  则不带前缀）。
- **定位**：本目录把候选窗做成单例、用启动时的 `IMKServer` 建（`main.swift`），
  而不是控制器自带。**风险**：文档说候选窗随光标定位是框架经控制器拿插入点矩形做的，
  控制器与窗口不同源时定位可能失效（候选窗出现在屏幕角落）。
  备选方案：`IMKInputController` 若有候选窗访问器（`candidateController` 之类）就用它。
- **`setCandidateData(_:)` 有一条「调用了但窗口不出来、抛
  `CandidateWindowNoCandidatesException`」的公开报告**，报告者改用
  `IMKInputController` 的 `candidates(_:)` 委托方法才成功。本目录两条路都写了
  （`update()` 里 `setCandidateData` + 控制器 `candidates(_:)` 覆写），任一条生效即可；
  若出现「窗口闪一下就消失」或异常，删掉 `setCandidateData` 那条。
  ⚠️ 覆写 `candidates(_:)` 的**形参类型必须是 `Any!`**（IMK 的声明是
  `- (NSArray *)candidates:(id)sender`）。写成 `IMKCandidates!` 是**另一个选择器**，
  `override` 不成立 —— 本目录曾这么写，本轮已按公开签名订正。症状是候选窗永远空
  且**不报错**（委托根本没被调）。
- `candidateSelected(_:)` 是 **IMKInputController** 的方法（不是 IMKCandidates 的），
  候选窗关闭**之后**才回调 —— 这是本目录 `candidateSelected` 里要自己 `hide()` 的原因
  （可能多余，也可能是必须）。
- **重复提交风险**：`setDismissesAutomatically:` 默认 `true`，此时「回车选中候选」由
  **面板**处理并回调 `candidateSelected`；而同一个回车也会进 `handle`（Rust 侧回车 =
  提交首候选）→ 同一个候选被提交两次。本目录在 `configure()` 里设了 `false` 并由本层
  驱动可见性。**若实测出现「回一次车出两个字」，先查这里**；反之若候选窗不再自动关闭，
  也先查这里。

### 4. `candidateSelected` 里怎么拿到客户端

`commit(_:to: client())` —— `IMKInputController` 是否暴露当前客户端（`client()`）
**未经核对**。若没有这个方法：在 `handle(_:client:)` 里把 `sender` 存进字段，
`candidateSelected` 用字段。注意这个字段在 `deactivateServer` 要清掉。

### 5. 启动与 run loop

`main.swift`：`IMKServer(name:bundleIdentifier:)` 的签名（来自一个 Swift 示例，
其中 `name` 取自 `Bundle.main.infoDictionary["InputMethodConnectionName"]`）+
`NSApplication.shared.run()` 在 `LSBackgroundOnly` 下是否正确。
不确定：是否要先 `NSApplication.shared`（或设 `setActivationPolicy(.accessory)`）
再建 server；`run()` 之外是否还需要 `NSRunLoop` 的额外配置。
**症状**：进程活着但按键进不来（连接名/控制器类错），或进程直接退出（run loop 起法错）。

### 6. `Info.plist` 的键

- `InputMethodConnectionName` / `InputMethodServerControllerClass` / `LSBackgroundOnly`
  / `tsInputMethodCharacterRepertoireKey` / `tsInputMethodIconFileKey` —— 键名来自公开
  资料，但**取值**没被核对过。
- `InputMethodServerControllerClass` 用了 `$(PRODUCT_MODULE_NAME).OpiInputController`：
  Xcode 工程模板下 Swift 类必须带模块名前缀，**否则「输入法能选但完全无响应」**。
  非 Xcode 构件系统不展开这个变量，要写字面值。本目录同时在 Swift 侧加了
  `@objc(OpiInputController)` 作为裸名的兜底。
- `tsInputMethodCharacterRepertoireKey`：资料来源说是 ISO 15924 **文字代码**（不是语言
  代码），array 决定它落在「输入源」面板的哪一类。本目录写 `Latn` + `zh-Hans` + `zh-Hant`
  —— 但后两个是 BCP-47 **语言**标签，与「ISO 15924 文字代码」的说法**对不上**
  （真正的文字代码是 `Hans` / `Hant`，不带 `zh-`）。**查不到权威取值表，故保留现状**：
  按猜测去改一个可能本来就正确的字面值，风险大于收益。
  ⚠️ **待 Mac 上核实**：打开 `/System/Library/Input Methods/` 下某个系统中文输入法的
  `Info.plist`，看这个键实际写的是什么，再决定这里的三个值是留是改。
- `tsInputMethodIconFileKey` 指向 `opi.tiff`，**仓库里没有这个文件**。缺文件时系统一般
  用默认图标；要正式发布就得补一个（或在 plist 里去掉这个键）。
- `CFBundleShortVersionString` / `CFBundleVersion` 是**手写的字面值**，**不**由
  `Cargo.toml` 的工作区版本自动生成 —— 发版时容易忘记改，本文件就曾经落后过仓库版本。
  维护方式二选一：发版清单里加一条「同步 `macos/Info.plist`」，或让构建脚本从
  `Cargo.toml` 注入。**本文件不写具体版本号** —— 写死的数字必然会漂（本仓库
  `bb3276c` 立的规矩）。

### 7. `NSEvent.keyCode` 的具体数值

`InputController.swift` 的 `specialKeys` 表：51/117/36/76/48/53/116/121/49/56/60/123-126
是**凭记忆写的** `kVK_*` 值（应核对 `Events.h` 的 `kVK_Delete`/`kVK_ForwardDelete`/
`kVK_Return`/`kVK_ANSI_KeypadEnter`/`kVK_Tab`/`kVK_Escape`/`kVK_PageUp`/`kVK_PageDown`/
`kVK_Space`/`kVK_Shift`/`kVK_RightShift`/`kVK_UpArrow` 等）。
**好消息**：键码错不会静默出错 —— 错的那个键会掉进可打印分支或 `NO_KEYVAL` 兜底
（两者都不产生垃圾字符，见 `OpiEngine.swift` 的 `OpiKeySpace` 注释），表现为「这个键
在输入态下没反应」，很容易在冒烟测试里发现。

### 8. `charactersIgnoringModifiers` 的语义

本目录按「Shift 已生效、Option 不生效、Command 不生效」来用（取
`.unicodeScalars.first` 作为可打印键码）。这与 fcitx5 轨取 xkb keysym 同语义；
引擎侧拼音模式会 `to_ascii_lowercase`，所以 `Shift+a` 给的 `'A'` 不会污染缓冲。
不确定：`Command` 是否真的不参与字符生成（若不参与而 macOS 给了控制符，
`router.rs` 的 META 直通也会兜住，见 #9）。

### 9. 修饰位与 `keyUp`

- `event.modifierFlags.intersection(.deviceIndependentFlagsMask)`：过滤掉
  `.function`/`.numericPad`（方向键、功能键会带上它们）。位名与掩码名来自记忆。
- **⌘（Command）必须如实置 `META` 位**，由 Rust 侧 `router.rs` 的直通掩码
  （`CTRL | ALT | META`）放行 —— 本层**故意没有**再拦一道（那会把规则抄成第二份）。
  ⚠️ 这条依赖要跟着 Rust 侧复核：掩码一旦去掉 META，⌘A 会被当成普通 `'a'` 吃进缓冲。
- `keyUp` **大概率根本收不到**（`InputController.swift:79` 那个 `.keyUp` 分支很可能是死代码）。
  查证（公开资料，**未在 Mac 上实测**）：`recognizedEvents:` 属于 `IMKStateSetting`
  协议，其**默认实现只返回 `NSKeyDownMask`**；而更麻烦的是，公开报告说**即使**
  在 `recognizedEvents:` 里加上 `NSKeyUpMask`，`NSKeyUp` **仍然**不会被投递给
  `handleEvent:client:` —— 被指为 IMKit 的已知问题（Radar 21376535），
  绕法是全局 `CGEventTap`（需要辅助功能授权），代价明显大于收益。
  **所以本轮不动 `recognizedEvents`**：改了也无法在本机验证，还可能引入新故障面。
  影响：`RELEASED` 位收不到时，退格/回车的「按下放行、抬起也放行」对称性不成立
  （`router.rs` 的注释解释了为什么两端必须同判）—— 在依赖键状态的控件里会表现为卡键。
  ⚠️ 但请注意这个影响的**前提也没被验证**（宿主应用是否真的因缺 keyup 而卡键，
  取决于 IMK 是否替我们转发了抬起）。**先按下面的顺序做**：
  1. Mac 上冒烟（`h a o 空格`），确认按键能进来；
  2. 在 `recognizedEvents:` 里**临时**加日志确认默认掩码与 keyUp 是否真被投递；
  3. 只有在第 2 步证明「keyUp 确实收不到」且「缺它确实导致卡键」之后，
     才谈绕法 —— 而且优先考虑删除死代码，而不是加 `CGEventTap`。

### 10. 词库路径

`~/Library/Application Support/opi/luna.opid`（`OpiEngine.defaultDictionaryPath`）。
镜像 fcitx5 轨的 `$XDG_DATA_HOME/opi/luna.opid` 与 Android 的
`EngineLoader.FILE_NAME`，但**这条 macOS 约定未经核对**。

### 11. 长按 ⇧ → Lock 在 macOS 端不可达

`NSEvent` 没有长按信号（同 fcitx5 轨的已知边界：
`crates/fcitx5-opi/cpp/README.md`「已知边界」）。本层**不合成** `LONG_PRESSED`
位（`1<<28`），所以 ⇧ 长按 = 单击。要接的话需要自己定入口（CapsLock 映射、或按住超时），
不要靠猜补一个信号。

## 已知缺口（不是 bug，是还没做）

- **G2 模式切换没有入口**：`opi_switch_mode`（0=Pinyin 1=English 2=Number 3=Symbol
  **4=Traditional**）
  在 `OpiEngine.swift` 里有包装，但 macOS 端没有任何触发 UI（菜单项/热键都没有）。
  Android 有模式按钮、fcitx5 轨没接。**接的时候必须走 `opi_switch_mode`**，
  不要直接打引擎：`router.rs` 的 `switch_mode` 会顺手清前端 ⇧ 三态，绕过它会让
  ⇧ Lock 跨模式残留（英文模式下打出全大写）。
- **G3 候选窗显示的是全局列表，不是 Rust 的当前页**：`InputController.swift` 现在把
  `opi_candidates(limit)`（引擎级全局序，从第 0 条起、**不分页**）整份塞给候选窗；
  而 `router.rs` 内部维护 `page` 与 `PAGE_SIZE = 8`，PageUp/PageDown 改的是它。
  后果：翻页后数字键选中的候选与候选窗高亮的位置**可能不是同一个**。
  这个洞有**两半**：显示侧拿的是全局列表，而 `opi_select(index)` 收的也是**全局**
  索引 —— 面板点击于是要自己算 `page * PAGE_SIZE + k`，`PAGE_SIZE` 的第二份拷贝
  正是从这半边冒出来的。**两半现在都堵上了**：

  | 要什么 | 用哪个出口（已声明） |
  |---|---|
  | 当前页候选（页内序） | `opi_candidates_page()` → JSON |
  | 点第 k 个候选（页内索引） | `opi_select_page(k)` → 权威提交文本 |
  | 页码 / 总页数 | `opi_page()` / `opi_page_count()` |
  | ⇧ 高亮（Single/Lock） | `opi_shift_state()` |

  这五个都已在 `cabi.rs` 落地、在 `OpiFFI.h` 声明、在归档里数得到。
  **但本目录还没有调用点**：按 team-lead 的要求，接调用点排在「Mac 上第一次编译
  通过」之后 —— 现在接只会得到一份更厚的未编译代码。
  接的时候两条禁令（都是 Rust 侧注释的原话）：**不要**拿 `opi_candidates()` 自己按 8
  切（页大小是引擎侧常量，UI 抄一份＝改一次就静默错位）；**不要**算
  `opi_page() * 8 + k` 再喂 `opi_select()`（`opi_select_page` 走的是 `Router::select`
  那份唯一换算，与数字键选词、回车提交同源，绕过它等于留下第三份页大小）。
  ⚠️ 一个曾经会咬人的状态问题已在本轮修掉：`api::select` / `input_space` / `clear`
  原先绕路由直接打引擎、没走 `reset_page_if_buffer_changed`，会出现「选完词
  `opi_page()==1` 而 `opi_page_count()==0`」→ UI 显示**第 2 页 / 共 0 页**
  （`api/mod.rs:247/260/330`，已有断言钉住）。所以接的时候可以信任
  `opi_page()` / `opi_page_count()` 是自洽的。
- **G4 无测试、无 CI、无工程文件**：`macos/` 里**跑过的只有头文件** ——
  `OpiFFI.h` 的 clang 语法检查与「28 个导出名取地址」那个 TU（见「已验证」）。
  **Swift 三个文件的验证数是 0**：没有编译、没有 lint、没有单测。
  第一条针对 Swift 的可执行验证就是「在 Mac 上编译通过」（见「第一件要做的事」#3）。
- **G5 宠物「小欧」未接（有意）**：几何真源是 `docs/opi-pet.svg`；Android 与 Windows
  候选窗共用 `shared/pet/OpiPet.kt` 这一份实现（本项目原则：各端 UI 不共享代码，
  但宠物是一张图）。macOS 侧**故意没画**：候选窗用的是系统面板 `IMKCandidates`
  （见「最不确定的 API 清单」#3，那个 `setCandidateData` 不生效的报告也在那里），
  **画不进去** —— 要摆小欧得先换成自绘 `NSPanel`，那是对未编译代码做结构改动。
  真要接时**别**新写一份 CoreGraphics 几何：同一张图的第三份拷贝必然漂移。
  顺序照旧 —— 先让 Swift 过编译器，再谈接。

## 与 Rust 侧的契约（键码 / 状态位）

键码与状态位的真源是 **`crates/engine-core/src/keys.rs`**（`KEY_*` / `KEY_STATE_*`
常量表）；路由**行为与状态**（`KeyRouter`、`KeyAction`、`PAGE_SIZE`、页码、`switch_mode`
清 ⇧、直通掩码）在 **`crates/engine-core/src/router.rs`**。两者都是 Apple 两平台的主路径。
（这两份文件本轮刚做过拆分：常量从 `router.rs` 移到了 `keys.rs`，查找时别只看一个。）

⚠️ **不要照 `crates/tsf-opi/src/logic.rs` 抄**：TSF 轨与 Apple 轨有四个键的编码不同 ——

| 键 | Apple 轨（`keys.rs`） | TSF 轨（`logic.rs`） |
|---|---|---|
| 空格 | `0x20`（**可打印段**） | `0x1_0020` |
| ⇧ | `0x1_0083` | `0x1_0010` |
| PageUp / PageDown | `0x1_0080` / `0x1_0081` | `0x1_0021` / `0x1_0022` |
| Delete | `0x1_0082` | `0x1_002e` |

空格那行最容易错：写成 `0x1_0020` 会落进 `router.rs` 的 `key_event` 非 ASCII 分支
被当成未知特殊键直通，表现为「拼音打一半按空格不提交候选、只输出一个空格」。

状态位（`states`）：`SHIFT=1<<0`、`CAPS_LOCK=1<<1`、`CTRL=1<<2`、`ALT=1<<3`、
`META=1<<4`、`RELEASED=1<<26`、`REPEAT=1<<27`、`LONG_PRESSED=1<<28`。

模式整数（`opi_mode` / `opi_switch_mode`）有 **五个**：0=Pinyin 1=English 2=Number
3=Symbol **4=Traditional**。少一个 case 的后果不是「不认识」而是「显示与行为不一致」
（UI 按拼音显示、引擎在跑繁体），见 `OpiEngine.swift` 的 `OpiMode` 注释。

## 状态

**草案。一行 Swift 都没有编译过，没有一个字节被 IMK 加载过。**
上面「已验证」一节里的 `cargo check` 是这整个目录里唯一的真实证据，而它证明的是
**Rust 侧**能编译，不是本目录能用。

下一步（按顺序）：在 Mac 上编译链接 → 最小冒烟（`h a o 空格` → 「好」）→ 逐个消掉
「最不确定的 API」清单 → 再谈功能。**顺序不能反。**
