// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选提交的学习记账与**误选撤销**（2026-10-26 智能输入第一批）。
//!
//! 独立成文件是 500 行门禁（同 `engine_segment.rs` / `engine_symbol_browse.rs` 的
//! 拆分先例）：本批要给 `Engine` 加状态字段与清链调用，而 `engine.rs` 在拆分时已
//! 贴着门禁。搬来的只有公开入口 [`Engine::select_from`]（签名与语义不变，实现位置
//! 换了文件）。
//!
//! ## 语义
//!
//! - **权重**：空格接受首候选记 [`SelectionSource::AcceptFirst`]（+1）；用户点选 /
//!   数字选词 / 面板 / 符号大全记 [`SelectionSource::Explicit`]（+2）。权重只进学习
//!   频次，不改变排序算法本身。
//! - **误选撤销**：同一 `buffer + mode + scheme` 下先显式选了 A、又显式选了 B
//!   （A ≠ B）⇒ 先退回 A 的那一次学习，再记 B。用户改选表达的是「刚才选错了」，
//!   不是「两个词都喜欢」。
//! - 锚点 [`LastCommit`] 只存**最近一次**提交；`Engine::clear` / 切模式 / 换方案
//!   （含 `switch_language`）显式清链；缓冲不同则由比较自然失效（不撤销、只换锚点）。
//! - 只有**显式**这次会触发撤销：空格接受首候选不撤销（用户没有在改选），但它**也会
//!   更新锚点** —— 随后同输入下的显式改选能把那一票退掉（那确实只是「接受」）。
//! - 重复显式选同一个词不撤销，只继续累加。
//! - **没有跨提交的上下文候选**：本模块只动学习频次，不产生任何新候选。

use crate::candidates::Candidate;
use crate::composer::Mode;
use crate::engine::Engine;
use crate::learner::SelectionSource;
use crate::scheme::Scheme;

/// 最近一次候选提交的锚点（误选撤销的比较基线）。`Default` = 空链。
#[derive(Default)]
pub(crate) struct LastCommit(Option<Commit>);

struct Commit {
    /// 提交**前**的缓冲原文（提交会清空/切短它，事后再取就分不清同一输入了）。
    buffer: String,
    mode: Mode,
    scheme: Scheme,
    text: String,
    /// 记进学习器的权重（= 当时来源的 `weight()`），撤销时原样退回。
    weight: u32,
}

impl LastCommit {
    /// 本次显式提交应当撤销的上一条（同输入上下文 + 文本不同）；其余情形 None。
    fn undo_for(
        &self,
        source: SelectionSource,
        buffer: &str,
        mode: Mode,
        scheme: Scheme,
        text: &str,
    ) -> Option<(String, u32)> {
        if source != SelectionSource::Explicit {
            return None;
        }
        let prev = self.0.as_ref()?;
        (prev.buffer == buffer && prev.mode == mode && prev.scheme == scheme && prev.text != text)
            .then(|| (prev.text.clone(), prev.weight))
    }

    fn note(&mut self, buffer: String, mode: Mode, scheme: Scheme, text: &str, weight: u32) {
        self.0 = Some(Commit {
            buffer,
            mode,
            scheme,
            text: text.to_string(),
            weight,
        });
    }

    /// 清链（`Engine::clear` / 切模式 / 换方案）。缓冲变化不需要显式清 ——
    /// 比较时天然不相等。
    pub(crate) fn clear(&mut self) {
        self.0 = None;
    }
}

impl Engine {
    /// 选中候选项。越界返回空串。记录学习（若开启）。
    ///
    /// **不设上限**（曾是 512，与 [`crate::router::FETCH_LIMIT`] 统一）：512 不省任何成本
    /// —— `rank_and_pick` 无论 limit 多大都全量收集 + 排序（正确性，见该模块「不能下推
    /// limit 到词库」），截断只是扔掉**已经算好**的尾巴 —— 却造出一条**静默失败的下标带**：
    /// `candidates(usize::MAX)` 里有第 513 项（真库 'y' 实测 8017 项），而这里返回空串，
    /// 不提交、不学习，调用方也无从分辨「没有这一项」与「我内部只取了 512 项」（返回值
    /// 只有一个字符串，没有地方承载这个区别）。下标空间与 [`crate::router::KeyRouter`]
    /// 的 `fetched()` 对齐后，同一份候选表算出的下标两条路必给同一个答案。
    /// 门禁：tests/select_index_bounds.rs 的 engine_select_index_space_agrees_with_unlimited_list。
    pub fn select_from(&mut self, cands: &[Candidate], index: usize) -> String {
        let out = self.select_from_source(cands, index, SelectionSource::Explicit);
        // 普通提交也建联想（Task 5）；联想点击的链式根在 Task 7 接。
        if !out.is_empty() {
            self.build_association(&out);
        }
        out
    }

    /// [`Self::select_from`] 的**来源标注**变体：空格接受首候选传 `AcceptFirst`，
    /// 其余（点选 / 数字 / 面板 / 符号）传 `Explicit`。行为逐字相同，只差学习权重
    /// 与「是否触发误选撤销」。
    pub(crate) fn select_from_source(
        &mut self,
        cands: &[Candidate],
        index: usize,
        source: SelectionSource,
    ) -> String {
        self.after_digit = false; // 选词/面板点击 = 数字上下文结束
        // 浏览态：select = 激活浏览项（类目下钻返回空串 / 符号上屏并回一级），不是提交候选。
        // 面板点击与数字键因此同一条，且任何路径都提交不出类目名（见 engine_symbol_browse.rs 头）。
        if self.symbol_browse.is_active() {
            return self.symbol_browse_select(index);
        }
        match cands.get(index) {
            Some(c) => {
                let text = c.text.clone();
                // 学习锚点取**提交前**的会话：commit_buffer_after_select 会把缓冲清掉
                // 或切成后缀，之后再读就分不清「同一输入下的改选」了。
                let buffer = self.composer.session().buffer.clone();
                let mode = self.composer.session().mode;
                let scheme = self.scheme();
                self.learn_selection(&text, source, buffer, mode, scheme);
                // 分词回退态只消耗前缀；**逐字态返回 None = 本轮不提交**（engine_segment.rs，
                // 2026-10-17「确认一个字，依然在候选栏」）。
                let commit = self.commit_buffer_after_select(&text);
                // 符号模式一次一符号（用户裁决 2026-09-27）：提交后回拼音，不跨键保持
                // ——下一次击键就是拼音，不需要先手动切回。放这一层是因为候选提交的收尾
                // 只此一处：四端（fcitx5 / TSF / Android / Apple）自动一致，
                // 前端各写一份必漂（`switch_mode` 顺带清 shift，与本层语义一致）。
                if self.composer.session().mode == Mode::Symbol {
                    self.switch_mode(Mode::Pinyin);
                }
                match commit {
                    Some(full) => {
                        // 逐字态收尾：整串也记一次 —— 下一次整词候选因此能顶上来。
                        // 整串不参与误选撤销的锚点比较（它只是同一动作的合成结果）：
                        // 直接按来源权重记账，不碰 `LastCommit`。
                        if full != text {
                            self.learner.record_selection_weighted(&full, source);
                        }
                        // 英文候选的提交（空格/数字选词/回车/面板点击）与各端**直传点**
                        // 同一条全角规则：直传是宿主调 `fullwidthize`，候选提交在引擎，
                        // 只能在这里宽化。非 ASCII 恒等（`café` 的 é 不动），中文/符号
                        // 模式不受影响；学习记的仍是规范词形（上面那行）。
                        if self.composer.session().mode == Mode::English {
                            self.fullwidthize(&full)
                        } else {
                            full
                        }
                    }
                    None => String::new(),
                }
            }
            None => String::new(),
        }
    }

    /// 记一次候选提交，并在满足条件时先撤销上一次误选。
    fn learn_selection(
        &mut self,
        text: &str,
        source: SelectionSource,
        buffer: String,
        mode: Mode,
        scheme: Scheme,
    ) {
        // 学习关闭：不记账，也**不建锚点**。否则重新打开学习后，一次同输入改选会
        // 退掉关闭期间（乃至更早）的既有数据 —— 那些票本次根本没记。
        if !self.learner.is_enabled() {
            self.last_commit.clear();
            return;
        }
        if let Some((prev_text, prev_weight)) = self
            .last_commit
            .undo_for(source, &buffer, mode, scheme, text)
        {
            self.learner.unrecord_selection(&prev_text, prev_weight);
        }
        self.learner.record_selection_weighted(text, source);
        self.last_commit
            .note(buffer, mode, scheme, text, source.weight());
    }
}
