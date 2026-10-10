// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

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
/// 判定与实测出处：`data/raw/LICENSES.md` 的 2026-10-11「外部词库并入」一节。
pub const PREFIX_MATCH_CAP: usize = 9216;

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
