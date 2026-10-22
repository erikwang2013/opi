// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入层的命中索引、抢位加成与命中上限（2026-10-18 从 `dict_layers.rs` 拆出：
//! 加预算/可抢占扫描后那里越了 500 行门禁；先例是 `engine_dicts.rs` 一族）。
//!
//! 拆的是**纯数据件**：`ExtraIndex`（合并用的词 → 键长/词频）、`boosted_import_freq`
//! （抢位代数，原注释随函数走）、`IMPORT_HIT_CAP`（rest 命中截断）。`dict_layers.rs`
//! 仍拥有查询/扫描/合并的流程。

use crate::trie::Entry;
use std::collections::HashMap;

/// 导入层每次查询保留的命中上限（**exact 与 rest 各一份**，2026-10-18 性能审计）。
///
/// 为什么需要：实测 10 万行导入库、`ni` 前缀 2.5 万命中时，扫描本身 ~8ms，但
/// `extra_index` 的 HashMap + 合并 + 排序 + 候选重排把 26 键 p95 推过 50ms ——
/// 成本在扫描之外。可见窗口只有 8（折叠）/64（抓取）条；导入条目的加成后词频几乎
/// 恒定，按最终排序键 `(词频, 词, 键长)` 取 top-2048 对可见窗口语义几乎无损。
///
/// 边界（如实）：精确命中原本不截（与 base 的 exact 语义一致），但实测 10 万行库里
/// `qiu` 的 1.25 万命中**全是 exact**，不截就仍要把 merge/排序成本吃满；故 exact 与
/// rest 各留 2048（合计 ≤4096），精确优先仍由候选层的 exact_bonus 表达。2048 与
/// 9/14 键的 `SCHEME_MATCH_CAP` 同量级，来自移动端「每屏 8 条」的可达翻页深度。
pub(crate) const IMPORT_HIT_CAP: usize = 2048;

/// 单次导入扫描的**命中条数硬上限**（2026-10-18 性能审计）。
///
/// 40 万条全挂在一个前缀下的病态导入层：即使扫描逐条可抢占、40ms 截止也会先
/// 把时间花掉（base 查询 + 组装另算），整键仍可能 ~80ms。命中数上限让这种层在
/// **几毫秒内**判定「太大、整层丢弃」，回纯内置；10 万行库的常见前缀（`ni` 2.5 万、
/// `qiu` 1.25 万）与康熙 2 万条都远在 5 万之下，正常导入层不受影响。
/// 超限语义与超时一致：**整层丢弃**，不允许半份结果。
pub(crate) const IMPORT_SCAN_HIT_LIMIT: usize = 50_000;

/// 导入层的机制性加成（2026-10-17 用户裁决「是词库就要抢位」）。
///
/// **广覆盖**：导入层命中的每个 (词, 键长) 都过本公式，合并时与主库重复的条目
/// 取 `max(主库原频, 加成后的导入频)` —— **覆盖即抢位**。为什么连重复项也抬
/// （2026-10-17 实证）：蘒在 luna 里**本来就有**（埋在 qiu 第 108 位）——只抬
/// 「独有条目」的语义下，它作为重复项不获加成，永远出不了手机抓取窗；
/// 旧的数据常数（3.98e9 写进 TSV）恰恰是「广覆盖」的常数版，用户验收过的
/// 落位（馫@43/蘒@50）就是它。加成落在**入池单点**（`extra_index`/`visit_entries`/
/// `scheme_entries` 收集侧），输出侧不重复施加。
///
/// **锚 = 主库的量纲之顶**（`base.max_freq()`，不是 boost 分数）：
/// `有效词频 = min(数据词频, 顶/128) + (顶 − 顶/128)`。
///
/// 为什么锚顶上而不是 boost 的某个分数档（实测教训）：luna 的单字层是**位置编码**
/// ——顶带（≈3.9–4e9）密排着各读音最热的几十个字。锚 `boost/4`（=2e9）时馫落在
/// xin 的 31 名开外（抓取窗 64 内没有它，门禁当场红）；锚到「顶下 0.8%」才复现
/// 旧数据常数实地验证过的落点（实测 馫@23、蘒@24 一带）。
///
/// 代数结论（门禁 `tests/dict_layers.rs` 三条 + C-ABI 两条钉住）：
/// · **严格低于顶**：`min(数据, 顶/128) ≤ 顶/128` ⇒ `eff ≤ 顶 − 1` ⇒ 站在量纲
///   顶上的主库条目（最热的那一撮）仍在导入条目之上 ——「抢位可以，但压不过天花板」；
/// · **高于顶带以下的一切**：`eff ≥ 顶 − 顶/128` ⇒ 静态词频低于顶带的条目全在
///   导入条目之下（馫蘒一族由此在抓取窗内；顶带以内的保持天然次序）；
/// · **永远压不过学过的**：用户档 = 静态 + 1×boost ≥ 2×顶 + 1 > 导入条目上限
///   （eff + exact_bonus ≤ 顶 + 顶 = 2×顶）。数据里写多大都没用 —— 公式对数据
///   只取到 `顶/128` 的夹紧（写超了也抬不动）；boost 也不给抬（锚不含导入层）；
/// · **u32 恒安全**：`eff ≤ 顶 ≤ u32::MAX`（主库词频本身是 u32），无需再夹。
pub(crate) fn boosted_import_freq(raw: u32, base_max: u32) -> u32 {
    let unit = base_max / 128;
    let clamped = raw.min(unit);
    // −1：整数除法下 min(数据, 顶/128) 最大取到 顶/128，若不减 1，数据写满时会
    // 得到「eff = 顶」——与顶并列（排序退到词序）；减 1 才是文档承诺的严格低于顶。
    clamped.saturating_add(base_max.saturating_sub(unit).saturating_sub(1))
}

/// 导入层在某次查询下的命中索引：词 → [(键长, 词频)]（跨导入层同键取最大）。
///
/// 键按**词**索引是为了 `get_mut(&str)` 免克隆 —— 合并要把 base 的**每一条**结果
/// 拿来查一次（单字母前缀可达 9216 条），每条克隆一个 String 会把合并的常数顶上去。
#[derive(Default)]
pub(crate) struct ExtraIndex {
    by_word: HashMap<String, Vec<(usize, u32)>>,
}

impl ExtraIndex {
    pub(crate) fn is_empty(&self) -> bool {
        self.by_word.is_empty()
    }

    pub(crate) fn add(&mut self, word: &str, pinyin_len: usize, freq: u32) {
        let slot = self.by_word.entry(word.to_string()).or_default();
        match slot.iter_mut().find(|(pl, _)| *pl == pinyin_len) {
            Some((_, f)) => *f = (*f).max(freq),
            None => slot.push((pinyin_len, freq)),
        }
    }

    /// 取走 (词, 键长) 的导入层词频（**消费制**：剩下没被取走的就是只在导入层出现的条目）。
    pub(crate) fn take(&mut self, word: &str, pinyin_len: usize) -> Option<u32> {
        let slot = self.by_word.get_mut(word)?;
        let at = slot.iter().position(|(pl, _)| *pl == pinyin_len)?;
        let (_, freq) = slot.swap_remove(at);
        if slot.is_empty() {
            self.by_word.remove(word);
        }
        Some(freq)
    }

    /// 剩余条目（= 没被 base 的条目消费掉的），按 (词频降序, 词, 键长) 排 ——
    /// HashMap 迭代序不定，排序只为输出确定。词频**已在入索引时加成**（勿重复施加）。
    pub(crate) fn rest(self) -> Vec<Entry> {
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

/// 把命中列表按最终排序键收口：exact 与 rest **各自**截到 [IMPORT_HIT_CAP]。
///
/// 入参 = `(词, 键长, 加成后词频)`；`plen` = 本次查询的输入串长度（字节）。
/// 返回顺序：exact（词频降序、词、键长）+ rest（同）。
pub(crate) fn cap_hits(hits: Vec<(String, usize, u32)>, plen: usize) -> Vec<(String, usize, u32)> {
    let (mut exact, mut rest): (Vec<_>, Vec<_>) = hits.into_iter().partition(|h| h.1 == plen);
    let key = |a: &(String, usize, u32), b: &(String, usize, u32)| {
        b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1))
    };
    // 只排「留下的」：先在 O(N) 内选出各自 top-CAP（键是全序，与最终排序键同一个），
    // 再只对这 ≤CAP 条排序。结果与「全量排序后 truncate」逐条一致（2026-10-23 实测
    // 20,939 命中：全量排序 8.9ms → 选择 2.6ms；同进程 A/B 输出逐条相等）。
    for part in [&mut exact, &mut rest] {
        if part.len() > IMPORT_HIT_CAP {
            part.select_nth_unstable_by(IMPORT_HIT_CAP, key);
            part.truncate(IMPORT_HIT_CAP);
        }
        part.sort_by(key);
    }
    exact.extend(rest);
    exact
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 「全量排序后截断」= 选择实现要复现的参照语义（两者逐条同序）。
    fn reference(mut hits: Vec<(String, usize, u32)>, plen: usize) -> Vec<(String, usize, u32)> {
        let key = |a: &(String, usize, u32), b: &(String, usize, u32)| {
            b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1))
        };
        let (mut exact, mut rest): (Vec<_>, Vec<_>) = hits.drain(..).partition(|h| h.1 == plen);
        exact.sort_by(key);
        exact.truncate(IMPORT_HIT_CAP);
        rest.sort_by(key);
        rest.truncate(IMPORT_HIT_CAP);
        exact.extend(rest);
        exact
    }

    #[test]
    fn cap_hits_selects_the_same_top_cap_as_full_sort() {
        let plen = 2;
        let mut hits = Vec::new();
        // 两类都超过上限，词频大面积并列（比较器必须退到词序/键长才有全序）。
        for i in 0..(IMPORT_HIT_CAP * 2 + 37) {
            let freq = (i as u32 / 7) % 5000;
            hits.push((format!("w{i:05}"), plen, freq));
            hits.push((format!("x{i:05}"), plen + 1, freq));
        }
        let want = reference(hits.clone(), plen);
        let got = cap_hits(hits, plen);
        assert_eq!(got, want);
        assert_eq!(got.iter().filter(|h| h.1 == plen).count(), IMPORT_HIT_CAP);
        assert_eq!(got.iter().filter(|h| h.1 != plen).count(), IMPORT_HIT_CAP);
    }

    #[test]
    fn cap_hits_leaves_small_inputs_fully_sorted() {
        let hits = vec![
            ("b".to_string(), 2, 10),
            ("a".to_string(), 2, 10),
            ("c".to_string(), 3, 99),
        ];
        assert_eq!(
            cap_hits(hits, 2),
            vec![
                ("a".to_string(), 2, 10),
                ("b".to_string(), 2, 10),
                ("c".to_string(), 3, 99),
            ]
        );
    }
}
