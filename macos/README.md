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
| `OpiFFI.h` | C ABI 声明面（**数量以 `crates/opi-ffi/tests/c_abi_contract.rs` 的门禁为准，别抄数字**），逐条对照 `crates/opi-ffi/src/cabi.rs` 抄写；`ios/OpiFFI.h` 是指向本文件的转发头。曾经落后 3 个（`opi_toggle_fullwidth` / `opi_fullwidth_state` / `opi_toggle_symbol`），**2026-09-27 已补齐**，见「全角 / 符号开关的出口」一节；本文件归 `ffi-contract`，别的分身不改 |
| `OpiEngine.swift` | C ABI 桥：`OpiString` 的持有/释放、键码/状态位常量、词库装载与回退、符号库三个出口（`symbolBlocks` / `symbolsInBlock` / `searchSymbols`） |
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

> ⚠️ 上面输出里的 `28` 是**跑这些命令时**的数字。2026-09-27 复查：`cabi.rs` 到 31 个导出，
> `OpiFFI.h` 仍 28 个 —— 那条 `check_opi.c` 现在**编不过**（3 个名字没有声明），
> 而正是这样它才有用。记录照原样保留。
>
> ⚠️ **同日更晚的订正（2026-09-27）**：`OpiFFI.h` 已补齐到 **31/31**，
> `cargo test -p opi_ffi --test c_abi_contract` = **14 passed / 0 failed**。
> ⇒ 上面那条「编不过」**已不再成立**。**数字从此以那道门禁为准，本文件不再抄具体值**
> —— 理由见 `harmony/README.md` 同日的订正：`cabi.rs` 的导出数走过 19 → 20 → 22 → 28 → 31，
> **改数字只是把下一次留给下一个人**。

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

#### 复验（2026-09-27，v1.0.16）

上面那批输出还是 v1.0.13 的。本轮用**全新 target-dir**（零缓存）重跑了三个目标：

```bash
$ cargo check -p opi_ffi --target aarch64-apple-darwin  --target-dir /tmp/opi_darwin_recheck
    Checking engine-core v1.0.16 ... engine-data v1.0.16 ... opi_ffi v1.0.16 ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.12s      # exit 0
$ cargo check -p opi_ffi --target x86_64-apple-darwin   --target-dir /tmp/opi_darwin_recheck
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.08s       # exit 0
$ cargo check -p opi_ffi --target aarch64-apple-ios     --target-dir /tmp/opi_darwin_recheck
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.46s       # exit 0
```

⚠️ 一个**方法上的坑**（本次踩到）：`rustup target add aarch64-apple-darwin` 会以
`detected conflict: '.../libaddr2line-*.rlib'` 失败，且 `rustup target list --installed`
**不列出**这两个 darwin 目标 —— 但 `cargo check` 照样能过（两个目标的 `.lib` 其实
各 59 个文件都在）。**别拿 `rustup target list` 的空输出当成「目标没装」**。

#### 类型级一致性：`cabi.rs` ↔ `OpiFFI.h`（2026-09-27）—— 当时 28/28 一致，其后一度 31 vs 28，**现已补回一致**

补上了上面那句「不比对类型」的一步。用一个临时脚本比 **名称 + 元数 + 参数类型序列 +
返回类型**（归一化 `bool`/`_Bool`、`usize`/`size_t`、指针空格差异）：

```
$ python3 /tmp/abi_parity.py
Rust 导出 = 28   头文件声明 = 28
名字差集 Rust−头: []
名字差集 头−Rust: []
签名不一致: 0
（exit=0）
```

被验的头文件指纹 `md5sum OpiFFI.h` = `e3b3708547b0192b88281aec23fa4399`，
mtime `2026-09-27 23:26:15`。**该文件的时效只到这个 mtime**。

> ⚠️ **同一天晚些时候复跑，结果变了**（`cabi.rs` 一侧新增了三个导出）：
> ```
> $ python3 /tmp/abi_parity.py
> Rust 导出 = 31   头文件声明 = 28
> 名字差集 Rust−头: ['opi_fullwidth_state', 'opi_toggle_fullwidth', 'opi_toggle_symbol']
> 名字差集 头−Rust: []
> 签名不一致: 0
> exit=1
> ```
> 头文件指纹与 mtime **一个字都没动** —— 所以这不是「头文件被改坏了」，
> 是**新加的导出没有被声明**。**签名不一致仍为 0** 也有意义：它说明新增的那三个
> 不是「写错了类型」，而是**根本没进声明面**（差集里出现，就不会进签名比对）。
>
> **项目自己的门禁也是红的**（`ffi-contract` 的 `crates/opi-ffi/tests/c_abi_contract.rs`，
> 对 `macos/OpiFFI.h` 要求**集合相等**；我跑它是只读的）：
> ```
> $ cargo test -p opi_ffi --test c_abi_contract
> test macos_header_matches_cabi ... FAILED
> crates/opi-ffi/src/cabi.rs 与 macos/OpiFFI.h 漂移（共 3 条）：
>   头文件缺少 Rust 导出 `opi_fullwidth_state`
>   头文件缺少 Rust 导出 `opi_toggle_fullwidth`
>   头文件缺少 Rust 导出 `opi_toggle_symbol`
> test result: FAILED. 13 passed; 1 failed
> ```
> **两个独立脚本给出同一组 3 个名字** —— 这比「我的脚本说 0」强得多。
> 注意同一跑里 `header_compiles_and_consumer_builds ... ok`：那是 `c_abi/consumer.c`
> 只调了**已声明的那些**，所以能过；**别把它读成「头文件没事」**。
> 补声明归 `ffi-contract`；本节只做记录。

#### 补齐后重跑：取地址 TU 现在能过，且这次是**编译器**说的（2026-09-27 同日更晚）

脚本对脚本仍是「互证」，所以补完之后我把**真正的那条**重跑了一遍 ——
「把 `cabi.rs` 的每个导出**取地址**」写成一个 TU，`-Werror` 下编它，**少一个声明就断**：

```
$ python3 … > /tmp/check_opi.c      # 从 cabi.rs 抽 #[unsafe(no_mangle)] 的 opi_* 名字
$ head -3 /tmp/check_opi.c
#include "OpiFFI.h"
const void *refs[] = {
    (const void *)&opi_ffi_free_string,
$ tail -2 /tmp/check_opi.c
    (const void *)&opi_export_user_words,
};
/* 取地址的导出数 = 31 */

$ clang -std=c11   -Wall -Wextra -Werror -Imacos -fsyntax-only -x c   /tmp/check_opi.c ; echo $?
0
$ clang -std=c++17 -Wall -Wextra -Werror -Imacos -fsyntax-only -x c++ /tmp/check_opi.c ; echo $?
0
```

**这条比我那两个 Python 脚本强**：`c11_exit=0` / `cxx_exit=0` 是 **clang 给的**，
不是「我写的脚本说 0」。它证的是「**31 个导出名在头文件里个个有声明，且头文件在
C 与 C++ 两种模式下都吃得下**」。

⚠️ 它**仍不**证：类型/签名逐个对得上（`&f` → `const void *` 对**任何**函数类型都成立，
所以这条检查对签名是瞎的 —— 签名那一层只有上面那个脚本可比，而它弱在自证）；
也不证能链接、不证 Swift 侧任何一行。

**两条互补，缺一不可**：clang 证「名字齐、头文件能解析」（强证据但盲于类型），
`abi_parity.py` 证「类型序列逐条一致」（弱在自证但覆盖类型）。

⚠️ **这个脚本本身出过错**：同一轮里它先后报出 16 / 5 / 6 / 19 个「不一致」，
**全部是脚本自己的 bug**（参数名没剥干净、`bool`/`_Bool` 映射不对称、
`const uint16_t *path` 里指针与名字粘连、`[a-z0-9_]` 在 UTF-8 相邻处的怪异行为）。
所以这条证据的强度是「**我写的一个脚本说 0**」，不是「编译器说 0」——
在 Mac 上链一次仍然是唯一能钉死这件事的办法。

这两条证明的是 **C ABI 能为 macOS 目标编译，并真能 codegen + 归档出一个含当时全部
`opi_*` 导出的 `.a`；且本头文件自身能被 clang 解析**（**具体条数不写在这里** ——
它随 `cabi.rs` 变，以 `cabi.rs` 的 `#[unsafe(no_mangle)]` 计数与 `c_abi_contract` 门禁为准）。
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
  Android 有模式按钮、fcitx5 轨没接。**iOS 侧本轮接了**（`ios/KeyboardViewController.swift`
  的 `cycleMode()`：中→繁→英→中，前两跳先 `clear()`，逐字对齐 Android `toggleMode()`）；
  macOS 侧**没有三态循环**（那需要 IMK 菜单项，是工程层的事），
  但**有一个热键**：`Ctrl+'` 切英文/回拼音（`hotkey(_:)` 判在 `handle(_:client:)` 里，
  与 `vk.rs` 的 `hotkey_target` 同判「来回切」）。**这两个入口不等价** ——
  热键那条不走 `clear()`（它调的是 `opi_switch_mode`，`router.rs:166` 的 `switch_mode`
  会顺手清前端 ⇧ 三态，**但仅在目标不是 English 时**：切到英文保留 ⇧ 是有意的，
  英文模式下还得靠它打大写）。
  **接的时候必须走 `opi_switch_mode`**，
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
  `OpiFFI.h` 的 clang 语法检查与「导出名取地址」那个 TU（**它现在能过**：
  2026-09-27 头文件补齐后，`cabi.rs` 的每个导出都在声明面里；
  它一度编不过，正是这条检查的价值所在，见「已验证」的订正注记）。
  **Swift 三个文件的验证数是 0**：没有编译、没有 lint、没有单测。
  第一条针对 Swift 的可执行验证就是「在 Mac 上编译通过」（见「第一件要做的事」#3）。
- **G5 宠物「小欧」未接（有意）**：几何真源是 `docs/opi-pet.svg`；Android 与 Windows
  候选窗共用 `shared/pet/OpiPet.kt` 这一份实现（本项目原则：各端 UI 不共享代码，
  但宠物是一张图）。macOS 侧**故意没画**：候选窗用的是系统面板 `IMKCandidates`
  （见「最不确定的 API 清单」#3，那个 `setCandidateData` 不生效的报告也在那里），
  **画不进去** —— 要摆小欧得先换成自绘 `NSPanel`，那是对未编译代码做结构改动。
  真要接时**别**新写一份 CoreGraphics 几何：同一张图的第三份拷贝必然漂移。
  顺序照旧 —— 先让 Swift 过编译器，再谈接。
- **G6 符号库：包装有了，UI 没有**（本轮新增）。`OpiEngine.swift` 现在有
  `symbolBlocks()` / `symbolsInBlock(id:)` / `searchSymbols(keyword:)`
  （对应 `opi_symbol_blocks` / `opi_symbols_in_block` / `opi_search_symbols`），
  但 **macOS 侧没有任何调用点** —— 没有符号面板。要接的话得先决定
  「符号面板在 IMK 里长什么样」（候选窗是 `IMKCandidates`，摆不下网格，
  见 G5 同一条理由）。iOS 侧的写法（`ios/SymbolPanel.swift`）可作参考，
  但**别**把 UIKit 代码搬过来。
  三条契约（与 iOS 侧同一份，写在这里免得各抄一遍）：
  ①`opi_symbol_blocks()` **只回 common 块**，JSON 里的 `common` 字段在该出口上恒为
  true（当前数据表只有 CJK 符号块是 common → 「常用」= 60 个）；
  ②块 id 在 Rust 侧是 `u16`、在 C ABI 是 **`int16_t`**，只能对齐到 0..=32767 ——
  按 `Int` 解码后**显式收窄**，直接解成 `Int16` 会在 id > 32767 时抛异常，
  而本层对解析失败的处理是「返回空数组」，后果是面板整块空掉且**无日志**；
  ③`opi_search_symbols("")` 返回**全部**条目，不是「没搜到」。

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

⚠️ **两个 Engine 能力的 C ABI 出口：Rust 已落地，声明头还差 3 个**（2026-09-27 核
`cabi.rs` / `api/mod.rs` / `jni.rs` / `OpiFFI.h`）：

| Engine 方法 | `cabi.rs` 导出 | `OpiFFI.h` 声明 | Apple 两平台 | Android(JNI) | fcitx5 轨 | TSF 轨 |
|---|---|---|---|---|---|---|
| `Engine::toggle_symbol()` | ✓ `opi_toggle_symbol` | ✓ | ✓ 已接键 | ✗ | ✗ | ✗ |
| `Engine::toggle_fullwidth()` | ✓ `opi_toggle_fullwidth` | ✓ | ✓ 已接键 | ✗ | ✓ | ✓ 已接键 |
| （读侧）`fullwidth_state()` | ✓ `opi_fullwidth_state` | ✓ | 包装已写、**无 UI 读** | ✗ | — | — |

⚠️ 最后一行的「无 UI 读」与 `shiftState()` 是**同一种状态**：出口有了、Swift 包装有了、
UI 没读 —— 所以也仍然**没有会漂移的镜像**（本仓反复被坑的那个形状）。
**故意不引「声明面」这一列的数字**：它随每次扩容变，以 `c_abi_contract` 门禁为准。

> ⚠️ **2026-09-27 更晚订正**：`OpiFFI.h` 已补齐到 **31/31**（同日，`ffi-contract` 补的），
> 所以下面这句里的「`OpiFFI.h` 是 28 个声明」**当时成立、现已不成立**。
> **「声明头滞后」这三次的历史仍然有效**（它记的是发生过什么），但**具体数字不要再抄**。

`cabi.rs` 那时是 **31** 个导出，`OpiFFI.h` 是 **28** 个声明 —— 差的正是这三个。
**这是「声明头滞后于 `cabi.rs`」第三次出现，而它比前两次更硬**：Swift 桥接头里没有的函数
Swift 编译器就当它不存在，`opi_toggle_symbol()` 这种调用**不是「运行时失败」，是编译不过**。
所以本目录**没接**这两件事，也**没有**在 Swift 侧自建一份（自建就是第五份实现）。
**头文件归 `ffi-contract`，不在本目录范围内；已报给 lead。**
（机器可查：`python3 /tmp/abi_parity.py`，按名字逐条比 `cabi.rs` ↔ `OpiFFI.h`，现在
应当报这 3 个名字的差集、退出码 1。）

三条约定（lead 已给，接线前必须照办，别按直觉推）：
1. **`opi_switch_mode` 与 `opi_toggle_symbol` 是仅有的两个「调完必须重读状态」的出口。**
   `toggle_symbol` 之后要重读 **mode / buffer / candidates / fullwidth 四样**。
   依据：`composer.rs` 的 `switch_mode` 无条件做 `fullwidth = mode.default_fullwidth()`，
   而 `default_fullwidth` 是 `matches!(self, Mode::Pinyin | Mode::Traditional)`
   —— 切模式会**顺手改全角**。
2. **客户端不许自己推导全角映射**，`opi_fullwidth_state()` 是给状态栏读的。
   理由：`engine.rs` 里 `'` 在 Pinyin/Traditional 且**缓冲非空**时是**音节分隔符**，
   只有缓冲为空时才是引号 —— 所以映射**不是** `(mode, fullwidth)` 的纯函数。
3. **全角不跨模式粘**：Pinyin/Traditional → 全角，English/Number/Symbol → 半角。
   全局粘会让英文模式下的 `,` 变成 `，`，违反「英文/数字半角直通」。
   键位：`Ctrl+'` 切英文、`Ctrl+\` 切 Symbol、`Shift+Space` 切全角（与桌面两轨一致）。

签名（头文件补齐后照抄，别改）：
```c
bool      opi_toggle_fullwidth(void);  /* 返回**切换后**的新状态 */
bool      opi_fullwidth_state(void);   /* 读侧；未装载 → false */
OpiString opi_toggle_symbol(void);     /* 返回**待上屏的文本**（切换前缓冲的挂起结果），
                                          空句柄 = 无提交；不是「新选中的那个符号」 */
```

⚠️ **顺带订正一句 Rust 侧注释**：`Engine::toggle_symbol` 与 `candidate.rs` / `logic.rs`
都写着「键位未定 / B0 未裁决」，但**隔壁代码已经裁决了、而且两轨一致** ——
`tsf-opi/src/vk.rs` 的 `mode_hotkey` 与 `opi_fcitx5.cpp` 的 `handleModeHotkey` 是
同一张表（**Ctrl+`'`** 切英文、**Ctrl+`\`** 切 Symbol）；全角是 **Shift+Space**
（`fcitx5-opi.cpp` 的 `handleFullwidthHotkey` 附了键位占用调查，`vk.rs` 的
`fullwidth_hotkey` 同判）。接手 macOS 热键的人照那两张表接，别照「未定」自创。

> ⚠️ **上面那两个符号名是承重的，路径和行号不是。** 2026-09-28 当场量到的实况：
> `crates/tsf-opi/src/vk.rs` 先是 **520 行**（超 500，`ime-platform` 在拆），几分钟后
> 复核已是 **222 行**（拆分落地）—— 而 `mode_hotkey` / `fullwidth_hotkey` /
> `hotkey_should_act` **都还在 `vk.rs` 里，行号也没变** —— 走掉的那 298 行是**测试模块**
> （搬到了 `crates/tsf-opi/src/vk_tests.rs`，由 `#[path = …]` 引入），不是语义拆分。
> 所以这一刻路径是对的，**但那是观察值不是承诺**。找它们用
> `grep -rn 'fn mode_hotkey' crates/tsf-opi/`，别照抄路径。

⚠️ **接的时候还有一条必须知道的（否则静默失效）**：这三个热键**不能送进
`opi_key_event`**，必须在 `InputController.swift` 里、**调引擎之前**判掉 ——
`router.rs` 的 `key_event` 直通分支（`router.rs:245`）对 `CTRL|ALT|META` **在 `key_event` 最前面就 `PassThrough`**
（有意的：⌘A/⌘C 不能被吃进拼音缓冲），所以 `Ctrl+'`/`Ctrl+\` 送进去等于交回宿主 App，
**模式不动、无日志**；而 `Shift+Space` 的 SHIFT **不在**直通掩码里，会走到 `KEY_SPACE`
分支，又因「空格分支不看 Shift 位」变成**选首候选**。桌面两轨正是这么做的
（`tsf.rs` 把 `mode_hotkey`/`fullwidth_hotkey` 判在引擎之前）。
**已按这条接线**：`hotkey(_:)` 判在 `handle(_:client:)` 里、`engine.keyEvent` 之前，
`performHotkey(_:sender:)` 执行。⚠️ 抬起也**认出来但只消费不执行**
（同 iOS 侧，以及桌面两轨的理由：只拦按下会给应用一个**没有 keydown 的 keyup**）。

⚠️ **autorepeat 同判「认出来、但不执行」**（2026-09-27 裁决）。按住 `Shift+Space` 稍久一点，
`NSEvent` 会发重复 keyDown（`isARepeat == true`）；让它们执行就会**反复切全角**，
而用户的语义是「一次切换」。**但不能简单地在 `hotkey(_:)` 里返回 `nil`** ——
那一颗键会漏给下面的 `keyEvent`，而 `Shift+Space` 的 SHIFT 不在直通掩码里
⇒ 会被当成**普通空格 = 选首候选**，比反复切更坏。
所以判定函数**故意不过滤** repeat / 抬起，由**调用处**决定「消费但不动作」：

```swift
if event.type == .keyDown, !event.isARepeat { performHotkey(hot, sender: sender) }
```

⚠️ **另两轨不在本分身的域内**（`crates/**` 各有其主）。**接之前先 grep 这两个名字确认**：
TSF 是 `vk.rs` 的 `hotkey_should_act`，fcitx5 是 `opi_fcitx5.cpp` 的 `isRepeatEvent`。
（`ios/README.md` 里有一张四轨对照表。）
> **2026-09-27 观察**：那时四轨都已经有这条，**同一个设计** ——
> 判定函数照常返回「是热键」（好让调用处**认领**这颗键），由调用处决定要不要动作。
> ⚠️ 另两轨那两处当时在**别人的工作区里、未提交**，所以这是**观察不是承诺**，以 `grep` 为准。

（iOS 侧的同一句是**预防性**的：`UIKeyModifierFlags` 里没有 repeat 位 —— 见 `ios/README.md`。）
注意 `Mode::Symbol` 本身**到得了**（`opi_switch_mode(3)` 是通的），缺的只是那个
「提交挂起缓冲再切」的干净入口。

## 状态

**草案。一行 Swift 都没有编译过，没有一个字节被 IMK 加载过。**
上面「已验证」一节里的 `cargo check` 是这整个目录里唯一的真实证据，而它证明的是
**Rust 侧**能编译，不是本目录能用。

下一步（按顺序）：在 Mac 上编译链接 → 最小冒烟（`h a o 空格` → 「好」）→ 逐个消掉
「最不确定的 API」清单 → 再谈功能。**顺序不能反。**
