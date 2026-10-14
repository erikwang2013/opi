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
//! 1. **常用单字**（`[COMMON_CHAR_SLOTS]` 个槽位）：拼音**精确**匹配（`pinyin_len ==
//!    输入长度`）的单字，按分取前若干条 —— 「常用」= 同一读音里词频最高的那几条；
//! 2. **联想**：其余全部（拼音延伸的字与词、拼音精确的词）。按 **拼音长度** 分组升序
//!    （「按拼音往后排」= 拼音越长越靠后，与 9 键左列 `短→长` 的既有裁决同一条读法）；
//!    **同一长度内先词后字**（用户给的次序 `字 → 词 → 延伸字`），**每一类各封顶
//!    `[PER_LENGTH_SLOTS]` 条**；
//! 3. **沉底**：① 拼音精确但没进第 1 档槽位的单字（`祢 鲵 逆`），② 封顶之外的一切
//!    （每组每类超出的条目）。按分（进来的次序），排在联想之后；
//! 4. **主块尾部**：没有分档元数据的条目（符号 / emoji / 逐音节汤）—— 它们在旧排序里
//!    就按微小分排在主块末尾，本层把它们整段放到三档之后（相对次序一字不动）。
//!
//! 组长（`ni` 真库实测，2026-10-14）：
//! `[你 泥 拟 呢 腻]`（1 档）→ `[你哦 你啊]`（3 字母·词）→ `[您 牛 扭 捏 纽 聂]`（3 字母·字）
//! → `[你的 你也 你不 你妈 你个]`（4·词）→ `[年 鸟 宁 凝 拧 黏]`（4·字）
//! → `[… 你们 你是 您好 你好 …]`（5·词，`你好` 在第 26 位）→ … → 沉底（`祢` 145）。
//! 调档手感 = 改 `[PER_LENGTH_SLOTS]`（调小 ⇒ 常用词更快浮上来，调大 ⇒ 同长度的字更全）。
//!
//! ⚠️ **为什么必须两个维度都封顶**（2026-10-14 逐版实测，三次翻车的记录）：
//! - 只按分数排（旧行为）：单音节精确单字有几百条，`你好` 永远出不来（用户报障的那条）；
//! - 只按「精确词 / 延伸」分档：`ni` 的延伸词有 **1195 条**，`您`/`年` 被顶到 1200 名外；
//! - 只按拼音长度分组：3 字母组（`nin`/`niu`/`nie` 一族）单组 200+ 条生僻字，
//!   `你好` 落到第 532 名；且**组内按分数排时生僻字会压过常用词**（真库实测：
//!   5 字母组的前 6 名是 `娘 酿 䖆 醸 𥽬 𪓃` —— 后四个是生僻字，分数被数据抬得比 `你好` 还高）。
//!
//! ⇒ 长度分组 + 组内词先于字 + 各自封顶，三条缺一不可。
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

/// 一档「常用单字」的槽位数：**5**。
///
/// 取值来自页面容量：桌面 / TSF 的候选页是 8 槽（`router.rs::PAGE_SIZE`），
/// 5 个字 + 3 个词组让首屏正好摆得下「常用字 + 一组常用词组」（`ni` → `你 泥 拟 呢 腻`
/// + `你好 你们 你的`）。Android 一次取 64 条、页内再切 8 —— 同一套槽位在那边同样成立。
///
/// ⚠️ 这是一条**产品口味**的常量，不是算法推导出来的：要调它只改这里，
/// 门禁 `tests/candidate_tiers.rs` 会跟着红（它钉的正是「前 N 位是精确单字」）。
pub(crate) const COMMON_CHAR_SLOTS: usize = 5;

/// 联想档里**每个拼音长度 × 每类（词 / 字）**取多少条（超出者沉底）。
///
/// 与 `[COMMON_CHAR_SLOTS]` 同一条性质：产品口味常量，三条理由（含真库实测数字）
/// 在头注 —— 取 6 = 每个（长度 × 类）凑不满一页，四五个组正好铺满前几页。
pub(crate) const PER_LENGTH_SLOTS: usize = 6;

/// 分档元数据：只有**主查询**（词库前缀命中）带它。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Meta {
    /// 词条拼音键的长度（`Entry::pinyin_len`）。分档用它判「精确」与联想的先后。
    pub pinyin_len: usize,
    /// 是否精确匹配（`pinyin_len == 输入长度`）。9/14 键那条路按**键长相等**判，
    /// 与全键盘同义（见 `candidates_scheme.rs` 头注）。
    pub exact: bool,
    /// 用户词频 > 0（选过）—— 见头注第 0 档。
    pub learned: bool,
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

/// 把**已按分排好序**的主块重排成三档（调用方保证：进来的次序就是分值降序 —— 本模块
/// 靠它取「分数最高的前 `COMMON_CHAR_SLOTS` 个精确单字」）。
///
/// 词条判定用 `text` 的字符数（`chars().count() == 1` = 单字），与候选层其它地方
/// 同一判据（别用 `len()`：那是字节数，一个汉字 3 字节会把单字全判成词组）。
pub(crate) fn apply<T: Tiered>(items: Vec<T>) -> Vec<T> {
    // 第 0 档：学过的（见头注）—— 输入已是分数降序，故这一档天然按分排。
    let mut learned: Vec<T> = Vec::new();
    let mut common: Vec<T> = Vec::new();
    // 联想主体：元数据跟着条目走（重排后不能再回头查）。键 =（拼音长度 × 是不是单字）。
    let mut assoc: Vec<(T, usize, bool)> = Vec::new();
    let mut per_slot: std::collections::HashMap<(usize, bool), usize> =
        std::collections::HashMap::new();
    // 沉底：精确超槽位的单字 + 每个（长度 × 类）超顶的条目（进来的次序即分数序）。
    let mut sunk: Vec<T> = Vec::new();
    let mut tail: Vec<T> = Vec::new();
    for it in items {
        let Some(m) = it.meta() else {
            tail.push(it);
            continue;
        };
        if m.learned {
            learned.push(it);
            continue;
        }
        let single = it.text().chars().count() == 1;
        if single && m.exact {
            // 精确单字：前 `COMMON_CHAR_SLOTS` 条进第 1 档，其余（生僻）沉底。
            if common.len() < COMMON_CHAR_SLOTS {
                common.push(it);
            } else {
                sunk.push(it);
            }
            continue;
        }
        let taken = per_slot.entry((m.pinyin_len, single)).or_insert(0);
        if *taken < PER_LENGTH_SLOTS {
            *taken += 1;
            assoc.push((it, m.pinyin_len, single));
        } else {
            sunk.push(it);
        }
    }
    // 联想主体：拼音长度升序 → 同长度**先词后字**（`single` 排在后面）—— 稳定排序，
    // 同 (长度, 类) 内保持进来的次序（= 分数降序 + 简体优先已分区，见头注那条）。
    assoc.sort_by_key(|(_, plen, single)| (*plen, *single));
    let mut out =
        Vec::with_capacity(learned.len() + common.len() + assoc.len() + sunk.len() + tail.len());
    out.extend(learned);
    out.extend(common);
    out.extend(assoc.into_iter().map(|(it, _, _)| it));
    out.extend(sunk);
    out.extend(tail);
    out
}
