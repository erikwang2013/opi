// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 主候选装配一族（2026-10-16 从 `engine.rs` 拆出 —— 075a382 给 9 键路加宽松兜底档
//! 参数后那里越了 500 行门禁；与 `engine_punct.rs` / `engine_wubi.rs` 同一套拆分先例）。
//!
//! 拆的是 [`Engine::candidates`] 一个方法：两条排名路的分流（9/14 键按键类展开、
//! 全键盘前缀管线）+ 双拼/五笔专属路的委派 + 笔画/五笔无词典时的回退姿态（`plain`）。
//! 语义一字未动 —— 搬移只动了它所在的文件；各臂的判据随注释一起走。
//!
//! 字段可见性：本文件要用 `Engine` 的一批配置/会话字段（`composer` / `symbol_browse` /
//! `learner` / `symbols` / `user_boost` / `stroke_dict` / `wubi_dict` / `fuzzy` /
//! `correction` / `jianpin` / `scheme_readings`）；尚未 `pub(crate)` 的按 `engine_punct.rs`
//! 的先例升级（拆分裂的代价，别再为此往 engine.rs 塞回方法）。

use crate::candidates::{
    Candidate, CandidateKind, RankOptions, rank_and_pick_scheme, rank_and_pick_with,
};
use crate::composer::Mode;
use crate::engine::Engine;
use crate::scheme::Scheme;

impl Engine {
    /// 正常候选装配（**不含**自定义短语：公开入口 [`Engine::candidates`] 在
    /// `custom_phrases.rs`，算完后把精确命中的短语前插）。
    pub(crate) fn candidates_base(&self, limit: usize) -> Vec<Candidate> {
        // 每键一份的导入层预算（2026-10-18）：主查询 / 模糊 / 纠错 / 简拼 / 逐音节 /
        // 分词回退共享同一截止，超时整层丢弃；guard 在本方法返回时恢复（不粘滞）。
        let _budget = self.dict.begin_query_budget();
        // 浏览态：候选 = 浏览器当前层（类目 / 该类目的符号）；分页照旧归宿主（skip/take）。
        if self.symbol_browse.is_active() {
            return self.symbol_browse.candidates(limit);
        }
        let s = self.composer.session();
        // 英文单词联想（2026-10-18）：缓冲是字母串、数据内嵌在 engine-core，不走主词库。
        // `candidates(limit)` 的 limit 由宿主决定（路由层 FETCH_LIMIT = 不设上限），
        // 排序只对命中项做（见 english_words.rs）。
        if s.mode == Mode::English {
            // 携带缓冲（中→英保留的拼音）不是英文前缀：不出候选 —— 空格按「无候选」
            // 提交原文、数字直传，与旧直传语义一致。
            if self.composer.carried_from().is_some() {
                return Vec::new();
            }
            // 单字母原文候选（2026-10-19）：缓冲恰为一个 ASCII 字母时，把用户输入的
            // 字母本身（保留大小写）放在首位；它占 1 个名额，词库只用 limit-1。
            // 词库里的同形词（`a` / `I`）按大小写不敏感去重，同一字母只出现一次。
            let single =
                limit > 0 && s.buffer.len() == 1 && s.buffer.as_bytes()[0].is_ascii_alphabetic();
            let mut cands = crate::english_words::candidates(&s.buffer, limit, &self.learner);
            if single {
                cands.retain(|c| !c.text.eq_ignore_ascii_case(s.buffer.as_str()));
                cands.truncate(limit - 1);
                cands.insert(
                    0,
                    Candidate {
                        text: s.buffer.clone(),
                        kind: CandidateKind::English,
                        // 与词频不同量纲：原文必须排在所有联想词之前。
                        score: u64::MAX,
                    },
                );
            }
            // emoji 输入匹配（2026-10-18）：追加在英文词之后、受 limit 约束 ——
            // 单词候选填满时一个也不进，`hello` 这类词的首位不受影响。
            crate::emoji_keywords::append_to(&mut cands, &s.buffer, limit);
            return cands;
        }
        // 提交后联想（2026-10-27）：缓冲空时候选行 = 提交时算好的联想项。
        // 状态只由 build_association 建立，输入入口清空（见 engine_association.rs）。
        if s.buffer.is_empty()
            && let Some(a) = &self.association
        {
            return a.items.iter().take(limit).cloned().collect();
        }
        // 输入方案分流（2026-10-09）：只在「会打拼音缓冲」的两个模式下有意义
        // （English/Number/Symbol 的缓冲不是拼音串）。
        if matches!(s.mode, Mode::Pinyin | Mode::Traditional) {
            // 英→中保留的英文词不是拼音：不出拼音候选（2026-10-09 裁决的另一半）——
            // 否则 "he" 会被当拼音匹配出「和」。空格按既有「无候选」语义提交原文。
            if self.composer.carried_from() == Some(Mode::English) {
                return Vec::new();
            }
            match self.scheme() {
                // 9/14 键：按键类展开 —— 前缀查询表达不了（数字串不是任何拼音键的
                // 前缀），走全量遍历 + 类匹配（candidates::rank_and_pick_scheme）。
                // 「精确加成」按**键长相等**给（与全键盘的 pinyin_len == 输入长度同义）。
                Scheme::Nine | Scheme::Fourteen => {
                    let (cands, readings) = rank_and_pick_scheme(
                        self.active_dict(),
                        &self.learner,
                        &s.buffer,
                        limit,
                        self.user_boost,
                        self.scheme(),
                        self.active_pin(),
                        // 简体优先：只简体模式开（繁体数据繁体在前）；`true` = 宽松兜底档（9 键）
                        s.mode == Mode::Pinyin,
                        true,
                    );
                    // 读数随每次取候选刷新（逐条、已分词），**并与当时的缓冲绑存**
                    // （见 display_buffer：缓冲一变这对就作废，不许把旧缓冲的读数报给新状态）。
                    *self.scheme_readings.borrow_mut() = (
                        s.buffer.clone(),
                        readings
                            .iter()
                            .map(|py| crate::pinyin::segment(py).join("'"))
                            .collect(),
                    );
                    return cands;
                }
                // 双拼（2026-10-13）：解码 + 每解析一次全拼排名 + 合并。两块双拼专属
                // 代码（本臂 + 布局 setter）在 `engine_shuangpin.rs`（500 行门禁拆出，
                // 同 `engine_segment.rs` 先例）。
                Scheme::Shuangpin => {
                    return self.candidates_shuangpin(&s.buffer, s.mode, limit);
                }
                // 笔画：换词典 + **关掉模糊音/简拼**（两者对 hspnz 串没有意义，
                // 开着只会把不相干的音节捞进来）。其余与全键盘完全同路。
                Scheme::Stroke => {
                    if let Some(d) = &self.stroke_dict {
                        return rank_and_pick_with(
                            d.as_ref(),
                            &self.symbols,
                            &self.learner,
                            &s.buffer,
                            s.mode,
                            limit,
                            self.user_boost,
                            RankOptions {
                                fuzzy: 0,
                                correction: false,
                                jianpin: false,
                                prefer_simplified: s.mode == Mode::Pinyin,
                                // 笔画串不是音节，分词回退对 hspnz 天然惰性 —— 保持
                                // 既有值（true），别顺手统一成 false（那是五笔的理由）。
                                segment_fallback: true,
                                // 组合识别与三档同理：hspnz 是编码不是拼音。
                                composition: false,
                                // 三档同理：hspnz 是编码不是拼音（见 RankOptions::tiers）。
                                tiers: false,
                            },
                        );
                    }
                    // 没装笔画词典：落到下面普通路径（候选基本为空）——
                    // 与 trad 缺失同一种「可选增强」姿态，不在引擎里报错。
                }
                // 五笔（2026-10-14，M4 3b）：纯前缀管线，候选装配（含混输合并与
                // 候选个数档位）在 engine_wubi.rs。槽为空 ⇒ 落到下面普通路径
                // （**如实**：不许假装有码表 —— 与 stroke 同一条「可选增强」姿态）。
                Scheme::Wubi => {
                    if self.wubi_dict.is_some() {
                        return self.candidates_wubi(&s.buffer, s.mode, limit);
                    }
                }
                Scheme::Full => {}
            }
        }
        // 笔画的**无词典回退支**也要守「模糊音/纠错/简拼对 hspnz 串没有意义」：
        // 上述装词典那支早关了这三个，落回本支时却只吃了全局开关 —— 模糊音在笔画串上
        // 极少命中所以一直没露；2026-10-13 加纠错后 `hs` 被纠出错音节，
        // `stroke_scheme_without_dict_is_empty_not_error` 当场抓到（红得正确）。
        // 五笔同族（2026-10-14）：无码表时也落本支，三档同关 + **分词回退也关**
        // （码串没有音节，理由见 engine_wubi.rs 头）—— `plain` 是这两条路的共同判据。
        let plain = matches!(self.scheme(), Scheme::Stroke | Scheme::Wubi);
        // 单键首击的有限窗口：只物化首位段，结果与全量列表逐条同前缀（见模块头）。
        // 主块够一屏时符号/表情都在后面，提前返回不会少候选。
        if !plain
            && limit != usize::MAX
            && s.buffer.chars().count() == 1
            && matches!(s.mode, Mode::Pinyin | Mode::Traditional)
            && let Some(fast) = crate::candidates_finite::single_key_prefix(
                self.active_dict(),
                &self.learner,
                &s.buffer,
                limit,
                self.user_boost,
                s.mode == Mode::Pinyin,
            )
        {
            return fast;
        }
        let mut cands = rank_and_pick_with(
            self.active_dict(),
            &self.symbols,
            &self.learner,
            &s.buffer,
            s.mode,
            limit,
            self.user_boost,
            RankOptions {
                fuzzy: if plain { 0 } else { self.fuzzy },
                correction: self.correction && !plain,
                jianpin: self.jianpin && !plain,
                prefer_simplified: s.mode == Mode::Pinyin,
                segment_fallback: self.scheme() != Scheme::Wubi,
                // 组合识别只对真拼音缓冲有意义：`plain`（笔画/五笔码串）关。
                composition: !plain,
                // 三档跟着 `plain` 走（笔画/五笔 = 码类，不分档）—— 这里正是 **全键盘
                // 拼音**那条路（Scheme::Full），`plain` 为假 ⇒ 分档生效（2026-10-14 裁决）。
                tiers: !plain,
            },
        );
        // 输入即得表情（2026-10-18）：只对**26 键拼音/繁体**这条路追加（`plain` 的
        // 笔画/五笔是编码串、Symbol 模式只出符号，都不该按拼音关键字猜表情）。
        // 位置固定在文本候选之后 ⇒ 受 limit 约束、绝不顶掉文本首候选。
        // ⚠️ 边界：9/14 键与双拼在上面各自 return，**本轮不覆盖**（数字/编码缓冲
        // 不是关键字前缀；要覆盖得先解码成拼音，见 tests/emoji_match.rs 文件头）。
        if !plain && matches!(s.mode, Mode::Pinyin | Mode::Traditional) {
            crate::emoji_keywords::append_to(&mut cands, &s.buffer, limit);
        }
        cands
    }
}
