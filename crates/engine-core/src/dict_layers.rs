// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入词库的合并层（M3 计划 Task 2b，2026-10-14）。
//!
//! `Engine` 的主库槽位持有本类型：**系统库（base）+ 导入层（extras）**。导入层排在
//! 系统库之前参与合并，其余语义与单库逐条一致 —— 排序、截断、精确加成不另起一套，
//! `candidates.rs` 照旧读同一个 `Dictionary` 口（这是「不许另造排序」的落点：合并只
//! 发生在词条层，权威排序仍是 `rank_and_pick`）。
//!
//! 四条裁决（2026-10-14）：
//!
//! 1. **去重键 = (拼音, 词)**（查询族里 `Entry` 只带拼音**长度**，键即 (词, 键长)；
//!    `visit_entries` 拿到真拼音串，用它）。与「把两层逐条插进同一个 trie」的键
//!    逐字一致。**只按词去重是错的**：单库自己允许同词挂多个读音（`行` 在 xing/hang
//!    各一条），按词折会把 base 的合法条目也吃掉 —— 那是单库语义，不是合并该管的。
//! 2. **同键取最大词频**（不是 min、不是相加）：与 `Trie::insert` 和解析器的「重复
//!    取最大」是同一条既有规则。base 先占位 ⇒ 词频并列时保留 base 的键长与词频，
//!    导入层不因并列翻转既有候选。
//! 3. **导入层只叠主库**：繁体模式仍查 trad 库（`Engine::active_dict` 的路由），
//!    导入层不参与；trad 缺失时 Traditional 回退主库，那时导入层自然可见。理由：
//!    trad.opid 是另一套文字数据，「导入一份词库」的语义是补主库，不是往繁体表里
//!    塞条目（真要换繁体数据，走 `install_trad` 那条既有通路）。
//!    门禁：tests/dict_layers.rs::trad_mode_does_not_see_import_layer。
//! 4. **`max_freq` 随层合并**：`Engine::user_boost`（= max(USER_BOOST, 最大词频×2)）
//!    的重算靠它把导入层高频词算进去，否则「一次选词压过全部静态词」的不变量被
//!    导入层破坏。门禁：import_high_freq_word_recomputes_user_boost。
//!
//! 已知边界（如实记，不是缺陷）：查询族走「base 的 top-N + 导入层全量」再合并，
//! 被 base 自身截断到 top-N 之外的条目若**只在导入层还重复出现**，合并结果里的词频
//! 取的是导入层的值（拿不到 base 那条的频 —— `Entry` 不含拼音串，无法回查）。
//! 方向恒为**低估**且只在 base 已有 9216 条以上前缀命中的极端输入上出现；base 的
//! 截断本身在 `PREFIX_MATCH_CAP` 的语义边界里（见 `dictionary.rs` 同常量）。

use crate::dictionary::{Dictionary, InMemoryDictionary};
use crate::scheme::Scheme;
use crate::trie::Entry;
use std::collections::HashMap;

/// 导入层在某次查询下的命中索引：词 → [(键长, 词频)]（跨导入层同键取最大）。
///
/// 键按**词**索引是为了 `get_mut(&str)` 免克隆 —— 合并要把 base 的**每一条**结果
/// 拿来查一次（单字母前缀可达 9216 条），每条克隆一个 String 会把合并的常数顶上去。
#[derive(Default)]
struct ExtraIndex {
    by_word: HashMap<String, Vec<(usize, u32)>>,
}

impl ExtraIndex {
    fn is_empty(&self) -> bool {
        self.by_word.is_empty()
    }

    fn add(&mut self, word: &str, pinyin_len: usize, freq: u32) {
        let slot = self.by_word.entry(word.to_string()).or_default();
        match slot.iter_mut().find(|(pl, _)| *pl == pinyin_len) {
            Some((_, f)) => *f = (*f).max(freq),
            None => slot.push((pinyin_len, freq)),
        }
    }

    /// 取走 (词, 键长) 的导入层词频（**消费制**：剩下没被取走的就是只在导入层出现的条目）。
    fn take(&mut self, word: &str, pinyin_len: usize) -> Option<u32> {
        let slot = self.by_word.get_mut(word)?;
        let at = slot.iter().position(|(pl, _)| *pl == pinyin_len)?;
        let (_, freq) = slot.swap_remove(at);
        if slot.is_empty() {
            self.by_word.remove(word);
        }
        Some(freq)
    }

    /// 剩余条目，按 (词频降序, 词, 键长) 排 —— HashMap 迭代序不定，排序只为输出确定。
    fn rest(self) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for (word, hits) in self.by_word {
            for (pinyin_len, freq) in hits {
                out.push(Entry {
                    word: word.clone(),
                    freq,
                    pinyin_len,
                });
            }
        }
        out.sort_by(|a, b| {
            b.freq
                .cmp(&a.freq)
                .then(a.word.cmp(&b.word))
                .then(a.pinyin_len.cmp(&b.pinyin_len))
        });
        out
    }
}

/// 系统库 + 导入层（见模块头）。
pub(crate) struct LayeredDictionary {
    base: Box<dyn Dictionary>,
    /// 导入层：名字（列表页）+ 库（`engine-data::load_extra_dict` 的产物）。
    extras: Vec<(String, InMemoryDictionary)>,
}

impl LayeredDictionary {
    pub(crate) fn new(base: Box<dyn Dictionary>) -> Self {
        LayeredDictionary {
            base,
            extras: Vec::new(),
        }
    }

    /// **全集替换**（M3 契约）：enable/disable = 在/不在这个集合里；不追加、不嵌套
    /// —— 直接换掉字段，不存在「上一轮的层还裹在里面」的形态。
    pub(crate) fn set_extras(&mut self, extras: Vec<(String, InMemoryDictionary)>) {
        self.extras = extras;
    }

    /// 列表页数据：名字 + 条数，顺序 = 传入顺序。
    pub(crate) fn extras_list(&self) -> Vec<(String, usize)> {
        self.extras
            .iter()
            .map(|(name, dict)| (name.clone(), dict.len()))
            .collect()
    }

    /// 导入层在某前缀下的全部命中（导入层通常小，全量取；免去「层内也要 top-N」的伪优化）。
    fn extra_index(&self, prefix: &str) -> ExtraIndex {
        let mut idx = ExtraIndex::default();
        for (_, dict) in &self.extras {
            for e in dict.query_all(prefix) {
                idx.add(&e.word, e.pinyin_len, e.freq);
            }
        }
        idx
    }

    /// 把导入层命中并进 base 的结果（同 (词, 键长) 取最大词频；base 先占位）。
    fn merge(&self, base: Vec<Entry>, idx: &mut ExtraIndex) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::with_capacity(base.len());
        for mut e in base {
            if let Some(extra) = idx.take(&e.word, e.pinyin_len) {
                e.freq = e.freq.max(extra);
            }
            out.push(e);
        }
        out
    }
}

impl Dictionary for LayeredDictionary {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        if self.extras.is_empty() {
            return self.base.query(pinyin, limit);
        }
        let mut idx = self.extra_index(pinyin);
        if idx.is_empty() {
            // 导入层在这个键上没有命中：base 的结果一字不动（省掉逐条查表 + 重排）。
            return self.base.query(pinyin, limit);
        }
        let mut out = self.merge(self.base.query(pinyin, limit), &mut idx);
        out.extend(idx.rest());
        out.sort_by(|a, b| {
            b.freq
                .cmp(&a.freq)
                .then(a.word.cmp(&b.word))
                .then(a.pinyin_len.cmp(&b.pinyin_len))
        });
        out.truncate(limit);
        out
    }

    fn query_all(&self, pinyin: &str) -> Vec<Entry> {
        if self.extras.is_empty() {
            return self.base.query_all(pinyin);
        }
        let mut idx = self.extra_index(pinyin);
        if idx.is_empty() {
            return self.base.query_all(pinyin);
        }
        let mut out = self.merge(self.base.query_all(pinyin), &mut idx);
        out.extend(idx.rest());
        out
    }

    fn query_prefix_top(&self, prefix: &str, n: usize) -> Vec<Entry> {
        if self.extras.is_empty() {
            return self.base.query_prefix_top(prefix, n);
        }
        // base 的 top-N（大库的流式出口）逐字保留，导入层**全量**并入后再做同一条取舍：
        // exact 全量 + rest 词频 top-n。被 base 截掉的 rest 尾巴词频 ≤ 它的第 n 名，
        // 而 base 前 n 名在合并里只会被抬频、不会被移除 ⇒ 尾巴挤不进合并后的 top-n
        // （freq 并列除外 —— 那本就是「n 边界不保证确定」的既有语义）。
        let plen = prefix.len();
        let mut idx = self.extra_index(prefix);
        if idx.is_empty() {
            // 无命中前缀直通 base：合并的代价只在「导入层真掺和了这个前缀」时付。
            return self.base.query_prefix_top(prefix, n);
        }
        let mut all = self.merge(self.base.query_prefix_top(prefix, n), &mut idx);
        all.extend(idx.rest());
        let mut exact: Vec<Entry> = Vec::new();
        let mut rest: Vec<Entry> = Vec::new();
        for e in all {
            if e.pinyin_len == plen {
                exact.push(e);
            } else {
                rest.push(e);
            }
        }
        rest.sort_by(|a, b| {
            b.freq
                .cmp(&a.freq)
                .then(a.word.cmp(&b.word))
                .then(a.pinyin_len.cmp(&b.pinyin_len))
        });
        rest.truncate(n);
        exact.extend(rest);
        exact
    }

    fn visit_entries(&self, f: &mut dyn FnMut(&str, &str, u32)) {
        if self.extras.is_empty() {
            return self.base.visit_entries(f);
        }
        // 键 = (拼音串, 词)：这里能拿到真拼音（不像查询族只有键长），与「一个大字典」
        // 的键逐字相同。索引只存**导入层**的条目（小），两级 HashMap 是为了 base 的
        // 每条都拿 `&str` 直查、不克隆 —— 单个 `HashMap<(&str, &str), _>` 会被
        // 生命周期间题卡住（闭包参数是 for<'x> 的高阶借用，装不进固定生命周期的键）。
        let mut idx: HashMap<String, HashMap<String, u32>> = HashMap::new();
        for (_, dict) in &self.extras {
            dict.visit_entries(&mut |py, w, freq| {
                idx.entry(py.to_string())
                    .or_default()
                    .entry(w.to_string())
                    .and_modify(|f| *f = (*f).max(freq))
                    .or_insert(freq);
            });
        }
        self.base.visit_entries(&mut |py, w, freq| {
            let merged = match idx.get_mut(py).and_then(|words| words.remove(w)) {
                Some(extra) => freq.max(extra),
                None => freq,
            };
            (*f)(py, w, merged);
        });
        // 剩余 = 只在导入层出现的条目，按 (拼音, 词) 排 —— HashMap 迭代序不定，排序只为确定。
        let mut rest: Vec<(String, String, u32)> = Vec::new();
        for (py, words) in idx {
            for (w, freq) in words {
                rest.push((py.clone(), w, freq));
            }
        }
        rest.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (py, w, freq) in rest {
            (*f)(&py, &w, freq);
        }
    }

    /// 按键类展开（9/14 键）：base 的区间枚举覆写照旧调用（**不落回全表扫描** ——
    /// 那正是 2026-10-11 移动端提速要消灭的），导入层小、走默认扫描，再按
    /// (拼音, 词) 去重取最大词频，最后套同一条取舍：exact 全量 + rest top-n。
    fn scheme_entries(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        n: usize,
    ) -> Vec<(Entry, String)> {
        if self.extras.is_empty() {
            return self.base.scheme_entries(input, scheme, pin, n);
        }
        let mut best: HashMap<(String, String), u32> = HashMap::new();
        let take = |e: Entry, py: String, best: &mut HashMap<(String, String), u32>| {
            best.entry((py, e.word))
                .and_modify(|f| *f = (*f).max(e.freq))
                .or_insert(e.freq);
        };
        for (e, py) in self.base.scheme_entries(input, scheme, pin, n) {
            take(e, py, &mut best);
        }
        for (_, dict) in &self.extras {
            for (e, py) in dict.scheme_entries(input, scheme, pin, n) {
                take(e, py, &mut best);
            }
        }
        let mut exact: Vec<(Entry, String)> = Vec::new();
        let mut rest: Vec<(Entry, String)> = Vec::new();
        for ((py, word), freq) in best {
            let e = Entry {
                word,
                freq,
                pinyin_len: py.len(),
            };
            // 与 `scheme_entries_scan` 同一分区规则：键字节长 == 输入键数 ⇒ 精确。
            if e.pinyin_len == input.len() {
                exact.push((e, py));
            } else {
                rest.push((e, py));
            }
        }
        rest.sort_by(|a, b| b.0.freq.cmp(&a.0.freq).then(a.0.word.cmp(&b.0.word)));
        rest.truncate(n);
        exact.extend(rest);
        exact
    }

    fn scheme_find(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        pred: &mut dyn FnMut(&str, &str) -> bool,
    ) -> Option<usize> {
        // 见证者语义（trait 文档）：与「base 全遍历、再看导入层」的合并遍历序一致
        // —— base 里先出现的为真者胜出；base 没有时按传入顺序取导入层的第一个。
        if let Some(len) = self.base.scheme_find(input, scheme, pin, pred) {
            return Some(len);
        }
        for (_, dict) in &self.extras {
            if let Some(len) = dict.scheme_find(input, scheme, pin, pred) {
                return Some(len);
            }
        }
        None
    }

    /// 条数 = base + 导入层**逐层相加**（跨层同键算两次）—— 它只用来判空、与当
    /// 简拼缓存代际（`(len, max_freq, …)`），不是精确计数。判空仍然正确：非空 ⇔
    /// 三层里至少一层非空。
    fn len(&self) -> usize {
        self.base.len() + self.extras.iter().map(|(_, d)| d.len()).sum::<usize>()
    }

    fn max_freq(&self) -> u64 {
        self.extras
            .iter()
            .map(|(_, d)| d.max_freq())
            .fold(self.base.max_freq(), u64::max)
    }
}
