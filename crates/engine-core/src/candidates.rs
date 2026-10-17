// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::candidates_tiers_impl as tiers;
use crate::candidates_tiers_impl::Meta;
use crate::composer::Mode;
use crate::correction;
use crate::dictionary::{Dictionary, PREFIX_MATCH_CAP};
use crate::fuzzy;
use crate::jianpin;
use crate::learner::Learner;
use crate::pinyin::segment;
use crate::symbols::SymbolEngine;
use crate::trie::Entry;

/// 用户词频权重下限：一次选词 ≈ 10 万次静态词频，保证学习迅速生效。
/// 实际权重在 Engine::new 按词典最大词频动态缩放（max_freq × 2），
/// 固定值在 luna 百万级词频下失配：选一次"我"(10万) 仍输给"倭"(50万)。
pub const USER_BOOST: u64 = 100_000;
/// 默认候选栏容量。
pub const DEFAULT_TOP_N: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    Hanzi,
    English,
    Emoji,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CandidateKind,
    pub score: u64,
}

/// 排序分：静态词频 + 用户词频 × boost。
pub fn rank_score(static_freq: u32, user_freq: u32, boost: u64) -> u64 {
    (static_freq as u64).saturating_add((user_freq as u64).saturating_mul(boost))
}

// 排序层开关 [`RankOptions`] 在 `candidates_options.rs`（500 行门禁拆出，见那里头注）。
pub use crate::candidates_options_impl::RankOptions;

/// 合并词典候选与符号候选，排序、去重、截断。
/// 两个开关全开的 [`rank_and_pick_with`]（测试与既有调用方的入口；
/// 与 `Engine::new` / `with_dictionaries` 的「旧入口 = 新入口的默认包装」同一条先例）。
pub fn rank_and_pick<D: Dictionary + ?Sized>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
    boost: u64,
) -> Vec<Candidate> {
    rank_and_pick_with(
        dict,
        symbols,
        learner,
        input,
        mode,
        limit,
        boost,
        RankOptions::default(),
    )
}

// 8 个参数（原 7 个 + 选项尾巴）踩了 clippy 的默认上限。**有意放行**：本函数是公开
// `rank_and_pick` 的直系（同参 + 一个 `RankOptions` 尾巴），把参数收进一个 struct
// 会让这两个 API 的形状分叉，而唯一的调用方只有 `Engine::candidates` 一处。
// 理由若要变（比如将来再加两个选项），改法是**收 struct**，不是再加 allow。
#[allow(clippy::too_many_arguments)]
pub fn rank_and_pick_with<D: Dictionary + ?Sized>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
    boost: u64,
    opts: RankOptions,
) -> Vec<Candidate> {
    // 旧入口 = 新入口的默认包装：丢掉分档元数据。**次序逐条相同**（元数据只用于排序，
    // 排完就没用了）；要元数据的调用方（双拼跨解析合并）走 `rank_and_pick_meta`。
    rank_and_pick_meta(dict, symbols, learner, input, mode, limit, boost, opts)
        .into_iter()
        .map(|(c, _)| c)
        .collect()
}

/// 同 [`rank_and_pick_with`]，但候选**连分档元数据一起**返回（2026-10-14 候选三档）。
///
/// 为什么元数据要出这一层：双拼一次按键解出多个候选拼音，**分档必须发生在跨解析合并
/// 之后**（单次解析内分档会被那边的全局重排抹平），而「拼音多长、是否精确」这两条
/// 出了本函数就没了。其余调用方（全键盘 / 9 键 / 五笔 / 笔画）只关心次序 ⇒ 走上面的
/// 包装。`None` = 不是主查询条目（符号 / emoji / 逐音节汤 / 分词回退）⇒ 分档时落主块尾部。
// 参数与包装逐字相同（8 个）—— allow 的理由见上面那条。
#[allow(clippy::too_many_arguments)]
pub(crate) fn rank_and_pick_meta<D: Dictionary + ?Sized>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
    boost: u64,
    opts: RankOptions,
) -> Vec<(Candidate, Option<Meta>)> {
    // ⚠️ **没有「上文拼接」档**（2026-10-12 晚用户裁决）：确认后再输入是**全新开始**，
    // 候选只按本次输入匹配。白天那版把「上次选中的词拼音 + 本次输入」的整串命中插在最前
    // （函数多一个 `ctx` 参数），点它会**重复上屏已确认的部分**（确认「你好」后再打 ma、
    // 点首位「你好吗」⇒ 字段里 你好+你好吗）。裁决原文：「输入确认后，再输入就是新的
    // 开始，不要和之前的拼接」。撤除时**只删行为、保留 candidates_scheme.rs 的文件拆分**
    // （删了它本文件重新越 500 行门禁）。回归钉：tests/association.rs。
    // 空输入守卫必须**先于**模式分派：`ranking_invariants.rs` 与 `adversarial_input.rs`
    // 都对「含 Symbol 的 5 模式 + 空输入」断言无候选（`search("")` 会返回全部条目）。
    if input.is_empty() {
        return Vec::new();
    }
    // 符号模式（用户裁决）：候选**只出符号**，不与词库合并。合并版本里符号永远赢不了 ——
    // dun 组汉字带精确加成（score ≈ 3.9e9 + exact_bonus），符号未学习态 score 0，
    // 被 DEFAULT_TOP_N 与 FETCH_LIMIT 双重截掉，模式开关等于没生效。
    // 顺序即 `symbols.search` 的关键字序（不重排：这个模式的语义是「只剩符号」）。
    if mode == Mode::Symbol {
        return symbols
            .search(input)
            .into_iter()
            .take(limit)
            .map(|s| {
                (
                    Candidate {
                        text: s.text.clone(),
                        kind: if s.emoji {
                            CandidateKind::Emoji
                        } else {
                            CandidateKind::Symbol
                        },
                        score: rank_score(0, learner.freq_of(&s.text), boost),
                    },
                    // 只出符号、没有词库主块 ⇒ 分档元数据恒 `None`（本臂不经过三档）。
                    None,
                )
            })
            .collect();
    }
    if !matches!(mode, Mode::Pinyin | Mode::Traditional) {
        return Vec::new();
    }
    let input_len = input.len();
    // 精确等长匹配加成 = boost/2。boost = max(USER_BOOST, 词典最大静态词频×2)
    // （见 Engine::with_dictionaries），故 boost/2 ≥ 最大静态词频 —— 任何精确匹配（哪怕
    // freq=0）都排在任何前缀扩展之前：输入 xian 不再由 xiang 的「想」占首位。
    // 而一次选词的最小收益是 1×boost > boost/2，学过的前缀扩展词仍能反超（缘起 #3 不受损）。
    //
    // ⚠️ **2026-10-14 起这条加成只是「主查询内部的分数」，不再是最终次序**：候选三档
    // （`candidates_tiers.rs`，用户裁决）会把它排出来的主块切成「常用单字 → 词组 → 联想」。
    // 它仍管着**同一档内部**的顺序，也仍把「想（xiang）」挡在 `xian` 的首位之外（想 落进
    // 联想档，因为拼音更长）；被让到词组之后的只有精确单字里的**生僻尾巴**。
    let exact_bonus = boost / 2;
    let hanzi = |e: Entry, exact: bool| Candidate {
        text: e.word.clone(),
        kind: CandidateKind::Hanzi,
        score: rank_score(e.freq, learner.freq_of(&e.word), boost).saturating_add(if exact {
            exact_bonus
        } else {
            0
        }),
    };
    // ── 多音字补读音（2026-10-15 用户报障「单独打『么』候选词没有」）──────────────
    // 词库缺的读音（`么` 没有 `mo`、`绿` 没有 `lu`…）在这里补上：条目**按精确匹配**算
    // （它确实是该读音）并吃 `exact_bonus`，词频已由生成器换算到**词库量纲**
    // （见 `polyphone.rs` 头注）。表里只有「词库真缺」的条目 ⇒ 去重交给下面的 `seen`。
    // 模式门（2026-10-15）：只在**拼音（简体）模式**补 —— 表是按简体 luna 建的，
    // 繁体模式查的是 trad 库、它自己有自己的读音表；把简体条目补进繁体候选是错的
    // （实测：繁体 `yao` 的首位被补出来的 `麼`… 不，被补出来的简体条目抢走，
    //  `ranking_quality::trad_ranking_quality` 当场红）。
    let dict_max = dict.max_freq(); // 量纲判据 + 缩放（见 `polyphone.rs`）
    let patch: Vec<(Candidate, Option<Meta>)> = crate::polyphone::hits(input)
        .iter()
        .filter(|_| mode == Mode::Pinyin && crate::polyphone::applies(dict_max))
        .map(|(word, freq)| {
            let user = learner.freq_of(word);
            let freq = crate::polyphone::scaled_freq(*freq, dict_max);
            (
                Candidate {
                    text: (*word).to_string(),
                    kind: CandidateKind::Hanzi,
                    score: rank_score(freq, user, boost).saturating_add(exact_bonus),
                },
                Some(Meta {
                    pinyin_len: input_len,
                    exact: true,
                    learned: user > 0,
                    tail: false,
                }),
            )
        })
        .collect();
    // 前缀扩展层按静态词频截到 PREFIX_MATCH_CAP（精确层不截）；取舍见 dictionary.rs 同常量。
    //
    // 主查询候选**连同分档元数据**一起收集（2026-10-14）：条目一出词库，「拼音多长、
    // 是不是精确」就丢了 —— 排完序再想分档就分不出来。其余档（模糊/简拼/纠错/逐音节汤/
    // 符号）不带元数据，分档时落「主块尾部」，相对次序一字不动。
    let mut merged: Vec<(Candidate, Option<Meta>)> = dict
        .query_prefix_top(input, PREFIX_MATCH_CAP)
        .into_iter()
        .map(|e| {
            let exact = e.pinyin_len == input_len;
            let meta = Meta {
                pinyin_len: e.pinyin_len,
                exact,
                // 学过的一律排在三档之前（第 0 档）—— 见 `candidates_tiers.rs` 头注。
                learned: learner.freq_of(&e.word) > 0,
                tail: false,
            };
            (hanzi(e, exact), Some(meta))
        })
        .collect();
    merged.extend(patch);
    // 多音节整串无命中时按音节逐段补候选（segment 此前是死代码）：
    // nihao → [你][好] 逐字可选；单字母音节跳过避免噪音。
    // 每音节仅取 top 3：luna 繁体词库下全量并入会把生僻字顶进 top-8。
    //
    // ⚠️ 本档只在**没有可回退前缀**时才跑到（上面那段：完整音节前缀有命中就整档接管，
    // 判定见 `segment_fallback`）—— 它现在管的是「首段切出来不是完整音节」这类输入
    // （如 `zhonx`：`zhon` 是前缀不是音节 ⇒ 回退不成立 ⇒ 逐音节汤仍在）。
    //
    // 这个 top 3 必须按**含 learner boost 的最终分**排，不能下推给
    // `dict.query(&syl, 3)` —— 后者截的是静态词频前 3，学过的常用字会在加 boost
    // 之前就被丢掉（用例见 tests/ranking_invariants.rs::per_syllable_fallback_picks_top3_by_final_score）。
    // 与下方 85-86 行主路径是同一条不变式，此处曾违反。
    // ── 模糊音：整串变体（常开；表与变体生成见 fuzzy.rs）──────────────────────
    // 先算，因为它决定逐音节回退里还要不要再做模糊（见下）：**更具体的匹配优先** ——
    // 与上面「整串命中就不逐音节」是同一条分层规则。
    //
    // **变体命中一律不算精确匹配**（`exact = false`）：`e.pinyin_len == input_len` 这条
    // 判定对变体查询同样成立 —— `n↔l` / `f↔h` 是保长替换（ni 打回 li，键长都是 2），
    // 变体的**前缀扩展**更会因键长巧合命中（输入 `shu` 的变体 `su` 打回键 `suo`）。
    // 误判的代价是吃满 exact_bonus，与真精确匹配同档硬碰词频（真实词频：你 ni
    // 3999128899 > 裏 li 3998755570，只差 0.009%），模糊档内部也不再按真实词频排。
    // 加成必须为 **0**（非精确同档）：任何候选 ≤ max_freq ≤ boost/2 = exact_bonus 是
    // 「精确 ≥ 前缀扩展」那条不变量的结构性保证，模糊档因此按真实词频排。
    // ⚠️ 此处曾写「给半额会让 `ranking_invariants.rs` 的 proptest 随机红」——**实测不复现**
    // （半额版连跑 10 次 10/10 绿：重叠文本被上面的 retain 先去重，逐音节路径又没有精确
    // 候选可违）。结论（0 加成）不变，但**那条机制不许再当理由引用**。
    // `opts.fuzzy = false`：整串变体与逐音节兜底**一起**关（两个入口同一开关，只关一个
    // 会让 `zongguo` 这类"整串查不到"的输入仍从逐音节拿到模糊结果）。纠错不在本档。
    let near_variants: Vec<String> = if opts.fuzzy != 0 {
        fuzzy::input_variants(input, opts.fuzzy)
    } else {
        Vec::new()
    };
    let mut fuzzy: Vec<Candidate> = if !near_variants.is_empty() {
        near_variants
            .iter()
            // 同样走上限：变体是全串替换，`ni` 的变体 `li` 会命中 3 万余条 ——
            // 全量物化与主层同一量级的开销（实测 17.9ms，2026-10-11）。
            .flat_map(|v| dict.query_prefix_top(v, PREFIX_MATCH_CAP))
            .map(|e| hanzi(e, false))
            .collect()
    } else {
        Vec::new()
    };
    // 纠错档（2026-10-13）：单独一档、排在简拼之外的最外档 —— 为什么不许并进 `fuzzy`
    // 与「混合参与排序」的关系，全在 `correction::tier_hits` 的文档里（实现与那段注释
    // 一起搬去了 `correction.rs`：本文件 521/500 撞门禁，2026-10-14 拆）。
    let mut corr: Vec<Candidate> = if opts.correction {
        correction::tier_hits(dict, input, learner, boost)
    } else {
        Vec::new()
    };
    // 分词回退的「有候选就别接管」判据（下一条注释）：纠错命中也算候选 —— 与它此前
    // 混在 fuzzy 里时的语义一致（`nihap` 纠出「你好」后不许再被 `niha` 的整档接管）。
    let wide_hits = !fuzzy.is_empty() || !corr.is_empty();
    // ── 分词回退（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）────────────
    // 整串**没有精确命中**、整串模糊命中也为空时，候选换成**第一个音节**的整档
    // （递归调本函数：精确/模糊/简拼/符号的分档规则全是同一条，不另起一套排序），
    // 并**取代**下面的逐音节汤 —— 汤是「逐字可选」（你/好），回退是「先把这个词打出来」
    // （`nihaob` → 你好一族）。选中后缓冲留剩下的后缀接着编（收尾见 engine_segment.rs）。
    //
    // ⚠️ 判据是「没有**精确**整串命中」，**不是**「候选池为空」（2026-10-18 用户
    // 报障「逐字确认没效果」）：前缀扩展层会把「比输入更长的词」也算成候选
    // （`zhongguor` 命中 `zhongguoren`），旧判据 `merged.is_empty()` 因此把它挡在
    // 回退之外，用户只看到「中国人」，永远进不了第一个字的整档。`Meta.exact` 只对
    // 主查询/多音字补丁的精确条目为真 —— 用它判。
    //
    // 判定的唯一实现在 `segment_fallback`（收尾层调的是同一个函数，**不另存状态**：
    // 结论由缓冲自证，不依赖「先取候选、再选中」的调用次序 —— 与 scheme_readings
    // 同一条纪律）。「模糊/纠错精确命中优先」的边界也在 `segment_fallback` 内部
    // （`has_exact`；前缀扩展不算）—— 这里**不再**用 `wide_hits` 拦：它只表示
    // 模糊/纠错候选池非空，前缀扩展也会把它点亮，正是 2026-10-18 报障的另一半。
    let exact_hit = merged
        .iter()
        .any(|(_, meta)| meta.as_ref().is_some_and(|m| m.exact));
    let mut seg_used = false;
    if !exact_hit
        && input.chars().count() > 1
        && opts.segment_fallback
        && let Some(fb) = segment_fallback(dict, input, opts.fuzzy, opts.correction)
    {
        // 递归深度 ≤ 1：「前缀有命中」是入选条件，到了里面 `merged` 必非空。
        merged = rank_and_pick_with(dict, symbols, learner, &fb.prefix, mode, limit, boost, opts)
            .into_iter()
            .map(|c| (c, None))
            .collect();
        seg_used = true;
    }
    // 逐音节汤与分词回退同一开闸（`segment_fallback`）：五笔关闸后这里也不许漏 ——
    // 它是同一族「按拼音音节猜」的档（2026-10-14，见 RankOptions::segment_fallback）。
    if merged.is_empty() && input.chars().count() > 1 && !seg_used && opts.segment_fallback {
        for syl in segment(input) {
            if syl.chars().count() < 2 {
                continue;
            }
            let mut syl_cands: Vec<Candidate> = dict
                .query_prefix_top(&syl, PREFIX_MATCH_CAP)
                .into_iter()
                .map(|e| {
                    // 精确性按**音节**判（该路径下"输入"就是音节），与主路径同一条规则。
                    let exact = e.pinyin_len == syl.len();
                    hanzi(e, exact)
                })
                .collect();
            syl_cands.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
            merged.extend(syl_cands.into_iter().take(3).map(|c| (c, None)));
            // 逐音节模糊：**只在整串变体一无所获时**才用。两库都有连写词组（新 luna 简体
            // 词组、trad），整串变体会带回「中国/中國」，那条更具体；再叠逐音节的「中/种/種」
            // 会把「中国」挤出 8 槽（实测 trad 7→10）。
            //
            // 整段回退（含上面那半段**不加模糊**的逐音节精确查询）非有不可 —— 这是**实测的
            // 前置条件**，不是推断：新 luna 打 `zongguo` 时 `query_all("zongguo")` 是 **0 行**
            // ⇒ `merged` 为空 ⇒ 回退成立；该输入 8 槽里的 国/國/过/总/總/宗 **六条只能**由
            // 逐音节子查询给出（整串变体 `zhongguo` 的 81 行里没有它们）。`sengseng` 同理：
            // 僧/䒏/鬙 来自逐音节，深深/神圣/生生… 来自整串变体，同一栏里两条机制并存。
            // ⚠️ 曾在此写过「luna 词组键空格分隔、21759 条全带空格」——旧 luna 的事实，
            // 2026-09-27 换简体词组源后已失效。
            //
            // ⚠️ **本块的可见性已被简繁两库的连写词组压得很低**（2026-09-27 实测，别据此删）：
            // - trad 仍承重：`nihao` 的 8 槽第 7/8 位「裏/裡」只能来自 `ni→li` 这条变体查询
            //   （`mingtian`→民、`zuotian`→桌/着、`yihou`→否/紑 同理）。
            // - 新 luna 上，**47 个常用词的实测里它一个候选都没贡献** —— 常用词现在多有条目键
            //   （`mingtian` 直接命中），整串变体也就非空 ⇒ `wide_hits` 拦下本块。它并未失效：
            //   64×64 音节对的穷举里仍有 1285 个输入以它为**唯一**模糊来源（如 `ninv`→
            //   `nin→lin/ling`、`nifei`→`fei→hei`），只是那些多不是词。
            // 每音节 ≤3 次额外查询且同样被 take(3) 截住，无笛卡尔积。
            // 逐音节相**只做模糊音**（2026-10-13 定案）：纠错的同类形态已被**整串单笔误
            // 枚举**覆盖（correction::input_corrections 的单段轮转），塞进本相是重复出查询
            // ——实测「aaaaaaaa」8 个音节各查 8 个变体，单键 +345ms（本机负载下）。
            let syl_variants: Vec<String> = if opts.fuzzy != 0 {
                fuzzy::variants(&syl, opts.fuzzy)
            } else {
                Vec::new()
            };
            let near_on = opts.fuzzy != 0;
            if near_on && !wide_hits {
                // ⚠️ **变体查询本身要封顶**（不是只封 take(3) 的结果）：纠错的逐音节变体
                // 可以十几二十个（单字母音节一次插入就出一大把合法音节），每个变体一次
                // 前缀查询（十几 ms 量级）—— 2026-10-13 实测这套没封时「aaaaaaaa」单键
                // 1.2s（本机负载下）。封到 8：给 fuzzy（每音节 ≤4）留余地，纠错的多余
                // 变体砍掉（它们按确定性序排在后面，砍的都是低优先形态）。
                let mut syl_fuzzy: Vec<Candidate> = syl_variants
                    .iter()
                    .take(8)
                    .filter(|v| *v != &syl)
                    .flat_map(|v| dict.query_prefix_top(v, PREFIX_MATCH_CAP))
                    .map(|e| hanzi(e, false))
                    .collect();
                syl_fuzzy.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
                merged.extend(syl_fuzzy.into_iter().take(3).map(|c| (c, None)));
            }
        }
    }
    fuzzy.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));

    // ── 简拼：缩写展开（常开；展开选择与上限见 jianpin.rs）────────────────────
    // 与模糊同一分档规则、同一「非精确」约定（`exact = false`）：纯缩写输入下这个 flag
    // 本无机会成立 —— 每个音节 ≥ 2 字母 ⇒ 展开串比缩写长，`pinyin_len` 不可能等于输入
    // 长度。显式写死是因为这条判定对**替身词库**（键任意）同样成立，别让它去赌数据。
    let mut jianpin: Vec<Candidate> = if opts.jianpin {
        jianpin::expansions(dict, learner, boost, input)
            .iter()
            .flat_map(|v| dict.query_all(v))
            .map(|e| hanzi(e, false))
            .collect()
    } else {
        Vec::new()
    };
    jianpin.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));

    // 单字符时仅并入 emoji：符号英文关键字前缀（如 comma→顿号）泄漏进拼音候选是噪音
    // ——真机 "c" 键唯一候选曾是顿号。emoji 保留作趣味反馈。过滤由 search 内部完成：
    // 单键 183 条命中只克隆 16 条 emoji，其余不再被造出来再丢掉。
    let sym_hits = if input.chars().count() == 1 {
        symbols.search_emoji(input)
    } else {
        symbols.search(input)
    };
    for s in sym_hits {
        // 符号不带拼音，故不参与「精确/扩展」分层（其分数本就只有学习权重）——
        // 分档侧同样：`None` 元数据 ⇒ 落主块尾部（与旧次序里它们按微小分垫底等价）。
        merged.push((
            Candidate {
                text: s.text.clone(),
                kind: if s.emoji {
                    CandidateKind::Emoji
                } else {
                    CandidateKind::Symbol
                },
                score: rank_score(0, learner.freq_of(&s.text), boost),
            },
            None,
        ));
    }
    // 不能下推 limit 到词库：学过的低静态词可反超截断线外的词，
    // 全量收集 + 排序是唯一正确方案（select 用有限 limit 只限 FFI 载荷）。
    // 故走 query_all：词库不必先按词频排一遍（下面这一排就是权威序）。
    merged.sort_by(|a, b| b.0.score.cmp(&a.0.score).then(a.0.text.cmp(&b.0.text)));
    let mut seen = std::collections::HashSet::new();
    merged.retain(|(c, _)| seen.insert(c.text.clone()));
    // ── 简体优先（见 `RankOptions::prefer_simplified`）**先于**分档 ──────────────
    // 两个步骤都是「按位稳定分区」：谁在后面，谁的次序就是最终次序。它必须在**前面**
    // （2026-10-14 实测：反过来的话它会把三档整段重排、档位消失）—— 跑在前面还让三档
    // 继承它（档内保持简体在前）。追加档（模糊/简拼/纠错）不在主块里，见下面那次调用。
    if opts.prefer_simplified {
        crate::trad_chars::demote_traditional_by(
            &mut merged,
            boost / 2,
            |(c, _)| c.score,
            |(c, _)| &c.text,
        );
    }
    // 主块的长度：分档只重排、不改长度（下面拿它对**追加档**那一截再跑一次简体优先）。
    let main_len = merged.len();
    // ── 候选三档（2026-10-14 用户裁决）────────────────────────────────────────
    // 「优先给符合拼音的候选字，然后是词组，最后是联想的」—— 排完序、去完重之后切档，
    // 档规则与理由全在 `candidates_tiers.rs`。`opts.tiers = false`（笔画/五笔）时
    // 一步不动：码串不分档。
    if opts.tiers {
        merged = tiers::apply(merged);
    }
    // 分档完毕：后面几档（模糊/简拼/纠错）不带元数据、按既有规则追加，故这里剥掉它。
    let mut merged: Vec<Candidate> = merged.into_iter().map(|(c, _)| c).collect();
    // 分档（用户裁决）：精确/前缀候选**非空** ⇒ 模糊结果**追加在最后**，既有候选的集合
    // 与顺序一字不变（= 开模糊后的列表以未开模糊的列表为前缀）；**为空** ⇒ 模糊结果就是
    // 主结果（打 zongguo 想打「中国」不该退到列表末尾）。两分支同一份代码：先追加再截断，
    // 故截断永远先砍模糊结果，精确档不受影响。
    fuzzy.retain(|c| seen.insert(c.text.clone()));
    if merged.is_empty() {
        merged = fuzzy;
    } else {
        merged.extend(fuzzy);
    }
    // 简拼档比模糊档更靠外（模糊是同音/近音的整串替换，简拼是缩写），故接在模糊之后。
    // 分档规则与上面逐字相同：**精确档为空时它就是这个输入的主结果** —— 打 `nh` 的人想要
    // 的就是「你好」，退到列表末尾等于没做（`nh` 这类纯缩写在词库里本就查不出东西，
    // 这是产品上最常见的那条路径，不是边角）。非空时只追加，截断先砍它。
    jianpin.retain(|c| seen.insert(c.text.clone()));
    if merged.is_empty() {
        merged = jianpin;
    } else {
        merged.extend(jianpin);
    }
    // 纠错档：**最外层**（见 `correction::tier_hits`）。分档规则同上面逐字相同：只有
    // 前三档全空时它才当主结果（`nihap` 的整串查不到东西，纠出的就是唯一来路）。
    corr.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
    corr.retain(|c| seen.insert(c.text.clone()));
    if merged.is_empty() {
        merged = corr;
    } else {
        merged.extend(corr);
    }
    // 简体优先（见 RankOptions::prefer_simplified）：**截断之前**分区 —— 截断后才排
    // 等于「被截掉的简体候选永远回不来」。
    // 追加档那一截的简体优先（主块那一截已在分档之前跑过 —— 这里**只碰追加部分**，
    // 碰主块会把档位重排掉，见上面那段）。
    if opts.prefer_simplified {
        crate::trad_chars::demote_traditional(&mut merged[main_len..], boost / 2);
    }
    merged.truncate(limit);
    // 出函数时统一补 `None`：后几档（模糊/简拼/纠错）本来就是「非主块」，不带元数据 ——
    // 双拼侧拿它当「不参与分档」的判据（那边的分档在跨解析合并之后做，见 `_meta` 头注）。
    merged.into_iter().map(|c| (c, None)).collect()
}

// 分词回退一族（`SegmentFallback` / `segment_fallback` / `raw_suffix`）拆去
// `candidates_segment.rs`（2026-10-14，500 行门禁）—— 判定函数走这里转发（对外路径
// 不变）；结构体 `SegmentFallback` 只由返回值类型推断使用，故只在本模块内可见。
pub(crate) use crate::candidates_segment_impl::segment_fallback;

// 按键类展开一族（9/14 键）在 `candidates_scheme.rs`（2026-10-12 拆出，拆分保留）。
pub use crate::candidates_scheme_impl::rank_and_pick_scheme;

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 router_tests.rs 同惯例。
#[cfg(test)]
#[path = "candidates_tests.rs"]
mod tests;
