# OPI 多端重构实施计划（M6）— 弃 Flutter → 三路并行原生

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 OPI 从 Flutter 单端迁移为多端原生架构：Android IME（Jetpack Compose）+ Linux fcitx5 插件 + Windows TSF 插件（CMP 候选窗），iOS 预留 C ABI（M7），全部共享同一 Rust 引擎，不引入任何外部输入法引擎。

**Architecture:** 一套代码 = Rust 引擎（engine_core/engine_data 四端共享）+ 输入逻辑语义一套 + Windows 候选窗 CMP UI；键盘 UI 各端原生。FFI 由 flutter_rust_bridge 换手写 JNI（Android）+ C ABI（iOS 预留）；Linux/Windows 插件直接依赖 engine_core（无 FFI 层）。三路并行：路 A Android / 路 B fcitx5 / 路 C TSF。

**Tech Stack:** Rust（engine_core 已有）+ jni crate + Jetpack Compose + Compose Multiplatform（仅候选窗）+ fcitx5 Rust 绑定（Linux）+ windows-rs（TSF COM）+ NDK 27 + aliyun 镜像构建链。

> 依据：设计文档 [2026-08-14-opi-multi-platform-design.md](../specs/2026-08-14-opi-multi-platform-design.md)（已确认）。旧版单端蓝图内容保留为本计划路 A 主体。

---

## 0. 三路并行总览

```
Step 0（共享）：环境验证 — cargo 基线 + 绑定可获取性（一次完成，阻塞三路）
        │
   ┌────┼──────────┐
   ▼    ▼          ▼
 路 A  路 B      路 C
Android fcitx5   TSF+CMP
 M6a    M6b      M6c
```

依赖关系：三路均直接依赖 engine_core（已有，不动核心）；opi-ffi 双 ABI 只服务路 A 与 iOS（M7）。路 B/C 不依赖 opi-ffi。

---

# 路 A：Android IME（M6a）

## A1. 目标与非目标（原单端蓝图 §1，原样保留）

### 目标
- QWERTY 键盘（3 行字母 + 底行 中/英、123、空格、⌫、↵）+ shift 状态机 + 中英模式切换
- 数字面板（Gboard 风格 5 行，数字/标点直传不经过引擎）
- 符号面板（常用/表情/全部 Tab + 拼音/英文搜索，250ms 防抖）+ 表情最近使用 + 搜索态 qwerty 叠盘
- 候选栏：每屏 8 个、翻页（页码钳制、buffer 变化重置）、"无匹配"提示、EN 模式条、缓冲显示
- 设置页（SettingsActivity）：学习开关、清除用户词库（确认对话框）、导出 JSON 到剪贴板
- 动态键盘高度：`0.42×min(宽,高)+168 coerceAtMost(高−168)` + 横屏适配（M4 公式原样保留）
- editorChanged 语义：onStartInput（非 restarting）/onFinishInputView → 清组合串 + 面板回 qwerty + 搜索失焦
- pending buffer 提交策略（开面板前：有候选选首项提交，无候选清掉）
- 黑屏根因消除：Compose 走普通 View 渲染，无 FlutterSurfaceView

### 非目标（YAGNI）
- SQLite 学习落盘（内存 Learner；单例化后设置页与 IME 共享实例，语义变化见 A2.1）
- composing text（候选栏即组合区）、滑选、无障碍 TalkBack
- 不迁移 `removeUserWord`、不迁移 Candidate.kind/score
- 双拼/五笔、云同步、RIME 等外部引擎一律不引入

## A2. JNI ABI 设计（原蓝图 §2 + 双 ABI 扩展）

### A2.1 对象生命周期：进程内静态单例 + JNI_OnLoad
- `crates/opi-ffi` 内 `static SINGLETON: Mutex<Option<Engine>>`，`JNI_OnLoad` 返回 `JNI_VERSION_1_6` 并 `RegisterNatives`（不用 `Java_io_...` 命名导出）。
- `load(path)` 显式构建（可重复调用，重载词库）；Kotlin 侧 `OpiEngine` companion 薄封装。
- **语义变更（记录为偏差）**：单例后 SettingsActivity 与 OpiImeService 共享同一引擎与学习者内存态。
- **panic 防线**：每个 `extern "C"` 入口包 `std::panic::catch_unwind`，出错返回 null/哨兵，Kotlin 侧 null → 降级直传；`inputKey` 永不 panic。

### A2.2 字符串约定：UTF-16 路线（防 emoji 陷阱）
- 入参：`GetStringChars`（jchar* UTF-16）→ `String::from_utf16`；出参：Rust String → UTF-16 Vec → `NewString`。
- **禁止** `GetStringUTFChars`/`NewStringUTF`（modified UTF-8 把 emoji 代理对编码成 CESU-8，Rust `from_utf8` 拒绝）。
- 两个 helper（`jstring_to_rust` / `rust_to_jstring`）配单测（含 😄 往返断言）。

### A2.3 JNI 函数清单（17 个，对照 frb Api）

| JNI 函数 | JNI 签名（Kotlin 侧） | 说明 |
|---|---|---|
| load | `(jstring path) -> jboolean` | load_or_fallback 语义：坏路径静默回退内置 35 词 |
| inputKey | `(jstring ch) -> jstring` | 返回需提交文本（空串=无提交）；空格在引擎内拦截 |
| backspace | `() -> void` | |
| clear | `() -> void` | |
| select | `(jint index) -> jstring` | 越界返回空串（旧语义） |
| switchMode | `(jint mode) -> void` | 0=Pinyin 1=English 2=Number 3=Symbol |
| setShift | `(jboolean on) -> void` | 仅 English 模式生效 |
| inputSpace | `() -> jstring` | 拼音选首候选 / 其余模式提交缓冲 |
| candidates | `(jint limit) -> jobjectArray` | 仅文本数组（kind/score UI 不用） |
| buffer | `() -> jstring` | |
| mode | `() -> jint` | |
| searchSymbols | `(jstring keyword) -> jobjectArray` | 文本数组 |
| symbolBlocks | `() -> jstring` | **JSON 字符串**（serde_json 现成：`[{id,start,end,name,common}]`） |
| symbolsInBlock | `(jshort id) -> jobjectArray` | 文本数组 |
| learnerEnabled | `() -> jboolean` | |
| setLearner | `(jboolean on) -> void` | |
| clearUserWords | `() -> void` | |
| exportUserWords | `() -> jstring` | 学习 JSON（剪贴板导出） |

### A2.4 C ABI 出口（iOS M7 预留，本轮实现）
- 同 crate 增加 `#[no_mangle] extern "C"` 同名函数，签名约定：字符串为 `(const uint16_t* utf16, usize len) -> struct { uint16_t* ptr; usize len; }`（返回分配在 Rust 侧，调用方 `opi_ffi_free_string` 释放）。
- 函数清单与 A2.3 一一对应，测试：Rust 侧单测 + host 直调。

### A2.5 cdylib 构建（复用 rust_builder，不新写 gradle cargo task）
- `crates/opi-ffi` Cargo.toml：移除 `flutter_rust_bridge`，新增 `jni = "0.22"`（E0 实测 crates.io 解析 0.22.4）；`crate-type = ["cdylib", "lib"]`；cdylib 名 `libopi_ffi.so`。
- Gradle 复用 **rust_builder（cargokit 独立版）**：从 `flutter/app/android/rust_builder` 整体迁至 `android/rust_builder`（M4 已打 Gradle 9 ExecOperations + compileSdk 34 补丁）。
- NDK 27.0.12077973 固定；目标 triples 已装（aarch64-linux-android/armv7-linux-androideabi/x86_64-linux-android）。

## A3. Compose IME 架构（原蓝图 §3，原样保留）

### A3.1 Compose 进 IME 窗口
- `onCreateInputView()`：创建 `ComposeView` → `setContent { OpiImeRoot() }` → 返回并缓存（沿用 M4 view 缓存防重入）。
- 生命周期：Service 持 `LifecycleRegistry` + 最小 `LifecycleOwner`；`setViewTreeLifecycleOwner(owner)` 置 `CREATED`；`onWindowShown → RESUMED`、`onWindowHidden → STARTED`；同时 `setViewTreeSavedStateRegistryOwner`（最小实现，空 save）。
- **不用 ViewModel**：面板状态放 Service 持有的 `ImeState`（`mutableStateOf`）经 CompositionLocal 下发。
- 搜索框：IME 窗口内 TextField 不唤起系统键盘，焦点态由 ImeState 管理并叠 qwerty 搜索盘（固定 4 行 × 44dp 区 = 176px）。

### A3.2 键盘高度 / Insets
- `keyboardHeight() = (0.42 × min(屏宽,屏高) + 168).coerceAtMost(屏高 − 168)`；`onConfigureWindow` `win.setLayout(MATCH_PARENT, keyboardHeight())` 原样迁移。
- Compose 根铺满窗口，不读 WindowInsets；键盘底部安全区 48dp（BOTTOM_SAFE_PX=168 同源）。
- 击键路径：`KeyRouter` 直接调 EngineController → 提交走 `currentInputConnection`（commitWithRetry 50ms、按码点删、IME_MASK_ACTION 读 action + 0 回退换行）。
- editorChanged：onStartInput/onFinishInputView 直接调 `imeState.onEditorChanged()`。

### A3.3 面板状态机归属
`ImeState`（Service 持有，纯 JVM 可测）：view(qwerty/number/symbol)、searchActive/searchQuery、250ms 防抖、editorChanged 重置、pending buffer 提交策略、搜索态叠盘。逻辑与 ime_main.dart 逐条对应。

## A4. 功能迁移清单（lib/ → Kotlin，逐条保留行为）

| 现状（Dart） | 目标（Kotlin） | 关键行为 |
|---|---|---|
| keyboards/qwerty.dart | keyboard/QwertyKeyboard.kt | 3 行字母 + 底行；模式标签 中/英；123 **不挂长按**；shift 高亮 |
| keyboards/key_button.dart | keyboard/KeyButton.kt | 键帽 + tap/longPress + highlighted + flex |
| keyboards/number_pad.dart | keyboard/NumberPad.kt | 5 行 Gboard 布局；数字/标点直传；ABC/?123/空格/⌫/↵ |
| keyboards/symbol_panel.dart | keyboard/SymbolPanel.kt | Tab 常用/表情/全部；250ms 防抖；搜索结果网格；emoji 记录最近 |
| keyboards/symbol_catalog.dart | keyboard/SymbolCatalog.kt | FFI 查询缓存 + 最近 emoji 内存表 |
| candidates/candidate_bar.dart | candidate/CandidateBar.kt | 缓冲 + 8/页 + ‹ n/m › 翻页 + 页码钳制 + 无匹配 + EN 模式条（44dp） |
| engine/engine_controller.dart | engine/EngineController.kt | JNI 封装；候选翻页（fetchLimit=64、buffer 变化重置页码、钳制）；shift off/single/lock 状态机 + single 消费复位；learner 透传 |
| ime/ime_router.dart | ime/KeyRouter.kt | 按键分流：英文空缓冲字母直传；buffer 非空空格/回车走引擎；候选选择提交 |
| platform/ime_channel.dart | **删除**（直接调用 service） | commitText/deleteBackward/performEnter/editorChanged 变直接调用 |
| ime/ime_main.dart | ime/ImeState.kt + ime/ImeScreen.kt | 视图状态机；pending buffer 提交；editorChanged 重置；搜索叠盘；候选栏显示条件（english 恒显/拼音有内容才显/面板视图不显） |
| settings/settings_page.dart | settings/SettingsScreen.kt | 学习开关 / 清词确认对话框 / 导出到剪贴板（ClipboardManager） |
| main.dart / MainActivity | settings/SettingsActivity.kt | ComponentActivity + setContent（弃 FlutterActivity） |
| src/rust/*（frb 生成物） | **删除** | — |

行为细节必须逐条保留：shift 仅 English 显示/可点（pinyin 传 null 禁用以防状态泄漏进 English，S1）；切英文前清残留拼音；候选栏 english 模式退化为模式条；开面板前 `_commitPendingBuffer`（有候选选首项、无候选清掉）；`_onEditorChanged` 取消组合串不提交。

## A5. Android 模块划分（每文件 <500 行）

```
android/app/src/main/kotlin/io/opi/input/
  OpiImeService.kt        # 宿主：view 创建/缓存、keyboardHeight、onConfigureWindow、
                          #   commitWithRetry、按码点删、performEnter action、lifecycle registry、editorChanged 桥
  ime/ImeState.kt         # 面板状态机 + 搜索态 + pending buffer + editorChanged 重置
  ime/KeyRouter.kt        # 按键分流规则
  ime/ImeScreen.kt        # 根 Composable：候选栏 + 键盘切换 + 搜索叠盘
  engine/EngineController.kt  # JNI 封装 + 分页 + shift 状态机 + learner 透传（compose state）
  jni/OpiEngine.kt        # JNI 声明（System.loadLibrary + external fun）
  jni/EngineLoader.kt     # luna.opid：assets 解压 filesDir + size 校验重拷 + load 编排/回退
  keyboard/QwertyKeyboard.kt / NumberPad.kt / SymbolPanel.kt / KeyButton.kt / SymbolCatalog.kt
  candidate/CandidateBar.kt
  settings/SettingsActivity.kt / SettingsScreen.kt
android/app/src/main/assets/luna.opid     # 词库迁移目标（data/generated/luna.opid 复制）
android/rust_builder/                     # cargokit 独立版迁入（M4 补丁已含）
crates/opi-ffi/src/jni.rs                 # extern "C" 全函数 + catch_unwind + JNI_OnLoad
crates/opi-ffi/src/jni_util.rs            # UTF-16 转换 helper（单测覆盖）
crates/opi-ffi/src/cabi.rs                # C ABI 出口 + free_string
```

## A6. 任务清单（TDD，每步可独立验证）

### Task A0: 环境验证（阻塞三路的共享前置）

**Files:**
- 验证：`cargo test --workspace`（基线 115 全绿）
- 验证：`cargo add jni@0.21 --dry-run`（离线可解析）

- [ ] **Step 1: cargo 基线确认**
  运行：`cargo test --workspace` → 115 passed，0 failed。
- [ ] **Step 2: jni crate 离线可解析确认**
  运行：`cargo add jni --dry-run -p opi-ffi`（在临时分支）→ 无网络错误。
- [ ] **Step 3: Compose 坐标解析确认（沿用 Step 0 结论）**
  settings.gradle.kts 临时工程已解析 `org.jetbrains.kotlin.plugin.compose:2.3.20`、compose-bom、activity-compose（此前已验证）。
- [ ] **Step 4: 路 B/C 绑定可获取性验证（并行任务）**
  运行：`cargo add fcitx5 --dry-run` 与 `cargo add windows --dry-run` → 记录结果，失败则按 A0-风险对策降级。
- [ ] **Step 5: 提交**
  ```bash
  git add -A && git commit -m "chore(m6): 环境验证结论（jni/fcitx5/windows 绑定可获取性）"
  ```

### Task A1: opi-ffi 双 ABI 重构（替换 frb）

**Files:**
- Modify: `crates/opi-ffi/Cargo.toml`
- Create: `crates/opi-ffi/src/jni_util.rs` / `crates/opi-ffi/src/jni.rs` / `crates/opi-ffi/src/cabi.rs`
- Test: `crates/opi-ffi/src/jni_util.rs`（#[cfg(test)] 内联）、`crates/opi-ffi/tests/cabi_test.rs`

- [ ] **Step 1: 写 UTF-16 helper 失败测试**
  ```rust
  // jni_util.rs #[cfg(test)]
  #[test]
  fn utf16_roundtrip_emoji() {
      let s = "中文😄a";
      let units: Vec<u16> = s.encode_utf16().collect();
      assert_eq!(String::from_utf16_lossy(&units), s);
  }
  ```
- [ ] **Step 2: 运行确认失败**：`cargo test -p opi-ffi` → 编译错误（jni_util 不存在）。
- [ ] **Step 3: 实现 jni_util.rs**
  ```rust
  pub unsafe fn jstring_to_rust(env: &JNIEnv, s: JString) -> Option<String> {
      let chars = env.get_string_chars(&s).ok()?;  // jchar* UTF-16
      String::from_utf16(chars.as_slice()).ok()
  }
  pub fn rust_to_jstring(env: &JNIEnv, s: &str) -> Result<JString, Error> {
      let units: Vec<u16> = s.encode_utf16().collect();
      env.new_string(&units)  // NewString 走 UTF-16
  }
  ```
  （jni crate 0.21 的 API：`get_string_chars` 返回 `JStringChars`、`new_string` 接受 `Into<JNIString>`；编译错误即视为 API 差异信号，以 0.21 实际签名为准，Step 4 的 emoji 往返测试兜底语义。）
- [ ] **Step 4: 测试通过**：`cargo test -p opi-ffi` → 含 emoji 往返全绿。
- [ ] **Step 5: 实现 jni.rs 17 函数 + JNI_OnLoad**
  每个函数模式：
  ```rust
  #[no_mangle]
  pub extern "system" fn Java_io_opi_ffi_load(env: JNIEnv, _class: JClass, path: JString) -> jboolean {
      std::panic::catch_unwind(|| {
          let path = jstring_to_rust(&env, path);
          // 调用 ENGINE.singleton() 逻辑
          ... jboolean
      }).unwrap_or(0)
  }
  ```
  **注意**：若采用 `JNI_OnLoad` RegisterNatives（推荐，防签名脆断），函数名可为任意非 `Java_` 名（如 `opijni_load`），由注册表绑定：
  ```rust
  #[no_mangle]
  pub extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut c_void) -> jint {
      let env = unsafe { vm.get_env().expect("env") };
      let methods = &[
          JNINativeMethod { name: c"load".as_ptr() as _, signature: c"(Ljava/lang/String;)Z".as_ptr() as _, fn_ptr: opijni_load as _ },
          // ... 17 项，签名对照 A2.3
      ];
      let class = env.find_class("io/opi/input/jni/OpiEngine").unwrap();
      env.register_natives(class, methods).unwrap();
      JNI_VERSION_1_6
  }
  ```
- [ ] **Step 6: 实现 cabi.rs C 出口（iOS 预留）**
  ```rust
  #[repr(C)]
  pub struct OpiString { pub ptr: *const u16, pub len: usize }
  #[no_mangle] pub extern "C" fn opi_load(path: *const u16, len: usize) -> bool { ... }
  #[no_mangle] pub extern "C" fn opi_ffi_free_string(s: OpiString) { ... }
  ```
- [ ] **Step 7: host JVM 冒烟（主验证路径）**
  1. `cargo build -p opi-ffi`（host cdylib → `target/debug/libopi_ffi.so`）
  2. 创建 `android/jni_smoke/Main.java`：
     ```java
     public class Main {
         static { System.load("/path/to/target/debug/libopi_ffi.so"); }
         public static void main(String[] args) {
             OpiEngine.load("/tmp/luna.opid");       // 真实 JNI 调用链
             OpiEngine.inputKey("w");
             String[] c = OpiEngine.candidates(8);
             assert c.length > 0 : "no candidates";
             String out = OpiEngine.select(0);
             assert !out.isEmpty();
             System.out.println("SMOKE-OK: " + out);
         }
     }
     ```
     （`OpiEngine` 即 A5 中 `jni/OpiEngine.kt` 的 Java 版声明：`System.loadLibrary` + native 方法。）
  3. `javac -d /tmp/smoke android/jni_smoke/Main.java && java -cp /tmp/smoke Main` → `SMOKE-OK`。
- [ ] **Step 8: 全量门禁**：`cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` 零警告。
- [ ] **Step 9: 提交**
  ```bash
  git add crates/opi-ffi && git commit -m "feat(ffi): opi-ffi 双 ABI（JNI+C）替换 frb，host JVM 冒烟通过"
  ```

### Task A2: gradle 去 flutter + Compose IME shell

**Files:**
- Create: `android/settings.gradle.kts`（重写）、`android/build.gradle.kts`、`android/app/build.gradle.kts`
- Create: `android/app/src/main/AndroidManifest.xml`（IME service + settings activity）
- Create: `android/app/src/main/kotlin/io/opi/input/jni/OpiEngine.kt`
- Modify: `OpiImeService.kt`（Flutter → ComposeView + lifecycle registry）
- Create: `android/app/src/main/kotlin/io/opi/input/ime/ImeState.kt`（骨架）

- [ ] **Step 1: 迁移 rust_builder**
  复制 `flutter/app/android/rust_builder/` → `android/rust_builder/`；`settings.gradle.kts` `includeBuild("rust_builder")`。
- [ ] **Step 2: 重写 gradle 工程**
  - `android/settings.gradle.kts`：删除 flutter plugin loader 段、本地 m2、`dev.flutter.flutter-gradle-plugin`；保留 aliyun 三镜像优先 + 不写 `google()`。
  - 依赖：compose-bom、`org.jetbrains.kotlin.plugin.compose`（随 Kotlin 版本）、activity-compose、material3、ui-test-junit4、relinker 1.4.5、jni smoke 不需要。
- [ ] **Step 3: 写 OpiEngine.kt（JNI 声明）**
  ```kotlin
  object OpiEngine {
      init { System.loadLibrary("opi_ffi") }
      external fun load(path: String): Boolean
      external fun inputKey(ch: String): String
      // ... 17 个 external fun，签名对照 A2.3
  }
  ```
- [ ] **Step 4: OpiImeService.kt 重写骨架**
  onCreateInputView：`ComposeView` + `setViewTreeLifecycleOwner` + `setContent { OpiImeRoot() }` + 缓存复用；onWindowShown/Hidden 驱动 LifecycleRegistry；keyboardHeight()/onConfigureWindow/commitWithRetry/按码点删/performEnter action 原样迁移（见旧版文件，全量保留）。
- [ ] **Step 5: ImeState.kt 骨架（view 状态机 + editorChanged 重置）**
  空根 UI 即可：`Column { Text("OPI") }` 验证窗口高度。
- [ ] **Step 6: 构建验证**：`cd android && ./gradlew assembleDebug` → BUILD SUCCESSFUL。
- [ ] **Step 7: 提交**
  ```bash
  git add android && git commit -m "build(m6): 独立 Android 工程接 rust_builder，ComposeView 进 IME 窗口"
  ```

### Task A3: 键盘 + 候选栏（核心交互）

**Files:**
- Create: `keyboard/KeyButton.kt` / `keyboard/QwertyKeyboard.kt` / `candidate/CandidateBar.kt`
- Create: `engine/EngineController.kt` / `ime/KeyRouter.kt`
- Test: `engine/EngineControllerTest.kt`、`ime/KeyRouterTest.kt`（纯 JVM）

- [ ] **Step 1: 写 EngineController 分页测试（失败先行）**
  覆盖：fetchLimit=64、buffer 变化重置页码、页码钳制、shift off/single/lock + single 消费复位。
- [ ] **Step 2: 实现 EngineController.kt**（OpiEngine 封装 + mutableStateOf；行为对照 engine_controller.dart）
- [ ] **Step 3: 写 KeyRouter 分流测试**：英文空缓冲字母直传；buffer 非空空格/回车走引擎；候选选择提交。
- [ ] **Step 4: 实现 KeyRouter.kt**（对照 ime_router.dart）
- [ ] **Step 5: 实现 KeyButton/QwertyKeyboard/CandidateBar**（对照 A4 迁移表；123 不挂长按；shift 仅 English 显示）
- [ ] **Step 6: JVM 测试全绿**：`cd android && ./gradlew testDebugUnitTest`
- [ ] **Step 7: 构建 + 提交**：assembleDebug 成功；`git commit -m "feat(ime): qwerty 键盘 + 候选栏 + 引擎路由"`

### Task A4: 面板（数字/符号/搜索叠盘）+ pending buffer

**Files:**
- Create: `keyboard/NumberPad.kt` / `keyboard/SymbolPanel.kt` / `keyboard/SymbolCatalog.kt`
- Modify: `ime/ImeState.kt`（pending buffer 提交、250ms 防抖、搜索态叠盘 176px、editorChanged 重置）

- [ ] **Step 1: 写 ImeState 面板状态机测试**：pending buffer（有候选选首项/无候选清掉）、防抖、editorChanged 清空。
- [ ] **Step 2: 实现 ImeState 完整逻辑**（对照 ime_main.dart）
- [ ] **Step 3: 实现 NumberPad（5 行 Gboard 布局，直传）**
- [ ] **Step 4: 实现 SymbolPanel/SymbolCatalog（Tab + 搜索 + 最近 emoji）**
- [ ] **Step 5: JVM 测试 + 构建**：全绿；assembleDebug 成功。
- [ ] **Step 6: 提交**：`git commit -m "feat(ime): 数字/符号面板 + 搜索叠盘 + pending buffer"`

### Task A5: 设置页 + 单例语义

**Files:**
- Create: `settings/SettingsActivity.kt` / `settings/SettingsScreen.kt`
- Create: `jni/EngineLoader.kt`（luna.opid 解压 + size 校验重拷 + load 编排/回退）

- [ ] **Step 1: EngineLoader 测试（纯 JVM）**：坏路径回退内置 35 词；size 不一致重拷。
- [ ] **Step 2: 实现 EngineLoader.kt**
- [ ] **Step 3: 实现 SettingsActivity（ComponentActivity + setContent）**
- [ ] **Step 4: 实现 SettingsScreen（学习开关/清词确认对话框/导出剪贴板）**
- [ ] **Step 5: 资产迁移**：`data/generated/luna.opid` → `android/app/src/main/assets/luna.opid`。
- [ ] **Step 6: 构建 + 提交**：`git commit -m "feat(settings): 设置页 + luna 资产加载"`

### Task A6: 集成门禁 + 删 flutter

- [ ] **Step 1: 全量门禁**：cargo test/clippy 全绿 + `./gradlew testDebugUnitTest` + assembleDebug。
- [ ] **Step 2: 验证 APK 无 flutter 残留**：`aapt list app-debug.apk | grep -i flutter` → 空。
- [ ] **Step 3: 删除 flutter/ 目录**（含 frb 生成物、opi_local_m2 引用）；更新 README。
- [ ] **Step 4: 提交**：`git commit -m "chore(m6): 删除 flutter/，原生化完成"`

---

# 路 B：Linux fcitx5 插件（M6b）

## B1. 设计要点

- Rust crate `crates/fcitx5-opi`：直接依赖 engine_core（无 FFI 层）；fcitx5 Rust 绑定（`fcitx5` crate，cxx 桥）。
- 插件只做后端：提交字符串、候选列表、翻页；候选窗 UI 由 fcitx5 渲染。
- 词库：打包到插件数据目录，首启解压到 XDG data dir（`~/.local/share/opi/`）；学习数据同目录。
- 加载语义与 Android 对齐：`load(path)` 失败回退内置 35 词；引擎单例天然成立（插件进程单实例）。

## B2. 任务清单

### Task B0: 绑定验证（E0 已执行，2026-08-14）

- [x] **Step 1: 验证 fcitx5 crate 可获取** — **结论：不可用**。`cargo search fcitx5` 无同名 crate；`cargo add fcitx5 --dry-run` 报 not found；github.com 不可达（git ls-remote 失败，与 dl.google.com 同源劫持）→ fcitx5-rs git 依赖死路；fcitx5-dbus 0.1.4 仅是前端 DBus 桥，非 IME 插件 API，弃用。
- [x] **Step 2: 降级定案（记录偏差）**：fcitx5 插件本体用 **C++**（fcitx5 原生插件 API 即 C++），内嵌 Rust cdylib（`crates/fcitx5-opi` 提供 `#[no_mangle] extern "C"` 导出：load/inputKey/backspace/clear/select/switchMode/setShift/inputSpace/candidates/buffer/mode，字符串 UTF-8 + length 约定），C++ 侧做 fcitx5 AddonInstance/InputMethod 胶水。逻辑与胶水分离原则不变（candidate.rs 仍为纯 Rust 可测）。
- [x] **Step 3: 提交** — E0 无文件变更，未提交；本降级结论随 B 路实现提交。

### Task B1: 插件骨架

**Files:**
- Create: `crates/fcitx5-opi/Cargo.toml`、`src/lib.rs`、`src/input_method.rs`、`src/candidate.rs`

- [ ] **Step 1: 写 candidate.rs 翻页测试（纯逻辑）**：8 候选/页、页码钳制、buffer 变化重置（与 Android 语义一致）。
- [ ] **Step 2: 实现 candidate.rs（包装 engine_core 候选查询）**
- [ ] **Step 3: 实现插件入口**：AddonInstance 注册（对照 fcitx5 绑定示例；绑定版本锁定于 Step 1 解析结果）。
- [ ] **Step 4: 单测 + clippy 全绿**：`cargo test -p fcitx5-opi && cargo clippy -p fcitx5-opi -- -D warnings`。
- [ ] **Step 5: 提交**：`git commit -m "feat(fcitx5): 插件骨架 + 候选翻页逻辑"`

### Task B2: 输入法实现（键事件 → 提交）

**Files:**
- Modify: `src/input_method.rs`

- [ ] **Step 1: 写键处理测试**：ASCII 字母入 buffer；空格/回车提交；backspace 按码点删；shift/中英切换语义与 Android KeyRouter 一致。
- [ ] **Step 2: 实现 input_method.rs**：键事件分流对照 A4 KeyRouter 行为表。
- [ ] **Step 3: 单测全绿 + 提交**：`git commit -m "feat(fcitx5): 键事件 → 引擎路由"`

### Task B3: 数据目录 + 打包

- [ ] **Step 1: 实现 XDG 数据初始化**：`~/.local/share/opi/` 建目录；luna.opid 解压（asset 随插件包分发）+ size 校验重拷（对照 EngineLoader 语义）。
- [ ] **Step 2: 集成验证**：fcitx5 环境（fcitx5-diagnose）加载插件，冒烟提交中文（需桌面环境，验收阶段执行）。
- [ ] **Step 3: 提交**：`git commit -m "feat(fcitx5): XDG 数据目录 + luna 词库加载"`

---

# 路 C：Windows TSF 插件 + CMP 候选窗（M6c）

## C1. 设计要点

- Rust crate `crates/tsf-opi`：windows-rs 写 TSF COM（ITfThreadMgr/ITfTextInputProcessor/ITfUIElement）。
- 候选窗：`desktop/` Compose Multiplatform 桌面窗口；TSF 提供候选位置（ITfCandidateListUIElement），CMP 窗跟随；无法跟随时降级固定位置提示（风险表中记录）。
- 逻辑层与 COM 胶水分离：候选/翻页/键路由逻辑纯 Rust 可测（与路 B 共用同一 engine_core 封装思路）。
- 词库/学习：`%LOCALAPPDATA%/opi/`。

## C2. 任务清单

### Task C0: windows-rs 验证（与 A0/B0 并行）

- [ ] **Step 1: 验证 windows crate 可获取**：`cargo add windows --dry-run`。失败（离线）则降级：C++/COM 胶水 + Rust cdylib，记录偏差。
- [ ] **Step 2: 提交**：`git commit -m "chore(m6): windows-rs 绑定验证结论"`

### Task C1: 逻辑层（纯 Rust，先行）

**Files:**
- Create: `crates/tsf-opi/Cargo.toml`、`src/lib.rs`、`src/logic.rs`（候选翻页/键路由，语义对照 A4 行为表）

- [ ] **Step 1: 写 logic.rs 测试**：键路由 + 候选翻页语义（与路 B 测试同源）。
- [ ] **Step 2: 实现 logic.rs**
- [ ] **Step 3: 单测 + clippy 全绿 + 提交**：`git commit -m "feat(tsf): 逻辑层（候选/路由）纯 Rust"`

### Task C2: TSF COM 胶水

**Files:**
- Create: `src/tsf.rs`（ITfTextInputProcessor 最小实现）

- [ ] **Step 1: 实现最小 TSF 接口**：注册、Activate/Deactivate、键事件转发 logic.rs（对照 windows-rs TSF 示例）。
- [ ] **Step 2: 构建验证**：`cargo build -p tsf-opi --target x86_64-pc-windows-msvc`（cross 检查；本机无法跑 Windows 构建则记录为验收待办）。
- [ ] **Step 3: 提交**：`git commit -m "feat(tsf): TSF COM 胶水最小实现"`

### Task C3: CMP 候选窗（desktop/）

**Files:**
- Create: `desktop/settings.gradle.kts`、`desktop/build.gradle.kts`、`desktop/src/main/kotlin/io/opi/candidate/Main.kt`（Compose Desktop 窗口，候选列表 + 翻页）
- Modify: `crates/tsf-opi`（候选数据经 JNI/共享内存或本地 socket 传给候选窗——**先定接口再实现**）

- [x] **Step 1: 定义候选窗通信接口**（方案：TSF 进程内共享内存 + event；或本地 named pipe。选择 named pipe，Rust 侧 windows-rs 现成）。
- [x] **Step 2: 实现 CMP 候选窗**：Compose Desktop 窗口 + 候选列表 + 页码 + 位置跟随（无法跟随降级固定位置）。
- [x] **Step 3: 构建验证**：`./gradlew :desktop:package`（Compose Desktop 打包）→ 成功。
- [x] **Step 4: 提交**：`git commit -m "feat(desktop): CMP 候选窗 + TSF 通信"`

---

# 门禁与验收（tester / reviewer）

### Task T: 全量测试门禁

- [ ] **Step 1: cargo 全 workspace 门禁**：`cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` 全绿。
- [ ] **Step 2: JNI host JVM smoke 重跑**（Task A1 Step 7 命令）→ SMOKE-OK。
- [ ] **Step 3: Android JVM/Robolectric**：`./gradlew testDebugUnitTest` 全绿（Robolectric 下载失败则降级纯 JVM + 记录偏差）。
- [ ] **Step 4: 提交**：`git commit -m "test(m6): 多端门禁全绿"`

### Task R: 审查 + 验收

- [ ] **Step 1: 代码审查**：JNI 注册表签名与 Kotlin 声明一致性、UTF-16 无 NewStringUTF 泄漏、panic 防线覆盖、行为迁移逐条对照（A4 表）。
- [ ] **Step 2: 真机验收（Android）**：IME 启用、输入中文、面板切换、设置页——待办（设备可用时）。
- [ ] **Step 3: 桌面验收（Linux fcitx5 / Windows TSF）**：插件加载冒烟——待办（桌面环境）。

---

# 风险与对策（原蓝图 §8 + 新增）

| 风险 | 对策 |
|---|---|
| 离线 Compose 依赖解析 | 坐标已在 Step 0 验证：compose-bom、activity-compose、material3、ui-test-junit4；aliyun 镜像优先 + 不写 `google()` |
| NDK 27 固定 | ndkVersion 27.0.12077973 原样保留；rustup targets 已装 |
| fcitx5 Rust 绑定不可获取 | **已确认（E0）**：crates.io 无、github 不可达 → C++ 插件 + Rust cdylib（B0 偏差已记录） |
| windows-rs / TSF 复杂度 | C0 验证；逻辑与 COM 胶水分离（C1 先行）；Windows 构建无法本机验证 → 验收待办 |
| CMP 候选窗与 TSF 位置联动 | 降级固定位置提示（C2 记录） |
| JNI 字符串编码（emoji） | UTF-16 helper + 😄 往返单测（A1） |
| Compose 非 Activity 窗口生命周期 | LifecycleRegistry + RegistryOwner 最小实现；不用 ViewModel |
| 删 flutter 时机 | 路 A 最后一步（A6）；此前 flutter/ 保留作对照与回退 |
| 单例共享状态语义变化 | A2.1 明确记录为偏差 |
| 三路并行 token 消耗 | 分步放行：每路 coder 独立交付 + 门禁后合并审查 |

# 里程碑映射与验收

- M6a（路 A）：A0-A6 完成 → assembleDebug + 门禁全绿 + APK 无 flutter。
- M6b（路 B）：B0-B3 完成 → fcitx5 插件冒烟（桌面验收待办）。
- M6c（路 C）：C0-C3 完成 → TSF 插件 + CMP 候选窗构建通过（Windows 运行验收待办）。
- M7（iOS）：opi-ffi C ABI 已就绪，SwiftUI 键盘扩展接入。

# 偏差记录预留

执行中偏离本计划（签名、镜像坐标、绑定降级、Robolectric 降级等）按 M2/M3/M4 惯例在计划与 M4 设计文档追加"实现偏差"节。
