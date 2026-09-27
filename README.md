**[English](README.en.md) · [中文](README.md)**

# Open People Input (OPI)

## 一款回归本质、人人可用的多端输入法

<img src="docs/opi-pet.svg" width="168" align="right" alt="OPI 项目宠物：键帽精灵「小欧」">

### 🐾 认识小欧

**小欧**是 OPI 的项目宠物 —— 一枚**键帽精灵**。

它长得就是键盘上的一颗键，因为输入法本该如此，不必是别的东西。

| 它身上 | 意思是 |
|---|---|
| **身体 = 键帽** | 输入的本体。不喧宾夺主，只做一件小事 |
| **嘴 = 字母 O** | Open —— 开源、开放、可审计 |
| **头顶天线 = 拼音声调 ˉ** | 拼音输入。朱砂色，全身上下唯一的暖色 |
| **侧壁刻字 = OPI** | 键帽的字符位，也正是项目名 |
| **底部涟漪** | 按键落下后只在本机扩散的余波 —— 不外传 |

形象见 [`docs/opi-pet.svg`](docs/opi-pet.svg)（纯手写 SVG，无外部依赖，可任意缩放）。它已经住进了代码里：

- **Android 设置页**：Compose 组件 `OpiPet`，表情跟着引擎状态走 —— 待命 / 等选 / 困惑 / 睡着（关了学习它就睡）
- **空状态**：Android 候选栏与 Windows 候选窗，拼音打不出候选时小欧出来摊手，替掉原来的一片空白
- **App 启动图标**：自适应图标，纯 VectorDrawable，不新增任何 PNG 密度桶
- **`opi-tools` 命令行**：编译与校验成功时打印字符版小欧（只用单宽度字符，CJK 终端不错位），`--version` 也带上它

两个 Compose 前端（Android 与 Windows 候选窗）共用同一份 [`shared/pet/OpiPet.kt`](shared/pet/OpiPet.kt)。各端 UI 不共享代码是这项目的原则，但宠物是一张图 —— 重复一份几何，两边迟早会长得不一样。

<br clear="right">

### 📖 项目缘起

**始于不满，成于热爱。**

在输入法几乎成为数字生活“基础设施”的今天，我们却越来越频繁地感到一种荒诞：

- 想输入一个生僻字，翻了三页候选词都找不到
- 明明关闭了所有隐私开关，输入法却依然“贴心”地推送着刚刚聊天提到的东西
- 词库越来越大，但你最常用的那个词永远排在最后
- 弹窗、皮肤商城、AI助手……功能多到让人眼花缭乱，却连“好好打字”这件事都没做好

我们并不是反感产品的迭代与进化。问题在于，**许多输入法在追求“大而全”的过程中，逐渐忘记了它最核心的使命——让输入这件事本身，变得简单、准确、高效。**

于是，**Open People Input** 诞生了。

这是一场“一气之下”的认真反抗，也是送给所有对现状感到失望的用户的一份礼物。

### 🎯 项目定位

**Open People Input** 是一款**开放、纯粹、跨端**的输入法，致力于为每一位用户提供**不被干扰、不被窥探、不被绑架**的输入体验。

它的名字就是它的全部信仰：

|  | 内涵 |
|---|---|
| **Open** | 引擎开源，词库开放，透明可审计。不玩黑箱，不藏后门，输入数据只属于你自己。 |
| **People** | 为人人而设计——无论你使用什么设备、使用什么语言、是否有特殊需求，都应该享有平等的输入权利。 |
| **Input** | 回归输入的本质。我们不喧宾夺主，只做一件小事，但要做到极致。 |

### ✨ 核心特性

> 本节只写**代码里已经跑得起来的东西**；尚未实现的愿景统一收到下方「🚧 未来规划」，逐条标注状态。此前的版本把两者混在一起、用陈述句写成现状，这里改了。

#### 1. 引擎一份，各端原生
**引擎只写一遍。** `engine-core` 是纯逻辑内核，无 IO、无平台依赖 —— **模块清单以 `crates/engine-core/src/lib.rs` 的 `pub mod` 声明为准**，惯常被点名的几块是 `Composer` 状态机 / `Pinyin` 音节切分 / `Trie` 码表 / `Candidates` 排序合并 / `Learner` 用户学习 / `Symbols` 符号表（**这是一份阅读导引，不是模块普查** —— 简拼 `jianpin`、模糊拼音 `fuzzy`、中文标点 `punctuation`、字节序 `bytes` 同在那一行 `pub mod` 里）。之上是唯一的 `Engine` 门面，UI 层只跟这个门面打交道。另有**平台中立的键路由层**（`router.rs` 的 `KeyRouter` + `keys.rs` 的键码表），供 Apple 与鸿蒙两端经 C ABI 使用 —— 放这里是因为它是纯逻辑；它是**路由层而非引擎模块**，语义以既有的 fcitx5 / TSF 两轨为准，不自创。出口层是三个薄壳（`opi-ffi` 双 ABI · `fcitx5-opi` · `tsf-opi`），每个进程持一个引擎单例。

**有端代码的是 3 个**：Android（Compose 原生 IME）、Linux（fcitx5 插件）、Windows（TSF 插件 + Compose Desktop 候选窗 —— 后者是**独立的 UI 代码库**，经命名管道与 TSF 通信）。各端 UI 不共享代码，只共享输入语义。

**Apple 两平台与鸿蒙目前只有草案，没有可用的端**——三个目录（[`ios/`](ios/)、[`macos/`](macos/)、[`harmony/`](harmony/)）里有 Swift 与 ArkTS 骨架，但**一行都没有被编译过**（本机既无 macOS/Xcode/Apple SDK，也无 DevEco/HarmonyOS SDK，连语法检查都做不到）。**已就绪的只有平台中立的 C ABI**（`crates/opi-ffi`，见 `tests/cabi_test.rs`）——它不属于任何一端，且**已实测能为 Apple 与鸿蒙的多个目标编译**。三者的硬约束与现状见「未来规划」。

#### 2. 隐私优先，而且可以自己核对
- **默认本地化**：输入数据只留在本机。引擎层与出口层的依赖表里**没有 HTTP 客户端**，没有遥测，没有广告 SDK
- **零权限**：Android 的 `AndroidManifest.xml` 里**一条 `uses-permission` 都没有**
- **离线可用**：核心输入完全离线运行；`.opid` 词库走 `mmap` 只读映射，整库不读进堆（命中条目在查询时按需转出 —— `loader.rs` 的 `MmapDictionary::query` 每条命中 `.to_string()`、`candidates.rs` 的 `rank_and_pick` 再 `clone()` 一次，**不是零拷贝**）

（多端云同步**已裁决不做**（用户裁决 2026-09-28），见「未来规划」——**当前仓库里没有任何同步或加密代码**。）

#### 3. 词库是「活」的，也有门禁守着
- **简繁双库**：`luna.opid`（简体）+ `trad.opid`（繁体）；繁体库缺失自动回退简体库。⚠️ **入口现状：只有 Android / iOS 能切到繁体**（`ImeScreen.kt` / `KeyboardViewController.swift` 的模式轮转）；**macOS / Linux / Windows / 鸿蒙没有入口** —— 各自的模式键只覆盖拼音⇄英文、拼音⇄符号（macOS 与 fcitx5 的 `Ctrl+'`、`tsf-opi/src/vk.rs` 的 `mode_hotkey`），Linux 侧 CMake 也只装 `luna.opid`。引擎与数据侧的繁体能力本身是完整的
- **单字全覆盖门禁**：GB2312 全量单字逐一断言有候选（`trad_coverage` 集成测试，字数判据写在测试里）。改词库若撞坏覆盖度，CI 直接红
- **词库坏了绝不崩输入法**：装载策略各端统一 —— **坏路径一律返回 `Err`，不静默回退**（`engine-data/src/dictionary.rs` 的 `load_or_fallback` 注释写明此前的静默回退是有意删除的：UI 会误以为完整词库已加载）；只有**未提供路径**（含空串）时才用内置回退词库（`data/raw/fallback.tsv`，条数以该文件行数为准）。是否再从 `Err` 兜底由调用方决定：Android 的 `EngineLoader` 接住后用内置词库重试（`EngineLoader.kt` 的 `fallback()`），fcitx5 / TSF 上抛（`fcitx5-opi/src/lib.rs` / `tsf-opi/src/tsf.rs` 的装载调用点）
- **词库分发通路**：Android 走 assets → filesDir（`EngineLoader.kt`）、fcitx5 走 XDG 数据目录且由 CMake 安装（`fcitx5-opi/cpp/CMakeLists.txt`）、Windows 走 `OPI_DICT_PATH` 环境变量 → DLL 同目录 → `%LOCALAPPDATA%\opi` → 内置回退（`tsf-opi/src/dict_path.rs`）。⚠️ **Windows 侧仍无打包步骤**，故开箱即用仍是内置回退词库 —— 通路已备、拷贝动作没有
- **词库共建**：源数据是纯文本 `data/raw/*.tsv`，提交 / 审核 / 合并走 PR。流程与**许可证要求**见 [`CONTRIBUTING.md`](CONTRIBUTING.md)

#### 4. 拒绝「功能膨胀」
- 引擎只做一件事：五模式（拼音 / 繁体 / 英文 / 数字 / 符号）的按键状态机 + 简拼 / 模糊拼音 + 候选排序 + 本地学习。没有皮肤商城，没有弹窗，没有 AI 助手
- **简拼**（纯缩写输入，`nh` → 你好、`zg` → 中国）：**常开、无开关**（与模糊拼音同源 —— fcitx5 与 TSF 没有配置面，做成开关只会让这两端不可关）。触发条件、展开规则与上限都在 `engine-core/src/jianpin.rs` 的头注释里，门禁是 `tests/jianpin_ranking.rs`
- **候选总数不设上限**：`FETCH_LIMIT` 与 `Engine::select` 内部那道截断都已拆掉，三处同值（`engine-core/src/router.rs` · `fcitx5-opi/src/candidate.rs` · `tsf-opi/src/logic.rs`，且互相之间有等值断言）。⚠️ **两台宿主机仍是旧上限**：Android（`EngineController.kt` 的 `fetchLimit`）与 macOS（`InputController.swift` 的 `candidates(limit:)`，它旁边的注释仍写「与 Rust 侧 FETCH_LIMIT 一致」—— **该注释已过期**）—— 两者都够不到引擎侧的分页出口；iOS 与鸿蒙走 `candidatesPage()`，从不由前端给 limit
- **两件「机制已备、客户端没接」的事**：其一，**中文标点与全角是两个独立开关**（引擎侧 `chinese_punct` 与 `fullwidth` 各自独立，门禁 `tests/punctuation_switches.rs`；`set_chinese_punct` / `toggle_chinese_punct` 与双 ABI 出口都在）—— 但**没有客户端入口**：全仓唯一的调用方是 JNI 冒烟表 `android/jni_smoke/Main.java`（Android 的 `toggleFullwidth` 同样只有它一个调用方）。**键位不在引擎层** —— `engine.rs` 的注释只给桌面两轨**建议**了暂定的 `Ctrl+/`，未定稿、两轨都还没实现，理由与实测见该注释。其二，**Rust 侧的学习落盘**（`engine-data/src/user_words.rs`，原子写）**全仓零调用方**：Android 的持久化走它自己的 Kotlin `UserWordStore.kt`，fcitx5 / TSF 学完照旧丢
- 排序有一条反直觉的设计 —— **不能把 limit 下推到词典查询**（学过的低频词可能反超截断线外的词）。这类细节才是本项目较真的地方，见下方「功能设计」

### 🚧 未来规划

以下是项目的愿景与**已裁决不做**的方向；**多数目前没有对应代码**，个别只做了一部分（见状态列）。写在这里是为了把边界说清楚，不是承诺时间表：

| 方向 | 状态 | 现状核对 |
|---|---|---|
| **更多平台**：Web（含小程序） | 未开始 | 代码与构建脚本里 grep 这些平台名零命中（`docs/` 也没有，只有本 README 提到） |
| **鸿蒙 HarmonyOS** | **仅有草案**（`harmony/`）| 与 Apple 两平台同一硬约束：**需要 DevEco Studio + HarmonyOS SDK 才能编译，本仓库的验证环境（Linux）无法编译 ArkTS，连语法检查都做不到**。`harmony/` 下的 ArkTS（`InputMethodExtensionAbility` 等）**一行都没有被编译过**，是「起点 + 契约」。已就绪并可实测的是 **Rust 侧**：C ABI 能为鸿蒙目标编译（`cargo check --target aarch64-unknown-linux-ohos` 等，见目录内 README）|
| **Apple 两平台**：iOS · macOS | **仅有草案**（`ios/` · `macos/`）| **硬约束：两端都需要 macOS + Xcode 才能编译，本仓库的验证环境（Linux）无法编译、链接或运行它们，连 Swift 语法检查都做不到**（UIKit / InputMethodKit 是 Apple 独有框架）。两个目录里的 Swift **一行都没有被编译过**，是「起点 + 契约」而不是可用实现。已就绪的只有平台中立的 **C ABI**（`crates/opi-ffi`），且它**已实测能为 Apple 三个目标编译**（`cargo check --target aarch64-apple-ios / aarch64-apple-ios-sim / aarch64-apple-darwin` 均通过，已进 CI；还能产出 arm64 静态库 `libopi_ffi.a` 且导出符号无缺失）。**任何 Apple 端代码在被 Mac 上的编译器看过之前，都不应被当作已实现** —— 本项目已有两次教训：fcitx5 的 C++ 与 Windows 的 TSF 都是「写完了、读起来像完成」，实测却发现从未被编译过（前者 7 处 API 误写，后者文本插入根本没写） |
| **多端云同步 / 端到端加密** | **不做**（用户裁决 2026-09-28） | 无加密库、无 HTTP 客户端：跨 `crates/` `android/` `desktop/` 的 `*.rs` / `*.toml` / `*.kt` 里，`reqwest` / `ureq` / `hyper` / `openssl` / `rustls` / `chacha` / `argon2` / `aes-gcm` / `https://` 逐个零命中。带 `sync` 的命中全是 Rust 同步原语与 TSF 标志（`std::sync`、`TF_ES_SYNC`、`trad_assets_in_sync`），与云同步无关。**注**：学习词库导出 JSON 里确有 `version` 字段，但那是**导出格式**的版本协商（`learner.rs` 的 `import_json` 拒收 `version != 1`），与云同步无关 |
| **更多输入方案**：双拼 · 五笔 · 仓颉 · 注音 · 自定义输入规则 | V2 预留 | `Mode` 的成员只有拼音 / 繁体 / 英文 / 数字 / 符号（成员清单以 `composer.rs` 的 `enum Mode` 为准）。注释里提到由 `InputScheme` 扩展 —— **该类型尚不存在** |
| **英文联想** | **仅数据** | `docs/superpowers/specs/2026-08-12-opi-ime-design.md` 的「V1 不做」那条写明「V1 仅做单词联想」—— 承诺过、此前没兑现。词源 `data/raw/en_words.tsv` 现已入库（来源、许可证 CC-BY 3.0 与上游 pin 见 `data/raw/LICENSES.md`，**该表同时是署名载体**）；**候选逻辑仍未接线** —— `candidates.rs` 对英文 / 数字模式直接返回空表 |
| **无障碍**：读屏软件 | 部分（仅 Android） | Android 键盘已接基础读屏语义：候选变化自动播报（`liveRegion`）、每个按键有可读名称（不再读字形「⇧」「⌫」）、⇧ 的锁定 / 单次大写状态可读。**其余平台（Windows / Linux / iOS）未接入** |
| **语音输入 · 扫描输入** | 未开始 | 零命中 |
| **少数民族语言与方言**：藏文 · 维文 · 蒙文 · 粤语 · 吴语 | 未开始 | 零命中 |
| **插件化**：所有「花活」均为可选插件 | 未开始 | 无插件注册表、无插件接口、无动态加载 —— 当前也还没有需要插件化的「花活」 |

### 🛠 技术路线

| 层级 | 方案 |
|---|---|
| **核心引擎** | 纯 Rust 实现，多 crate workspace（`engine-core` / `engine-data` / `opi-tools`）；其中 **`engine-core` 无 IO、无平台依赖**，`engine-data` 负责文件映射与字节解析，`opi-tools` 是编译 CLI |
| **出口层** | `opi-ffi` 双 ABI（JNI + C）· `fcitx5-opi`（cdylib）· `tsf-opi`（cdylib COM 服务器），均以进程内单例持引擎 |
| **客户端 UI** | 各端原生，不引入跨端框架：Android 为 Jetpack Compose；Linux 为 C++ AddonInstance 调 Rust 逻辑；Windows 为 TSF COM + Compose Desktop 候选窗（命名管道 NDJSON 通信）|
| **平台接入** | Android (InputMethodService)、Linux (fcitx5)、Windows (TSF)、**iOS / macOS / 鸿蒙（仅有草案，分别需 macOS + Xcode 与 DevEco + HarmonyOS SDK 才能编译验证）** |
| **数据同步** | **不做**（用户裁决 2026-09-28）：原计划为端到端加密 + 自托管服务支持；现决定**不提供云同步、不设账号体系**，用户数据只在本机 |
| **版本** | 单一版本源：根 `Cargo.toml` 的 `[workspace.package] version`，全部 workspace 成员（清单见同文件 `members`）共用；Android `versionName` 与 desktop `packageVersion` 向它对齐，发布 tag 取同一号 |

### 🧭 架构设计

<img src="docs/diagrams/architecture.svg" alt="OPI 架构设计：客户端层、出口层、引擎层、数据层、构建管线" width="100%">

**五层，依赖只向下走。** 越往下越稳定，越往上越接近用户：

- **客户端层** —— 各端原生 UI。各端之间不共享界面代码，只共享输入语义。
- **出口层** —— ABI 薄壳，只做类型转换、边界校验、panic 隔离。每个进程持一个引擎单例，例如 Android 的设置页与输入法共享同一个 Rust 单例，所以设置页里改动学习开关，输入法里立刻生效。
- **引擎层** —— `engine-core`，**纯逻辑，无 IO、无平台依赖**。这是整个项目的心脏，也是最容易测的一层：`Composer` 状态机 / `Pinyin` 音节切分 / `Trie` 码表 / `Candidates` 排序合并 / `Learner` 用户学习 / `Symbols` 符号表，**完整模块清单以 `src/lib.rs` 的 `pub mod` 为准**；之上是唯一的 `Engine` 门面，UI 层只跟这个门面打交道。同层还有**平台中立的键路由**（`router.rs` / `keys.rs`），供 Apple 与鸿蒙经 C ABI 使用 —— 它是路由层而非引擎模块，语义以既有两轨为准。
- **数据层** —— `.opid` 二进制词库：定长头 + 定长条目表 + 双 blob + FNV-1a64 校验尾（各段长度以 `engine-data/src/format.rs` 的 `HEADER_LEN` / `ENTRY_LEN` 等常量为准，别抄数字）。装载走 `mmap` 只读映射，**整库不读进堆**；查询时每条命中的词都要转出一次 `String`（见 `loader.rs` 的 `MmapDictionary::query`），不是零拷贝。
- **构建管线** —— `data/raw` 的 TSV 源数据经 `opi-tools` 编译成 `.opid`，再经 `verify` 做校验和 / 顺序两道校验，并由 `trad_coverage` 测试守住 GB2312 单字全覆盖，才入库。

> **关键不变式：词库坏了绝不崩输入法。** 坏路径**不静默回退**（`engine-data/src/dictionary.rs` 的 `load_or_fallback` 注释有意如此），由调用方接住：Android 用内置回退词库重试，fcitx5 / TSF 把 `Err` 上抛 —— 只有内置词库自身损坏才是不可恢复的。

### 🧩 功能设计

<img src="docs/diagrams/features.svg" alt="OPI 功能设计：五种输入模式与六大功能域" width="100%">

**五种输入模式**由 `Composer` 状态机按模式整数（即 `Mode` 枚举的判别值，**具体取值以 `composer.rs` 的 `enum Mode` 为准**；三个跨语言出口 JNI / C ABI / fcitx5 取值一致）判定每个按键的去留：

| 模式 | 行为 |
|---|---|
| **拼音**（默认） | 小写字母与分隔符 `'` 进缓冲（缓冲上限见 `composer.rs` 的 `MAX_BUFFER`），查主词典出候选 |
| **繁体** | 行为与拼音完全一致，只是路由到 `trad` 词典；trad 缺失则回退简体库 |
| **英文** | `⇧` 决定单次大写；缓冲为空时**根本不经过引擎**，直传上屏 |
| **数字** | 仅数字进缓冲，空格 / 回车提交 |
| **符号** | 缓冲不接受任何按键，由符号面板直传；开面板前先提交 pending 拼音，不留残影 |

**六大功能域**里最值得一提的是候选排序。它有一个反直觉的设计：**不能把 limit 下推到词典查询**。因为学过的低频词可能反超截断线外的词，所以必须全量收集、统一排序、再去重截断。排序分是

```
score = 静态词频 + 用户词频 × boost
```

其中 `boost` 按词典最大词频**动态缩放**（`engine.rs` 的 `USER_BOOST.max(max_freq × 2)`，常量与算式以代码为准），而不是写死一个常数 —— 早期只写死常量时，在 luna 百万级词频下选一次「我」仍会输给静态词「倭」。动态缩放保证了「选一次就压过全部静态词」这个体验。

### 🔄 生命周期

<img src="docs/diagrams/lifecycle.svg" alt="OPI 生命周期：词库装载生命周期与击键生命周期" width="100%">

两条独立的生命线，只有一个交点 —— **装进进程单例**。

**词库生命周期**在进程启动时跑一次：定位（Android 走 assets→filesDir，fcitx5 走 XDG 数据目录，Windows 依次试 `OPI_DICT_PATH` → DLL 同目录 → `%LOCALAPPDATA%\opi` → 内置回退，见 `tsf-opi/src/dict_path.rs`）→ 比对 size 决定是否重拷（幂等，防陈旧词库）→ `mmap` 映射 → 校验 → 装入单例。之后 `install_trad` 可以随时热替换繁体词典，且不影响简体模式。

**击键生命周期**每次按键跑一圈，十个步骤：① 按键事件 → ② KeyRouter 分流 → ③ `Composer` 状态机 → ④ 缓冲更新 → ⑤ `rank_and_pick` 排序 → ⑥ 候选栏渲染 → ⑦ 用户选择 → ⑧ 学习记录 → ⑨ 提交上屏 → ⑩ 缓冲清空。关键分支在第 ② 步：**缓冲为空时按键根本不进引擎**（英文/数字模式直传），这是输入法「不卡」的关键 —— 引擎只在真正需要组合的时候才被唤醒。

装载之后，每次击键都在只读映射上遍历、命中条目按需转出 —— 整库不进城，但**不是零拷贝**。

### 🏗 构建与测试

```bash
cargo test --workspace                   # 单元 + 集成 + 属性测试（含 fcitx5 / TSF 两轨）
cargo clippy --workspace --all-targets -- -D warnings   # 门禁：零警告
cd android && ./gradlew testDebugUnitTest   # Android 单测（引擎 FFI + IME 状态机 + 键盘路由 + 宠物）
cd android && ./gradlew assembleDebug       # 构建 debug APK（cargokit 编译 opi-ffi 三 ABI .so）
cargo build --release -p fcitx5_opi         # Linux 插件：只编 Rust cdylib
cmake -S crates/fcitx5-opi/cpp -B build-fcitx5 -DCMAKE_BUILD_TYPE=Release   # 完整通路（含 C++ 胶水，需 fcitx5-dev）
cmake --build build-fcitx5 && sudo cmake --install build-fcitx5             # 安装落点由 opi_locate_check 探针与 CI 守着
cd desktop && ./gradlew package             # Windows 候选窗（Compose Desktop）
./target/debug/opi-tools --version          # 版本号 + 小欧
```

> **`assembleDebug` 需要机器上有 `dart`**：cargokit 的构建工具是用 Dart 写的，缺了会在
> `cargokitCargoBuildOpi_ffiDebug` 任务上报 `dart: command not found`（退出码 127）。
> 只跑单测不需要它 —— `testDebugUnitTest` 走纯 JVM 假实现，不依赖 `.so`。

### 📁 项目结构

```
crates/                        # Rust 工作区（crate 清单以根 Cargo.toml 的 members 为准）
  engine-core/                 # 纯逻辑内核：无 IO、无平台依赖
    src/                       #   模块清单以 src/lib.rs 的 pub mod 为准：
                               #   composer · pinyin · trie · dictionary · candidates ·
                               #   learner · symbols · engine，另有 jianpin（简拼）·
                               #   fuzzy（模糊拼音）· punctuation · keys · router · bytes
    tests/                     #   engine_integration · proptests · trad_mode · jianpin_ranking ·
                               #   punctuation_switches · select_index_bounds（清单见 tests/ 目录）
  engine-data/                 # .opid 二进制词库：格式、FNV-1a64 校验、mmap 装载、损坏回退
    src/user_words.rs          #   用户词落盘（原子写）—— 文件 IO 归本 crate，engine-core 保持零 IO
  opi-tools/                   # 词库编译 CLI：tsv / dict.yaml → .opid，含 verify 子命令
  opi-ffi/                     # 双 ABI 出口：JNI（Android）+ C（Apple / 鸿蒙）
  fcitx5-opi/                  # Linux fcitx5 插件：Rust 逻辑（cdylib）+ cpp/ AddonInstance 胶水
    cpp/CMakeLists.txt         #   **唯一的编译 + 安装通路**，落点由 opi_locate_check 探针与 CI 守着
  tsf-opi/                     # Windows TSF 插件：Rust 逻辑 + COM 服务器 + 候选窗线协议
    src/dict_path.rs           #   词库查找顺序：环境变量 → DLL 同目录 → %LOCALAPPDATA% → 内置回退
android/                       # Android IME（Kotlin + Jetpack Compose）
  app/                         #   IME 服务 · 键盘 / 候选栏 / 面板 · 设置页（含用户词导入导出）· 宠物组件
    src/main/assets/           #   luna.opid（简体）· trad.opid（繁体）
    src/main/res/              #   自适应启动图标（VectorDrawable，含小欧）
  rust_builder/                #   cargokit 独立版：编译 crates/opi-ffi → 三 ABI .so
  jni_smoke/                   #   JNI 连通性冒烟测试
desktop/                       # Windows 候选窗（Compose Desktop / JVM，命名管道 NDJSON）
ios/                           # iOS 键盘扩展 —— ⚠️ 草案，一行 Swift 都没编译过（见目录内 README）
macos/                         # macOS 输入法（InputMethodKit）—— ⚠️ 同上
harmony/                       # 鸿蒙输入法（ArkTS + N-API 原生模块）—— ⚠️ 同上，一行 ArkTS 都没编译过
shared/                        # 跨端共享的 Kotlin 源码
  pet/OpiPet.kt                #   项目宠物「小欧」的 Compose 绘制（Android 与 desktop 共用一份）
data/                          # 词库数据
  raw/                         #   源数据 TSV + LICENSES.md（逐条记录来源、许可证与上游 pin）：
                               #   symbols.tsv · symbol_blocks.tsv · trad_hanzi.tsv ·
                               #   trad_phrases.tsv · en_words.tsv · fallback.tsv
  generated/                   #   编译产物：fallback.opid · trad.opid 入库；
                               #   luna.opid 未入库（本地重编产物，入库副本在 android assets）
docs/                          # 宠物、图与设计文档
  opi-pet.svg                  #   项目宠物「小欧」
  diagrams/                    #   架构设计 · 功能设计 · 生命周期
  superpowers/                 #   specs（设计规格）+ plans（实施计划）
  weixinpay.png · alipay.png   #   赞赏码（页脚引用）
scripts/                       # 词库生成脚本：gen_luna_dict.py · gen_trad_dict.py · gen_symbols.py
                               #   （+ symbol_keywords.py 手写关键字表）· gen_en_dict.py · hanzi_freq.py
.github/workflows/ci.yml       # CI：fmt / cargo test / clippy 零警告 / C 消费者 + JNI 冒烟 /
                               #   TSF Windows 目标 / Apple 目标 / Android 单测 / fcitx5 编译 + 打包落点
LICENSE · CONTRIBUTING.md      # MIT 全文 · 贡献指南（含词库许可证要求）
```

### 📅 项目状态

> **当前阶段：M6 多端原生（2026-08）：Android 路已完成、Flutter 已删除；Linux / Windows 两路待目标平台验收**

V1 里程碑进度：

- [x] **M1 引擎内核**：cargo workspace + Composer 按键状态机 + 拼音音节表/切分 + Trie 码表 + 候选排序合并 + 本地学习 + Unicode 符号引擎 + Engine 门面
- [x] **M2 数据管线**：opi-tools 编译词库 → `.opid` 二进制（mmap 加载、校验、损坏回退）
- [x] **M3 FFI**：flutter_rust_bridge 绑定 + EngineController（已被 M6 opi-ffi 双 ABI 取代）
- [x] **M4/M5 Android 接入与 UI**：InputMethodService + 键盘/面板/设置页（Flutter 版，M6 原生重写）
- [x] **M6a Android 原生重构**：opi-ffi 双 ABI（JNI + C）替换 frb；Compose 原生 IME + 键盘/候选栏/面板/设置页；删除 flutter/
- [x] **简繁双词库**（spec §8 未给它 M6 编号，独立成项）：`Mode::Traditional` + 双词典路由 + `trad.opid` + GB2312 单字全覆盖门禁
- [~] **M6b Linux fcitx5 插件**：Rust 逻辑与单测完成；**C++ 胶水此前从未被编译器看过**，现已补上 CMake 编译 + 安装通路（`crates/fcitx5-opi/cpp/CMakeLists.txt`）与 CI 的 fcitx5 job（编译 + 安装落点断言）—— 待在目标平台实机验收
- [~] **M6c Windows TSF 插件 + CMP 候选窗**：Rust 逻辑、候选窗线协议、词库分发通路与 Compose Desktop 候选窗完成 —— COM 服务端按目标平台门控，**且仓库里没有打包步骤**（词库开箱仍是内置回退那几十条词），待在 Windows 上验收
- [ ] **M7 iOS / macOS**：C ABI 已就绪，且**已实测能为 Apple 目标编译**（`cargo check` 的 Apple 目标全过，清单见 `.github/workflows/ci.yml`；能产出 arm64 静态库 `libopi_ffi.a`，导出符号无缺失 —— 数量以 `crates/opi-ffi/src/cabi.rs` 的导出为准）；`ios/` 与 `macos/` 下的 Swift 草案**从未被编译器看过** —— 需在 Mac 上先让编译通过，再谈功能

> 里程碑编号以 `docs/superpowers/specs/2026-08-14-opi-multi-platform-design.md` §8 与 M6 实施计划为准
> （M6a=Android / M6b=fcitx5 / M6c=TSF+候选窗）。

### 📄 许可证

- **代码**：MIT，全文见 [`LICENSE`](LICENSE)。源码文件头带机器可读的 **SPDX 标识**（`SPDX-FileCopyrightText: 2026 erik.xyz` + `SPDX-License-Identifier: MIT`），Rust 侧另有 `Cargo.toml` 的 `[workspace.package] license = "MIT"` 规范声明
- **词库数据**：**与代码分开授权** —— `data/raw/*.tsv` 及由其编译出的 `.opid` **不适用 MIT**，各自按上游许可证（rime-luna-pinyin 为 LGPL-3.0；符号与 Unihan 系数据为 Unicode License；英文联想词源为 CC-BY 3.0，**该许可证要求署名** —— `LICENSES.md` 的对应行即署名载体，分发含该数据的产品时须一并保留），逐条来源、许可证与上游 pin 见 [`data/raw/LICENSES.md`](data/raw/LICENSES.md)；提交词表改动前请读 [`CONTRIBUTING.md`](CONTRIBUTING.md) 的许可证一节

---

### 💬 写在最后

> *”我实在受不了了，干脆自己做一个。”*

---

### 🤝 欢迎支持

> 如果 OPI 对你有帮助，欢迎扫描下方赞赏码支持我们（微信 / 支付宝均可，金额随意，心意无价）。

| 微信赞赏 | 支付宝赞赏 |
|---|---|
| <img src="docs/weixinpay.png" width="130" height="130" alt="微信赞赏码"> | <img src="docs/alipay.png" width="130" height="130" alt="支付宝赞赏码"> |
