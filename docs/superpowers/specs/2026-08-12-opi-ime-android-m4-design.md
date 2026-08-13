# M4 设计：Android InputMethodService + Flutter 键盘进 IME 窗口

> 日期：2026-08-12
> 状态：已批准（2026-08-12，两节设计均获用户 OK）
> 关联：主规格 [2026-08-12-opi-ime-design.md](./2026-08-12-opi-ime-design.md) 的 M4 里程碑行；前置 [M3 计划](../plans/2026-08-12-opi-ffi-m3.md)（FFI 绑定已完成）

## 1. 目标与范围

**目标**：让 OPI 在 Android 上成为可用的输入法——把 Flutter 键盘 UI 放进 `InputMethodService` 的 IME 窗口，击键经 FFI 走 Rust 引擎，候选经通道回传提交到任意应用。

**范围（M4 做）**：

- Kotlin `OpiImeService : InputMethodService`，`onCreateInputView()` 返回 FlutterView（独立 FlutterEngine，Dart entrypoint `imeMain`）
- 最小 QWERTY 键盘（3 行字母 + 底部功能行），候选栏 top-8 点击选择
- `MethodChannel("opi/ime")` 协议，3 个方法：`commitText(String)`、`deleteBackward()`、`performEnter()`
- 包名迁移：`com.example.app` → `io.opi.input`（applicationId / namespace / MainActivity 路径 / manifest 声明）
- 拼音候选显示在候选栏（不设 composing 文本，候选栏即组合区）

**范围外（YAGNI，留待后续里程碑）**：

- composing text 上屏（`setComposingText`）——候选栏即组合区，M5 之前不做
- 符号 / Emoji / 数字面板、候选翻页、动态键盘高度——M5
- 键盘设置 UI、主题——M5
- 学习闭环、性能门槛、无障碍——M6

**用户决策（2026-08-12 AskUserQuestion）**：

| 问题 | 决策 |
|---|---|
| 真机/模拟器可用？ | 两者都没有 → 验收降级为构建 + 测试 + 静态检查；真机验证列为待办 |
| 包名 | `io.opi.input` |
| 组合态显示 | 无 composing，拼音显示在候选栏 |

## 2. 方案对比与组件

### 方案

| 方案 | 描述 | 结论 |
|---|---|---|
| **A（采用）** | Kotlin IME shell（最小 InputMethodService）+ FlutterView 作为输入视图；独立 FlutterEngine + `imeMain` entrypoint；Dart 侧持有全部键盘/候选 UI | 复用 M3 Flutter 资产，键盘逻辑纯 Dart 可测；shell 最小化，Kotlin 只做 3 个通道方法 |
| B | Kotlin 原生键盘 UI，Flutter 仅候选栏 | 键盘逻辑分两处（Kotlin + Dart），可测性差，被拒 |
| C | 完整原生（Kotlin 键盘 + Rust JNI） | 放弃 Flutter 复用，工作量最大，被拒 |

### 组件清单

| 文件 | 职责 |
|---|---|
| `lib/ime/ime_main.dart` | IME entrypoint：建立通道、装配键盘 + 候选栏 + EngineController |
| `lib/keyboards/qwerty.dart` | 最小 QWERTY 键盘 widget + 击键分发逻辑 |
| `lib/candidates/candidate_bar.dart` | 候选栏 widget（top-8，点击选择） |
| `lib/platform/ime_channel.dart` | `ImeChannel` 抽象接口 + `MethodChannelIme` 实现（测试注入 fake） |
| `android/app/src/main/kotlin/io/opi/input/OpiImeService.kt` | InputMethodService：FlutterView 入 IME 窗口 + 通道宿主 |
| `android/app/src/main/kotlin/io/opi/input/MainActivity.kt` | 设置入口 activity（从 `com/example/app/` 迁移） |
| `android/app/src/main/res/xml/method.xml` | IME 元数据（settingsActivity → MainActivity） |
| `android/app/src/main/AndroidManifest.xml` | IME service 声明 + BIND_INPUT_METHOD 权限 |

## 3. 击键路径与通道协议

### 击键数据流

```
Dart 按键 → EngineController.input(ch)（同步 FFI）
         → 引擎回候选 → CandidateBar 渲染
         → 用户点选 → EngineController.select(i) → 文本
         → ImeChannel.commitText(text) → Kotlin → InputConnection.commitText → 应用
```

### 通道协议（MethodChannel "opi/ime"）

| 方法 | 参数 | Kotlin 侧行为 |
|---|---|---|
| `commitText` | String | `currentInputConnection.commitText(text, 1)` |
| `deleteBackward` | 无 | `currentInputConnection.deleteSurroundingText(1, 0)` |
| `performEnter` | 无 | `currentInputConnection.performEditorAction(IME_ACTION_SEND)` |

`currentInputConnection` 每次调用时现取（不缓存），避免连接切换后引用失效。

### 按键分流（buffer 空 / 非空）

| 按键 | buffer 非空（拼音组合中） | buffer 空 |
|---|---|---|
| 字母 | `engine.input(ch)` 加入组合 | `engine.input(ch)` 进入拼音单字（输入模式） |
| 空格 | `engine.inputSpace()` 选首候选 | `commitText(" ")` |
| 退格 | `engine.backspace()` | `deleteBackward()` |
| 回车 | `engine.select(0)` 上屏首候选 | `performEnter()` |

注：M4 默认拼音模式，buffer 空时的字母按键实际进入拼音单字组合；英文模式（switchMode）下 buffer 空且字母键按下时直接提交 ASCII（后续细化，M4 以拼音为主路径）。

## 4. 键盘 UI

- 3 行 QWERTY（qwertyuiop / asdfghjkl / zxcvbnm），底部行：`🌐`（模式切换）、`123`（占位，M5 生效）、空格、退格、回车
- 候选栏固定在键盘顶部：top-8，点击选择；拼音（buffer）显示在候选栏首行（**无 composing**）
- 英文模式下隐藏候选栏，按键直接提交 ASCII
- 固定键盘高度：屏幕宽度 × 0.42 左右（M5 再做动态高度/横屏适配）

## 5. 错误处理

| 场景 | 处理 |
|---|---|
| FFI 引擎调用失败 | 沿用 M3 fallback 策略（引擎不可用时静默回退） |
| 通道连接为空（IME 未绑定/输入连接已断） | 方法调用 no-op，不抛异常 |
| 引擎未加载 | 沿用 M3：加载失败时降级为无引擎直传（buffer 空时按键直接提交 ASCII） |
| IME 未启用/未选择 | 系统设置流程，应用内不处理 |

## 6. 测试与验收

### 测试

| 层 | 内容 |
|---|---|
| Dart widget 测试 | 键盘渲染 + 击键分发、候选栏渲染 + 点击选择，使用 fake `ImeChannel`（不触发真通道） |
| Dart 单元测试 | 按键分流逻辑（buffer 空/非空 × 空格/退格/回车） |
| 现有门禁 | `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`flutter analyze` 保持全绿 |
| 构建验收 | `flutter build apk --debug` 成功 |

### 验收清单

1. `flutter build apk --debug` 构建成功
2. AndroidManifest 含 IME service 声明 + BIND_INPUT_METHOD 权限 + `method.xml` 元数据，包名 `io.opi.input`
3. cargo 门禁全绿（workspace test + clippy）
4. `flutter test` / `flutter analyze` 全绿
5. **真机验收：推迟**（当前无设备/模拟器可用），列为后续待办——安装 APK、启用 IME、实测键入提交路径

## 7. 与 V1 主规格的关系

- M4 对应主规格 [2026-08-12-opi-ime-design.md](./2026-08-12-opi-ime-design.md) 里程碑表中的 Android 接入行
- M5（符号/Emoji/数字面板、设置页）在 `lib/keyboards/`、`lib/candidates/` 内扩展，协议不变
- M6（SQLite 学习闭环、性能门槛、TalkBack）不涉及通道协议变更
- `setComposing` 为未来协议扩展点（M5/M6 需要时再加通道方法，M4 不做）
- **实现偏差**：M4 执行中如偏离本设计（如 frb 签名、cargokit 行为），按 M3 惯例追加「实现偏差」小节至此文档

## 8. 实现偏差（2026-08-12）

- **真机验收未执行**：当前无设备/模拟器可用，APK 安装 + IME 启用 + 实测键入提交路径列为后续待办（与设计「验收清单 5」一致）
- **Task 5/6 测试代码偏差**：`testWidgets` FakeAsync zone 下直接 `await EngineController.load()` 会挂死（isolate 回复不派发）→ 用 `tester.runAsync()` 包装；`RustLib.init()` 非幂等（二次调用抛 Bad state）→ 提升到 `setUpAll`
- **Task 6 代码质量审查发现**：英文模式切换回 pinyin 前若 buffer 非空，候选栏隐藏期间 buffer 滞留引擎中（无 UI 提示）——M4 接受，M5 处理
- **Task 8 代码质量审查 3 项 M5 加固候选**：onCreateInputView 重入时销毁旧引擎；onWindowHidden 时暂停引擎；onDestroy 前先 detachFromFlutterEngine
- **APK 构建失败（本机环境缺陷，非代码问题）**：`flutter build apk --debug` 两次尝试均失败——AGP 报 `NDK not configured. Download it with SDK manager. Preferred NDK version is '28.2.13676358'`；Google 仓库 manifest 经代理下载返回 400（`HTTP/1.1 400 Bad Request`），无法自动安装 NDK
  - 已做修复：`rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`（成功，rust-lang.org 网络可用）
  - 尝试修复（已回退）：`app/build.gradle.kts` 曾将 `ndkVersion` 固定为本机 `27.0.12077973`，但 AGP 只在 `<sdk.dir>/ndk/<version>` 下解析 NDK（`sdk.dir=/usr/lib/android-sdk` 下无 ndk 目录），固定无效，已回退为 `flutter.ndkVersion`
  - 仍失败原因：flutter 工具将 `local.properties` 的 `sdk.dir` 重写回 `/usr/lib/android-sdk`（仅含 platform-tools，无 NDK）；`flutter config` 的 `android-sdk` 指向不存在的 `/usr/local/Android/Sdk`；直接 `gradlew` 绕行也因依赖解析网络阻塞挂起。环境需安装 NDK 28.2.13676358 或统一 sdk.dir 后重试
- **包名验证（源码级）**：无 APK 可跑 aapt，改为源码验证——`namespace`/`applicationId = io.opi.input`，manifest 含 IME service + BIND_INPUT_METHOD + `@xml/method`，无任何 `com.example` 残留
- **门禁**：cargo test 114 通过 / clippy 0 警告；flutter analyze 0 问题；flutter test 17/17 通过——全绿

### 2026-08-13 追加：APK 构建成功（环境修复完整记录）

`flutter build apk --debug` 最终成功（`✓ Built build/app/outputs/flutter-apk/app-debug.apk`，138MB debug，含三 ABI libflutter.so + opi_ffi.so）。本节为上一节「APK 构建失败」的收尾，8 次构建尝试累计的修复链：

1. **sdk.dir 统一**：`flutter/config` 的 `android-sdk` 与 `local.properties` 统一到 `/home/component/Android/sdk`（flutter 不再重写）
2. **NDK 固定**：`app/build.gradle.kts` 固定 `ndkVersion = "27.0.12077973"`（本机 SDK 实装版本；AGP 9 不再要求 28.2.x）
3. **阿里云镜像**（本机 dl.google.com 被 DNS 劫持至 ~2KB/s，且 Google IP 段被网络封锁、直接连接全部超时）：`settings.gradle.kts` pluginManagement 与 dependencyResolutionManagement、根 `build.gradle.kts` allprojects buildscript 均加 aliyun google/gradle-plugin/public 镜像优先
4. **本地引擎 m2**（`io.flutter` 引擎产物只发 dl.google.com，镜像均 404）：从 SDK 缓存 `/home/component/flutter/bin/cache/artifacts/engine/{android-arm64,arm,x64}/flutter.jar` 构造 `/home/erik/opi_local_m2`，含 flutter_embedding_debug + 三 ABI libflutter.so jar；`dependencyResolutionManagement` 首条仓库指向它
5. **移除 `google()`**（dependencyResolutionManagement）：Gradle 动态版本（`androidx.test:runner:1.2+`）会查询所有声明仓库的 maven-metadata.xml，dl.google.com 连接超时直接失败；aliyun google 镜像为完整镜像，移除后全部走镜像
6. **embedding POM 补全传递依赖**：本地 m2 的 flutter_embedding_debug POM 原为极简版（无 dependencies 块），导致 integration_test 编译缺 `androidx.fragment.app.FragmentActivity` 等类；从 embedding 类文件的常量池反推引用集，补全 9 个 androidx 依赖（activity 1.8.2 / annotation 1.0.0 / core 1.13.1 / exifinterface 1.0.0 / fragment 1.1.0 / lifecycle-common 2.0.0 / lifecycle-runtime 2.0.0 / window 1.0.0 / window-java 1.0.0）
7. **cargokit Gradle 9 兼容**：`rust_builder/cargokit/gradle/plugin.gradle` 的 `project.exec(Closure)` 在 Gradle 9 全部重载被移除（报 `Could not find method exec()`），改为注入 `ExecOperations` 服务 + 显式 `Action<ExecSpec>`
8. **代码 bug 修复**（M4 实现时的 API 误用，编译期暴露）：`OpiImeService.kt` 的 `import io.flutter.injector.FlutterInjector` → `io.flutter.FlutterInjector`（jar 内实际包名）；不存在的 `FlutterView.LayoutParams` → `FlutterView(this)`
9. **compileSdk 33→34**（`rust_builder/android/build.gradle`）：本机 SDK 无 android-33 平台包（33 平台下载走被封锁的 dl.google.com）

**保持待办**：真机验收（APK 安装 + IME 启用 + 实测键入）；重生成 frb 代码会重置 `rust_builder/android/build.gradle` 的 compileSdk 34 为模板值 33，需重打该补丁

### 2026-08-14 追加：真机闪退修复（relinker 传递依赖缺失）

真机首验（Redmi aurorapro / arm64-v8a / Android 16）发现**打开 app 即闪退**，logcat 定位根因：

```
NoClassDefFoundError: Failed resolution of: Lcom/getkeepsafe/relinker/ReLinker$Logger;
  at io.flutter.embedding.engine.FlutterJNI$Factory.provideFlutterJNI
```

- **根因**：Flutter 引擎（engine commit 7a06558 起）改用 ReLinker 加载 `libflutter.so`（规避 Play feature delivery / 低 minSdk 的 dlopen 问题）。本地 m2 的 embedding POM 为手工补全（上一节第 6 项），反推常量池时遗漏 `com.getkeepsafe.relinker:relinker` —— 编译期不暴露（embedding jar 已编译），运行时才解析类 → 真机闪退。验证：embedding jar 字节码引用 `ReLinker`/`ReLinker$Logger`/`ReLinkerInstance`；APK 首个 dex 无该类。
- **修复**（双保险）：
  1. 本地 m2 POM（`/home/erik/opi_local_m2/.../flutter_embedding_debug-*.pom`，不在 git）补 `com.getkeepsafe.relinker:relinker:1.4.5`（官方版本，StackOverflow 报错与 Maven Central 佐证）
  2. `flutter/app/android/app/build.gradle.kts` 显式 `implementation("com.getkeepsafe.relinker:relinker:1.4.5")` —— m2 重建会丢 POM 补丁，项目内持久化
- **坐标注意**：groupId 是 `com.getkeepsafe.relinker`（含 `.relinker`），误写 `com.getkeepsafe` 会导致 Gradle 解析失败（首构建即暴露）
- **验证**：重建 + 真机安装 → 打开 app 正常渲染（Impeller Vulkan + Dart VM service），IME 启用/设为默认成功，进程稳定，无崩溃。键盘弹出路径待用户手动确认
- **扫描结论**：embedding jar 其余外部引用（play-core 仅 `FlutterPlayStoreSplitApplication`/deferred-components 用，本 app 不触发；`org.json` Android 内置）均无需补
