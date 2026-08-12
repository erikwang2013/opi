# OPI 输入法 V1 设计规格

日期：2026-08-12
状态：已获用户逐节确认

## 1. 项目定位与范围

OPI (Open People Input) —— 开源、隐私优先、跨端输入法。本文档定义 V1（Android 平台、全拼方案）。

**V1 范围：**
- 平台：Android（InputMethodService 接入）
- 方案：全拼（引擎预留 `InputScheme` trait，双拼/五笔为 V2）
- 覆盖：GB18030 全量汉字（约 2.7 万）+ Unicode 15.1 全量 emoji（~3700）+ 常用符号默认面板 + 按 Unicode 区块分组的全量符号浏览面板
- 学习：可开关本地学习 + 用户词库导出（JSON，为云同步预留格式）
- 词库：开源数据整合（rime 社区等），经 `opi-tools` 编译为自定义压缩二进制

**V1 不做（YAGNI，架构已预留）：** 云同步、语音输入、皮肤系统、英文自动纠错（V1 仅做单词联想，纠错算法留 V2）、双拼/五笔、其他平台。

## 2. 架构

### 2.1 分层

| 层 | 说明 |
|---|---|
| 平台接入层 | 每个平台一个薄壳。V1 为 Android `InputMethodService` |
| Flutter 前端 | 键盘视图、候选栏、符号面板、表情面板、设置页。可替换 |
| opi-ffi | flutter_rust_bridge 生成绑定，薄壳。同步调用（击键路径）+ async（加载） |
| opi-engine-core | 纯逻辑内核，无 IO 无平台依赖。scheme/composer/candidates/symbols/learner/按键状态机 |
| opi-engine-data | 词库二进制格式 + 加载/校验/回退 + SQLite 用户库 |

### 2.2 关键技术选型

- **Flutter ↔ Rust 通信**：flutter_rust_bridge（类型安全 FFI，击键路径同步调用，目标 <30ms 端到端）
- **Cargo workspace 多 crate**：`engine-core` / `engine-data` / `opi-tools` / `opi-ffi`，核心 crate 纯 `cargo test` 可测
- **词库格式**：自定义二进制（魔数 `OPID` + 版本 + DAWG trie + 词频表），mmap 加载不复制
- **用户数据**：SQLite（`user_words` / `word_freq` / `meta`）

### 2.3 关键设计原则

1. 内核不可变：core 不碰文件系统、网络、UI
2. 击键路径零分配：一次同步 FFI 调用，无 IPC 无异步
3. UI 可替换：引擎与 UI 通过"击键 → 候选"协议解耦
4. 数据驱动：词库/符号表编译工具与引擎分离，词库更新不发版本

## 3. 核心交互数据流

### 3.1 中文输入（全拼）

```
用户按键 → InputMethodService 捕获 → 同步 FFI input_key(c) → core Composer 追加拼音串
→ CandidateEngine 查 DAWG trie 前缀 → 排序（静态词频 × 用户学习权重）→ top 8 候选
→ Flutter 候选栏渲染 → 用户选词 → FFI commit_candidate() → Learner 记录（若开启）
→ Android 提交文本
```

### 3.2 其他输入模式

- 英文/数字/标点：同一 Composer 按键盘模式分流，不进拼音码表
- 符号面板：面板模式，引擎只做查询不维护状态
- Emoji：拼音直打候选混合（`xiao` → 😄）+ 表情面板浏览/搜索（CLDR 名称）

### 3.3 状态机

- `Session` 不可变状态：`{ 拼音串, 已提交文本, 光标位置, 当前模式 }`
- 每次击键返回 `SessionUpdate`（增量 diff），Flutter 只渲染 diff
- 引擎单线程无共享可变状态，天然无锁

### 3.4 学习闭环

选词 → Learner 更新用户词频（SQLite 异步批量落盘）→ 下次排序用户词权重高于静态词频。
支持：删除自造词、一键清除学习数据、导出 JSON。

## 4. Flutter UI 结构

```
lib/
  engine/        # EngineController（封装 opi-ffi，单一状态源，Riverpod）
  keyboards/     # qwerty / symbols / emoji / numbers 四视图
  candidates/    # 候选栏组件（点击选词 + 横滑翻页，V1 无滑选）
  settings/      # 设置页：学习开关、清除数据、导出词库、词库版本
  platform/      # Android IME 平台通道适配
```

- 主键盘：26 键 QWERTY；长按符号键出变体；长按 ⇧ 大写锁定
- 符号面板：Tab 分类（常用/数字/数学/货币/希腊/拉丁/注音/CJK 扩展/全部…），"全部…"按 Unicode 区块浏览 150+ 区块；拼音/英文关键字搜索
- Emoji 面板：Unicode 15.1 全量，CLDR 名称搜索，最近使用优先
- 候选栏：V1 每屏 8 个，横滑翻页，无滑选手势

## 5. 错误处理与测试

### 5.1 错误边界（仅在系统边界防御）

| 边界 | 策略 |
|---|---|
| 数据加载 | 魔数+版本+哈希校验；损坏 → 回退内置精简兜底词库 + 设置页提示；不 panic |
| 用户词库 | SQLite 损坏 → 自动重建（旧文件改名备份） |
| FFI | 所有字符串 UTF-8 校验；非法输入返回 Err；`input_key` 永不 panic |
| 输入逻辑 | 纯函数状态机自然拒绝非法输入，无异常路径 |

### 5.2 测试策略

| 层 | 测试方式 | 覆盖重点 |
|---|---|---|
| engine-core | Rust 单元测试 + proptest | 状态机、排序、符号查询、边界 |
| 词库数据 | 构建期校验 | 每个拼音至少一条、词频单调 |
| opi-ffi | Dart 集成测试 | 类型转换往返、错误码映射 |
| Flutter UI | widget + 集成测试 | 候选渲染、面板切换、设置交互 |
| E2E | Android 真机 adb 注入 | 击键延迟基准 |

### 5.3 性能门槛（CI 强制）

- 词库冷加载 < 200ms（mmap）
- 每击键候选生成 < 5ms（P99，引擎侧）
- 候选栏首帧 < 16ms

## 6. 项目结构

```
opi/
  crates/
    engine-core/
    engine-data/
    opi-tools/       # 词库编译工具（独立二进制）
    opi-ffi/
  flutter/
    app/
    android/         # InputMethodService 接入
  data/
    raw/             # 开源词库原始数据 + 来源/许可证记录
    generated/       # 编译产物 .opid
  docs/
    specs/
```

## 7. V1 里程碑

| 阶段 | 内容 | 验收标准 |
|---|---|---|
| M1 引擎内核 | workspace；Composer + PinyinScheme + 候选排序 + SymbolEngine | cargo test 全绿 + proptest |
| M2 数据管线 | opi-tools 编译词库；格式定版；加载/校验/回退 | 词库 <30MB，冷加载 <200ms |
| M3 FFI | flutter_rust_bridge 绑定 + EngineController | Dart 集成测试通过 |
| M4 Android 接入 | InputMethodService + Flutter 键盘进 IME 窗口 | 真机打字 + 选词 |
| M5 UI 完善 | 符号/Emoji/数字面板 + 设置页 | widget 测试 + 手工走查 |
| M6 学习打磨 | SQLite 学习闭环；性能门槛；无障碍 TalkBack 标签 | E2E <30ms/键 |

## 8. V2 展望（不做，仅预留）

双拼/五笔（InputScheme trait）、云同步（用户词库 JSON 格式已定）、Windows/Linux 接入、皮肤系统、语音输入、英文自动纠错。

## 9. 许可证

- 代码：MIT
- 词库数据：按上游许可证单独声明（rime 社区数据 BSD/GPL 混合），`data/raw` 逐条记录来源与许可证，代码与数据解耦避免传染

## M2 实现偏差（2026-08-12）

1. **存储结构**：spec 原定 DAWG trie。实现为排序平面表（entries 表 + pinyin/word blob）+ 前缀二分。理由：~71K 条 rime-luna-pinyin 全量编译约 4MB，远低于 30MB 预算；DAWG 后缀共享最多省 ~5MB，却显著增加编译与加载复杂度。格式已版本化（magic OPID + version=1），后续可演进。
2. **许可证**：spec 原记 rime 词库 BSD/GPL 混合。实际 rime-luna-pinyin 为 **LGPL-3.0**（data/raw/LICENSES.md 已记录）。
3. **频率合成**：rime-luna-pinyin 无词频列 → 3 列行 `word\tpinyin\tNN.NN%` → freq = round(percent×1000)；2 列行 → freq = 1000；重复 (pinyin,word) 取 max。
4. **损坏回退**：spec 原定回退"内置精简词典"。实现为编译提交的内置 fallback.opid（35 条高频词，opi-tools 从 data/raw/fallback.tsv 生成，提交进仓库），与 M1 内置词等价且可再编译。

## M3 实现偏差（2026-08-12）

1. **集成后端**：spec 原定 native-assets 后端。flutter_rust_bridge 2.12 的 `flutter_rust_bridge_codegen create` 仅支持 **cargokit** backend（无 `--integration-backend native-assets` 选项，无 hook/build.dart）。cargokit 在 `flutter test` 时自动构建主机 Rust 库。
2. **crate 命名**：cargokit 按 package name 逐字推导库文件名（`lib${name}.so`），而 cargo 将连字符转下划线 → package name 用 `opi_ffi`（下划线），目录保持 `crates/opi-ffi`；Android 侧 build.gradle 显式 `libname = "opi_ffi"`。
3. **API 签名偏差**（以生成绑定为准）：frb 命名必填参数（`inputKey(ch:)`、`setShift(on_:)`——Rust `on` 为保留字）；usize/u64 → Dart **BigInt**（候选 score 为 BigInt）；`Api.loadFallback()` 为 static；无 instance dispose（RustOpaque 由 GC 管理）；测试需 `setUpAll(() => RustLib.init())`（init 幂等）。
4. **Dictionary trait**：frb auto-opaque 要求 `Api: Send + Sync` → `Dictionary` trait 增加 `Send + Sync` supertraits（两个实现者均为纯数据容器，已验证安全）。
5. **同步核心 + async 外壳**：`load_fallback_sync()` 纯同步可测，`pub async fn load_fallback()` 薄包给 Dart（不引入 tokio），符合击键路径同步调用原则。
6. **损坏路径语义**：`load_or_fallback` 对缺失/损坏路径**静默回退**内置 fallback 词典（不抛错）；bad-path 测试断言回退成功而非异常。
7. **手工 loader 配置**：`frb_generated.dart` 的 `kDefaultExternalLibraryLoaderConfig` 需手工设置 `stem: 'opi_ffi'` + `ioDirectory: '../../target/debug/'`（workspace member 需要）；codegen 重生成会清除 ioDirectory，须重新应用。
