// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 简拼（声母缩写）门禁：`nh` → 你好、`zg` → 中国 的**缩写一侧**。
//!
//! 走 `Engine::candidates`（= `rank_and_pick`）：分档规则全长在那一层，绕到 `dict.query`
//! 就让缺陷对门禁不可见（memory: ranking-gate-bypasses-ranking-code）。
//!
//! **一律可控小词库，不在真库上断言「简拼生效」**：真库下目标词可能排在几十位之外、
//! 超 `FETCH_LIMIT = 64`，那种验证是假绿（本仓库栽过）。这里只验两件事：
//! ① 缩写能查到**只有缩写能查到**的词；② 简拼档只**追加**在精确档之后。
//!
//! 词频取真实量级（`data/raw/trad_hanzi.tsv`）：简拼档按真实词频排，合成的小数字会把
//! 「档内按词频」这条门槛调没。

use engine_core::Engine;
use engine_core::candidates::{USER_BOOST, rank_and_pick};
use engine_core::composer::Mode;
use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::jianpin::{MAX_EXPANSIONS, expansions, per_position_cap};
use engine_core::learner::Learner;
use engine_core::pinyin::{SYLLABLES, is_syllable};
use engine_core::symbols::SymbolEngine;
use engine_core::trie::Entry;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine(d: InMemoryDictionary) -> Engine {
    Engine::new(Box::new(d), no_symbols(), false)
}

/// 敲入拼音并取候选，`limit` 条。
fn cands_n(e: &mut Engine, input: &str, limit: usize) -> Vec<String> {
    e.clear();
    for ch in input.chars() {
        e.input_key(ch);
    }
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 取**全量**候选（`usize::MAX`：本文件看的是排序与分档，不是截断）。
fn cands(e: &mut Engine, input: &str) -> Vec<String> {
    cands_n(e, input, usize::MAX)
}

fn pos(list: &[String], word: &str) -> usize {
    list.iter()
        .position(|t| t == word)
        .unwrap_or_else(|| panic!("「{word}」不在候选里：{list:?}"))
}

// ---------- 缩写能到、且只有缩写能到 ----------

/// 词库里**没有**任何以 `nh` 开头的键 ⇒ `nh` 的精确档为空 ⇒ 简拼档就是主结果。
/// 两个位置各有 3 个「有证据的音节」，网格 3×3；`nehe` 那一格专门用来证明网格
/// **不是只有第一格**（展开若是「只取每位置第一名的组合」就查不到「讷河」）。
fn abbrev_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("ni", "你", 3_999_128_899u32),
        ("na", "那", 3_995_000_000),
        ("ne", "呢", 3_900_000_000),
        ("hao", "好", 3_997_635_583),
        ("hai", "还", 3_996_000_000),
        ("he", "和", 3_992_000_000),
        // 只有两个音节**连写**才存在的词：全拼要打全，缩写打 nh
        ("nihao", "你好", 47_594),
        ("nehe", "讷河", 900),
    ] {
        d.insert(py, w, f);
    }
    d
}

#[test]
fn abbreviation_reaches_words_the_exact_tier_cannot() {
    let mut e = engine(abbrev_dict());
    // 背景：全拼「nihao」首位是「你好」（精确等长匹配，同音无竞争者）
    assert_eq!(pos(&cands(&mut e, "nihao"), "你好"), 0);

    // 被测：缩写「nh」也要能到。精确档为空 ⇒ 简拼档就是主结果。
    let got = cands(&mut e, "nh");
    assert!(
        got.contains(&"你好".into()),
        "打 nh 到不了「你好」：{got:?}"
    );
    assert!(
        got.contains(&"讷河".into()),
        "网格不完整（ne+he 那一格没查）：{got:?}"
    );
    // 档内按真实词频排（「你好」47594 > 「讷河」900）
    assert!(
        pos(&got, "你好") < pos(&got, "讷河"),
        "简拼档没按词频排：{got:?}"
    );
}

/// 「只增不删」的**可判定**形态。词库故意构造得能分辨「追加」与「并排重排」：
/// - `nh` 键存在（嗯哈 freq 5）⇒ 精确档非空，且带 `exact_bonus` 必居首；
/// - `nhao` 是 `nh` 的**前缀扩展**（那好 freq 100）⇒ 精确档里有一条**低分**候选；
/// - 「你好」freq 47594 只能由简拼档（查 `nihao`）带来。
///
/// 并排重排会让 47594 的「你好」压过 100 的「那好」，追加则不会 —— 这条因此可判定。
#[test]
fn abbreviation_tier_only_appends_after_the_exact_ones() {
    let mut e = engine(append_dict());
    let got = cands(&mut e, "nh");
    assert_eq!(got[0], "嗯哈", "精确匹配 + exact_bonus 必须首位：{got:?}");

    let (p_ext, p_jp) = (pos(&got, "那好"), pos(&got, "你好"));
    assert!(
        p_ext < p_jp,
        "简拼档与精确档并排重排了：「你好」在第 {p_jp} 位，必须排在精确档的前缀扩展「那好」（第 {p_ext} 位）之后：{got:?}"
    );

    // 精确档成员一个不少、且整体集中在开头
    let probe = append_dict();
    let exact: HashSet<String> = probe.query_all("nh").into_iter().map(|x| x.word).collect();
    let flags: Vec<bool> = got.iter().map(|t| exact.contains(t)).collect();
    let k = flags.iter().position(|f| !f).unwrap_or(flags.len());
    assert!(
        flags[k..].iter().all(|f| !f),
        "简拼档插进了精确档中间：{got:?}"
    );
    for w in &exact {
        assert!(got.contains(w), "精确候选「{w}」被简拼档挤掉了：{got:?}");
    }
    // 非真空：关掉简拼档这一行必须红（否则本文件只剩「什么都没变」的断言）
    assert!(
        got.len() > exact.len(),
        "简拼档压根没接上（精确档 {} 条，候选 {} 条）：{got:?}",
        exact.len(),
        got.len()
    );
}

fn append_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("nh", "嗯哈", 5u32),
        ("nhao", "那好", 100),
        ("ni", "你", 3_999_128_899),
        ("hao", "好", 3_997_635_583),
        ("nihao", "你好", 47_594),
    ] {
        d.insert(py, w, f);
    }
    d
}

/// 繁体模式走的是同一条分档（`rank_and_pick` 对 Pinyin/Traditional 同码路径），
/// 简拼档不得只在 Pinyin 下接线。
#[test]
fn abbreviation_also_reaches_in_traditional_mode() {
    let d = append_dict();
    let l = Learner::new(false);
    let got = rank_and_pick(
        &d,
        &no_symbols(),
        &l,
        "nh",
        Mode::Traditional,
        usize::MAX,
        USER_BOOST,
    );
    let texts: Vec<String> = got.into_iter().map(|c| c.text).collect();
    assert!(
        texts.contains(&"你好".into()),
        "繁体模式没接简拼：{texts:?}"
    );
}

// ---------- 上限 ----------

/// 每个 z-/g- 音节各挂一条（词频递减，排序确定）—— 把网格铺满，让「每位置取几个」
/// 这件事在断言下可见。
fn full_grid_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (i, s) in SYLLABLES
        .iter()
        .filter(|s| s.starts_with('z') || s.starts_with('g'))
        .enumerate()
    {
        d.insert(s, &format!("字{i:03}"), 10_000 - i as u32);
    }
    d
}

/// 上限是**组合数**的上限：k 个位置 × 每位置 cap 个音节是 cap^k 的笛卡尔积。
/// 关掉上限时 2 个位置就是 37×19 = 703 个展开键（每次击键上）。
///
/// 数字写死一处：上限是**产品决策**（见 jianpin.rs 模块头「取 512 而非 fuzzy 的 16」），
/// 改它必须连带更新这里的期望值，不许悄悄漂。
#[test]
fn expansion_grid_is_capped() {
    assert_eq!(
        MAX_EXPANSIONS, 512,
        "上限被改了？连带核对 jianpin.rs 的取 512 理由"
    );
    let d = full_grid_dict();
    let l = Learner::new(false);
    // 每个位置取满 cap，但网格是 `∏ min(cap, 该字母音指数)` —— g 只有 19 个音节 < cap，
    // 故两位置是 22×19，不是 512。
    let two = expansions(&d, &l, USER_BOOST, "zg");
    assert_eq!(two.len(), grid_size("zg"), "两位置的完整网格：{two:?}");
    let mut uniq = two.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), two.len(), "展开键重复：{two:?}");
    // 3/4/8 个位置：8^3 = 512、4^4 = 256、2^8 = 256（不封顶时 37·19·37 = 26011）
    for input in ["zgz", "zgzg", "zzzzzzzz"] {
        let got = expansions(&d, &l, USER_BOOST, input);
        assert_eq!(got.len(), grid_size(input), "{input} 的网格：{got:?}");
        assert!(got.len() <= MAX_EXPANSIONS, "{input} 突破了上限：{got:?}");
    }
}

/// 每位置取 `per_position_cap(k)` 个音节，但该字母不足这么多时按实际数量 —— 期望值由
/// `SYLLABLES` 现算，不写死音节数（表被 410 条断言锁着，但数一遍比抄一遍稳）。
fn grid_size(letters: &str) -> usize {
    let cap = per_position_cap(letters.chars().count());
    letters
        .chars()
        .map(|l| {
            SYLLABLES
                .iter()
                .filter(|s| s.starts_with(l))
                .count()
                .min(cap)
        })
        .product()
}

/// 记录 `query_all` 调用。上限的真实含义是**每次击键的词库查询次数**，只看候选条数
/// 会被词库大小稀释 —— 查询次数要能直接数出来。
/// `Dictionary: Send + Sync` ⇒ 用 `Mutex` 而非 `RefCell`。
struct Recorder {
    inner: InMemoryDictionary,
    calls: Mutex<Vec<String>>,
}

impl Recorder {
    fn new(inner: InMemoryDictionary) -> Arc<Self> {
        Arc::new(Recorder {
            inner,
            calls: Mutex::new(Vec::new()),
        })
    }

    /// 简拼展开键 = **非音节**的整串（展开必是 ≥2 个音节的拼接，永远不等于某个音节）；
    /// 逐音节扫描查的是**完整音节**。两者用 `is_syllable` 就能分开。
    /// 输入自身要排除：主路径第一件事就是 `query_all(input)`，而纯缩写按定义不是音节
    /// （不排除就会多算 1 —— 实测 16 个展开键被数成 17）。
    fn joins(&self, input: &str) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_str() != input && !is_syllable(c))
            .cloned()
            .collect()
    }
}

/// `Arc<Recorder>` 交出 `Box<dyn Dictionary>` 后仍要能读到调用记录（Engine 拿走所有权）。
struct Shared(Arc<Recorder>);

impl Dictionary for Shared {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        self.0.query(pinyin, limit)
    }
    fn query_all(&self, pinyin: &str) -> Vec<Entry> {
        self.0.query_all(pinyin)
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn max_freq(&self) -> u64 {
        self.0.max_freq()
    }
}

impl Dictionary for Recorder {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        self.inner.query(pinyin, limit)
    }
    fn query_all(&self, pinyin: &str) -> Vec<Entry> {
        self.calls.lock().unwrap().push(pinyin.to_owned());
        self.inner.query_all(pinyin)
    }
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn max_freq(&self) -> u64 {
        self.inner.max_freq()
    }
}

/// 引擎路径上查的就是那个**完整网格**（恰好 `∏ min(cap, 该字母音指数)` 次展开键）；
/// 长缩写（8 个字母）不随长度爆炸。非真空：中间那行断言「至少查了一次」—— 关掉简拼档即为 0，红。
#[test]
fn per_keystroke_joined_queries_are_capped() {
    for input in ["zg", "zzzzzzzz"] {
        let rec = Recorder::new(full_grid_dict());
        let mut e = Engine::new(Box::new(Shared(Arc::clone(&rec))), no_symbols(), false);
        e.clear();
        for ch in input.chars() {
            e.input_key(ch);
        }
        let _ = e.candidates(usize::MAX);
        let n = rec.joins(input).len();
        assert!(n > 0, "{input}: 一个展开键都没查（简拼档没接上）");
        assert!(
            n <= MAX_EXPANSIONS,
            "{input}: 展开了 {n} 个键，超过上限 {MAX_EXPANSIONS}"
        );
        assert_eq!(n, grid_size(input), "{input}: 查的不是完整网格");
    }
}

// ---------- 守卫：只在纯缩写上展开 ----------

/// 非纯缩写一律不展开（尤其：**单字母输入不展开** —— 否则每个词的第一键都要扫全字母
/// 音节表）。「真音节」`a/e/o` 也不例外：打 `a` 要的是「啊」，不是 `ai/an/ang` 的缩写。
#[test]
fn expansion_guard_fires_only_on_pure_abbreviations() {
    let d = full_grid_dict();
    let l = Learner::new(false);
    for input in [
        "nihao", "ni", "zhongguo", "zh", "n", "a", "e", "o", "", "n1",
    ] {
        assert!(
            expansions(&d, &l, USER_BOOST, input).is_empty(),
            "{input:?} 不该展开：{:?}",
            expansions(&d, &l, USER_BOOST, input)
        );
    }
    // 非真空：真缩写必须展开
    assert!(
        !expansions(&d, &l, USER_BOOST, "zg").is_empty(),
        "真缩写没展开"
    );
    // 词库里没有任何 q- 音节条目 ⇒ 不凭空造查库串
    assert!(
        expansions(&d, &l, USER_BOOST, "zq").is_empty(),
        "'q' 音节无条目，不该有网格位"
    );
}
