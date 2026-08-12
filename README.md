**[English](README.en.md) · [中文](README.md)**

# Open People Input (OPI)

## 一款回归本质、人人可用的多端输入法

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
| **核心引擎** | 纯 Rust 实现（`engine-core` / `engine-data` / `opi-tools` / `opi-ffi` 多 crate workspace），轻量高效 |
| **跨端框架** | Flutter（键盘视图、候选栏、符号/表情面板、设置页） |
| **平台接入** | Android (InputMethodService)、iOS/macOS (IMK)、Windows (TSF)、Linux (fcitx/IBus)、鸿蒙 (IME Kit) |
| **数据同步** | V2 预留：端到端加密 + 自托管服务支持，用户可选择使用官方服务或自建同步服务器 |

### 🏗 构建与测试

```bash
cargo test --workspace                   # 单元 + 集成 + 属性测试
cargo clippy --workspace --all-targets -- -D warnings   # 门禁：零警告
cd flutter/app && flutter test           # Dart 集成测试（FFI 往返 + 引擎流程 + 控制器）
cd flutter/app && flutter analyze        # 静态分析
```

仓库结构：

```
crates/
  engine-core/    # 纯逻辑内核，无 IO 无平台依赖
    src/          # composer / pinyin / trie / dictionary / learner / symbols / candidates / engine
    tests/        # engine_integration（11）+ proptests（5）
  engine-data/    # .opid 二进制词库：格式、FNV-1a 校验、mmap 加载、损坏回退（M2）
  opi-tools/      # 词库编译工具：dict.yaml → .opid + verify 校验（M2）
  opi-ffi/        # flutter_rust_bridge 绑定（M3）
docs/superpowers/ # 设计规格与实施计划
flutter/app/      # Flutter 应用：EngineController（Riverpod）+ 集成测试（M3）
data/             # 词库源数据（raw）与编译产物（generated，fallback.opid 入库）
```

### 📅 项目状态

> **当前阶段：M3 FFI 绑定 ✅ 完成（2026-08）**

V1 里程碑进度：

- [x] **M1 引擎内核**：cargo workspace + Composer 按键状态机 + 拼音音节表/切分 + Trie 码表 + 候选排序合并 + 本地学习 + Unicode 符号引擎 + Engine 门面（62 测试全绿，clippy 零警告）
- [x] **M2 数据管线**：opi-tools 编译词库 → `.opid` 二进制（mmap 加载、校验、损坏回退）
- [x] **M3 FFI**：flutter_rust_bridge 绑定 + EngineController
- [ ] **M4 Android 接入**：InputMethodService + Flutter 键盘进 IME 窗口
- [ ] **M5 UI 完善**：符号/Emoji/数字面板 + 设置页
- [ ] **M6 学习打磨**：SQLite 学习闭环、性能门槛（<30ms/键）、TalkBack 无障碍

### 📄 许可证

- **代码**：MIT
- **词库数据**：按上游许可证单独声明（rime-luna-pinyin 为 LGPL-3.0），`data/raw` 逐条记录来源与许可证

---

### 💬 写在最后

> *“我实在受不了了，干脆自己做一个。”*
