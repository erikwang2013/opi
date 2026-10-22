// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **双拼**的引擎侧一半（2026-10-13 用户裁决「主流同款全套」）：布局设置 + 候选分支。
//! 独立成文件是 500 行门禁（`engine.rs` 加完这两块后 536 行越线，见
//! `engine_segment.rs` / `engine_symbol_browse.rs` 同一套先例）。
//!
//! 解码语义在 `shuangpin.rs`（键序 → 全拼串，DFS），键位表在 `shuangpin_tables.rs`
//! （生成物）。本文件只做「解码结果 → 候选」这一段：**每个解析走一遍标准全拼管线**
//! （`rank_and_pick_with`），合并去重后按同一分档规则排 —— 解析命中的是真拼音，
//! 精确加成照吃（打 nihk 出「你好」就该是精确档）；模糊/纠错/简拼对解码后的拼音同样
//! 生效（它们是拼音层的事），故 opts 用全套。
//!
//! 字段可见性：本文件要用 `Engine::user_boost`（private → `pub(crate)`），其余
//! （`active_dict`/`symbols`/`learner`/`scheme_readings`/`fuzzy`/`correction`/`jianpin`）
//! 早因 `engine_segment.rs` / `engine_user_words.rs` 升过 —— 同一条拆分裂代价。

use crate::candidates::{Candidate, RankOptions, rank_and_pick_meta};
use crate::candidates_tiers_impl as tiers;
use crate::candidates_tiers_impl::Meta;
use crate::composer::Mode;
use crate::engine::Engine;

impl Engine {
    /// 双拼布局（0=自然码，1=微软，2=小鹤，3=智能ABC，4=紫光，5=拼音加加）。
    /// **过大钳到末位**（不是钳到 0：越界的值最可能来自版本领先的宿主，落到末位
    /// 比把用户静默跳回自然码更接近它想表达的）；负值由 FFI 侧先 `max(0)` 再进来
    /// （usize 转换会把负数变成天文数字，`min` 恰好兜住 —— 但那是巧合，不当地基）。
    /// **不清缓冲**：布局只改「键序怎么解码」，缓冲里已有的键序按新布局重解即可
    /// （与 `set_scheme` 的区别正是在这里 —— 那条必须清）。
    pub fn set_shuangpin_layout(&mut self, index: usize) {
        self.shuangpin_layout = index.min(crate::shuangpin::layout_count() - 1);
    }

    pub fn shuangpin_layout(&self) -> usize {
        self.shuangpin_layout
    }

    /// 双拼候选（`Engine::candidates` 的 `Scheme::Shuangpin` 臂）。
    pub(crate) fn candidates_shuangpin(
        &self,
        buffer: &str,
        mode: Mode,
        limit: usize,
    ) -> Vec<Candidate> {
        let parses = crate::shuangpin::decode_keys(buffer, self.shuangpin_layout);
        // 分档元数据跟着候选一起跨解析合并（2026-10-14）：**不许**在每次解析内部就分档
        // —— 下面那次全局重排会把档位顺序抹平（分档必须发生在合并之后，见 `_meta` 与
        // `candidates_tiers.rs`）。故每次解析传 `tiers: false`、这里合并完自己调 `apply`。
        let mut merged: Vec<(Candidate, Option<Meta>)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        // 解析数封顶另有一道：MAX_PARSES 已封 8，这里再取前 4（每次击键
        // 每个解析都是一次全量排名）。
        for p in parses.iter().take(4) {
            for (c, meta) in rank_and_pick_meta(
                self.active_dict(),
                &self.symbols,
                &self.learner,
                p,
                mode,
                limit,
                self.user_boost,
                RankOptions {
                    fuzzy: self.fuzzy,
                    correction: self.correction,
                    jianpin: self.jianpin,
                    prefer_simplified: mode == Mode::Pinyin,
                    // 解码结果就是**真拼音**，分词回退/逐音节汤对它照旧有效（既有行为）。
                    segment_fallback: true,
                    // 组合枚举要在**整串**解码拼音上做；逐解析的中间串关掉（2026-10-23）。
                    composition: false,
                    // 分档在**合并之后**（见上）—— 这里只收元数据。
                    tiers: false,
                },
            ) {
                if seen.insert(c.text.clone()) {
                    merged.push((c, meta));
                }
            }
        }
        merged.sort_by(|a, b| b.0.score.cmp(&a.0.score).then(a.0.text.cmp(&b.0.text)));
        // 双拼 = 拼音输入 ⇒ 三档常开（与全键盘同一条裁决；档规则见 `candidates_tiers.rs`）。
        let mut merged: Vec<Candidate> = tiers::apply(merged).into_iter().map(|(c, _)| c).collect();
        merged.truncate(limit);
        // 读数缓存（同九键那对）：展示形带 `'` 分词，与**当时的缓冲**绑存。
        *self.scheme_readings.borrow_mut() = (
            buffer.to_string(),
            parses
                .iter()
                .map(|p| crate::shuangpin::display(p))
                .collect(),
        );
        merged
    }
}
