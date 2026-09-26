# OPI 鸿蒙（HarmonyOS）端 —— **草案，从未编译，未验证**

> ## ⚠️ 先读这段
>
> 本目录的 **ArkTS（`.ets`）与 N-API 桥（`.c`）一个字符都没有编译过**。
> 它们是在 Linux 上写的：本机没有 DevEco Studio、没有 HarmonyOS SDK、
> 没有鸿蒙设备或模拟器，`import { InputMethodExtensionAbility } from '@kit.IMEKit'`
> 在普通 tsc 下就断，连语法检查都做不到。**不要把这里当成「鸿蒙端已完成」。**
>
> 这份东西的定位是：**给鸿蒙开发者的起点 + 精确契约**。
> 它替你省掉「从零读 C ABI、从零搭输入法骨架、从零搞清 N-API 怎么注册」的时间，
> **不**替你做「代码能跑」这件事。
>
> 本项目已经被「没被编译器看过的代码」坑过两次，不要让它成为第三次：
> - **fcitx5 C++**：写在仓库里、README 记着「待验收」，一编译发现 7 处 API 误写
>   （连 `fcitx::InputMethod` 这个类都不存在）
> - **Windows TSF**：README 记着「完成，待在 Windows 上验收」，实测
>   `DllGetClassObject` 恒返 `CLASS_E_CLASSNOTAVAILABLE`、无 CLSID、
>   文本插入从未实现 → 整个端不可用
>
> **本目录里唯一经过验证的东西是「Rust 侧能为鸿蒙目标编译，并能产出真归档」**
> （见下方「实测记录」）。其余 12 个文件全部是待编译验证的草案。
> **全目录没有一处可以称为「完成」或「已实现」。**

---

## 在 DevEco 上第一件要做的事

**顺序不能反：先让编译过，一行逻辑都别改。** 不要一上来就改 UI、也不要先看功能对不对 ——
在这些代码从未过过编译器的情况下，「写得对不对」这个问题还没有意义。
**把编译错误当成草案的待办清单来读**：这份草案是尽力按记忆写的，但记忆一定有错，
编译器报的每一条都是在替你列 TODO。

```
第 1 步：建工程，把 cpp/ 与 ets/ 放进去，让它编译过
  用 DevEco 的 `New > Extension Ability > InputMethod` 生成一份最小输入法模板
  （生成的模板**才是权威**，本目录的 module.json5 是给你对答案用的片段，
   不要整个覆盖）。然后把：
     cpp/*            → entry/src/main/cpp/
     cpp/types/libopiime/ → entry/src/main/cpp/types/libopiime/
     ets/InputMethodExtensionAbility/* → entry/src/main/ets/InputMethodExtensionAbility/
     resources/base/profile/input_method_config.json → 同名位置
  并在 module.json5 里按 harmony/module.json5 的注释逐键补 extensionAbilities。
  → 目标：**编译通过**。此时功能一个都不能用，这是正常的。
  → 大概率要修一堆 API 名字与签名，见下方「最不确定的 API」。

第 2 步：补 main_pages.json 与 string.json
  resources/base/profile/main_pages.json 里加上 "pages/Index"
    （与 KeyboardController 的 setUiContent('pages/Index') **必须一致**，
     不一致的表现是「面板起来了但是一片白」）。
  resources/base/element/string.json 里补 input_method_config.json 用到的三个 key：
    subtype_pinyin_label / subtype_english_label / subtype_number_label
    缺了会让 profile 解析失败，可能表现为输入法直接不出现在系统列表里。
  （本仓库**故意没有**生成 string.json 与 main_pages.json —— 那是 DevEco
   生成的文件，覆盖会毁掉你已有的其它配置。）

第 3 步：按下方「构建集成」把 libopi_ffi.a 弄出来，让链接通过
  → 目标：链接通过 + 28 个符号能解析（全称见 crates/opi-ffi/src/cabi.rs）；
    少一个的表现是「undefined reference to opi_xxx」。
  → 至此，`import opiime from 'libopiime.so'` 应该不再是 undefined。
    **如果仍是 undefined，先查四个名字是否一致**（见 cpp/napi_bridge.c 顶部）。

第 4 步：在真机「设置」里启用这个输入法，然后把光标放进任意输入框
  → 到这一步才算「鸿蒙端能用」的第一天。
  → 第一件该验证的事：**打 `ni`，看候选栏出不出字**
    （词库现在是内置回退库，见下方「已知缺口」，词少但够验证通路）。

第 5 步之后才是：补符号面板、补用户词持久化、补 rawfile 完整词库、
  补子类型切换（setSubtype）、调键盘高度。
```

**不要跳过第 1 步去「顺手重构一下分层」。** 这份代码的分层是按
「Rust 侧是唯一逻辑真源」设计的：本目录**没有一行输入法逻辑**，
改之前先读 `crates/engine-core/src/router.rs` 与 `crates/engine-core/src/keys.rs`
的模块头注释（那里解释了键码为什么不能照抄 fcitx5 / TSF 轨）。

---

## 文件清单

| 文件 | 行数 | 作用 |
|------|-----:|------|
| `cpp/opi_ffi.h` | 147 | C ABI 的 **use-site 声明面**（只声明桥用到的 16 个，**故意不抄全 28 个**）+ 键码常量 + 字符串所有权约定 |
| `cpp/napi_bridge.c` | 323 | **N-API 薄壳**：JS 字符串 ⇄ UTF-16 搬运、模块注册、释放 `OpiString`。15 个模块导出 |
| `cpp/CMakeLists.txt` | 60 | 把 Rust 静态库 + N-API 桥编成 `libopiime.so` |
| `cpp/types/libopiime/index.d.ts` | 72 | `libopiime.so` 的 ArkTS 类型声明（15 条，与 bridge 的 15 条**机械核对一致**） |
| `cpp/types/libopiime/oh-package.json5` | 12 | 把 `libopiime.so` 映射到上面的 `.d.ts` |
| `ets/InputMethodExtensionAbility/InputMethodService.ets` | 34 | `InputMethodExtensionAbility` 入口。**故意只做转发**，零逻辑 |
| `ets/InputMethodExtensionAbility/KeyboardController.ets` | 273 | 输入法会话唯一持有者：开面板、拿 `InputClient`、按键按 `action` 分派、状态推给 UI |
| `ets/InputMethodExtensionAbility/OpiEngine.ets` | 235 | C ABI 薄桥：枚举、**键码常量唯一定义处**、JSON 候选解码 |
| `ets/InputMethodExtensionAbility/OpiPet.ets` | 337 | 项目宠物「小欧」的 ArkTS 复刻（键帽精灵，表情跟引擎状态走） |
| `ets/InputMethodExtensionAbility/pages/Index.ets` | 284 | 键盘 UI 最小骨架：候选栏 + preedit + 三行 QWERTY + 功能行 |
| `module.json5` | 48 | `extensionAbilities` 配置**片段**（不是能直接用的完整 manifest） |
| `resources/base/profile/input_method_config.json` | 39 | 输入法子类型配置 |

**上表 12 个文件合计 1864 行**；加上本 README（421 行），`harmony/` 共
13 个文件 / 2285 行。除下方「实测记录」里列出的 Rust 侧编译检查外，
**本目录无任何验证**。

### 宠物小欧的真源不是本目录

`OpiPet.ets` 是复刻，不是发明。几何与表情的规格在别处，改小欧请四处同步改：

| 文件 | 角色 |
|------|------|
| `docs/opi-pet.svg` | 静态基准：240×250 设计空间、锚点坐标、配色释义 |
| `shared/pet/OpiPet.kt` | 带情绪的 Compose 版（坐标已按情绪分化，**`OpiPet.ets` 照抄的是它**） |
| `android/app/src/test/kotlin/io/opi/input/pet/OpiPetMoodTest.kt` | **`petMood()` 的验收规格** —— 鸿蒙端跑不了测试，这份 Kotlin 单测就是它的标准答案 |
| `harmony/ets/.../OpiPet.ets` | 本文件 |

`petMood()` 的四条分支顺序**不能改**（学习关闭优先于一切，连候选都不看）。

---

## 实测记录（唯一经过验证的部分）

### 1. Rust 侧为鸿蒙目标编译 —— **三个目标全部通过**

`cargo check` 不链接，所以 Linux 上不需要 OHOS SDK。实测原始输出：

```
$ rustup target add aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos
（exit 0）

$ cargo check -p opi_ffi --target aarch64-unknown-linux-ohos
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.10s
exit=0

$ cargo check -p opi_ffi --target armv7-unknown-linux-ohos
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
exit=0

$ cargo check -p opi_ffi --target x86_64-unknown-linux-ohos
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
exit=0
```

> 途中一次「失败」值得记一笔：首次跑 `x86_64-unknown-linux-ohos` 时报
> `error[E0463]: can't find crate for 'std'`，看着像代码问题，
> 实际是后台的 `rustup target add` 还没下完 —— 重跑即过。
> **判断「环境问题还是代码问题」时，先确认目标真的装上了**
> （`rustup target list --installed`）。

### 2. 真产物：aarch64 静态库 —— **产出成功**

不只过了类型检查，静态库也真的产出来了（静态库归档不需要 OHOS 链接器）：

```
$ cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release --crate-type staticlib
    Finished `release` profile [optimized] target(s) in 20.24s
exit=0

$ ls -l target/aarch64-unknown-linux-ohos/release/libopi_ffi.a
-rw-rw-r-- 1 erik erik 9085656  9月27日 01:08 .../libopi_ffi.a

$ ar t libopi_ffi.a | wc -l
404
```

归档里的目标文件**确实是 AArch64**（不是拿宿主 x86_64 糊弄的）：

```
$ file opi_ffi-*.rcgu.o
ELF 64-bit LSB relocatable, ARM aarch64, version 1 (SYSV), with debug_info, not stripped

$ readelf -h opi_ffi-*.rcgu.o | grep -E '类别|系统架构'
类别:     ELF64
系统架构: AArch64
```

### 3. 导出符号 —— **实测 28 个，与 cabi.rs / macos/OpiFFI.h 一致**

```
$ nm -g --defined-only libopi_ffi.a | awk '$2 ~ /^[TtDdBbRr]$/ {print $3}' \
    | grep '^opi_' | grep -vx 'opi_ffi' | sort -u | wc -l
28
```

`opi_backspace` 到 `opi_symbols_in_block` 共 28 个，与 `crates/opi-ffi/src/cabi.rs`、
`macos/OpiFFI.h` 的数量**逐个数一致**。

### 4. 本目录的 C 代码 —— 过了 `clang -fsyntax-only`（**有重要保留**）

`cpp/opi_ffi.h` 与 `cpp/napi_bridge.c` 都用 `clang -std=c11 -Wall -Wextra
-fsyntax-only` 过了，**零警告**；`opi_ffi.h` 另以 C++17 模式过了一遍
（验 `extern "C"` 卫哨可用）。

> **这条证据的效力必须说清楚，别高估：**
> 本机没有 `napi/native_api.h`，所以我是**手写了一个 stub 头**放在 `/tmp`
> （**故意不放进仓库** —— 放进 `harmony/cpp/` 会 shadow 掉真 NDK 头，
> 那比没有更危险）才跑起来的。stub 里的 N-API 函数签名**来自和我写桥时
> 同一个记忆**，所以它**证明不了 N-API 签名是对的**。
> 它真正证明的是三件小事：
> 1. 我的 C 语法合法；
> 2. `opi_ffi.h` 与 `napi_bridge.c` 互相对得上（类型、调用形式）；
> 3. `extern "C"` 卫哨在 C++ 下也成立。
>
> **N-API 那一层的正确性，仍然完全未经验证。**

### 5. 声明与真符号的机械核对 —— **16/16 命中，0 落空**

我数着 `nm` 的输出，把 `cpp/opi_ffi.h` 的每条声明都对了一遍：

```
declared in harmony/cpp/opi_ffi.h : 16
real exports in libopi_ffi.a      : 28
declared but NOT in archive       : （空）
not bridged                       : 12
```

16 已桥 + 12 未桥 = 28，**没有一条声明指向不存在的符号**。
未桥的 12 个（`opi_backspace` `opi_candidates` `opi_clear_user_words`
`opi_export_user_words` `opi_import_user_words` `opi_input_space` `opi_load_trad`
`opi_remove_user_word` `opi_search_symbols` `opi_select` `opi_symbol_blocks`
`opi_symbols_in_block`）对应符号面板、用户词导入导出、繁体词库这些**还没做的功能**。
要做哪个，就去 `cabi.rs` 抄那条声明、加进 `opi_ffi.h`、在 bridge 里挂上。

> ⚠️ 这里**只核对了名字**。**签名（参数类型、返回值）没有核对** ——
> 那正是第 1 步编译要抓的东西。

### 6. 没验证的（比上面长得多）

- ArkTS：**0 行**编译过。装饰器、`@kit.IMEKit`、`@StorageLink`、Canvas 全部未验证。
- N-API：**0 行**编译过（见上面第 4 条的保留）。模块能否注册成功未知。
- `module.json5` / `input_method_config.json` / `oh-package.json5`：
  **未过任何 schema 校验**。
- `CMakeLists.txt`：**从未被 CMake 配置过**，连语法都没验。
- 链接：**从未把 `libopi_ffi.a` 链进任何东西**。Rust 静态库带 std 与
  compiler_builtins，与 OHOS 运行时是否会符号冲突**未知**。
- 真机行为：**完全没有**。没有设备、没有模拟器、没有 SDK。

---

## 构建集成

### 为什么是 N-API 薄壳 + 复用 C ABI

鸿蒙原生模块走 **Node-API（N-API）**，不是 JNI、也不是裸 C ABI 直连。
但**底下的 Rust 引擎不必改，也不该改**：

```
  ArkTS (ets/**)
      │  import opiime from 'libopiime.so'
      ▼
  libopiime.so  ←─ cpp/napi_bridge.c（约 300 行，只做搬运 + 释放）
      │  extern "C" 直接调
      ▼
  libopi_ffi.a  ←─ crates/opi-ffi（**同一个 crate，28 个 C 导出，一个字符没改**）
      │
      ▼
  crates/engine-core（与 Android / iOS / macOS / 桌面 / fcitx5 / TSF **同一份引擎**）
```

**不给鸿蒙再造一套引擎接口** —— 那会变成第 N 份会漂移的语义拷贝，
而本项目已经为「同一语义抄三份」付过代价。

**为什么不引 `napi-rs`**：它要在 `crates/opi-ffi/Cargo.toml` 加依赖、加 `#[napi]` 宏，
那会改到 `crates/**`，并把「一份引擎 + 各端薄壳」变成「Rust 侧长出平台分支」。
手写 N-API 声明约 300 行、**零新依赖、Rust 侧一个字符不用动**。
代价就是那 300 行没有编译器的保护 —— 所以它被明确标成草案。

### 产出 Rust 静态库

静态库（而不是 `cdylib`）能让最终只出一个 `.so`，
不必再把第二个库塞进 HAP 的 `libs/` 目录、也不必操心加载顺序。
`crates/opi-ffi/Cargo.toml` 的 `crate-type` 已含 `staticlib`，无需改动：

```bash
# 在仓库根执行。三个 ABI 按需产出。
rustup target add aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos

cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release --crate-type staticlib
cargo rustc -p opi_ffi --target armv7-unknown-linux-ohos  --release --crate-type staticlib
cargo rustc -p opi_ffi --target x86_64-unknown-linux-ohos --release --crate-type staticlib

# 拷进 DevEco 工程（目录名是鸿蒙 ABI 名，不是 Rust triple 名）
cp target/aarch64-unknown-linux-ohos/release/libopi_ffi.a  entry/src/main/cpp/libs/arm64-v8a/
cp target/armv7-unknown-linux-ohos/release/libopi_ffi.a    entry/src/main/cpp/libs/armeabi-v7a/
cp target/x86_64-unknown-linux-ohos/release/libopi_ffi.a   entry/src/main/cpp/libs/x86_64/
```

ABI 目录名对应关系（`CMakeLists.txt` 靠 `libs/${OHOS_ARCH}/` 找）：

| 鸿蒙 ABI | Rust target |
|---|---|
| `arm64-v8a` | `aarch64-unknown-linux-ohos` |
| `armeabi-v7a` | `armv7-unknown-linux-ohos` |
| `x86_64` | `x86_64-unknown-linux-ohos` |

还要在 `entry/build-profile.json5` 里登记 CMake，**否则 CMake 根本不会被调用**：

```json5
"buildOption": {
  "externalNativeOptions": {
    "path": "./src/main/cpp/CMakeLists.txt",
    "abiFilters": ["arm64-v8a"]
  }
}
```

> 体积提醒：本机产出的 release 静态库 **9.0 MB / 404 个目标文件**，
> 因为带了 debug_info、`std`、`compiler_builtins`，**还含一份 Android JNI 出口**
> （`crates/opi-ffi/src/jni.rs` 在鸿蒙目标下也照编，是死重量）。
> 想去掉 JNI 那份得给 `jni.rs` 加 `#[cfg]` 门 —— 那要改 `crates/**`，不在本轮边界内，
> 记在「已知缺口」。`CMakeLists.txt` 里已有 `-Wl,-s` 与 `--gc-sections` 兜着。

### 建议进 CI（**本节只是建议，CI 文件本轮未改**）

`.github/workflows/ci.yml` 里已有 TSF（Windows）与 Apple 两条同样思路的 job。
鸿蒙可以照抄 Apple 那条的形状，加在它后面：

```yaml
      # 同理：check 不链接，Linux runner 上无需 OHOS SDK。
      # 这守的是「C ABI 还能为鸿蒙目标编译」—— 它**不**证明能链接成 .so，
      # 更**不**证明 harmony/ 的 ArkTS 与 N-API 桥能编译（那需要 DevEco + OHOS SDK）。
      - name: cargo check -p opi_ffi (鸿蒙目标)
        run: |
          # 与 Apple 不同：这三个目标不在默认安装里，**必须先 add**。
          rustup target add aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos
          for t in aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos; do
            echo "== $t =="
            cargo check -p opi_ffi --target "$t"
          done
```

---

## 平台限制

> ⚠️ **本节每一条都是「凭记忆 + 公开文档检索」，本机无法验证。逐条标了需要核对什么。**
> 检索到的官方文档之间还有互相矛盾之处，见「最不确定的 API」。

| # | 限制 | 状态 |
|---|------|------|
| P1 | **用户必须在系统「设置」里手动启用**这个输入法，它才会出现在输入法列表里。装完不会自动生效。 | 凭记忆 + 检索，**需核对** |
| P2 | 输入法必须注册成 **`extensionAbilities[].type = "inputMethod"`**，且 `exported: true` —— 否则装了也看不到。**不是** `module.type`。 | 检索确认，**与部分网上文章冲突**（那些文章说要写 `module.type: "inputmethod"` + `BIND_INPUT_METHOD` 权限，多篇资料指出是错的） |
| P3 | **版本兼容是大坑**：部分 HarmonyOS NEXT 版本**不开放 InputMethod 类型的 Extension Ability**；有社区反馈旧版 DevEco 也建不出 InputMethod 扩展（建议 DevEco 6.0+）。**开发前先确认目标系统版本支持**。 | 检索，**需核对** |
| P4 | 子类型（中/英/数字键盘）**共用同一个 `InputMethodExtensionAbility`**，靠 `inputMethodAbility.on('setSubtype')` 区分。本目录**尚未监听它** → 切子类型不会换键盘。 | 检索确认 + 已知缺口 |
| P5 | 安全模式：输入法扩展运行在受限沙箱，网络/剪贴板/联系人/定位等能力受限；主应用与输入法扩展**是隔离沙箱**，要共享词库/配置需申请 `dataGroupId`。 | 检索，**需核对** |
| P6 | 输入法实例由系统**按输入会话反复创建/销毁**，进程可能被回收 —— 不能把词库/会话状态当应用级单例长期持有，状态必须能快速重建。 | 检索，代码已按此写（`init()` 有幂等挡、`inputStart` 时清缓冲） |
| P7 | 拉起输入法选择弹框用 `inputMethod.getSetting().showOptionalInputMethods()`（旧接口 `displayOptionalInputMethod` 自 API 9 废弃）。**这是主应用侧**的设置入口，不是输入法扩展里调的。 | 检索确认 |
| P8 | `showOptionalInputMethods` 有跨用户限制（仅 user 0 授权，错误码 12800025）；社区反馈该弹框在某些版本上「点了不生效」，替代方案是 `switchInputMethod` 或 `startAbility` 拉起设置页（`com.huawei.hmos.settings` / `uri: "set_input"`）。 | 检索，**需核对** |
| P9 | 上架前需 strip；release 静态库 9 MB（本机实测），未 strip 会明显撑大 HAP。 | 本机实测（体积） |

---

## 我最不确定的 API

**这是给接手人最省时间的一节。** 以下按「错了会怎样」的代价从高到低排。
每一条我都**无法验证**（没有 SDK），只能是「凭记忆 + 部分检索」。

### 极高：错了整个端起不来

| # | 我写的 | 不确定什么 | 错了的表现 |
|---|--------|-----------|-----------|
| U1 | `InputMethodExtensionAbility` 继承自 `@kit.IMEKit`，重写 `onCreate(want)` / `onDestroy()` | **导入路径与基类名**。旧文档是 `@ohos.inputMethodExtensionAbility`，新的是 `@kit.IMEKit`。生命周期方法签名也可能带别的参数。 | 编译不过。若签名不符会编译过但生命周期不被调用 → 键盘永远不出现 |
| U2 | `inputMethodEngine.getInputMethodAbility()` → `.createPanel(ctx, panelInfo)` → `panel.resize(w,h)` / `.moveTo(x,y)` / `.setUiContent('pages/Index')` / `.destroyPanel()` | **整条创建面板的链路**。方法名、`PanelInfo` 的字段名（`type`/`flag`）、`PanelType.SOFT_KEYBOARD`、`PanelFlag.FLG_FIXED`、以及 `resize`/`moveTo` 的参数顺序与单位（px 还是 vp）全是记忆。 | 键盘面板不出现，或尺寸/位置错乱 |
| U3 | `ability.on('inputStart', (kbController, inputClient) => {})` / `on('inputStop', ...)` / `off(...)` | **回调签名与参数顺序**。我按 `(kbController, client)` 写的。 | 拿不到 `InputClient` → 一个字都上不了屏 |
| U4 | `client.insertText(text)` / `client.deleteForward(n)` | **`InputClient` 的方法名与参数**。`deleteForward(1)` 的「1」是字符数还是码点数也没把握（emoji 可能算 2）。 | 上屏/删除失效 |

### 高：错了功能静默坏掉

| # | 我写的 | 不确定什么 | 错了的表现 |
|---|--------|-----------|-----------|
| U5 | `napi/native_api.h` 是 N-API 的包含路径；`napi_module_register()` 在其中声明 | **头文件名与函数声明位置**。有资料用 `node_api.h`。 | 编译不过（相对容易发现，属好情况） |
| U6 | `napi_module` 结构体字段名：`nm_version` / `nm_flags` / `nm_filename` / `nm_register_func` / `nm_modname` / `nm_priv` / `reserved[4]` | **字段名与顺序**。 | 编译不过，或注册静默失败 → `import` 得到 `undefined` |
| U7 | 注册靠 `__attribute__((constructor))` + `napi_module_register(&mod)` | **构造器属性是否是鸿蒙认的注册方式**（也可能是 `napi_register_module_v1` 之类的固定符号）。 | `import` 得到 `undefined`，且**不报错** —— 最难查的一种 |
| U8 | `napi_get_value_string_utf16(env, v, NULL, 0, &len)` 先问长度、再取内容；`bufsize` 含结尾 NUL | **第五个参数语义（是否含 NUL）**。多算/少算会截断字符串。 | 候选词被截断，或写入越界 |
| U9 | `napi_create_string_utf16` 会**拷贝**数据，所以可以「先 create 再 free」 | **是否确实拷贝**（Node-API 规范说拷贝，鸿蒙实现未验证）。若不拷贝，`take_string` 就是 use-after-free。 | 花屏/崩溃，且**难以复现** |
| U10 | `module.json5` 的 `metadata.name`：我写了 `"ohos.extension.input_method"` | **点 vs 下划线**：V14 文档写 `ohos_extension.input_method`，其它文档写 `ohos.extension.input_method`。**官方文档自己就不一致。** | 输入法能选，但子类型（中/英键盘）不出现 |
| U11 | `srcEntry`（新） | 旧文档是 `srcEntrance`。 | 编译/安装不过 |
| U12 | `input_method_config.json` 的 `mode` 字段取值 `"lower"` | **合法取值完全没查到**。 | profile 解析失败 → 输入法可能不出现 |
| U13 | `@Entry` 页面路径 `'pages/Index'`（`setUiContent` 与 `main_pages.json` 两处一致） | **路径格式**：相对 `src/main/ets`？带不带 `.ets`？ | 面板起来了但是**一片白** |

### 中：错了 UI 不对但键盘能用

| # | 我写的 | 不确定什么 |
|---|--------|-----------|
| U14 | `CanvasRenderingContext2D` 有 `ellipse(cx,cy,rx,ry,rot,a0,a1)`（小欧的嘴和落地投影用它） | 是否提供 `ellipse`。**没有的话改用 4 段 `bezierCurveTo` 的椭圆近似（kappa≈0.5523）**，`arcTo`/`quadraticCurveTo`/`arc` 是确定有的。 |
| U15 | `ctx.arcTo()` 拼圆角矩形（`roundRect` 在 ArkUI 上不保证存在，故没用） | `arcTo` 的参数与行为 |
| U16 | `ctx.measureText(s).width`、`ctx.fillText(s,x,y)`、`ctx.font = 'bold 21px monospace'`、`ctx.textBaseline` | Canvas 字体串能否解析 `monospace`（鸿蒙系统字体是 HarmonyOS Sans，等宽族是否落到它身上不确定）→ 刻字可能变宽/变窄。成功与否**只影响观感**。 |
| U17 | `AppStorage.setOrCreate<T>(key, value)` 与 `@StorageLink` 能双向同步 `string[]` | 数组类型的同步行为（ArkUI 对数组的观测有时要 `@Observed`） |
| U18 | `@Builder` 方法带**多个参数 + 函数参数**（`KeyCap(label, action, weight, active)`） | ArkUI 的 `@Builder` 多参传递在部分版本有「值传递不刷新」的限制。不刷新只影响高亮。 |
| U19 | `display.getDefaultDisplaySync()` 返回的 `width`/`height` **单位是 px**，直接喂给 `panel.resize()` | 单位是否为 px、要不要 `vp2px` 换算。**键盘高度明显不对时先查这里。** |
| U20 | `JSON.parse(raw) as string[]` 在 ArkTS 严格模式下可用 | ArkTS 对 `JSON.parse` 返回值（`Object`）的断言限制 |

### 明确「没查到、也没猜」的

- 鸿蒙是否要求输入法声明某个权限（如 `ohos.permission.INPUT_METHOD`）。
  检索**没有**找到可靠说法；P2 里那个 `BIND_INPUT_METHOD` 疑似错误信息，未采纳。
- `deleteForward(n)` 的 `n` 对代理对（emoji）如何计数。

---

## 已知缺口（**不是 bug，是还没做**）

1. **未监听 `setSubtype`** → 切中/英子类型不会换键盘（P4）。
2. **词库是内置回退库**（`loadDictionary('')`）。完整词库要先把 rawfile
   拷进沙箱再传绝对路径 —— 引擎要文件系统路径，不是资源 ID。
   步骤写在 `KeyboardController.ensureDictionary()` 的注释里。
3. **无符号面板**（`opi_search_symbols` / `opi_symbol_blocks` /
   `opi_symbols_in_block` 未桥接）。Symbol 模式现在只有一页硬编码符号。
4. **无用户词持久化**（`opi_import_user_words` / `opi_export_user_words` /
   `opi_clear_user_words` / `opi_remove_user_word` 未桥接）。
5. **无繁体切换入口**（`opi_load_trad` 未桥接；`OpiMode.Traditional` 有枚举
   但 UI 的轮转里进不去）。
6. **未处理「抬手」事件**（`STATE_RELEASED`）。软键盘一次点击送一次按下事件，
   对路由够用；但要接**物理键盘**就必须补抬手，否则 ⇧ 三态会卡住。
7. **深色主题未接**（`OpiPet` 支持 `dark: true`，但 `Index.ets` 恒传 `false`，
   没读系统主题）。
8. **Android JNI 出口在鸿蒙目标下是死重量**（约 9 MB 里的一部分），
   要拆得改 `crates/**`，不在本轮边界内。

---

## 参考（同为「未被验证」的同类，可对照着读）

- `macos/OpiFFI.h` + `ios/README.md`：同一套 C ABI 的 Apple 侧声明面与草案写法
  （那两份的 Swift 同样一行没编译过，但 `OpiFFI.h` 至少过了 `clang -fsyntax-only`）。
- `crates/fcitx5-opi/cpp/opi_fcitx5.cpp` + 其 `README.md`：**已被真实 fcitx5 加载并测过**
  的同类适配层 —— 里面那张「7 处 API 误写对照表」就是本文件「最不确定的 API」
  这一节的由来。
