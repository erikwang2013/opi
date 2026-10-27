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
//! 2026-10-17 补两条（用户裁决「是词库就要抢位」「如果响应时间超过 50ms 就优先使用
//! 自带词库」）：
//!
//! 5. **抢位加成**：导入层命中的条目过 `boosted_import_freq`（广覆盖：独有与
//!    重复一视同仁，重复取 max(主库原频, 加成值)；锚 = 主库量纲之顶）。代数边界
//!    （严格低于顶、恒不越过用户档、数据写多大都抬不动）写在该函数头；数据侧不再
//!    需要「写大词频」的 hack（`gen_kangxi_dict.py` 的 3.98e9 已回退成 4000）。
//!    门禁：import_only_entries_take_rank_above_ordinary_builtin、
//!    learned_words_still_beat_import_layer、duplicate_word_takes_max_freq（重写版）。
//! 6. **超时兜底（2026-10-18 重构）**：导入层扫描**每键共享一份预算**
//!    （`dict_budget`：2026-10-22 起总账 30ms、初始导入上限 20ms），逐条可抢占（每 64 条查一次
//!    截止）；base 查询之后还会按「总账 − 20ms 后处理 − 已耗时」动态收紧，宽前缀
//!    下导入层提前停。超时 ⇒ 导入层结果**整体丢弃**、本轮纯内置，不粘滞。口子
//!    `Engine::set_import_scan_budget`（None = 不设限）。
//!    门禁：import_scan_over_budget_falls_back_to_builtin_only、
//!    nine_key_scheme_honours_import_budget。
//!
//! 已知边界（如实记，不是缺陷）：查询族走「base 的 top-N + 导入层（rest 截到
//! `IMPORT_HIT_CAP`）」再合并；被 base 自身截断到 top-N 之外的条目若**只在导入层
//! 还重复出现**，合并结果里的词频取的是导入层的值（拿不到 base 那条的频 ——
//! `Entry` 不含拼音串，无法回查）。方向恒为**低估**且只在 base 已有 9216 条以上
//! 前缀命中的极端输入上出现；base 的截断本身在 `PREFIX_MATCH_CAP` 的语义边界里
//! （见 `dictionary.rs` 同常量）。导入层 rest 的 2048 截断是同日性能审计的取舍：
//! 可见窗口只有 8/64 条，精确命中不截（与 base 的 exact 语义一致）。

use crate::dict_budget::{self, QueryState};
use crate::dict_index::{self, ExtraIndex, boosted_import_freq};
use crate::dictionary::{Dictionary, InMemoryDictionary};
use crate::scheme::Scheme;
use crate::trie::Entry;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 导入层扫描的时延截止（2026-10-17 用户裁决「响应超时就优先使用自带词库」；
/// 2026-10-22 总账由 50ms 收紧到 30ms）。
///
/// 取 20ms 而不是 30：30ms 是**总账**（`docs/performance.md` 键反馈 <30ms 的上限），
/// 截止之外还要给内置查询与候选装配留余量；扫描独占 20ms ⇒ 最坏情况整个查询仍在
/// 30ms 内。
///
/// 2026-10-18：截止不再由每次 `Dictionary` 调用各建一份，而是由 `dict_budget`
/// 在**每键查询入口**建一份、所有查询家族共享；`trie` 走可停止遍历，扫描逐条
/// 可抢占（每 64 条查一次），单层不再「层内不可中断」。
/// 本值是**初始上限**：base 查询之后还会按「总账 30ms − 20ms 后处理 − 已耗时」
/// 动态收紧（`import_deadline_dynamic`），宽前缀/慢 base 时导入层提前停。
/// 超时 ⇒ 导入层结果**整体丢弃**，本轮走纯内置；**不粘滞**（下一键重新计时）。
pub(crate) const IMPORT_SCAN_BUDGET: Duration = Duration::from_millis(20);

/// 系统库 + 导入层（见模块头）。
pub(crate) struct LayeredDictionary {
    pub(crate) base: Box<dyn Dictionary>,
    /// 导入层：名字（列表页）+ 库（`engine-data::load_extra_dict` 的产物）。
    pub(crate) extras: Vec<(String, InMemoryDictionary)>,
    /// 导入层扫描预算：`None` = 不设限（测试/高级口）；缺省 `IMPORT_SCAN_BUDGET`。
    import_scan_budget: Option<Duration>,
    /// 每键查询预算槽：`Engine::candidates` 在入口建立 guard（见 `dict_budget`）。
    query_budget: Mutex<QueryState>,
}

impl LayeredDictionary {
    pub(crate) fn new(base: Box<dyn Dictionary>) -> Self {
        LayeredDictionary {
            base,
            extras: Vec::new(),
            import_scan_budget: Some(IMPORT_SCAN_BUDGET),
            query_budget: Mutex::new(QueryState::Inactive),
        }
    }

    /// 改导入层扫描预算（`None` = 不设限）。见 [`IMPORT_SCAN_BUDGET`]。
    pub(crate) fn set_import_scan_budget(&mut self, budget: Option<Duration>) {
        self.import_scan_budget = budget;
    }

    /// 每键查询预算 guard（`Engine::candidates` 在入口调用；见 `dict_budget` 模块头）。
    pub(crate) fn begin_query_budget(&self) -> dict_budget::BudgetGuard<'_> {
        dict_budget::begin(&self.query_budget, self.import_scan_budget)
    }

    /// 当前导入层截止（`None` = 不设限）。没有活动查询时退化成一调用一份（旧语义）。
    pub(crate) fn import_deadline(&self) -> Option<Instant> {
        dict_budget::import_deadline(&self.query_budget, self.import_scan_budget)
    }

    /// base 查询之后的动态截止：总账 30ms − 20ms 后处理 − 已耗时（见 `dict_budget`）。
    fn import_deadline_dynamic(&self) -> Option<Instant> {
        dict_budget::import_deadline_dynamic(&self.query_budget, self.import_scan_budget)
    }

    /// 抢位加成的锚：**主库自己的最大静态词频**（量纲之顶）。不含导入层 ——
    /// 锚是「自带词库的尺」，导入数据不该抬它（叠加见 `boosted_import_freq`）。
    /// ⚠️ 与 `Engine::recompute_user_boost` 的输入**不同源是故意的**：那是
    /// 「学习压过一切」的全局尺（含导入层），这是「抢位到哪一档」的锚。
    pub(crate) fn base_max(&self) -> u32 {
        self.base.max_freq().min(u32::MAX as u64) as u32
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

    /// 导入层在某前缀下的命中索引。
    ///
    /// `None` = **扫描超预算**（2026-10-17 裁决的兜底语义）：调用方必须整体回退到
    /// 纯内置结果 —— 不许用「部分层的结果」充数（半份导入层与「优先自带」不符，
    /// 且不同击键会出不同半份，行为不可预测）。
    ///
    /// 2026-10-18：流式扫描 + 每键共享截止（`dict_budget`），逐条检查可抢占；
    /// 命中按「精确全留 + rest top-[dict_index::IMPORT_HIT_CAP]」收口，把合并/排序
    /// 成本压到有界（实测 2.5 万命中时成本在扫描之外）。
    fn extra_index(&self, prefix: &str, deadline: Option<Instant>) -> Option<ExtraIndex> {
        if self.extras.is_empty() {
            return Some(ExtraIndex::default());
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return None; // 预算为 0 / 本键已耗尽：整层丢弃
        }
        let base_max = self.base_max();
        let mut hits: Vec<(String, usize, u32)> = Vec::new();
        let mut seen = 0u32;
        for (_, dict) in &self.extras {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return None;
            }
            let mut stop = false;
            let ok = dict.for_each_prefix_until(prefix, &mut |py, w, freq| {
                seen += 1;
                if seen.is_multiple_of(64) && deadline.is_some_and(|d| Instant::now() >= d) {
                    stop = true;
                    return false;
                }
                if hits.len() >= dict_index::IMPORT_SCAN_HIT_LIMIT {
                    stop = true;
                    return false;
                }
                hits.push((w.to_string(), py.len(), boosted_import_freq(freq, base_max)));
                true
            });
            if !ok || stop {
                return None;
            }
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return None;
        }
        let hits = dict_index::cap_hits(hits, prefix.len());
        let mut idx = ExtraIndex::default();
        for (w, pl, f) in hits {
            idx.add(&w, pl, f);
        }
        Some(idx)
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
        // 2026-10-18：base 先查（动态导入截止要用它的耗时），再扫导入层。
        let base = self.base.query(pinyin, limit);
        if self.extras.is_empty() {
            return base;
        }
        let Some(mut idx) = self.extra_index(pinyin, self.import_deadline_dynamic()) else {
            // 超预算：本轮纯内置（2026-10-17 裁决「超时就优先使用自带词库」；2026-10-22 总账 30ms）。
            return base;
        };
        if idx.is_empty() {
            // 导入层在这个键上没有命中：base 的结果一字不动（省掉逐条查表 + 重排）。
            return base;
        }
        let mut out = self.merge(base, &mut idx);
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
        let base = self.base.query_all(pinyin);
        if self.extras.is_empty() {
            return base;
        }
        let Some(mut idx) = self.extra_index(pinyin, self.import_deadline_dynamic()) else {
            return base; // 超预算兜底，同 `query`
        };
        if idx.is_empty() {
            return base;
        }
        let mut out = self.merge(base, &mut idx);
        out.extend(idx.rest());
        out
    }

    fn query_prefix_top(&self, prefix: &str, n: usize) -> Vec<Entry> {
        let base = self.base.query_prefix_top(prefix, n);
        if self.extras.is_empty() {
            return base;
        }
        // base 的 top-N（大库的流式出口）逐字保留，导入层（exact/rest 各截到
        // IMPORT_HIT_CAP）并入后再做同一条取舍：exact 全量 + rest 词频 top-n。
        // 被 base 截掉的 rest 尾巴词频 ≤ 它的第 n 名，
        // 而 base 前 n 名在合并里只会被抬频、不会被移除 ⇒ 尾巴挤不进合并后的 top-n
        // （freq 并列除外 —— 那本就是「n 边界不保证确定」的既有语义）。
        let plen = prefix.len();
        let Some(mut idx) = self.extra_index(prefix, self.import_deadline_dynamic()) else {
            return base; // 超预算兜底，同 `query`
        };
        if idx.is_empty() {
            // 无命中前缀直通 base：合并的代价只在「导入层真掺和了这个前缀」时付。
            return base;
        }
        let mut all = self.merge(base, &mut idx);
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
        // 实现在 `dict_layers_visit.rs`（500 行门禁拆出）；trait impl 只能有一处。
        self.visit_entries_budgeted(f);
    }

    fn words_with_prefix(&self, prefix: &str) -> Vec<(String, u32)> {
        self.base.words_with_prefix(prefix)
    }

    fn association_suffixes(&self, root: &str) -> Vec<(String, u32)> {
        self.base.association_suffixes(root)
    }

    /// 前缀区间遍历：base 走 `visit_prefix`（mmap 二分区间），导入层同区间合并 ——
    /// 默认实现会全表扫描再过滤，见 `visit_prefix_budgeted` 头注。
    fn visit_prefix(&self, prefix: &str, f: &mut dyn FnMut(&str, &str, u32)) {
        self.visit_prefix_budgeted(prefix, f);
    }

    /// 按键类展开（9/14 键）：base 的区间枚举覆写照旧调用（**不落回全表扫描** ——
    /// 那正是 2026-10-11 移动端提速要消灭的）；导入层走**可抢占扫描 + rest 截断**，
    /// 共享每键预算，超时整层丢弃回 base（2026-10-18 性能审计：旧实现无截止）。
    fn scheme_entries(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        n: usize,
    ) -> Vec<(Entry, String)> {
        let base = self.base.scheme_entries(input, scheme, pin, n);
        if self.extras.is_empty() {
            return base;
        }
        // base 已算完：动态截止把它的耗时计入总账（宽前缀/慢 base 时导入层提前停）。
        let deadline = self.import_deadline_dynamic();
        let base_max = self.base_max();
        let mut hits: Vec<(Entry, String)> = Vec::new();
        let mut seen = 0u32;
        for (_, dict) in &self.extras {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return base;
            }
            let mut stop = false;
            let ok = dict.for_each_entry_until(&mut |py, w, freq| {
                seen += 1;
                if seen.is_multiple_of(64) && deadline.is_some_and(|d| Instant::now() >= d) {
                    stop = true;
                    return false;
                }
                if !scheme.matches(input, py) {
                    return true;
                }
                // 钉读音 = 读音恰等（与 `scheme_entries_scan` 的过滤逐字同语义）。
                if let Some(p) = pin
                    && py != p
                {
                    return true;
                }
                if hits.len() >= dict_index::IMPORT_SCAN_HIT_LIMIT {
                    stop = true;
                    return false;
                }
                hits.push((
                    Entry {
                        word: w.to_string(),
                        freq: boosted_import_freq(freq, base_max),
                        pinyin_len: py.len(),
                    },
                    py.to_string(),
                ));
                true
            });
            if !ok || stop {
                return base;
            }
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return base;
        }
        // exact 与 rest 各自按最终排序键截到 IMPORT_HIT_CAP（与查询族同一条取舍）。
        let (mut exact, mut rest): (Vec<_>, Vec<_>) = hits
            .into_iter()
            .partition(|(e, _)| e.pinyin_len == input.len());
        exact.sort_by(|a, b| b.0.freq.cmp(&a.0.freq).then(a.0.word.cmp(&b.0.word)));
        exact.truncate(dict_index::IMPORT_HIT_CAP);
        rest.sort_by(|a, b| b.0.freq.cmp(&a.0.freq).then(a.0.word.cmp(&b.0.word)));
        rest.truncate(dict_index::IMPORT_HIT_CAP);
        exact.extend(rest);
        // base + 导入命中按 (拼音, 词) 去重取最大；再 exact 全量 + rest top-n。
        let mut best: HashMap<(String, String), u32> = HashMap::new();
        let take = |e: Entry, py: String, best: &mut HashMap<(String, String), u32>| {
            best.entry((py, e.word))
                .and_modify(|f| *f = (*f).max(e.freq))
                .or_insert(e.freq);
        };
        for (e, py) in base {
            take(e, py, &mut best);
        }
        for (e, py) in exact {
            take(e, py, &mut best);
        }
        let mut exact_out: Vec<(Entry, String)> = Vec::new();
        let mut rest_out: Vec<(Entry, String)> = Vec::new();
        for ((py, word), freq) in best {
            let e = Entry {
                word,
                freq,
                pinyin_len: py.len(),
            };
            // 与 `scheme_entries_scan` 同一分区规则：键字节长 == 输入键数 ⇒ 精确。
            if e.pinyin_len == input.len() {
                exact_out.push((e, py));
            } else {
                rest_out.push((e, py));
            }
        }
        rest_out.sort_by(|a, b| b.0.freq.cmp(&a.0.freq).then(a.0.word.cmp(&b.0.word)));
        rest_out.truncate(n);
        exact_out.extend(rest_out);
        exact_out
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
        if self.extras.is_empty() {
            return None;
        }
        // 2026-10-18：导入层扫描共享每键预算、逐条可抢占；超时整层丢弃（base 已
        // 无见证者 ⇒ 返回 None，与「没有导入层」同义）。base 已算完 ⇒ 动态截止。
        let deadline = self.import_deadline_dynamic();
        let mut seen = 0u32;
        for (_, dict) in &self.extras {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return None;
            }
            let mut found = None;
            let mut stop = false;
            let ok = dict.for_each_entry_until(&mut |py, w, _f| {
                seen += 1;
                if seen.is_multiple_of(64) && deadline.is_some_and(|d| Instant::now() >= d) {
                    stop = true;
                    return false;
                }
                if !scheme.matches(input, py) {
                    return true;
                }
                // 钉读音 = 读音恰等（同 `scheme_entries_scan` 的过滤）。
                if let Some(p) = pin
                    && py != p
                {
                    return true;
                }
                if pred(py, w) {
                    found = Some(py.len());
                    return false;
                }
                true
            });
            if let Some(len) = found {
                return Some(len);
            }
            if !ok || stop {
                return None;
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

#[cfg(test)]
#[path = "dict_layers_assoc_tests.rs"]
mod assoc_tests;
