# OPI 多端架构设计（M6）

日期：2026-08-14
状态：已确认（用户批准 2026-08-14）

## 1. 目标与非目标

### 目标
- 三端全要：Android IME + Linux 系统级输入法 + Windows 系统级输入法
- iOS 键盘扩展（M7 排期，本轮架构预留 C ABI）
- 一套代码：Rust 引擎 + 输入逻辑语义 + Windows 候选窗 CMP UI；UI 各端原生
- 不用任何外部输入法引擎（RIME 等），纯自研

### 非目标
- 桌面候选窗不覆盖 fcitx5（其自带候选窗 UI）
- iOS 键盘扩展本轮不实现
- 不引入跨平台 UI 框架硬套键盘本体（iOS 键盘扩展必须原生 SwiftUI）

## 2. 平台矩阵

| 平台 | 形态 | UI 技术 | 引擎接入 | 候选窗 |
|------|------|---------|----------|--------|
| Android | InputMethodService | Jetpack Compose | JNI（opi-ffi） | 键盘内联 |
| Linux | fcitx5 插件 | fcitx5 自带 | 直接依赖 engine_core | fcitx5 渲染 |
| Windows | TSF COM 插件 | windows-rs | 直接依赖 engine_core | CMP 桌面窗口 |
| iOS（M7） | 键盘扩展 | SwiftUI | C ABI（opi-ffi） | 键盘内联 |

## 3. 架构总览

```
opi/
├── crates/
│   ├── engine_core/        # 拼音引擎（已有，四端共享，核心不动）
│   ├── engine_data/        # 词库/学习（已有）
│   ├── opi-ffi/            # 改：双 ABI ── JNI（Android）+ C ABI（iOS 预留）
│   ├── fcitx5-opi/         # 新：Linux fcitx5 插件
│   └── tsf-opi/            # 新：Windows TSF 插件
├── android/                # Android IME（Kotlin + Jetpack Compose）
├── desktop/                # CMP 候选窗 UI（TSF 专用）
└── ios/                    # M7：SwiftUI 键盘扩展 + C ABI
```

### 共享边界（"一套代码"的落点）
1. **Rust 引擎一套**：engine_core/engine_data 四端共享，不因平台分裂
2. **输入逻辑语义一套**：组合串/候选/翻页/shift/面板切换规则各端行为一致（以 Flutter 版行为迁移表为准）
3. **Windows 候选窗 CMP UI**：唯一跨平台 UI 落点（desktop/ 模块）
4. 键盘 UI 各端原生：Android=Compose、iOS=SwiftUI、Linux=fcitx5 自带

## 4. Rust 层设计

### opi-ffi 双 ABI 出口（替换 flutter_rust_bridge）
- JNI 出口：`JNI_OnLoad` RegisterNatives，17 函数表（load/inputKey/backspace/clear/select/switchMode/setShift/inputSpace/candidates/buffer/mode/searchSymbols/symbolBlocks/symbolsInBlock/learnerEnabled/setLearner/clearUserWords/exportUserWords）
- C ABI 出口：`#[no_mangle] extern "C"` 同名函数（utf16 + length 约定），iOS M7 接入
- 静态单例 `static SINGLETON: Mutex<Option<Engine>>`；设置页与 IME 共享 Learner（语义变更，与蓝图一致）
- UTF-16 字符串约定（JNI 侧 GetStringChars/NewString）
- 每个入口 `panic::catch_unwind`，Kotlin 侧 null → 静默降级

### fcitx5-opi（Linux）
- Rust crate，fcitx5 Rust 绑定，直接依赖 engine_core（无 FFI）
- AddonInstance + InputMethod 接口：提交字符串、候选列表、翻页
- 词库：打包到插件数据目录，首启解压到 XDG data dir（与 Android filesDir 语义对齐）
- 学习数据存 XDG data dir

### tsf-opi（Windows）
- Rust crate，windows-rs 写 TSF COM（ITfThreadMgr/ITfTextInputProcessor/ITfUIElement）
- 候选窗：CMP 桌面窗口（desktop/ 模块），TSF 提供候选位置（ITfCandidateListUIElement），CMP 窗跟随
- 词库/学习：%LOCALAPPDATA%/opi/ 对齐 XDG 语义

## 5. Android IME 设计（M6a，沿用已批准蓝图）

- Jetpack Compose（键盘本体用纯 Compose；CMP 仅用于 Windows 候选窗，见 desktop/）
- `ImeState`（mutableStateOf）+ CompositionLocal；不用 ViewModel（IME 窗口无 ViewModelStoreOwner）
- ComposeView + LifecycleRegistry + ViewTreeLifecycleOwner/SavedStateRegistryOwner
- 键盘高度公式：`(0.42 × min(屏宽,屏高) + 168).coerceAtMost(屏高 − 168)`，BOTTOM_SAFE_PX=168
- 行为迁移表（Flutter → Kotlin）：shift 仅 English 可见、面板打开前提交 pending、editorChanged 重置、8 候选/页 + fetchLimit 64 分页、250ms 搜索防抖、symbol 搜索态 qwerty 叠加层（176px）
- luna.opid：android assets → 解压 filesDir，size 校验后重拷贝（_loadLuna 升级逻辑）
- 构建：aliyun 镜像 + 本地 m2 + NDK 27 + compileSdk 34 + relinker 1.4.5；复用 rust_builder（cargokit 独立版）

## 6. 数据与状态

| 数据 | Android | Linux | Windows |
|------|---------|-------|---------|
| 词库 luna.opid | filesDir | XDG data dir | %LOCALAPPDATA%/opi |
| 学习词频 | filesDir | XDG data dir | %LOCALAPPDATA%/opi |
| Learner 共享 | 静态单例（设置页↔IME） | 插件单例 | 插件单例 |

## 7. 测试策略

- cargo 门禁：全 workspace（当前 115 测试）+ 新插件单测（候选逻辑与 COM/fcitx 胶水分离，纯逻辑可测）
- JNI host JVM smoke：cargo build host cdylib + javac Main.java + java（完全离线）
- Compose 单测：Android UI + 候选窗 UI（compose-runtime 纯 Kotlin 可 JVM 测）；Robolectric 视下载情况降级
- fcitx5/TSF 集成测试需桌面环境，推迟到验收阶段

## 8. 里程碑

| 里程碑 | 内容 | 并行路 |
|--------|------|--------|
| M6a | Android：opi-ffi 双 ABI → Compose IME → 设置页 → 删 flutter/ | A |
| M6b | fcitx5-opi 插件 + 逻辑单测 | B |
| M6c | tsf-opi + desktop CMP 候选窗 + 逻辑单测 | C |
| M7 | iOS SwiftUI 键盘扩展 + C ABI 接入 | 后续 |

三路并行开工（团队模式）：coder 路 A/B/C 各自推进，tester/reviewer 门禁。

## 9. 风险表

| 风险 | 等级 | 缓解 |
|------|------|------|
| fcitx5 Rust 绑定 / windows-rs TSF 接口离线可获取性 | 高 | Step 0 先行验证（同 aliyun 链路做法）；失败则降级 C 插件胶水 |
| TSF COM 复杂度（候选窗位置跟踪、UIElement 同步） | 高 | 胶水与逻辑分离，逻辑纯 Rust 可测 |
| CMP 桌面窗口与 TSF 联动（位置/焦点） | 中 | 简化：候选窗不跟随光标时降级为固定位置提示 |
| 三路并行 token 消耗 | 高 | 分步放行，每路独立验收门 |
| iOS C ABI 设计回炉 | 低 | opi-ffi C 出口本轮设计，M7 直接接 |

## 10. 偏差记录

（重构过程中偏离本设计之处，记录于此，与 M4 偏差记录同例）

| # | 偏差 | 说明 |
|---|------|------|
| 1 | CMP 打包任务名 | 计划 Step 写 `./gradlew :desktop:package`；desktop/ 为独立 Gradle 工程（自带 settings.gradle.kts），实际在 desktop/ 目录执行 `./gradlew package`（CMP 1.11 umbrella 任务，packageDeb/packageDistributionForCurrentOS 亦可用） |
| 2 | Linux 分发格式为 Deb | CMP 1.11 已移除 Zip 格式枚举（仅 AppImage/Deb/Rpm/Dmg/Pkg/Exe/Msi）；Linux 主机用 dpkg-deb 验证 → targetFormats(Deb)，产物 opi-candidates_0.1.0_amd64.deb；Windows .msi 留验收阶段 |
| 3 | 候选窗 UI material3 → foundation 自绘 | CMP 1.11 的 material3 弃用；候选窗 UI 改用 foundation 自绘（desktop/src/main/kotlin/io/opi/candidate/Main.kt） |
| 4 | gradle 需 --refresh-dependencies | aliyun 镜像 probe 404 被 gradle 缓存，首次解析 JNA 5.6.0 前须 `--refresh-dependencies` 清缓存 |
| 5 | JNA/CMP 真实 API 修正 | JVM 侧 named pipe server 用 JNA 5.6.0 raw Function（CreateNamedPipe/ConnectNamedPipe/ReadFile）+ WinBase.INVALID_HANDLE_VALUE 等真实 API 核对（与初稿虚拟 API 不同） |
| 6 | tsf-opi Deactivate 为骨架（正式范围降级） | §4 设计为完整 TSF 生命周期；实际 Deactivate（tsf.rs:142）缺 UnadviseKeyEventSink + composition/候选窗释放，明示标注"验收补全点"。正式降级：Windows 运行验收阶段补全，不在 M6 范围内 |
