// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **声母缩写整串匹配**（2026-10-22，用户「所有端输入增加匹配成语、歇后语、短语、俚语」）。
//!
//! 与 `jianpin.rs` 的分工：那边是「单字母 → 候选音节网格 → 拼出全拼键再查库」的
//! **猜测**，每位置只留 top-N 个音节（cap4 时成语几乎到不了：`yffs` 需要 `yuan/fan/
//! feng/shun`，四个都不在各自字母的前 4）；本模块反过来，沿**词库已有的键**扫描，
//! 一位一位对声母 —— 只要那条词真在库里且首字母序列对得上，就一定能找到。
//!
//! ## 触发面（由调用方 `candidates.rs` 保证）
//!
//! `opts.jianpin`（与简拼同一个开关）+ **主查询前缀为空**。后者是数据自证：
//! `zh` / `sh` / `ni` 这类能当前缀打的输入主查询非空 ⇒ 本档不跑，既有前缀/分档
//! 行为一字不动；`yffs` / `hstz` / `nh` 这类键库里没有 ⇒ 主查询空，本档接手。
//! 单字母输入也不跑（`merge` 要求 ≥2 个字母），与简拼「最贵的一击键不落在首键」同源。
//!
//! ## 匹配语义
//!
//! 键按音节切分后，逐个声母比对输入字母：
//! - 键的音节数 == 输入字母数 ⇒ **精确**（吃 `exact_bonus`，排在前）；
//! - 键还有后续音节 ⇒ **前缀**（`zmkhjjg` 命中整条歇后语 `芝麻开花——节节高；节节上升`；
//!   歇后语的常见形式因此不必拆成两行数据）。
//! - **两字母输入只认精确**（恰好两个音节）：更多音节的短语请打更多首字母。`nh`
//!   的三音节前缀在真库上万条，扫不完也不是两字母的意图（`zg` 的两个音节仍有
//!   约 2500 个精确候选 —— 由 `HIT_CAP` 按最终分截到 512）。
//!
//! 切分用位图 DP，不做贪婪：`fangan` 既是 `fang+an` 也是 `fan+gan`，贪婪会随词条
//! 意外选错首字母。位图固定 4×u64（.opid 拼音键 ≤255 字节，格式校验保证）。
//!
//! ## 成本
//!
//! 扫描面按**字面前两位**收窄：任何匹配都必须以某个首音节 `s1` 开头，且第二音节
//! 的首字节就是输入第 2 个字母 `c2` ⇒ 键一定以 `s1c2` 开头。对首字母的每个候选
//! 音节各做一次 `visit_prefix`（真库 `z` 的整段约 8 万条，收窄后只剩几千条）；
//! 音节间有前缀关系时（`yi`/`yin` + `n`）短前缀已覆盖长前缀，跳过长的避免重扫。
//! 两字母再进一步：枚举 `per[0]×per[1]` 的音节对查 `s1s2`、只取等长键（不产生
//! 前缀档）。命中条目才物化 `Entry`（精确/前缀各封顶 `HIT_CAP` 条，按含学习加成
//! 的最终分留）。这是本档能在全键盘每次击键上跑的前提 —— 全表 93 万条逐条
//! `segment()` 的实测是 60ms 量级（见 `dictionary.rs` 的 `scheme_entries` 注）。

use crate::candidates::{Candidate, CandidateKind, rank_score};
use crate::candidates_tiers_impl::Meta;
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::pinyin::{SYLLABLES, is_syllable};
use crate::trie::Entry;

/// 拼音键最长 255 字节（`engine-data::format` 校验）⇒ 256 个位置位。
const WORDS: usize = 4;
/// 精确档与前缀档**各自**最多留多少条：按最终分（含学习加成）取最高的。
/// 2 字母输入在真库能精确命中几千条（`zg` 约 2500 个 z+g 双音节词），全量交给
/// 候选装配实测 37ms（2026-10-22）；512 条已远超任何宿主的分页深度。
const HIT_CAP: usize = 512;
/// 单条键最长（字节）；超出的不匹配（格式已拒，这里只防替身词库）。
const MAX_KEY: usize = WORDS * 64 - 1;

#[derive(Debug)]
pub(crate) struct Hit {
    pub entry: Entry,
    pub exact: bool,
    /// 最终分（`rank_score`，含学习者加成）—— `merge` 直接加精确加成，不重算。
    pub score: u64,
    pub learned: bool,
}

/// 输入是否为「纯字母缩写串」（去掉手写切分符 `'` 后 ≥2 个 ASCII 字母）。
/// `v` 在普通话里不作声母 —— 含它的串在 `hits` 里直接无解，不必进扫描。
fn letters_of(input: &str) -> Option<String> {
    let s: String = input
        .chars()
        .filter(|c| *c != '\'')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (s.len() >= 2 && s.bytes().all(|b| b.is_ascii_lowercase())).then_some(s)
}

/// 主查询前缀为空时，按声母缩写查整串命中（调用方负责开关与「主查询为空」判据）。
///
/// 两类档各有 `HIT_CAP` 条上限，堆里只克隆留下的条目；排序键是**最终分**
/// （`rank_score`，含学习者加成），所以被截掉的只会是排在 512 名之后的。
pub(crate) fn hits<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    input: &str,
) -> Vec<Hit> {
    let Some(letters) = letters_of(input) else {
        return Vec::new();
    };
    // 每个字母可开头的音节表；任一字母没有音节 ⇒ 整串不可能命中（如 `v`）。
    let mut per: Vec<Vec<&'static str>> = Vec::with_capacity(letters.len());
    for b in letters.bytes() {
        let v: Vec<&'static str> = SYLLABLES
            .iter()
            .copied()
            .filter(|s| s.as_bytes()[0] == b)
            .collect();
        if v.is_empty() {
            return Vec::new();
        }
        per.push(v);
    }
    let mut exact: std::collections::BinaryHeap<Ranked> = std::collections::BinaryHeap::new();
    let mut prefix: std::collections::BinaryHeap<Ranked> = std::collections::BinaryHeap::new();
    if letters.len() == 2 {
        // **两字母 = 恰好两个音节**：只认整串键。两字母输入的三音节以上前缀在真库有
        // 上万条（`nh`），既扫不完也不是用户意图 —— 长短语请打更多首字母。
        // 逐对枚举音节再查该二字前缀，比扫「首音节 + 第二字母」的整段小一个量级。
        for s1 in &per[0] {
            for s2 in &per[1] {
                let mut start = String::with_capacity(s1.len() + s2.len());
                start.push_str(s1);
                start.push_str(s2);
                dict.visit_prefix(&start, &mut |py, word, freq| {
                    if py.len() != start.len() {
                        return; // 更长的键属于「更多音节」，两字母档不接
                    }
                    let user = learner.freq_of(word);
                    let score = rank_score(freq, user, boost);
                    push_ranked(&mut exact, score, freq, user, py.len(), word);
                });
            }
        }
    } else {
        let c2 = letters.as_bytes()[1] as char;
        // 完备性：匹配的首音节必在 `per[0]`，且其后紧接的第 2 音节首字节 = `c2`。
        // 因此只需扫 `s1 + c2` 这些字面前缀；`s1` 之间互为前缀时短者覆盖长者（如
        // `yin` 覆盖 `yinn`），跳过长者避免重叠区间重复扫描。
        let mut starts: Vec<String> = per[0]
            .iter()
            .map(|s| {
                let mut p = String::with_capacity(s.len() + 1);
                p.push_str(s);
                p.push(c2);
                p
            })
            .collect();
        starts.sort();
        let mut keep: Vec<String> = Vec::with_capacity(starts.len());
        for p in starts {
            if !keep.iter().any(|k| p.starts_with(k.as_str())) {
                keep.push(p);
            }
        }
        for start in &keep {
            dict.visit_prefix(start, &mut |py, word, freq| {
                if py.len() < letters.len() || py.len() > MAX_KEY {
                    return;
                }
                let Some(is_exact) = match_key(py, &per) else {
                    return;
                };
                let user = learner.freq_of(word);
                let score = rank_score(freq, user, boost);
                let heap = if is_exact { &mut exact } else { &mut prefix };
                push_ranked(heap, score, freq, user, py.len(), word);
            });
        }
    }
    let mut out = drained(exact, true);
    out.extend(drained(prefix, false));
    out
}

/// 堆 → 结果：`Ord` 是「分高在前、同分按词升序」，`into_sorted_vec` 出来的正是这个序。
fn drained(heap: std::collections::BinaryHeap<Ranked>, exact: bool) -> Vec<Hit> {
    heap.into_sorted_vec()
        .into_iter()
        .map(|r| Hit {
            entry: Entry {
                word: r.word,
                freq: r.freq,
                pinyin_len: r.pinyin_len,
            },
            exact,
            score: r.score,
            learned: r.learned,
        })
        .collect()
}

/// 有界选择：堆顶 = **最差**（分低；同分时 word 大 —— 与最终排序的升序词序相反），
/// 新条目更好才替换。
struct Ranked {
    score: u64,
    freq: u32,
    pinyin_len: usize,
    word: String,
    learned: bool,
}

impl Ord for Ranked {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.score
            .cmp(&self.score)
            .then_with(|| self.word.cmp(&o.word))
    }
}

impl PartialOrd for Ranked {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl PartialEq for Ranked {
    fn eq(&self, o: &Self) -> bool {
        self.score == o.score && self.freq == o.freq && self.word == o.word
    }
}

impl Eq for Ranked {}

fn push_ranked(
    heap: &mut std::collections::BinaryHeap<Ranked>,
    score: u64,
    freq: u32,
    user: u32,
    pinyin_len: usize,
    word: &str,
) {
    let better = |worst: &Ranked| {
        score > worst.score || (score == worst.score && word < worst.word.as_str())
    };
    if heap.len() < HIT_CAP {
        heap.push(Ranked {
            score,
            freq,
            pinyin_len,
            word: word.to_string(),
            learned: user > 0,
        });
        return;
    }
    let worst = heap.peek().expect("len == HIT_CAP > 0");
    if better(worst) {
        heap.pop();
        heap.push(Ranked {
            score,
            freq,
            pinyin_len,
            word: word.to_string(),
            learned: user > 0,
        });
    }
}

/// 把缩写命中并进主块（`merged` 为空时才动手 —— 与简拼的晋升规则同一条）。
/// 返回「是否产出候选」：调用方据此跳过逐音节汤/分词回退（`addjsbs` 首段 `a`
/// 是合法音节，否则整档会被 `a` 的 3767 条单字接管）。
///
/// 元数据把命中放进「词组」档：`pinyin_len` 统一取输入长（同档内保持分数序），
/// `exact` 只影响精确档的加成与语义；学过的词照旧进第 0 档。
/// 精确档加成取 `boost / 2`（`candidates.rs` 的唯一定义 —— 精确等长匹配加成）；
/// 不另收参数，避免两个分档常量漂移。
pub(crate) fn merge<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    input: &str,
    merged: &mut Vec<(Candidate, Option<Meta>)>,
    enabled: bool,
) -> bool {
    if !enabled || !merged.is_empty() {
        return false;
    }
    let hits = hits(dict, learner, boost, input);
    if hits.is_empty() {
        return false;
    }
    for h in hits {
        merged.push((
            Candidate {
                text: h.entry.word,
                kind: CandidateKind::Hanzi,
                score: h.score.saturating_add(if h.exact { boost / 2 } else { 0 }),
            },
            Some(Meta {
                pinyin_len: input.len(),
                exact: h.exact,
                learned: h.learned,
                tail: false,
            }),
        ));
    }
    true
}

/// 键的声母序列是否以输入字母序列开头；`Some(true)` = 恰好整串，`Some(false)` = 前缀。
fn match_key(key: &str, per: &[Vec<&'static str>]) -> Option<bool> {
    let kb = key.as_bytes();
    let mut cur = [0u64; WORDS];
    set(&mut cur, 0);
    for sylls in per {
        let mut next = [0u64; WORDS];
        let mut any = false;
        for (wi, &word) in cur.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let pos = wi * 64 + b;
                if pos > kb.len() {
                    continue;
                }
                for s in sylls {
                    let sb = s.as_bytes();
                    if kb.len() - pos >= sb.len() && &kb[pos..pos + sb.len()] == sb {
                        set(&mut next, pos + sb.len());
                        any = true;
                    }
                }
            }
        }
        if !any {
            return None;
        }
        cur = next;
    }
    if get(&cur, kb.len()) {
        return Some(true);
    }
    // 前缀：输入已吃完，键还有后续音节；剩余部分必须能整段切成音节（防替身词库的垃圾键）。
    for (wi, &word) in cur.iter().enumerate() {
        let mut bits = word;
        while bits != 0 {
            let b = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            let pos = wi * 64 + b;
            if pos < kb.len() && all_syllables(&kb[pos..]) {
                return Some(false);
            }
        }
    }
    None
}

/// 字节串能否完整切成合法音节（DP；`fangan` 两种切法都算成功）。
fn all_syllables(s: &[u8]) -> bool {
    if s.is_empty() {
        return true;
    }
    let mut cur = [0u64; WORDS];
    set(&mut cur, 0);
    for pos in 0..s.len() {
        if !get(&cur, pos) {
            continue;
        }
        for len in 1..=6.min(s.len() - pos) {
            let Ok(syl) = std::str::from_utf8(&s[pos..pos + len]) else {
                continue;
            };
            if is_syllable(syl) {
                set(&mut cur, pos + len);
            }
        }
    }
    get(&cur, s.len())
}

fn set(bits: &mut [u64; WORDS], pos: usize) {
    if pos < WORDS * 64 {
        bits[pos / 64] |= 1u64 << (pos % 64);
    }
}

fn get(bits: &[u64; WORDS], pos: usize) -> bool {
    pos < WORDS * 64 && bits[pos / 64] & (1u64 << (pos % 64)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::InMemoryDictionary;

    fn per(input: &str) -> Vec<Vec<&'static str>> {
        input
            .bytes()
            .map(|b| {
                SYLLABLES
                    .iter()
                    .copied()
                    .filter(|s| s.as_bytes()[0] == b)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn match_is_not_greedy_about_syllable_boundaries() {
        // fangan = fang+an（方案）或 fan+gan（反感）：两种声母序列都必须认。
        assert_eq!(match_key("fangan", &per("fa")), Some(true));
        assert_eq!(match_key("fangan", &per("fg")), Some(true));
        assert_eq!(match_key("fangan", &per("fx")), None);
    }

    #[test]
    fn longer_key_matches_as_prefix_only() {
        let key = "zhimakaihuajiejiegaojiejieshangsheng";
        assert_eq!(match_key(key, &per("zmkhjjg")), Some(false));
        assert_eq!(match_key(key, &per("zmkhjjgjjss")), Some(true));
        assert_eq!(match_key(key, &per("zmkhjjgjjxx")), None);
    }

    fn learner() -> Learner {
        Learner::new(false)
    }

    #[test]
    fn hits_prefers_exact_and_rejects_garbage_keys() {
        let mut d = InMemoryDictionary::new();
        d.insert("nihaohao", "你好号", 9_000);
        d.insert("nihaohaoma", "你好号吗", 8_000);
        d.insert("ni", "你", 9_500);
        d.insert("qzx", "垃圾键", 9_999); // 不是音节序列
        let got = hits(&d, &learner(), 0, "nhh");
        let words: Vec<&str> = got.iter().map(|h| h.entry.word.as_str()).collect();
        assert_eq!(words, ["你好号", "你好号吗"]);
        assert!(got[0].exact && !got[1].exact);
        assert!(got[0].score >= got[1].score, "分数序：{got:?}");
    }

    /// 两字母 = 两个音节：更长的键不进（`nh` 的三音节前缀在真库上万条，
    /// 既不可能是两字母用户的意图，也是这条路径的成本闸门）。
    #[test]
    fn two_letter_input_is_exact_two_syllable_only() {
        let mut d = InMemoryDictionary::new();
        d.insert("nihao", "你好", 9_000);
        d.insert("nihaoa", "你好啊", 8_000);
        let got = hits(&d, &learner(), 0, "nh");
        let words: Vec<&str> = got.iter().map(|h| h.entry.word.as_str()).collect();
        assert_eq!(words, ["你好"]);
        assert!(got[0].exact);
    }

    #[test]
    fn letters_guard() {
        assert!(letters_of("Nh").is_some_and(|s| s == "nh"));
        assert!(letters_of("n'h").is_some_and(|s| s == "nh"));
        assert!(letters_of("n").is_none());
        assert!(letters_of("n1").is_none());
        assert!(hits(&InMemoryDictionary::new(), &learner(), 0, "nh").is_empty());
    }
}
