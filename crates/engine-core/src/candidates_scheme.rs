// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **按键类展开一族**（9/14 键方案）—— 2026-10-12 从 `candidates.rs` 拆出（当初为
//! 500 行门禁；拆分先例与 `engine_punct.rs` / `engine_readings.rs` 同套，语义一字未动）。
//! 同日该拆分随「上文拼接」一并被裁决撤除**行为**，但**文件拆分保留**（删了它
//! `candidates.rs` 会重新越线）。
//!
//! 为什么它值得单独一个文件：这条路**表达不成前缀查询**（「64」不是任何键的前缀），
//! 与全键盘路的数据来源、算法形状都不同（见 [`rank_and_pick_scheme`] 的长头注）。
//! 对外路径不变：`candidates.rs` 里 `pub use` 回来，调用方仍写
//! `candidates::rank_and_pick_scheme`。

use crate::candidates::{Candidate, CandidateKind, rank_score};
use crate::candidates_tiers_impl as tiers;
use crate::candidates_tiers_impl::Meta;
use crate::dictionary::{Dictionary, SCHEME_MATCH_CAP};
use crate::learner::Learner;

/// **按键类展开**（9/14 键方案，2026-10-09）：`input` 是一串按键符号，按
/// [`crate::scheme::Scheme::matches`] 的类前缀语义匹配词库键。
///
/// 为什么另起一条路而不是塞进 [`rank_and_pick_with`]：这条匹配**表达不成前缀
/// 查询**（「64」不是任何键的前缀），只能全量遍历词库逐键比对
/// （`Dictionary::visit_entries`）—— 数据来源、算法形状都不同，硬合并会把两条
/// 路都读不懂。打分/去重/截断规则与全键盘**逐字相同**（exact 加成按「键长相等」
/// 判，与 pinyin_len == 输入长度同义），保证两套键盘的候选行为一致；整串无命中时
/// 还有 26 键同一条**回退**（2026-10-10 扩到 9 键：`644269` → 你好一族）。
/// 返回 `(候选, 逐条读音)`：`readings[i]` = `cands[i]` 的词典拼音（截断后同序，
/// 两串等长）。9 键的缓冲是数字串，界面上要显示「拼音读数」时靠它 ——
/// `display_buffer` 取第一条（搜狗 9 键候选栏左侧那格同款），
/// `candidate_readings` 读同一份缓存并按读音**去重保序**（2026-10-12 用户裁决
/// 「左列去重」；逐条对应只到本函数返回值与引擎内部缓存为止，不再是出口契约）。
/// 没有候选时两条都为空。14 键同路返回（它的缓冲本来就是字母串，读数字段无副作用）。
// 9 个参数（7 个 + `prefer_simplified` + `tail_guess`）踩了 clippy 的默认上限。**有意放行**：
// 这两个开关必须在**本函数内部的截断之前**生效（截断后再分区 = 被截掉的简体候选回不来），
// 所以只能从参数进来；而把它们收进 struct 会牵动唯一调用方之外的测试与全键盘那条路
// （`RankOptions` 已有三个开关，两条路的开关面并不相同 —— 合并要另立一次裁决）。
// `tail_guess` 的唯一「关」的调用方是收尾探针（`engine_segment::digit_tail_to_keep`）：
// 它问的是「这条尾巴在**严格档**下有没有候选」，宽松档必须被排除，否则 2026-10-15 那条
// 「长死尾巴不留」的裁决会被本档悄悄废掉（详见 `tail_guess` 的注记）。
#[allow(clippy::too_many_arguments)]
pub fn rank_and_pick_scheme<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    input: &str,
    limit: usize,
    boost: u64,
    scheme: crate::scheme::Scheme,
    pin: Option<&str>,
    prefer_simplified: bool,
    tail_guess: bool,
) -> (Vec<Candidate>, Vec<String>) {
    if input.is_empty() || limit == 0 {
        return (Vec::new(), Vec::new());
    }
    let exact_bonus = boost / 2;
    // 一趟扫描 = 一个输入串的整档（读音随条目一起暂存：pinyin 的生命周期出不了
    // visit_entries 的回调）。两趟（回退态）共用这一份。
    // 每条 =（候选，逐条读音，分档元数据）—— 三样必须**一起搬**（分档只换次序，
    // 候选↔读音的配对不能散；元数据出了本函数就没有了）。`None` = 非主查询条目。
    let scan = |input: &str| -> Vec<(Candidate, String, Option<Meta>)> {
        let input_len = input.len();
        // 按键类匹配走 `Dictionary::scheme_entries`（区间枚举 + 静态词频 top-N 截断）——
        // 2026-10-11 移动端实测：此前是全表逐条 `matches`，936k 条下单键 268ms（桌面
        // release），手机端每键秒级。pin 过滤（左列点一行拼音）语义在实现里同义保留。
        dict.scheme_entries(input, scheme, pin, SCHEME_MATCH_CAP)
            .into_iter()
            .map(|(e, pinyin)| {
                let exact = pinyin.len() == input_len;
                let meta = Meta {
                    pinyin_len: pinyin.len(),
                    exact,
                    // 同全键盘：学过的排第 0 档（见 `candidates_tiers.rs` 头注）。
                    learned: learner.freq_of(&e.word) > 0,
                };
                let score = rank_score(e.freq, learner.freq_of(&e.word), boost)
                    .saturating_add(if exact { exact_bonus } else { 0 });
                (
                    Candidate {
                        text: e.word,
                        kind: CandidateKind::Hanzi,
                        score,
                    },
                    pinyin,
                    Some(meta),
                )
            })
            .collect()
    };
    let mut merged = scan(input);
    // ── 多音字补读音（2026-10-15「单独打『么』没有候选」那一类）────────────────
    // 9 键的缓冲是数字串，走 `digit_hits`（按音节建的 `hits` 查不到数字串）。
    // 精确性按**键长相等**判 —— 与全键盘那条路同义（本文件头注的既有契约）；
    // 表里只有词库真缺的条目 ⇒ 去重交给下面的 `seen`。
    let input_len = input.len();
    let dict_max = dict.max_freq(); // 量纲判据 + 缩放（见 `polyphone.rs`）
    for (word, freq, plen) in crate::polyphone::digit_hits(input)
        .iter()
        .filter(|_| crate::polyphone::applies(dict_max))
    {
        let exact = *plen == input_len;
        let user = learner.freq_of(word);
        let freq = crate::polyphone::scaled_freq(*freq, dict_max);
        merged.push((
            Candidate {
                text: (*word).to_string(),
                kind: CandidateKind::Hanzi,
                score: rank_score(freq, user, boost).saturating_add(if exact {
                    exact_bonus
                } else {
                    0
                }),
            },
            // 读音串不进这条出口（`readings` 是给左列的显示读数）：补读音的字没有
            // 「逐条读数」可言，按**输入本身的键序**给它一条 —— 9 键左列显示的仍是
            // 「还能接哪些键」那套语义，不是新造一个读音。
            input.to_string(),
            Some(Meta {
                pinyin_len: *plen,
                exact,
                learned: user > 0,
            }),
        ));
    }
    // ── 数字空间回退（2026-10-10 扩到 9 键）：整串无命中改扫最长音节对齐键前缀的整档
    // （判定唯一实现 `scheme::scheme_fallback_len`；命中态非空 ⇒ 一行不重，重扫深度 ≤ 1）。
    // ⚠️ 判据在**补读音之后**算（与全键盘那条路同姿态）：补读音命中时 `merged` 非空 ⇒ 不回退。
    // `tail` = 严格档**读不出来**的那一段余键（宽松兜底档的输入，见 `tail_guess`）：
    // 整串严格命中 ⇒ 空串（本档一行不做）；回退吃了前缀 ⇒ 余键；连回退都没落点 ⇒ 整串。
    let mut tail: &str = "";
    if merged.is_empty() {
        match crate::scheme::scheme_fallback_len(dict, input, scheme, pin) {
            Some(k) => {
                merged = scan(&input[..k]);
                tail = &input[k..];
            }
            None => tail = input,
        }
    }
    merged.sort_by(|a, b| b.0.score.cmp(&a.0.score).then(a.0.text.cmp(&b.0.text)));
    let mut seen = std::collections::HashSet::new();
    merged.retain(|(c, _, _)| seen.insert(c.text.clone()));
    // ── 宽松兜底档（用户裁决 2026-10-15「没匹配的用按钮上最前的开始匹配字、词」）────
    // 触发面：有尾巴（上面那三种情形）且调用方允许（收尾探针会关它）。
    if tail_guess && !tail.is_empty() {
        let guess = tail_guess_hits(dict, learner, tail, scheme, boost, &mut seen);
        if !guess.is_empty() {
            // 读数改写（**只在产出非空时**）：本次逐条读数统一成「前缀首条读音 + 尾字母」
            // —— 9 键拼音行因此把这条尾巴**显示出来**（`ni'g` / `ni'hao'w`），
            // 而不是像报障当时那样「后面还藏着东西但看不出来」。产出为空 ⇒ 一个字不改
            // （稀疏夹具的既有断言因此保绿）。
            let letter = tail
                .chars()
                .next_back()
                .and_then(|k| scheme.key_leading_letter(k));
            let mut reading = merged
                .first()
                .map(|(_, p, _)| p.clone())
                .unwrap_or_default();
            if let Some(l) = letter {
                reading.push(l);
            }
            merged.extend(guess);
            for (_, p, _) in merged.iter_mut() {
                *p = reading.clone();
            }
        }
    }
    // ── 候选三档（2026-10-14 用户裁决）────────────────────────────────────────
    // 与全键盘**同一条**（「两套键盘的候选行为一致」是本文件头注里的既有契约）：
    // 9/14 键的缓冲是按键串，读数就是它展开出的拼音，档位判据（精确 = 键长相等）
    // 与全键盘同义。`apply` 只换次序，三元组整体搬 ⇒ 候选↔读音的配对不散。
    // 简体优先（见参数注）：**截断之前**分区，且**必须先于分档** —— 两步都是按位稳定
    // 分区，跑在后面会把三档重排掉（理由与实测见 `candidates_tiers.rs` 头注）。
    // 成对分区（读音跟候选一起搬 —— 两者必须保持同序），档位线同全键盘路。
    if prefer_simplified {
        crate::trad_chars::demote_traditional_by(
            &mut merged,
            boost / 2,
            |(c, _, _)| c.score,
            |(c, _, _)| &c.text,
        );
    }
    merged = tiers::apply(merged);
    merged.truncate(limit);
    // 读音**逐条**带出（与候选同序、同一次排名）—— 调用方按需取首条或全量；
    // 分开再算一次 = 第二次全量扫描，别在调用链上再开这条路。
    merged.into_iter().map(|(c, p, _)| (c, p)).unzip()
}

/// 前段（整段当一个音）出的**单字**上限。
const TAIL_CHARS: usize = 4;
/// 前段出的**词**上限。
const TAIL_WORDS: usize = 4;
/// 后段（逐键起音）每个键出的单字上限。
const TAIL_PER_KEY_CHARS: usize = 2;
/// 后段总上限。
const TAIL_KEY_CHARS_MAX: usize = 8;
/// 每次字母前缀查询的取回上限（兜底档不追求穷尽；与 26 键那条路的 `PREFIX_MATCH_CAP` 同量级）。
const TAIL_LETTER_CAP: usize = 256;

/// 三元组别名（与 `scan` 的产物同形）——直写会踩 clippy 的 `type_complexity`（同
/// `polyphone::DigitHit` 的先例）。
type TailHit = (Candidate, String, Option<Meta>);

/// **宽松兜底档**（用户裁决 2026-10-15；报障原文「输入 ni 后不停点 i 所在的按钮……再次
/// 输入其他拼音无反应，候选词显示无匹」+「没匹配的用按钮上最前的开始匹配字、词」）。
///
/// `tail` = 严格档读不出来的那段余键。**先段后键**两段（用户从三选一里拍板）：
/// 1. **前段**（整段当一个音）：取**最后一个**键的首字母 `L1`（9 键 `4`→`g`），一次前缀
///    查询后按最终分排、分两串：单字 ≤ [`TAIL_CHARS`]、词 ≤ [`TAIL_WORDS`]，**字在前词在后**；
/// 2. **后段**（逐键起音）：尾巴里每个键各取首字母出单字（每键 ≤ [`TAIL_PER_KEY_CHARS`]、
///    总 ≤ [`TAIL_KEY_CHARS_MAX`]）；**同字母只查一次**（连按同一个键是报障现场的常态）。
///
/// 产物 `Meta = None` + 中间那格（读音）留空：`None` 让分档整段落到主块尾部；读音由调用方
/// 在**产出非空**时统一改写成「前缀首条读音 + 尾字母」（见 `rank_and_pick_scheme` 那段），
/// 本函数不自己造读数。一律不吃 `exact_bonus` —— 同「变体命中一律 exact=false」的硬纪律。
///
/// 任一键取不到类（`0`/`1`/字母混进缓冲）⇒ 空（整档不启用，与 `Scheme::matches` 同姿态）。
/// `seen` 与既有候选共用一份（前段字/前段词/后段字之间也不重复）。
fn tail_guess_hits<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    tail: &str,
    scheme: crate::scheme::Scheme,
    boost: u64,
    seen: &mut std::collections::HashSet<String>,
) -> Vec<TailHit> {
    // ⚠️ **本版只给 9 键**：14 键的键类首字母 == 键自身（`'b' => ['b','n']`）⇒ 本档退化
    // 成「把缓冲当字面字母串做前缀匹配」，与既有的 14 键回退重叠且语义重复。一行可开
    // （把这里改成 `Nine | Fourteen` 并补 14 键用例），但那是另一次裁决。
    if scheme != crate::scheme::Scheme::Nine {
        return Vec::new();
    }
    let heads: Option<Vec<char>> = tail.chars().map(|k| scheme.key_leading_letter(k)).collect();
    let Some(heads) = heads else {
        return Vec::new();
    };
    let Some(&l1) = heads.last() else {
        return Vec::new();
    };
    // 每个字母的**前缀池**（排好序，只查一次、按字母缓存）：连按同一个键是报障现场的
    // 常态，同一个字母不该查第二遍 —— 计数门禁按「每次抓取尾档 1 次查询」钉着
    // （`tests/fetch_counts.rs`）。池子两段共用：前段取字与词，后段从同一池里继续取字。
    let mut cache: std::collections::HashMap<char, Vec<Candidate>> =
        std::collections::HashMap::new();
    let pool_of = |cache: &mut std::collections::HashMap<char, Vec<Candidate>>,
                   letter: char|
     -> Vec<Candidate> {
        cache
            .entry(letter)
            .or_insert_with(|| {
                let mut pool: Vec<Candidate> = dict
                    .query_prefix_top(&letter.to_string(), TAIL_LETTER_CAP)
                    .into_iter()
                    .map(|e| {
                        let score = rank_score(e.freq, learner.freq_of(&e.word), boost);
                        Candidate {
                            text: e.word,
                            kind: CandidateKind::Hanzi,
                            score,
                        }
                    })
                    .collect();
                pool.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
                pool
            })
            .clone()
    };
    let mut out: Vec<TailHit> = Vec::new();
    // 前段（整段当一个音）：字 ≤ TAIL_CHARS、词 ≤ TAIL_WORDS，同一个池里分区、字在前词在后。
    let mut chars = 0usize;
    let mut words = 0usize;
    for c in pool_of(&mut cache, l1) {
        let single = c.text.chars().count() == 1;
        let (slot, cap) = if single {
            (&mut chars, TAIL_CHARS)
        } else {
            (&mut words, TAIL_WORDS)
        };
        if *slot >= cap || !seen.insert(c.text.clone()) {
            continue;
        }
        *slot += 1;
        out.push((c, String::new(), None));
    }
    // 后段（逐键起音）：每个键各取首字母、从同一个池里继续取字（每键 ≤ TAIL_PER_KEY_CHARS、
    // 总 ≤ TAIL_KEY_CHARS_MAX）；`seen` 保证与前段不重。
    let mut per_key_total = 0usize;
    for &letter in &heads {
        if per_key_total >= TAIL_KEY_CHARS_MAX {
            break;
        }
        let mut taken = 0usize;
        for c in pool_of(&mut cache, letter) {
            if taken >= TAIL_PER_KEY_CHARS || per_key_total >= TAIL_KEY_CHARS_MAX {
                break;
            }
            if c.text.chars().count() != 1 || !seen.insert(c.text.clone()) {
                continue;
            }
            taken += 1;
            per_key_total += 1;
            out.push((c, String::new(), None));
        }
    }
    out
}
