# 提交后智能联想（常用词句续接）设计

> 2026-10-27。状态：**待用户批准**；批准前不写生产代码。批准后的下一步是
> `superpowers:writing-plans`，不是直接实现。
>
> 用户已裁决（原话）：「智能输入要支持智能联想，联想常用词句」、「联想词句」、
> 前缀与语义下一词「两种都要」、「可以」、「全做」、「用现有短语表抽小表（推荐）」、
> 「正常执行（推荐）」、「六端都做」。本文把这些话落成可实施的边界；与用户原话
> 不同的工程取舍集中在 §12，供复核。
>
> **订正（2026-10-28，终审）**：§5.6 末条「`drain_pending` 已在 `select_and_insert`
> 后推快照；改完 `CandidateSink` 的可见性即可链式」不成立 —— TSF 候选点选经管道
> 排队、整段延迟到下一次按键执行（`crates/tsf-opi/src/state.rs` 模块头），同一
> keydown 里新链随即被 dismiss 收起；TSF 上联想项只能点选落字（延迟一次按键），
> 不能链式。§11 已补记。另：HarmonyOS 空缓冲回车是既存死键，IME Kit 实际有
> `InputClient.getEditorAttributeSync` + `sendKeyFunction`，可另案补，需真机验证；
> §11 已补记。

## 1. 背景与目标

引擎在候选提交后清空缓冲，`rank_and_pick_meta` 对空输入直接返回空
（`crates/engine-core/src/candidates.rs` 的空输入守卫），候选行随即消失。
「智能输入第一批」（2026-10-26）已有选词学习与误选撤销
（`crates/engine-core/src/engine_learning.rs`），但不产生提交后的候选。

本设计增加**提交后联想**：提交一个中文词后，候选行立即给出该词的常用续接；
点一个续接项就把它接到文档末尾，并以接上后的整串为新根继续联想。全程本地、
离线、无语言模型，只用现有词库、词排序索引与一张从现有短语表抽出的小表。

**目标**

- 提交后立即给出最多 8 个续接项；显示路径不查库。
- 同时覆盖前缀续接（词库里以根开头的词）与语义续接（根后面常接的词/短语）。
- 六个平台同一套引擎语义：Android、iOS、HarmonyOS、fcitx5、TSF、macOS。
- 不引入 9–12MB 的预计算联想法；语义表约 0.1–0.4MiB。

**非目标**

- 不做统计语言模型、n-gram、云端补全或跨句上下文。
- 不做「用上文的拼音参与下一次候选排序」——2026-10-12 裁决仍然有效：
  **确认后再输入就是全新开始，不和之前的拼接**。
- 不做英文、数字、符号模式下的联想；不做整句补全。
- 不改变 `.opid` 的版本号，不新增 FFI 出口，不新增用户可见开关。
- 不做 fcitx5 / TSF 候选窗的空白点击；macOS 候选窗同样不做（原因见 §6.4）。

**反目标（技术上能跑也算失败）**

- 点联想项后把已经上屏的根**再上屏一遍**。
- 联想候选进入正常输入排序，或在新输入里带出旧上下文。
- 为联想在每次击键时扫描全词库。
- 六端各写一套联想算法；算法只在 engine-core 一处。
- 吞掉用户的键：联想态按空格/退格/回车/方向键，必须是正常动作，不是「先收一次」。

## 2. 可见行为

### 2.1 触发

一次候选提交成功后，若同时满足：

1. 提交时模式是拼音或繁体（`Mode::Pinyin` / `Mode::Traditional`）；
2. 提交文本（全角化后的实际上屏文本）至少含 2 个汉字；
3. 提交后缓冲为空，且没有逐字确认片段（`pending_text()` 为空）；

则进入联想态。单字提交不触发（噪声大、根太短）；英数符号模式不触发。

触发与输入方案无关：全键盘、双拼、9/14 键、笔画、五笔下提交中文词都可以触发；
联想查询用的是当前生效的**中文主库/繁体库**（`active_dict()`），不是五笔/笔画码表。
五笔四码唯一自动上屏走 `Engine::input_key` 内部的 `wubi_autocommit()`
（`crates/engine-core/src/engine_wubi.rs`），不经过 `select_from_source`，因此必须在
它提交成功后单独调用 `build_association(&text)`。

9/14 键的逐字态/留尾：`commit_buffer_after_select` 返回 `None` 时没有整段提交，
不建联想；只有整段消耗、缓冲与逐字片段都空时才建。`input_space_at` 冲刷逐字片段时
根取最终 `out`（pending + 选中文本），不是选中文本本身。

### 2.2 显示

联想复用现有候选行，不新增 UI 行、不产生 composition/preedit、不向应用发
marked text。候选行显示最多 8 个**续接项**，固定配比：

- 前 3 条：前缀续接（任意续接长度，允许 1 字）；
- 再 3 条：长前缀续接（续接长度 2–6 字）；
- 最后 2 条：语义续接（来自 assoc 小表，天然不是词库前缀）；
- 非前缀项只填剩余槽位，不挤掉前缀项；总数不足 8 是正常的。

候选栏的显隐仍由宿主现有的「候选非空」判据决定。9/14 键联想项的拼音读数为空
（它们不是输入键序），宿主左列留空/隐藏即可。

### 2.3 点选与链式

点一个续接项：只提交**续接项本身**（不是完整词条），然后以「旧根 + 续接项」
为新根重新联想。文档里已经有旧根，所以拼接结果正好是完整短语。

示例（当前词库的示意，实际排序随词库权重与 assoc 表变化）：

| 已提交（文档末尾） | 联想行（续接项） | 点「晚上」后 | 新根 |
|---|---|---|---|
| 今天 | 是 / 在 / 晚上 / 下午 / 回家 / 为止 … | 今天**晚上** | 今天晚上 |
| 中国 | 人 / 共产党 / 人民 / 政策 / 故事 … | 中国**人民** | 中国人民 |
| 北京 | 市 / 大学 / 的 / 时间 / 现代 … | 北京**大学** | 北京大学 |

链式条件：

- 新根 = `cap16(旧根 + 本次提交文本)`；`本次提交文本` 对联想点击就是续接项；
- 新根**至少 2 个汉字**才 build；不是按续接项本身判断。1 字续接（「中国」+「人」
  =「中国人」）必须能继续链；
- 提交后仍须缓冲空、逐字片段空；
- build 无命中 → 联想清空，链自然结束；不显示空行。

联想点击**不记学习**：续接片段本身不是独立词，按片段记账会污染词频；同时清
`last_commit`，使连续点击不被当成「同一输入下改选」。链式仍可继续。

### 2.4 收起

**任何键**：先收起联想，然后**照常执行原动作**，不吞键、不要求再按一次。

- 文本键（字母、标点）：收起后正常开始新输入。
- 空格：收起后正常上屏空格；空缓冲不会去选联想首项。
- 退格：收起后正常删文档最后一个字符（过引擎的路径由宿主先 dismiss，再走原路由）。
- 回车：收起后正常执行回车动作。
- Tab/Esc/方向键/Home/End/PageUp/PageDown：收起后正常交给应用或路由。
- 数字键：收起后正常输入数字；**联想态数字不是选联想项**。
- ⇧：收起后正常走 ⇧ 状态机。
- Ctrl/Alt/⌘ 组合：**先收起联想，再直通给应用**；不再保留「组合键不收起」。
- 宿主热键（fcitx5 的模式/全角键、TSF 的同名热键、macOS 的 Ctrl+' 等）：
  先收起，再执行热键本身。
- 点击联想项：提交并链式继续（§2.3），不是收起。
- 点击候选行/候选窗空白处：收起（Android、iOS、HarmonyOS；macOS 与 fcitx5/TSF
  受候选窗所有权限制不做，见 §6）。
- 切换模式、输入方案、符号面板、清空会话：收起（§4.5）。
- 光标移动或选区变化：收起（§6）。

「任何键」包括没有抬起事件的屏内键盘（Android/Harmony/macOS 屏内）：因为从不吞键，
不存在「按住重复卡死」的旧问题，也不需要 `swallowed` 状态。

### 2.5 为什么显示/提交的是续接项，而不是完整词条

提交后根已经在文档里。若候选行显示完整词条「今天是」，用户点它时：

- 若插入完整词条，文档会变成「今天今天**是**」（违反反目标）；
- 要避免重复，必须在六个宿主上实现「删掉上一次提交再插入」；Android 的
  `deleteBackward`、iOS 的 `deleteBackward`、fcitx5 的无删除出口、TSF 的 COM
  编辑会话各不相同，做不到同一语义；
- 现有候选模型只有一个 `text`，显示与插入共用，不能「显示整词、只插差值」。

因此续接项 = 完整词条去掉根之后的剩余部分，显示与插入同一个字符串。这是本设计
最重要的一处取舍。前缀续接允许 1 字剩余（「是」「在」「人」）；长前缀与语义续接
只取 2–6 字（「晚上」「共产党」「回家」），避免单字语义噪声。

## 3. 数据：词排序索引 + 语义小表

### 3.1 为什么是索引 + 小表

原型统计（当前部署词库，2026-10-27）：

- 2 字根的前缀命中最多 1254 条，p99 = 49，中位 1；
- 3 字根最多 321 条，p99 = 7；
- 4 字根最多 117 条，p99 = 3。

当前词库条目数：luna 937,780、trad 139,355。词索引 4B/条 ⇒ 3.75MB + 0.56MB
≈ 4.31MB。每次提交只查一次，扫描量完全可接受。预计算「根 → 联想项」要 9–12MB；
语义小表按 5,000 根、每根 3 条约 123KiB（实测规模见 §3.8）。两者都不进入逐键路径。

### 3.2 格式：v1 + flags bit0/bit1

**版本号保持 1**。11 字节头的 `flags`（偏移 6）原先恒为 0；本设计定义：

```
flags bit0 = 1  ⇒ word blob 之后追加 count × u32 的词排序索引
flags bit1 = 1  ⇒ 索引之后再追加 assoc 小表与 assoc_len u32
未知位                  ⇒ FormatError::BadFlags(u8)
```

尾部的**固定顺序**：

```
header 11B
entry table          count × 14B
pinyin blob
word blob
word_index           count × u32，若 flags bit0；值 = entry 下标
assoc                assoc 段，若 flags bit1
assoc_len u32        assoc 段字节数（含 root_count 与目录），若 flags bit1
trailer 8B           FNV-1a64 over [11, len-8)
```

bit0、bit1 相互独立：可以只开索引、只开 assoc，或都开。luna/trad 两个都开。
没有尾部时 `word_end = tail`；有 bit0 时 `word_end = assoc_start 或 tail - count*4`，
有 bit1 时 `assoc_end = tail - 4`、`assoc_start = assoc_end - assoc_len`。
新读者的真实 `word_total = word_end - word_start`，**不把索引/assoc 算进 word blob**。

### 3.3 词排序索引

- 数组按**词字节序**升序；
- 同词按（词库权重降序、entry 下标升序）稳定排序，保证可复现构建；
- 值 = entry 下标（u32）；
- 同一词条可在索引里出现多次（同词多读音），查询结果由联想层按续接项去重；
- `MmapDictionary.words_with_prefix` 在索引上做 lower_bound，然后顺序收集
  `word.starts_with(prefix)` 的条目，返回 `(word, weight)`。没有索引时返回空。

「词库权重」指 `.opid` 条目的 `freq` 字段。luna 的 freq 由行位置编码
（`scripts/gen_luna_dict.py`），**不是真实词频**；本设计所有排序只说「词库权重」，
不称「词频」。

### 3.4 assoc 小表布局

```
assoc section:
  root_count        u32
  directory         root_count × 6B { root_off u32, suffix_count u8, root_len u8 }
  records           按目录顺序紧排：
                      root bytes (root_len)
                      suffix_count × { suffix_len u8, suffix bytes, rank u32 }
```

- `root_off` 相对 assoc 起点，指向该记录 root bytes 的起点；
- 目录按 root 字节序**严格升序**，无重复；
- 记录 canonical 紧排：第一条在 `records_start = assoc_start + 4 + root_count*6`，
  每条的 `root_off` 必须等于前一条结束位置；最后一条必须结束于 `assoc_end`；
- `root_len >= 1`，`suffix_len >= 1`，root/suffix 必须是 UTF-8；
- `rank` = 后缀自身在词库中的权重（不是短语出现次数；不是词库词时为 0）；
- 读取时按 `(rank desc, len asc, bytes asc)` 排序返回；写入也按同一顺序，
  但读者仍排序（每根最多 cap 条，成本可忽略）；
- `assoc_len` = `4 + root_count*6 + records` 的字节数，写在 trailer 前 4B。

### 3.5 兼容性

- **旧读者读新文件**：旧 `validate` 不读 flags，按 `tail - word_start` 算 word
  区长度，把索引/assoc 当成 word blob 的尾部未用字节；条目自己的 word 偏移/长度
  仍然有效，校验与解析都通过。旧二进制能装载新词库，只是不产生联想。
- **新读者读旧文件**：flags=0，无索引/assoc，联想为空，其余完全不变。
- `scripts/merge_luna_kangxi.py` 的 `read_opid` 硬检查 version==1，只按条目偏移
  读词；尾部天然被忽略。`scripts/gen_kangxi_extra.py` 同样不读 flags。
  两个 Python 读取器**不需要改**。
- 词库可以先于二进制发布，也可以单独回滚；不会出现「新词库装不上旧应用」。

旧读者兼容性不能只靠「重写一个旧读取器」来证：发布门禁用**上一版 `opi-tools`**
（真实旧二进制）跑 `verify`，见 §9.5。

### 3.6 校验

`crates/engine-data/src/format.rs`：

- `serialize` 保持写 flags=0，行为与字节不变。
- 新增 `serialize_with_extras(entries, word_index: bool, assoc_rows)`：
  - `assoc_rows` 为空且 `word_index == false` 时，产物必须与 `serialize` 逐字节相同；
  - 写 flags、word_index、assoc、assoc_len、trailer；
  - 写入前断言索引按词有序、assoc 目录按 root 有序；
  - 全部长度用 checked 算术，延续该文件对 32 位 target 的现有防护。
- `validate` 读 flags；出现 `0b11` 之外的位返回 `FormatError::BadFlags(u8)`。
- flags bit1 时：至少 4B 放 `assoc_len`；`assoc_len >= 4`；
  `root_count*6` checked；`assoc_end = tail - 4`、
  `assoc_start = assoc_end - assoc_len`，要求 `word_start <= assoc_start`；
  `records_start <= assoc_end`；逐条检查 `root_off == expected`、UTF-8、
  root 严格升序、最后 `expected == assoc_end`、`assoc_len` 与实际段长一致。
- flags bit0 时：`index_len = count*4`（checked），`index_start = word_end`，
  要求 `word_start <= index_start`；逐条检查索引值 `< count`。
- `word_end` 由尾部反推后再算 `word_total`；条目 `wo + wl` 必须落在真实
  `word_total` 内。这样条目即使越界指向索引/assoc 区，也会被 `BadOffsets` 拒收。
- `Validated` 增加 `flags`、`word_end`、`word_index: Option<usize>`、
  `assoc: Option<AssocView>`（`start/len/root_count/records_start`）；
  `parse` 仍只物化条目。
- 版本测试 `BadVersion(2)` **保持有效**（版本仍是 1）。

**不在加载时校验索引有序性**：那要对近百万条做词切片比较，给每次启动加成本；
索引值已做越界检查，乱序最多让联想质量下降，不会越界/崩溃；trailer 的 FNV 已覆盖
索引字节，意外损坏会被 checksum 捕到。有序性由 `serialize_with_extras` 的写入断言
+ `opi-tools verify`/测试门禁保证。

### 3.7 查询接口

`crates/engine-core/src/dictionary.rs` 的 `Dictionary` 增两个默认方法：

```
fn words_with_prefix(&self, prefix: &str) -> Vec<(String, u32)> { Vec::new() }
fn association_suffixes(&self, root: &str) -> Vec<(String, u32)> { Vec::new() }
```

- `LayeredDictionary`（`crates/engine-core/src/dict_layers.rs`）委托给 `base`。
  **导入层不参与联想**：它是运行时内存索引，没有词排序索引/assoc 表；每次提交全扫
  导入层不值得。已知边界，见 §11。
- `MmapDictionary`（`crates/engine-data/src/loader.rs`）实现两者；没有对应 flags
  时返回空。两个方法都要求 `prefix/root` 非空：空串返回空（不返回全库）。
- `InMemoryDictionary` 与内置 fallback 保持默认空实现：没有索引 = 没有联想，
  不影响现有测试与「导入/回退库」路径。
- `crates/engine-data/src/loader.rs` 当前正好 500 行（500 行门禁），实现时：
  - 用 `#[path = "loader_tests.rs"] mod tests;` 把文件尾测试拆出去；
  - 索引查询放新 `loader_word_index.rs`，assoc 查询放新 `loader_assoc.rs`；
  - `loader.rs` 只留结构体、`from_validated`、`query_prefix_top`、`record`。
- `crates/fcitx5-opi/src/candidate.rs` 491 行：`dismiss_association` 放新
  `candidate_association.rs`；把 `invalidate_fetch_cache` 提升为 `pub(crate)`。

### 3.8 语义小表生成

脚本：`scripts/gen_assoc_table.py`。

```
gen_assoc_table.py --opid <词库.opid> --phrases <短语.tsv> \
                   --out <assoc.tsv> --roots 5000 --cap 3
```

算法：

1. 从 `.opid` 读条目（只读条目，忽略尾部；`read_opid` 的 version==1 检查不变）。
2. 候选根 = 词库中 2–3 个汉字的词；按**在短语中出现次数**降序取前 `--roots`
   个（同次数按词字节序）。常用根才有语义续接价值。
3. 源短语：`data/raw/extra_phrases.tsv`（简体）、`data/raw/trad_phrases.tsv`
   （繁体）。源短语必须全 CJK；否则跳过并计入统计。
4. 对每个短语中根的**每次出现**，取根后剩余子串；对长度 2、3、4 各取一次
   紧随根后的子串作为后缀；过滤以 `的/了/呢/吗/啊/啦/吧/得/地` 开头的后缀。
5. 只保留 `root+suffix` **不是词库词**（精确匹配）的新增对；已是词库词的叶子
   由前缀索引负责，不重复存。
6. 每个根的候选按：
   - 后缀自身词库权重 desc（不是词库词则 0）；
   - 该后缀在短语中的出现次数 desc；
   - 后缀长度 asc；
   - 后缀字节 asc；

   取前 `--cap` 条；最终写入按 `(rank desc, len asc, bytes asc)`（与读取一致）。
7. 输出 TSV：每行 `root<TAB>suffix<TAB>rank`，UTF-8，无表头；root 按字节序升序；
   同一 root+suffix 不得重复。编译器会再校验/分组，脚本输出本身也应可复现。

规模（当前词库实测，2026-10-27）：

- 5,000 根、cap3：7,744 对，约 123KiB；
- 5,000 根、cap5：10,593 对，约 155KiB；
- 20,000 根、cap3：20,544 对，约 336KiB。

本设计取 **5,000 根、cap3**（assoc 槽位只有 2，cap3 留一条余量给去重/前缀冲突）。

质量边界（实测抽样）：

- 约 6 成可用：今天→回家/为止，中国→声音/政策/故事/力量，时间→加班/工作/使用/
  发展/学习，发展→项目/国家/研究/分析/报告；
- 约 2.5 成跨词/不成句：京东→路外滩、吉利→海峡、国足→球队、经营→养素、
  得很→有道理、们的→一生；
- 约 1.5 成两可：个人→在对决、去了→灵魂、密接→触者。

因此非前缀项只占 2 个槽位；语义表是「常用搭配提示」，不是语言模型。

**繁体必须自建表**：用 trad 自己的条目 + `data/raw/trad_phrases.tsv` 跑同一算法，
不能套简体表（简体根/后缀在繁体词库里可能不是词，权重与搭配也不同）。

`data/raw/extra_phrases.tsv` 第 3 列不是频次；luna 的 `freq` 是行位置编码。
两者都不能当真实词频使用。

### 3.9 编译与资产

- `opi-tools compile` 增开关：

  ```
  opi-tools compile [--word-index] [--assoc <assoc.tsv>] <input.tsv|dict.yaml> <output.opid>
  ```

  默认两项都关，保持现有产物字节不变。`--assoc` 给空文件时视为不开 bit1
  （产物与不带 `--assoc` 相同），便于脚本统一传参。
- `crates/opi-tools/src/compiler.rs` 增 `compile_with_extras`（或同等入口），调用
  `serialize_with_extras`；`crates/opi-tools/src/main.rs` 解析新开关。
- 构建顺序（luna）：

  1. 按 `CONTRIBUTING.md` 现有命令得到基线 `luna.opid`（未加尾部）；
  2. `scripts/gen_assoc_table.py --opid <基线> --phrases data/raw/extra_phrases.tsv
     --out data/generated/luna.assoc.tsv --roots 5000 --cap 3`；
  3. `opi-tools compile --word-index --assoc data/generated/luna.assoc.tsv
     <luna.tsv> android/app/src/main/assets/luna.opid`。

  trad 用 `data/generated/trad.opid` 的基线条目 + `data/raw/trad_phrases.tsv`
  生成 `data/generated/trad.assoc.tsv`，再编译 `data/generated/trad.opid` 与
  `android/app/src/main/assets/trad.opid`。
- **入库** `data/generated/{luna,trad}.assoc.tsv`（约 123KiB + 小表）。理由：
  - `crates/opi-tools/tests/dict_crosslayer.rs` 要用同一份 assoc 行重编 trad 并
    逐字节比较；不入库就无门禁；
  - CI 可用 Python 重新生成并 diff，发现「代码改了、表没重生成」。
- `data/generated/luna.opid` 仍被 gitignore（文件约 33MB，不入库）；入库的是
  Android assets 的 `luna.opid` 与 `trad.opid`。
- 只给 **luna** 与 **trad** 打开索引/assoc。stroke / wubi86 / wubi98 / fallback /
  导入层不建。
- Harmony rawfile 由 `scripts/prepare_harmony_dicts.sh` 从 Android assets 复制；
  实现时按现有脚本刷新。
- `CONTRIBUTING.md` 补 `--word-index`/`--assoc` 与 assoc 生成步骤。
- 旧资产改造不需要重新抓上游数据：用 `scripts/merge_luna_kangxi.py` 把现有
  `.opid` dump 成 TSV，再按上面步骤重编；`parse` 对比新旧条目（拼音、词、权重）
  逐条相同，证明只加了尾部。

### 3.10 成本

- 文件：词索引 4B × 条目数；luna 937,780 条 + trad 139,355 条 ⇒
  3.75MB + 0.56MB ≈ 4.31MB；assoc 表按 §3.8 规模约 123KiB + 小表。
  APK/pkg 压缩后增量以构建实测为准。
- 加载：多一次 O(count) 的 u32 越界检查 + assoc 目录校验；不排序、不物化词。
- 提交：一次 `words_with_prefix`（二分 + 范围扫描）+ 一次 assoc 二分查根；
  不进入逐键路径。显示 8 条是 O(1) 克隆。
- 输入：联想态下第一次击键 O(1) 清空，没有全库扫描。

## 4. 引擎：状态与生成

### 4.1 状态

新增 `crates/engine-core/src/engine_association.rs`，`Engine` 增加一个字段：

```rust
struct Association {
    root: String,              // 必须是文档末尾的连续片段
    items: Vec<Candidate>,     // 已按显示顺序排好，最多 8 条
}
Engine.association: Option<Association>
```

`items` 在提交时算好并存下，`candidates()` 只克隆/截断；这样显示路径不查库，
也避免 `candidates(&self)` 需要 RefCell。

对外方法：

- `association_active() -> bool`：`association.is_some()`。不变式保证它只在
  拼音/繁体、缓冲空、无逐字片段时成立。
- `dismiss_association() -> bool`：清空联想；返回清之前是否存在。不碰缓冲/学习/
  模式。返回 bool 是给 Rust 宿主判断「要不要立刻重推候选窗」；不关心可忽略。
- `build_association(&mut self, root: &str)`：crate 内使用；也供 `wubi_autocommit`
  与 `input_space_at` 的收尾调用。

### 4.2 生成算法

```
build(root):
  root = last_16_chars(root)
  if mode not in {Pinyin, Traditional}: return None
  if buffer not empty or pending not empty: return None
  if cjk_count(root) < 2: return None
  root_len = root.chars().count()
  dict = active_dict()

  // 前缀类：词库里以 root 开头的词，去掉 root 后的续接
  conts = {}                                   // cont -> max weight
  for (word, weight) in dict.words_with_prefix(root):
      if word == root: continue
      cont = word.chars().skip(root_len).collect()
      if cont.is_empty(): continue
      conts[cont] = max(conts[cont], weight)   // 同键取最大权重

  prefix_sorted = [(cont, weight)] sorted by (weight desc, cont bytes asc)
  a = first 3 of prefix_sorted                       // 任意长度，允许 1 字
  b = first 3 of (prefix_sorted minus a) with 2 <= chars <= 6   // 长前缀
  selected = [text of (text, weight) in a + b]

  // 非前缀类：assoc 小表，按 rank desc/len asc/bytes asc
  c = []                                       // (suffix, rank)
  for (suffix, rank) in dict.association_suffixes(root):
      if suffix in selected: continue
      if suffix has prefix relation with any item in selected: continue
      c.push((suffix, rank))
      if c.len() == 2: break

  items = [Candidate{text: t, kind: Hanzi, score: w} for (t, w) in a + b + c]
  if items.is_empty(): return None
  return Association { root, items }
```

- 前缀项来自 `words_with_prefix`；长前缀只是同一批里续接 2–6 字的部分，不是第二
  套数据。语义项来自 assoc 表，天然不是词库前缀（生成时已排除 `root+suffix` 是词
  库词的对）。
- 去重键 = 续接项文本；前缀来源优先。assoc 后缀若与已选前缀项有**前缀关系**
  （任一方向）则跳过，避免「晚上」与「晚上好」同时出现。
- 输出顺序固定为「前 3 → 长前缀 3 → 非前缀 2」，类内按权重/rank 排；不按分数混排。
- 续接项不回显根、不包含根；总长上限 8。
- `root` 超过 16 字时截最后 16 字；仍与文档末尾连续，但通常已无命中，链自然结束。
- 汉字判定用 CJK 统一表意文字基本区（U+4E00–U+9FFF）；扩展区留待以后。
- 根含标点/英文时，查询仍可执行；无命中就返回 None，不特殊处理。
- `words_with_prefix("")` / `association_suffixes("")` 必须返回空；build 本身也
  先判 root 至少 2 汉字，不依赖词典实现。

### 4.3 候选装配

在 `crates/engine-core/src/engine_candidates.rs` 的 `candidates_base` 中，放在
符号浏览、英文分支、携带缓冲之后，方案分流之前：

```rust
if s.buffer.is_empty() && let Some(a) = &self.association {
    return a.items.iter().take(limit).cloned().collect();
}
```

这一支在 `rank_and_pick_meta` 的空输入守卫之前，是联想可见的唯一原因。空输入守卫
本身不改。`custom_phrases::candidates` 在 `candidates_base` 之后运行；空缓冲不会
命中自定义短语（导入校验拒绝空 trigger），因此不插入也不截断联想。`limit == 0`
返回空。

### 4.4 选词、链式与学习

`Engine::select_from`（`crates/engine-core/src/engine_learning.rs`）是点击/数字/
面板/路由选词的统一包装；`input_space_at` 是唯一绕过它、直接调用
`select_from_source` 的空格路径。三处协作：

1. **`input_space_at` 入口**先 `dismiss_association()`。这是硬要求：否则
   `self.candidates(limit)` 会取到联想候选，空格会选中联想首项，违背「空格正常
   执行」。入口清掉后，空格路径永远不会被当成联想点击。
2. **`select_from` 包装**在调用 `select_from_source` 前捕获：
   - 是否联想点击：`association_active()` + 缓冲空 + `cands[index]` 与
     `association.items[index]` 文本一致；
   - 旧根 `chained_root = association.root`（必须在清联想前捕获）。
   调用后若返回非空 `out` 且是联想点击，调用
   `build_association(&cap16(chained_root + out))`。
3. **`select_from_source` 入口**先读 `association` 判断这次是否为联想点击
   （缓冲空 + 文本匹配；读必须在清之前），再无条件清 `association`。若是联想点击：
   - 跳过 `learn_selection`；
   - `last_commit.clear()`；
   - 照旧 `commit_buffer_after_select`。
   非联想点击不变。直接调用它的只有 `input_space_at`，而那里入口已 dismiss，
   因此不会误判。

补充：

- 空缓冲下 `commit_buffer_after_select` 返回完整选中文本（续接项会作为独立文本
  上屏），因此链式根与文档末尾一致。
- 若 `commit_buffer_after_select` 返回 `None`（9/14 留尾、逐字态），`out` 为空：
  不建链，联想已清；这是明确行为，不重试。
- `select_from_source` 的 `None` 分支返回空串，`select_from` 不建联。
- 五笔四码自动上屏：`Engine::input_key` 拿到 `wubi_autocommit()` 的提交文本后，
  在 `composer.commit_buffer()` 之后调用 `build_association(&text)`；它不经过
  `select_from_source`，不能靠上面的包装。
- 9/14 键：`digit_tail_to_keep` 留 1–2 键尾巴时 `commit_buffer_after_select`
  返回 `None`，不出联想；整段消耗才出。
- 联想建立后 `fetch_cache` 可能仍以 `(buffer, pending)` 为键；两轨宿主在
  `dismiss_association` 与 `build_association` 后都要作废缓存（§5.5/§5.6）。

### 4.5 清空矩阵

以下入口在入口处 `association = None`，再执行原有逻辑：

| 入口 | 所在 |
|---|---|
| `input_key` / `input_punct_at` | `engine.rs` / `engine_punct.rs` |
| `input_space_at`（`input_space` 也经过它） | `engine.rs` |
| `backspace` | `engine.rs` |
| `clear` | `engine.rs` |
| `switch_mode` / `switch_language` | `engine.rs` |
| `set_scheme` | `engine.rs` |
| `toggle_symbol` | `engine.rs` |
| `set_shift` | `engine.rs` |
| `set_extra_dicts` | `engine_dicts.rs` |
| `set_trad_dict` / `set_stroke_dict` / `set_wubi_dict` | `engine_dicts.rs` |
| `select_from_source` | `engine_learning.rs`（先清，成功提交后重建） |

`toggle_fullwidth`、`toggle_chinese_punct`、`set_enter_action`、`set_wubi_style`
等设置项不改输入上下文，不清联想；如果实现时发现某设置项会改变候选集合，按同一
原则加入矩阵。

`set_shift` 清联想意味着 Android 的 `shiftTap`/`shiftLongPress` 在调用
`api.setShift` 后必须补 `refresh()`（当前它们不 refresh）。

### 4.6 与 2026-10-12 裁决的关系

`crates/engine-core/tests/association.rs` 现有三条测试钉住：「确认后接着打，候选只
按本次输入匹配，不拼上文；上屏文本不重复」：

- `full_scheme_starts_fresh_after_commit`
- `nine_scheme_starts_fresh_after_commit`
- `second_input_matches_a_cold_engine`

本设计不违反：

- 联想只在**空缓冲**出现；任何文本输入在入口清联想，新输入从零开始；
- 联想项是这次点击的独立提交，不是把旧根塞进新输入的候选；
- 学习仍发生在 `select_from_source`，候选排序算法一行不改。

实现时要在这个文件里补一条**带索引/assoc 的词典**（当前夹具是 `InMemoryDictionary`，
默认无联想，现有三条不能充当正控）的契约测试：提交「你好」后打 `ma` 不得出
「你好吗」、不得重复；并显式覆盖联想态。

## 5. 六端接线

统一原则：**引擎只负责产生/清除联想；宿主在按键入口先收起联想，然后走原路径**。
不新增 FFI 出口。Rust 宿主（fcitx5/TSF）用 `dismiss_association()`；Android/iOS/
macOS/HarmonyOS 用现有 `clear()`（它会顺带清 `last_commit`，只影响学习撤销，
不删文本，见 §6.7）。所有宿主都不允许「吞键」式收联想。

### 5.1 Android

- `EngineController.kt` 增只读属性 `associationActive`：
  `buffer.isEmpty() && pendingText.isEmpty() && candidates.isNotEmpty() &&
   (mode == PINYIN || mode == TRADITIONAL)`。这是精确推断：空缓冲只有联想会产生
  候选。文件已 498 行，属性放新 `EngineControllerAssociation.kt` 的扩展属性
  （或先拆出一小段再放），不要顶破 500 行。
- `refresh()` 已无条件取候选；`ImeState.candidatesShowable` 已是「英文或缓冲非空
  或候选非空」，空缓冲联想自动显示。
- `KeyRouter.kt` 的 `handleKey`、`handleSpace`、`handleBackspace`、`handleEnter`、
  `commitText` 入口：若 `associationActive`，先 `controller.clear()`（已 refresh），
  然后**继续原逻辑**：
  - 空格：buffer/pending 已空 → 走 `commit(" ")`；
  - 退格：空缓冲 → 走 `deleteBackward`（删文档字符）；
  - 回车：空缓冲+pending → 走 `performEnter`；
  - 文本/数字：候选已空 → 走正常输入，不选联想项。
  `handleCandidate`/`handleCandidateAbsolute` **不** dismiss（选联想项）。
- `ImeScreen.kt` 里绕过 `handleKey` 的直接提交点（数字直提、分词键、面板直提）
  逐个加同样的 dismiss；实现时按该文件的直接提交点逐一核，不靠「应该走了
  handleKey」的假设。
- `shiftTap`/`shiftLongPress` 在 `api.setShift` 后补 `refresh()`（`set_shift` 清联想）。
- 候选栏 `CandidateBar.kt` 外层 `Row` 加空白点击：仅 `associationActive` 时
  `controller.clear()`，键盘不收起。注意与 `horizontalScroll` 和子项
  `combinedClickable` 的手势关系，需真机/模拟器实测。
- 选区：`OpiImeService.onUpdateSelection` 增加「非自我提交 + 联想态 → clear」；
  自我提交守卫覆盖 `commitWithRetry`（`dropComposing` 的
  `deleteSurroundingTextInCodePoints` + `commitText` 可能产生两个选区事件）、
  `BracketCommit.insertPair`、`PetStickerCommit` 等直接插入点。建议用
  「短时间窗（150–250ms）+ 无条件更新 `selectionNonEmpty`」；一次性标记挡不住
  delete+commit 两个事件。
- 文件行数：`EngineController.kt` 498、`OpiImeService.kt` 500、`ImeScreen.kt` 500。
  实现必须拆文件（扩展属性/新 helper/把编辑方法移出），不能靠压注释。

### 5.2 iOS

- `KeyboardViewController.route` 最前面：若 `associationActive`（从上次 refresh 缓存）
  → `engine.clear()` + `refresh()`，然后继续原路由。硬件 `routeHardware` 首键同样
  先 dismiss。
- `didTapFunction` 的非候选导航分支（`.commitText`、`.segment` 空缓冲、`.retype`、
  `.showNumbers` 的 `leaveCompositionForNumbers`、面板直提）先 dismiss；
  `selectCandidate` **不** dismiss。
- `KeyboardNumbers.leaveCompositionForNumbers` 的 `guard !engine.buffer().isEmpty`
  在联想态（buffer 空）会直接返回，联想不会清；因此 `route` 顶部必须先 dismiss，
  不能只依赖这个函数。
- 选区：新增 `selectionDidChange(_:)`。自我插入守卫要覆盖 `insert`、
  `insertTyped`（括号配对直接 `proxy.insertText` + `adjustTextPosition`）、
  `deleteBackwardTyped`（直接 `proxy.deleteBackward()`）、数字层直提、面板直提。
  守卫窗口内只更新状态、不 dismiss；窗口外若联想态 → `engine.clear()` + `refresh()`。
- 候选栏空白点击：`KeyboardCandidateBar` 空槽 `isUserInteractionEnabled=false`，
  点击会落到父视图；在父视图加 tap gesture，回调 controller dismiss + refresh。
- `refresh()` 已用 `allCandidates(limit)`，`setTopBar(candidatesVisible:
  !texts.isEmpty || !buffer.isEmpty)` 已能显示联想，无需改可见性。
- 文件 500 行：新增 `KeyboardAssociation.swift`（extension）放 dismiss/选区/空白
  点击，`KeyboardViewController.swift` 只留一行调用。

### 5.3 HarmonyOS

- `KeyboardController.pressKey` 开头：若缓存 `associationActive` → `engine.clear()`
  + `publish()`，然后继续 `OpiEngine.keyEvent`。`commitText`、`openSymbolPage`、
  `openRare`、`selectInputMode`、`togglePanel`、`tapChar`（英文直提）、`tapSegment`
  （空缓冲直提 `'1'`）、`tapTiledCandidate` 等绕过引擎的入口也先 dismiss。
- `OpiEngine` 没有 pendingText 出口；联想态推断用：
  `buffer() == '' && displayBuffer() == '' && candidatesPage() 非空 &&
   mode() 为拼音/繁体`。在 `publish()` 后把结果缓存到 AppStorage（或 controller
  字段），避免每个键重新取候选。
- `StatePublish.publishState()` 已无条件取候选，`candidatesShowable` 已是「英文或
  缓冲非空或候选非空」，联想自动显示。
- 候选行 `TopBar.CandidateRow` 外层 `Row` 加 `onClick`；子候选 `Text` 的 `onClick`
  会消费，空白区冒泡需真机核。
- 选区：`inputMethodEngine.getKeyboardDelegate().on('selectionChange', cb)`，
  `onInputStart` 注册、`onInputStop`/`release` 退订（以 API 19 SDK 声明为准）。
  自我插入守卫覆盖 `insertText`、`deleteForward`、括号直提；窗口外联想态 →
  `engine.clear()` + `publish()`。
- 文件 498 行：选区注册/dismiss helper 放新 `AssociationGuard.ets`，controller 只
  加一行调用。

### 5.4 macOS

- `InputController.handle(_:client:)` 的**最前面**（在 Ctrl+'、Ctrl+\、Ctrl+Shift+F、
  Ctrl+Shift+Z、Shift+Space 等热键判断之前）dismiss；这样 Ctrl/⌘ 组合也会先收联想。
- `virtualKey` 屏内键盘入口最前面 dismiss；`commitFromOutside` dismiss + refresh。
- `refresh()`：
  - 候选窗可见性改为 `!buffer.isEmpty || !list.isEmpty`；
  - 现在只在 `list != lastCandidates` 时 `update`，**可见性变化也要触发 update**
    （否则从有候选到空缓冲联想时窗口不刷新）；
  - `candidateSelected` 现在提交后不 refresh（只 hide + commit），必须补 refresh，
    否则链式联想不出现。
- 文档鼠标点击：实现 `IMKMouseHandling.mouseDown(onCharacterIndex:coordinate:
  withModifier:continueTracking:client:)`，联想态时 dismiss 并 `return false`
  放行点击。`IMKCandidates` 候选窗本身没有空白区回调，空白点击不做。
- 文件 500 行、且头部标注「草案·从未编译·未验证」；新增 `InputControllerAssociation.swift`
  放 mouseDown/dismiss helper。改动只能靠 Apple workflow 编译验证（该 workflow
  无 PR 触发，需手动跑），实现时如实记录未真机验证的部分。

### 5.5 fcitx5

Rust 侧（`crates/fcitx5-opi/src/input_method.rs`，466 行）：

- `handle_key` 第一行**之前**（Ctrl/Alt 直通之前）：
  `if keydown { state.dismiss_association(); }`。不吞键；release 不 dismiss。
  这样空格/退格/回车空缓冲的 `PassThrough`、数字、方向键、Ctrl/Alt 组合都先收联想，
  再走原语义。
- `dismiss_association` 必须作废 `fetch_cache`：缓存键是 `(buffer, pending)`，
  联想态两者都不变，不失效会遮住新状态。放新 `candidate_association.rs`；
  `invalidate_fetch_cache` 提升为 `pub(crate)`。
- 提交/选词路径（`CandidateState::select`、`input_space` 等）已有
  `reset_page_if_buffer_changed`（无条件作废缓存）；联想建立后也要确保走到它。

C++ 侧（`crates/fcitx5-opi/cpp/opi_fcitx5.cpp`）：

- `refreshingPanelFor` 现在 `buffer.empty()` 就直接 `panel.reset()`。改为：
  1. 先 `take(opi_fcitx5_candidates(...))` 拿 `texts`；
  2. `buffer.empty() && texts.empty()` → `panel.reset()`；
  3. `buffer.empty() && !texts.empty()`（联想态）→ 先 `panel.reset()` 清 preedit，
     再只设候选表（**不设 preedit**）；`wantsClientPreedit` 时按现有尾部调用
     `ic->updatePreedit()` 清内联预编辑；
  4. `!buffer.empty()` → 现有逻辑。
- **联想态（buffer 空）不要设 selection key**：否则数字键会经
  `OpiCandidateWord::select` 直接选联想项，绕开「数字正常输入」。鼠标点选仍可用。
- `refreshingPanelFor` 末尾记录 `g_pushedHadCandidates = !texts.empty();`。
- `keyEventImpl` 最前面（`shiftToggle_.handle` 之前）：
  ```
  const bool associationShown = !keyEvent.isRelease()
      && g_pushedBuffer.empty() && g_pushedHadCandidates;
  if (associationShown) {
      take(opi_fcitx5_clear());       // 缓冲已空，等价于只清联想
      refreshingPanelFor(keyEvent.inputContext());
  }
  ```
  这覆盖 `handleModeHotkey` / `handleFullwidthHotkey` 这两个在 Rust 之前就 return
  的路径（全角键原本不 refresh，现在先清再刷新）。复用 `opi_fcitx5_clear()`
  会顺带清 `last_commit`，只影响学习撤销，接受并记录。
- 直通跳过条件加 `&& !associationShown`：
  ```
  if ((result.action == 0 || shiftOnly)
      && take(opi_fcitx5_buffer()) == g_pushedBuffer
      && !associationShown) return;
  ```
  否则 dismiss 后空缓冲不变，面板会停在旧候选上。
- `OpiCandidateWord::select` 点选后已 `refreshingPanelFor`，链式自动刷新。
- `opi_panel_driver.cpp` 是手工 harness（`run-harness.sh`，不进 CI）；第 4 节
  「点选后候选面板清空」的断言与本设计相反，需改成「点选后续接项/链式面板仍在」。

### 5.6 TSF

- `crates/tsf-opi/src/logic_snapshot.rs`：`StateSnapshot` 增加
  `association: self.engine.association_active()`；`composing` 保持
  `has_composing()`（缓冲或逐字片段非空）。caret 条件改为
  `snapshot.composing || snapshot.association`（联想态缓冲空，但仍需要 caret 定位）。
- `crates/tsf-opi/src/candidate_sink.rs` 的 `on_composition_changed`：
  现在 `buffer.is_empty()` 就 `hide()`；改为**缓冲空且候选空才 hide**，
  否则 `show(buffer, candidates, ...)`（buffer 可为空）。
- `crates/tsf-opi/src/key_sink.rs` 的 `OnTestKeyDown` 是**每个 keydown 都经过的唯一
  入口**：`NO_KEYVAL` 的方向键/功能键、Ctrl/Alt、Win 组合在测试阶段就返回 FALSE，
  根本不会进 `OnKeyDown`，所以 dismiss 放 `handle_key_inner` 覆盖不到它们。顺序：
  1. `self.drain_pending()`：排队的候选点选先落地。必须在 dismiss 之前 ——
     `select_and_insert(index)` 依赖当前候选表，先清联想会让点选越界、静默丢失；
     也必须在认领判定之前 —— 排队点选提交后缓冲已空，Enter/Tab 等应从「消费」
     改判「放行」；
  2. 锁内 `dismiss_association()`；返回 true 就抓 `snapshot`，锁外 `push_state`
     （测试阶段没有别处会推状态）；
  3. 再走既有的 `shift_tap_key_down`、热键、Ctrl/Alt/Win、`claim_on_test`。
  `handle_key_inner` 保留 `drain_pending()`（幂等；key-up 路径仍需要；从
  `key_sink.rs` 跨模块调用需提升为 `pub(crate)`），不再放 dismiss。
- `handle_key_inner` 的 `KeyOutcome::Commit` 现在只 `insert_into + sink.on_commit`，
  不抓/推快照；提交后若 `snapshot.association`，补 `caret + push_state`（顺序：
  `insert_into` → `on_commit`（hide）→ caret → `push_state`（show 联想））。
  `snapshot` 在 `input_key` 后锁内抓，Commit 分支也要抓。
- `Consumed` 不需要额外推：dismiss 时已经推过。
- `drain_pending` 已在 `select_and_insert` 后推快照；改完 `CandidateSink` 的
  可见性即可链式。
- `logic.rs` 498 行：`dismiss_association` 放新 `logic_association.rs`，
  内部 `invalidate_fetch_cache()`（已是 `pub(crate)`）。`tsf.rs` 486 行：
  dismiss+push / commit-push 的 helper 放新 `tsf_association.rs`。
- 两轨（fcitx5/TSF）的按键逻辑保持逐行同构；配对文件按既有约束同步改。

## 6. 光标/选区与空白点击

### 6.1 统一语义

联想根必须在文档末尾。光标移动或选区变化意味着用户不再位于根后面，必须收起联想。
六端都做；能观测到的用回调，观测不到的只覆盖键盘路径，并如实记录边界。

### 6.2 Android

- 回调：`onUpdateSelection`。
- 自我提交守卫：`commitWithRetry`（`dropComposing` + `commitText`）、括号直提、
  宠物直提等插入点设短时间窗；`onUpdateSelection` 无条件更新 `selectionNonEmpty`，
  窗口内不 dismiss，窗口外联想态 → `clear()`。
- 空白点击：`CandidateBar` 外层（见 §5.1）。

### 6.3 iOS

- 回调：新增 `selectionDidChange`。
- 守卫覆盖 `insert`、`insertTyped`（括号直提）、`deleteBackwardTyped`、数字层直提、
  面板直提；窗口外联想态 → `engine.clear() + refresh()`。
- 空白点击：候选栏父视图 tap gesture（见 §5.2）。

### 6.4 HarmonyOS / macOS

- HarmonyOS：`selectionChange`，`onInputStart` 注册、`onInputStop`/`release` 退订；
  守卫覆盖 `insertText`/`deleteForward`/括号直提。空白点击：候选行外层 `onClick`。
- macOS：`IMKMouseHandling.mouseDown` 观察文档鼠标点击，联想态 dismiss + 返回 false；
  键盘方向键/Home/End 由 `handle` 顶部的 dismiss 覆盖。`IMKCandidates` **没有**
  候选窗空白区回调，空白点击不做。

### 6.5 fcitx5 / TSF

- 鼠标/触摸选区没有可靠回调：只覆盖键盘方向键/Home/End（fcitx5 的 `handle_key`
  与 TSF 的 `OnTestKeyDown` 在放行前统一 dismiss），鼠标点击文档导致的选区变化
  不收起。**已知边界**，见 §11。
- 候选窗空白点击不做：fcitx5 面板由框架绘制，TSF 候选窗是独立窗口且收起要经管道
  新增控制动作；本轮不做。

### 6.6 清联想方式对照

| 端 | 清联想 | 是否清 `last_commit` | 影响 |
|---|---|---|---|
| fcitx5 / TSF（Rust） | `dismiss_association()` | 否 | 学习撤销锚点保留 |
| Android / iOS / HarmonyOS / macOS | `clear()` + refresh | 是 | 只影响学习撤销，不删文本 |
| fcitx5 C++ 热键早退路径 | `opi_fcitx5_clear()` | 是 | 同上；缓冲已空，安全 |

`last_commit` 只影响「同一输入下改选」的学习退票，不删除已上屏文本。非 Rust 宿主
没有 `dismiss_association` 出口，本设计不为它新增 FFI。

### 6.7 宿主联想态推断

- Android：`buffer` + `pendingText` + `candidates` + `mode`，精确。
- iOS/macOS/HarmonyOS：C ABI 没有 pending 出口，用
  `buffer 空 && displayBuffer 空 && candidates 非空 && mode 拼音/繁体` 推断；
  `displayBuffer` 含逐字片段，所以两者都空等价于 pending 空。结果是上次 refresh
  的缓存，不新增 FFI。
- fcitx5/TSF：直接用引擎的 `association_active()`/`dismiss_association()`。

## 7. 按键路由细节

`crates/engine-core/src/router.rs` 已 499 行，**不加联想分支、不加 `swallowed`**。
所有「特殊键正常执行」由宿主在按键入口先 dismiss 完成：

- Android `KeyRouter` 五个入口（§5.1）；
- iOS `route`/`routeHardware`/`didTapFunction` 非候选分支（§5.2）；
- Harmony `pressKey` 与绕过入口（§5.3）；
- macOS `handle` 顶部（热键之前）/`virtualKey`/`commitFromOutside`（§5.4）；
- fcitx5 Rust `handle_key` 顶部（Ctrl/Alt 之前）/C++ 热键前（§5.5）；
- TSF `OnTestKeyDown`（每个 keydown 都经过，含测试阶段放行的方向键/Ctrl/Alt/Win；
  在 `shift_tap_key_down` 之前 drain + dismiss）（§5.6）。

引擎入口的 `dismiss_association()` 是**兜底**：即使某个宿主漏了，直接调引擎的
路径（C ABI/JNI/测试）也不会带着旧联想进入新输入。但宿主不能只靠兜底——宿主在
决定「空格是否上屏」「数字是否选词」时读的是本地缓存，必须自己先清。

特殊键语义（联想态先 dismiss，然后）：

| 键 | 正常动作 |
|---|---|
| 空格 | 上屏空格（不选联想首项） |
| 退格 | 删文档最后一个字符/码点 |
| 回车 | 宿主回车动作（新行/发送/引擎回车设置） |
| 方向键 / Home / End | 移动光标（宿主/应用） |
| PageUp / PageDown | 翻页/应用动作 |
| Tab / Esc | 应用动作 |
| 数字 1–9 | 输入数字；不是选联想项 |
| 字母/标点 | 开始新输入 |
| ⇧ | 正常 ⇧ 状态机 |
| Ctrl/Alt/⌘ + 键 | 直通应用 |

fcitx5 特别条款：联想态不能设候选 selection key（§5.5），否则数字键会被
fcitx5 的候选机制直接吃掉。Android 数字直提路径在 dismiss + refresh 后候选为空，
自然落回正常输入；iOS/Harmony/macOS 的数字层直提在入口 dismiss 后插入数字。

## 8. 500 行门禁与文件拆分

实测行数（2026-10-27）：

- Rust：`loader.rs` 500、`router.rs` 499、`logic.rs` 498、`candidate.rs` 491、
  `engine_wubi.rs` 486、`tsf.rs` 486、`dict_layers.rs` 472、`input_method.rs` 466、
  `engine.rs` 461。
- Kotlin/Swift/ArkTS：`OpiImeService.kt` 500、`ImeScreen.kt` 500、
  `KeyboardViewController.swift` 500、`InputController.swift` 500、
  `EngineController.kt` 498、`KeyboardController.ets` 498。

实现时必须先拆再改：

- `loader.rs`：`#[path = "loader_tests.rs"] mod tests;` 拆测试；新增
  `loader_word_index.rs`、`loader_assoc.rs`。
- `format.rs` 373：assoc 序列化/校验若超过 500，拆 `format_assoc.rs`；否则内联。
- `engine-core`：新 `engine_association.rs`；`engine.rs` 的各入口只加一行
  `dismiss_association()`（约 10 行，461 → 471），`input_space_at` 加构建调用；
  `engine_wubi.rs` 486 只加 1–2 行；`dict_layers.rs` 472 加两个委托（约 10 行，
  仍 <500）；`router.rs` 不动。
- fcitx5：`candidate.rs` 491 → `candidate_association.rs`；`input_method.rs` 466
  加 dismiss 约 6 行，安全。
- TSF：`logic.rs` 498 → `logic_association.rs`；`tsf.rs` 486 → `tsf_association.rs`；
  `logic_snapshot.rs` 151 +1 字段；`candidate_sink.rs` 86 改判据。
- Android：`EngineControllerAssociation.kt` 扩展属性；`OpiImeService.kt` 的
  `onUpdateSelection`/编辑直提拆 helper；`ImeScreen.kt` 直接提交点抽 helper。
- iOS：`KeyboardAssociation.swift`（dismiss/选区/空白点击），controller 只留调用。
- macOS：`InputControllerAssociation.swift`（mouseDown/dismiss），controller 只留调用。
- Harmony：`AssociationGuard.ets`（dismiss/选区注册），controller 只留调用。

所有拆分遵守本仓既有约束：物理行 ≤500（`crates/opi-ffi/tests/line_limit.rs`），
机械搬移不夹带语义改动，搬完逐项核对。

## 9. 测试与验收

### 9.1 engine-core

新增测试（`crates/engine-core/tests/` 新文件 + 一个实现两个查询方法的测试词典）：

- 正控夹具必须带 `words_with_prefix` 与 `association_suffixes`；`InMemoryDictionary`
  默认空实现不能充当正控（现有 `tests/association.rs` 三条就是这种情况）。
- 提交 2 字根后出联想；续接项不含根、不是完整词条；前 3/长前缀 3/非前缀 2 的顺序
  与去重正确；assoc 与前缀有前缀关系时跳过。
- 链式：点续接项只提交续接部分；新根 = 旧根 + 续接；下一次联想以新根查询；
  1 字续接（「中国」+「人」）能继续链；新根不足 2 汉字/无命中时链结束。
- 1 字提交不触发；英文/数字/符号提交不触发；无索引词典不触发；
  `words_with_prefix("")`/`association_suffixes("")` 返回空。
- 文本键清联想；`input_space_at`（入口清，且空格不选联想项）、`backspace`、
  `clear`、`switch_mode`、`switch_language`、`set_scheme`、`toggle_symbol`、
  `set_shift`、`set_extra_dicts`、`set_trad_dict`、`set_stroke_dict`、
  `set_wubi_dict` 清联想。
- 传统模式走 trad 词典；trad 缺失时回退主库；五笔四码自动上屏后出联想；
  9/14 留尾不出联想，整段消耗出联想。
- `limit` 截断、根 16 字截断、联想点击不学习、`last_commit` 被清
  （不触发误选撤销）、链式仍可继续。
- `select_from_source` 返回 None 的路径（留尾/逐字）不建链、联想已清。
- `tests/adversarial_input.rs` 的「空缓冲 → 候选空」性质要显式排除联想态
  （或先 `dismiss_association()`），否则带联想的夹具会让性质失效。
- 现有 `tests/association.rs` 三条保持，并补「带索引时新输入仍从头开始」。

### 9.2 engine-data

- `serialize_with_extras` 往返：flags bit0/bit1、`Validated` 新字段、`parse` 条目不变。
- `assoc_rows` 为空且 `word_index=false` 时，产物与 `serialize` **逐字节相同**。
- flags=0 的旧文件仍通过；`BadVersion(2)` 用例仍红（版本仍 1）。
- 未知 flags 位 → `BadFlags`；`count*4`/`root_count*6`/`assoc_len` 回绕或截断 →
  `Truncated`/`BadOffsets`。
- 词索引有序性：写入产物逐条非降；`words_with_prefix` 的 lower_bound 边界
  （空前缀、无命中、全命中、重复词）。
- assoc 校验：root 非升序、root_off 不等 expected、最后不落在 assoc_end、
  root/suffix 非 UTF-8、`assoc_len` 不匹配、root_count 过大、空 root/suffix。
- 旧读者兼容性：**不重写旧解析器**。发布门禁用上一版 `opi-tools`（真实旧二进制）
  对新资产跑 `verify`，见 §9.5。

### 9.3 opi-tools / 真库

- `crates/opi-tools/tests/dict_crosslayer.rs` 的
  `committed_dictionaries_match_their_sources`：trad 必须用同一份
  `data/generated/trad.assoc.tsv` 调 `serialize_with_extras(entries, true, assoc)`
  再与 `data/generated/trad.opid` 逐字节比较；fallback（无 extras）保持
  `serialize` 对比。不要只比条目。
- `crates/opi-tools/tests/phrase_quality.rs` 的整文件
  `String::from_utf8_lossy(...).contains(word)` 会扫到 assoc 字节；改为**只遍历
  条目词**（`parse` 后的 entries）做 trad-only 检查。
- 真库门禁用 `android/app/src/main/assets/luna.opid`：
  - 断言 flags bit0/bit1、索引与 assoc 都存在；
  - 提交「今天」后候选非空、全部是续接项（不含根、不重复），并至少包含一个
    当前 assoc 表里的已知项（如「回家」；表重生成时同步更新该断言）；
  - 最坏根「什么」的前缀命中约 1257 条，commit 时查询不得卡顿（性能测试见 §9.6）。
- trad 至少断言 flags、assoc 非空、空缓冲联想非空；trad 的 assoc 行必须能复现
  `data/generated/trad.opid` 的字节。
- `scripts/gen_assoc_table.py` 的确定性：CI 重新生成 `{luna,trad}.assoc.tsv` 与
  入库文件 `diff`（若 luna 基线可复现；不可复现的部分如实记录）。

### 9.4 宿主

- Android：`KeyRouter` 单测覆盖联想态空格/退格/回车/数字的「先收起、再正常执行」；
  `associationActive` 推断；shift 后 refresh；`OpiImeService` 自我提交守卫；
  `assembleRelease`（debug 的 Rust 是 debug 编译，不作为性能判据）。
- fcitx5：Rust 轨按现有镜像测试加「keydown dismiss、release 不 dismiss、Ctrl/Alt
  也 dismiss、数字不选联想项」；C++ `opi_panel_driver` 手工 harness 更新断言
  （点选后链式面板仍在）；两轨逐行同构。
- TSF：`StateSnapshot.association`、`CandidateSink` 可见性、Commit 后推快照、
  `OnTestKeyDown` 的 drain→dismiss→认领顺序（含 `NO_KEYVAL`/Ctrl/Alt/Win 放行前
  收起）、caret 条件；Windows 目标测试按现有 CI 方式跑。
- iOS/macOS/HarmonyOS：编译门禁；macOS 可见性/点选后 refresh 路径检查；
  六端各自真机/模拟器验收：提交出联想、点选链式、任意键正常执行、光标移动收起、
  空白点击（四端）。
- Apple workflow 无 PR 触发、macOS 文件是未编译草案：实现时手动跑 Apple workflow，
  未跑到的部分在交付说明里如实列出。

### 9.5 发布前

- `cargo test --workspace --no-fail-fast`、clippy `-D warnings`、
  `cargo fmt --all --check`、`opi-ffi/tests/line_limit.rs`、engine-core 零 IO。
- `cargo metadata --locked`（必须带 `--locked`）。
- 旧读者门禁：

  ```
  git worktree add /tmp/opi-old v1.26.2
  (cd /tmp/opi-old && cargo run -q -p opi-tools -- verify \
      /home/wwwroot/bag/opi/android/app/src/main/assets/luna.opid)
  (cd /tmp/opi-old && cargo run -q -p opi-tools -- verify \
      /home/wwwroot/bag/opi/android/app/src/main/assets/trad.opid)
  git worktree remove /tmp/opi-old
  ```

  旧 `verify` 用的是旧 `engine-data` 的真实加载路径；通过 = 旧二进制能读新库。
- 新读者读旧库：`opi-tools verify data/generated/fallback.opid`（无 flags）仍通过。
- Android debug/release 单测与 APK；Apple 编译；fcitx5/TSF 两轨测试；Harmony 构建
  （自托管 runner 离线时如实记录未跑）。
- 真库资产与代码同批提交；`data/generated/{luna,trad}.assoc.tsv` 入库；
  `data/generated/luna.opid` 不入库。

### 9.6 性能

- 加一个 `#[ignore]` 基准：真实 luna 上提交「今天」「什么」「中国」各测一次
  `build_association`，记录 release 实测值（桌面/中端机）。
- 目标 <5ms 是**实测记录**，不是硬门禁；超过就报数并分析，不静默放宽。

## 10. 性能与资源预算

- 每次提交一次前缀二分 + 一次 assoc 二分；不在 `candidates()`/逐键路径里查库。
- 词索引 4B/条；assoc 约 123KiB（5k 根 cap3）。mmap 常驻增量同文件增量。
- 显示 O(8) 克隆；清联想 O(1)。
- 不新增线程、不新增后台任务、不写盘。
- 9/14 键只在整段提交后查一次，不在数字键逐键路径。

## 11. 已知边界（如实记录，不当缺陷重报）

- 导入词库层不参与联想（`LayeredDictionary` 只查 base）；用户导入的词不会出现在
  联想行，直到将来给导入层建索引。
- stroke/wubi 码表本身没有索引；但五笔/笔画方案提交中文词后，联想查的是
  `active_dict()`（主库/繁体库），所以**仍可联想**；只有主库/繁体库缺失时才无联想。
- 内置 fallback 没有索引/assoc；回退到内置库时无联想（不影响正常输入）。
- 单字提交不触发；但链式里 1 字续接项可以被点，累加后的根（≥2 汉字）可继续联想。
- 9/14 键留尾（`digit_tail_to_keep`）时不出联想；整段消耗才出。
- 语义续接是从短语表抽的常用搭配，不是统计语言模型；约 6 成可用，只显示 2 条
  非前缀项。反例见 §3.8，不逐条修数据是本设计的已知质量边界。
- 前缀续接允许 1 字；长前缀/语义续接 2–6 字；根被截到最后 16 字。
- 9/14 键联想行没有拼音读数（不是输入键序），左列显示由宿主决定。
- 根含标点/英文时按原样查询，无命中即无联想；汉字计数只算基本区。
- fcitx5/TSF 鼠标/触摸选区不可观测；只覆盖键盘方向键/Home/End。
- TSF 候选点选整体延迟到下一次按键（既有架构，见 `crates/tsf-opi/src/state.rs`
  模块头）：联想项可点选落字，但下一条链在同一 keydown 被收起，**不能链式**；
  即时插入要走 COM 异步编辑会话或 GIT 代理，两者都需真机验证。
- HarmonyOS 空缓冲回车无宿主动作（既存平台缺口，与联想无关）：IME Kit 有
  `InputClient.getEditorAttributeSync()` 与 `sendKeyFunction()`，补法明确，
  需真机验证；本设计不覆盖。
- macOS 候选窗（`IMKCandidates`）空白点击不可做；文档鼠标点击用 `IMKMouseHandling`。
- 非 Rust 宿主用 `clear()` 清联想会顺带清 `last_commit`；只影响学习撤销，不删文本。
- 词索引/assoc 的有序性不在加载时全量校验（理由见 §3.6），靠写入断言、checksum
  与构建门禁。
- 六端候选槽位数不同；最多 8 条，至少能在一页内显示，不新增分页语义。
- 联想点击不学习；用户连续点联想项不会提升任何词的词频。
- 不新增 FFI；Android 用现有状态推断，Apple/Harmony 用上次 refresh 缓存推断。

## 12. 我做的决定（用户复核这一节）

以下不是用户原话，是为落地做的判断。**已按用户裁决执行**的条目标注「用户已定」。

1. **续接项而不是完整词条**（§2.5）。用户已定「可以」；这是本设计最重要的取舍。
2. **特殊键正常执行**（§2.4/§7）。用户已定「正常执行（推荐）」；删除旧稿的
   `swallowed` 与「Ctrl/Alt 保持联想」。
3. **六端光标都做**（§6）。用户已定「六端都做」；平台不可观测处记边界。
4. **语义续接用现有短语表抽小表**（§3.8）。用户已定「用现有短语表抽小表（推荐）」；
   不引入外部语料/语言模型。
5. 触发条件：拼音/繁体、提交文本至少 2 个汉字、缓冲与逐字片段都空；单字不触发。
6. 配比：最多 8 = 前缀 3 + 长前缀 3 + 非前缀 2；非前缀只填剩余槽；assoc 与前缀
   有前缀关系时跳过。5,000 根、cap3、约 123KiB。
7. 链式根 = `cap16(旧根 + 本次提交文本)`；新根至少 2 汉字才 build；1 字续接可链。
8. 联想点击不学习、清 `last_commit`；链式仍可继续。
9. 格式用 v1 + flags bit0/bit1，不升版本；加载只校验边界/目录，不校验索引有序。
10. 只在 luna/trad 建索引/assoc；导入层、stroke、wubi、fallback 不建；五笔/笔画
    联想仍走 `active_dict()`。
11. 不新增 FFI：Android 用现有状态推断，Apple/Harmony 用 refresh 缓存，
    fcitx5/TSF 用引擎方法。
12. 空白点击：Android/iOS/Harmony 做；macOS/fcitx5/TSF 不做（候选窗所有权）。
13. 不新增开关，拼音/繁体下常开。
14. 9/14 键联想行无读音；不做额外 UI 行。
15. 汉字判定用基本区；根含标点/英文时按原样查询。
16. 联想建立放在 `select_from` 包装 + `input_space_at` 最终 out + 五笔自动上屏钩子；
    `select_from_source` 只负责识别联想点击、跳过学习、清 `last_commit`。
17. `input_space_at` 入口必须清联想（否则空格会选联想首项）。
18. fcitx5/TSF 的 `dismiss_association` 必须作废 `fetch_cache`；联想建立后也要
    确保缓存失效。
19. fcitx5 联想态不设候选 selection key（数字键正常输入）。
20. 非 Rust 宿主用 `clear()` 清联想，接受清 `last_commit`；fcitx5 C++ 热键早退
    路径复用 `opi_fcitx5_clear()`。
21. 性能 <5ms 作为实测记录，不设硬门禁。
22. 入库 `data/generated/{luna,trad}.assoc.tsv`；不入库 `data/generated/luna.opid`。
23. TSF 的 dismiss 放 `OnTestKeyDown`（每个 keydown 的唯一入口），而不是
    `handle_key_inner`：方向键/`NO_KEYVAL`/Ctrl/Alt/Win 在测试阶段就放行，后者
    覆盖不到；`drain_pending` 必须先于 dismiss 与认领判定，否则排队点选会丢或
    被误判为「消费」。

**需要用户确认的**（只有这些会改变构建范围/手感）：

- 第 6 条：5,000 根 / cap3 / 3+3+2、只显示 2 条非前缀，是否接受（语义表约 6 成
  可用，见 §3.8 反例）。
- 第 12 条：fcitx5/TSF 鼠标/触摸选区不可观测、macOS 候选窗空白点击不可做，是否
  接受为平台边界。
- 第 20 条：非 Rust 宿主用 `clear()` 会清 `last_commit`（只影响学习撤销），是否接受。
- 第 21 条：性能只记录实测、不设硬门禁，是否接受。
- 第 22 条：assoc TSV 入库（约 123KiB + 小表），是否接受。

其余条目是构建者可自行落地的工程取舍；若任一条不符合预期，请在批准前指出。

## 13. 发布与回滚

- 词库与二进制可任意顺序发布：旧二进制忽略尾部；新二进制在旧词库上联想为空。
- 回滚二进制不必回滚词库；回滚词库不必回滚二进制。
- 回滚词库（保留新二进制，联想变空但输入正常）：

  ```
  git checkout <old-tag> -- \
    android/app/src/main/assets/luna.opid \
    android/app/src/main/assets/trad.opid \
    data/generated/trad.opid
  scripts/prepare_harmony_dicts.sh
  ```

  `data/generated/luna.opid` 被 gitignore，需按 §3.9 从基线重建，或直接用
  Android assets 副本覆盖。旧 tag 没有 `{luna,trad}.assoc.tsv`（本轮新增文件）；
  它们只是构建输入，留着不影响新二进制在旧词库上运行（flags=0 ⇒ 无联想）。
- 回滚 assoc 表（保留新二进制与词索引，只关语义续接）：删掉或清空
  `{luna,trad}.assoc.tsv` 后重编，且**不带** `--assoc`（产物 flags bit1=0，
  前缀续接仍可用）；或直接重编不带 `--assoc`，不依赖旧表存在。
- 旧资产改造前后用 `opi-tools parse`/测试对比条目，索引/assoc 之外的字节与行为
  不变；旧读者门禁见 §9.5。
- 版本号与发布流程不在本设计内；实现完成后按既有发布清单走。
