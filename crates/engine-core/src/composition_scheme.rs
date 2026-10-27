// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **9/14 键多字词组合识别**（2026-10-23；14 键 2026-10-27 开）——
//! `composition.rs` 的按键空间镜像。
//!
//! 全键盘那条把**拼音字母串**切成词库精确键；9/14 键的缓冲是**按键串**
//! （9 键 `926248694364`、14 键 `bu…`），一个键覆盖多个字母，切法不能只看缓冲 ——
//! 只能问词库：「这段按键有没有**恰好等长**的键？」（[`Dictionary::scheme_entries`]
//! 的 n = 0 只回精确条目）。每段条目带自己的拼音读数（拼音行要用），因此段内音节数、
//! 真实读音都按**条目**算，不按按键串算。
//!
//! ## 判据（与 `composition.rs` 逐条对齐）
//!
//! 1. 输入是纯按键串（9 键 `2`–`9`；14 键 `q e t u o / a d g j l / z c b m`）、
//!    ≤ `MAX_BUFFER`；
//! 2. 整串切成 2..=`MAX_CHUNKS` 段、每段 ≥2 键、每段都是词库精确键（等长）；
//! 3. 每段条目的拼音能整切成音节、≤ `MAX_CHUNK_SYLLABLES` 个；整句**至少一个
//!    多音节条目**、总音节 ≥3 —— 否则就是「随便几个同音字凑一串」，与全键盘同一条；
//! 4. 段数从少到多，第一档有结果即停；同段数内按**各段词频的平均分**降序
//!    （同分再按「首段更长」、再按文本，保持确定性）；
//! 5. 用户点名的新词复用 `composition::PINNED`（拼音键按当前方案编码对表）：只在
//!    算法已经产出时置顶，不造词；
//! 6. 查询预算 [`MAX_QUERIES`]：每次调用最多这么多次 `scheme_entries`，超了**停止
//!    枚举**；已经凑出的完整切法保留（一条都没有才整档放弃，回既有回退语义）——
//!    短按键串的精确条目很多（真库 `926` 有 1177 条），预算既是延迟上限，也是
//!    「垃圾长串不做无谓枚举」的闸门。
//!
//! ## 成本（2026-10-23 luna 实测，桌面 release）
//!
//! 单次 `scheme_entries(段, scheme, None, 0)` 亚毫秒级（段越短条目越多，`926` 约
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
///
/// 2026-10-27：24 → 64 → 128。9/14 键要枚举**所有**切法（旧「凑够 16 条即停」会
/// 漏掉用户本意的切法）。2 段全枚举要 `2(n−3)` 次（`MAX_BUFFER` 32 键 ⇒ 58），
/// 3 段在 19 键的整句上还要几十次；64 会在找到本意前耗尽
/// （`tashiwodehaopengyou` 退化成单字）。128 仍是有界预算，且只在长串上才会用满。
pub(crate) const MAX_QUERIES: usize = 128;

/// **非末段**最多跨几个键（2026-10-27）。末段不限。
///
/// 词库词按拼音 12 键已覆盖到 4 字词（`yiluxiangbei` = 12）。更长的非末段放进
/// 长度枚举会让切法数爆炸（19 键：17 种首段长度，每种再 × 次段枚举），预算全花在
/// 长前缀上，本意（`tashi` = 5 键）还没轮到就耗尽。封顶后长句由 3 段拼出
/// （`中华 + 人民 + 共和国`），末段不限。
const MAX_CHUNK_KEYS: usize = 12;

/// 组合候选：文本 + 各段拼音的拼接读数（长度 == 输入键数）+ 最终分
/// （**各段词频的平均值**，与单词同一量纲；见 `candidates` 的返回值）。
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
    /// 本次调用的方案（9/14 键）：段查询按它展开按键类（2026-10-27 起 14 键也走本档）。
    scheme: Scheme,
    map: HashMap<&'a str, Option<Rc<Chunk>>>,
    queries: usize,
    /// 预算用尽：停止枚举；已凑出的完整切法保留，一条都没有才整档放弃（判据 6）。
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
    // 9 键（数字串）与 14 键（两字母键的规范符号，如 `bu` = n+i）同一条：两者的
    // 缓冲都是**按键符号**，一个键覆盖多个字母，切法只能问词库。14 键不是数字串，
    // 但 `key_class` 的类展开与「等长精确键」判据逐字同义（2026-10-27 开）。
    if !matches!(scheme, Scheme::Nine | Scheme::Fourteen)
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
        scheme,
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
        // **按分排序**（2026-10-27 改）：9/14 键一个按键串有多种读音/切法，
        // 「首段更长」只对全键盘的确定性拼音成立；对按键串它会稳定选出更长的
        // **错误**切法（`96242345464` →「我查 + 地精」压过「我爱 + 北京」）。
        // 同分时仍用长度签名（首段更长）再按文本，保持确定性。
        //
        // **枚举所有切法**（不再「凑够 16 条即停」）：`combinations` 的末层不截，
        // 一次切法就可能吐满 16+ 条，旧停止条件在第一档切法就触发，后面的切法
        // （往往才是用户本意）根本没被查询。查询次数仍由 [`MAX_QUERIES`] 封顶。
        let mut all: Vec<(Vec<usize>, u64, String, String)> = Vec::new();
        let mut path = Vec::new();
        walk(input, &mut cache, 0, chunk_count, &mut path, &mut |path| {
            let sig: Vec<usize> = path.iter().map(|c| c.bytes).collect();
            for (score, text, pinyin) in combinations(path) {
                all.push((sig.clone(), score, text, pinyin));
            }
            true
        });
        // 预算用尽：**保留已经凑出的完整切法**（若有），没有才整档放弃。旧实现
        // 无条件返回空 —— 长整句上枚举到一半就被判死，用户看到的是首字回退
        // （`tashiwodehaopengyou` → 只有「他」）。
        if cache.exhausted && all.is_empty() {
            return Vec::new();
        }
        if all.is_empty() {
            continue;
        }
        all.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)).then(a.2.cmp(&b.2)));
        let mut seen = HashSet::new();
        all.retain(|(_, _, text, _)| seen.insert(text.clone()));
        // 点名的新词若已在算法结果里，提到首位（分数抬到当前最高之上）；不在就不造
        // —— 边界见 `composition::PINNED`。
        if let Some((_, word)) = PINNED
            .iter()
            .find(|(py, _)| encode(scheme, py).as_deref() == Some(input))
            && let Some(pos) = all.iter().position(|(_, _, text, _)| text == word)
        {
            let top = all.iter().map(|(_, score, _, _)| *score).max().unwrap_or(0);
            let mut hit = all.remove(pos);
            // 分数在下面按段数取平均（`/ chunk_count`）；加一个**段数**而不是 1，
            // 除完才严格高于当前最高平均分（加 1 会被整除吞掉）。
            hit.1 = top.saturating_add(chunk_count as u64);
            all.insert(0, hit);
        }
        all.truncate(MAX_RESULTS);
        return all
            .into_iter()
            .map(|(_, score, text, pinyin)| Hit {
                text,
                pinyin,
                // **段平均分**，不是和（2026-10-27）：词频是「每个词」的量纲，两段相加
                // 会天然压过任何单个词库词（实测 `yiluxiangbei`：「一句 + 长辈」把
                // 词库里的「一路向北」顶出第一页）。除以段数后与单词同一量纲。
                // 也**不加 exact 加成**：组合是推断，词库里的整词才是直命中；两者
                // 同键长时由词频裁决（`Meta.exact` 仍为真 ⇒ 仍排在拼音延伸词之前）。
                score: score / chunk_count as u64,
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

/// 拼音字母串 → 按键串（只用于把 `composition::PINNED` 的拼音键对到 9/14 键输入）。
/// 每个字母取**类里包含它的那个键**；该类表与 `scheme.rs` 同一份，不另抄。
fn encode(scheme: Scheme, letters: &str) -> Option<String> {
    letters.chars().map(|c| scheme.key_for_letter(c)).collect()
}

/// 枚举「`chunk_count` 段、整串全覆盖」的切法；每段 ≥2 键（非末段 ≤
/// [`MAX_CHUNK_KEYS`]）。每凑出一条切法就回调 `visit`，回调返回 `false` 立即停止
/// 整个枚举（当前调用方恒 `true`，靠查询预算收口）。段长从长到短只是枚举次序；
/// 最终次序由调用方**按分**排序（2026-10-27 起，见 `candidates` 的注释）。
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
    // 剩下的每一段至少要跨 2 键（调用方已保证 `last >= 2 * chunks_left`）；
    // 非末段再受 [`MAX_CHUNK_KEYS`] 封顶（末段 `chunks_left == 1` 不受限）。
    let max_end = (last - 2 * (chunks_left - 1)).min(pos + MAX_CHUNK_KEYS);
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
            .scheme_entries(sub, cache.scheme, None, 0)
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

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 `candidates_tests.rs` 同惯例。
#[path = "composition_scheme_tests.rs"]
#[cfg(test)]
mod tests;
