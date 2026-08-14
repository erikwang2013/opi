# OPI 原生重构蓝图（弃 Flutter → Compose + 手写 JNI）

> 日期：2026-08-14 · 状态：待批准 · 关联：主规格 [2026-08-12-opi-ime-design.md](../specs/2026-08-12-opi-ime-design.md)、M4 设计（含本机构建环境修复链）、M1/M2/M3/M4 计划
> 决策依据（用户已确认）：FlutterSurfaceView 在 IME 窗口 show/hide 后不渲染（根因：同引擎 attachToFlutterEngine 是 no-op + 同尺寸 surface 重建不出帧，M4 的 onWindowShown detach/attach workaround 仅缓解）；UI 全换 Jetpack Compose；FFI 由 flutter_rust_bridge 换手写 JNI，不引入外部输入法引擎。

## 1. 目标与非目标

### 1.1 目标（M5 完成度全量迁移，逐项见 §4）
- QWERTY 键盘（3 行字母 + 底行 中/英、123、空格、⌫、↵）+ shift 状态机 + 中英模式切换
- 数字面板（Gboard 风格 5 行，数字/标点直传不经过引擎）
- 符号面板（常用/表情/全部 Tab + 拼音/英文搜索，250ms 防抖）+ 表情最近使用 + 搜索态 qwerty 叠盘
- 候选栏：每屏 8 个、翻页（页码钳制、buffer 变化重置）、"无匹配"提示、EN 模式条、缓冲显示
- 设置页（SettingsActivity）：学习开关、清除用户词库（确认对话框）、导出 JSON 到剪贴板
- 动态键盘高度：`0.42×min(宽,高)+168 coerceAtMost(高−168)` + 横屏适配（M4 公式原样保留）
- editorChanged 语义：onStartInput（非 restarting）/onFinishInputView → 清组合串 + 面板回 qwerty + 搜索失焦
- pending buffer 提交策略（开面板前：有候选选首项提交，无候选清掉）；打开面板 view 切换
- 黑屏根因消除：Compose 走普通 View 渲染，无 FlutterSurfaceView

### 1.2 非目标（YAGNI）
- SQLite 学习落盘（维持内存 Learner；单例化后设置页与 IME 共享实例，语义变化见 §2.1）
- composing text（候选栏即组合区，维持 M4 决策）、滑选、无障碍 TalkBack（留待后续）
- 不迁移 `removeUserWord`（frb 有、Dart UI 未用）、不迁移 Candidate.kind/score（UI 只消费 text，§2.3）
- 双拼/五笔、云同步、RIME 等外部引擎一律不引入

## 2. JNI ABI 设计

### 2.1 对象生命周期：进程内静态单例 + JNI_OnLoad（决策 1）
- 现状：frb RustOpaque 实例由 Dart GC 管理，IME isolate 与设置 isolate 各持一个 Api。
- 目标：`crates/opi-ffi` 内 `static SINGLETON: Mutex<Option<Engine>>`，`JNI_OnLoad` 返回 `JNI_VERSION_1_6` 并 `RegisterNatives` 注册全部函数（不用名字导出的 `Java_io_...` 命名，避免签名脆断）。
- `load(path)` 显式构建（可重复调用，重载词库）；Kotlin 侧 `OpiEngine` companion 薄封装。
- **语义变更（须记录为偏差）**：单例后 SettingsActivity 与 OpiImeService 共享同一引擎与学习者内存态——设置页的学习开关/清词/导出即时作用于 IME。这与 M6 SQLite 统一方向一致，视为改进；相关 JVM 测试按新语义写。
- **panic 防线**：每个 `extern "C"` 入口包 `std::panic::catch_unwind`，出错返回 null/哨兵，Kotlin 侧 null → 降级直传（沿用 M3/M4 "引擎不可用静默回退"语义）；`inputKey` 永不 panic。

### 2.2 字符串约定：UTF-16 路线（防 emoji 陷阱，关键）
- 入参：`GetStringChars`（jchar* UTF-16）→ `String::from_utf16`；出参：Rust String → UTF-16 Vec → `NewString`。
- **禁止** `GetStringUTFChars`/`NewStringUTF`：JNI modified UTF-8 把 emoji 等增补平面字符的代理对编码成 6 字节 CESU-8，Rust `from_utf8` 直接拒绝 → 候选/符号中 emoji 全部损坏。提供两个 ~20 行 helper（`jstring_to_rust` / `rust_to_jstring`）并配单测。
- 输入边界校验与 frb 版一致：inputKey 仅收单字符（空/多字符/非 ASCII 拒绝，引擎内小写化）。

### 2.3 函数清单（对照 frb Api）

| JNI 函数 | JNI 签名（Kotlin 侧） | 对应 frb | 说明 |
|---|---|---|---|
| load | `(JNIEnv, JClass, jstring path) -> jboolean` | Api.load/loadFallback | load_or_fallback 语义：坏路径静默回退内置 35 词；仅内置损坏才 false |
| inputKey | `(env, cls, jstring ch) -> jstring` | inputKey | 返回需提交文本（空串=无提交）；空格在引擎内拦截 |
| backspace | `() -> void` | backspace | |
| clear | `() -> void` | clear | |
| select | `(jint index) -> jstring` | select | 越界返回空串（旧语义） |
| switchMode | `(jint mode) -> void` | switchMode | 0=Pinyin 1=English 2=Number 3=Symbol |
| setShift | `(jboolean on) -> void` | setShift | 仅 English 模式生效 |
| inputSpace | `() -> jstring` | inputSpace | 拼音选首候选 / 其余模式提交缓冲 |
| candidates | `(jint limit) -> jobjectArray` | candidates | **仅文本数组**（kind/score UI 不用，裁剪；如需 emoji 样式再加 candidatesKinds: ByteArray） |
| buffer | `() -> jstring` | buffer | |
| mode | `() -> jint` | mode | |
| searchSymbols | `(jstring keyword) -> jobjectArray` | searchSymbols | 文本数组 |
| symbolBlocks | `() -> jstring` | symbolBlocks | **JSON 字符串**（serde_json 现成：`[{id,start,end,name,common}]`），低频调用 |
| symbolsInBlock | `(jshort id) -> jobjectArray` | symbolsInBlock | 文本数组（name/keywords UI 不用） |
| learnerEnabled | `() -> jboolean` | learnerEnabled | |
| setLearner | `(jboolean on) -> void` | setLearner | |
| clearUserWords | `() -> void` | clearUserWords | |
| exportUserWords | `() -> jstring` | exportUserWords | 学习 JSON（剪贴板导出） |

### 2.4 cdylib 构建（决策 2：复用 rust_builder 而非自研 gradle 集成）
- `crates/opi-ffi` Cargo.toml：移除 `flutter_rust_bridge`，新增 `jni = "0.21"`（纯 Rust crate，crates.io 可离线解析——本机 rust-lang.org 网络已验证可用）；`crate-type = ["cdylib", "lib"]`（lib 供 cargo test）；cdylib 名 `libopi_ffi.so`。
- Gradle 侧复用 **rust_builder（cargokit 独立版）**：M4 已对 Gradle 9 `project.exec` 移除问题打过 `ExecOperations` 补丁、compileSdk 34 补丁（本机构建硬约束），从 `flutter/app/android/rust_builder` 整体迁至 `android/rust_builder`，Android 工程独立 includeBuild。**不新写** cargo Exec task（省一条踩坑路径）；删 flutter 后该插件与 flutter 引擎无耦合，可独立工作。
- 目标 triples 已装（M4）：aarch64-linux-android / armv7-linux-androideabi / x86_64-linux-android；ndkVersion 27.0.12077973 固定不变。
- 迁移后 `android/settings.gradle.kts`：删除 flutter plugin loader 段、本地 m2（`file:///home/erik/opi_local_m2` 只含 flutter 工件，不再需要）、`dev.flutter.flutter-gradle-plugin`；保留 aliyun 三镜像优先 + 不写 `google()` 的模式。

## 3. Compose IME 架构

### 3.1 Compose 进 IME 窗口（非 Activity 窗口的已知问题与解法）
- `onCreateInputView()`：创建 `ComposeView` → `setContent { OpiImeRoot() }` → 返回并缓存（沿用 M4 "view 缓存复用 + 防重入"策略）。纯 View 渲染，黑屏根因天然消失。
- **生命周期（关键）**：IME 窗口无 Activity，ComposeView 缺 `ViewTreeLifecycleOwner`。方案：
  1. Service 持 `LifecycleRegistry` + 最小 `LifecycleOwner` 实现；
  2. onCreateInputView 时 `composeView.setViewTreeLifecycleOwner(owner)` 并置 `CREATED`；`onWindowShown → RESUMED`、`onWindowHidden → STARTED`；
  3. 同时 `setViewTreeSavedStateRegistryOwner`（最小 RegistryOwner 实现，空 save 即可）。
- **不用 ViewModel（决策 3）**：IME 无 ViewModelStoreOwner，且窗口反复重建会丢 store。面板状态放 Service 持有的 `ImeState` 普通类（`mutableStateOf` 驱动 Compose），经 CompositionLocal 下发；view 缓存使状态跨窗口重建存活。规避 SavedStateRegistry/ViewModelStore 全套问题。
- 搜索框注意：IME 窗口内 TextField/BasicTextField 不会（也不能）唤起系统键盘，焦点态由 ImeState 管理并叠 qwerty 搜索盘（固定 4 行 × 44dp 区，176px，沿用 Dart 注释的触控 slop 理由）。

### 3.2 键盘高度 / Insets 策略（沿用，不重设计）
- `keyboardHeight() = (0.42 × min(屏宽,屏高) + 168).coerceAtMost(屏高 − 168)`；`onConfigureWindow` `win.setLayout(MATCH_PARENT, keyboardHeight())` 原样迁移。
- Compose 根铺满窗口即得到正确高度，**不读 WindowInsets**（窗口高度由 onConfigureWindow 决定）；键盘底部安全区 48dp 保留（与 `BOTTOM_SAFE_PX=168` 同源常量，曲面屏圆角 r≈147px 注释一并迁移）。
- 击键路径无通道：`KeyRouter` 直接调 EngineController → 提交走 `currentInputConnection`（commitWithRetry 50ms、按码点删、IME_MASK_ACTION 读 action + 0 回退换行——全部原样迁移）。
- editorChanged：onStartInput/onFinishInputView 直接调 `imeState.onEditorChanged()`（清 buffer + 面板回 qwerty + 搜索失焦），取代 MethodChannel 通知。

### 3.3 面板状态机归属（决策 3 续）
`ImeState`（Service 持有，纯 JVM 可测）：view(qwerty/number/symbol)、searchActive/searchQuery、250ms 防抖、editorChanged 重置、pending buffer 提交策略、搜索态叠盘。逻辑与 ime_main.dart 逐条对应。

## 4. 功能迁移清单（现状 lib/ → Compose 目标）

| 现状（Dart） | 目标（Kotlin） | 关键行为 | 测试 |
|---|---|---|---|
| keyboards/qwerty.dart | keyboard/QwertyKeyboard.kt | 3 行字母（第 3 行含 ⇧）+ 底行；模式标签 中/英；123 **不挂长按**（长按吞 tap 导致面板打不开）；shift 高亮 | JVM 逻辑 + Robolectric 渲染冒烟 |
| keyboards/key_button.dart | keyboard/KeyButton.kt | 键帽 + tap/longPress + highlighted + flex | 同上 |
| keyboards/number_pad.dart | keyboard/NumberPad.kt | 5 行 Gboard 布局；数字/标点直传；ABC/?123/空格/⌫/↵ | 同上 |
| keyboards/symbol_panel.dart | keyboard/SymbolPanel.kt | Tab 常用/表情/全部；250ms 防抖；搜索结果网格；emoji 记录最近 | 同上 |
| keyboards/symbol_catalog.dart | keyboard/SymbolCatalog.kt | FFI 查询缓存（blocks/entries）+ 最近 emoji 内存表 | JVM 单测 |
| candidates/candidate_bar.dart | candidate/CandidateBar.kt | 缓冲 + 8/页 + ‹ n/m › 翻页 + 页码钳制 + 无匹配 + EN 模式条（44dp） | JVM + Robolectric |
| engine/engine_controller.dart | engine/EngineController.kt | JNI 封装；候选翻页（fetchLimit=64、buffer 变化重置页码、钳制）；shift off/single/lock 状态机 + single 消费复位；learner 透传 | JVM 单测（compose-runtime 可在纯 JVM 跑） |
| ime/ime_router.dart | ime/KeyRouter.kt | 按键分流：英文空缓冲字母直传；buffer 非空空格/回车走引擎；候选选择提交 | JVM 单测 |
| platform/ime_channel.dart | **删除**（直接调用 service） | commitText/deleteBackward/performEnter/editorChanged 变直接调用 | — |
| ime/ime_main.dart | ime/ImeState.kt + ime/ImeScreen.kt | 视图状态机；pending buffer 提交；editorChanged 重置；搜索叠盘；候选栏显示条件（english 恒显/拼音有内容才显/面板视图不显） | JVM + Robolectric |
| settings/settings_page.dart | settings/SettingsScreen.kt | 学习开关 / 清词确认对话框 / 导出到剪贴板（ClipboardManager） | JVM + Robolectric |
| main.dart / MainActivity | settings/SettingsActivity.kt | ComponentActivity + setContent（弃 FlutterActivity） | 构建验收 |
| src/rust/*（frb 生成物） | **删除** | — | — |

行为细节必须逐条保留：shift 仅 English 显示/可点（pinyin 传 null 禁用以防状态泄漏进 English，S1）；切英文前清残留拼音；候选栏 english 模式退化为模式条；开面板前 `_commitPendingBuffer`（有候选选首项、无候选清掉）；`_onEditorChanged` 取消组合串不提交。

## 5. 模块划分（每文件 <500 行）

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
android/app/src/main/assets/luna.opid     # 词库迁移目标（§8）
android/rust_builder/                     # cargokit 独立版迁入（M4 补丁已含）
crates/opi-ffi/src/jni.rs                 # extern "C" 全函数 + catch_unwind + JNI_OnLoad
crates/opi-ffi/src/jni_util.rs            # UTF-16 转换 helper（单测覆盖）
```

## 6. 迁移步骤顺序（每步可独立验证）

- **Step 0 环境验证**：settings.gradle.kts 临时工程解析 Compose 坐标（§8 清单）成功；`cargo test --workspace` 确认 114 全绿基线。
- **Step 1 JNI 层先行**：opi-ffi 换 jni 实现（保留 `#[cfg(test)]` 直接测 Rust 侧语义）+ UTF-16 helper 单测；`cargo test --workspace && clippy` 全绿；**宿主 JVM 冒烟**（§7）：host cdylib + javac Main.java + java 跑 load/inputKey/select/export 真 JNI 链路。此步产出可独立验收的 ABI。
- **Step 2 Compose IME shell**：gradle 工程去 flutter 插件、接 rust_builder；ComposeView 进 IME 窗口 + lifecycle registry + keyboardHeight + 空根 UI。验证：`gradle assembleDebug` 成功（黑屏问题在架构上消失，无需真机证明）。
- **Step 3 键盘**：KeyButton/QwertyKeyboard/KeyRouter/EngineController + 候选栏骨架；JVM 单测（路由/分页/shift）+ Robolectric 冒烟 + 构建。
- **Step 4 面板**：NumberPad/SymbolPanel/SymbolCatalog + 搜索叠盘 + pending buffer + editorChanged。
- **Step 5 设置**：SettingsActivity（ComponentActivity + Compose）+ 单例语义测试。
- **Step 6 集成与收尾**：全量门禁（cargo + JVM/Robolectric + assembleDebug）；**最后一步删除 flutter/ 目录**（含 frb 生成物、flutter 相关 gradle 段、opi_local_m2 引用）；更新 README 与主规格里程碑；真机验收列为待办（无设备，同 M4）。

## 7. 测试策略

- **cargo 门禁保留**：`cargo test --workspace`（114）+ `cargo clippy --workspace --all-targets -- -D warnings` 零警告。
- **JNI 测试（host 侧可行路径）**：
  1. `jni = { version = "0.21", features = ["mock"] }` 单测 UTF-16 helper 与注册表（不执行真实调用）；
  2. **宿主 JVM 冒烟（推荐主路径）**：`cargo build`（host cdylib）+ 仓库内 `android/jni_smoke/Main.java`（`System.load` + 各函数调用断言）+ `java` 运行——本机 JDK 17 现成（AGP 9 依赖），完全离线，真 JNI 调用链全通；
  3. Android 侧仅构建期验证（无设备，真机列待办）。
- **Compose UI 测试**：**Robolectric 可行**（org.robolectric:robolectric 经 aliyun public 镜像 = maven central 同步；android-all 大包首次 ~100MB 下载）配合 `androidx.compose.ui:ui-test-junit4` 跑渲染/交互冒烟。若 android-all 下载失败，降级为纯 JVM 逻辑测试 + 构建验收（按 M4 惯例记录偏差）。
- **逻辑层 JVM 单测（零 Android 依赖，主力）**：ImeState/KeyRouter/EngineController（compose-runtime 为纯 Kotlin 多平台工件，mutableStateOf 可在 JVM 单测运行）——覆盖分页、shift 状态机、按键分流、pending buffer、editorChanged。
- 验收清单：gradle assembleDebug 成功；cargo 全绿；JVM + Robolectric 测试全绿；APK 内无 libflutter.so（aapt 验证删净）。

## 8. 风险与对策

| 风险 | 对策 |
|---|---|
| 离线 Compose 依赖解析 | 坐标：`org.jetbrains.kotlin.plugin.compose:2.3.20`（与 Kotlin 同版本，AGP 9 已配 2.3.20）、`androidx.compose:compose-bom:2025.06.00+`（选 aliyun google 镜像已同步版本，Step 0 先解析验证）、`androidx.activity:activity-compose`、`androidx.compose.material3`、`ui-test-junit4`；坚持 aliyun 镜像优先 + 不写 `google()` 模式 |
| Kotlin 2.3.20 的 compose compiler 插件 | 2.0 起编译器随 Kotlin 发布，插件坐标即 Kotlin 版本号，无版本漂移问题 |
| NDK 27 固定（环境仅此版） | ndkVersion 27.0.12077973 原样保留；rustup targets 已装 |
| luna.opid asset 迁移 | `data/generated/luna.opid` 复制到 `android/app/src/main/assets/`；`EngineLoader.kt` 用 AssetManager 解压到 `filesDir/luna.opid`，**size 校验不一致即重拷**（镜像 engine_controller._loadLuna 的升级重拷逻辑）；坏路径回退内置 35 词（引擎侧语义不变）。asset 在 AGP 默认打包，无需额外配置 |
| JNI 字符串编码（emoji 损坏） | §2.2 UTF-16 路线 + helper 单测（含 😄 往返断言） |
| Compose 非 Activity 窗口生命周期 | §3.1 LifecycleRegistry + RegistryOwner 最小实现；不用 ViewModel；view 缓存策略沿用 |
| onCreateInputView 重入 / onDestroy | view/引擎缓存复用（M4 P1）；onDestroy 清理 registry + 撤 retryRunnable（M4 逻辑原样） |
| 删 flutter 时机 | 最后一步（Step 6）；此前 flutter/ 保留作对照与回退 |
| Robolectric android-all 下载失败 | 降级纯 JVM 逻辑测试 + 构建验收，记录偏差 |
| 单例共享状态语义变化 | §2.1 明确记录为偏差并更新测试（与 M6 方向一致） |

## 9. 里程碑映射与验收

- 对应主规格"UI 可替换"原则落地：引擎与 UI 经"击键 → 候选"协议解耦不变，仅传输层 frb → JNI。
- 验收：① gradle assembleDebug 成功且 APK 无 flutter 残留；② cargo 门禁全绿；③ JVM/Robolectric 测试全绿；④ 真机验收推迟（待办，同 M4）。

## 10. 偏差记录预留

执行中偏离本蓝图（签名、镜像坐标、robolectric 降级等）按 M2/M3/M4 惯例在蓝图与 M4 设计文档追加"实现偏差"节。
