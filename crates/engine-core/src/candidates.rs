// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::composer::Mode;
use crate::dictionary::Dictionary;
use crate::fuzzy;
use crate::jianpin;
use crate::learner::Learner;
use crate::pinyin::{is_syllable, segment};
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

/// 排序层的**开关**（用户在「属性设置」里能改的那两个；见 `Engine` 的同名字段）。
///
/// 默认 = **两个都开**：这是本层的既有行为，也是 [`rank_and_pick`] 的语义
/// （测试与旧调用方一行不动）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankOptions {
    /// 模糊音（`z/zh`、`n/l`… 整串变体 + 逐音节兜底，表见 `fuzzy.rs`）。
    pub fuzzy: bool,
    /// 简拼（声母缩写 `nh → 你好`，展开见 `jianpin.rs`）。
    pub jianpin: bool,
}

impl Default for RankOptions {
    fn default() -> Self {
        Self {
            fuzzy: true,
            jianpin: true,
        }
    }
}

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
            .map(|s| Candidate {
                text: s.text.clone(),
                kind: if s.emoji {
                    CandidateKind::Emoji
                } else {
                    CandidateKind::Symbol
                },
                score: rank_score(0, learner.freq_of(&s.text), boost),
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
    let mut merged: Vec<Candidate> = dict
        .query_all(input)
        .into_iter()
        .map(|e| {
            let exact = e.pinyin_len == input_len;
            hanzi(e, exact)
        })
        .collect();
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
    // `opts.fuzzy = false`：整串变体与逐音节兜底**一起**关（两个入口同一开关，
    // 只关一个会让 `zongguo` 这类"整串查不到"的输入仍从逐音节拿到模糊结果）。
    let mut fuzzy: Vec<Candidate> = if opts.fuzzy {
        fuzzy::input_variants(input)
            .iter()
            .flat_map(|v| dict.query_all(v))
            .map(|e| hanzi(e, false))
            .collect()
    } else {
        Vec::new()
    };
    let wide_hits = !fuzzy.is_empty();
    // ── 分词回退（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）────────────
    // 整串直接命中为空、整串模糊命中也为空时，候选换成**最长可匹配前缀**的整档
    // （递归调本函数：精确/模糊/简拼/符号的分档规则全是同一条，不另起一套排序），
    // 并**取代**下面的逐音节汤 —— 汤是「逐字可选」（你/好），回退是「先把这个词打出来」
    // （`nihaob` → 你好一族）。选中后缓冲留剩下的后缀接着编（收尾见 engine_segment.rs）。
    //
    // 判定的唯一实现在 `segment_fallback`（收尾层调的是同一个函数，**不另存状态**：
    // 结论由缓冲自证，不依赖「先取候选、再选中」的调用次序 —— 与 scheme_readings
    // 同一条纪律）。`!wide_hits` = 边界「整串模糊命中优先」：`zongguo` 有中国档时
    // 不许被 `zong` 的整档接管。
    let mut seg_used = false;
    if merged.is_empty()
        && input.chars().count() > 1
        && !wide_hits
        && let Some(fb) = segment_fallback(dict, input, opts.fuzzy)
    {
        // 递归深度 ≤ 1：「前缀有命中」是入选条件，到了里面 `merged` 必非空。
        merged = rank_and_pick_with(dict, symbols, learner, &fb.prefix, mode, limit, boost, opts);
        seg_used = true;
    }
    if merged.is_empty() && input.chars().count() > 1 && !seg_used {
        for syl in segment(input) {
            if syl.chars().count() < 2 {
                continue;
            }
            let mut syl_cands: Vec<Candidate> = dict
                .query_all(&syl)
                .into_iter()
                .map(|e| {
                    // 精确性按**音节**判（该路径下"输入"就是音节），与主路径同一条规则。
                    let exact = e.pinyin_len == syl.len();
                    hanzi(e, exact)
                })
                .collect();
            syl_cands.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
            merged.extend(syl_cands.into_iter().take(3));
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
            if opts.fuzzy && !wide_hits {
                let mut syl_fuzzy: Vec<Candidate> = fuzzy::variants(&syl)
                    .iter()
                    .filter(|v| *v != &syl)
                    .flat_map(|v| dict.query_all(v))
                    .map(|e| hanzi(e, false))
                    .collect();
                syl_fuzzy.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
                merged.extend(syl_fuzzy.into_iter().take(3));
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
        // 符号不带拼音，故不参与「精确/扩展」分层（其分数本就只有学习权重）。
        merged.push(Candidate {
            text: s.text.clone(),
            kind: if s.emoji {
                CandidateKind::Emoji
            } else {
                CandidateKind::Symbol
            },
            score: rank_score(0, learner.freq_of(&s.text), boost),
        });
    }
    // 不能下推 limit 到词库：学过的低静态词可反超截断线外的词，
    // 全量收集 + 排序是唯一正确方案（select 用有限 limit 只限 FFI 载荷）。
    // 故走 query_all：词库不必先按词频排一遍（下面这一排就是权威序）。
    merged.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
    let mut seen = std::collections::HashSet::new();
    merged.retain(|c| seen.insert(c.text.clone()));
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
    merged.truncate(limit);
    merged
}

/// 分词回退的落点：候选出 `prefix` 的整档，选中后缓冲里留 `suffix` 接着编。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SegmentFallback {
    /// 被消耗的前缀：**音节对齐**，且已去掉分词符（词库键不含 `'`，格式约定）。
    pub prefix: String,
    /// 留在缓冲里的**原文后缀**（其间的 `'` 原样保留）。
    pub suffix: String,
}

/// 分词回退的**唯一判定**（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）：
/// 给定缓冲，返回「候选该出哪一段、选完缓冲剩什么」。
///
/// 两个调用点共用它 —— 候选层（[`rank_and_pick_with`] 的回退档）与收尾层
/// （`Engine::commit_buffer_after_select`）；**纯函数，故不存状态**：收尾层不需要
/// 「先取候选、再选中」的次序假设（缓冲自证，与 `scheme_readings` 的 `(缓冲, 值)`
/// 绑存是同一条纪律）。
///
/// 判据（全成立才回退；**先算不查库的两条**，收尾层每次选中都要过一次它）：
/// 1. 缓冲不止一个字符 —— 空/单字符维持既有「无匹配」；
/// 2. 切分出来**至少两段** —— 单音节没有「分词」可言（边界：单音节无匹配维持原样）；
/// 3. **整串直接命中为空** —— 有命中 = 更具体的档，不回退；
/// 4. **整串模糊命中为空**（`fuzzy_enabled` 关掉时这条不问）—— 整串变体是整串替换、
///    比前缀更具体：`zongguo` 有「中国」档时不许被 `zong` 的整档接管；
///    然后从**最长**的音节对齐前缀往短试，第一个「每段都是完整音节 + 词库有命中」的胜出。
///
/// 「完整音节」是**门槛**（用户裁决的边界）：`qqq` / `zxcvb` 这类打错串切出来的段不是
/// 完整音节 ⇒ 不回退 ⇒ 维持无候选（「打英文」的逃生口靠它，
/// 钉子见 tests/punctuation.rs 与 tests/adversarial_input.rs）。
///
/// ⚠️ 门槛只管「段是不是音节」：首段**恰好**是完整音节的英文串会回退（`hello` → 前缀
/// `he`，留 `llo`）。这是本规则的已知形态、不是漏网 —— 这类串在旧行为里同样有候选
/// （逐音节汤也命中 `he`），而逃生口本就只对**无候选**的串开放（无候选时空格提交原文）。
///
/// 前缀串取**各段拼接**（去掉 `'`）：词库键不含分隔符，`ni'hao` 的整串直接查询本是空的，
/// 拼起来查才是用户的本意（`ni'hao` → 你好一族）。
pub(crate) fn segment_fallback<D: Dictionary + ?Sized>(
    dict: &D,
    input: &str,
    fuzzy_enabled: bool,
) -> Option<SegmentFallback> {
    if input.chars().count() <= 1 {
        return None;
    }
    let segs = segment(input);
    if segs.len() < 2 {
        return None;
    }
    if !dict.query_all(input).is_empty() {
        return None;
    }
    if fuzzy_enabled
        && fuzzy::input_variants(input)
            .iter()
            .any(|v| !dict.query_all(v).is_empty())
    {
        return None;
    }
    // 各段在拼接串里的累计字符数：`prefix_len[k-1]` = 前 k 段拼起来的长度，
    // 用来在**原文**里定位后缀从哪开始。
    let mut prefix_len = Vec::with_capacity(segs.len());
    let mut total = 0usize;
    for s in &segs {
        total += s.chars().count();
        prefix_len.push(total);
    }
    for k in (1..=segs.len()).rev() {
        if !segs[..k].iter().all(|s| is_syllable(s)) {
            continue;
        }
        let prefix: String = segs[..k].concat();
        if !dict.query_all(&prefix).is_empty() {
            return Some(SegmentFallback {
                suffix: raw_suffix(input, prefix_len[k - 1]),
                prefix,
            });
        }
    }
    None
}

/// 缓冲原文里「前 `consumed` 个非分隔符字符」之后的剩余（其间的 `'` 原样保留）：
/// `("nihaob", 5)` → `"b"`、`("ni'hao", 5)` → `""`、`("ni'hao'x", 5)` → `"'x"`。
/// 前缀串是去 `'` 的拼接，故按「数够 `consumed` 个非 `'` 字符」定位 ——
/// 分隔符归**前段**（它标记的正是这个边界）。
fn raw_suffix(input: &str, consumed: usize) -> String {
    let mut seen = 0usize;
    for (i, ch) in input.chars().enumerate() {
        if ch != '\'' {
            seen += 1;
        }
        if seen == consumed {
            return input.chars().skip(i + 1).collect();
        }
    }
    String::new()
}

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
/// `candidate_readings` 带全量（9 键左列按候选逐条显示，2026-10-09 用户裁决）。
/// 没有候选时两条都为空。14 键同路返回（它的缓冲本来就是字母串，读数字段无副作用）。
pub fn rank_and_pick_scheme<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    input: &str,
    limit: usize,
    boost: u64,
    scheme: crate::scheme::Scheme,
    pin: Option<&str>,
) -> (Vec<Candidate>, Vec<String>) {
    if input.is_empty() || limit == 0 {
        return (Vec::new(), Vec::new());
    }
    let exact_bonus = boost / 2;
    // 一趟扫描 = 一个输入串的整档（读音随条目一起暂存：pinyin 的生命周期出不了
    // visit_entries 的回调）。两趟（回退态）共用这一份。
    let scan = |input: &str| -> Vec<(Candidate, String)> {
        let input_len = input.len();
        let mut merged: Vec<(Candidate, String)> = Vec::new();
        dict.visit_entries(&mut |pinyin, word, freq| {
            if !scheme.matches(input, pinyin) {
                return;
            }
            // 钉读音（左列点一行拼音）：候选收窄到该读音一族 —— 前缀比较（`niu` 含
            // `niuyou`），pin 已在边界处去掉分词 `'`（见 `Engine::set_pinned_reading`）。
            if let Some(p) = pin
                && !pinyin.starts_with(p)
            {
                return;
            }
            let exact = pinyin.len() == input_len;
            merged.push((
                Candidate {
                    text: word.to_string(),
                    kind: CandidateKind::Hanzi,
                    score: rank_score(freq, learner.freq_of(word), boost)
                        .saturating_add(if exact { exact_bonus } else { 0 }),
                },
                pinyin.to_string(),
            ));
        });
        merged
    };
    // ── 数字空间回退（2026-10-10 扩到 9 键）：整串无命中改扫最长音节对齐键前缀的整档
    // （判定唯一实现 `scheme::scheme_fallback_len`；命中态非空 ⇒ 一行不重，重扫深度 ≤ 1）。
    let mut merged = scan(input);
    if merged.is_empty()
        && let Some(k) = crate::scheme::scheme_fallback_len(dict, input, scheme, pin)
    {
        merged = scan(&input[..k]);
    }
    merged.sort_by(|a, b| b.0.score.cmp(&a.0.score).then(a.0.text.cmp(&b.0.text)));
    let mut seen = std::collections::HashSet::new();
    merged.retain(|(c, _)| seen.insert(c.text.clone()));
    merged.truncate(limit);
    // 读音**逐条**带出（与候选同序、同一次排名）—— 调用方按需取首条或全量；
    // 分开再算一次 = 第二次全量扫描，别在调用链上再开这条路。
    merged.into_iter().unzip()
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 router_tests.rs 同惯例。
#[cfg(test)]
#[path = "candidates_tests.rs"]
mod tests;
