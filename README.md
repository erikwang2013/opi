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

#### 1. 真正的多端覆盖
一次开发，多端部署。覆盖 **Android、iOS、鸿蒙、Windows、macOS、Linux、Web（含小程序）**，在不同设备上拥有一致的输入体验，词库与个性化设置云端同步（端到端加密）。

#### 2. 开放透明的生态
- **代码开源**：核心引擎及主要客户端代码完全开放，接受社区审计与贡献
- **词库共建**：支持用户提交、审核、合并新词，让词库真正“活”起来
- **自定义输入方案**：支持拼音、双拼、五笔、仓颉、注音等多种编码，甚至允许你定义自己的输入规则

#### 3. 隐私优先
- **默认本地化**：所有输入数据默认仅保存在本地，不上传任何云端
- **离线可用**：核心输入功能完全离线运行，不依赖网络
- **可选云同步**：如需多端同步，采用端到端加密，服务端无法读取任何内容

#### 4. 无障碍与包容性
- 完整支持读屏软件（TalkBack、VoiceOver、NVDA 等）
- 支持语音输入、扫描输入等辅助输入方式
- 内置主流少数民族语言与方言输入方案（藏文、维文、蒙文、粤语、吴语等）

#### 5. 拒绝“功能膨胀”
- **极简核心模式**：所有“花活”均为可选插件，默认只提供最干净的输入界面
- 你需要的，自己装上；你不需要的，绝不强塞

### 🛠 技术路线

| 层级 | 方案 |
|---|---|
| **核心引擎** | 纯 Rust 实现，多 crate workspace（`engine-core` / `engine-data` / `opi-tools`），无 IO、无平台依赖 |
| **出口层** | `opi-ffi` 双 ABI（JNI + C）· `fcitx5-opi`（cdylib）· `tsf-opi`（cdylib COM 服务器），均以进程内单例持引擎 |
| **客户端 UI** | 各端原生，不引入跨端框架：Android 为 Jetpack Compose；Linux 为 C++ AddonInstance 调 Rust 逻辑；Windows 为 TSF COM + Compose Desktop 候选窗（命名管道 NDJSON 通信） |
| **平台接入** | Android (InputMethodService)、Linux (fcitx5)、Windows (TSF)、iOS (M7，C ABI 已就绪) |
| **数据同步** | V2 预留：端到端加密 + 自托管服务支持，用户可选择使用官方服务或自建同步服务器 |

### 🧭 架构设计

<img src="docs/diagrams/architecture.svg" alt="OPI 架构设计：客户端层、出口层、引擎层、数据层、构建管线" width="100%">

**五层，依赖只向下走。** 越往下越稳定，越往上越接近用户：

- **客户端层** —— 各端原生 UI。四端之间不共享界面代码，只共享输入语义。
- **出口层** —— ABI 薄壳，只做类型转换、边界校验、panic 隔离。每个进程持一个引擎单例，例如 Android 的设置页与输入法共享同一个 Rust 单例，所以设置页里改动学习开关，输入法里立刻生效。
- **引擎层** —— `engine-core`，**纯逻辑，无 IO、无平台依赖**。这是整个项目的心脏，也是最容易测的一层：六个模块（`Composer` 状态机 / `Pinyin` 音节切分 / `Trie` 码表 / `Candidates` 排序合并 / `Learner` 用户学习 / `Symbols` 符号表）之上是唯一的 `Engine` 门面，UI 层只跟这个门面打交道。
- **数据层** —— `.opid` 二进制词库：11 字节头 + 14 字节定长条目表 + 双 blob + FNV-1a64 校验尾。装载走 `mmap` 只读映射，查询零拷贝。
- **构建管线** —— `data/raw` 的 TSV 源数据经 `opi-tools` 编译成 `.opid`，再经 `verify` 校验和 / 顺序 / 覆盖率三重门禁后才入库。

> **关键不变式：词库坏了绝不崩输入法。** 任一环节失败都回退到内置 35 词词库 —— 只有内置词库自身损坏才是不可恢复的。

### 🧩 功能设计

<img src="docs/diagrams/features.svg" alt="OPI 功能设计：五种输入模式与六大功能域" width="100%">

**五种输入模式**由 `Composer` 状态机按模式整数（0..4，五端一致）判定每个按键的去留：

| 模式 | 行为 |
|---|---|
| **拼音**（0，默认） | 小写字母与分隔符 `'` 进缓冲，上限 16 字符，查主词典出候选 |
| **繁体**（4） | 行为与拼音完全一致，只是路由到 `trad` 词典；trad 缺失则回退简体库 |
| **英文**（1） | `⇧` 决定单次大写；缓冲为空时**根本不经过引擎**，直传上屏 |
| **数字**（2） | 仅数字进缓冲，空格 / 回车提交 |
| **符号**（3） | 缓冲不接受任何按键，由符号面板直传；开面板前先提交 pending 拼音，不留残影 |

**六大功能域**里最值得一提的是候选排序。它有一个反直觉的设计：**不能把 limit 下推到词典查询**。因为学过的低频词可能反超截断线外的词，所以必须全量收集、统一排序、再去重截断。排序分是

```
score = 静态词频 + 用户词频 × boost
```

其中 `boost` 按词典最大词频**动态缩放**（`max_freq × 2`），而不是写死一个常数 —— 早期写死 10 万时，在 luna 百万级词频下选一次「我」仍会输给静态词「倭」。动态缩放保证了「选一次就压过全部静态词」这个体验。

### 🔄 生命周期

<img src="docs/diagrams/lifecycle.svg" alt="OPI 生命周期：词库装载生命周期与击键生命周期" width="100%">

两条独立的生命线，只有一个交点 —— **装进进程单例**。

**词库生命周期**在进程启动时跑一次：定位（Android 走 assets→filesDir，fcitx5 走 XDG 数据目录）→ 比对 size 决定是否重拷（幂等，防陈旧词库）→ `mmap` 映射 → 校验 → 装入单例。之后 `install_trad` 可以随时热替换繁体词典，且不影响简体模式。

**击键生命周期**每次按键跑一圈，十个步骤：按键分流 → `Composer` 更新缓冲 → `rank_and_pick` 排序 → 候选栏渲染 → 用户选择 → 学习记录 → 提交上屏 → 清空缓冲。关键分支在第二步：**缓冲为空时按键根本不进引擎**（英文/数字模式直传），这是输入法「不卡」的关键 —— 引擎只在真正需要组合的时候才被唤醒。

装载之后，每次击键都只是在只读内存上做零拷贝查询。

### 🏗 构建与测试

```bash
cargo test --workspace                   # 单元 + 集成 + 属性测试（245 项，含 fcitx5 61 / TSF 42）
cargo clippy --workspace --all-targets -- -D warnings   # 门禁：零警告
cd android && ./gradlew testDebugUnitTest   # Android 单测（引擎 FFI + IME 状态机 + 键盘路由 + 宠物）
cd android && ./gradlew assembleDebug       # 构建 debug APK（cargokit 编译 opi-ffi 三 ABI .so）
cargo build --release -p fcitx5_opi         # Linux 插件（C++ 胶水另需 fcitx5-dev）
cd desktop && ./gradlew package             # Windows 候选窗（Compose Desktop）
./target/debug/opi-tools --version          # 版本号 + 小欧
```

> **`assembleDebug` 需要机器上有 `dart`**：cargokit 的构建工具是用 Dart 写的，缺了会在
> `cargokitCargoBuildOpi_ffiDebug` 任务上报 `dart: command not found`（退出码 127）。
> 只跑单测不需要它 —— `testDebugUnitTest` 走纯 JVM 假实现，不依赖 `.so`。

### 📁 项目结构

```
crates/                        # Rust 工作区（6 个 crate）
  engine-core/                 # 纯逻辑内核：无 IO、无平台依赖
    src/                       #   composer / pinyin / trie / dictionary /
                               #   candidates / learner / symbols / engine
    tests/                     #   engine_integration · proptests · trad_mode
  engine-data/                 # .opid 二进制词库：格式、FNV-1a64 校验、mmap 装载、损坏回退
  opi-tools/                   # 词库编译 CLI：tsv / dict.yaml → .opid，含 verify 子命令
  opi-ffi/                     # 双 ABI 出口：JNI（Android）+ C（iOS）
  fcitx5-opi/                  # Linux fcitx5 插件：Rust 逻辑（cdylib）+ cpp/ AddonInstance 胶水
  tsf-opi/                     # Windows TSF 插件：Rust 逻辑 + COM 服务器 + 候选窗线协议
android/                       # Android IME（Kotlin + Jetpack Compose）
  app/                         #   IME 服务 · 键盘 / 候选栏 / 面板 / 设置页 · 宠物组件
    src/main/assets/           #   luna.opid（简体）· trad.opid（繁体）
    src/main/res/              #   自适应启动图标（VectorDrawable，含小欧）
  rust_builder/                #   cargokit 独立版：编译 crates/opi-ffi → 三 ABI .so
  jni_smoke/                   #   JNI 连通性冒烟测试
desktop/                       # Windows 候选窗（Compose Desktop / JVM，命名管道 NDJSON）
shared/                        # 跨端共享的 Kotlin 源码
  pet/OpiPet.kt                #   项目宠物「小欧」的 Compose 绘制（Android 与 desktop 共用一份）
data/                          # 词库数据
  raw/                         #   源数据 TSV + LICENSES.md（逐条记录来源与许可证）
  generated/                   #   编译产物：fallback.opid · luna.opid · trad.opid
docs/                          # 宠物、图与设计文档
  opi-pet.svg                  #   项目宠物「小欧」
  diagrams/                    #   架构设计 · 功能设计 · 生命周期
  superpowers/                 #   specs（设计规格）+ plans（实施计划）
scripts/                       # 词库生成脚本：gen_luna_dict.py · gen_trad_dict.py
```

### 📅 项目状态

> **当前阶段：M6 多端原生 ✅（2026-08）：Flutter 已删除，Android 全原生**

V1 里程碑进度：

- [x] **M1 引擎内核**：cargo workspace + Composer 按键状态机 + 拼音音节表/切分 + Trie 码表 + 候选排序合并 + 本地学习 + Unicode 符号引擎 + Engine 门面
- [x] **M2 数据管线**：opi-tools 编译词库 → `.opid` 二进制（mmap 加载、校验、损坏回退）
- [x] **M3 FFI**：flutter_rust_bridge 绑定 + EngineController（已被 M6 opi-ffi 双 ABI 取代）
- [x] **M4/M5 Android 接入与 UI**：InputMethodService + 键盘/面板/设置页（Flutter 版，M6 原生重写）
- [x] **M6a Android 原生重构**：opi-ffi 双 ABI（JNI + C）替换 frb；Compose 原生 IME + 键盘/候选栏/面板/设置页；删除 flutter/
- [x] **M6b 简繁双词库**：`Mode::Traditional` + 双词典路由 + `trad.opid` + GB2312 单字全覆盖门禁
- [~] **M6c Linux fcitx5 插件**：Rust 逻辑与单测完成，C++ AddonInstance 胶水已写 —— 需 `fcitx5-dev` 头文件方可编译验收
- [~] **M6d Windows TSF 插件**：Rust 逻辑、候选窗线协议与 Compose Desktop 候选窗完成 —— COM 服务端按目标平台门控，待在 Windows 上验收
- [ ] **M7 iOS**：C ABI 出口已就绪，待 SwiftUI 键盘扩展接入

### 📄 许可证

- **代码**：MIT
- **词库数据**：按上游许可证单独声明（rime-luna-pinyin 为 LGPL-3.0），`data/raw` 逐条记录来源与许可证

---

### 💬 写在最后

> *”我实在受不了了，干脆自己做一个。”*

---

### 🤝 欢迎支持

> 如果 OPI 对你有帮助，欢迎扫描下方赞赏码支持我们（微信 / 支付宝均可，金额随意，心意无价）。

| 微信赞赏 | 支付宝赞赏 |
|---|---|
| <img src="docs/weixinpay.png" width="130" height="130" alt="微信赞赏码"> | <img src="docs/alipay.png" width="130" height="130" alt="支付宝赞赏码"> |
