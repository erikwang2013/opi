# 鸿蒙（HarmonyOS）端 —— 构建 / 打包 / 安装 / 验证

> ## 这份文档的性质：**起点与契约，不是可用实现**
>
> 教程里的每一步要分清**三层**，它们的状态差得很远：
>
> | 层 | 状态 | 判据 |
> |---|---|---|
> | Rust 侧 C ABI（`crates/opi-ffi`） | **已实测**：能为鸿蒙三个目标编译、能产出 `libopi_ffi.a` | 见「第 1 步」，本机可复现 |
> | N-API 桥（`harmony/cpp/napi_bridge.c` + `opi_ffi.h`） | **过了本机 clang 的语法/类型检查**（对着 node 的真 N-API 头），**但从未链接、从未运行**；用的是**宿主 clang，不是 OHOS NDK** | 见「第 1 步」与「验证」 |
> | ArkTS（`harmony/ets/**`，5 个文件） | **一行都没有被编译过，也没有被检查过** | ⚠️ **拦路的不是 SDK 缺失，是仓库缺工程脚手架** —— 见下 |
>
> ### ⚠️ 归因订正（2026-09-28 实测）：**本机是有鸿蒙工具链的**
>
> 本表上一版写「本机是 Linux，没有 DevEco / HarmonyOS SDK」。**那半句实测为假**：
> `/home/component/command-line-tools/` 下装着 Command Line Tools **5.1.1.820**、
> SDK `default/openharmony/{ets,js,native,toolchains,previewer}`（`oh-uni-package.json`
> 逐个读过：apiVersion **19** / 5.1.1.202）、以及 `hvigor` · `codelinter` · `ohpm 5.1.4`。
> 注意它是 **Command Line Tools，不是 DevEco Studio**（两个东西）。
>
> **真正的原因**：`harmony/` **不是一个工程** —— 缺工程级 `build-profile.json5`、
> `hvigorfile.ts`、模块级 `build-profile.json5`（判据：`find harmony/ -name 'build-profile.json5'` 零命中）。
> 实测 `codelinter harmony/ets` 报 **「The entered inspection path is incorrect, please make
> sure this path is under the project path.」** ⇒ **任何工具都处理不了它，连静态检查都不肯做。**
>
> **补上脚手架之后它真的会被解析**：把 `harmony/ets` 原样放进一个补齐脚手架的项目里，
> `codelinter` 会**如实报出故意塞进去的语法错误**（`@parsing-error`）⇒ 那是阳性对照，
> 证明解析器不是空转。
> ⇒ **结论不变（ArkTS 一个字符没被检查过），但归因变了；而且新归因是可执行的** ——
> 它把「本机做不到」换成了「**缺三份脚手架文件**」。
> ⚠️ 本机是否有鸿蒙**设备或模拟器**未核实，别据本段推断。
> **别把「Rust 侧能编」读成「鸿蒙端能用」。** 本项目已经被「没被编译器看过的代码」
> 坑过两次，本文档的存在正是为了避免第三次：
>
> | 前两次 | 代价 |
> |---|---|
> | fcitx5 C++ 插件 | 写在仓库里、README 记着「待验收」，一编译发现 **7 处 API 误写**（连 `fcitx::InputMethod` 这个类都不存在） |
> | Windows TSF | README 记着「完成，待在 Windows 上验收」，实测 `DllGetClassObject` 恒返 `CLASS_E_CLASSNOTAVAILABLE`、**文本插入从未实现** → 整个端不可用 |
>
> 所以本文的目标不是「照着做就能用」，而是**把第一轮编译要撞的墙提前标出来**，
> 并说清哪些步骤走完之后才算真的接对了（见「验证」一节）。
> 更细的依据、原始实测记录、以及 20 条「我最不确定的 API」全在 `harmony/README.md`
> —— **那份是权威依据，本文是它的操作化版本**，两者冲突时以它为准。
> 引擎侧的真源在 `crates/opi-ffi/src/cabi.rs`（C 导出）与 `crates/engine-core/src/router.rs`（键路由）。
>
> ⚠️ 本文引用 `harmony/README.md` 用的是它的小节编号（§1–§5d 指**「实测记录」那一节内的**编号）。
> **编号会随那份文档增删而漂** —— 对不上时按小节标题里的关键词找
> （如「Rust 侧为鸿蒙目标编译」「`CMakeLists.txt` + 真编译」「导出符号」），别按号数找。

---

## 1. 前提

| 项 | 要求 | 依据 / 状态 |
|---|---|---|
| DevEco Studio | **仓库没有声明任何最低版本**。`harmony/README.md` 的「平台限制」P3 记的是「检索建议 DevEco 6.0+」 | **未核实**：这是检索结论，不是实测；装之前先确认你手上的版本能建 InputMethod 类型的扩展 |
| HarmonyOS SDK / API 版本 | **仓库没有声明**。`harmony/` 下**没有** `build-profile.json5`（也不该有 —— 那是 DevEco 生成的文件） | 未核实：以你 SDK 版本的模板为准 |
| 目标系统 | P3 另记：部分 HarmonyOS NEXT 版本**不开放 InputMethod 类型的 Extension Ability** | 检索，**需核对**：开发前先确认目标系统支持 |
| Rust | 根 `Cargo.toml` 的 `[workspace.package] rust-version`（当前声明 `1.88`）。**这是声明**，实际下限由 CI 的 `msrv` job 量出来 | `Cargo.toml` · `.github/workflows/ci.yml` |
| Rust target | 三个都要装：`aarch64-unknown-linux-ohos`（arm64-v8a）、`armv7-unknown-linux-ohos`、`x86_64-unknown-linux-ohos` | `harmony/README.md`「构建集成」；它们**不在**默认安装里，必须先 `rustup target add` |
| CMake | `harmony/cpp/CMakeLists.txt` 里写的是 `cmake_minimum_required(VERSION 3.4.1)`。在 cmake ≥3.31 是**弃用警告**，在 **CMake 4.0+ 是硬错误** | 本机 cmake 3.31.4 观察到的警告见 `harmony/README.md` 实测记录 §4b。**起来第一件事**就是把它抬到 3.10+ |
| 词库 | **现在不需要**：本端走 `opi_load('')` = 引擎**内置回退词库**（`engine-data/src/dictionary.rs` 的 `fallback_dict`）。完整词库 `data/generated/luna.opid` 的通路**尚未接线**（rawfile → 沙箱绝对路径，见「已知限制」G2） | `KeyboardController.ets` 的 `ensureDictionary()` |
| 设备 | 真机或模拟器。**输入法不会自动生效**，必须在系统设置里手动启用 | `harmony/README.md` 的 P1（检索）|
| 宿主 App | 输入法扩展**不能独立安装**，要在一个 DevEco 应用工程里 | 与 Apple 两端同一形态（未核实，凭记忆）|
| 工程文件 | 仓库里**没有** `build-profile.json5`、没有工程级 `oh-package.json5`、没有 `hvigorfile.ts` —— 工程要在 DevEco 里新建 | 判据：`find harmony/ -name 'build-profile.json5'` 零命中 |

```bash
rustup target add aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos
```

⚠️ **别拿 `rustup target list --installed` 的空输出当判据** —— 它对**已装**的目标也会漏报
（`harmony/README.md` 实测记录 §1 记了 `aarch64-apple-darwin` 的实例）。
非空输出可以信，空输出不能当判据；**可靠做法是拿它真跑一次 `cargo check`**（下一条命令）。

---

## 2. 第 1 步：Rust 侧 —— **本文唯一在本机实测过的部分**

### 2.1 三个鸿蒙目标编译

`cargo check` 不链接，所以 Linux 上不需要 OHOS SDK。本机实测（Linux，2026-09-28）：

```
$ cargo check -p opi_ffi --target aarch64-unknown-linux-ohos
    Checking engine-core v1.3.1 (...)
    Checking engine-data v1.3.1 (...)
    Checking opi_ffi v1.3.1 (...)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 55s     # exit 0
$ cargo check -p opi_ffi --target armv7-unknown-linux-ohos                   # exit 0
$ cargo check -p opi_ffi --target x86_64-unknown-linux-ohos                  # exit 0
```

（输出里的 `v1.3.1` 是**跑这些命令时**的工作区版本，不是「当前版本」—— 当前版本以根
`Cargo.toml` 的 `[workspace.package] version` 为准。本文照原样保留记录，不把它改写成
「看起来是最新的」。）

### 2.2 产出 `libopi_ffi.a`

`crates/opi-ffi/Cargo.toml` 的 `crate-type` 已含 `staticlib`，**不需要改 Cargo 配置**。
**静态库归档这一步不需要 OHOS 链接器**，所以 Linux 上也能出：

```bash
# 在仓库根执行，三个 ABI 按需产出
cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release --crate-type staticlib
cargo rustc -p opi_ffi --target armv7-unknown-linux-ohos  --release --crate-type staticlib
cargo rustc -p opi_ffi --target x86_64-unknown-linux-ohos --release --crate-type staticlib
```

本机实测（aarch64，2026-09-28）：

```
$ cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release --crate-type staticlib
    Finished `release` profile [optimized] target(s) in 38.57s                   # exit 0

$ ar t target/aarch64-unknown-linux-ohos/release/libopi_ffi.a | wc -l
404                                  # 归档成员数，随代码增长漂 —— 只是个规模感

$ ar x libopi_ffi.a && file $(ls *.o | head -1) | cut -c1-100
ELF 64-bit LSB relocatable, ARM aarch64, version 1 (SYSV), with debug_info, not stripped
```

最后那条是**必查项**：确认归档里的目标文件真是 **AArch64**，而不是拿宿主 x86_64 糊弄的。
（成员名与 `file` 措辞按你的 Rust 工具链版本会有出入，判据是「出现 `aarch64`」；
上面那个成员名是 compiler-builtins 的，**别抄**。）

### 2.3 导出符号齐不齐：做集合比对，别数个数

```bash
A=target/aarch64-unknown-linux-ohos/release/libopi_ffi.a
nm -g --defined-only "$A" | awk '$2 ~ /^[TtDdBbRr]$/ {print $3}' \
  | grep '^opi_' | grep -vx 'opi_ffi' | sort -u > /tmp/opi_exports.txt
wc -l < /tmp/opi_exports.txt            # 只为看规模；判据在下面
```

**本文不写导出条数** —— 它随每次扩容变（`cabi.rs` 已经走过 19 → 20 → 22 → 28 → 31 → …，
`harmony/README.md` 实测记录 §3 专门为这件事立了订正）。
条数与契约**以 `crates/opi-ffi/tests/c_abi_contract.rs` 的门禁为准**：

```
$ cargo test -p opi_ffi --test c_abi_contract --no-fail-fast
...
[契约] harmony/cpp/opi_ffi.h：19 条声明，全部与 cabi.rs 同型且都被桥调用（全集 34 条，有意子集）
[契约] macos/OpiFFI.h 与 cabi.rs 逐条一致（34 个导出）
[契约] .../target/release/libopi_ffi.so 里核对到全部 34 个导出符号
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out     # exit 0
```

上面那两行数字（19 / 34）是**门禁当场打印的**，不是本文写的常量 —— 门禁扩容后它自己会变，
这正是它该有的样子。

### 2.4 这一步**不**证明什么

- `cargo check` **不链接** ⇒ 不证明能链成 `.so`。
- `staticlib` 归档**不需要链接器** ⇒ 也不证明能被 OHOS 运行时接受。
- Rust 静态库自带 `std` 与 `compiler_builtins`，**与 OHOS 运行时是否符号冲突未知**。
- 鸿蒙的三个目标**不在 CI 里**。判据：`grep -n 'ohos\|harmony' .github/workflows/ci.yml`
  （本文写作时零命中；Apple 的三个目标则在，见 `cargo check -p opi_ffi (Apple 目标)` 那一步）。
  要加的话脚本在 `harmony/README.md` 的「建议进 CI」一节，**那节自己就写着「只是建议，CI 文件本轮未改」**。

---

## 3. 第 2 步：在 DevEco 工程里落地 —— 本机**不可编译**的部分从这里开始

### 3.1 顺序不能反：先让编译过，一行逻辑都别改

在这些代码从未过过编译器的情况下，「写得对不对」还没有意义。
**把编译错误当成草案的待办清单来读**：这份草案是尽力按记忆写的，编译器报的每一条都是在替你列 TODO。

```
1. 用 DevEco 的 `New > Extension Ability > InputMethod` 生成一份最小输入法模板
   （**生成的模板才是权威**；harmony/module.json5 是给你对答案用的片段，不要整个覆盖）
2. 按下面「文件落点」把文件搬进去，逐键比对 module.json5 的 extensionAbilities
3. → 目标：**编译通过**。此时功能一个都不能用，这是正常的
```

### 3.2 文件落点

| 仓库里的文件 | 放进 DevEco 工程的 |
|---|---|
| `harmony/cpp/opi_ffi.h` · `napi_bridge.c` · `CMakeLists.txt` | `entry/src/main/cpp/` |
| `harmony/cpp/types/libopiime/`（`index.d.ts` + `oh-package.json5`） | `entry/src/main/cpp/types/libopiime/` |
| `harmony/ets/InputMethodExtensionAbility/*.ets`（5 个） | `entry/src/main/ets/InputMethodExtensionAbility/`（含 `pages/Index.ets`）|
| `harmony/resources/base/profile/input_method_config.json` | `entry/src/main/resources/base/profile/` |

**仓库故意不提供、必须由 DevEco 生成的两个文件**（覆盖会毁掉你已有的配置）：

- `resources/base/profile/main_pages.json` —— 内容 `{ "src": ["pages/Index"] }`。
  它里面那条路径必须与 `KeyboardController.ets` 的 `panel.setUiContent('pages/Index')`
  **逐字一致**，否则表现是「面板起来了但是一片白」。
- `resources/base/element/string.json` —— `input_method_config.json` 的 `label` 引用了
  `subtype_pinyin_label` / `subtype_english_label` / `subtype_number_label` 三个 key，
  缺了会让 profile 解析失败，**可能表现为输入法直接不出现在系统列表里**。

### 3.3 四处名字必须一致（不一致的表现是**静默** `import` 得到 `undefined`）

```
CMakeLists.txt 的 target 名  ⇄  napi_bridge.c 的 nm_modname  ⇄  产物 libopiime.so  ⇄  ArkTS 的 import 'libopiime.so'
```

见 `harmony/cpp/napi_bridge.c` 顶部「模块名一致性」那段。**注册失败不报错**，
这是全目录最难查的一条（`harmony/README.md` 的 U7）。

### 3.4 登记 CMake，否则 CMake 根本不会被调用

`entry/build-profile.json5` 里要有（否则表现是「没生成 .so、ArkTS 侧 import 得到 undefined」）：

```json5
"buildOption": {
  "externalNativeOptions": {
    "path": "./src/main/cpp/CMakeLists.txt",
    "abiFilters": ["arm64-v8a"]
  }
}
```

`CMakeLists.txt` 靠 `libs/${OHOS_ARCH}/libopi_ffi.a` 找 Rust 库，
找不到时它会 `FATAL_ERROR` **明确报错**（不是一堆 `undefined reference`）。ABI 目录名是**鸿蒙的名字**，
不是 Rust triple 名：

| 鸿蒙 ABI 目录 | Rust target |
|---|---|
| `arm64-v8a` | `aarch64-unknown-linux-ohos` |
| `armeabi-v7a` | `armv7-unknown-linux-ohos` |
| `x86_64` | `x86_64-unknown-linux-ohos` |

```bash
cp target/aarch64-unknown-linux-ohos/release/libopi_ffi.a  entry/src/main/cpp/libs/arm64-v8a/
cp target/armv7-unknown-linux-ohos/release/libopi_ffi.a    entry/src/main/cpp/libs/armeabi-v7a/
cp target/x86_64-unknown-linux-ohos/release/libopi_ffi.a   entry/src/main/cpp/libs/x86_64/
```

### 3.5 链接期缺符号 → 对着这两组查，别先怀疑 C 面

`harmony/README.md` 实测记录 §4b 用 `nm -u` 列全了桥要**外部提供**的符号（那是编译器给的，不是手抄的）：

- 全部 `opi_*` → 来自 `libopi_ffi.a`（少一个 = Rust 库没放对地方）；
- `napi_*` 那一组 → 来自 `libace_napi.z.so`（鸿蒙专有库）。**本文不抄那份名单**：
  它是编译器给的（`nm -u` 那个 `.o`），原始输出在 `harmony/README.md` 实测记录 §4b，要现值就自己 `nm -u`；
- 外加 `malloc` / `free` / `memcpy`。

报 `undefined reference to napi_xxx` 时对着那组名字查，就知道是**鸿蒙那边的库没链上**，
不是 C 面写错了。

### 3.6 编译要撞的墙

ArkTS 侧每一个 IME Kit 调用都是**凭记忆写的**，`harmony/README.md` 的「我最不确定的 API」
按「错了会怎样」从高到低排了 20 条（U1–U20）。**先看那三组**：

| 组 | 撞了的表现 |
|---|---|
| U1–U4（`InputMethodExtensionAbility` 基类、`createPanel` 链路、`on('inputStart')` 回调签名、`InputClient.insertText`） | 编译不过；**若只是签名不符会编译过而生命周期不被调用 → 键盘永远不出现 / 一个字都上不了屏** |
| U10–U13（`metadata.name` 的点 vs 下划线、`srcEntry` vs `srcEntrance`、`input_method_config.json` 的 `mode` 取值、`pages/Index` 路径格式） | 输入法装上了但列表里没有 / 子类型不出现 / 面板一片白 |
| U14–U20（Canvas API、`@Builder` 多参、`display` 单位 px vs vp、`JSON.parse` 断言） | UI 不对但键盘能用 |

其中 **U3 / U10 / U11 / U12 / U13 是二手资料互相矛盾的**（`module.json5` 的注释里逐条记了
「哪些文档怎么写、以你 SDK 版本的模板为准」）—— 遇到时**信模板**。

---

## 4. 第 3 步：打包（HAP）

> ⚠️ **本节全部是「凭记忆 + 公开文档」，本机无法验证**：仓库里**没有任何**打包 / 安装 / 签名
> 流程的记录 —— 所以它是**从零写的操作指引，不是记录**。判据（`git grep` 只搜跟踪文件，
> 因此不会被本文自身污染）：`git grep -n 'hdc'` 全仓**零命中**；`HAP` / `hvigor` 的命中
> 只是顺带提到的名词（`harmony/README.md` 的 P9、`harmony/module.json5` 的注释）；
> `签名` 的命中全是「逐条**签名**核对」那个意思，与代码签名无关。
> 标了「未核实」的地方，以 DevEco 的构建输出为准。

### 4.1 产物构成（未核实）

一个 HAP 里大致是：`module.json5`（清单）、`ets/`（编译后的 ArkTS）、`resources/`（资源与
`input_method_config.json`）、**`libs/<abi>/*.so`（原生库）**、以及签名文件。DevEco 的
`Build > Build Hap(s)` 直接产出带签名的 HAP；原生库是**构建期由 CMake 编出来、由 hvigor 收进
`libs/<abi>/`** 的，所以「`.so` 没进包」的表现是运行期 `import` 得到 `undefined`，**不是**安装失败。

### 4.2 Rust 产物怎么进包

**出的是 `.so` 一个，不是两个**：Rust 侧产**静态库** `libopi_ffi.a`（第 1 步），
由 `CMakeLists.txt` 与 `napi_bridge.c` 一起链成 **`libopiime.so`**，
HAP 里只装这一个原生库 —— 这也是选静态库而不是 `cdylib` 的理由（`harmony/README.md`「构建集成」）。

### 4.3 体积

未 strip 的 release 静态库是 **MB 级**（2026-09-28 本机实测 `ls -l` 得 9.3 MB，
它随代码增长漂，要现值就自己 `ls -l`）。原因：带了 `debug_info`、`std`、`compiler_builtins`，
**还含一份 Android JNI 出口**（`crates/opi-ffi/src/jni.rs` 在鸿蒙目标下也照编，是死重量 ——
见「已知限制」G8）。`CMakeLists.txt` 里已有 `-Wl,-s` 与 `--gc-sections` 兜着，**上架前 strip 一次**。

⚠️ 那三条 `target_link_options`（`--gc-sections` / `--allow-multiple-definition` / `-s`）
被 GNU ld 接受，**但 OHOS 的 ld（lld）认不认没验** —— `CMakeLists.txt` 里原本就标了「凭记忆的兜底」。

---

## 5. 第 4 步：安装与启用

> ⚠️ 整节的**命令与菜单名都是凭记忆**（本机没有 hdc、没有设备）。能用的一定是 DevEco 的
> `Run` 或它自带的 Device Manager；`hdc` 那几条当**方向**读，参数以 `hdc help` 为准。

### 5.1 装到设备

1. **最稳的一条**：DevEco 里把工程 Run 到已连接的真机/模拟器（它负责签名、安装、拉起）。
2. 命令行（未核实）：`hdc install <path>.hap`；覆盖安装加 `-r`。
   hdc 是**设备连接器**（与 Android 的 adb 同角色），设备要先在开发者模式里开 USB 调试。

### 5.2 启用输入法

1. **用户必须在系统「设置」里手动启用**（P1）—— 装完不会自动生效，输入法也不会自己弹窗。
2. 到任意输入框，用系统输入法的切换入口（键盘上的地球/切换键，或状态栏）切到「OPI 拼音」。
3. 主应用侧若想主动拉起「选择输入法」弹框：`inputMethod.getSetting().showOptionalInputMethods()`
   （P7。旧接口 `displayOptionalInputMethod` 自 API 9 废弃）。
   ⚠️ 这是**主应用侧**的设置入口，**不是输入法扩展里调的**；P8 另记了它的跨用户限制
   （仅 user 0 授权，错误码 12800025）与「某些版本上点了不生效」的社区反馈。

### 5.3 排障顺序（从最可能到最不可能）

| 症状 | 先查 |
|---|---|
| 设置里根本看不到这个输入法 | `extensionAbilities[].type` 是不是 `"inputMethod"`（**不是** `module.type`）、`exported` 是不是 `true`；`string.json` 的三个 key 全不全；`input_method_config.json` 的 `mode` 取值 |
| 能选中，但键盘面板不出现 | `createPanel` / `resize` / `moveTo` 那条链路（U2）；`on('inputStart')` 回调有没有被调用（U3） |
| 面板起来了但是**一片白** | `main_pages.json` 里有没有 `pages/Index`，且与 `setUiContent('pages/Index')` **逐字一致**（U13） |
| 键盘正常、**一个键都不出字** | `opi_load` 的返回值有没有接（false 时所有出口退化成空操作）；词库文件坏掉也是这个症状 |
| 面板起来了、按键有反应，但候选栏/preedit **永远不刷新** | `@StorageLink` 的字面量键名两边是否一一对应（见「已知限制」/ `Index.ets` 顶部的核对命令）|
| 一次按键**上屏两次** | `onDestroy` 里没 `off('inputStart')`，回调叠加了 |
| 用一会儿键盘消失 | 输入法进程被系统回收 —— 这是设计（P6），状态必须能重建 |

---

## 6. 第 5 步：验证 —— 怎么知道接对了

按顺序走，**每一步只证明一件事**，别跳。前 4 层**本机可跑**，第 5 层只能上设备。

### 6.1 本机可跑的四层

| 层 | 命令 | 证明了什么 | 不证明什么 |
|---|---|---|---|
| L1 目标编译 | `cargo check -p opi_ffi --target aarch64-unknown-linux-ohos`（三个目标） | C ABI 还能为鸿蒙目标编译 | 不链接 |
| L2 静态库 | `cargo rustc … --crate-type staticlib` + `nm` 集合比对 + `file` 看架构 | 归档产得出、符号在、是 AArch64 | 能被 OHOS 接受 |
| L3 桥的语法/类型 | 见下 | `napi_bridge.c` 与真 N-API 头**签名对得上** | **不是** OHOS 编译器，注册与链接未验 |
| L4 ABI 冒烟 | 见 6.2 | 引擎经 **C ABI**（鸿蒙桥消费的同一套导出）在回退词库下真的出字 | **不经过 N-API 桥、不经过 ArkTS** |

**L3 的跑法**（本机 clang 17.0.6 + node v22.17.0 的真 N-API 头）：

```bash
mkdir -p /tmp/opi-napi-shim/napi
echo '#include <node_api.h>' > /tmp/opi-napi-shim/napi/native_api.h   # 垫片，故意不进仓库
cd harmony/cpp
clang -std=c11 -Wall -Wextra -Wpedantic -fsyntax-only \
  -I/tmp/opi-napi-shim -I/usr/local/node/include/node -I. napi_bridge.c   # 本机实测 exit 0，零诊断
```

`-I` 里的 node 头路径**按你机器的安装位置改**（本机是 `/usr/local/node/include/node`，
里面那两个头是 `node_api.h` / `js_native_api.h`）。判据是**垫片真的被读到** ——
要确认，用 `clang -H` 看包含树（`harmony/README.md` 实测记录 §4 贴过一份）。

> **垫片故意不放进仓库** —— 放进 `harmony/cpp/` 会 shadow 掉真 NDK 头，那比没有更危险。
> **「零诊断」验过有没有牙齿**（2026-09-28 本机复跑）：另写一份负向对照，把
> `napi_module` 的字段名写错、给 `napi_get_value_string_utf16` 少传一个参数、
> 给 `opi_toggle_fullwidth` 多传一个参数 —— clang **3 条 error、exit 1**。
> 所以上面那个 exit 0 不是空跑。（原始记录另见 `harmony/README.md` 实测记录 §4。）

### 6.2 L4：本机实测的期望输出（真机冒烟就照它对照）

做法：一份 `/tmp` 里的小 C 程序，**include 的正是 `harmony/cpp/opi_ffi.h`**（那份 use-site 声明面），
链宿主 `libopi_ffi.so`，只经 C ABI 调用 —— 不经过 N-API，不经过 ArkTS。
可打印字符走 `opi_key_event`（与 `KeyboardController.pressKey` 同一条路）。
本机实测（2026-09-28，**内置回退词库**，节选；另打了一份**码位视图**，见下面的读法表）：

```
opi_load(NULL,0) = true
  n key_event(0x6E).action=1 text=
  i key_event(0x69).action=1 text=
  buffer:ni
  candidates_page:["你","⑨","⑲","⑼","⒆","⒐","⒚","⓳"]
  page_count = 5, page = 0
  select_page(0) = 你
  h key_event(0x68).action=1 text=
  a key_event(0x61).action=1 text=
  o key_event(0x6F).action=1 text=
  buffer:hao
  SPACE key_event(0x20).action=2 text=好
  RETURN(空缓冲) key_event(0x1000D).action=0 text=
```

**怎么读这张表**（每一条都对应一个可判定的验收点）：

| 观测 | 含义 |
|---|---|
| `ni` → `action=1`（Consumed）而**不是** Commit | 字母进的是缓冲（preedit），不是直接上屏 |
| `page_count = 5` 且 `select_page(0)` = `你` | 候选**分页**真实存在，点候选是页内索引（别自己算 `page*8+k`）|
| `ni` 的候选里除「你」还有 `⑨⑲…` | **内置符号表**会给同一条拼音多出候选 —— **不是 bug**，别去「修」。码位视图：`⑨` = U+2468 起的带圈数字；`hao` 的另外两条是 U+1F60B / U+1F646（emoji），同样来自符号表 |
| `hao` + 空格 → `action=2` + `text=好` | 引擎侧的最小闭环：空格提交首候选 |
| 空缓冲按回车 → `action=0`（PassThrough）| 引擎按设计把回车**交还应用** —— 而本端 `handlePassThrough` 不接 `KEY_RETURN`（>`0xFFFF`）⇒ **↵ 是死键**（「已知限制」G10）|

> ⚠️ 上表的候选**内容**会随后端词库变：这里用的是回退词库（`data/generated/fallback.opid`，
> 由 `data/raw/fallback.tsv` 生成，里面确实有「你」「好」）。真机上接完整词库后候选会更多，
> **判据是「出现『你』」，不是「只有『你』」**。

### 6.3 真机冒烟（只能上设备）

1. **设置里能看到「OPI 拼音」** ⇒ 清单与打包对了。此时还不能说明任何代码能跑。
2. **打 `n` `i` ⇒ 候选栏出现「你」** ⇒ 「引擎 + 词库 + C ABI + N-API 桥 + ArkTS UI」整条链路的
   第一个可见成功点。**失败时对照 6.2**：本机同一套导出是出字的 ⇒ 问题在**桥/UI 层**，不在引擎。
3. **打 `h` `a` `o` 再按空格 ⇒ 上屏「好」**（跨端最小冒烟）。
   这条与 fcitx5 轨的实测表逐字一致：`crates/fcitx5-opi/cpp/README.md` 那张表里，
   **词库不存在（走内置回退）**那一行就是 `h`/`a`/`o`/`SPACE` → action 1/1/1/2、
   `text=好` —— **本端现在走的正是这一行**（`loadDictionary('')`）。
   **同一个引擎，各端的最小可见成功必须一致**，不一致就是桥接层的问题。
4. **退格**：缓冲非空 ⇒ 删掉拼音字母；缓冲**空** ⇒ 删掉已上屏的字（这条走的正是 6.2 里那条
   PassThrough 通路，引擎放行、应用自己删 —— 写错成无条件 `deleteForward(1)` 的表现是「删不掉已上屏的字」）。
5. **点候选栏最左边的小欧** ⇒ 学习开关取反，preedit 栏出现「不记词」。
6. **⇧** ⇒ 点亮/大写键帽。**LOCK（⇪）到不了**（「已知限制」G9）。
7. 只有 1–6 都过之后，才谈补功能（符号面板、用户词、繁体、全角指示……）。

### 6.4 那条「加了声明就必须加调用」的门禁（**不是整洁要求，是有牙的契约**）

鸿蒙头文件是**use-site 子集**（只声明 N-API 桥真正调用的出口，**故意不抄全**），
这条设计由项目自己的门禁守着：`crates/opi-ffi/tests/c_abi_contract.rs` 的
**`harmony_header_is_consistent_use_site_subset`**。

**判据**（读那条测试的代码得出，不是听名字）：

1. 解析 `harmony/cpp/opi_ffi.h` 里每条声明 → 在 `cabi.rs` 里必须**存在**且**逐参同型**
   （参数类型、返回值；`parse_rust_exports` 比的是 `Sig{args, ret}`）；
2. `harmony/cpp/napi_bridge.c` **去掉注释后**，每个已声明的名字必须**出现过且紧跟可选空白后是 `(`**
   —— 也就是**真的被调用**。只加声明不加调用，红的就是这一条，消息是：
   ```
   `opi_toggle_symbol` 在 harmony/cpp/opi_ffi.h 里声明了，但 napi_bridge.c 从未调用
   ```
3. 另一条测试 **`harmony_struct_layouts_match_macos`**：头文件里 `OpiString` / `OpiKeyEventResult`
   的 typedef 字段必须与 `macos/OpiFFI.h` 布局一致（抄错字段顺序 = 运行期静默读错位）。

**跑法**（本机实测，2026-09-28）：

```
$ cargo test -p opi_ffi --test c_abi_contract --no-fail-fast
[契约] harmony/cpp/opi_ffi.h：19 条声明，全部与 cabi.rs 同型且都被桥调用（全集 34 条，有意子集）
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out     # exit 0
```

**这条门禁在 CI 里有牙**：`cargo test --workspace --no-fail-fast` 会跑到它
（判据：`grep -n 'cargo test' .github/workflows/ci.yml`）。所以「加声明、忘调用」会在 CI 红，
**不需要谁记得**。

⚠️ **它不管什么**（别把绿灯读大）：判据是**文本级**的「名字后面跟 `(`」——
**不校验实参对不对**（那是 6.1 的 L3 clang 层的事），更不校验运行期行为。

✅ **绿灯为什么可信**：它自带「**解析器失明**」护栏 —— 头文件解析出 0 条声明就直接红
（`assert!(!decls.is_empty(), …)`），`harmony_struct_layouts_match_macos` 也会先钉住
「两侧都解析出非空字段」再比。所以「改了个写法让解析器瞎掉、于是集合比对假绿」这条不成立。

**新增一条出口的正确顺序**（照 `harmony/README.md` 实测记录 §5d 那轮的落地方式）：

```
1. crates/opi-ffi/src/cabi.rs        加导出（真源，其余一切都是抄它）
2. harmony/cpp/napi_bridge.c         加 JsXxx + desc[] 一条 + **真调用**
3. harmony/cpp/opi_ffi.h             加声明            ← 忘了第 2 步的调用 ⇒ 门禁红
4. harmony/cpp/types/libopiime/index.d.ts  加类型声明   ← 门禁**不查这条**，得自己对
5. harmony/ets/.../OpiEngine.ets     加类型化包装      ← 无门禁，要靠人
6. 跑：门禁 + L3 的 clang + 下面这条 diff（无输出 = 一致）
```

```bash
# .d.ts 的导出名 ⇄ 桥 desc[] 里的 JS 名字（两边都不写条数，逐条对齐）
diff <(grep -oE '^\s*\{"[A-Za-z]+"' harmony/cpp/napi_bridge.c | grep -oE '"[A-Za-z]+"' | tr -d '"') \
     <(grep -oE '^export const [A-Za-z]+' harmony/cpp/types/libopiime/index.d.ts | awk '{print $3}')
```

⚠️ **两个「鸿蒙桥 N 条」不是一回事，引用时说清是哪个 N**：`index.d.ts` 数的是 **JS 描述符**
（ArkTS 能调到的名字），`opi_ffi.h` 数的是**桥调用的 `opi_*` C 函数**。两者不相等是正常的
（C 侧含 `opi_ffi_free_string` 这种没有 JS 描述符的辅助出口，一个 `JsXxx` 也可能调多个 `opi_*`）。

**想知道「哪些能力已经到了 `cabi.rs`、桥还没挂」**（符号面板、用户词、繁体库、中文标点开关都在里面）：

```bash
comm -23 \
  <(grep -o 'pub unsafe extern "C" fn opi_[a-z_0-9]*' crates/opi-ffi/src/cabi.rs | sed 's/.*fn //' | sort -u) \
  <(grep -oE '\bopi_[a-z_0-9]+\s*\(' harmony/cpp/napi_bridge.c | sed 's/[[:space:]]*($//' | sort -u)
```

**别把这份清单手抄进文档**（手抄的会被当成「工作项」，而且一扩容就假）——
`harmony/README.md` 实测记录 §5 就是为这件事把清单换成命令的。

---

## 7. 已知限制（诚实清单）

**这些不是 bug，是「还没做」或「有意的取舍」** —— 交付时如实转述，别当缺陷重报，
也别当没看见。完整清单在 `harmony/README.md` 的「已知缺口」（**编号以那份为准**；
本文沿用它的编号，只挑会**被用户看见**的几条 —— **缺号不是遗漏**，是那条用户碰不到）：

| # | 限制 | 说明 |
|---|---|---|
| G1 | **子类型切换不换键盘** | 未监听 `inputMethodAbility.on('setSubtype')` ⇒ 切中/英/数字子类型，键盘不变（P4）。 |
| G2 | **词库是内置回退库** | `loadDictionary('')`。完整词库要把 rawfile 拷进沙箱再传**绝对路径**（引擎要的是文件系统路径，不是资源 ID），步骤写在 `KeyboardController.ensureDictionary()` 的注释里。**候选少是这个词库的锅**，不是引擎。 |
| G3 | **无符号面板** | 能提交字面符号的 UI 不存在（`opi_search_symbols` 一族未桥接）。Symbol 模式只有一页硬编码符号。 |
| G4/5 | **无用户词持久化 / 无繁体切换入口** | 对应出口未桥接；`OpiMode.Traditional` 有枚举值，但 UI 的模式轮转进不去。 |
| G7 | **深色主题未接** | `OpiPet` 支持 `dark: true`，但 `Index.ets` 恒传 `false`（没读系统主题）⇒ 深色下候选栏仍是浅色。 |
| G8 | **Android JNI 出口在鸿蒙目标下是死重量** | `jni.rs` 照编，要拆得改 `crates/**`，不在本轮边界内。 |
| G9 | **⇧ 的 LOCK 态用户按不出来** | 进 Lock 的唯一入口是长按（`router.rs` 的 `shift_long_press`），而 `Index.ets` **没给 ⇧ 绑长按手势**。DevEco 上要做的是**一行**：绑长按 → `KeyboardController.tapShiftLongPress()`。 |
| G10 | **空缓冲下 ↵ 是死键** | 引擎返回 PassThrough（「这个回车归应用」），而 `handlePassThrough` 只接退格/Delete 与可打印段 ⇒ 什么都不发生。修它要 IME Kit 的「把按键交给应用」接口，**那个接口叫什么本机查不到**，所以只把缺口写在 `tapEnter` 的注释里，**没有凭记忆补一个 API 名**。 |
| G12 | **全角/符号开关：能力到了桥，UI 没接** | `OpiEngine.toggleFullwidth()` / `fullwidthState()` / `toggleSymbol()` **从 ArkTS 可以调到**，但没有任何键绑 `toggleSymbol()`，`publish()` 推的七样里也**没有 fullwidth** ⇒ 用户看不出状态变了。做的时候回到 `opi_ffi.h` 里那四条契约，**特别是第 1 条**（只有 `opi_switch_mode` 与 `opi_toggle_symbol` 会改全角，后者最易漏）。 |
| G13 | **无中文标点开关** | 中文标点表**默认开**，所以 `，` `。` 照常出得来（与 G3 说的「缺能提交符号的 UI」**不是同一件事**）；缺的是**关掉它 / 读它** —— 用户没有任何入口切回半角 ASCII。 |

> 另有一处**不是缺口、是刻意取舍**：`OpiEngine.ets` 与 `opi_ffi.h` 都**没有** `KEY_UP/DOWN/LEFT/RIGHT`
> —— 软键盘没有方向键，路由对这四个键一律 PassThrough，抄了也没有分支可写。接物理键盘时再补。
> 那三个热键（`Ctrl+'` / `Ctrl+\` / `Shift+Space`）同理：它们是**软键盘产生不出来的组合键**。

---

## 8. 未验证清单（**主体**，照抄现状，别读成已完成）

**ArkTS：0 行编译过。** `harmony/ets/**` 的 5 个文件**连语法都没过过编译器**：
装饰器（`@Entry` `@Component` `@Prop` `@Watch` `@StorageLink` `@Builder`）、
`@kit.IMEKit` / `@kit.ArkUI` 的 `display`、`Canvas` 与 `CanvasRenderingContext2D`、
`JSON.parse(...) as string[]` 的断言，**全部凭记忆写的**。
`harmony/README.md` 的「最不确定的 API」列了 U1–U20，**至今只消掉了 U5 / U6 / U8 的「签名」部分**
（靠对着 node 的真 N-API 头过 clang）—— 哪几条消了、哪几条没消，以那份的订正注记为准，
**列表本身不在这里重述一遍**（重述就是又一份会漂的拷贝）。

**N-API 的实现侧（语法检查看不出来的那几条）：**

- **模块能否注册成功**（U7）：`__attribute__((constructor))` + `napi_module_register` 鸿蒙认不认。
  失败的表现是 `import` 得到 `undefined` **且不报错** —— 全目录最难查的一条。
- 运行期语义：`napi_create_string_utf16` 是否真**拷贝**（U9，规范说拷贝；若不拷贝，
  `take_string` 就是 use-after-free）；`napi_get_value_string_utf16` 的 `bufsize` 是否含 NUL（U8）。
- 鸿蒙 NDK 自己的 `napi/native_api.h` **是否提供**桥引用的那组 `napi_*` 符号
  （签名已对着 node 的真头核过；**这个头在鸿蒙侧存不存在、内容是否一致没验**。
  清单是编译器给的，见 `harmony/README.md` 实测记录 §4b 的 `nm -u` 输出，**本文不另抄一份**）。

**链接与产物：**

- **从未把 `libopi_ffi.a` 链进任何东西**。`harmony/README.md` §4b 那次「链接」是在宿主 gcc 上，
  而且还没走到符号解析就先卡在 OHOS 专有库（`libace_napi.z.so` / `libhilog_ndk.z.so`）上了。
- 那三条 `target_link_options` 是否被 OHOS 的 lld 接受，未验。
- CMake 的 `cmake_minimum_required(VERSION 3.4.1)` 在 CMake 4.0+ **会直接配置失败**
  （据弃用警告推断，本机 cmake 是 3.31.4，碰不到硬错误那一档）。

**配置与清单：** `module.json5` / `input_method_config.json` / `oh-package.json5`
**未过任何 schema 校验**（⚠️ 订正：原因**不是**「没有 hvigor」—— `hvigor` 本机是有的；
是这些文件从未被任何校验器看过，因为 `harmony/` 不是工程）。
`input_method_config.json` 的 `mode: "lower"` **合法取值至今没查到**（U12）。

**打包 / 安装 / 签名：完全没有验证。** 仓库里 `hdc` 零命中、没有任何打包或签名流程的记录
（判据见第 4 节开头）⇒ 本文第 4、5 节是**从零写的操作指引**，不是记录。
真机行为同理：**没有设备、没有模拟器**（⚠️ 订正：SDK 与工具链本机**是有的**，见文首订正段 ——
缺的是 `harmony/` 的工程脚手架，所以那套工具用不起来）—— **「打 `ni` 出不出字」这件事，本机无法回答**。
（唯一能回答一半的是 6.2：本机经 C ABI 在回退词库下确实出字。）

**做过但强度有限的检查（别当成「编译过了」）：**

- `opi_ffi.h` 每条声明的**签名**与 `cabi.rs` 逐条比对 —— 只证**声明面**对齐（有门禁守着）。
- `index.d.ts` 与桥 `desc[]` 的**名字**一一对应 —— 只证名字，**不证返回类型**
  （`.d.ts` 是手写的，ArkTS 对 `.d.ts` 的解析与 TypeScript 并不完全等同）。
- `napi_bridge.c` 的 clang 语法/类型检查 —— 宿主 clang + node 的头，**不是 OHOS NDK**。
- `harmony/README.md` §5d 记的那次变异（去掉 `opi_toggle_symbol` 的调用、保留声明 →
  门禁只红那一条）**是上一轮的记录，本文写作时没有重跑**。

---

## 9. 下一步顺序（别跳）

```
1. DevEco 建工程（New > Extension Ability > InputMethod），把 cpp/ 与 ets/ 搬进去
   —— 先让编译过，一行逻辑都别改；把编译错误当成草案的待办清单
2. 补 main_pages.json 与 string.json；按 harmony/module.json5 逐键补 extensionAbilities
3. 出 libopi_ffi.a（第 1 步，本机可跑），放进 cpp/libs/arm64-v8a/，登记 build-profile.json5
   → 目标：链接通过、opi_* 与 napi_* 全部解析
4. 真机装上、设置里启用 → 打 ni 出现「你」→ hao + 空格上屏「好」
   （失败时回来跑 6.1 的 L1–L4：本机同一条路是通的 ⇒ 问题在桥/UI）
5. 只有 4 过了，才谈补功能：符号面板、用户词、繁体、全角指示、⇧ 长按绑定
```

**顺序不能反。** 更细的依据、U1–U20 的不确定清单、每一次实测的原始输出与订正记录，
全在 `harmony/README.md`；C ABI 的真源在 `crates/opi-ffi/src/cabi.rs`；
键路由的语义在 `crates/engine-core/src/router.rs` 与 `keys.rs`。
