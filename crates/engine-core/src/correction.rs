// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 拼音**纠错**（typo 容错；2026-10-13 用户裁决「拼音纠错（混合参与排序）」）。
//!
//! 与 [`crate::fuzzy`] 同一族、同一条管线（整串变体 + 逐音节兜底，见 `candidates.rs`
//! 的两个接入点），差别只在变体怎么生成：模糊音是**语言学**的近音对（zh/z、n/l…），
//! 纠错是**键盘/手速**的编辑距离 1 —— 三类：
//! 1. **相邻键错按**（`hap` → `hao`：p 与 o 在 QWERTY 上相邻）；
//! 2. **少打一个字母**（`gu` → `guo`：用户漏了一键）；
//! 3. **多打一个字母**（`haoo` → `hao`）。
//!
//! ⚠️ 生成物**必须仍是合法音节**（同 fuzzy 的纪律）：拿 `zhong` 去插入字母会得到
//! 一堆 `zhonga`/`zhaong` 之类的非音节，查库只会白费查询。合法性过滤交给
//! [`crate::pinyin::is_syllable`]（410 音节表的二分）。
//!
//! ⚠️ 一律 `exact = false`（调用点、与 fuzzy 同一条）：纠错是"猜"，绝不能吃
//! `exact_bonus` —— 否则 `exact_bonus ≥ max_freq` 的结构性不变量被打破，
//! 一份高词的纠错命中会把真精确匹配顶掉（fuzzy.rs 头注记着同款事故的完整推演）。
//!
//! 成本与封顶：单音节生成 ~O(26·len) 次 `is_syllable`（二分，微秒级）；**贵的是
//! 每个变体一次词库查询** —— 变体走的是**前缀查询**（同 fuzzy），一次十几 ms 量级。
//!
//! ⚠️ 成本的三次实测（真 luna 库 + `candidates(9)`，2026-10-13，本机负载偏高、
//! 绝对值按 5-10× 打折看，**看的是增量**）：
//! 1. 套 fuzzy 的 `MAX_VARIANTS=16` + 前缀查询：**avg 101ms / worst 1180ms**
//!    （基线 30/173）—— 超标两三倍；
//! 2. 封顶到 [`MAX_CORRECTIONS`]（6 个）、砍多段相、逐音节相不再掺纠错：worst 降到
//!    500ms —— **还贵在前缀查询本身**（fuzzy 头注：一次变体前缀查询可命中 3 万余条）；
//! 3. **变体查询改精确键**（`query_all`，见 candidates.rs 的纠错档注释）：avg 44.6ms /
//!    worst 204ms（基线 26.9/147）—— 增量回到 ~18ms 量级（负载机），安静机上是
//!    单键查找的微秒-毫秒级，**回到 50ms 预算内**。
//!
//! 语义代价（如实记）：纠错只修**完整词**（typo 的本来就是完整词——nihap 想打 nihao），
//! 不展开前缀；「修出一个更长词的前缀」这种形态留给模糊音那条既有路（它仍走前缀）。

use crate::pinyin::{is_syllable, segment};

/// QWERTY 相邻键表（每行 = 物理相邻；表里每对写一次，[`neighbors`] 双向归一）。
/// 只收**字母键**的左右/上下邻居（斜向、数字行、Shift 不收 —— 多一组 = 每次击键
/// 多一批变体查询，宁缺毋滥）。单测钉：覆盖 a–z、关系双向。
const ADJACENT: &[(char, char)] = &[
    // 上行 q w e r t y u i o p
    ('q', 'w'),
    ('w', 'e'),
    ('e', 'r'),
    ('r', 't'),
    ('t', 'y'),
    ('y', 'u'),
    ('u', 'i'),
    ('i', 'o'),
    ('o', 'p'),
    // 中行 a s d f g h j k l
    ('a', 's'),
    ('s', 'd'),
    ('d', 'f'),
    ('f', 'g'),
    ('g', 'h'),
    ('h', 'j'),
    ('j', 'k'),
    ('k', 'l'),
    // 下行 z x c v b n m
    ('z', 'x'),
    ('x', 'c'),
    ('c', 'v'),
    ('v', 'b'),
    ('b', 'n'),
    ('n', 'm'),
    // 上下相邻（同一指列）
    ('q', 'a'),
    ('w', 's'),
    ('e', 'd'),
    ('r', 'f'),
    ('t', 'g'),
    ('y', 'h'),
    ('u', 'j'),
    ('i', 'k'),
    ('o', 'l'),
    ('a', 'z'),
    ('s', 'x'),
    ('d', 'c'),
    ('f', 'v'),
    ('g', 'b'),
    ('h', 'n'),
    ('j', 'm'),
];

/// `c` 的相邻字母（双向；字母表外返回空表）。
pub fn neighbors(c: char) -> Vec<char> {
    let mut out = Vec::new();
    for &(a, b) in ADJACENT {
        if a == c {
            out.push(b);
        } else if b == c {
            out.push(a);
        }
    }
    out
}

/// 一段（音节或**被 typo 切碎的残段**）的纠错变体（**不含自身**，去重、顺序确定：
/// 错按 → 少打 → 多打）。输出侧只留合法音节（过滤见 `push`）。
///
/// ⚠️ **输入端不设音节门槛**（起草时设过，被测试打回）：纠错恰恰要修的就是「不是音节
/// 的那一段」—— `nihap`（想打 nihao）会被贪婪切分器切成 `ni|ha|p`，末段 `p` 不是音节；
/// 允许对非音节段做变异，`p`→`o`（相邻键）才出得来 `ni`+`ha`+`o` = `nihao`。
/// 对真正的垃圾段（`zxcvb`）只是多几次二分合法性检查（变体全非法 ⇒ 零查询），成本可忽略。
pub fn corrections(syllable: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let chars: Vec<char> = syllable.chars().collect();
    let push = |s: String, out: &mut Vec<String>| {
        if is_syllable(&s) && !out.contains(&s) {
            out.push(s);
        }
    };
    // 1) 相邻键错按：逐位替换成物理邻居（最常见的手速错）。
    for (i, c) in chars.iter().enumerate() {
        for n in neighbors(*c) {
            let mut v = chars.clone();
            v[i] = n;
            push(v.into_iter().collect(), &mut out);
        }
    }
    // 2) 少打一个字母：删一位。
    for i in 0..chars.len() {
        let mut v = chars.clone();
        v.remove(i);
        push(v.into_iter().collect(), &mut out);
    }
    // 3) 多打一个字母：插一位（a–z 全试，合法性会滤掉绝大多数）。
    for i in 0..=chars.len() {
        for c in 'a'..='z' {
            let mut v = chars.clone();
            v.insert(i, c);
            push(v.into_iter().collect(), &mut out);
        }
    }
    out
}

/// 纠错变体的**自带封顶**（比 fuzzy 的 `MAX_VARIANTS` 严得多；理由见模块头注释）。
pub const MAX_CORRECTIONS: usize = 6;

/// 整串纠错变体（**不含自身**）：逐段生成后拼接，封顶 [`MAX_CORRECTIONS`]。
/// `'` 硬分隔时不生成（口径同 fuzzy：分隔符表达「就按这个切」，不是笔误）。
///
/// ⚠️ **枚举只做单段改动、且跨段轮转**（起草时套 fuzzy 的里程表，被测试打回）：
/// 里程表从左往右的次序会把预算喂给「最左段的各种改法」—— `nihap`（= ni|ha|p，
/// 想打 nihao）的 16 个名额全被 ni 段的变体吃光，**末段 p→o 都出不来**。
/// 轮转保证每段先拿头部变体；多段相在性能实测后砍掉（见模块头）。
pub fn input_corrections(input: &str) -> Vec<String> {
    if input.contains('\'') {
        return Vec::new();
    }
    let segs = segment(input);
    let per: Vec<Vec<String>> = segs.iter().map(|s| corrections(s)).collect();
    let cap = MAX_CORRECTIONS;
    let mut out: Vec<String> = Vec::new();
    let push = |cand: String, out: &mut Vec<String>| -> bool {
        if cand != input && !out.contains(&cand) {
            out.push(cand);
        }
        out.len() >= cap
    };
    // 一相：只改一段（其余段保持原样）。**跨段轮转**（seg0 第 1 个 → seg1 第 1 个 →
    // … → seg0 第 2 个）—— 顺序枚举会让靠前的段吃光预算（`nihap` 的 16 个名额曾
    // 全被 ni 段的变体占满，末段 p→o 都出不来）；轮转保证**每一段都先拿到自己的
    // 头部变体**，末段手速错不再被饿死。段内次序仍按 corrections 的确定性顺序。
    let max_len = per.iter().map(|v| v.len()).max().unwrap_or(0);
    for rank in 0..max_len {
        for (i, vs) in per.iter().enumerate() {
            let Some(v) = vs.get(rank) else { continue };
            let cand: String = segs
                .iter()
                .enumerate()
                .map(|(j, s)| if j == i { v.as_str() } else { s.as_str() })
                .collect();
            if push(cand, &mut out) {
                return out;
            }
        }
    }
    // 多段相已砍（预算留单段轮转）：见模块头注释的成本实测。
    out
}

/// 纠错档的候选（`candidates::rank_and_pick_with` 里的 `corr` 档；2026-10-14 从那里
/// 搬来 —— 加完下面的注释后那个文件 521/500 撞门禁）。每变体查**精确键**
/// （`query_all`，不展开前缀：typo 修的是完整词，前缀查询又是那条路的贵项），
/// 每变体取 top-3（同逐音节相），最后按分排序。
///
/// ⚠️ **它是独立一档、排在最外**（主档 → 模糊 → 简拼 → 纠错），**不许并进 `fuzzy`**
/// —— 真词库探针抓到的产品回归（2026-10-14 修）：
/// ```text
/// 输入 `nh`（简拼「你好」），产品默认（纠错开）：
///   纠错开：安徽 安徽省 那好 那会 呐喊 暗黑 恩惠 嗯哼 蒽环类 | 南航 内耗 你好 …  ← 第一页没有「你好」
///   纠错关：南航 内耗 你好 女孩 女孩子 您好 你会 男孩 …                        ← 正常
/// ```
/// 机制：`fuzzy` 档有「主档为空时它就是这个输入的主结果」的晋升规则（`zongguo` →
/// 「中国」那条）；纠错混进去后，`nh` 的主档本就为空 ⇒ `merged = fuzzy`（全是纠错
/// 噪声）⇒ 简拼档只能被**追加**、再被 `truncate(limit)` 砍掉。噪声来源实测：`nh` 的
/// 单段变体 `anh` 在真 luna 里键真有词（`query_all("anh")` 命中 安哈尔特/俺还/暗害/…，
/// 含高频「安徽」= 3787750734）。
///
/// 排最外与用户裁决「混合参与排序」不冲突：纠错**一直参与**（有别的候选时也照出），
/// 只是永远在最外层 —— 它是三档里最没把握的一档（「猜」）。生产回归钉见
/// `crates/engine-core/tests/correction_tier_order.rs`（夹具版，含变异自检）。
pub(crate) fn tier_hits<D: crate::dictionary::Dictionary + ?Sized>(
    dict: &D,
    input: &str,
    learner: &crate::learner::Learner,
    boost: u64,
) -> Vec<crate::candidates::Candidate> {
    let mut out: Vec<crate::candidates::Candidate> = Vec::new();
    for v in input_corrections(input) {
        let mut hits: Vec<crate::candidates::Candidate> = dict
            .query_all(&v)
            .into_iter()
            .map(|e| crate::candidates::Candidate {
                text: e.word.clone(),
                kind: crate::candidates::CandidateKind::Hanzi,
                // 变体命中一律 **exact = false**（理由见 candidates.rs 里那段
                // 「变体命中一律不算精确匹配」—— 保长替换会让前缀扩展也巧合等长）。
                score: crate::candidates::rank_score(e.freq, learner.freq_of(&e.word), boost),
            })
            .collect();
        hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
        out.extend(hits.into_iter().take(3));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacency_table_is_symmetric_and_covers_az() {
        for c in 'a'..='z' {
            let n = neighbors(c);
            assert!(!n.is_empty(), "{c} 没有任何邻居 —— 表漏了这个键");
            for x in &n {
                assert!(
                    neighbors(*x).contains(&c),
                    "邻居关系必须双向：{c}→{x} 但 {x} 没有 {c}"
                );
            }
        }
    }

    /// 三类纠错各钉一条（都取真实按键形态）。
    #[test]
    fn three_typo_classes_are_corrected() {
        // 错按：p 与 o 相邻 ⇒ hap → hao（"nihap" 想打 "nihao" 的末键错按）。
        assert!(
            corrections("hap").contains(&"hao".to_string()),
            "错按：hap → hao"
        );
        // 少打：gu → guo（"zhonggu" 想打 "zhongguo"）。
        assert!(
            corrections("gu").contains(&"guo".to_string()),
            "少打：gu → guo"
        );
        // 多打：haoo → hao。
        assert!(
            corrections("haoo").contains(&"hao".to_string()),
            "多打：haoo → hao"
        );
        // 负控：不相邻的键不该凭空纠出来（q 与 o 不相邻）。
        assert!(
            !corrections("haq").contains(&"hao".to_string()),
            "q↔o 不相邻，不得纠出"
        );
    }

    #[test]
    fn variants_are_always_legal_syllables() {
        for s in ["ni", "hao", "zhong", "guo", "xian"] {
            for v in corrections(s) {
                assert!(is_syllable(&v), "{s} 的纠错变体 {v} 不是合法音节");
                assert_ne!(v, s, "变体不含自身");
            }
        }
    }

    /// 整串：拆音节后逐段纠错拼接（`nihap` = ni + hap → ni + hao）。
    #[test]
    fn whole_string_corrections_compose_per_syllable() {
        let vs = input_corrections("nihap");
        assert!(
            vs.contains(&"nihao".to_string()),
            "nihap 应收出 nihao（{vs:?}）"
        );
        // 封顶与 fuzzy 同值；且不含自身。
        assert!(vs.len() <= MAX_CORRECTIONS);
        assert!(!vs.contains(&"nihap".to_string()));
        // 硬分隔（用户声明了切法）：不生成。
        assert!(input_corrections("ni'hap").is_empty());
    }
}
