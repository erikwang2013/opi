# OPI 引擎内核 M1 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 Cargo workspace 与 engine-core 纯逻辑内核：拼音切分、Trie 码表、候选排序、符号引擎、本地学习，全部 cargo test + proptest 验证通过。

**Architecture:** 单 crate `engine-core`，零 IO 零平台依赖。拼音切分（最长匹配）+ BTreeMap Trie（确定性排序）+ 频率排序（静态词频 × 用户学习权重）+ 不可变 `Session` 状态机。数据通过 trait（`Dictionary`）注入，为 M2 词库二进制格式预留接口。

**Tech Stack:** Rust edition 2024 (rustc 1.97.1)、serde/serde_json、proptest（dev-dep）。

---

## 文件结构

| 文件 | 职责 |
|---|---|
| `Cargo.toml` | workspace 根（members = crates/engine-core） |
| `crates/engine-core/Cargo.toml` | crate 清单（serde derive + serde_json + proptest） |
| `crates/engine-core/src/lib.rs` | 模块声明 + 公共 re-export |
| `crates/engine-core/src/pinyin.rs` | 410 音节表 + 最长匹配切分 |
| `crates/engine-core/src/trie.rs` | BTreeMap 前缀 Trie |
| `crates/engine-core/src/dictionary.rs` | `Dictionary` trait + `InMemoryDictionary` |
| `crates/engine-core/src/composer.rs` | 按键状态机（模式/缓冲/大小写） |
| `crates/engine-core/src/learner.rs` | 本地学习 + JSON 导出 |
| `crates/engine-core/src/symbols.rs` | 符号块 + 关键字索引 + 内置数据 |
| `crates/engine-core/src/candidates.rs` | 排序公式 + 候选合并 |
| `crates/engine-core/src/engine.rs` | 门面 + 集成测试 |
| `crates/engine-core/tests/engine_integration.rs` | 端到端集成测试 |
| `crates/engine-core/tests/proptests.rs` | 性质测试 |

---

### Task 1: workspace 脚手架 + engine-core crate

**Files:**
- Create: `Cargo.toml`
- Create: `crates/engine-core/Cargo.toml`
- Create: `crates/engine-core/src/lib.rs`

- [ ] **Step 1: 初始化 git 仓库**

```bash
cd /home/wwwroot/bag/opi && git init
```

Expected: `Initialized empty Git repository in /home/wwwroot/bag/opi/.git/`

- [ ] **Step 2: 创建 workspace 根 Cargo.toml**

`Cargo.toml`：

```toml
[workspace]
resolver = "2"
members = ["crates/engine-core"]

[profile.release]
lto = true
```

- [ ] **Step 3: 创建 engine-core crate 清单**

`crates/engine-core/Cargo.toml`：

```toml
[package]
name = "engine-core"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
proptest = "1"
```

- [ ] **Step 4: 创建 lib.rs（模块声明 + re-export）**

`crates/engine-core/src/lib.rs`：

```rust
//! engine-core：OPI 输入法纯逻辑内核（M1 全拼）。
//! 模块文件在 Task 1 以空占位创建，各任务逐个填充；re-export 随类型就位时添加。

pub mod candidates;
pub mod composer;
pub mod dictionary;
pub mod engine;
pub mod learner;
pub mod pinyin;
pub mod symbols;
pub mod trie;
```

（re-export 随各自任务添加：Task 3 `trie::Entry`、Task 4 dictionary、Task 5 composer、Task 6 learner、Task 9 `engine::Engine`。占位文件见 Step 5。）

- [ ] **Step 5: 创建 8 个空模块占位文件**

为让 crate 从 Task 1 起即可编译（否则 `cargo test -p engine-core --lib <name>` 会因其他模块缺失报 E0583，无法逐任务红→绿），每个模块先建非空占位文件（git 不追踪空文件，故各含一行注释）：

`crates/engine-core/src/candidates.rs`、`composer.rs`、`dictionary.rs`、`engine.rs`、`learner.rs`、`pinyin.rs`、`symbols.rs`、`trie.rs` 各写入：

```rust
// 占位：由 M1 计划对应任务填充
```

- [ ] **Step 6: 验证 build 通过**

Run: `cd /home/wwwroot/bag/opi && cargo build`
Expected: PASS — 空模块占位 + 无 re-export，crate 可编译

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/engine-core/Cargo.toml crates/engine-core/src/lib.rs crates/engine-core/src/
git commit -m "chore: scaffold cargo workspace with engine-core crate"
```

---

### Task 2: 拼音音节表 + 切分

**Files:**
- Modify: `crates/engine-core/src/pinyin.rs`（Task 1 的占位文件）
- Test: `crates/engine-core/src/pinyin.rs`（模块内 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/pinyin.rs`（先只写测试与 stub，Step 4 补全实现）：

```rust
/// 全拼音节表。为保确定性排序，音节按字典序排列，查询用二分。
pub const SYLLABLES: &[&str] = &[
    "a", "ai", "an", "ang", "ao", "ba", "bai", "ban", "bang", "bao", "bei",
    "ben", "beng", "bi", "bian", "biao", "bie", "bin", "bing", "bo", "bu",
    "ca", "cai", "can", "cang", "cao", "ce", "cen", "ceng", "cha", "chai",
    "chan", "chang", "chao", "che", "chen", "cheng", "chi", "chong", "chou",
    "chu", "chua", "chuai", "chuan", "chuang", "chui", "chun", "chuo", "ci",
    "cong", "cou", "cu", "cuan", "cui", "cun", "cuo", "da", "dai", "dan",
    "dang", "dao", "de", "dei", "den", "deng", "di", "dia", "dian", "diao",
    "die", "ding", "diu", "dong", "dou", "du", "duan", "dui", "dun", "duo",
    "e", "ei", "en", "eng", "er", "fa", "fan", "fang", "fei", "fen", "feng",
    "fo", "fou", "fu", "ga", "gai", "gan", "gang", "gao", "ge", "gei", "gen",
    "geng", "gong", "gou", "gu", "gua", "guai", "guan", "guang", "gui", "gun",
    "guo", "ha", "hai", "han", "hang", "hao", "he", "hei", "hen", "heng",
    "hong", "hou", "hu", "hua", "huai", "huan", "huang", "hui", "hun", "huo",
    "ji", "jia", "jian", "jiang", "jiao", "jie", "jin", "jing", "jiong",
    "jiu", "ju", "juan", "jue", "jun", "ka", "kai", "kan", "kang", "kao",
    "ke", "kei", "ken", "keng", "kong", "kou", "ku", "kua", "kuai", "kuan",
    "kuang", "kui", "kun", "kuo", "la", "lai", "lan", "lang", "lao", "le",
    "lei", "leng", "li", "lia", "lian", "liang", "liao", "lie", "lin",
    "ling", "liu", "lo", "long", "lou", "lu", "luan", "lun", "luo", "lv",
    "lve", "ma", "mai", "man", "mang", "mao", "me", "mei", "men", "meng",
    "mi", "mian", "miao", "mie", "min", "ming", "miu", "mo", "mou", "mu",
    "na", "nai", "nan", "nang", "nao", "ne", "nei", "nen", "neng", "ni",
    "nian", "niang", "niao", "nie", "nin", "ning", "niu", "nong", "nou",
    "nu", "nuan", "nun", "nuo", "nv", "nve", "o", "ou", "pa", "pai", "pan",
    "pang", "pao", "pei", "pen", "peng", "pi", "pian", "piao", "pie", "pin",
    "ping", "po", "pou", "pu", "qi", "qia", "qian", "qiang", "qiao", "qie",
    "qin", "qing", "qiong", "qiu", "qu", "quan", "que", "qun", "ran", "rang",
    "rao", "re", "ren", "reng", "ri", "rong", "rou", "ru", "ruan", "rui",
    "run", "ruo", "sa", "sai", "san", "sang", "sao", "se", "sen", "seng",
    "sha", "shai", "shan", "shang", "shao", "she", "shei", "shen", "sheng",
    "shi", "shou", "shu", "shua", "shuai", "shuan", "shuang", "shui", "shun",
    "shuo", "si", "song", "sou", "su", "suan", "sui", "sun", "suo", "ta",
    "tai", "tan", "tang", "tao", "te", "teng", "ti", "tian", "tiao", "tie",
    "ting", "tong", "tou", "tu", "tuan", "tui", "tun", "tuo", "wa", "wai",
    "wan", "wang", "wei", "wen", "weng", "wo", "wu", "xi", "xia", "xian",
    "xiang", "xiao", "xie", "xin", "xing", "xiong", "xiu", "xu", "xuan",
    "xue", "xun", "ya", "yan", "yang", "yao", "ye", "yi", "yin", "ying",
    "yo", "yong", "you", "yu", "yuan", "yue", "yun", "za", "zai", "zan",
    "zang", "zao", "ze", "zei", "zen", "zeng", "zha", "zhai", "zhan",
    "zhang", "zhao", "zhe", "zhei", "zhen", "zheng", "zhi", "zhong", "zhou",
    "zhu", "zhua", "zhuai", "zhuan", "zhuang", "zhui", "zhun", "zhuo", "zi",
    "zong", "zou", "zu", "zuan", "zui", "zun", "zuo",
];

/// 断言音节表已按字典序排序（保护确定性）。
pub fn assert_sorted() {
    debug_assert!(SYLLABLES.windows(2).all(|w| w[0] < w[1]));
}

/// 判断 s 是否为合法音节前缀。
pub fn is_syllable_prefix(s: &str) -> bool {
    SYLLABLES
        .binary_search_by(|&cand| {
            if cand.starts_with(s) {
                std::cmp::Ordering::Equal
            } else {
                cand.as_bytes().cmp(s.as_bytes())
            }
        })
        .is_ok()
}
```

测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syllables_sorted() {
        assert_sorted();
    }

    #[test]
    fn syllable_count_is_410() {
        assert_eq!(SYLLABLES.len(), 410);
    }

    #[test]
    fn longest_match_basic() {
        assert_eq!(segment("xian"), vec!["xian"]);
    }

    #[test]
    fn greedy_longest_chain() {
        assert_eq!(segment("shurufa"), vec!["shu", "ru", "fa"]);
    }

    #[test]
    fn apostrophe_is_hard_separator() {
        assert_eq!(segment("xi'an"), vec!["xi", "an"]);
        assert_eq!(segment("ni'hao"), vec!["ni", "hao"]);
    }

    #[test]
    fn single_letters_fallback() {
        assert_eq!(segment("abc"), vec!["a", "b", "c"]);
    }

    #[test]
    fn max_syllable_len_six() {
        assert_eq!(segment("zhuangzhuang"), vec!["zhuang", "zhuang"]);
    }

    #[test]
    fn prefix_checked() {
        assert!(is_syllable_prefix("zh"));
        assert!(!is_syllable_prefix("zx"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --lib pinyin`
Expected: FAIL — `cannot find function segment`（stub 未实现）

- [ ] **Step 3: 实现切分逻辑**

在 `pinyin.rs` 中追加：

```rust
/// 对输入的拼音串做最长匹配切分（贪婪，最大音节长 6）。
/// `'` 为硬分隔符，单字母未匹配时按单字母切。
pub fn segment(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\'' {
            i += 1;
            continue;
        }
        let mut matched = None;
        let mut len = (chars.len() - i).min(6);
        while len >= 1 {
            let cand: String = chars[i..i + len].iter().collect();
            if is_syllable_prefix(&cand) {
                matched = Some(cand);
                break;
            }
            len -= 1;
        }
        match matched {
            Some(syl) => {
                // 先取长度再 push：`syl` 在 push 时被移动（use-after-move）
                i += syl.chars().count();
                result.push(syl);
            }
            None => {
                result.push(chars[i].to_string());
                i += 1;
            }
        }
    }
    result
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --lib pinyin`
Expected: PASS — 8 tests, 0 failed

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/pinyin.rs
git commit -m "feat: pinyin syllable table and longest-match segmentation"
```

---

### Task 3: Trie 码表

**Files:**
- Modify: `crates/engine-core/src/trie.rs`（Task 1 的占位文件）
- Test: `crates/engine-core/src/trie.rs`（模块内 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/trie.rs`（先写测试 + 空 stub）：

```rust
/// 词典条目：词语 + 静态词频。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub word: String,
    pub freq: u32,
}

use std::collections::BTreeMap;

/// 以拼音为键的前缀 Trie，BTreeMap 子节点保证确定性遍历。
pub struct Trie {
    // TODO: Task 3 Step 3 实现
}

impl Trie {
    pub fn new() -> Self {
        unimplemented!()
    }

    /// 插入或更新 (pinyin, word) 条目。同词重复插入取更高频。
    pub fn insert(&mut self, pinyin: &str, word: &str, freq: u32) {
        unimplemented!()
    }

    /// 查询前缀，按词频降序截断到 limit。
    pub fn query_prefix(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        unimplemented!()
    }

    pub fn len(&self) -> usize {
        unimplemented!()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query() {
        let mut t = Trie::new();
        t.insert("hao", "好", 5000);
        t.insert("hao", "号", 1200);
        let got = t.query_prefix("hao", 8);
        assert_eq!(got[0].word, "好");
        assert_eq!(got[0].freq, 5000);
        assert_eq!(got[1].word, "号");
    }

    #[test]
    fn query_prefix_picks_longest_entry() {
        let mut t = Trie::new();
        t.insert("xiao", "笑", 3000);
        t.insert("xiang", "想", 4000);
        let got = t.query_prefix("xiang", 8);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].word, "想");
    }

    #[test]
    fn upsert_keeps_higher_freq() {
        let mut t = Trie::new();
        t.insert("hao", "好", 100);
        t.insert("hao", "好", 5000);
        let got = t.query_prefix("hao", 8);
        assert_eq!(got[0].freq, 5000);
    }

    #[test]
    fn limit_truncates_sorted() {
        let mut t = Trie::new();
        for (i, w) in ["甲", "乙", "丙", "丁"].iter().enumerate() {
            t.insert("jia", w, 100 - i as u32);
        }
        let got = t.query_prefix("jia", 2);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "甲");
        assert_eq!(got[1].word, "乙");
    }

    #[test]
    fn empty_prefix_returns_nothing() {
        let mut t = Trie::new();
        t.insert("hao", "好", 5000);
        assert!(t.query_prefix("", 8).is_empty());
    }

    #[test]
    fn len_counts_entries() {
        let mut t = Trie::new();
        t.insert("hao", "好", 1);
        t.insert("hao", "号", 1);
        t.insert("xiao", "笑", 1);
        assert_eq!(t.len(), 3);
    }
}
```

- [ ] **Step 1.5: lib.rs re-export**（随类型就位添加）

在 `crates/engine-core/src/lib.rs` 模块声明后追加：

```rust
pub use trie::Entry;
```

（`Entry` 已在 Step 1 的 trie.rs 中定义，re-export 立即可编译。）

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --lib trie`
Expected: FAIL — `unimplemented!` panics（`new` 处）

- [ ] **Step 3: 实现 Trie**

替换 stub 实现（保留 `Entry` 定义与测试）：

```rust
/// Trie 节点：末端词频表 + 子节点表（BTreeMap 保证确定性顺序）。
/// 词频表以词为键，允许同一拼音路径下挂多个词（如 "hao" → 好/号）。
struct Node {
    entries: BTreeMap<String, u32>,
    children: BTreeMap<char, Node>,
}

impl Node {
    fn new() -> Self {
        Node { entries: BTreeMap::new(), children: BTreeMap::new() }
    }
}

pub struct Trie {
    root: Node,
    len: usize,
}

impl Trie {
    pub fn new() -> Self {
        Trie { root: Node::new(), len: 0 }
    }

    pub fn insert(&mut self, pinyin: &str, word: &str, freq: u32) {
        let mut node = &mut self.root;
        for c in pinyin.chars() {
            node = node.children.entry(c).or_insert_with(Node::new);
        }
        let entry = node.entries.entry(word.to_owned()).or_insert(0);
        if freq > *entry {
            if *entry == 0 {
                self.len += 1;
            }
            *entry = freq;
        }
    }

    pub fn query_prefix(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        if pinyin.is_empty() {
            return Vec::new();
        }
        let mut node = &self.root;
        for c in pinyin.chars() {
            match node.children.get(&c) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut acc = Vec::new();
        collect(node, &mut acc);
        acc.sort_by(|a, b| b.freq.cmp(&a.freq));
        acc.truncate(limit);
        acc
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn collect(node: &Node, acc: &mut Vec<Entry>) {
    for (word, freq) in &node.entries {
        acc.push(Entry { word: word.clone(), freq: *freq });
    }
    for child in node.children.values() {
        collect(child, acc);
    }
}
```

（修正说明：原设计单槽 `freq` 存不下同拼音多词（`insert_and_query` 要求 "好"+"号" 同现），且 collect 无法从节点反推词——词必须显式存于末端节点的词频表。`query_prefix` 需对空串提前返回（`empty_prefix_returns_nothing` 测试要求）。）

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --lib trie`
Expected: PASS — 6 tests, 0 failed

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/trie.rs crates/engine-core/src/lib.rs
git commit -m "feat: BTreeMap prefix trie for pinyin dictionary"
```

---

### Task 4: Dictionary trait + InMemoryDictionary

**Files:**
- Modify: `crates/engine-core/src/dictionary.rs`（Task 1 的占位文件）
- Test: `crates/engine-core/src/dictionary.rs`（模块内 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/dictionary.rs`（先写测试 + stub 方法体，Step 3 补实现）：

```rust
use crate::trie::{Entry, Trie};

/// 词典抽象。engine 只依赖此 trait，M2 换成 mmap 二进制实现。
pub trait Dictionary {
    /// 按拼音查候选，返回按词频降序、长度不超过 limit 的条目。
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct InMemoryDictionary {
    trie: Trie,
}

impl InMemoryDictionary {
    pub fn new() -> Self {
        unimplemented!()
    }

    pub fn insert(&mut self, _pinyin: &str, _word: &str, _freq: u32) {
        unimplemented!()
    }
}

impl Dictionary for InMemoryDictionary {
    fn query(&self, _pinyin: &str, _limit: usize) -> Vec<Entry> {
        unimplemented!()
    }

    fn len(&self) -> usize {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_query_works() {
        let mut d = InMemoryDictionary::new();
        d.insert("hao", "好", 5000);
        d.insert("hao", "号", 1200);
        let got = d.query("hao", 8);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "好");
    }

    #[test]
    fn trait_default_is_empty() {
        let d = InMemoryDictionary::new();
        assert!(d.is_empty());
        assert_eq!(d.len(), 0);
    }
}
```

- [ ] **Step 1.5: lib.rs re-export**（随类型就位添加）

在 `crates/engine-core/src/lib.rs` 模块声明后追加：

```rust
pub use dictionary::{Dictionary, InMemoryDictionary};
```

（trait 与类型已在 Step 1 定义，re-export 立即可编译。）

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --lib dictionary`
Expected: FAIL — `unimplemented!` panic（`new` 处）

- [ ] **Step 3: 补齐实现**

替换 stub 方法体（保留 `Entry` 使用、trait 定义与测试）：

```rust
impl InMemoryDictionary {
    pub fn new() -> Self {
        InMemoryDictionary { trie: Trie::new() }
    }

    pub fn insert(&mut self, pinyin: &str, word: &str, freq: u32) {
        self.trie.insert(pinyin, word, freq);
    }
}

impl Dictionary for InMemoryDictionary {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        self.trie.query_prefix(pinyin, limit)
    }

    fn len(&self) -> usize {
        self.trie.len()
    }
}

impl Default for InMemoryDictionary {
    fn default() -> Self {
        Self::new()
    }
}
```

并确认 `lib.rs` 中 `pub mod dictionary;` 已声明（Task 1 已建）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --lib dictionary`
Expected: PASS — 2 tests, 0 failed

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/dictionary.rs crates/engine-core/src/lib.rs
git commit -m "feat: Dictionary trait and in-memory implementation"
```

---

### Task 5: Composer 按键状态机

**Files:**
- Modify: `crates/engine-core/src/composer.rs`（Task 1 的占位文件）
- Test: `crates/engine-core/src/composer.rs`（模块内 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/composer.rs`（先写测试 + stub，Step 3 补实现）：

```rust
/// 输入模式。V1 固定四模式，双拼/五笔经 InputScheme 扩展（V2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 默认模式。
    #[default]
    Pinyin,
    English,
    Number,
    Symbol,
}

/// 一次击键的效果。提交由 Engine 层统一处理（空格键），Composer 只区分更新/忽略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEffect {
    /// 缓冲更新（未提交）。
    Updated,
    /// 按键被忽略（如拼音模式收到非字母）。
    Ignored,
}

/// 输入会话的不可变快照。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    pub mode: Mode,
    pub buffer: String,
    pub shift: bool,
}

pub struct Composer {
    session: Session,
}

impl Composer {
    pub fn new() -> Self {
        unimplemented!()
    }

    /// 处理一次击键，返回效果与新的会话快照。
    pub fn input_key(&mut self, ch: char) -> (KeyEffect, Session) {
        unimplemented!()
    }

    pub fn backspace(&mut self) -> Session {
        unimplemented!()
    }

    pub fn clear(&mut self) -> Session {
        unimplemented!()
    }

    pub fn set_shift(&mut self, on: bool) -> Session {
        unimplemented!()
    }

    /// 切换模式会清空缓冲。
    pub fn switch_mode(&mut self, mode: Mode) -> Session {
        unimplemented!()
    }

    /// 提交当前缓冲（不记录学习，由 Engine 层处理）。
    pub fn commit_buffer(&mut self) -> Session {
        unimplemented!()
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
}

// 注意：`Mode` 的 `Default` 由 `#[derive(Default)]` + `#[default]` 提供（clippy derivable_impls），
// 不要写手写 impl Default for Mode。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinyin_lowercases_letters_and_keeps_apostrophe() {
        let mut c = Composer::new();
        let (eff, s) = c.input_key('N');
        assert_eq!(eff, KeyEffect::Updated);
        assert_eq!(s.buffer, "n");
        let (_, s) = c.input_key('i');
        assert_eq!(s.buffer, "ni");
        let (_, s) = c.input_key('\'');
        assert_eq!(s.buffer, "ni'");
    }

    #[test]
    fn pinyin_ignores_digits_and_symbols() {
        let mut c = Composer::new();
        c.input_key('x');
        let (eff, s) = c.input_key('1');
        assert_eq!(eff, KeyEffect::Ignored);
        assert_eq!(s.buffer, "x");
    }

    #[test]
    fn english_respects_shift() {
        let mut c = Composer::new();
        c.switch_mode(Mode::English);
        c.set_shift(true);
        let (_, s) = c.input_key('a');
        assert_eq!(s.buffer, "A");
        c.set_shift(false);
        let (_, s) = c.input_key('b');
        assert_eq!(s.buffer, "Ab");
    }

    #[test]
    fn number_mode_only_digits() {
        let mut c = Composer::new();
        c.switch_mode(Mode::Number);
        let (eff, _) = c.input_key('a');
        assert_eq!(eff, KeyEffect::Ignored);
        let (_, s) = c.input_key('2');
        assert_eq!(s.buffer, "2");
        let (_, s) = c.input_key('0');
        assert_eq!(s.buffer, "20");
    }

    #[test]
    fn symbol_mode_ignores_all() {
        let mut c = Composer::new();
        c.switch_mode(Mode::Symbol);
        let (eff, _) = c.input_key('a');
        assert_eq!(eff, KeyEffect::Ignored);
        assert_eq!(c.session().buffer, "");
    }

    #[test]
    fn switch_mode_clears_buffer() {
        let mut c = Composer::new();
        c.input_key('n');
        c.switch_mode(Mode::English);
        assert_eq!(c.session().buffer, "");
        assert_eq!(c.session().mode, Mode::English);
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut c = Composer::new();
        c.input_key('n');
        c.input_key('i');
        let s = c.backspace();
        assert_eq!(s.buffer, "n");
    }

    #[test]
    fn commit_buffer_clears() {
        let mut c = Composer::new();
        c.input_key('n');
        c.input_key('i');
        let s = c.commit_buffer();
        assert_eq!(s.buffer, "");
        assert_eq!(c.session().buffer, "");
    }

    #[test]
    fn clear_empties_buffer() {
        let mut c = Composer::new();
        c.input_key('n');
        let s = c.clear();
        assert_eq!(s.buffer, "");
    }
}
```

- [ ] **Step 1.5: lib.rs re-export**（随类型就位添加）

在 `crates/engine-core/src/lib.rs` 模块声明后追加：

```rust
pub use composer::{Composer, KeyEffect, Mode, Session};
```

（四种类型已在 Step 1 定义，re-export 立即可编译。）

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --lib composer`
Expected: FAIL — `unimplemented!` panic

- [ ] **Step 3: 实现 Composer**

替换 stub 方法体：

```rust
impl Composer {
    pub fn new() -> Self {
        Composer { session: Session::default() }
    }

    pub fn input_key(&mut self, ch: char) -> (KeyEffect, Session) {
        use KeyEffect::*;
        let effect = match self.session.mode {
            Mode::Pinyin => {
                if ch.is_ascii_lowercase() || ch == '\'' {
                    self.session.buffer.push(ch);
                    Updated
                } else if ch.is_ascii_uppercase() {
                    self.session.buffer.push(ch.to_ascii_lowercase());
                    Updated
                } else {
                    Ignored
                }
            }
            Mode::English => {
                if ch.is_ascii_alphabetic() {
                    if self.session.shift {
                        self.session.buffer.push(ch.to_ascii_uppercase());
                    } else {
                        self.session.buffer.push(ch);
                    }
                    Updated
                } else {
                    Ignored
                }
            }
            Mode::Number => {
                if ch.is_ascii_digit() {
                    self.session.buffer.push(ch);
                    Updated
                } else {
                    Ignored
                }
            }
            Mode::Symbol => Ignored,
        };
        (effect, self.session.clone())
    }

    pub fn backspace(&mut self) -> Session {
        self.session.buffer.pop();
        self.session.clone()
    }

    pub fn clear(&mut self) -> Session {
        self.session.buffer.clear();
        self.session.clone()
    }

    pub fn set_shift(&mut self, on: bool) -> Session {
        self.session.shift = on;
        self.session.clone()
    }

    pub fn switch_mode(&mut self, mode: Mode) -> Session {
        self.session.mode = mode;
        self.session.buffer.clear();
        self.session.clone()
    }

    pub fn commit_buffer(&mut self) -> Session {
        self.session.buffer.clear();
        self.session.clone()
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
}

impl Default for Composer {
    fn default() -> Self {
        Self::new()
    }
}
```

> clippy 修复（quality review 强制）：`Mode` 用 `#[derive(Default)]` + `#[default]` 替代手写 impl；若 clippy 将 `impl Default for Composer` 判为 derivable_impls，同样改为 `#[derive(Default)]`。全 crate 最终须 `cargo clippy -p engine-core -- -D warnings` 零错误（Task 10 门禁）。

> 设计取舍（Task 5 quality review 确认，计划审定）：Session 形状定为 `{ mode, buffer, shift }`（不含已提交文本/光标），`input_key` 返回整 Session 克隆而非增量 diff——缓冲 ≤6 字符，克隆成本可忽略，Task 9 Engine 层负责提交文本与光标语义。`commit_buffer` 只清缓冲，Engine 调用前须先读 `session().buffer`。`switch_mode` 保留 shift 状态为有意行为。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --lib composer`
Expected: PASS — 9 tests, 0 failed（规格代码逐字含 9 个测试）

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/composer.rs crates/engine-core/src/lib.rs
git commit -m "feat: key event state machine with pinyin/english/number/symbol modes"
```

---

### Task 6: Learner 本地学习 + JSON 导出

**Files:**
- Modify: `crates/engine-core/src/learner.rs`（Task 1 的占位文件）
- Test: `crates/engine-core/src/learner.rs`（模块内 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/learner.rs`（先写测试 + stub）：

```rust
use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};

/// 用户词条（学习记录）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserWord {
    pub text: String,
    pub freq: u32,
}

/// 导出 JSON 的顶层结构，version 为将来云同步的格式协商预留。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserWordExport {
    pub version: u32,
    pub words: Vec<UserWord>,
}

pub struct Learner {
    enabled: bool,
    user_freq: BTreeMap<String, u32>,
    user_words: BTreeSet<String>,
}

impl Learner {
    pub fn new(enabled: bool) -> Self {
        Learner { enabled, user_freq: BTreeMap::new(), user_words: BTreeSet::new() }
    }

    /// 记录一次选词。disabled 时为 no-op。
    pub fn record_selection(&mut self, text: &str) {
        unimplemented!()
    }

    /// 删除自造词（同时清掉频次）。
    pub fn remove_word(&mut self, text: &str) {
        unimplemented!()
    }

    pub fn clear(&mut self) {
        unimplemented!()
    }

    /// 用户词频查询（无记录返回 0）。
    pub fn freq_of(&self, text: &str) -> u32 {
        unimplemented!()
    }

    pub fn is_enabled(&self) -> bool {
        unimplemented!()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        unimplemented!()
    }

    /// 导出为用户可读/可迁移的 JSON。
    pub fn export_json(&self) -> String {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_learner_ignores_selections() {
        let mut l = Learner::new(false);
        l.record_selection("好");
        assert_eq!(l.freq_of("好"), 0);
        assert_eq!(l.export_json(), r#"{"version":1,"words":[]}"#);
    }

    #[test]
    fn records_and_counts_selections() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.record_selection("好");
        l.record_selection("号");
        assert_eq!(l.freq_of("好"), 2);
        assert_eq!(l.freq_of("号"), 1);
    }

    #[test]
    fn remove_word_drops_freq() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.remove_word("好");
        assert_eq!(l.freq_of("好"), 0);
    }

    #[test]
    fn clear_empties_everything() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.clear();
        assert_eq!(l.freq_of("好"), 0);
        assert_eq!(l.export_json(), r#"{"version":1,"words":[]}"#);
    }

    #[test]
    fn export_json_has_version_and_words() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.record_selection("号");
        let json = l.export_json();
        let parsed: UserWordExport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.words.len(), 2);
    }
}
```

- [ ] **Step 1.5: lib.rs re-export**（随类型就位添加）

在 `crates/engine-core/src/lib.rs` 模块声明后追加：

```rust
pub use learner::{Learner, UserWord, UserWordExport};
```

（三种类型已在 Step 1 定义，re-export 立即可编译。）

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --lib learner`
Expected: FAIL — `unimplemented!` panic

- [ ] **Step 3: 实现 Learner**

替换 stub 方法体：

```rust
impl Learner {
    pub fn record_selection(&mut self, text: &str) {
        if !self.enabled {
            return;
        }
        *self.user_freq.entry(text.to_string()).or_insert(0) += 1;
        self.user_words.insert(text.to_string());
    }

    pub fn remove_word(&mut self, text: &str) {
        self.user_freq.remove(text);
        self.user_words.remove(text);
    }

    pub fn clear(&mut self) {
        self.user_freq.clear();
        self.user_words.clear();
    }

    pub fn freq_of(&self, text: &str) -> u32 {
        self.user_freq.get(text).copied().unwrap_or(0)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn export_json(&self) -> String {
        let words = self
            .user_words
            .iter()
            .map(|w| UserWord {
                text: w.clone(),
                freq: self.user_freq.get(w).copied().unwrap_or(0),
            })
            .collect();
        serde_json::to_string(&UserWordExport { version: 1, words }).unwrap()
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --lib learner`
Expected: PASS — 5 tests, 0 failed

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/learner.rs crates/engine-core/src/lib.rs
git commit -m "feat: local learner with JSON export contract"
```

---

### Task 7: SymbolEngine 符号块 + 关键字索引

**Files:**
- Modify: `crates/engine-core/src/symbols.rs`（Task 1 的占位文件）
- Test: `crates/engine-core/src/symbols.rs`（模块内 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/symbols.rs`（先写测试 + stub）：

```rust
/// Unicode 区块 ID（V1 用 u16 编号，M2 数据管线扩展）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u16);

/// Unicode 区块定义。common=true 表示进"常用"面板。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub common: bool,
}

/// 符号条目。keywords 为拼音/英文搜索词，emoji 标记表情。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntry {
    pub text: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub block: BlockId,
    pub emoji: bool,
}

pub struct SymbolEngine {
    blocks: Vec<Block>,
    entries: Vec<SymbolEntry>,
    keyword_index: std::collections::HashMap<String, Vec<usize>>,
    block_index: std::collections::HashMap<BlockId, Vec<usize>>,
}

impl SymbolEngine {
    /// blocks 与 entries 由调用方提供（M2 起来自数据文件），此处构建索引。
    pub fn new(mut blocks: Vec<Block>, entries: Vec<SymbolEntry>) -> Self {
        unimplemented!()
    }

    /// 内置精简符号集（覆盖主要用例，M2 由编译产物替代）。
    pub fn builtin() -> Self {
        unimplemented!()
    }

    /// 查询字符所属区块。
    pub fn block_of(&self, ch: char) -> Option<Block> {
        unimplemented!()
    }

    /// 常用区块列表（面板 Tab）。
    pub fn common_blocks(&self) -> Vec<Block> {
        unimplemented!()
    }

    /// 区块内全部符号。
    pub fn entries_in_block(&self, id: BlockId) -> Vec<SymbolEntry> {
        unimplemented!()
    }

    /// 关键字精确搜索（拼音或英文小写）。
    pub fn search(&self, keyword: &str) -> Vec<SymbolEntry> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_block_lookup() {
        let e = SymbolEngine::builtin();
        let b = e.block_of('。').unwrap();
        assert_eq!(b.id, BlockId(1));
        let b = e.block_of('♥').unwrap();
        assert_eq!(b.id, BlockId(3));
    }

    #[test]
    fn builtin_common_blocks_sorted_by_start() {
        let e = SymbolEngine::builtin();
        let blocks = e.common_blocks();
        assert_eq!(blocks.len(), 5);
        assert!(blocks.windows(2).all(|w| w[0].start <= w[1].start));
    }

    #[test]
    fn entries_in_block_returns_members() {
        let e = SymbolEngine::builtin();
        let texts: Vec<String> =
            e.entries_in_block(BlockId(3)).iter().map(|s| s.text.clone()).collect();
        assert!(texts.contains(&"♥".to_string()));
    }

    #[test]
    fn search_by_pinyin_keyword() {
        let e = SymbolEngine::builtin();
        let got = e.search("xiao");
        assert!(got.iter().any(|s| s.text == "😄"));
    }

    #[test]
    fn search_by_english_keyword() {
        let e = SymbolEngine::builtin();
        let got = e.search("heart");
        assert!(got.iter().any(|s| s.text == "♥"));
    }

    #[test]
    fn search_no_match_empty() {
        let e = SymbolEngine::builtin();
        assert!(e.search("zzz").is_empty());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --lib symbols`
Expected: FAIL — `unimplemented!` panic

- [ ] **Step 3: 实现 SymbolEngine**

替换 stub 方法体（含 `builtin()` 数据）：

```rust
impl SymbolEngine {
    pub fn new(mut blocks: Vec<Block>, entries: Vec<SymbolEntry>) -> Self {
        blocks.sort_by_key(|b| b.start);
        let mut keyword_index: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut block_index: std::collections::HashMap<BlockId, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            block_index.entry(e.block).or_default().push(i);
            for kw in &e.keywords {
                keyword_index.entry(kw.to_lowercase()).or_default().push(i);
            }
        }
        SymbolEngine { blocks, entries, keyword_index, block_index }
    }

    pub fn builtin() -> Self {
        let blocks = vec![
            Block { id: BlockId(1), start: 0x3000, end: 0x303F, name: "CJK 符号".into(), common: true },
            Block { id: BlockId(2), start: 0x25A0, end: 0x25FF, name: "几何图形".into(), common: true },
            Block { id: BlockId(3), start: 0x2600, end: 0x26FF, name: "杂项符号".into(), common: true },
            Block { id: BlockId(4), start: 0x1F600, end: 0x1F64F, name: "表情符号".into(), common: true },
            Block { id: BlockId(5), start: 0x3400, end: 0x4DBF, name: "CJK 扩展 A".into(), common: false },
            Block { id: BlockId(6), start: 0x3040, end: 0x309F, name: "平假名".into(), common: true },
        ];
        let entries = vec![
            SymbolEntry { text: "、".into(), name: "顿号".into(), keywords: vec!["dun".into(), "comma".into()], block: BlockId(1), emoji: false },
            SymbolEntry { text: "。".into(), name: "句号".into(), keywords: vec!["ju".into(), "period".into()], block: BlockId(1), emoji: false },
            SymbolEntry { text: "〈".into(), name: "左书名号".into(), keywords: vec!["shu".into()], block: BlockId(1), emoji: false },
            SymbolEntry { text: "▲".into(), name: "上三角".into(), keywords: vec!["sjx".into(), "triangle".into()], block: BlockId(2), emoji: false },
            SymbolEntry { text: "♥".into(), name: "黑桃心".into(), keywords: vec!["heart".into(), "ai".into(), "xin".into()], block: BlockId(3), emoji: false },
            SymbolEntry { text: "★".into(), name: "实心星".into(), keywords: vec!["star".into(), "xing".into()], block: BlockId(3), emoji: false },
            SymbolEntry { text: "😄".into(), name: "微笑".into(), keywords: vec!["xiao".into(), "smile".into(), "laugh".into()], block: BlockId(4), emoji: true },
            SymbolEntry { text: "あ".into(), name: "平假名a".into(), keywords: vec!["a".into()], block: BlockId(6), emoji: false },
        ];
        SymbolEngine::new(blocks, entries)
    }

    pub fn block_of(&self, ch: char) -> Option<Block> {
        let cp = ch as u32;
        self.blocks
            .iter()
            .find(|b| cp >= b.start && cp <= b.end)
            .cloned()
    }

    pub fn common_blocks(&self) -> Vec<Block> {
        self.blocks.iter().filter(|b| b.common).cloned().collect()
    }

    pub fn entries_in_block(&self, id: BlockId) -> Vec<SymbolEntry> {
        self.block_index
            .get(&id)
            .map(|idx| idx.iter().map(|&i| self.entries[i].clone()).collect())
            .unwrap_or_default()
    }

    pub fn search(&self, keyword: &str) -> Vec<SymbolEntry> {
        self.keyword_index
            .get(&keyword.to_lowercase())
            .map(|idx| idx.iter().map(|&i| self.entries[i].clone()).collect())
            .unwrap_or_default()
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --lib symbols`
Expected: PASS — 6 tests, 0 failed

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/symbols.rs
git commit -m "feat: symbol engine with unicode block and keyword index"
```

---

### Task 8: 候选排序与合并

**Files:**
- Modify: `crates/engine-core/src/candidates.rs`（Task 1 的占位文件）
- Test: `crates/engine-core/src/candidates.rs`（模块内 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/candidates.rs`（先写测试 + stub）：

```rust
use crate::composer::Mode;
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::symbols::{SymbolEngine, SymbolEntry};

/// 用户词频权重：一次选词 ≈ 10 万次静态词频，保证学习迅速生效。
pub const USER_BOOST: u64 = 100_000;
/// 默认候选栏容量。
pub const DEFAULT_TOP_N: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    Hanzi,
    English,
    Emoji,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CandidateKind,
    pub score: u64,
}

/// 排序分：静态词频 + 用户词频 × USER_BOOST。
pub fn rank_score(static_freq: u32, user_freq: u32) -> u64 {
    static_freq as u64 + user_freq as u64 * USER_BOOST
}

/// 合并词典候选与符号候选，排序、去重、截断。
pub fn rank_and_pick<D: Dictionary>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
) -> Vec<Candidate> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::InMemoryDictionary;

    fn test_dict() -> InMemoryDictionary {
        let mut d = InMemoryDictionary::new();
        d.insert("hao", "好", 5000);
        d.insert("hao", "号", 1200);
        d.insert("hao", "豪", 800);
        d.insert("xiao", "笑", 3000);
        d.insert("xiao", "小", 2000);
        d.insert("xiao", "校", 1000);
        d
    }

    #[test]
    fn rank_score_boost_dominates() {
        assert!(rank_score(0, 1) > rank_score(5000, 0));
    }

    #[test]
    fn picks_dictionary_sorted_by_freq() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].text, "好");
        assert_eq!(got[1].text, "号");
        assert_eq!(got[2].text, "豪");
        assert!(got.iter().all(|c| c.kind == CandidateKind::Hanzi));
    }

    #[test]
    fn empty_input_gives_empty() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        assert!(rank_and_pick(&d, &s, &l, "", Mode::Pinyin, DEFAULT_TOP_N).is_empty());
    }

    #[test]
    fn non_pinyin_mode_gives_empty() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        assert!(rank_and_pick(&d, &s, &l, "hao", Mode::English, DEFAULT_TOP_N).is_empty());
    }

    #[test]
    fn learner_boost_reorders() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let mut l = Learner::new(true);
        l.record_selection("豪");
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got[0].text, "豪");
    }

    #[test]
    fn emoji_via_keyword_merge() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "xiao", Mode::Pinyin, DEFAULT_TOP_N);
        assert!(got.iter().any(|c| c.kind == CandidateKind::Emoji && c.text == "😄"));
        assert!(got[0].text == "笑" || got[0].text == "小" || got[0].text == "校");
    }

    #[test]
    fn limit_truncates() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, 1);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "好");
    }

    #[test]
    fn dedupes_by_text() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let mut l = Learner::new(true);
        l.record_selection("好");
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got.iter().filter(|c| c.text == "好").count(), 1);
    }

    #[test]
    fn symbol_only_input() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "heart", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "♥");
        assert_eq!(got[0].kind, CandidateKind::Symbol);
    }

    // quality review 补测：跨源同词去重（dict 与 symbols 同时命中，分数不同故排序后不相邻）。
    fn colliding_symbols() -> SymbolEngine {
        SymbolEngine::new(
            vec![Block { id: BlockId(1), start: 0x3000, end: 0x303F, name: "CJK 符号".into(), common: true }],
            vec![SymbolEntry {
                text: "好".into(),
                name: "好".into(),
                keywords: vec!["hao".into()],
                block: BlockId(1),
                emoji: false,
            }],
        )
    }

    #[test]
    fn dedupes_across_sources() {
        let d = test_dict();
        let s = colliding_symbols();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got.iter().filter(|c| c.text == "好").count(), 1);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --lib candidates`
Expected: FAIL — `unimplemented!` panic

- [ ] **Step 3: 实现 rank_and_pick**

替换 stub：

```rust
pub fn rank_and_pick<D: Dictionary>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
) -> Vec<Candidate> {
    if input.is_empty() || mode != Mode::Pinyin {
        return Vec::new();
    }
    let mut merged: Vec<Candidate> = dict
        .query(input, usize::MAX)
        .into_iter()
        .map(|e| Candidate {
            text: e.word.clone(), // `e.word` 移动后 `&e.word` 借用会 E0382，需 clone
            kind: CandidateKind::Hanzi,
            score: rank_score(e.freq, learner.freq_of(&e.word)),
        })
        .collect();
    for s in symbols.search(input) {
        let SymbolEntry { text, emoji, .. } = s;
        merged.push(Candidate {
            text,
            kind: if emoji { CandidateKind::Emoji } else { CandidateKind::Symbol },
            score: rank_score(0, learner.freq_of(&s.text)),
        });
    }
    merged.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
    // 不能下推 limit 到 dict.query：USER_BOOST 可让低静态词反超截断线外的词，全量收集+排序是唯一正确方案。
    let mut seen = std::collections::HashSet::new();
    merged.retain(|c| seen.insert(c.text.clone()));
    merged.truncate(limit);
    merged
}
```

注意：`emoji` 字段被解构后 `s` 部分移动，需改为借用。用如下写法避免部分移动问题：

```rust
    for s in symbols.search(input) {
        merged.push(Candidate {
            text: s.text.clone(),
            kind: if s.emoji { CandidateKind::Emoji } else { CandidateKind::Symbol },
            score: rank_score(0, learner.freq_of(&s.text)),
        });
    }
```

（若用 `let SymbolEntry { text, emoji, .. } = s;` 解构，`text` 会从 `s` 移出，之后 `s.text` 不可用——统一用 `s.text.clone()`。）

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p engine-core --lib candidates`
Expected: PASS — 9 tests, 0 failed

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/candidates.rs
git commit -m "feat: candidate ranking with user-boost formula"
```

---

### Task 9: Engine 门面 + 集成测试

**Files:**
- Modify: `crates/engine-core/src/engine.rs`（Task 1 的占位文件）
- Create: `crates/engine-core/tests/engine_integration.rs`

- [ ] **Step 1: 写失败集成测试**

`crates/engine-core/tests/engine_integration.rs`：

```rust
use engine_core::candidates::{CandidateKind, DEFAULT_TOP_N};
use engine_core::composer::Mode;
use engine_core::{Dictionary, Engine, InMemoryDictionary};

fn test_engine(learner_enabled: bool) -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 5000);
    d.insert("hao", "号", 1200);
    d.insert("hao", "豪", 800);
    d.insert("xiao", "笑", 3000);
    d.insert("xiao", "小", 2000);
    d.insert("xiao", "校", 1000);
    Engine::new(Box::new(d), engine_core::symbols::SymbolEngine::builtin(), learner_enabled)
}

#[test]
fn candidates_sorted_by_freq() {
    let mut e = test_engine(false);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    let got = e.candidates(DEFAULT_TOP_N);
    assert_eq!(got[0].text, "好");
    assert_eq!(got[1].text, "号");
    assert_eq!(got[2].text, "豪");
}

#[test]
fn space_commits_top_candidate() {
    let mut e = test_engine(false);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    assert_eq!(e.input_key(' '), "好");
    assert_eq!(e.candidates(DEFAULT_TOP_N).len(), 0);
}

#[test]
fn no_candidate_space_commits_raw_buffer() {
    let mut e = test_engine(false);
    e.input_key('x');
    e.input_key('y');
    e.input_key('z');
    assert_eq!(e.input_key(' '), "xyz");
}

#[test]
fn select_records_learning() {
    let mut e = test_engine(true);
    e.input_key('x');
    e.input_key('i');
    e.input_key('a');
    e.input_key('o');
    let c = e.candidates(DEFAULT_TOP_N);
    let idx = c.iter().position(|c| c.text == "小").unwrap();
    assert_eq!(e.select(idx), "小");
    let exported = e.export_user_words();
    assert!(exported.contains("小"));
}

#[test]
fn learner_boost_reorders_after_repeats() {
    let mut e = test_engine(true);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    for _ in 0..3 {
        // 每次循环重算：选中后排序已变，冻结的 idx 会选到别的词。
        let idx = e.candidates(DEFAULT_TOP_N).iter().position(|c| c.text == "豪").unwrap();
        e.select(idx);
        e.input_key('h');
        e.input_key('a');
        e.input_key('o');
    }
    let got = e.candidates(DEFAULT_TOP_N);
    assert_eq!(got[0].text, "豪");
}

#[test]
fn emoji_mixed_into_candidates() {
    let mut e = test_engine(false);
    for ch in "xiao".chars() {
        e.input_key(ch);
    }
    let got = e.candidates(DEFAULT_TOP_N);
    let emoji = got.iter().find(|c| c.kind == CandidateKind::Emoji && c.text == "😄");
    assert!(emoji.is_some());
}

#[test]
fn english_mode_commits_shifted() {
    let mut e = test_engine(false);
    e.switch_mode(Mode::English);
    e.set_shift(true);
    e.input_key('H');
    // shift 是粘滞态（Task 5 语义），打完大写后手动释放
    e.set_shift(false);
    e.input_key('i');
    assert_eq!(e.input_key(' '), "Hi");
}

#[test]
fn number_mode_commits_digits() {
    let mut e = test_engine(false);
    e.switch_mode(Mode::Number);
    for ch in ['2', '0', '2', '6'] {
        e.input_key(ch);
    }
    assert_eq!(e.input_key(' '), "2026");
}

#[test]
fn disabled_learner_exports_empty() {
    let mut e = test_engine(false);
    e.input_key('x');
    e.input_key('i');
    e.input_key('a');
    e.input_key('o');
    let c = e.candidates(DEFAULT_TOP_N);
    let idx = c.iter().position(|c| c.text == "笑").unwrap();
    e.select(idx);
    assert_eq!(e.export_user_words(), r#"{"version":1,"words":[]}"#);
}

#[test]
fn symbol_panel_queries() {
    let mut e = test_engine(false);
    let blocks = e.symbol_blocks();
    assert!(blocks.iter().any(|b| b.name == "杂项符号"));
    let hearts = e.search_symbols("heart");
    assert_eq!(hearts[0].text, "♥");
    let in_block = e.symbols_in_block(engine_core::symbols::BlockId(3));
    assert!(in_block.iter().any(|s| s.text == "♥"));
}

#[test]
fn backspace_clears_buffer() {
    let mut e = test_engine(false);
    e.input_key('h');
    e.input_key('a');
    e.backspace();
    // 缓冲 "h" 仍是音节前缀，还有候选；退两次后缓冲为空。
    assert!(!e.candidates(DEFAULT_TOP_N).is_empty());
    e.backspace();
    assert_eq!(e.candidates(DEFAULT_TOP_N).len(), 0);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p engine-core --test engine_integration`
Expected: FAIL — `cannot find Engine in engine_core`（`Engine` 尚未导出/实现）

- [ ] **Step 3: 实现 Engine 门面**

`crates/engine-core/src/engine.rs`：

```rust
use crate::candidates::{rank_and_pick, Candidate, DEFAULT_TOP_N};
use crate::composer::{Composer, KeyEffect, Mode};
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::symbols::{Block, BlockId, SymbolEngine, SymbolEntry};

/// 引擎门面：输入法 UI 层（经 FFI）交互的唯一入口。
pub struct Engine {
    dict: Box<dyn Dictionary>,
    composer: Composer,
    symbols: SymbolEngine,
    learner: Learner,
}

impl Engine {
    pub fn new(dict: Box<dyn Dictionary>, symbols: SymbolEngine, learner_enabled: bool) -> Self {
        Engine {
            dict,
            composer: Composer::new(),
            symbols,
            learner: Learner::new(learner_enabled),
        }
    }

    /// 处理一次击键，返回需要提交的文本（空串 = 无提交）。
    /// 空格键在 Engine 层拦截：拼音模式选首候选，其他模式提交缓冲。
    pub fn input_key(&mut self, ch: char) -> String {
        if ch == ' ' {
            return self.input_space();
        }
        let (effect, _session) = self.composer.input_key(ch);
        match effect {
            KeyEffect::Updated | KeyEffect::Ignored => String::new(),
        }
    }

    /// 空格键：拼音模式选中首候选（无候选则提交原始缓冲）；英文/数字模式提交缓冲。
    pub fn input_space(&mut self) -> String {
        match self.composer.session().mode {
            Mode::Pinyin => {
                let buffer = self.composer.session().buffer.clone();
                let cands = self.candidates(DEFAULT_TOP_N);
                if cands.is_empty() {
                    self.composer.commit_buffer();
                    buffer
                } else {
                    let top = cands[0].text.clone();
                    self.learner.record_selection(&top);
                    self.composer.commit_buffer();
                    top
                }
            }
            _ => {
                let buffer = self.composer.session().buffer.clone();
                self.composer.commit_buffer();
                buffer
            }
        }
    }

    pub fn backspace(&mut self) {
        self.composer.backspace();
    }

    pub fn clear(&mut self) {
        self.composer.clear();
    }

    pub fn switch_mode(&mut self, mode: Mode) {
        self.composer.switch_mode(mode);
    }

    pub fn set_shift(&mut self, on: bool) {
        self.composer.set_shift(on);
    }

    pub fn buffer(&self) -> &str {
        &self.composer.session().buffer
    }

    pub fn mode(&self) -> Mode {
        self.composer.session().mode
    }

    pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
        let s = self.composer.session();
        rank_and_pick(
            &*self.dict,
            &self.symbols,
            &self.learner,
            &s.buffer,
            s.mode,
            limit,
        )
    }

    /// 选中候选项。越界返回空串。记录学习（若开启）。
    pub fn select(&mut self, index: usize) -> String {
        let cands = self.candidates(usize::MAX);
        match cands.get(index) {
            Some(c) => {
                let text = c.text.clone();
                self.learner.record_selection(&text);
                self.composer.commit_buffer();
                text
            }
            None => String::new(),
        }
    }

    pub fn set_learner(&mut self, enabled: bool) {
        self.learner.set_enabled(enabled);
    }

    pub fn learner_enabled(&self) -> bool {
        self.learner.is_enabled()
    }

    pub fn remove_user_word(&mut self, text: &str) {
        self.learner.remove_word(text);
    }

    pub fn clear_user_words(&mut self) {
        self.learner.clear();
    }

    pub fn export_user_words(&self) -> String {
        self.learner.export_json()
    }

    pub fn symbol_blocks(&self) -> Vec<Block> {
        self.symbols.common_blocks()
    }

    pub fn symbols_in_block(&self, id: BlockId) -> Vec<SymbolEntry> {
        self.symbols.entries_in_block(id)
    }

    pub fn search_symbols(&self, keyword: &str) -> Vec<SymbolEntry> {
        self.symbols.search(keyword)
    }
}
```

（Task 5 的 `KeyEffect` 只含 `Updated` / `Ignored`，此处的 match 已穷尽，无需改动 composer.rs。）

**实现修正（已验证，同步至计划）：** `Engine` 以 `Box<dyn Dictionary>` 持有词典，`rank_and_pick(&*self.dict, ...)` 传 `&dyn Dictionary`，而泛型 `D: Dictionary` 默认有隐式 `Sized` 约束导致编译失败——需将 `candidates.rs` 的签名改为 `pub fn rank_and_pick<D: Dictionary + ?Sized>`。因此 commit 需包含 `candidates.rs`（共 4 个文件）。

- [ ] **Step 3.5: lib.rs re-export**（随类型就位添加）

在 `crates/engine-core/src/lib.rs` 模块声明后追加：

```rust
pub use engine::Engine;
```

（Step 2 的 FAIL 正是 `Engine` 未导出——测试目标无法编译；本步补上导出后 Step 4 转绿。）

- [ ] **Step 4: 运行全部测试确认通过**

Run: `cargo test -p engine-core`
Expected: PASS — 全部单测 + 11 集成测试通过

- [ ] **Step 5: Commit**

```bash
git add crates/engine-core/src/engine.rs crates/engine-core/src/lib.rs crates/engine-core/tests/engine_integration.rs
git commit -m "feat: engine facade with space-select and learning loop"
```

---

### Task 10: proptest 性质测试 + 最终门禁

**Files:**
- Create: `crates/engine-core/tests/proptests.rs`

- [ ] **Step 1: 写失败 proptests**

`crates/engine-core/tests/proptests.rs`：

```rust
use engine_core::candidates::rank_score;
use engine_core::composer::{Composer, Mode};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::pinyin::segment;
use engine_core::trie::Trie;
use engine_core::{Dictionary, Learner};
use proptest::prelude::*;

proptest! {
    #[test]
    fn composer_buffer_matches_input(s in "[a-z']{0,30}") {
        let mut c = Composer::new();
        for ch in s.chars() {
            c.input_key(ch);
        }
        prop_assert_eq!(c.session().buffer, s);
    }

    #[test]
    fn pinyin_segment_joins_to_input(s in "[a-z']{1,20}") {
        // `'` 是硬分隔符，segment 输出不含它，期望值需过滤
        let expected: String = s.chars().filter(|c| *c != '\'').collect();
        let parts = segment(&s);
        let joined: String = parts.iter().flat_map(|p| p.chars()).collect();
        prop_assert_eq!(joined, expected);
    }

    #[test]
    fn trie_roundtrip(words in prop::collection::vec("[a-z]{1,6}", 1..20), freqs in prop::collection::vec(1u32..100_000, 1..20)) {
        let mut t = Trie::new();
        for (w, f) in words.iter().zip(freqs.iter()) {
            t.insert(w, w, *f);
        }
        let got = t.query_prefix(&words[0], usize::MAX);
        prop_assert!(!got.is_empty());
        prop_assert!(got.windows(2).all(|x| x[0].freq >= x[1].freq));
    }

    #[test]
    fn rank_score_monotonic(static_f: u32, user_f: u32) {
        prop_assert!(rank_score(static_f, user_f) >= static_f as u64);
        prop_assert!(rank_score(0, user_f) >= rank_score(0, 0));
    }

    #[test]
    fn learner_counts_match_selections(words in prop::collection::vec("[a-z]{1,8}", 1..10)) {
        let mut l = Learner::new(true);
        for w in &words {
            l.record_selection(w);
        }
        for w in &words {
            prop_assert_eq!(l.freq_of(w), words.iter().filter(|x| *x == w).count() as u32);
        }
    }
}
```

- [ ] **Step 2: 运行测试确认通过**（无红阶段——所有引用类型均已在 Task 2-9 就位）

Run: `cargo test -p engine-core --test proptests`
Expected: PASS — 5 个性质测试通过

- [ ] **Step 3: 最终门禁**

Run: `cargo test -p engine-core && cargo clippy -p engine-core -- -D warnings`
Expected: PASS — 全部测试通过，clippy 零警告

- [ ] **Step 4: Commit**

```bash
git add crates/engine-core/tests/proptests.rs
git commit -m "test: property tests for composer, trie, ranking, learner"
```

---

## M1 验收清单

- [x] `cargo test -p engine-core` 全绿（单元 + 集成 + proptest）—— 2026-08-12 完成
- [x] `cargo clippy -p engine-core -- -D warnings` 零警告 —— 2026-08-12 完成
- [x] 10 个 commit，每个任务一个，测试先行（红 → 绿 → commit）—— 2026-08-12 完成
- [x] 引擎无 IO 无平台依赖：`crates/engine-core` 不出现 `std::fs` / `std::net` / 平台条件编译 —— 2026-08-12 完成
- [x] Session 快照语义（规格 §3.3）：M1 返回完整快照（`Session::clone`），增量 diff（`SessionUpdate`）在 M3 FFI 层计算 —— 已确认

## 后续里程碑（另行计划）

- **M2 数据管线**：`opi-tools` 编译词库 → `.opid` 二进制（魔数 `OPID` + 版本 + DAWG + 词频表），`engine-data` 实现 `Dictionary` 的 mmap 加载 + 校验 + 回退
- **M3 FFI**：flutter_rust_bridge 绑定 `Engine`，`EngineController`（Riverpod）单状态源
- **M4 Android 接入**：InputMethodService + Flutter 键盘进 IME 窗口
- **M5 UI 完善**：符号/Emoji/数字面板 + 设置页
- **M6 学习打磨**：SQLite 落盘、性能门槛（<30ms/键）、TalkBack 无障碍
