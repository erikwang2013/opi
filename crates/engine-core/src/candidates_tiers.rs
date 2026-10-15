// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **候选分档**（2026-10-14 用户裁决）—— 单独成文件是因为 `candidates.rs` 已 475 行、
//! 加本层必越 500 行门禁（拆分先例与 `candidates_scheme.rs` / `candidates_options.rs` /
//! `engine_punct.rs` 同套）。
//!
//! 裁决原文：「候选词里优先给符合拼音的候选字，然后是词组，最后是联想的，联想的字、
//! 词组按照拼音往后排」。**起因是实测**：真库打 `ni` 时主档被精确加成（`boost/2`）压成
//! 一片单字海 —— 前 40 位全是单字（`你 泥 拟 呢 腻 祢 铌 鲵 𢘝 …`），「你好 / 你们 /
//! 你的」这些词组**永远挤不进来**（任何精确匹配都排在所有前缀扩展之前，而单音节精确单字
//! 在真库里有几百个）。这不是 bug 而是旧排序的必然结果，改的是**分档**。
//!
//! ## 学过的档 + 三档 + 主块尾部（次序即本模块的输出次序）
//!
//! 0. **学过的**（`user_freq > 0`）：**不受三档约束**，按分排在最前。这不是顺手加的 ——
//!    它保住两条既有不变式：README 缘起 #3「你最常用的那个词下次要能排到首位」与
//!    `tests/scheme_readings.rs` 那条「学过的长词压过精确档 ⇒ 读数短→长仍要成立」的
//!    夹具前提。学习是比「字先于词」更强的用户偏好；没有这一档，用户选过的词会掉到
//!    第 6 位之后 —— 那是把「学习」这个功能作废，不是本裁决想要的。
//! 1. **精确单字**：拼音**精确**匹配（`pinyin_len == 输入长度`）的**全部**单字，
//!    按分排 —— **不封顶**（2026-10-15 用户裁决「输入的拼音匹配候选词是字的时候，拼音
//!    优先匹配字，匹配完后再匹配词」：原先只取前 5 条「常用单字」，现在全给）；
//! 2. **词组**：多字条目（精确的与拼音延伸的都在这里），按 **拼音长度**升序（精确词
//!    的 plen == 输入长 ⇒ 天然最前），同长度按分；
//! 3. **联想**：拼音**延伸的单字**（`ni` → `nian` 年 / `nin` 您 / `niang` 娘），
//!    按拼音长度升序，**每个长度封顶 `[PER_LENGTH_SLOTS]`**（超出的沉底）；
//! 4. **主块尾部**：没有分档元数据的条目（符号 / emoji / 逐音节汤）—— 整段放到最后。
//!
//! ⚠️ **这条裁决的代价（用户知情并选了它，2026-10-14 测量、2026-10-15 复述警告）**：
//! 精确单字不封顶 ⇒ 单音节输入下真库有**几百个**精确字（`ni` 一族 580 个）排在任何
//! 词组之前 —— 打 `ni` 时「你好 / 你们」要在几百名之后才出现；打全拼（`nihao`）不受
//! 影响（那种输入没有精确单字 ⇒ 第 1 档为空、词组直接顶上）。这一条**不是回归**，
//! 别再按 10-14 的旧形态「修回去」；代价由 `tests/candidate_tiers.rs` 与
//! `engine-data/tests/polyphone_real.rs` 两处门禁如实钉住。
//!
//! ## 边界与不变量（改之前先读）
//!
//! - **只对「主查询」分档**：模糊音 / 简拼 / 纠错 / 分词回退这几档的候选**不带元数据**，
//!   它们仍按既有规则**追加在主块之后**（`candidates.rs` 的分档规则一字未动）。
//! - **精确 ≥ 前缀扩展**那条老不变量**在本层被有意削弱**（不是违反）：它的原意是
//!   「打 `xian` 不由 `xiang` 的想占首位」—— 本层下 `想` 落进联想档（拼音更长），
//!   首位仍是精确单字；被让上去的是**词组**（`xian` → 现在 / 先生），这正是裁决要的。
//!   `candidates.rs` 里那条注释已同步改写，别按旧文字「修回去」。
//! - **`prefer_simplified` 必须跑在本层之前**（2026-10-14 实测踩到）：它也是一次
//!   「按位稳定分区」（`trad_chars::demote_traditional_by`），**谁在后面谁就是最终
//!   次序** —— 跑在后面会把三档整段重排（实测 `xian`：`現` 被抬到词组之前、档位当场
//!   消失）。跑在前面则三档**继承**它：档内保持「简体在前」的顺序（联想档只在同拼音
//!   长度内继承 —— 跨长度的先后由裁决本身决定）。追加档（模糊/简拼/纠错）不在主块里，
//!   由调用方对它们自己那一截再跑一次。
//! - **两条排名路都要分档**（全键盘 `rank_and_pick_with` 与 9/14 键 `rank_and_pick_scheme`）：
//!   「两套键盘的候选行为一致」是 `candidates_scheme.rs` 头注里的既有契约。9/14 键那条
//!   还要把**逐条读音**跟着候选一起搬（`Tiered` 的第二个实现）。
//! - **码类方案不分档**：笔画 / 五笔的缓冲是编码不是拼音（`RankOptions::tiers` 显式关），
//!   拿「单字 / 词组 / 联想」去切码串是似是而非 —— 与 `segment_fallback` 在五笔上的
//!   裁决同一条理由。

use crate::candidates::Candidate;

/// **联想档**（拼音延伸的单字）每个拼音长度取多少条（超出者沉底）。
///
/// 产品口味常量：取 6 = 每组凑不满一页，四五个组正好铺满前几页。⚠️ 第 1 档
/// （精确单字）与第 2 档（词组）**没有这个封顶**（2026-10-15 裁决：字全出完再出词）。
pub(crate) const PER_LENGTH_SLOTS: usize = 6;

/// 分档元数据：只有**主查询**（词库前缀命中）与**9 键宽松兜底档**带它。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Meta {
    /// 词条拼音键的长度（`Entry::pinyin_len`）。分档用它判「精确」与联想的先后。
    pub pinyin_len: usize,
    /// 是否精确匹配（`pinyin_len == 输入长度`）。9/14 键那条路按**键长相等**判，
    /// 与全键盘同义（见 `candidates_scheme.rs` 头注）。
    pub exact: bool,
    /// 用户词频 > 0（选过）—— 见头注第 0 档。
    pub learned: bool,
    /// **9 键宽松兜底档**的产物（判据见 `candidates_scheme::tail_guess_hits`）。分档把它
    /// 排在**词组之后、联想单字之前**（用户裁决 2026-10-16：严格解读永远优先）。
    /// 报障串 `6444444` 真库实测：严格族 2418 条（精确单字 386）⇒ 尾巴排「词组之前」
    /// 也要第 386 位、排「词组之后」第 1634 位 —— Android 只抓 64 条（翻页纯客户端），
    /// 两种位置在那条串上都看不见；排到一切严格候选之前虽能看见，但会把 `你`/`迷航`
    /// 这类首选顶下去（实测 `6444444` 首候选变「个」、`64426444` 的「迷航」掉到第 9），
    /// 故不做。相对 075a382 的位置（联想+沉底之后）本档位仍提前了「联想」那一段。
    /// 26 键那条路恒 `false`。
    pub tail: bool,
}

/// 分档排序需要的最小视图。两个实现对应两条排名路各自的附加数据形态：
/// 全键盘是 `(候选, 元数据)`，9/14 键是 `(候选, 逐条读音, 元数据)`（读音必须跟着搬）。
pub(crate) trait Tiered {
    fn text(&self) -> &str;
    /// `None` = 不是主查询条目（符号 / emoji / 逐音节汤）⇒ 主块尾部。
    fn meta(&self) -> Option<Meta>;
}

impl Tiered for (Candidate, Option<Meta>) {
    fn text(&self) -> &str {
        &self.0.text
    }
    fn meta(&self) -> Option<Meta> {
        self.1
    }
}

impl Tiered for (Candidate, String, Option<Meta>) {
    fn text(&self) -> &str {
        &self.0.text
    }
    fn meta(&self) -> Option<Meta> {
        self.2
    }
}

/// 把**已按分排好序**的主块重排成三档（调用方保证：进来的次序就是分值降序 ——
/// 本模块各档内部的「按分」直接沿用这个次序）。
///
/// 词条判定用 `text` 的字符数（`chars().count() == 1` = 单字），与候选层其它地方
/// 同一判据（别用 `len()`：那是字节数，一个汉字 3 字节会把单字全判成词组）。
pub(crate) fn apply<T: Tiered>(items: Vec<T>) -> Vec<T> {
    // 第 0 档：学过的（见头注）—— 输入已是分数降序，故这一档天然按分排。
    let mut learned: Vec<T> = Vec::new();
    // 第 1 档：**全部**拼音精确匹配的单字（2026-10-15 用户裁决「字全出完再出词」——
    // 原先只取前 `COMMON_CHAR_SLOTS` 条，现在不封顶）。次序 = 进来的次序（分数降序）。
    let mut chars: Vec<T> = Vec::new();
    // 第 2 档：词组（精确 + 拼音延伸）——元数据（拼音长度）跟着走：按长度升序、同长度按分。
    let mut words: Vec<(T, usize)> = Vec::new();
    // 第 2.5 档：**9 键宽松兜底档**的产物（`meta.tail`，见 `Meta::tail`）：排在词组之后、
    // 联想单字之前 —— 顺序按进来的次序（本档自己那套 字→词→字 已由产出方排好）。
    let mut pending: Vec<T> = Vec::new();
    // 第 3 档：拼音延伸的**单字**（联想；每个拼音长度封顶 `PER_LENGTH_SLOTS`）。
    let mut assoc: Vec<(T, usize)> = Vec::new();
    let mut per_slot: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    // 沉底：联想档超出每长度封顶的条目（进来的次序即分数序）。
    let mut sunk: Vec<T> = Vec::new();
    let mut tail: Vec<T> = Vec::new();
    for it in items {
        let Some(m) = it.meta() else {
            tail.push(it);
            continue;
        };
        if m.tail {
            pending.push(it);
            continue;
        }
        if m.learned {
            learned.push(it);
            continue;
        }
        let single = it.text().chars().count() == 1;
        if single && m.exact {
            chars.push(it); // 全部精确单字，不封顶（裁决）
            continue;
        }
        if !single {
            words.push((it, m.pinyin_len)); // 词组：精确的 plen == 输入长 ⇒ 天然最前
            continue;
        }
        let taken = per_slot.entry(m.pinyin_len).or_insert(0);
        if *taken < PER_LENGTH_SLOTS {
            *taken += 1;
            assoc.push((it, m.pinyin_len));
        } else {
            sunk.push(it);
        }
    }
    // 词与联想：拼音长度升序；同长度保持进来的次序（分数降序 + 简体优先已分区，见头注）。
    words.sort_by_key(|(_, plen)| *plen);
    assoc.sort_by_key(|(_, plen)| *plen);
    let mut out = Vec::with_capacity(
        learned.len()
            + chars.len()
            + words.len()
            + pending.len()
            + assoc.len()
            + sunk.len()
            + tail.len(),
    );
    out.extend(learned);
    out.extend(chars);
    out.extend(words.into_iter().map(|(it, _)| it));
    out.extend(pending);
    out.extend(assoc.into_iter().map(|(it, _)| it));
    out.extend(sunk);
    out.extend(tail);
    out
}
