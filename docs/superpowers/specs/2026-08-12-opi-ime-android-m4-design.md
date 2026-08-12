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
