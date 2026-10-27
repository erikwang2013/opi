# 智能联想（续接项 + 语义小表 + 六端接线）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 提交中文词后，候选行立即显示最多 8 个常用续接项；点续接项只上屏续接部分并以「旧根 + 续接」继续联想；六端同一套引擎语义，任意键先收起联想再正常执行。

**Architecture:** 数据层给 `.opid` v1 的可选尾部加「词排序索引 + 语义小表」（flags bit0/bit1，旧读者忽略尾部）；引擎在提交时用 `words_with_prefix` 与 `association_suffixes` 一次算好联想项存进 `Engine.association`，显示路径只克隆；宿主在按键入口先 `dismiss_association()`/`clear()` 再走原路由，不吞键、不新增 FFI。

**Tech Stack:** Rust workspace（engine-core / engine-data / opi-tools / fcitx5-opi / tsf-opi，MSRV 1.88，edition 2024）、Kotlin/Android、Swift/iOS+macOS、ArkTS/HarmonyOS、Python 3 标准库。

**Spec:** `docs/superpowers/specs/2026-10-27-smart-association-design.md`

## Global Constraints

- OPID **版本号保持 1**；`serialize` 的输出字节必须与改动前逐字节相同。
- 旧二进制必须能读新词库；新二进制读旧词库时联想为空、其余行为不变。
- `crates/engine-core/src/` 不允许文件 IO：`grep -rn "std::fs" crates/engine-core/src/` 必须零命中。
- 所有源码文件物理行 ≤ 500（`crates/opi-ffi/tests/line_limit.rs`）；文档不受限。先拆再改，机械搬移不夹带语义改动。
- **不新增 FFI 出口**；`crates/engine-core/src/router.rs` 不加联想分支、不加 `swallowed`。
- 只在 luna/trad 打开索引/assoc；fallback、stroke、wubi86、wubi98、导入层不开。
- 联想只在 `Mode::Pinyin` / `Mode::Traditional`、缓冲空、逐字片段空、根至少 2 个基本区汉字时产生。
- 最多 8 项，固定配比 前缀 3 + 长前缀 3 + 非前缀 2；续接项**不含根**。
- 联想点击不记学习、清 `last_commit`；链式根 = `cap16(旧根 + 本次提交文本)`。
- 任何键先收起联想再正常执行；数字键不是联想选择键；绝不吞键。
- 不新增依赖；Python 脚本只用标准库。
- 提交信息不加 `Co-Authored-By`。
- 所有命令从仓库根执行；**唯一例外是 Gradle**：仓库根没有 `gradlew`，Android 命令写成 `(cd android && ./gradlew ...)`。提交前每个任务必须跑完自己的测试命令。

## Review Focus

1. **联想态按空格/退格/回车/数字必须正常执行**，不吞键、不误选联想项 —— 空格上屏空格、退格删文档字符、回车走宿主回车动作、数字输入数字。
2. **光标移动/选区变化后联想必须收起**；能观测选区的宿主用回调，观测不到的只覆盖键盘方向键/Home/End 并如实记录。
3. **空缓冲且无联想时不得显示候选，也不得把普通状态误判成联想态**（English/数字/符号、缓冲非空、逐字片段非空都必须排除）。
4. **链式必须自然结束且不重复上屏**：新根无命中、根超过 16 字、1 字续接后不足 2 汉字都不应崩溃或把旧根再上屏一遍。
5. **旧二进制读新词库、新二进制读旧词库都正常**（数据兼容不能只靠重写一个旧读取器证明）。

---

## File Structure

**engine-data**
- `format.rs`：基础 v1 布局 + `Validated` 新字段 + `validate` 调尾部校验。
- `format_assoc.rs`（新）：`AssocRow` / `AssocView` / `serialize_with_extras` / 尾部解析与校验。
- `loader.rs`：`MmapDictionary` 新字段、`from_validated`、trait 委托。
- `loader_tests.rs`（新，从 `loader.rs` 搬出）：现有加载器测试。
- `loader_word_index.rs`（新）：词索引 lower_bound + 前缀收集。
- `loader_assoc.rs`（新）：assoc 目录二分 + 后缀读取排序。

**engine-core**
- `dictionary.rs`：`Dictionary` 两个默认方法。
- `dict_layers.rs`：`LayeredDictionary` 委托 base。
- `engine_association.rs`（新）：`Association` 状态、`build_association`、候选注入辅助。
- `engine.rs`：字段初始化、清空矩阵、`input_space_at` 拆分。
- `engine_candidates.rs`：联想候选注入。
- `engine_learning.rs`：联想点击识别、跳学习、链式重建。
- `engine_punct.rs`：冲刷改走不建联想的 core。
- `engine_wubi.rs`：自动上屏后建联想。
- `engine_dicts.rs`：设置词典清联想。

**opi-tools**
- `compiler.rs`：`parse_assoc`、`compile_file_with_extras`。
- `main.rs`：`--word-index` / `--assoc`。
- `tests/cli_extras.rs`（新）、`tests/association_real.rs`（新）。
- `tests/dict_crosslayer.rs`、`tests/phrase_quality.rs`：随 extras 修正判据。

**scripts / data**
- `scripts/gen_assoc_table.py`（新）、`scripts/tests/test_gen_assoc_table.py`（新）。
- `data/generated/{luna,trad}.assoc.tsv`（新，入库）。
- `android/app/src/main/assets/{luna,trad}.opid`、`data/generated/trad.opid`（重编）。

**宿主**
- Android：`EngineControllerAssociation.kt`、`ImeCommitGuard.kt`（新）；`KeyRouter.kt`、`EngineController.kt`、`ImeScreen.kt`、`CandidateBar.kt`、`OpiImeService.kt`、`ComposingText.kt`。
- iOS：`KeyboardAssociation.swift`（新）；`KeyboardViewController.swift`、`KeyboardCandidates.swift`、`KeyboardCandidateBar.swift`。
- HarmonyOS：`AssociationGuard.ets`（新）；`KeyboardController.ets`、`StatePublish.ets`、`KeyboardAppStorage.ets`、`TopBar.ets`。
- macOS：`InputControllerAssociation.swift`（新）；`InputController.swift`。
- fcitx5：`candidate_association.rs`（新）、`input_method.rs`、`candidate.rs`、`cpp/opi_fcitx5.cpp`、`cpp/opi_panel_driver.cpp`。
- TSF：`logic_association.rs`、`tsf_association.rs`（新）；`tsf.rs`、`key_sink.rs`、`logic_snapshot.rs`（挂载点）、`candidate_sink.rs`。

---

### Task 1: OPID 尾部序列化与校验

**Files:**
- Create: `crates/engine-data/src/format_assoc.rs`
- Modify: `crates/engine-data/src/format.rs`
- Modify: `crates/engine-data/src/lib.rs`
- Test: `crates/engine-data/src/format_assoc.rs` 内联 `#[cfg(test)] mod tests`（超过 500 行时拆 `format_assoc_tests.rs` 并用 `#[path]` 引入）

**Interfaces:**
- Consumes: `format::{OpDict, RawEntry, FormatError, MAGIC, VERSION, HEADER_LEN, ENTRY_LEN, TRAILER_LEN, serialize}`、`checksum::fnv1a64`。
- Produces:
  - `pub struct AssocRow { pub root: String, pub suffix: String, pub rank: u32 }`
  - `pub struct AssocView { pub start: usize, pub len: usize, pub root_count: usize, pub records_start: usize }`
  - `pub fn serialize_with_extras(dict: &OpDict, word_index: bool, assoc_rows: &[AssocRow]) -> Vec<u8>`
  - `FormatError::BadFlags(u8)`
  - `Validated { count, pinyin_total, max_freq, flags: u8, word_end: usize, word_index: Option<usize>, assoc: Option<AssocView> }`
  - lib.rs：`pub mod format_assoc;`，并 `pub use format_assoc::{AssocRow, AssocView, serialize_with_extras};`（`FormatError` 已在 `pub use format::{...}` 里，`BadFlags` 随之导出）。

- [ ] **Step 1: 写失败测试（布局、往返、负例）**

在 `format_assoc.rs` 写测试；夹具用两条拼音有序、词可重复的条目：

```rust
fn sample() -> OpDict {
    OpDict {
        entries: vec![
            RawEntry { pinyin: "jintian".into(), word: "今天".into(), freq: 900 },
            RawEntry { pinyin: "jintian".into(), word: "今天".into(), freq: 800 },
            RawEntry { pinyin: "jintianwanshang".into(), word: "今天晚上".into(), freq: 700 },
            RawEntry { pinyin: "huijia".into(), word: "回家".into(), freq: 600 },
        ],
        pinyin_total: "jintianjintianjintianwanshanghuijia".len(),
    }
}

#[test]
fn extras_absent_is_byte_identical_to_serialize() {
    let d = sample();
    assert_eq!(serialize_with_extras(&d, false, &[]), format::serialize(&d));
}

#[test]
fn roundtrip_flags_index_and_assoc() {
    let d = sample();
    let rows = vec![
        AssocRow { root: "今天".into(), suffix: "回家".into(), rank: 600 },
        AssocRow { root: "今天".into(), suffix: "晚上好".into(), rank: 0 },
    ];
    let bytes = serialize_with_extras(&d, true, &rows);
    let v = format::validate(&bytes).expect("新文件必须通过校验");
    assert_eq!(v.flags, 0b11);
    assert!(v.word_index.is_some());
    let assoc = v.assoc.expect("assoc");
    assert_eq!(assoc.root_count, 1);
    assert_eq!(format::parse(&bytes).unwrap().entries.len(), 4);
}

#[test]
fn assoc_without_index_sets_only_bit1() {
    let rows = vec![AssocRow { root: "今天".into(), suffix: "回家".into(), rank: 600 }];
    let bytes = serialize_with_extras(&sample(), false, &rows);
    let v = format::validate(&bytes).expect("只开 assoc 也要通过校验");
    assert_eq!(v.flags, 0b10);
    assert!(v.word_index.is_none());
    assert_eq!(v.assoc.expect("assoc").root_count, 1);
    assert_eq!(format::parse(&bytes).unwrap().entries.len(), 4);
}

#[test]
fn unknown_flag_bit_is_rejected() {
    let mut bytes = serialize_with_extras(&sample(), true, &[]);
    // flags 在 offset 6，不在 checksum 覆盖的 [HEADER_LEN..tail) 内，改它不用重算 FNV。
    bytes[6] |= 0b100;
    assert!(matches!(format::validate(&bytes).unwrap_err(), FormatError::BadFlags(0b100)));
}

#[test]
fn word_index_is_sorted_by_word_bytes() {
    let bytes = serialize_with_extras(&sample(), true, &[]);
    let v = format::validate(&bytes).unwrap();
    let start = v.word_index.expect("词索引起始偏移");
    let dict = format::parse(&bytes).unwrap();
    let words: Vec<&str> = (0..v.count)
        .map(|i| {
            let off = start + i * 4;
            let idx = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
            dict.entries[idx].word.as_str()
        })
        .collect();
    assert!(
        words.windows(2).all(|w| w[0] <= w[1]),
        "词索引必须按词字节序非降（Spec §9.2）：{words:?}"
    );
}
```

同一测试模块再补负例（除 flags 外都先 `reseal` 重算 FNV，确保走到结构校验而不是 checksum）：
`assoc_root_out_of_order_rejected`、`assoc_root_offset_gap_rejected`、
`assoc_last_record_off_end_rejected`、`assoc_len_mismatch_rejected`、
`word_index_out_of_range_rejected`、`truncated_index_rejected`，
以及 `oversized_tail_lengths_do_not_panic`：把 `count` / `assoc_len` / 目录里的
`root_off` 改成 `u32::MAX` 附近的值再 `reseal`，断言只返回 `Truncated` /
`BadOffsets` 之类的 `Err`，不 panic、不越界切片（所有 `count*4`、`root_count*6`、
`assoc_len` 的算术都用 `checked_mul`/`checked_add`）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-data format_assoc`
Expected: 编译失败，`serialize_with_extras` / `AssocRow` / `BadFlags` 未定义。

- [ ] **Step 3: 实现**

`format.rs`：
1. 把 `serialize` 的公共部分抽成 `pub(crate) fn build_parts(dict: &OpDict) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<&RawEntry>)`（entry table、pinyin blob、word blob、按 pinyin 排好的条目；`format_assoc` 要调它）；`serialize` 用它拼 header + trailer，输出必须不变。
2. `FormatError` 加 `BadFlags(u8)`（该枚举没有 `Display` 实现，不必补）；`validate` 发现未知位时返回 `BadFlags(flags & !0b11)`（只报未知位，测试断言 `0b100`）。
3. `Validated` 加 `flags`、`word_end`、`word_index: Option<usize>`、`assoc: Option<AssocView>`。
4. `validate` 在读 flags 后调用 `format_assoc::parse_tail(data, count, word_start, tail)`；用返回的 `word_end` 计算真实 `word_total`；条目边界检查改用真实 `word_total`。

`format_assoc.rs`：
- `serialize_with_extras`：`!word_index && assoc_rows.is_empty()` 时直接 `return format::serialize(dict)`；否则用 `build_parts` 拼基础段，**按存在性独立算 flags**（`if word_index { flags |= 0b01 }`、`if !assoc_rows.is_empty() { flags |= 0b10 }`；bit0/bit1 相互独立，Spec §3.2，不能无条件写 `0b11`），再按 bit0/bit1 追加：
  - 词索引：`count × u32`，条目索引按 `(词字节升序, freq 降序, entry 下标升序)` 排序；
  - assoc：按 root 字节分组，root 升序；records canonical 紧排；目录 `{root_off u32, suffix_count u8, root_len u8}`；同 root 内按 `(rank desc, len asc, bytes asc)` 写；最后写 `assoc_len u32`；
  - trailer：`fnv1a64(&out[HEADER_LEN..])`。
- `parse_tail`（`pub(crate)`）：按 Spec §3.2 从 `tail` 反推 `assoc_end/assoc_start/index_start/word_end`，全部 checked 算术；**bit1 无 bit0 时 `word_end = assoc_start`**（`index_start` 为空）；校验 `assoc_len >= 4`、`word_start <= assoc_start/index_start`、索引值 `< count`、目录严格升序、`root_off == expected`、root/suffix UTF-8 且非空、最后 `expected == assoc_end`。
- 写入前断言：索引按词有序、assoc 目录按 root 严格升序、同 root+suffix 不重复、长度不溢出 u8/u32。
- **不**在加载时校验索引有序性（Spec §3.6）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-data`
Expected: 全部 PASS，且既有 `layout_byte_by_byte` / `roundtrip` 仍绿。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-data/src/format.rs crates/engine-data/src/format_assoc.rs crates/engine-data/src/lib.rs
git commit -m "feat(engine-data): OPID v1 可选词索引与语义小表尾部"
```

---

### Task 2: 加载器拆测试 + 词排序索引查询

**Files:**
- Modify: `crates/engine-data/src/loader.rs`
- Create: `crates/engine-data/src/loader_tests.rs`（从 `loader.rs` 尾部原样搬出现有测试）
- Create: `crates/engine-data/src/loader_word_index.rs`
- Test: `loader_word_index.rs` 内联 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: Task 1 的 `Validated.word_index`、`serialize_with_extras`。
- Produces:
  - `MmapDictionary` 新私有字段 `word_index_start: Option<usize>`、`assoc: Option<AssocView>`；
  - `impl MmapDictionary { pub(super) fn words_with_prefix_impl(&self, prefix: &str) -> Vec<(String, u32)>; }`
  - 空 prefix、无索引、`count == 0` 均返回空。

- [ ] **Step 1: 写失败测试**

在 `loader_word_index.rs` 写（`use super::*;` + `use crate::format::serialize;` + `use crate::format_assoc::serialize_with_extras;` —— 父模块 `loader.rs` 只导入 `validate`，`use super::*` 拿不到 `serialize`；`sample()` 复制 Task 1 的那 4 条夹具）。

```rust
#[test]
fn prefix_query_returns_word_and_weight_in_word_order() {
    let bytes = fixture_with_index(); // serialize_with_extras(&sample(), true, &[])
    let d = load_bytes(bytes).expect("load");
    // sample() 里「今天」出现两次（900/800）：索引按 (词, freq 降序, entry 下标) 排，
    // 查询结果**不去重**，去重是联想层的事（Spec §3.3）。
    assert_eq!(
        d.words_with_prefix_impl("今天"),
        vec![
            ("今天".to_string(), 900),
            ("今天".to_string(), 800),
            ("今天晚上".to_string(), 700),
        ]
    );
}

#[test]
fn empty_prefix_and_missing_index_return_empty() {
    let d = load_bytes(fixture_with_index()).unwrap();
    assert!(d.words_with_prefix_impl("").is_empty());
    let plain = load_bytes(serialize(&sample())).unwrap();
    assert!(plain.words_with_prefix_impl("今天").is_empty());
}
```

再补边界：无命中、全命中（prefix = 第一条的词首字节）、同词多读音返回多条。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-data loader_word_index`
Expected: 编译失败（字段/方法不存在）。

- [ ] **Step 3: 实现**

1. 把 `loader.rs` 尾部现有 `#[cfg(test)] mod tests { ... }` 原样搬到 `loader_tests.rs`，在 `loader.rs` 用：
   ```rust
   #[cfg(test)]
   #[path = "loader_tests.rs"]
   mod tests;
   ```
2. `MmapDictionary` 加字段并在 `from_validated` 里从 `v.word_index` / `v.assoc` 填充。
3. `loader_word_index.rs` 实现 `words_with_prefix_impl`：
   - `word_index_start` 为 `Some(start)` 时，在 `start..start+count*4` 上对 entry 下标做 lower_bound（比较目标词的 UTF-8 字节与 prefix 字节）；
   - lower_bound 后顺序扫描到不再 `starts_with(prefix)`，逐条物化 `(word, freq)`；
   - 读取 word 用 `record(data, idx)` 的 `wo/wl` + `word_start()`。
4. 不在加载时校验索引有序性。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-data`
Expected: 新测试与搬出的旧测试全部 PASS；`loader.rs` 行数 < 500。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-data/src/loader.rs crates/engine-data/src/loader_tests.rs crates/engine-data/src/loader_word_index.rs
git commit -m "feat(engine-data): 词排序索引查询与加载器测试拆分"
```

---

### Task 3: 加载器语义小表查询

**Files:**
- Create: `crates/engine-data/src/loader_assoc.rs`
- Test: `loader_assoc.rs` 内联 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: Task 1 的 `AssocView` / `Validated.assoc`；Task 2 的 `MmapDictionary` 字段。
- Produces: `impl MmapDictionary { pub(super) fn association_suffixes_impl(&self, root: &str) -> Vec<(String, u32)>; }`

- [ ] **Step 1: 写失败测试**

`use super::*;` + `use crate::format::serialize;` + `use crate::format_assoc::{AssocRow, serialize_with_extras};`；`sample()` 同 Task 1。

```rust
#[test]
fn suffixes_are_sorted_by_rank_then_length_then_bytes() {
    let rows = vec![
        AssocRow { root: "今天".into(), suffix: "晚上".into(), rank: 10 },
        AssocRow { root: "今天".into(), suffix: "回家".into(), rank: 90 },
        AssocRow { root: "今天".into(), suffix: "下午茶".into(), rank: 10 },
        AssocRow { root: "中国".into(), suffix: "人民".into(), rank: 80 },
    ];
    let d = load_bytes(serialize_with_extras(&sample(), true, &rows)).unwrap();
    assert_eq!(
        d.association_suffixes_impl("今天"),
        vec![("回家".to_string(), 90), ("晚上".to_string(), 10), ("下午茶".to_string(), 10)]
    );
    assert!(d.association_suffixes_impl("今天").iter().all(|(s, _)| s != "人民"));
}

#[test]
fn empty_root_missing_root_and_missing_table_return_empty() {
    let rows = vec![AssocRow { root: "今天".into(), suffix: "回家".into(), rank: 90 }];
    let d = load_bytes(serialize_with_extras(&sample(), true, &rows)).unwrap();
    assert!(d.association_suffixes_impl("").is_empty());
    assert!(d.association_suffixes_impl("不存在").is_empty());
    let plain = load_bytes(serialize(&sample())).unwrap();
    assert!(plain.association_suffixes_impl("今天").is_empty());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-data loader_assoc`
Expected: 编译失败（方法不存在）。

- [ ] **Step 3: 实现**

`association_suffixes_impl`：
1. `root.is_empty()` / `assoc.is_none()` / `root_count == 0` → 空。
2. 目录起点 `assoc.start + 4`，每条 6B；对 root 字节做二分（严格升序）。
3. 命中后读 `root_off`、`suffix_count`、`root_len`；记录从 `assoc.start + root_off` 开始；跳过 root 字节后逐条读 `{len u8, suffix bytes, rank u32}`。
4. 按 `(rank desc, len asc, bytes asc)` 排序返回；`suffix.len()` 用字节数。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-data`
Expected: 全部 PASS。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-data/src/loader_assoc.rs
git commit -m "feat(engine-data): 语义小表二分查询"
```

---

### Task 4: Dictionary 接口与 LayeredDictionary 委托

**Files:**
- Modify: `crates/engine-core/src/dictionary.rs`
- Modify: `crates/engine-core/src/dict_layers.rs`
- Modify: `crates/engine-data/src/loader.rs`（trait 覆写）
- Test: `crates/engine-core/src/dict_layers.rs` 加 `#[cfg(test)] #[path = "dict_layers_assoc_tests.rs"] mod assoc_tests;`（新文件；`dict_layers.rs` 已 472 行，不要内联）

**Interfaces:**
- Consumes: Task 2/3 的 `words_with_prefix_impl` / `association_suffixes_impl`。
- Produces:
  - `Dictionary::words_with_prefix(&self, prefix: &str) -> Vec<(String, u32)>`（默认空）
  - `Dictionary::association_suffixes(&self, root: &str) -> Vec<(String, u32)>`（默认空）
  - `LayeredDictionary` 只委托 `base`，导入层不参与。

- [ ] **Step 1: 写失败测试**

在 `dict_layers_assoc_tests.rs` 写；`StubDict` 只实现必需的 `query`/`len`，再覆写两个联想方法（字段 `prefix`/`suffixes` 按上面的夹具）。`use super::*;` 后可直接用 `LayeredDictionary::new`/`set_extras`。

```rust
#[test]
fn layered_dictionary_delegates_association_to_base_only() {
    let mut base = StubDict::default();
    base.prefix = vec![("今天晚上".into(), 7)];
    base.suffixes = vec![("回家".into(), 9)];
    let mut d = LayeredDictionary::new(Box::new(base));
    d.set_extras(vec![("导入".into(), {
        let mut e = InMemoryDictionary::new();
        e.insert("jintian", "今天导入", 99);
        e
    })]);
    assert_eq!(d.words_with_prefix("今天"), vec![("今天晚上".into(), 7)]);
    assert_eq!(d.association_suffixes("今天"), vec![("回家".into(), 9)]);
}

#[test]
fn in_memory_dictionary_has_no_association() {
    let mut d = InMemoryDictionary::new();
    d.insert("jintian", "今天", 1);
    assert!(d.words_with_prefix("今天").is_empty());
    assert!(d.association_suffixes("今天").is_empty());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core layered_dictionary_delegates_association_to_base_only`
Expected: 编译失败（trait 方法不存在）。

- [ ] **Step 3: 实现**

1. `Dictionary` trait 尾部加两个默认方法（空实现），文档写明「空串必须返回空」。
2. `LayeredDictionary` 的 `impl Dictionary` 里加：
   ```rust
   fn words_with_prefix(&self, prefix: &str) -> Vec<(String, u32)> {
       self.base.words_with_prefix(prefix)
   }
   fn association_suffixes(&self, root: &str) -> Vec<(String, u32)> {
       self.base.association_suffixes(root)
   }
   ```
3. `MmapDictionary` 的 `impl Dictionary` 里委托到 `words_with_prefix_impl` / `association_suffixes_impl`。
4. `InMemoryDictionary` 不覆写（默认空）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core dict_layers && cargo test -p engine-data`
Expected: 全部 PASS；`dict_layers.rs` < 500 行。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-core/src/dictionary.rs crates/engine-core/src/dict_layers.rs crates/engine-core/src/dict_layers_assoc_tests.rs crates/engine-data/src/loader.rs
git commit -m "feat(engine-core): Dictionary 联想查询接口与层委托"
```

---

### Task 5: 引擎联想状态、生成算法与候选注入

**Files:**
- Create: `crates/engine-core/src/engine_association.rs`
- Modify: `crates/engine-core/src/engine.rs`（字段 + 模块声明）
- Modify: `crates/engine-core/src/engine_candidates.rs`
- Modify: `crates/engine-core/src/engine_learning.rs`（`select_from` 成功后建联想）
- Modify: `crates/engine-core/src/lib.rs`（如模块声明在那里）
- Modify: `crates/engine-core/tests/association.rs`（补带索引/assoc 的契约测试，Spec §4.6）
- Modify: `crates/engine-core/tests/adversarial_input.rs`（「空缓冲 ⇒ 候选空」性质排除联想态，Spec §9.1）
- Test: `crates/engine-core/tests/association_engine.rs`、`crates/engine-core/tests/common/mod.rs`（加 `AssocDict` 夹具）；根截断测试放 `engine_association.rs` 内联 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: Task 4 的两个 trait 方法；`Candidate { text, kind: Hanzi, score }`。
- Produces:
  - `pub(crate) struct Association { pub(crate) root: String, pub(crate) items: Vec<Candidate> }`
  - `Engine.association: Option<Association>`（`pub(crate)`）
  - `pub fn association_active(&self) -> bool`
  - `pub fn dismiss_association(&mut self) -> bool`
  - `pub(crate) fn build_association(&mut self, root: &str)`（集成测试不可见，根截断用内联单测）
  - `select_from` 在所有非空输出后调用 `build_association(&out)`；`select_from_source` 不建（Task 7 细化链式）。
  - `candidates_base` 在空缓冲时返回 `association.items` 的截断。
- 夹具 `AssocDict`：
  ```rust
  pub struct AssocDict {
      pub entries: Vec<(String, String, u32)>,       // (pinyin, word, freq)
      pub prefixes: Vec<(String, Vec<(String, u32)>)>, // (root, [(word, weight)])
      pub suffixes: Vec<(String, Vec<(String, u32)>)>, // (root, [(suffix, rank)])
  }
  ```
  实现 `query`（按拼音过滤、freq 降序）、`visit_entries`（供 9/14 键）、`words_with_prefix`、`association_suffixes`、`len`、`max_freq`；`query_prefix_top` 用默认。`prefixes`/`suffixes` 按 root **精确查表**（模拟索引，不做字符串扫描），这样「新根无命中」的负例才可控。

- [ ] **Step 1: 写失败测试**

`association_engine.rs` 用夹具驱动「输入 pinyin → select 候选」：

```rust
fn engine() -> Engine {
    let d = AssocDict::new()
        .word("jintian", "今天", 900)
        .word("tian", "天", 400)
        .prefix("今天", &[
            ("今天", 900),              // 与根相同：跳过
            ("今天晚上", 700),          // 续接「晚上」
            ("今天下午", 600),          // 「下午」
            ("今天天气", 550),          // 「天气」
            ("今天早上", 540),          // 「早上」
            ("今天上午", 530),          // 「上午」
            ("今天回家", 520),          // 「回家」
            ("今天气", 500),            // 「气」1 字：只够填 b 的 2–6 字约束，不入选
        ])
        .suffix("今天", &[("吃饭", 90), ("真好", 80), ("晚上好", 70)]);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

fn find(e: &Engine, word: &str) -> usize {
    e.candidates(usize::MAX)
        .iter()
        .position(|c| c.text == word)
        .unwrap_or_else(|| panic!("候选里没有 {word}"))
}

#[test]
fn two_char_commit_shows_continuations_without_root() {
    let mut e = engine();
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    assert_eq!(e.select(idx), "今天");
    assert!(e.association_active());
    let got: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    // 3 前缀（晚上/下午/天气）+ 3 长前缀（早上/上午/回家）+ 2 assoc（吃饭/真好）；
    // 「晚上好」与已选「晚上」有前缀关系，跳过。
    assert_eq!(got, vec!["晚上", "下午", "天气", "早上", "上午", "回家", "吃饭", "真好"]);
    assert!(!got.iter().any(|s| s.contains("今天")));
}

#[test]
fn single_char_commit_does_not_associate() {
    let mut e = engine();
    type_str(&mut e, "tian");
    let idx = find(&e, "天");
    assert_eq!(e.select(idx), "天");
    assert!(!e.association_active());
}

#[test]
fn dictionary_default_methods_return_empty() {
    // `Dictionary` 的两个默认方法（Spec §3.7）：`InMemoryDictionary` 不覆写 ⇒ 一律空。
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 900);
    assert!(d.words_with_prefix("").is_empty());
    assert!(d.words_with_prefix("你好").is_empty());
    assert!(d.association_suffixes("你好").is_empty());
}

#[test]
fn in_memory_dictionary_does_not_associate() {
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 900);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    type_str(&mut e, "nihao");
    let idx = find(&e, "你好");
    assert_eq!(e.select(idx), "你好");
    assert!(!e.association_active(), "无词索引/assoc 的词典不得产生联想");
}

#[test]
fn non_chinese_modes_do_not_associate() {
    for mode in [Mode::English, Mode::Number, Mode::Symbol] {
        let mut e = engine();
        e.switch_mode(mode);
        type_str(&mut e, "nihao");
        let _ = e.select(0); // 有候选就提交；没有就是空串，两者都不得建联想
        assert!(!e.association_active(), "{mode:?} 提交后不得联想");
    }
}

#[test]
fn limit_truncates_association() {
    let mut e = engine();
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    e.select(idx);
    assert_eq!(e.candidates(3).len(), 3);
    assert_eq!(e.candidates(0).len(), 0);
}
```

再补：assoc 与已选前缀有前缀关系时被跳过；前缀去重（同续接文本只出现一次）。这两条用上面的夹具或变体断言即可。**根 20 字只取最后 16 字**放 `engine_association.rs` 内联单测（`build_association` 是 `pub(crate)`，集成测试不可见）。

**Spec §4.6 契约测试（写进现有 `tests/association.rs`，文件头裁决注释保持不变）**：加 `mod common;` 与 `use common::AssocDict;`，再写：

```rust
fn assoc_engine() -> Engine {
    let d = AssocDict::new()
        .word("nihao", "你好", 900)
        .word("nihaoma", "你好吗", 500)
        .word("ma", "吗", 800)
        .word("ma", "妈", 700)
        .prefix("你好", &[("你好吗", 500)])
        .suffix("你好", &[("吗", 0)]);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

#[test]
fn association_does_not_leak_into_the_next_input() {
    let mut e = assoc_engine();
    type_str(&mut e, "nihao");
    assert_eq!(e.select(0), "你好");
    assert!(e.association_active(), "前置：提交后进入联想态");
    assert!(texts(&e).contains(&"吗".to_string()), "前置：联想候选可见");

    type_str(&mut e, "ma");
    let got = texts(&e);
    assert!(!got.contains(&"你好吗".to_string()), "联想不得拼进下文：{got:?}");
    assert_eq!(e.select(0), "吗");
}
```

**Spec §9.1 性质测试（`tests/adversarial_input.rs` 约 line 343–352）**：把「缓冲空 ⇒ 候选空」显式排除联想态：

```rust
// 缓冲空 ⇒ 候选必空（联想态是唯一合法例外：缓冲空但候选非空）
if e.buffer().is_empty() && !e.association_active() {
    prop_assert!(e.candidates(8).is_empty(), "空缓冲仍有候选");
}
```

同文件（`engine_association.rs`）的内联单测再补一条**直接打模式判据**的用例（不依赖公开路由是否恰好提交）：

```rust
#[test]
fn non_chinese_modes_do_not_build_association() {
    let mut e = Engine::new(
        Box::new(InMemoryDictionary::new()),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    for mode in [Mode::English, Mode::Number, Mode::Symbol] {
        e.switch_mode(mode);
        e.build_association("你好");
        assert!(!e.association_active(), "{mode:?} 不得建联想");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --test association_engine`
Expected: 编译失败（`association_active` / `build_association` 不存在）。

- [ ] **Step 3: 实现**

`engine_association.rs`：
- `Association` 结构体与三个方法：
  ```rust
  pub fn association_active(&self) -> bool {
      matches!(self.composer.session().mode, Mode::Pinyin | Mode::Traditional)
          && self.composer.session().buffer.is_empty()
          && self.pending_text().is_empty()
          && self.association.is_some()
  }
  pub fn dismiss_association(&mut self) -> bool {
      self.association.take().is_some()
  }
  ```
- `build_association` 按 Spec §4.2：**第一行 `self.association = None;`**，之后任何前置失败/空结果都保持 None（否则 1 字提交会把上一次联想留在屏幕上）。
  1. `root = last_16_chars(root)`；模式必须 Pinyin/Traditional；`buffer().is_empty() && pending_text().is_empty()`；`cjk_count(root) >= 2`。
  2. `conts: BTreeMap<String, u32>` 收 `self.active_dict().words_with_prefix(root)`（**必须走 `active_dict()`**，繁体模式才用 trad 表；Task 8 的繁体测试钉这一点），跳过 `word == root`，续接 = 去掉 root 前缀，同键取最大权重。
  3. 排序 `(weight desc, cont bytes asc)`；`a` = 前 3 条（任意长度）；`b` = 剩余里续接 2–6 字的前 3 条。
  4. `selected` 文本集合；遍历 `self.active_dict().association_suffixes(root)`（已排序），跳过重复、与任一已选项有前缀关系（任一方向）的后缀，取 2 条。
  5. 构造 `Candidate { text, kind: CandidateKind::Hanzi, score: weight/rank as u64 }`；空则 `association = None`，否则 Some。
- `cjk_count` 只算 `'\u{4E00}'..='\u{9FFF}'`；`last_16_chars` 用 `chars().rev().take(16).collect::<Vec<_>>().into_iter().rev().collect()` 或等价。
- `engine.rs`：字段 `pub(crate) association: Option<Association>`，构造处 `None`；`mod engine_association;`。
- `engine_learning.rs` 的 `select_from`：**普通提交也必须建联想**（这一步不做，整条功能不成立）。本任务先写：
  ```rust
  pub fn select_from(&mut self, cands: &[Candidate], index: usize) -> String {
      let out = self.select_from_source(cands, index, SelectionSource::Explicit);
      if !out.is_empty() {
          self.build_association(&out);
      }
      out
  }
  ```
  Task 7 会把它改成「联想点击用旧根 + 续接」的版本；`select_from_source` 本任务不建联想（它同时服务 `input_space_at`，那边要先拼 pending）。
- `engine_candidates.rs`：在符号浏览、英文、携带缓冲之后、`matches!(mode, Pinyin|Traditional)` 之前插入：
  ```rust
  if s.buffer.is_empty() && let Some(a) = &self.association {
      return a.items.iter().take(limit).cloned().collect();
  }
  ```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --test association_engine && cargo test -p engine-core`
Expected: 全部 PASS；`association.rs` 既有三条仍绿。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-core/src/engine_association.rs crates/engine-core/src/engine.rs crates/engine-core/src/engine_candidates.rs crates/engine-core/src/engine_learning.rs crates/engine-core/tests/association_engine.rs crates/engine-core/tests/association.rs crates/engine-core/tests/adversarial_input.rs crates/engine-core/tests/common/mod.rs
git commit -m "feat(engine-core): 提交后联想状态与生成算法"
```

---

### Task 6: 清空矩阵

**Files:**
- Modify: `crates/engine-core/src/engine.rs`
- Modify: `crates/engine-core/src/engine_dicts.rs`
- Modify: `crates/engine-core/src/engine_punct.rs`
- Test: `crates/engine-core/tests/association_lifecycle.rs`

**Interfaces:**
- Consumes: Task 5 的 `dismiss_association()`。
- Produces: 以下入口先清联想：`input_key`、`input_punct_at`、`input_space_at`、`backspace`、`clear`、`switch_mode`、`switch_language`、`set_scheme`、`toggle_symbol`、`set_shift`、`set_extra_dicts`、`set_trad_dict`、`set_stroke_dict`、`set_wubi_dict`；`select_from_source` 在 Task 7 处理。

- [ ] **Step 1: 写失败测试**

```rust
fn active_engine() -> Engine {
    // 用 tests/common 的 AssocDict（Task 5 加）：type_str("jintian") → select 「今天」→ 断言 active。
    // 集成测试不能调 pub(crate) 的 build_association。
}

#[test]
fn every_input_entry_clears_association() {
    let mut e = active_engine();
    e.input_key('a');
    assert!(!e.association_active(), "input_key");

    let mut e = active_engine();
    e.backspace();
    assert!(!e.association_active(), "backspace");

    let mut e = active_engine();
    let _ = e.input_punct_at('，', 0);
    assert!(!e.association_active(), "input_punct_at");

    let mut e = active_engine();
    e.clear();
    assert!(!e.association_active(), "clear");

    let mut e = active_engine();
    e.switch_mode(Mode::English);
    assert!(!e.association_active(), "switch_mode");

    let mut e = active_engine();
    e.switch_language(Mode::English, Scheme::Full);
    assert!(!e.association_active(), "switch_language");

    let mut e = active_engine();
    e.set_scheme(Scheme::Nine);
    assert!(!e.association_active(), "set_scheme");

    let mut e = active_engine();
    e.toggle_symbol();
    assert!(!e.association_active(), "toggle_symbol");

    let mut e = active_engine();
    e.set_shift(true);
    assert!(!e.association_active(), "set_shift");
}

#[test]
fn dictionary_setters_clear_association() {
    let mut e = active_engine();
    e.set_extra_dicts(Vec::new());
    assert!(!e.association_active(), "set_extra_dicts");
    // 其余三个逐个重建 active_engine 后调用 set_trad_dict(None)/set_stroke_dict(None)/set_wubi_dict(None)
}
```

`input_space_at` 的空格语义测试放 Task 7（会建/不建联想，需要链式上下文）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --test association_lifecycle every_input_entry_clears_association`
Expected: FAIL（联想仍 active）。

- [ ] **Step 3: 实现**

在上述每个入口的**第一行**调用 `self.dismiss_association();`（`engine_dicts.rs` 的三个 setter 同样）。`input_punct_at` 另按下节处理内部冲刷。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --test association_lifecycle && cargo test -p engine-core`
Expected: 全部 PASS；`engine.rs` < 500 行。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-core/src/engine.rs crates/engine-core/src/engine_dicts.rs crates/engine-core/src/engine_punct.rs crates/engine-core/tests/association_lifecycle.rs
git commit -m "feat(engine-core): 联想清空矩阵"
```

---

### Task 7: 选词链式、学习跳过与空格路径

**Files:**
- Modify: `crates/engine-core/src/engine_learning.rs`
- Modify: `crates/engine-core/src/engine_association.rs`（`association_click_root`）
- Modify: `crates/engine-core/src/engine.rs`（`input_space_at` 拆分）
- Modify: `crates/engine-core/src/engine_punct.rs`（冲刷改走 core）
- Test: `crates/engine-core/tests/association_lifecycle.rs`

**Interfaces:**
- Consumes: Task 5 `Association` / `build_association`；Task 6 清空矩阵。
- Produces:
  - `input_space_at(&mut self, skip) -> String` = `dismiss_association()` + `input_space_core(skip)` + 非空时 `build_association(&out)`；
  - `pub(crate) input_space_core(&mut self, skip) -> String`（原实现体，不建联想）；
  - `select_from` 捕获旧根；点击联想时 `build_association(&format!("{root}{out}"))`，否则仍是 `build_association(&out)`；
  - `pub(crate) association_click_root(&self, cands, index) -> Option<String>`；
  - `select_from_source` 识别联想点击：跳过 `learn_selection`、`last_commit.clear()`、无条件清 `association`，但自己不建联想。

- [ ] **Step 1: 写失败测试**

夹具：`prefix("今天", [("今天晚上", 700)])`、`prefix("今天晚上", [("今天晚上好", 600)])`、
`prefix("中国", [("中国人", 500)])`，以及正控变体再加 `prefix("中国人", [("中国人多", 400)])`。
`AssocDict` 的 `prefix`/`suffix` 按 root 精确查表（模拟索引，不做字符串前缀扫描）。

```rust
#[test]
fn clicking_continuation_commits_only_continuation_and_chains() {
    let mut e = active_engine_with_chain(); // 已选「今天」进入联想态
    let before = e.export_user_words();      // 「今天」本身的记账发生在点击之前
    let first = e.candidates(8);
    let idx = first.iter().position(|c| c.text == "晚上").unwrap();
    assert_eq!(e.select_from(&first, idx), "晚上");
    assert_eq!(e.buffer(), "");
    assert!(e.association_active());
    let second: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert!(second.iter().all(|s| s != "今天晚上"));
    assert!(second.iter().any(|s| s == "好"), "链式根 = 今天晚上");
    assert_eq!(e.export_user_words(), before, "联想点击不得记账");
}

#[test]
fn one_char_continuation_can_chain() {
    let mut e = chain_engine(true); // 中国 → 中国人 → 中国人多
    assert_eq!(e.select(0), "人");
    assert!(e.association_active());
}

#[test]
fn chain_ends_when_new_root_has_no_hits() {
    let mut e = chain_engine(false); // 无 中国人 的索引
    let _ = e.select(0);
    assert!(!e.association_active(), "中国人 无命中 → 联想结束");
}

#[test]
fn space_at_entry_dismisses_and_never_selects_association() {
    let mut e = active_engine();
    assert_eq!(e.input_space_at(0), "");
    assert!(!e.association_active());
    assert_eq!(e.buffer(), "");
}
```

再补：`select_from_source` 返回 `None` 时（9/14 留尾）联想已清且不建链；`last_commit` 被清（连续两次联想点击都不触发学习退票，用 `export_user_words` 为空间接证明）。根 20 字截断已在 Task 5 的内联单测里，本任务不重复。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --test association_lifecycle clicking_continuation_commits_only_continuation_and_chains`
Expected: FAIL（提交的是完整词或联想不链）。

- [ ] **Step 3: 实现**

1. `engine.rs` 把 `input_space_at` 原体改名为 `pub(crate) fn input_space_core(&mut self, skip: usize) -> String`，新 `input_space_at`：
   ```rust
   pub fn input_space_at(&mut self, skip: usize) -> String {
       self.dismiss_association();
       let out = self.input_space_core(skip);
       if !out.is_empty() {
           self.build_association(&out);
       }
       out
   }
   ```
   注意 `input_space_core` 内不能再调 `dismiss_association`（避免重复）。
2. `engine_punct.rs` 的「缓冲非空先冲刷」改调 `self.input_space_core(skip)`，使标点路径不建联想；入口仍 `dismiss_association()`。
3. `engine_association.rs` 加识别辅助（`select_from` 与 `select_from_source` 共用，必须在 `dismiss_association` 之前调用）：
   ```rust
   pub(crate) fn association_click_root(&self, cands: &[Candidate], index: usize) -> Option<String> {
       // 与 Spec §4.4 一致：association_active() 已含模式 / 缓冲空 / pending 空的判据。
       if !self.association_active() { return None; }
       let a = self.association.as_ref()?;
       let c = cands.get(index)?;
       a.items.iter().any(|x| x.text == c.text).then(|| a.root.clone())
   }
   ```
4. `engine_learning.rs` `select_from` 改成（普通提交与联想点击都建联想）：
   ```rust
   let old_root = self.association_click_root(cands, index);
   let out = self.select_from_source(cands, index, SelectionSource::Explicit);
   if !out.is_empty() {
       match old_root {
           Some(root) => self.build_association(&format!("{root}{out}")),
           None => self.build_association(&out),
       }
   }
   out
   ```
5. `select_from_source` 入口（在现有 `after_digit = false` 之后）：
   ```rust
   let is_association = self.association_click_root(cands, index).is_some();
   self.dismiss_association();
   ```
   把 `learn_selection` 包成 `if is_association { self.last_commit.clear(); } else { self.learn_selection(...); }`。
   `select_from_source` 自己**不**建联想：`input_space_at` 走它时还没拼 pending。
6. 9/14 留尾：`commit_buffer_after_select` 返回 `None` → `out` 空 → 不建链（无需额外代码，补测试钉住）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --test association_lifecycle && cargo test -p engine-core`
Expected: 全部 PASS；`engine.rs` < 500 行。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-core/src/engine_learning.rs crates/engine-core/src/engine_association.rs crates/engine-core/src/engine.rs crates/engine-core/src/engine_punct.rs crates/engine-core/tests/association_lifecycle.rs
git commit -m "feat(engine-core): 联想点击链式与空格路径"
```

---

### Task 8: 方案钩子（五笔自动上屏、9/14 键、繁体）

**Files:**
- Modify: `crates/engine-core/src/engine_wubi.rs`
- Test: `crates/engine-core/tests/association_schemes.rs`

**Interfaces:**
- Consumes: Task 5/7 的 `build_association`。
- Produces:
  - `wubi_autocommit` 在 `composer.commit_buffer()` 后调用 `build_association(&text)`；
  - 9/14 键留尾不出联想、整段消耗出联想（由 Task 7 逻辑自然满足，补测试）；
  - 繁体模式 `active_dict()` 为 trad 时用 trad 的前缀/assoc。

- [ ] **Step 1: 写失败测试**

```rust
mod common;

use common::{AssocDict, no_symbols};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::{Engine, Mode, Scheme};

fn texts(e: &Engine) -> Vec<String> {
    e.candidates(8).into_iter().map(|c| c.text).collect()
}

fn find(e: &Engine, word: &str) -> usize {
    e.candidates(usize::MAX)
        .iter()
        .position(|c| c.text == word)
        .unwrap_or_else(|| panic!("候选里没有 {word}：{:?}", texts(e)))
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

#[test]
fn wubi_auto_commit_builds_association() {
    let main = AssocDict::new()
        .word("nihao", "你好", 900)
        .prefix("你好", &[("你好吗", 500)]);
    let mut wubi = InMemoryDictionary::new();
    wubi.insert("abcd", "你好", 900);
    let mut e = Engine::new(Box::new(main), no_symbols(), true);
    e.set_wubi_dict(Some(Box::new(wubi)));
    e.set_scheme(Scheme::Wubi);

    type_str(&mut e, "abcd");
    assert_eq!(e.buffer(), "", "前置：四码唯一自动上屏");
    assert!(e.association_active(), "自动上屏也要建联想");
    assert!(texts(&e).contains(&"吗".to_string()));
}

fn nine_engine() -> Engine {
    let main = AssocDict::new()
        .word("nihao", "你好", 900)
        .prefix("你好", &[("你好吗", 500)]);
    let mut e = Engine::new(Box::new(main), no_symbols(), true);
    e.set_scheme(Scheme::Nine);
    e
}

#[test]
fn nine_tail_kept_does_not_associate_but_full_consumption_does() {
    let mut tail = nine_engine();
    type_str(&mut tail, "644264"); // nihao + 尾巴 4
    let idx = find(&tail, "你好");
    assert_eq!(tail.select(idx), "你好");
    assert!(!tail.buffer().is_empty(), "前置：尾巴保留");
    assert!(!tail.association_active(), "缓冲非空不得联想");

    let mut full = nine_engine();
    type_str(&mut full, "64426"); // 恰好 nihao
    let idx = find(&full, "你好");
    assert_eq!(full.select(idx), "你好");
    assert_eq!(full.buffer(), "");
    assert!(full.association_active(), "整段消耗后建联想");
}

fn trad_pair() -> (AssocDict, AssocDict) {
    let main = AssocDict::new()
        .word("jintian", "今天", 900)
        .suffix("今天", &[("吃饭", 90)]);
    let trad = AssocDict::new()
        .word("jintian", "今天", 900)
        .suffix("今天", &[("回家", 90)]);
    (main, trad)
}

#[test]
fn traditional_mode_uses_trad_dictionary_for_association() {
    let (main, trad) = trad_pair();
    let mut e = Engine::with_dictionaries(Box::new(main), Some(Box::new(trad)), no_symbols(), true);
    e.switch_mode(Mode::Traditional);
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    assert_eq!(e.select(idx), "今天");
    assert!(e.association_active());
    assert_eq!(texts(&e), vec!["回家"], "繁体模式必须读 trad 的 assoc");
}

#[test]
fn traditional_without_trad_dict_falls_back_to_main() {
    let (main, _) = trad_pair();
    let mut e = Engine::with_dictionaries(Box::new(main), None, no_symbols(), true);
    e.switch_mode(Mode::Traditional);
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    assert_eq!(e.select(idx), "今天");
    assert!(e.association_active());
    assert_eq!(texts(&e), vec!["吃饭"], "trad 缺失时回退主库");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --test association_schemes`
Expected: 五笔/9 键用例 FAIL（联想未建立）。

- [ ] **Step 3: 实现**

`engine_wubi.rs` 的 `wubi_autocommit`：
```rust
self.composer.commit_buffer();
self.build_association(&text);
Some(text)
```
9/14 与繁体只在测试里钉行为；若测试暴露差异，修在 `build_association`/`select_from` 的通用判据，不为方案加分支。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --test association_schemes && cargo test -p engine-core`
Expected: 全部 PASS；`engine_wubi.rs` < 500 行。

- [ ] **Step 5: 提交**

```bash
git add crates/engine-core/src/engine_wubi.rs crates/engine-core/tests/association_schemes.rs
git commit -m "feat(engine-core): 五笔自动上屏与 9/14 键联想钩子"
```

---

### Task 9: opi-tools 的 assoc 解析与编译开关

**Files:**
- Modify: `crates/opi-tools/src/compiler.rs`
- Modify: `crates/opi-tools/src/main.rs`
- Create: `crates/opi-tools/tests/cli_extras.rs`
- Test: `compiler.rs` 内联测试 + `cli_extras.rs`

**Interfaces:**
- Consumes: Task 1 的 `AssocRow` / `serialize_with_extras`。
- Produces:
  - `pub fn parse_assoc(text: &str) -> Result<Vec<AssocRow>, String>`
  - `pub fn compile_file_with_extras(report: Parsed, input: &Path, output: &Path, word_index: bool, assoc_rows: &[AssocRow]) -> Result<(), String>`（`Parsed` 就是现有 `compile_file` 的第一参类型，`pub use engine_data::dict_text::{Parsed, ...}` 已导出）
  - CLI：`opi-tools compile [--word-index] [--assoc <assoc.tsv>] <input> <output>`；空 assoc 文件 = 不开 bit1。

- [ ] **Step 1: 写失败测试**

`compiler.rs` 内联：

```rust
#[test]
fn parse_assoc_reads_root_suffix_rank() {
    let rows = parse_assoc("今天\t回家\t600\n中国\t人民\t500\n").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], AssocRow { root: "今天".into(), suffix: "回家".into(), rank: 600 });
}

#[test]
fn parse_assoc_rejects_malformed_duplicate_and_empty_fields() {
    assert!(parse_assoc("今天\t回家\n").is_err());
    assert!(parse_assoc("今天\t回家\tx\n").is_err());
    assert!(parse_assoc("\t回家\t1\n").is_err());
    assert!(parse_assoc("今天\t回家\t1\n今天\t回家\t2\n").is_err());
}
```

`cli_extras.rs`：

```rust
use engine_data::{serialize, validate};
use opi_tools::compiler::{compile, parse_dict};
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("opi-cli-extras-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn compile_with_index_and_assoc_verifies_and_default_stays_plain() {
    let d = dir("extras");
    let tsv = d.join("t.tsv");
    let assoc = d.join("a.tsv");
    let out = d.join("t.opid");
    let plain = d.join("plain.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n今天\tjintian\t900\n").unwrap();
    std::fs::write(&assoc, "今天\t回家\t600\n今天\t晚上好\t0\n").unwrap();

    let o = run(&[
        "compile", "--word-index", "--assoc", assoc.to_str().unwrap(),
        tsv.to_str().unwrap(), out.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let bytes = std::fs::read(&out).unwrap();
    let v = validate(&bytes).unwrap();
    assert_eq!(v.flags, 0b11);
    assert!(v.word_index.is_some());
    assert_eq!(v.assoc.expect("assoc").root_count, 1);

    let verify = run(&["verify", out.to_str().unwrap()]);
    assert!(verify.status.success(), "{}", String::from_utf8_lossy(&verify.stderr));

    let o = run(&["compile", tsv.to_str().unwrap(), plain.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let expected = serialize(&compile(parse_dict(&std::fs::read_to_string(&tsv).unwrap())));
    assert_eq!(std::fs::read(&plain).unwrap(), expected, "默认 compile 字节必须不变");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn empty_assoc_file_does_not_enable_bit1() {
    let d = dir("empty");
    let tsv = d.join("t.tsv");
    let assoc = d.join("empty.tsv");
    let out = d.join("t.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n").unwrap();
    std::fs::write(&assoc, "").unwrap();

    let o = run(&[
        "compile", "--word-index", "--assoc", assoc.to_str().unwrap(),
        tsv.to_str().unwrap(), out.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(validate(&std::fs::read(&out).unwrap()).unwrap().flags, 0b01);
    let _ = std::fs::remove_dir_all(&d);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p opi-tools --test cli_extras`
Expected: CLI 把 `--word-index` 当输入文件，报读文件失败/usage。

- [ ] **Step 3: 实现**

1. `compiler.rs`：`parse_assoc` 按行拆分，跳过空行；必须 3 列；root/suffix 非空；rank 解析 u32；同 root+suffix 重复报错（带行号）。
2. `compile_file` 改为 `compile_file_with_extras(report, input, output, false, &[])` 的薄包装。
3. `compile_file_with_extras` 用 `serialize_with_extras(&compile(report), word_index, assoc_rows)`。
4. `main.rs`：从 `args[2..]` 解析前置选项（`--word-index` 无值、`--assoc` 取下一参数）；遇到第一个非选项作为 input，下一个作为 output；多余参数报 usage。
5. 更新 usage 字符串。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p opi-tools --test cli_extras && cargo test -p opi-tools`
Expected: 全部 PASS；既有 CLI 测试不变。

- [ ] **Step 5: 提交**

```bash
git add crates/opi-tools/src/compiler.rs crates/opi-tools/src/main.rs crates/opi-tools/tests/cli_extras.rs
git commit -m "feat(opi-tools): assoc 小表解析与 compile 开关"
```

---

### Task 10: 语义小表生成脚本与单测

**Files:**
- Create: `scripts/gen_assoc_table.py`
- Create: `scripts/tests/test_gen_assoc_table.py`
- Test: 脚本单测

**Interfaces:**
- Consumes: `scripts/merge_luna_kangxi.py` 的 `read_opid(path) -> list[(word, pinyin, freq)]`。
- Produces: `scripts/gen_assoc_table.py --opid <词库.opid> --phrases <短语.tsv> --out <assoc.tsv> --roots 5000 --cap 3`；输出 TSV `root<TAB>suffix<TAB>rank`，root 字节升序，无重复。

- [ ] **Step 1: 写失败测试**

`test_gen_assoc_table.py` 用纯函数夹具（不依赖 33MB 词库）：

```python
class RootSelectionTest(unittest.TestCase):
    def test_roots_are_2_3_cjk_ranked_by_phrase_occurrences(self):
        words = {"今天": 900, "中国": 800, "中国人": 700, "的": 1}
        phrases = ["今天回家", "今天下午", "中国声音", "中国政策"]
        roots = g.select_roots(words, phrases, roots=2)
        self.assertEqual(roots, ["中国", "今天"])  # 同次数按字节序

    def test_suffixes_skip_particles_and_dictionary_leaves(self):
        rows = g.collect_rows(
            roots=["今天"],
            words={"今天回家": 600, "今天": 900},
            phrases=["今天回家", "今天了", "今天的", "今天下午"],
            cap=3,
        )
        self.assertNotIn(("今天", "了"), [(r.root, r.suffix) for r in rows])
        self.assertNotIn(("今天", "的"), [(r.root, r.suffix) for r in rows])
        # 今天回家 已是词库词 → 不写入
        self.assertNotIn(("今天", "回家"), [(r.root, r.suffix) for r in rows])
        self.assertIn(("今天", "下午"), [(r.root, r.suffix) for r in rows])

    def test_output_is_sorted_and_unique(self):
        rows = g.collect_rows(roots=["今天", "中国"], words={}, phrases=["今天下午", "中国政策"], cap=3)
        self.assertEqual(g.format_rows(rows), sorted(g.format_rows(rows)))
```

再加一条集成测试：用 `data/generated/fallback.opid` 跑 `g.read_opid`，断言能读出 35 条且不读尾部；若文件不存在则 `skipTest`。

- [ ] **Step 2: 运行测试确认失败**

Run: `python3 scripts/tests/test_gen_assoc_table.py -v`
Expected: `ModuleNotFoundError` / 函数不存在。

- [ ] **Step 3: 实现**

1. CLI 用 `argparse`；`ROOT = Path(__file__).resolve().parent.parent`；`sys.path.insert(0, str(Path(__file__).resolve().parent))` 后 `from merge_luna_kangxi import read_opid`。
2. `select_roots`：候选根 = 词库中 2–3 个 CJK 的词，字典 `word -> max(freq)`；扫全部短语，对每个位置取 2/3 字子串，若在候选根集合里则计数；按 `(count desc, root bytes asc)` 取前 `roots`。
3. `collect_rows`：对选中根集合再扫短语，每处根后取长度 2/3/4 的 CJK 后缀（不足则跳过）；后缀首字不在 `的了呢吗啊啦吧得地`；`root+suffix` 不得是词库词；按 `(rank desc, count desc, len asc, bytes asc)` 取前 `cap`；`rank` = 词库中 `root+suffix` 的 freq（没有则 0）。
4. `format_rows`：按 root 字节升序，同 root 按 `(rank desc, len asc, bytes asc)`；输出无表头。
5. 短语表是 `word<TAB>pinyin<TAB>freq`（`extra_phrases.tsv` / `trad_phrases.tsv` 实测都是三列）：**按 tab 切分，取第 1 列作为短语**；第 1 列必须非空且全 CJK，否则跳过并计数；拼音与第 3 列不使用。别把整行当短语 —— 那样 `pinyin` 里的 ASCII 会让所有行都被跳过。
6. 两遍扫描，不按 `roots × phrases` 做 250M 次查找。

- [ ] **Step 4: 运行测试确认通过**

Run: `python3 scripts/tests/test_gen_assoc_table.py -v`
Expected: 全部 PASS。

- [ ] **Step 5: 提交**

```bash
git add scripts/gen_assoc_table.py scripts/tests/test_gen_assoc_table.py
git commit -m "feat(scripts): 从短语表生成语义联想小表"
```

---

### Task 11: 重建 luna/trad 资产、更新跨层门禁与旧读者验证

**Files:**
- Modify: `crates/opi-tools/tests/dict_crosslayer.rs`
- Modify: `crates/opi-tools/tests/phrase_quality.rs`
- Create: `crates/opi-tools/tests/association_real.rs`
- Generated (commit): `data/generated/{luna,trad}.assoc.tsv`、`data/generated/trad.opid`、`android/app/src/main/assets/{luna,trad}.opid`
- Generated (do not commit): `data/generated/luna.opid`

**Interfaces:**
- Consumes: Task 9 CLI、Task 10 脚本、Task 5 引擎。
- Produces: 两份真词库带 flags=`0b11`；跨层测试用同一份 assoc 重编；真库联想门禁。

- [ ] **Step 1: 写失败测试**

1. 改 `dict_crosslayer.rs` 的 trad 分支：**保留**现有 `use engine_data::format::serialize;`（`good_bytes()`、fallback 分支、trad 重建前的旧路径仍在用），**新增** `use engine_data::serialize_with_extras;`，并把 `use opi_tools::compiler::{compile, parse_dict};` 扩成 `{compile, parse_assoc, parse_dict}`：
   ```rust
   let assoc = parse_assoc(&std::fs::read_to_string(TRAD_ASSOC).expect("读 trad.assoc.tsv")).unwrap();
   let rebuilt = serialize_with_extras(&compile(parse_dict(&merged)), true, &assoc);
   ```
   并加 `const TRAD_ASSOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/generated/trad.assoc.tsv");`。fallback 分支保持 `serialize`。
2. 改 `phrase_quality.rs` 的 `no_traditional_word_forms_leaked_into_luna`：不再对整文件 `String::from_utf8_lossy` 做 `contains`，改为：
   ```rust
   fn words(path: &str) -> Vec<String> {
       let bytes = std::fs::read(path).unwrap();
       engine_data::parse(&bytes).unwrap().entries.into_iter().map(|e| e.word).collect()
   }
   ```
   trad 正控用 `words(TRAD_OPID).iter().any(|w| w == word)`，luna 否定同法。
3. 新建 `association_real.rs`：
   ```rust
   use engine_core::dictionary::Dictionary;
   use engine_core::symbols::SymbolEngine;
   use engine_core::Engine;

   const LUNA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../android/app/src/main/assets/luna.opid");
   const TRAD: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../android/app/src/main/assets/trad.opid");
   const LUNA_ASSOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/generated/luna.assoc.tsv");

   fn type_str(e: &mut Engine, s: &str) {
       for c in s.chars() {
           e.input_key(c);
       }
   }

   fn known_suffixes(root: &str) -> Vec<String> {
       opi_tools::compiler::parse_assoc(&std::fs::read_to_string(LUNA_ASSOC).unwrap())
           .unwrap()
           .into_iter()
           .filter(|r| r.root == root)
           .map(|r| r.suffix)
           .collect()
   }

   #[test]
   fn luna_has_index_and_assoc_and_associates_after_commit() {
       let bytes = std::fs::read(LUNA).unwrap();
       let v = engine_data::validate(&bytes).unwrap();
       assert_eq!(v.flags, 0b11);
       assert!(v.word_index.is_some());
       assert!(v.assoc.expect("luna assoc").root_count > 0);
       let dict = engine_data::load_bytes(bytes).unwrap();
       // 不硬编码语义表内容：先钉词索引真的能按前缀取词（luna 有「今天晚上」）。
       let prefixed = dict.words_with_prefix("今天");
       assert!(prefixed.iter().any(|(w, _)| w == "今天晚上"), "索引缺 今天→今天晚上：{prefixed:?}");
       let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), false);
       type_str(&mut e, "jintian");
       let idx = e.candidates(512).iter().position(|c| c.text == "今天").expect("候选缺 今天");
       assert_eq!(e.select(idx), "今天");
       assert!(e.association_active());
       let got: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
       assert!(!got.is_empty());
       assert!(!got.iter().any(|s| s.contains("今天")), "续接项不得包含根：{got:?}");
       let mut uniq = got.clone();
       uniq.sort();
       uniq.dedup();
       assert_eq!(uniq.len(), got.len(), "联想候选不得重复：{got:?}");

       // Spec §9.3：候选里至少有一个当前 assoc 表里的已知项（随表动态取，不硬编码）。
       let known = known_suffixes("今天");
       assert!(!known.is_empty(), "luna.assoc.tsv 里没有「今天」根：检查生成参数（roots 太小或短语表换了）");
       assert!(
           got.iter().any(|s| known.iter().any(|k| s == k)),
           "联想未包含 assoc 表里的已知后缀：known={known:?} got={got:?}"
       );
   }

   #[test]
   fn trad_has_extras() {
       let bytes = std::fs::read(TRAD).unwrap();
       let v = engine_data::validate(&bytes).unwrap();
       assert_eq!(v.flags, 0b11);
       assert!(v.assoc.expect("trad assoc").root_count > 0);
       let dict = engine_data::load_bytes(bytes).unwrap();
       assert!(
           dict.words_with_prefix("中國").len() > 1,
           "trad 词索引应能按「中國」取词（trad 用繁体字形）"
       );
       let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), false);
       type_str(&mut e, "jintian");
       let idx = e.candidates(512).iter().position(|c| c.text == "今天").expect("候选缺 今天");
       assert_eq!(e.select(idx), "今天");
       assert!(e.association_active());
       assert!(!e.candidates(8).is_empty(), "trad 空缓冲联想非空");
   }
   ```
   说明：**不要**把「今天→回家」写死 —— 短语表里没有「今天回家」，luna 词库里也没有这个词；语义表内容会随脚本参数漂。`known_suffixes("今天")` 是 Spec §9.3 要求的「当前 assoc 表里的已知项」；若某次重生成让「今天」的全部已知后缀都因前缀关系被跳过而红，就从同一 TSV 换一个根（同样动态取 `known_suffixes(root)`），并在测试注释记下原因，**不要删断言**。trad 的词条是**繁体字形**（`中國` 而不是 `中国`），前缀断言别用简体。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p opi-tools --test association_real`
Expected: FAIL（资产还没有 flags/assoc）。

- [ ] **Step 3: 重建资产**

```bash
python3 scripts/merge_luna_kangxi.py --luna android/app/src/main/assets/luna.opid --extra /nonexistent > /tmp/luna_before.tsv
python3 scripts/merge_luna_kangxi.py --luna data/generated/trad.opid --extra /nonexistent > /tmp/trad_before.tsv
python3 scripts/gen_assoc_table.py --opid android/app/src/main/assets/luna.opid --phrases data/raw/extra_phrases.tsv --out data/generated/luna.assoc.tsv --roots 5000 --cap 3
python3 scripts/gen_assoc_table.py --opid data/generated/trad.opid --phrases data/raw/trad_phrases.tsv --out data/generated/trad.assoc.tsv --roots 5000 --cap 3
cargo run --release -p opi-tools --bin opi-tools -- compile --word-index --assoc data/generated/luna.assoc.tsv /tmp/luna_before.tsv data/generated/luna.opid
cargo run --release -p opi-tools --bin opi-tools -- compile --word-index --assoc data/generated/trad.assoc.tsv /tmp/trad_before.tsv data/generated/trad.opid
cp data/generated/luna.opid android/app/src/main/assets/luna.opid
cp data/generated/trad.opid android/app/src/main/assets/trad.opid
scripts/prepare_harmony_dicts.sh
```

条目不变证明：
```bash
python3 scripts/merge_luna_kangxi.py --luna android/app/src/main/assets/luna.opid --extra /nonexistent > /tmp/luna_after.tsv
python3 scripts/merge_luna_kangxi.py --luna data/generated/trad.opid --extra /nonexistent > /tmp/trad_after.tsv
diff /tmp/luna_before.tsv /tmp/luna_after.tsv
diff /tmp/trad_before.tsv /tmp/trad_after.tsv
```
Expected: 两个 `diff` 无输出（只加了尾部）。

- [ ] **Step 4: 运行测试与旧读者门禁**

Run:
```bash
cargo test -p opi-tools --test association_real --test dict_crosslayer --test phrase_quality
cargo run -q -p opi-tools --bin opi-tools -- verify android/app/src/main/assets/luna.opid
cargo run -q -p opi-tools --bin opi-tools -- verify android/app/src/main/assets/trad.opid
git rev-parse -q --verify v1.26.2 || git fetch --tags
git worktree add /tmp/opi-old v1.26.2
(cd /tmp/opi-old && cargo run -q -p opi-tools --bin opi-tools -- verify /home/wwwroot/bag/opi/android/app/src/main/assets/luna.opid)
(cd /tmp/opi-old && cargo run -q -p opi-tools --bin opi-tools -- verify /home/wwwroot/bag/opi/android/app/src/main/assets/trad.opid)
git worktree remove /tmp/opi-old
```
Expected: 全部通过（旧二进制 verify 新库 = 旧读者兼容）。

- [ ] **Step 5: 提交**

```bash
git add data/generated/luna.assoc.tsv data/generated/trad.assoc.tsv data/generated/trad.opid \
  android/app/src/main/assets/luna.opid android/app/src/main/assets/trad.opid \
  crates/opi-tools/tests/dict_crosslayer.rs crates/opi-tools/tests/phrase_quality.rs crates/opi-tools/tests/association_real.rs
git commit -m "data: luna/trad 词索引与语义小表，旧读者兼容验证"
```

---

### Task 12: Android 按键路由、候选栏与 ⇧

**Files:**
- Create: `android/app/src/main/kotlin/xyz/erik/opi/engine/EngineControllerAssociation.kt`
- Modify: `android/app/src/main/kotlin/xyz/erik/opi/ime/KeyRouter.kt`
- Modify: `android/app/src/main/kotlin/xyz/erik/opi/engine/EngineController.kt`
- Verify only (do not modify): `android/app/src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt`（文件恰 500 行；已核所有直提点都经 `router::commitText` / `router.handleKey`，本任务只确认，不改）
- Modify: `android/app/src/main/kotlin/xyz/erik/opi/candidate/CandidateBar.kt`
- Modify: `android/app/src/test/kotlin/xyz/erik/opi/engine/FakeEngine.kt`（`setShift` 在空缓冲时清候选，镜像 `set_shift` 清联想）
- Test: `android/app/src/test/kotlin/xyz/erik/opi/ime/KeyRouterAssociationTest.kt`、`android/app/src/test/kotlin/xyz/erik/opi/engine/EngineControllerAssociationTest.kt`

**Interfaces:**
- Consumes: 引擎 `clear()`；`EngineMode`。
- Produces:
  ```kotlin
  val EngineController.associationActive: Boolean
      get() = buffer.isEmpty() && pendingText.isEmpty() && candidates.isNotEmpty() &&
              (mode == EngineMode.PINYIN || mode == EngineMode.TRADITIONAL)
  ```
  五个入口（`handleKey`/`handleSpace`/`handleBackspace`/`handleEnter`/`commitText`）先 `if (controller.associationActive) controller.clear()`；`handleCandidate*` 不 dismiss。

- [ ] **Step 1: 写失败测试**

`EngineControllerAssociationTest`（controller 的字段是 `private set`，只能经 `refresh()` 从 fake 灌入）：
```kotlin
private fun associationController(): Pair<FakeEngine, EngineController> {
    val fake = FakeEngine()
    val ctrl = EngineController(fake)
    fake.buf = ""; fake.pending = ""; fake.cands = arrayOf("回家"); fake.mode = EngineMode.PINYIN.value
    ctrl.refresh()
    return fake to ctrl
}

@Test fun activeRequiresEmptyBufferPendingCandidatesAndChineseMode() {
    val (fake, ctrl) = associationController()
    assertTrue(ctrl.associationActive)
    fake.buf = "ni"; ctrl.refresh(); assertFalse(ctrl.associationActive)
    // 再各起一条：pending 非空 / cands 空 / mode = ENGLISH
}

@Test fun shiftTapRefreshesAfterSetShift() {
    val (fake, ctrl) = associationController()
    ctrl.shiftTap() // fake.setShift 在空缓冲时清 cands，镜像真引擎的 set_shift 清联想
    assertFalse(ctrl.associationActive)
}
```
`KeyRouterAssociationTest`：
```kotlin
@Test fun spaceDismissesThenCommitsSpace() {
    val (fake, ctrl) = associationController()
    val committed = mutableListOf<String>()
    val r = KeyRouter(ctrl, { committed += it }, {}, {})
    r.handleSpace()
    assertEquals(1, fake.clearCalls)
    assertEquals(listOf(" "), committed)
}
@Test fun backspaceDismissesThenDeletes() {
    val (fake, ctrl) = associationController()
    var deletes = 0
    val r = KeyRouter(ctrl, {}, { deletes++ }, {})
    r.handleBackspace()
    assertEquals(1, fake.clearCalls)
    assertEquals(1, deletes)
}
@Test fun enterDismissesThenPerformsEnter() {
    val (fake, ctrl) = associationController()
    var enters = 0
    val r = KeyRouter(ctrl, {}, {}, { enters++ })
    r.handleEnter()
    assertEquals(1, fake.clearCalls)
    assertEquals(1, enters)
}
@Test fun digitInputDismissesAndDoesNotSelectAssociation() {
    val (fake, ctrl) = associationController()
    val r = KeyRouter(ctrl, {}, {}, {})
    r.handleKey("2")
    assertEquals(1, fake.clearCalls, "数字键必须先收联想")
    assertEquals(listOf("2"), fake.inputCalls, "数字必须落回输入路由，不得当联想选择键")
}
@Test fun candidateTapDoesNotDismiss() {
    val (fake, ctrl) = associationController()
    fake.selectResults[0] = "回家"
    val r = KeyRouter(ctrl, {}, {}, {})
    r.handleCandidate(0)
    assertEquals(0, fake.clearCalls)
}
```
`FakeEngine.setShift` 补：`if (buf.isEmpty()) cands = null`（真引擎 `set_shift` 清联想；空缓冲时候选只来自联想）。

- [ ] **Step 2: 运行测试确认失败**

Run: `(cd android && ./gradlew :app:testDebugUnitTest --tests 'xyz.erik.opi.ime.KeyRouterAssociationTest' --tests 'xyz.erik.opi.engine.EngineControllerAssociationTest')`
Expected: 编译失败（`associationActive` 不存在）。（仓库根没有 `gradlew`，gradle 命令都要 `cd android` 或 `android/gradlew -p android`。）

- [ ] **Step 3: 实现**

1. 新文件放 `associationActive` 扩展属性。
2. `KeyRouter` 五个入口第一行加 dismiss；**不**改 `handleCandidate*`。
3. `EngineController.shiftTap`/`shiftLongPress` 在 `api.setShift` 后补 `refresh()`。
4. `ImeScreen.kt` **只核对不改**（已核 line 240/246/315 的 `onCommit = router::commitText`、line 364/379 数字 `router.commitText("1")`、line 406/407/423/464 的 `onCommitText`/`onAlternative` 都经 `KeyRouter`）。若实际树已漂移到有绕过者，先报告再动（文件恰 500 行，改一行就破门禁）。
5. `CandidateBar.kt` 外层 `Row` 加 `.clickable(enabled = controller.associationActive) { controller.clear() }`（子候选的 `combinedClickable`/`IconButton` 会消费点击，`horizontalScroll` 只吃拖动）；该文件包名是 `xyz.erik.opi.candidate`，要 `import xyz.erik.opi.engine.associationActive`。模拟器实测只作为验证步骤，不再决定写法。
6. 行数超过 500 的先把相关方法抽到新 helper 文件，不压注释。

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
(cd android && ./gradlew :app:testDebugUnitTest --tests 'xyz.erik.opi.ime.*Association*' --tests 'xyz.erik.opi.engine.EngineControllerAssociationTest')
(cd android && ./gradlew :app:assembleRelease)
```
Expected: 单测全绿，release APK 构建成功（debug 的 Rust 是 debug 编译，不作性能判据）。

- [ ] **Step 5: 提交**

```bash
git add android/app/src/main/kotlin/xyz/erik/opi/engine/EngineControllerAssociation.kt \
  android/app/src/main/kotlin/xyz/erik/opi/engine/EngineController.kt \
  android/app/src/main/kotlin/xyz/erik/opi/ime/KeyRouter.kt \
  android/app/src/main/kotlin/xyz/erik/opi/candidate/CandidateBar.kt \
  android/app/src/test/kotlin/xyz/erik/opi/ime/KeyRouterAssociationTest.kt \
  android/app/src/test/kotlin/xyz/erik/opi/engine/EngineControllerAssociationTest.kt \
  android/app/src/test/kotlin/xyz/erik/opi/engine/FakeEngine.kt
git commit -m "feat(android): 联想态按键先收起再正常执行"
```

---

### Task 13: Android 选区变化与自我提交守卫

**Files:**
- Create: `android/app/src/main/kotlin/xyz/erik/opi/ime/ImeCommitGuard.kt`
- Modify: `android/app/src/main/kotlin/xyz/erik/opi/OpiImeService.kt`（**恰 500 行**：删掉私有 `selectionNonEmpty` 字段及其注释、`onUpdateSelection` 收成一行、`deleteBackward` 改读 guard，净行数 ≈ 0）
- Modify: `android/app/src/main/kotlin/xyz/erik/opi/ComposingText.kt`
- Test: `android/app/src/test/kotlin/xyz/erik/opi/ime/ImeCommitGuardTest.kt`

**Interfaces:**
- Consumes: Task 12 `associationActive`；`EngineController.clear()`。
- Produces:
  ```kotlin
  internal class ImeCommitGuard(
      private val windowMs: Long = 200,
      private val now: () -> Long = { SystemClock.uptimeMillis() },
  ) {
      /** 从 OpiImeService 搬来（那边撞 500 行；语义仍是 service 的一部分）。 */
      internal var selectionNonEmpty = false
      fun noteSelfCommit()
      fun inSelfCommitWindow(): Boolean
  }

  internal fun ImeCommitGuard.onSelectionChanged(
      ctrl: EngineController, newSelStart: Int, newSelEnd: Int,
  ) {
      selectionNonEmpty = newSelStart >= 0 && newSelEnd >= 0 && newSelStart != newSelEnd
      if (!inSelfCommitWindow() && ctrl.associationActive) ctrl.clear()
  }
  ```
  `OpiImeService` 持有 `internal val selfCommitGuard = ImeCommitGuard()`（必须 `internal`，`ComposingText.kt` 的扩展要读）。`dropComposing` 在 `if (lastComposing.isEmpty()) return` **之前**调 `selfCommitGuard.noteSelfCommit()`（正常路径 `lastComposing` 恒空，所以位置是硬要求）；`onUpdateSelection` 只留 `super(...)` + `selfCommitGuard.onSelectionChanged(engineController, newSelStart, newSelEnd)`；`deleteBackward` 改读 `selfCommitGuard.selectionNonEmpty`。

- [ ] **Step 1: 写失败测试**

```kotlin
private fun activeController(): Pair<FakeEngine, EngineController> {
    val fake = FakeEngine()
    val ctrl = EngineController(fake)
    fake.buf = ""; fake.pending = ""; fake.cands = arrayOf("回家"); fake.mode = EngineMode.PINYIN.value
    ctrl.refresh()
    return fake to ctrl
}

@Test fun windowIsInclusiveAndExpires() {
    var t = 1000L
    val g = ImeCommitGuard(windowMs = 200, now = { t })
    assertFalse(g.inSelfCommitWindow())
    g.noteSelfCommit(); assertTrue(g.inSelfCommitWindow())
    t += 200; assertTrue(g.inSelfCommitWindow())
    t += 1; assertFalse(g.inSelfCommitWindow())
}

@Test fun consecutiveSelfCommitsExtendWindow() {
    var t = 1000L
    val g = ImeCommitGuard(windowMs = 200, now = { t })
    g.noteSelfCommit()
    t += 150
    g.noteSelfCommit() // 第一次窗口到 1200；第二次应续到 1350
    t += 150
    assertTrue(g.inSelfCommitWindow())
    t += 51
    assertFalse(g.inSelfCommitWindow())
}

@Test fun selectionChangeOutsideWindowDismissesAssociation() {
    var t = 1000L
    val g = ImeCommitGuard(windowMs = 200, now = { t })
    val (fake, ctrl) = activeController()
    g.onSelectionChanged(ctrl, 1, 2)
    assertEquals(1, fake.clearCalls)
}

@Test fun selectionChangeInsideSelfCommitWindowKeepsAssociation() {
    var t = 1000L
    val g = ImeCommitGuard(windowMs = 200, now = { t })
    val (fake, ctrl) = activeController()
    g.noteSelfCommit()
    g.onSelectionChanged(ctrl, 1, 2)
    assertEquals(0, fake.clearCalls, "自我提交触发的光标变化不得把刚建好的联想收掉")
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `(cd android && ./gradlew :app:testDebugUnitTest --tests 'xyz.erik.opi.ime.ImeCommitGuardTest')`
Expected: 编译失败（类不存在）。

- [ ] **Step 3: 实现**

1. 新 `ImeCommitGuard` + `onSelectionChanged` 扩展（同文件）；文件在 `xyz.erik.opi.ime`，需 `import xyz.erik.opi.engine.EngineController` 与 `import xyz.erik.opi.engine.associationActive`。
2. `OpiImeService` 持有 `internal val selfCommitGuard = ImeCommitGuard()`；`dropComposing` 第一行 `selfCommitGuard.noteSelfCommit()`。
3. 搬 `selectionNonEmpty`：删 `OpiImeService` 的私有字段（line 51）与注释，`onUpdateSelection` 改一行 `selfCommitGuard.onSelectionChanged(engineController, newSelStart, newSelEnd)`，`deleteBackward`（line 318）改读 `selfCommitGuard.selectionNonEmpty`。
4. 逐一核对自我提交路径：`commitWithRetry`、`BracketCommit.insertPair`、`PetStickerCommit`、`performEnter` 的新行路径都经 `dropComposing`；有遗漏的直接插入点补 `noteSelfCommit()`。
5. 收尾跑 `wc -l` 确认 `OpiImeService.kt` ≤ 500（现已 500 整，搬入 guard 的部分必须等量删掉）。

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
(cd android && ./gradlew :app:testDebugUnitTest --tests 'xyz.erik.opi.ime.ImeCommitGuardTest')
(cd android && ./gradlew :app:assembleRelease)
```
Expected: 全绿。

- [ ] **Step 5: 提交**

```bash
git add android/app/src/main/kotlin/xyz/erik/opi/ime/ImeCommitGuard.kt \
  android/app/src/main/kotlin/xyz/erik/opi/OpiImeService.kt \
  android/app/src/main/kotlin/xyz/erik/opi/ComposingText.kt \
  android/app/src/test/kotlin/xyz/erik/opi/ime/ImeCommitGuardTest.kt
git commit -m "feat(android): 选区变化收起联想，自我提交守卫"
```

---

### Task 14: iOS 接线

**Files:**
- Create: `ios/KeyboardAssociation.swift`
- Modify: `ios/KeyboardViewController.swift`
- Modify: `ios/KeyboardCandidates.swift`
- Modify: `ios/KeyboardCandidateBar.swift`

**Interfaces:**
- Consumes: `OpiEngine.clear()`、`refresh()`、`engine.buffer()`/`displayBuffer()`/`mode()`/`allCandidates(limit:)`、`layout.candidateTexts`。
- Produces:
  - `associationActive` 推断：`engine.buffer().isEmpty && engine.displayBuffer().isEmpty && !layout.candidateTexts.isEmpty && (mode == .pinyin || mode == .traditional)`；
  - `dismissAssociation()` = `engine.clear() + refresh()`；
  - `route` / `routeHardware` 首键、`didTapFunction` 非候选分支先 dismiss；
  - `selectionDidChange` 与自我插入守卫；
  - 候选栏父视图 tap gesture 空白点击 dismiss。

- [ ] **Step 1: 写验收检查表（无 Swift 单测 harness）**

在任务提交说明里列必须人工/编译验证的点：
1. 提交「今天」后候选行出现续接项，且不产生 marked text。
2. 点续接项只上屏续接部分，候选行继续。
3. 空格/退格/回车/数字先收联想再正常执行。
4. 光标移动/选区变化后联想收起。
5. 候选栏空白点击收起，键盘不收起。
6. 数字层、分词键、面板直提入口先 dismiss。

- [ ] **Step 2: 先编译现状确认基线**

仓库**没有** `.xcodeproj`：Swift 的编译入口是 `scripts/apple/package-ios.sh`（CI `apple.yml` 的 ios job 同款）。本机 Linux 跑不了 Swift，二选一：
- 有 Mac：`cargo build --release -p opi_ffi --target aarch64-apple-ios`、`... --target aarch64-apple-ios-sim`，再 `bash scripts/apple/package-ios.sh "$(scripts/apple/version.sh)" /tmp/opi-ios-baseline`；
- 没有 Mac：跳过本步，编译交给 `apple.yml`（手动 dispatch；iOS 路径 push 也会触发），并在提交说明写「本机未编译」。

Expected: 基线能编译（若基线红，先记录再动）。

- [ ] **Step 3: 实现**

1. 新 `KeyboardAssociation.swift`：dismiss、`associationActive`、`selectionDidChange`、父视图 tap、自我插入时间窗（`ProcessInfo.processInfo.systemUptime`，150–250ms）。
2. `KeyboardViewController.swift` 只留调用；把 `applyFallback` 9 行搬进新文件腾行。
3. `route` 顶部与 `routeHardware` 首键先 dismiss；`didTapFunction` 的 `.commitText`、`.segment` 空缓冲、`.retype`、`.showNumbers`、面板直提先 dismiss；`selectCandidate` 不 dismiss。
4. `refresh()` 已能显示联想，不改可见性；`setTopBar` 判据保持。
5. 在 `insert`、`insertTyped`、`deleteBackwardTyped`、数字层直提、面板直提升始记时间戳；`selectionDidChange` 窗口外联想态 dismiss。

- [ ] **Step 4: 编译验证**

Run: 同 Step 2（Mac 上跑 `package-ios.sh`；没有 Mac 就等 `apple.yml` 的 ios job）。
Expected: 编译通过；人工验收清单逐项记录（模拟器/真机）；未跑的项如实列出。

- [ ] **Step 5: 提交**

```bash
git add ios/KeyboardAssociation.swift ios/KeyboardViewController.swift ios/KeyboardCandidates.swift ios/KeyboardCandidateBar.swift
git commit -m "feat(ios): 联想收起、选区守卫与空白点击"
```

---

### Task 15: HarmonyOS 接线

**Files:**
- Create: `harmony/entry/src/main/ets/InputMethodExtensionAbility/AssociationGuard.ets`
- Modify: `harmony/entry/src/main/ets/InputMethodExtensionAbility/KeyboardController.ets`
- Modify: `harmony/entry/src/main/ets/InputMethodExtensionAbility/StatePublish.ets`
- Modify: `harmony/entry/src/main/ets/InputMethodExtensionAbility/KeyboardAppStorage.ets`
- Modify: `harmony/entry/src/main/ets/InputMethodExtensionAbility/TopBar.ets`

**Interfaces:**
- Consumes: `OpiEngine.clear()`、`publish()`、`buffer()/displayBuffer()/candidatesPage()/mode()`。
- Produces:
  - `KEY_ASSOCIATION` 缓存（`publish()` 时写）；
  - `AssociationGuard.ets`：dismiss、`selectionChange` 注册/退订、自我插入时间窗；
  - `pressKey`/`commitText`/`openSymbolPage`/`openRare`/`selectInputMode`/`togglePanel`/`tapChar`/`tapSegment` 先 dismiss；
  - **`tapTiledCandidate` 不 dismiss**（这是候选点击，交给引擎链式；设计稿 §5.3 把它列入绕过入口与 §2.3/§5.1 矛盾，按链式正确性处理并在提交说明注明）。

- [ ] **Step 1: 写验收检查表**

同 iOS 六项；另加：`tapTiledCandidate` 点续接项必须链式，不得先 clear（否则点选变空提交）。

- [ ] **Step 2: 基线构建**

Run: `(cd harmony && "$HARMONY_CLI_HOME/bin/hvigorw" clean --no-daemon && "$HARMONY_CLI_HOME/bin/hvigorw" assembleHap -p buildMode=release --no-daemon)`（仓库内没有 `hvigorw`，来自 Harmony CLI；CI 用自托管 `[self-hosted, linux, harmony]` runner）。
Expected: 基线能构建；本机没有 Harmony CLI 时如实记录「未在本机构建」，交给 `harmony.yml`/真机。

- [ ] **Step 3: 实现**

1. `KeyboardAppStorage.ets` 加 `KEY_ASSOCIATION`。
2. `publish()` 后缓存 `associationActive = buffer()=='' && displayBuffer()=='' && candidatesPage() 非空 && mode 拼音/繁体`；避免每键重新取候选。
3. `AssociationGuard.ets` 放 dismiss（`OpiEngine.clear()+publish()`）、`selectionChange` 注册（`onInputStart`）/退订（`onInputStop`/`release`）、自我插入时间窗。
4. 各入口按 Interfaces 加 dismiss；`tapTiledCandidate` 不加。
5. `TopBar.CandidateRow` 外层 `Row` 加 `onClick`：仅联想态 `clear()+publish()`；子候选 `Text.onClick` 消费，空白冒泡需真机核。

- [ ] **Step 4: 构建验证**

Run: 同 Step 2（本机没有 Harmony CLI 就等 `harmony.yml`）；真机/模拟器逐项验收并记录。
Expected: 构建通过；验收清单落实；未跑的项如实列出。

- [ ] **Step 5: 提交**

```bash
git add harmony/entry/src/main/ets/InputMethodExtensionAbility/AssociationGuard.ets \
  harmony/entry/src/main/ets/InputMethodExtensionAbility/KeyboardController.ets \
  harmony/entry/src/main/ets/InputMethodExtensionAbility/StatePublish.ets \
  harmony/entry/src/main/ets/InputMethodExtensionAbility/KeyboardAppStorage.ets \
  harmony/entry/src/main/ets/InputMethodExtensionAbility/TopBar.ets
git commit -m "feat(harmony): 联想收起、选区守卫与空白点击"
```

---

### Task 16: macOS 接线

**Files:**
- Create: `macos/InputControllerAssociation.swift`
- Modify: `macos/InputController.swift`

**Interfaces:**
- Consumes: `OpiEngine.clear()`、`refresh`、`engine.candidatesPage()`、`engine.buffer()`、`engine.displayBuffer()`、`engine.mode()`。
- Produces:
  - `associationActive` 推断：`engine.buffer().isEmpty && engine.displayBuffer().isEmpty && !engine.candidatesPage().isEmpty && (engine.mode() == .pinyin || engine.mode() == .traditional)`；`mouseDown` 用它（不直接看 `lastCandidates`，避免把旧缓存当联想态）；
  - `handle` 最前面（热键之前）dismiss；`virtualKey` 最前面 dismiss；`commitFromOutside` dismiss + refresh；
  - `refresh()` 可见性 = `!buffer.isEmpty || !list.isEmpty`，可见性变化也 `OpiCandidateWindow.update`；
  - `candidateSelected` 提交后 refresh（不再强制清空/隐藏）；
  - `IMKMouseHandling.mouseDown` 文档点击：联想态 dismiss 并 `return false`。

- [ ] **Step 1: 写验收检查表**

1. 提交后候选窗显示续接项（buffer 空时窗口不隐藏）。
2. 点续接项后窗口继续显示链式。
3. 任意键/热键/方向键先收联想。
4. 文档鼠标点击收联想。
5. 候选窗空白点击不做（已知边界，不报缺陷）。

- [ ] **Step 2: 基线编译**

仓库**没有** `.xcodeproj`：入口是 `scripts/apple/package-macos.sh`（CI `apple.yml` 的 macos job 同款）。有 Mac 时先 `cargo build --release -p opi_ffi --target aarch64-apple-darwin` 与 `x86_64-apple-darwin`，再 `bash scripts/apple/package-macos.sh "$(scripts/apple/version.sh)" /tmp/opi-macos-baseline`；没有 Mac 就交给 `apple.yml`，并在提交说明写「本机未编译」。
Expected: 基线结果记录（若基线红先记录再动）。

- [ ] **Step 3: 实现**

1. 新 `InputControllerAssociation.swift`：`associationActive` 推断（buffer 空 + displayBuffer 空 + candidatesPage 非空 + mode 拼音/繁体）、dismiss、`mouseDown`、refresh 的可见性逻辑；把 `refresh` 整段搬入（controller 只留调用）。
2. `lastMarked`/`lastCandidates` 提升为 internal，加 `lastCandidatesVisible`。
3. `refresh`：
   ```swift
   let visible = !buffer.isEmpty || !list.isEmpty
   if list != lastCandidates || visible != lastCandidatesVisible {
       lastCandidates = list
       lastCandidatesVisible = visible
       OpiCandidateWindow.shared.update(list, visible: visible)
   }
   ```
4. `candidateSelected` 去掉 `lastCandidates = []` 与隐藏，commit 后 `refresh`。
5. `handle` 顶部、`virtualKey` 顶部、`commitFromOutside` 加 dismiss。
6. `IMKMouseHandling` 协议方法签名以 SDK 头为准；联想态（`associationActive`）`dismiss + return false`，非联想态照旧。

- [ ] **Step 4: 编译验证**

Run: 同 Step 2（Mac 上跑 `package-macos.sh`；没有 Mac 就等 `apple.yml`），结果记录到提交说明（未跑的路径如实列出）。
Expected: 编译通过。

- [ ] **Step 5: 提交**

```bash
git add macos/InputControllerAssociation.swift macos/InputController.swift
git commit -m "feat(macos): 联想窗口可见性、点选链式与鼠标收起"
```

---

### Task 17: fcitx5 接线（Rust + C++）

**Files:**
- Create: `crates/fcitx5-opi/src/candidate_association.rs`
- Modify: `crates/fcitx5-opi/src/candidate.rs`
- Modify: `crates/fcitx5-opi/src/input_method.rs`
- Modify: `crates/fcitx5-opi/cpp/opi_fcitx5.cpp`
- Modify: `crates/fcitx5-opi/cpp/opi_panel_driver.cpp`
- Test: `crates/fcitx5-opi/src/input_method_association_tests.rs`（`#[path]` 引入）

**Interfaces:**
- Consumes: `Engine::dismiss_association()`、`association_active()`、`engine.candidates()`；`invalidate_fetch_cache()`（提升 `pub(crate)`）。
- Produces:
  - `CandidateState::dismiss_association() -> bool`：清引擎联想并作废 `fetch_cache`；
  - `handle_key` 在 Ctrl/Alt 判定**之前**取 `released`，keydown 先 dismiss；
  - C++ `g_pushedHadCandidates`、`associationShown` 早清、联想态不设 selection key；
  - 直通跳过条件加 `&& !associationShown`。

- [ ] **Step 1: 写失败测试**

`association_state()`：用 `engine_data::serialize_with_extras` 造内存词库（`今天`/`今天晚上` + assoc `今天→回家`），`load_bytes` 装载，`switch_mode(Mode::Pinyin)`，喂 `jintian` 再 `engine.select(idx)` 进联想态。

```rust
#[test]
fn keydown_dismisses_association_before_dispatch() {
    let mut state = association_state();
    assert!(state.engine.association_active());
    assert_eq!(handle_key(&mut state, KEY_SPACE, 0), KeyAction::Input(" ".into()));
    assert!(!state.engine.association_active());
}

#[test]
fn release_does_not_dismiss() {
    let mut state = association_state();
    let _ = handle_key(&mut state, KEY_SPACE, KEY_STATE_RELEASED);
    assert!(state.engine.association_active());
}

#[test]
fn ctrl_combo_dismisses_but_passes_through() {
    let mut state = association_state();
    assert_eq!(handle_key(&mut state, 'a' as u32, KEY_STATE_CTRL), KeyAction::PassThrough);
    assert!(!state.engine.association_active());
}

#[test]
fn digit_in_association_does_not_select_candidate() {
    let mut state = association_state();
    let action = handle_key(&mut state, '1' as u32, 0);
    // 空缓冲数字落 PassThrough（不进 select），且联想已清
    assert_eq!(action, KeyAction::PassThrough);
    assert!(!state.engine.association_active());
}
```

若现有测试无法直接构造「有联想」的 `CandidateState`，用可用的测试构造器或 `CandidateState::new` + 测试词典；不要为测试新增生产 FFI。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p fcitx5-opi association`
Expected: 编译失败（`dismiss_association` 不存在）。

- [ ] **Step 3: 实现**

Rust：
1. `candidate.rs` 的 `invalidate_fetch_cache` **已是** `pub(crate)`（line 151），确认即可，不改可见性。
2. 新 `candidate_association.rs`（子模块不继承父模块的 `use`，必须自带导入）：
   ```rust
   use super::CandidateState;

   impl CandidateState {
       pub(crate) fn dismiss_association(&mut self) -> bool {
           self.invalidate_fetch_cache();
           self.engine.dismiss_association()
       }
   }
   ```
   在 `candidate.rs` 末尾挂 `#[path = "candidate_association.rs"] mod association;`。**必须用 `#[path]`**：普通 `mod candidate_association;` 会去 `src/candidate/` 找文件；当前 `candidate.rs` 491 行，加 2 行仍 < 500。
3. `input_method.rs` `handle_key` 顶部：先 `let released = ...;`，再 `if !released { state.dismiss_association(); }`，然后保留 Ctrl/Alt 直通与其余逻辑。不吞键。末尾挂 `#[cfg(test)] #[path = "input_method_association_tests.rs"] mod association_tests;`。

C++：
1. 顶部 `static bool g_pushedHadCandidates = false;`。
2. `refreshingPanelFor`：**在分支前**统一取一次
   `const auto texts = parseJsonStringArray(take(opi_fcitx5_candidates(kOpiCandidateFetchLimit)));`
   - `buffer.empty() && texts.empty()` → `panel.reset()`；
   - `buffer.empty() && !texts.empty()`（联想）→ 先 `panel.reset()`，再**只**建候选表：
     ```cpp
     auto list = std::make_unique<fcitx::CommonCandidateList>();
     list->setPageSize(static_cast<int>(texts.size()));
     for (size_t i = 0; i < texts.size(); ++i) list->append<OpiCandidateWord>(texts[i], i);
     panel.setCandidateList(std::move(list));
     ```
     **不设** preedit / clientPreedit / `setSelectionKey` / `setGlobalCursorIndex`（数字键必须落回 Rust 路由，Spec §5）；
   - `!buffer.empty()` → 现有逻辑原样（preedit + 候选表 + selection key + global cursor），**但 `texts` 改用上面那次统一取的值**，不要重复调 `opi_fcitx5_candidates`（重复取会把 Rust 的页游标推进两次）；
   - **三种分支之后保留现有尾部原样**：`if (wantsClientPreedit) { ic->updatePreedit(); }`、`ic->updateUserInterface(...)`，再接 `g_pushedBuffer = buffer; g_pushedHadCandidates = !texts.empty();`。联想分支 `panel.reset()` 清的是面板；声明了 Preedit 的客户端内联预编辑仍要靠 `updatePreedit()` 这条通道清（Spec §5.5），漏掉会让联想态下旧拼音挂光标处。
3. `keyEventImpl` 最前（`shiftToggle_.handle` 之前）：
   ```cpp
   const bool associationShown = !keyEvent.isRelease()
       && g_pushedBuffer.empty() && g_pushedHadCandidates;
   if (associationShown) {
       opi_fcitx5_clear();
       refreshingPanelFor(keyEvent.inputContext());
   }
   ```
4. 直通跳过加 `&& !associationShown`。
5. `opi_panel_driver.cpp` 第 4 节点选断言改为「点选后链式候选仍在/面板未清空」。该文件已 494 行，改完必须 ≤ 500（只改断言，不搬大段）。
6. **已接受的代价**：`opi_fcitx5_clear()` 是 void 且走 `CandidateState::clear()`，会顺带清 `last_commit`（学习撤销锚点）。设计稿 §5.5 已记录：Rust 宿主只有这一条公开入口，不为此扩 FFI；影响仅限「联想收起后立刻退票」这一个动作。

- [ ] **Step 4: 运行测试与 C++ 编译**

Run:
```bash
cargo test -p fcitx5-opi
bash crates/fcitx5-opi/cpp/run-harness.sh
```
Expected: Rust 测试全绿；手工 harness 编译通过、第 4 节断言按新语义通过。旧头（5.0.x）语法检查按仓库既有方式另跑或记录未跑。

- [ ] **Step 5: 提交**

```bash
git add crates/fcitx5-opi/src/candidate_association.rs crates/fcitx5-opi/src/candidate.rs \
  crates/fcitx5-opi/src/input_method.rs crates/fcitx5-opi/src/input_method_association_tests.rs \
  crates/fcitx5-opi/cpp/opi_fcitx5.cpp crates/fcitx5-opi/cpp/opi_panel_driver.cpp
git commit -m "feat(fcitx5): 联想收起与链式面板"
```

---

### Task 18: TSF 接线

**Files:**
- Create: `crates/tsf-opi/src/logic_association.rs`
- Create: `crates/tsf-opi/src/tsf_association.rs`
- Modify: `crates/tsf-opi/src/tsf.rs`
- Modify: `crates/tsf-opi/src/key_sink.rs`
- Modify: `crates/tsf-opi/src/logic_snapshot.rs`（挂载 `logic_association.rs` 与其测试；**不改 `logic.rs`** —— 它已 498 行，加 `#[path]` 模块会破 500）
- Modify: `crates/tsf-opi/src/candidate_sink.rs`
- Test: `crates/tsf-opi/src/logic_association_tests.rs`（`#[path]` 挂在 `logic_snapshot.rs`）+ `candidate_sink.rs` 内联测试

**Interfaces:**
- Consumes: `Engine::dismiss_association()` / `association_active()`；`TsfLogic::invalidate_fetch_cache()`（已 `pub(crate)`）；`push_state`。
- Produces:
  - `StateSnapshot.association: bool`；caret 条件 `composing || association`；
  - `CandidateSink::on_composition_changed`：`buffer.is_empty() && candidates.is_empty()` 才 hide，否则 show；
  - `OnTestKeyDown` 顺序：`drain_pending()` → keydown dismiss + snapshot → 锁外 `push_state` → 既有 shift/热键/Ctrl/Alt/Win/claim 判定；
  - `handle_key_inner` 的 Commit 分支提交后若 `composing || association` 补 caret + push；
  - `key_sink.rs` 是 `tsf.rs` 的**子模块**（`#[path] mod key_sink;` 在 `tsf.rs` 末尾），能直接调父模块的私有 `drain_pending`；只需补 `use crate::tsf::push_state;`（`tsf.rs` 已 `pub(crate) use` 它）。

- [ ] **Step 1: 写失败测试**

`logic_association_tests.rs`（镜像 fcitx5 命名）。`association_logic()` 照 fcitx5 的 `association_state()`：`serialize_with_extras` 造内存词库 → `load_bytes` → **`TsfLogic::with_dict(Box::new(dict))`**（别手写结构体字面量，字段会漂）→ `switch_mode(Mode::Pinyin)` → 喂 `jintian` 选「今天」。测试挂成 `logic::snapshot` 的子模块（`logic_snapshot.rs` 里的 `#[path]`），`TsfLogic` 字段是 `pub(crate)`，构造无碍。

```rust
#[test]
fn snapshot_association_flag_and_visibility() {
    let logic = association_logic();
    let s = logic.snapshot();
    assert!(s.association);
    assert!(s.buffer.is_empty());
    assert!(!s.candidates.is_empty());
}
```
在 `logic_association_tests.rs` 里补：`dismiss_association` 清联想并作废 fetch cache；`select_from` 点联想项后 `association_active()` 为真且提交的是续接项（不往 `logic.rs` 加测试，行数不允许）。
`candidate_sink` 可见性测试：buffer 空 + candidates 非空 → 不 hide；都空 → hide。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p tsf-opi`
Expected: 编译失败（字段/方法不存在）。Windows 目标测试按 CI 方式跑；本机用 `cargo xwin build --release -p tsf_opi --target x86_64-pc-windows-msvc` 至少验证编译（按仓库既有做法）。

- [ ] **Step 3: 实现**

1. `logic_snapshot.rs`：`StateSnapshot` 加 `association`，`snapshot()` 填 `self.engine.association_active()`。
2. `candidate_sink.rs`：hide 判据改 `buffer.is_empty() && candidates.is_empty()`。
3. `logic_snapshot.rs` 末尾挂载（`logic.rs` 498 行，不能加）：
   ```rust
   #[path = "logic_association.rs"]
   mod association;
   #[cfg(test)]
   #[path = "logic_association_tests.rs"]
   mod association_tests;
   ```
   `logic_association.rs` 放（`logic::snapshot::association` 是更深一层的模块，`TsfLogic` 不在作用域内）：
   ```rust
   use crate::logic::TsfLogic;

   impl TsfLogic {
       pub(crate) fn dismiss_association(&mut self) -> bool {
           self.invalidate_fetch_cache();
           self.engine.dismiss_association()
       }
   }
   ```
4. `key_sink.rs` `OnTestKeyDown`：
   - 第一行 `self.drain_pending();`
   - 锁内 `let dismissed = logic.dismiss_association(); let snapshot = dismissed.then(|| logic.snapshot());`
   - 锁外 `if let Some(s) = snapshot { push_state(&*self.sink, &s); }`
   - 然后保留原有 `shift_tap_key_down`、热键、Ctrl/Alt、Win、`claim_on_test`。
   - 补 `use crate::tsf::push_state;`；`drain_pending` 不用改可见性（子模块可访问父模块私有项）。
5. `tsf.rs`：`handle_key_inner` 保留 `drain_pending()`；Commit 分支在 `insert_into` + `on_commit` 后，若 `snapshot.association || snapshot.composing` 则 `on_caret` + `push_state`（Commit 的 snapshot 必须在 `input_key` 后锁内抓）。
6. `tsf_association.rs` 放 dismiss+push / commit-push helper，`tsf.rs` 只留调用；在 `tsf.rs` 末尾加 `#[path = "tsf_association.rs"] mod association;`。helper 必须 `pub(crate)`（`key_sink` 与 `tsf` 是兄弟模块，私有项互相看不见），别写成无可见性。
7. 两轨保持逐行同构（配对 fcitx5 的 `candidate_association.rs`）。

- [ ] **Step 4: 运行测试与 Windows 编译**

Run:
```bash
cargo test -p tsf-opi
cargo xwin build --release -p tsf_opi --target x86_64-pc-windows-msvc
```
Expected: Rust 测试（可在 Linux 编译的部分）全绿；Windows DLL 链接成功。Windows 目标单测由 CI 跑，结果记录。

- [ ] **Step 5: 提交**

```bash
git add crates/tsf-opi/src/logic_association.rs crates/tsf-opi/src/tsf_association.rs \
  crates/tsf-opi/src/tsf.rs crates/tsf-opi/src/key_sink.rs \
  crates/tsf-opi/src/logic_snapshot.rs crates/tsf-opi/src/candidate_sink.rs \
  crates/tsf-opi/src/logic_association_tests.rs
git commit -m "feat(tsf): OnTestKeyDown 先收起联想，Commit 后补推快照"
```

---

### Task 19: 文档、性能记录与全量门禁

**Files:**
- Modify: `CONTRIBUTING.md`
- Create: `crates/opi-tools/tests/association_perf.rs`（`#[ignore]` 基准）
- Test: 全 workspace 门禁

**Interfaces:**
- Consumes: Task 9 CLI、Task 11 资产。
- Produces: 可复现的构建步骤、性能记录、发布前检查清单。

- [ ] **Step 1: 写性能基准（失败/无输出基线）**

`build_association` 是 `pub(crate)`（Spec §4.1），集成测试调不到；基准改测**公开路径**：`select`（内部含 `build_association`）+ `candidates(8)`（显示路径）。测到的是上界，Spec §9.6 只记录不设门禁。

`association_perf.rs`：
```rust
const LUNA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../android/app/src/main/assets/luna.opid");

#[test]
#[ignore = "真实 luna 33MB；cargo test --release -p opi-tools --test association_perf -- --ignored --nocapture"]
fn association_latency_on_real_luna() {
    let bytes = std::fs::read(LUNA).unwrap();
    let dict = engine_data::load_bytes(bytes).unwrap();
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), false);
    for (root, pinyin) in [("今天", "jintian"), ("什么", "shenme"), ("中国", "zhongguo")] {
        for ch in pinyin.chars() { let _ = e.input_key(ch); }
        let idx = e.candidates(512).iter().position(|c| c.text == root).unwrap();
        let t0 = std::time::Instant::now();
        let out = e.select(idx); // select 内部含 build_association
        let select_time = t0.elapsed();
        let t1 = std::time::Instant::now();
        let shown = e.candidates(8).len();
        println!("{root}: select={select_time:?} candidates={:?} shown={shown} out={out}", t1.elapsed());
    }
}
```
Run: `cargo test --release -p opi-tools --test association_perf -- --ignored --nocapture`
Expected: 输出三组实测值；记录到提交说明。目标 <5ms 是实测记录，不是硬门禁。

- [ ] **Step 2: 更新 CONTRIBUTING**

在现有 compile 配方后补：
```
# 词排序索引 + 语义小表（luna；trad 同理，短语表换 trad_phrases.tsv）
python3 scripts/gen_assoc_table.py --opid <基线.opid> \
    --phrases data/raw/extra_phrases.tsv \
    --out data/generated/luna.assoc.tsv --roots 5000 --cap 3
cargo run --release -p opi-tools --bin opi-tools -- compile \
    --word-index --assoc data/generated/luna.assoc.tsv \
    <luna.tsv> data/generated/luna.opid
```
并说明：`--assoc` 空文件 = 不开 bit1；`data/generated/{luna,trad}.assoc.tsv` 入库；`luna.opid` 不入库；旧二进制可读新库。

- [ ] **Step 3: 跑全量门禁**

Run:
```bash
cargo test --workspace --no-fail-fast
cargo clippy --all-targets --workspace -- -D warnings
cargo fmt --all --check
cargo test -p opi-ffi --test line_limit
test -z "$(grep -rn 'std::fs' crates/engine-core/src/ || true)"
cargo metadata --locked >/dev/null
python3 scripts/tests/test_gen_assoc_table.py -v
(cd android && ./gradlew :app:testDebugUnitTest --no-daemon)
```
Expected: 全绿；行数门禁通过；engine-core 零 IO；gen 脚本单测通过。

- [ ] **Step 4: 读一次真库联想与旧读者结果**

Run:
```bash
cargo test -p opi-tools --test association_real -- --nocapture
cargo run -q -p opi-tools --bin opi-tools -- verify android/app/src/main/assets/luna.opid
cargo run -q -p opi-tools --bin opi-tools -- verify android/app/src/main/assets/trad.opid
# Spec §9.5：新读者读旧库（未带尾部扩展的 fallback.opid）
cargo run -q -p opi-tools --bin opi-tools -- verify data/generated/fallback.opid
# Spec §9.3：生成脚本确定性（重生成入库的小表，必须逐字节相同）
python3 scripts/gen_assoc_table.py --opid android/app/src/main/assets/luna.opid \
  --phrases data/raw/extra_phrases.tsv --out /tmp/luna.assoc.tsv --roots 5000 --cap 3
diff /tmp/luna.assoc.tsv data/generated/luna.assoc.tsv
python3 scripts/gen_assoc_table.py --opid data/generated/trad.opid \
  --phrases data/raw/trad_phrases.tsv --out /tmp/trad.assoc.tsv --roots 5000 --cap 3
diff /tmp/trad.assoc.tsv data/generated/trad.assoc.tsv
```
Expected: 真库联想非空、条目不含根；四条 `verify` 全过；两个 `diff` 无输出（脚本确定性）。若 `diff` 有差异，如实记录差异并说明是脚本/输入哪一侧变了，不得把带差异的小表当作通过。把 luna 基准的 `select`/`candidates` 实测值写进交付说明（注明这是含 `build_association` 的公开路径，不是裸耗时）。

- [ ] **Step 5: 提交**

```bash
git add CONTRIBUTING.md crates/opi-tools/tests/association_perf.rs
git commit -m "docs+test: 联想构建步骤与性能基准"
```

---

## Self-Review

- **Spec 覆盖**：
  - §3.2–3.7 → Task 1–4；§3.8–3.9 → Task 10–11。
  - §4.1–4.5 → Task 5–8；§4.6 契约测试 → Task 5（`tests/association.rs` 补带索引/assoc 用例）。
  - §5.1–5.6 → Task 12–18；§6 → Task 12–18；§7 → Task 12–18。
  - §8 → 各任务 Files 拆分。
  - §9.1 → Task 5（英文/数字/符号、无索引、`adversarial_input.rs` 显式排除联想态）、Task 8（繁体 trad 缺失回退主库）、Task 11/13/17/18（各宿主）；§9.2 → Task 1（词索引按词非降 + `count*4`/`root_count*6`/`assoc_len` 溢出负例）；§9.3 → Task 11（从 `luna.assoc.tsv` 动态取已知 assoc 项）/Task 19（生成脚本逐字节 diff）；§9.4 → Task 13（`onSelectionChanged` 窗口外清、窗口内不清）；§9.5 → Task 19（`data/generated/fallback.opid` 的 verify）。
  - §10 → Task 19；§11 → 各宿主边界注记；§12 已由用户批准；§13 回滚命令在 Spec 中保留。
- **类型一致性**：`AssocRow`/`AssocView`/`Validated` 字段、`words_with_prefix`/`association_suffixes`、`association_active`/`dismiss_association`/`build_association`、`parse_assoc`/`compile_file_with_extras`、`StateSnapshot.association`、`KEY_ASSOCIATION` 在各任务中同名同型。`associationActive` 按端用该语言惯例承载（Kotlin 扩展属性、Swift/ArkTS 计算属性或缓存、macOS controller 计算属性），但四端判据逐字相同：缓冲空 + displayBuffer 空 + 候选非空 + 拼音/繁体。
- **Review Focus 对应测试**：
  1 → Task 7/12/17/18 的自动测试 + Task 14/15/16 的人工验收清单（iOS/HarmonyOS/macOS 无本地单测 harness，清单是这些端唯一的执行点）；
  2 → Task 13 的 `onSelectionChanged` + Task 16 `mouseDown` 用 `associationActive` + Task 17/18 方向键测试 + Task 14/15 清单；
  3 → Task 5（非中文模式、无索引词典）/Task 8（繁体）/Task 12/18；
  4 → Task 7/8；
  5 → Task 1/11/19。
- **比例**：计划只写接口、关键判据与测试断言；算法细节引用 Spec §3.2/§3.8/§4.2，未逐行抄实现。计划约为 Spec 的 2×；其中测试代码占大头，是 Spec §9 逐条要求的执行点（含六端人工清单），不是实现抄本。
