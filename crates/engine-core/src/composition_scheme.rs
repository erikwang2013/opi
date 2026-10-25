// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **9 键多字词组合识别**（2026-10-23）—— `composition.rs` 的数字空间镜像。
//!
//! 全键盘那条把**拼音字母串**切成词库精确键；9 键的缓冲是**按键串**
//! （`926248694364`），一个键覆盖 3–4 个字母，切法不能只看缓冲 —— 只能问词库：
//! 「这段按键有没有**恰好等长**的键？」（[`Dictionary::scheme_entries`] 的 n = 0 只回
//! 精确条目）。每段条目带自己的拼音读数（9 键拼音行要用），因此段内音节数、真实
//! 读音都按**条目**算，不按按键串算。
//!
//! ## 判据（与 `composition.rs` 逐条对齐）
//!
//! 1. 输入是纯 9 键按键（`2`–`9`）、≤ `MAX_BUFFER`；
//! 2. 整串切成 2..=`MAX_CHUNKS` 段、每段 ≥2 键、每段都是词库精确键（等长）；
//! 3. 每段条目的拼音能整切成音节、≤ `MAX_CHUNK_SYLLABLES` 个；整句**至少一个
//!    多音节条目**、总音节 ≥3 —— 否则就是「随便几个同音字凑一串」，与全键盘同一条；
//! 4. 段数从少到多、同段数首段更长优先、同切法按各段词频和降序；第一档有结果即停；
//! 5. 用户点名的新词复用 `composition::PINNED`（拼音键按 9 键编码对表）：只在算法
//!    已经产出时置顶，不造词；
//! 6. 查询预算 [`MAX_QUERIES`]：每次调用最多这么多次 `scheme_entries`，超了整档放弃
//!    （回既有回退语义）—— 短按键串的精确条目很多（真库 `926` 有 1177 条），预算
//!    既是延迟上限，也是「垃圾长串不做无谓枚举」的闸门。
//!
//! ## 成本（2026-10-23 luna 实测，桌面 release）
//!
//! 单次 `scheme_entries(段, Nine, None, 0)` 亚毫秒级（段越短条目越多，`926` 约
//! 0.4ms）；两个报障串的 2 段枚举各 18 次查询。整档引擎耗时见交付说明 —— 上限
//! 由 [`MAX_QUERIES`] 钉住，不随输入长度增长。

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::candidates::rank_score;
use crate::composer::MAX_BUFFER;
use crate::composition::{MAX_CHUNK_SYLLABLES, MAX_CHUNKS, MAX_RESULTS, PINNED, min_syllables};
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::scheme::Scheme;

/// 一次调用最多几次词库查询（见头注判据 6）。
pub(crate) const MAX_QUERIES: usize = 24;

/// 组合候选：文本 + 各段拼音的拼接读数（长度 == 输入键数）+ 最终分（含精确加成）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hit {
    pub text: String,
    pub pinyin: String,
    pub score: u64,
}

/// 一个段（词库键）的候选条目，按分降序。
#[derive(Debug)]
struct Chunk {
    entries: Vec<EntryHit>,
    /// 段在按键串里的字节长（排序签名用）。
    bytes: usize,
}

#[derive(Debug, Clone)]
struct EntryHit {
    text: String,
    /// 条目自己的拼音读数（长度 == 段长）—— 整句读音由各段拼接而成。
    pinyin: String,
    syllables: usize,
    score: u64,
}

/// 查询缓存 + 预算（同一段只查一次）。
///
/// 缓存键 = **段内容**而不是 (start, end)：长串里重复的片段（连按同一键、
/// 重复音节）只查一次。这是 2026-10-26 的延迟修复 —— 16 个相同按键原先会
/// 对每个位置各查一遍，24 次预算全花在同一段内容上。
struct Cache<'a, D: Dictionary + ?Sized> {
    dict: &'a D,
    learner: &'a Learner,
    boost: u64,
    map: HashMap<&'a str, Option<Rc<Chunk>>>,
    queries: usize,
    /// 预算用尽：整档放弃（见头注判据 6）。
    exhausted: bool,
}

/// 组合中间态：(分数, 文本, 拼音读数, 已含多音节条目, 总音节)。
type Partial = (u64, String, String, bool, usize);

/// 组合候选（按最终次序）。输入不成立任何判据 ⇒ 空。
pub(crate) fn candidates<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    scheme: Scheme,
    input: &str,
) -> Vec<Hit> {
    // 本版只给 9 键（14 键/全键盘的缓冲不是数字串，见头注）；14 键要开得另立裁决。
    if scheme != Scheme::Nine
        || input.is_empty()
        || input.len() > MAX_BUFFER
        || !input.chars().all(|c| scheme.key_class(c).is_some())
    {
        return Vec::new();
    }
    let n = input.len();
    let mut cache = Cache {
        dict,
        learner,
        boost,
        map: HashMap::new(),
        queries: 0,
        exhausted: false,
    };
    // 段数从少到多：第一档有结果即停（更碎的组合不参与排序）。
    for chunk_count in 2..=MAX_CHUNKS {
        // 每段至少 2 键；段数再涨只会更不够切。
        if n < 2 * chunk_count {
            break;
        }
        // 同段数内：首段更长的切法在前（`yanchun+zheng` 先于 `yan+chunzheng`）；
        // 同切法内按分。同分按文本升序 —— 与候选层其它地方同一条确定性规则。
        //
        // 枚举次序 = 各段长度向量（排序签名）降序，**凑够 `MAX_RESULTS` 即停**：首选
        // 切法通常就有 ≥16 条组合，此时结果与「全枚举后排序截断」逐条相同；只有首选
        // 切法不足 16 条才会少几条尾部（已返回的次序一字不变）。省下的是尾部切点的
        // 词库查询 —— 2 段枚举从 2(n−3) 次降到命中切点为止（真库 12 键：18 → 8）。
        let mut all: Vec<(Vec<usize>, u64, String, String)> = Vec::new();
        let mut path = Vec::new();
        walk(input, &mut cache, 0, chunk_count, &mut path, &mut |path| {
            let sig: Vec<usize> = path.iter().map(|c| c.bytes).collect();
            for (score, text, pinyin) in combinations(path) {
                all.push((sig.clone(), score, text, pinyin));
            }
            all.len() < MAX_RESULTS
        });
        if cache.exhausted {
            return Vec::new();
        }
        if all.is_empty() {
            continue;
        }
        all.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        let mut seen = HashSet::new();
        all.retain(|(_, _, text, _)| seen.insert(text.clone()));
        // 点名的新词若已在算法结果里，提到首位（分数抬到当前最高之上）；不在就不造
        // —— 边界见 `composition::PINNED`。
        if let Some((_, word)) = PINNED
            .iter()
            .find(|(py, _)| t9(py).as_deref() == Some(input))
            && let Some(pos) = all.iter().position(|(_, _, text, _)| text == word)
        {
            let top = all.iter().map(|(_, score, _, _)| *score).max().unwrap_or(0);
            let mut hit = all.remove(pos);
            hit.1 = top.saturating_add(1);
            all.insert(0, hit);
        }
        all.truncate(MAX_RESULTS);
        return all
            .into_iter()
            .map(|(_, score, text, pinyin)| Hit {
                text,
                pinyin,
                // 整串被精确键覆盖：与主查询精确档同一加成（唯一定义在 candidates.rs）。
                score: score.saturating_add(boost / 2),
            })
            .collect();
    }
    Vec::new()
}

/// 选中文本是否是一条覆盖**整个输入**的组合候选（收尾层用：整段消耗，不留后缀键）。
pub(crate) fn covers<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    scheme: Scheme,
    input: &str,
    selected: &str,
) -> bool {
    candidates(dict, learner, boost, scheme, input)
        .iter()
        .any(|h| h.text == selected)
}

/// 拼音字母串 → 9 键按键串（只用于把 `composition::PINNED` 的拼音键对到数字输入）。
fn t9(letters: &str) -> Option<String> {
    letters
        .chars()
        .map(|c| ('2'..='9').find(|k| Scheme::Nine.key_class(*k).is_some_and(|cs| cs.contains(&c))))
        .collect()
}

/// 枚举「`chunk_count` 段、整串全覆盖」的切法；每段 ≥2 键。每凑出一条切法就回调
/// `visit`，回调返回 `false` 立即停止整个枚举（调用方的「凑够即停」）。
/// 段长从长到短枚举 ⇒ 回调次序 = 排序签名降序（与组装时的 sort 键一致）。
fn walk<'a, D: Dictionary + ?Sized>(
    s: &'a str,
    cache: &mut Cache<'a, D>,
    pos: usize,
    chunks_left: usize,
    path: &mut Vec<Rc<Chunk>>,
    visit: &mut dyn FnMut(&[Rc<Chunk>]) -> bool,
) -> bool {
    let last = s.len();
    if chunks_left == 1 {
        let Some(c) = chunk(s, pos, last, cache) else {
            return true;
        };
        path.push(c);
        let go_on = visit(path);
        path.pop();
        return go_on;
    }
    // 剩下的每一段至少要跨 2 键（调用方已保证 `last >= 2 * chunks_left`）。
    let max_end = last - 2 * (chunks_left - 1);
    for end in ((pos + 2)..=max_end).rev() {
        let Some(c) = chunk(s, pos, end, cache) else {
            continue;
        };
        path.push(c);
        let go_on = walk(s, cache, end, chunks_left - 1, path, visit);
        path.pop();
        if !go_on {
            return false;
        }
    }
    true
}

/// 取一段（`s[start..end]`）的精确词库条目；不合格/无命中 ⇒ `None`（带缓存与预算）。
fn chunk<'a, D: Dictionary + ?Sized>(
    s: &'a str,
    start: usize,
    end: usize,
    cache: &mut Cache<'a, D>,
) -> Option<Rc<Chunk>> {
    let sub = &s[start..end];
    if let Some(cached) = cache.map.get(sub) {
        return cached.clone();
    }
    let built = if end - start < 2 {
        None
    } else if cache.queries >= MAX_QUERIES {
        cache.exhausted = true;
        None
    } else {
        cache.queries += 1;
        let mut entries: Vec<EntryHit> = cache
            .dict
            .scheme_entries(sub, Scheme::Nine, None, 0)
            .into_iter()
            // n = 0 已只返回精确条目；再按长度钉一次（替身词库也守同一条）。
            .filter(|(e, _)| e.pinyin_len == sub.len())
            .filter_map(|(e, pinyin)| {
                // 条目自己的读音决定段内音节数（按键串本身不是拼音）。
                let syllables = min_syllables(&pinyin)?;
                (syllables <= MAX_CHUNK_SYLLABLES).then(|| EntryHit {
                    score: rank_score(e.freq, cache.learner.freq_of(&e.word), cache.boost),
                    text: e.word,
                    pinyin,
                    syllables,
                })
            })
            .collect();
        if entries.is_empty() {
            None
        } else {
            entries.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
            entries.truncate(MAX_RESULTS);
            Some(Rc::new(Chunk {
                entries,
                bytes: sub.len(),
            }))
        }
    };
    cache.map.insert(sub, built.clone());
    built
}

/// 一段切法内各段条目的笛卡尔积。中间层按分数截到 `MAX_RESULTS`（前 K 和只用到各段
/// 前 K 条，无损失）；**最后一层不截**，交给调用方先做 `PINNED` 提升再截。
///
/// 为什么最后一层不能截：数字空间里**一个段有多个读音**（`9262486` 同时是
/// `yanchun`/`zaochun`/`wanchun`…），高频读音会把低频读音的整片组合挤出前 16
/// （真库实测：全是「早春 + 正/政/争…」，「厌蠢 + 症」排不进来）。全键盘那条一个段
/// 只有一种读音，所以截断是无损的；这里不是。末层规模仍有界（前层 ≤16 × 本段 ≤16）。
/// 末尾按「至少一个多音节条目、总音节 ≥3」过滤。
fn combinations(chunks: &[Rc<Chunk>]) -> Vec<(u64, String, String)> {
    let mut partial: Vec<Partial> = vec![(0, String::new(), String::new(), false, 0)];
    for (i, c) in chunks.iter().enumerate() {
        let mut next = Vec::with_capacity(partial.len() * c.entries.len());
        for (score, text, pinyin, multi, syllables) in &partial {
            for e in &c.entries {
                let mut joined = String::with_capacity(text.len() + e.text.len());
                joined.push_str(text);
                joined.push_str(&e.text);
                let mut joined_py = String::with_capacity(pinyin.len() + e.pinyin.len());
                joined_py.push_str(pinyin);
                joined_py.push_str(&e.pinyin);
                next.push((
                    score.saturating_add(e.score),
                    joined,
                    joined_py,
                    *multi || e.syllables >= 2,
                    syllables + e.syllables,
                ));
            }
        }
        next.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        if i + 1 < chunks.len() {
            next.truncate(MAX_RESULTS);
        }
        partial = next;
    }
    partial
        .into_iter()
        .filter(|(_, _, _, multi, syllables)| *multi && *syllables >= 3)
        .map(|(score, text, pinyin, _, _)| (score, text, pinyin))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::InMemoryDictionary;
    use crate::trie::Entry;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn dict() -> InMemoryDictionary {
        let mut d = InMemoryDictionary::new();
        // 两个报障词：`厌蠢 + 症`、`样样 + 通`（整词不是键）。
        d.insert("yanchun", "厌蠢", 700);
        d.insert("zheng", "症", 850);
        d.insert("zheng", "正", 950);
        d.insert("yangyang", "样样", 700);
        d.insert("tong", "通", 850);
        d.insert("tong", "同", 950);
        // 负控制：只有单音节键（`ma`）。
        d.insert("ma", "妈", 900);
        // 负控制：只有前缀扩展键（`zhengren` 不是 `zheng`）。
        d.insert("zhengren", "症人", 990);
        d
    }

    fn hits(input: &str) -> Vec<Hit> {
        candidates(&dict(), &Learner::new(false), 1_800, Scheme::Nine, input)
    }

    #[test]
    fn user_reported_words_compose_in_digit_space() {
        let got = hits("926248694364"); // yanchunzheng
        assert_eq!(got[0].text, "厌蠢症", "PINNED 应置顶: {got:?}");
        assert_eq!(got[0].pinyin, "yanchunzheng", "整串拼音读数");
        assert!(got.iter().any(|h| h.text == "厌蠢症"));
        let got = hits("926492648664"); // yangyangtong
        assert_eq!(got[0].text, "样样通", "PINNED 应置顶: {got:?}");
        assert_eq!(got[0].pinyin, "yangyangtong");
    }

    /// 全是单音节段 ⇒ 不拼（与全键盘的负控制同一条判据）。
    #[test]
    fn all_single_syllable_chunks_are_not_composed() {
        // `ma` × 2：只有 2 个音节（总音节 ≥3 不成立）。
        assert!(hits("6262").is_empty(), "两个单音节段不该组合");
        // `ma` × 3：总音节够，但没有多音节条目。
        assert!(hits("626262").is_empty(), "单音节段拼接是噪音");
    }

    /// 段必须是**等长精确键**；只有以它开头的更长键不算。
    #[test]
    fn chunk_lookup_is_exact_not_prefix() {
        let mut d = InMemoryDictionary::new();
        d.insert("yanchun", "厌蠢", 700);
        d.insert("zhengren", "症人", 990); // 只有 `zheng` 的前缀扩展
        let got = candidates(
            &d,
            &Learner::new(false),
            1_800,
            Scheme::Nine,
            "926248694364",
        );
        assert!(
            !got.iter()
                .any(|h| h.text.starts_with("厌蠢症") || h.text.starts_with("厌蠢正")),
            "`94364` 不该拿 `zhengren` 当精确键: {got:?}"
        );
    }

    /// 只有 9 键走这条路；非法按键（`0`/`1`/字母/`'`）一律拒绝。
    #[test]
    fn only_nine_key_digits_are_accepted() {
        assert!(
            candidates(
                &dict(),
                &Learner::new(false),
                1_800,
                Scheme::Full,
                "926248694364"
            )
            .is_empty()
        );
        for input in ["926248694360", "92624869436a", "926'2486"] {
            assert!(hits(input).is_empty(), "非法输入 {input} 不该出组合");
        }
    }

    /// 同一输入两次结果逐条相同（排序/去重无隐藏状态）。
    #[test]
    fn results_are_deterministic() {
        assert_eq!(hits("926248694364"), hits("926248694364"));
    }

    /// 查询预算：长输入最多查 `MAX_QUERIES` 次，超了整档放弃（不返回半档结果）。
    #[test]
    fn query_budget_caps_every_call() {
        struct Counted {
            inner: InMemoryDictionary,
            calls: Arc<AtomicUsize>,
        }
        impl Dictionary for Counted {
            fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
                self.inner.query(pinyin, limit)
            }
            fn scheme_entries(
                &self,
                input: &str,
                scheme: Scheme,
                pin: Option<&str>,
                n: usize,
            ) -> Vec<(Entry, String)> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                crate::dictionary::scheme_entries_scan(&self.inner, input, scheme, pin, n)
            }
            fn len(&self) -> usize {
                self.inner.len()
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Counted {
            inner: dict(),
            calls: calls.clone(),
        };
        // 20 键：2 段枚举要 2×(20−3) = 34 次查询 ⇒ 撞预算，整档放弃。
        let got = candidates(
            &counted,
            &Learner::new(false),
            1_800,
            Scheme::Nine,
            "92624869436492624869",
        );
        assert!(got.is_empty(), "预算用尽应整档放弃: {got:?}");
        assert_eq!(
            calls.load(Ordering::Relaxed),
            MAX_QUERIES,
            "查询次数必须封顶"
        );
    }
}
