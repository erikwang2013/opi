// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::scheme::Scheme;
use crate::trie::{Entry, Trie};

/// 前缀扩展层的静态词频截断（2026-10-11，用户裁决；整合主流词库后的性能出口）。
///
/// 主路径的 `merged` 层（[`Dictionary::query_prefix_top`]）只取「精确匹配全量 +
/// 前缀扩展里静态词频最高的 9216 条」。为什么是 9216：**覆盖旧库全部既有行为** ——
/// 旧 luna 最大单字母前缀（`z`）原始命中 8,613 条（去重后 8,447），n 取 9216 时旧库
/// 任何输入都不受影响（探针自证：old `z` 在新代码下仍为 8,447）；新库 `z` = 80,502 条，
/// 被截掉的是从没见过第 1025 页以后的尾巴。
/// 语义边界如实记：学习词借 boost 跳进前缀 top-8 的机会只在 top-N 池内有；
/// **精确层不截**（打全拼音翻页不受影响，「候选总数不设上限」的承诺在那里）。
/// 9/14 键的按键类层走同一条取舍（见 [`Dictionary::scheme_entries`]）。
/// 判定与实测出处：`data/raw/LICENSES.md` 的 2026-10-11「外部词库并入」一节。
pub const PREFIX_MATCH_CAP: usize = 9216;

/// 主查询前缀层的实际取回上限（2026-10-18 性能）：没有学习词时 boost 不会把池外
/// 的低静态词提到前面，最终只留 `limit` 条 ⇒ 取 `limit*8`（下限 256）就够；
/// 有学习词时必须留满 `PREFIX_MATCH_CAP`，否则「学过一次的低频词」会被提前截掉。
pub(crate) fn prefix_cap(has_user_words: bool, limit: usize) -> usize {
    if has_user_words || limit == usize::MAX {
        PREFIX_MATCH_CAP
    } else {
        PREFIX_MATCH_CAP.min(limit.saturating_mul(8).max(256))
    }
}

/// 9/14 键按键类层的静态词频截断（2026-10-11 移动端提速；与 [`PREFIX_MATCH_CAP`]
/// 同一族取舍）。为什么比 26 键小：scheme 层每个候选还要付**引擎侧**的钱 —— 逐条
/// `segment()` 出左列读音 + 排序去重（实测 9k 候选 ≈ 11ms 桌面；loader 的区间枚举
/// 本体只 1.9ms）。9 键候选栏每屏 8 个、真机可达翻页深度 ≪ 256 页（2048/8），
/// 取 2048 把单键拉回 ~4ms（桌面），语义边界与 [`PREFIX_MATCH_CAP`] 逐字相同：
/// 精确（键长 == 输入键数）不截、只截前缀扩展尾巴。
pub const SCHEME_MATCH_CAP: usize = 2048;

/// [`Dictionary::scheme_entries`] 的默认/回退实现（全表扫描 + 逐条 [`Scheme::matches`]）。
/// `MmapDictionary` 的区间枚举覆写在**分词输入**（`'`，matches_segmented 按段长比）时
/// 也走这里 —— 那套语义前缀区间表达不了。
pub fn scheme_entries_scan<D: Dictionary + ?Sized>(
    dict: &D,
    input: &str,
    scheme: Scheme,
    pin: Option<&str>,
    n: usize,
) -> Vec<(Entry, String)> {
    let mut all: Vec<(Entry, String)> = Vec::new();
    dict.visit_entries(&mut |py, w, f| {
        if !scheme.matches(input, py) {
            return;
        }
        // 钉读音 = **读音恰等**（2026-10-16 用户裁决「左侧选择拼音的字了，就不要联想词组了，
        // 选择的是字」）。此前是 `starts_with`（以钉住读音开头）—— 读音更长的词与延伸读音的
        // 字都跟着进列表；真库实测钉 `me` 第 12 条起出「美俄/每个」、钉 `ni'hao` 第 3 条起
        // 出「你好啊」，门禁在 `tests/scheme_readings.rs` 与 `engine-data/tests/pin_reading_real.rs`。
        // ⚠️ 四处过滤（本扫描两处 + `MmapDictionary` 两处）必须同语义。
        if let Some(p) = pin
            && py != p
        {
            return;
        }
        all.push((
            Entry {
                word: w.to_string(),
                freq: f,
                pinyin_len: py.len(),
            },
            py.to_string(),
        ));
    });
    let plen = input.len();
    let mut exact: Vec<(Entry, String)> = Vec::new();
    let mut rest: Vec<(Entry, String)> = Vec::new();
    for e in all {
        if e.0.pinyin_len == plen {
            exact.push(e);
        } else {
            rest.push(e);
        }
    }
    rest.sort_by(|a, b| b.0.freq.cmp(&a.0.freq).then(a.0.word.cmp(&b.0.word)));
    rest.truncate(n);
    exact.extend(rest);
    exact
}

/// [`Dictionary::scheme_find`] 的默认/回退实现（全表 + 早停；`'` 输入同 [`scheme_entries_scan`]）。
pub fn scheme_find_scan<D: Dictionary + ?Sized>(
    dict: &D,
    input: &str,
    scheme: Scheme,
    pin: Option<&str>,
    pred: &mut dyn FnMut(&str, &str) -> bool,
) -> Option<usize> {
    let mut found = None;
    dict.visit_entries(&mut |py, w, _f| {
        if found.is_some() || !scheme.matches(input, py) {
            return;
        }
        // 钉读音 = 读音恰等（同 `scheme_entries_scan` 的过滤，改一处必同改另一处）。
        if let Some(p) = pin
            && py != p
        {
            return;
        }
        if pred(py, w) {
            found = Some(py.len());
        }
    });
    found
}

/// 词典抽象。engine 只依赖此 trait，M2 换成 mmap 二进制实现。
/// Send + Sync：FFI 层 frb auto-opaque 句柄要求 Api: Send + Sync。
pub trait Dictionary: Send + Sync {
    /// 按拼音查候选，返回按词频降序、长度不超过 limit 的条目。
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry>;
    /// 同 [`Self::query`]，但**不保证顺序**，且不截断 —— 给「全量收集后按自己的权威键
    /// 重排」的调用方（`rank_and_pick`）。默认实现退化为 `query(pinyin, usize::MAX)`。
    fn query_all(&self, pinyin: &str) -> Vec<Entry> {
        self.query(pinyin, usize::MAX)
    }
    /// 前缀层 top-N（大词库的性能出口，2026-10-11 加）：返回「`prefix` 的**全部精确
    /// 匹配**（键 == prefix）+ 其余前缀扩展中**静态词频最高的至多 n 条**」；顺序不保证。
    ///
    /// 为什么存在：主路径原先 `query_all` 全量物化 —— 整合主流开源词库后单字母前缀实测
    /// 80,502 条、每次击键 41–50ms（其中物化占 24.6ms；出处见 `data/raw/LICENSES.md`
    /// 2026-10-11 一节）。本方法把**前缀扩展层**按静态词频截到 n 条，**精确层不截** ——
    /// 「候选总数不设上限」的承诺保留在用户可达的精确查询上（打全拼音翻页不受影响）。
    ///
    /// 语义边界（如实）：n 取 `dictionary::PREFIX_MATCH_CAP`（9216，覆盖旧库全部既有
    /// 行为 —— 旧库最大单字母前缀原始命中 8,613 条）；被截掉的是新库的前缀尾巴
    /// （z 80,502→9,216）。学习词借 boost 跳进前缀 top-8 的机会只在 top-N 池内有
    /// （打全拼音走精确层，不受影响）。
    ///
    /// 默认实现 = `query_all` 后按词频截断（语义等价、慢）；`MmapDictionary` 覆写为
    /// 流式实现（只物化 exact + top-N，不物化其余）。
    fn query_prefix_top(&self, prefix: &str, n: usize) -> Vec<Entry> {
        let plen = prefix.len();
        let all = self.query_all(prefix);
        let mut exact: Vec<Entry> = Vec::new();
        let mut rest: Vec<Entry> = Vec::new();
        for e in all {
            if e.pinyin_len == plen {
                exact.push(e);
            } else {
                rest.push(e);
            }
        }
        rest.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.word.cmp(&b.word)));
        rest.truncate(n);
        exact.extend(rest);
        exact
    }
    /// 全量遍历条目（零拷贝借用）。给**按键类展开**（9/14 键方案）用：那种匹配
    /// 没法用前缀查询表达 —— 「64」不是任何拼音键的前缀，得沿**键类**（6∈{m,n,o}）
    /// 比对全部键（见 `scheme.rs`）。默认空实现：替身词库（测试）按需覆写，
    /// 生产词库（`MmapDictionary`）覆写为零拷贝扫描。
    fn visit_entries(&self, _f: &mut dyn FnMut(&str, &str, u32)) {}

    /// 只遍历拼音键以 `prefix` 开头的条目（零拷贝借用），顺序 = 表序（拼音字节序）。
    ///
    /// 用途：声母缩写匹配（`abbr.rs`）只要看**首字母**那一段 —— 全表 93 万条对
    /// 每次击键太贵（`scheme_entries` 的 60ms 实测见上）。默认实现走全表 + `starts_with`
    /// 过滤（替身词库够用）；`MmapDictionary` 覆写为二分区间扫描。
    fn visit_prefix(&self, prefix: &str, f: &mut dyn FnMut(&str, &str, u32)) {
        self.visit_entries(&mut |py, w, fr| {
            if py.starts_with(prefix) {
                f(py, w, fr);
            }
        });
    }

    /// 按键类匹配（9/14 键）的整档：**精确匹配（键字节长 == 输入键数）全量 + 其余匹配里
    /// 静态词频最高的至多 n 条**（与 [`Self::query_prefix_top`] 同一条取舍）；每条带拼音串
    /// （9 键左列逐条读数要用）。顺序不保证。
    ///
    /// `pin` = 钉读音（9 键左列点一行拼音）：只保留拼音**恰等于**它的条目（2026-10-16
    /// 用户裁决，见 `scheme_entries_scan` 的注）—— `None` = 不钉。
    ///
    /// 语义与逐条 [`Scheme::matches`] + pin 过滤**等价**（`'` 分词输入也在内 —— 覆写把它
    /// 交给 [`scheme_entries_scan`]）。默认实现 = 全表扫描（替身词库的量级无所谓）；
    /// `MmapDictionary` 覆写为**按键类区间枚举**：表按拼音字节序排，「一位按键类 = 一组
    /// 字母前缀 = 一组连续区间」，逐位枚举 + `lower_bound` 剪空子树 —— 2026-10-11 移动端
    /// 实测：936k 条全表扫一次 ≈ 60ms 桌面，手机端每键秒级卡顿（那时还没有本方法）。
    fn scheme_entries(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        n: usize,
    ) -> Vec<(Entry, String)> {
        scheme_entries_scan(self, input, scheme, pin, n)
    }

    /// 按键类匹配的**存在性/见证者查询**：在所有匹配键（含 pin 过滤）里用 `pred(pinyin, word)`
    /// 找第一个为真者，返回其**键字节长度**；无则 None。`scheme::scheme_fallback_len` 的
    /// 判据 3/4 专用 —— **不许被 top-N 截断**，故与 [`Self::scheme_entries`] 分开。
    /// 默认实现 = 全表 + 早停；`MmapDictionary` 覆写为区间枚举 + 早停。
    fn scheme_find(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        pred: &mut dyn FnMut(&str, &str) -> bool,
    ) -> Option<usize> {
        scheme_find_scan(self, input, scheme, pin, pred)
    }
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// 全库最大静态词频（学习权重缩放依据，未知返回 0）。
    fn max_freq(&self) -> u64 {
        0
    }
}

#[derive(Default)]
pub struct InMemoryDictionary {
    trie: Trie,
    max_freq: u32,
}

impl InMemoryDictionary {
    pub fn new() -> Self {
        InMemoryDictionary {
            trie: Trie::new(),
            max_freq: 0,
        }
    }

    pub fn insert(&mut self, pinyin: &str, word: &str, freq: u32) {
        self.max_freq = self.max_freq.max(freq);
        self.trie.insert(pinyin, word, freq);
    }

    /// 可停止的前缀遍历（导入层预算扫描用；语义同 `query_all(prefix)`，但流式、
    /// 不排序、逐条可停）。返回 false = 被回调停。
    pub(crate) fn for_each_prefix_until(
        &self,
        prefix: &str,
        f: &mut dyn FnMut(&str, &str, u32) -> bool,
    ) -> bool {
        self.trie.for_each_prefix_until(prefix, f)
    }

    /// 可停止的全量遍历（9/14 键 scheme 扫描用）。
    pub(crate) fn for_each_entry_until(&self, f: &mut dyn FnMut(&str, &str, u32) -> bool) -> bool {
        self.trie.for_each_until(f)
    }
}

impl Dictionary for InMemoryDictionary {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        self.trie.query_prefix(pinyin, limit)
    }

    fn visit_entries(&self, f: &mut dyn FnMut(&str, &str, u32)) {
        self.trie.for_each(f);
    }

    fn len(&self) -> usize {
        self.trie.len()
    }

    fn max_freq(&self) -> u64 {
        self.max_freq as u64
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
