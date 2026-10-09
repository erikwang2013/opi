// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `Engine` 的符号浏览一族（装载 / 入口 / 出口 / 选中）+ 符号表查询的转发出口。
//! impl 体搬出 `engine.rs` 的两条理由（与 engine_punct.rs 同一先例）：① `engine.rs`
//! 贴着 500 行门禁；② 这一族自洽，只依赖 `symbol_browse` 模块 —— 而热路径上的几处
//! 读侧分派（`buffer` / `candidates` / `select_from` / `input_key`…）留在那里，各是
//! 一个 `if`，搬走只让读者跳来跳去。
//!
//! 字段可见性：本文件要碰 `Engine` 的 `symbol_browse` 与 `symbols` 两个字段，故它们
//! 在 engine.rs 里声明为 `pub(crate)`（crate 内，不泄漏给依赖方）—— 同 engine_punct.rs
//! 的做法与理由。
//!
//! # 宿主契约（2026-10-10 lead 裁决，两轨同办）
//!
//! ① **浏览态劫持 `buffer()`**：返回的是标题（一级「符号」/ 二级「符号·名称」），
//!    不是 composer 的真缓冲。这不是意外而是载体：fcitx5 面板与 tsf 候选窗都按
//!    「buffer 非空」决定显示，没有这个非空标题，**面板根本不显示**（这是「UI 零改动」
//!    的全部代价）。⇒ 任何**读缓冲去提交**的路径（切走确认 W1、IME 停用）**必须先
//!    `exit_symbol_browse()`**，否则提交出「符号」两个字。单点实现 =
//!    [`Engine::confirm_pending`]（两轨的 `switch_mode_confirmed` 走它）。
//! ② **浏览期间未处理的键** = 先退出浏览恢复真缓冲 → 再按正常流程处理那个键
//!    （不许带着标题落进正常路由）。路由整段在宿主（fcitx5 `input_method.rs` / tsf
//!    `logic_input_method.rs` 的浏览分支，两轨逐行同构）；引擎层的
//!    [`Engine::leave_symbol_browse_if_needed`] 是同一规则的兜底，供直调引擎的出口
//!    （C ABI 的 input_space / Android 侧不可能浏览，见各文件）不落进标题态。
//! ③ **`select_from` 在浏览态 = 激活浏览项**：类目下钻（返回空串）/ 符号上屏并回一级。
//!    不是「提交候选」—— 面板点击类目与数字键因此走同一条换算，且**任何**路径都
//!    提交不出类目名（下钻那一支返回空串，见 `symbol_browse_select`）。
//!
//! # fcitx5 侧的入口形态（2026-10-10 与 fcitx5-glue 对齐，硬契约）
//!
//! C ABI **只有一个新名字**：`opi_fcitx5_symbol_browse(int32_t action)` ——
//! 0 = 退出（幂等）/ 1 = 进入（已在浏览态 no-op）/ 2 = 切换（引擎按自身状态决定）。
//! C++ 侧看不到浏览态、又要求「热键来回切」，故切换语义必须在引擎侧；glue 用
//! **弱符号**声明它（我落地前 nullptr，落地后同一二进制下次加载自动接上）。
//! 硬性语义：**退出是同步的**（返回时 buffer 已是真缓冲）—— 切走/切英文的
//! W1 顺序是「先 action 0 退浏览 → 再按 W1 提交（有候选首候选、无候选原文）」，
//! 且 glue 的提交路径每次切走都**无条件**先调一次 action 0 ⇒ 幂等是承重条件。
//! 键位（Ctrl+Shift+Z，rawKey 判据）与面板刷新归 glue；浏览**期间**的
//! 1-9、`-`/`=`、Esc、未处理键路由归宿主输入层（fcitx5 `input_method.rs` /
//! tsf `logic_input_method.rs`，两轨逐行同构）。

use crate::engine::Engine;
use crate::symbol_browse::Selection;

impl Engine {
    /// 装载类目数据（宿主把 TSV 串传进来；engine-core 禁 IO，文件读取归宿主）。
    ///
    /// 返回**跳过的坏行数**（0 = 全好）—— 宿主据此打日志（「别让解析失败静默」），
    /// 与 Android 端 `SymbolCategories.parse` 的「坏行跳过而不抛」同义。
    /// 空数据集（空串 / 全坏）→ 数据集清空：入口保持禁用（`enter_symbol_browse`
    /// 返回 false），与「文件缺失 ⇒ 入口禁用」同一条降级。
    /// 重复调用 = **替换**数据集并复位浏览态（旧下标对新数据不再有意义）；
    /// 数据与浏览态是两件事，别退回「一个 Option 全包」—— 那会让装载完的
    /// 第一次打字把数据集当浏览态清掉（回归见 tests/symbol_browse.rs）。
    pub fn load_symbol_categories(&mut self, tsv: &str) -> usize {
        let (categories, skipped) = crate::symbol_browse::parse_categories(tsv);
        self.symbol_browse.load(categories);
        skipped
    }

    /// 类目数据是否可用（入口闸门：文件缺失/坏 ⇒ 假 ⇒ 宿主把入口禁用）。
    pub fn symbol_browse_available(&self) -> bool {
        !self.symbol_browse.is_empty()
    }

    /// 进入符号浏览（一级 = 类目）。无数据 → false（入口禁用），状态不动；
    /// 已在浏览态 → no-op 返回 true（不弹回一级，见 `SymbolBrowse::enter`）。
    /// **不动 composer**：浏览是叠加态 —— 打到一半的拼音留在真缓冲里被标题遮着，
    /// 退出即恢复（Esc 放弃的是浏览，不是输入）。
    pub fn enter_symbol_browse(&mut self) -> bool {
        self.symbol_browse.enter()
    }

    /// 退出符号浏览（宿主路由：Esc；fcitx5 侧切走/切英文前的强制前置；以及
    /// 「未处理键先退出再正常处理」那条的第一半）。**同步**：返回时
    /// [`Self::buffer`] 已是真缓冲（C ABI `opi_fcitx5_symbol_browse(0)` 靠这条）。
    /// 非浏览态调用 = 空操作（幂等 —— 宿主的提交路径每次切走都无条件先调一次）。
    /// **退出不提交任何文本、不丢数据** —— 真缓冲原样可见可续打，类目表留着待再进。
    pub fn exit_symbol_browse(&mut self) {
        self.symbol_browse.exit();
    }

    pub fn is_symbol_browsing(&self) -> bool {
        self.symbol_browse.is_active()
    }

    /// 契约 ② 的单点实现：浏览态下任何「非浏览专属」的入口先调它 ——
    /// 出错形态只有一个（带着「符号」标题落进正常路由），单点免得某个入口漏掉。
    pub(crate) fn leave_symbol_browse_if_needed(&mut self) {
        self.symbol_browse.exit(); // exit 幂等且不动数据，无条件调用即可
    }

    /// 切走/切英文的**直接确认**（W1，2026-10-10 lead 裁决）：先退浏览（恢复真缓冲
    /// —— 顺序承重，见文件头契约 ①），再把待处理的缓冲按「**有候选上首候选、
    /// 无候选上原文**」换算成待上屏文本。返回空串 = 无提交（空缓冲）。
    ///
    /// 与 [`Self::switch_mode`]（清缓冲：打到一半的拼音被静默丢弃）和
    /// [`Self::toggle_symbol`]（无候选把原文**丢掉**，Android `commitPendingBuffer`
    /// 语义）的差别都在「无候选」这一支：W1 交出原文。收尾本身与
    /// [`Self::input_space`] 同一条（记学习 / 清缓冲 / 页码只此一份），逐模式随之
    /// 继承 —— 符号模式无候选**不**提交关键字（关键字不是要上屏的文本，判据在
    /// input_space）。调用方负责把非空返回插入文档；拿不到插入通道的端别调
    /// （同 [`Self::toggle_symbol`]）。
    ///
    /// 状态机侧的「确认 + 切换」入口：两轨的 `switch_mode_confirmed`（配对）。
    pub fn confirm_pending(&mut self) -> String {
        // 契约 ①。`input_space` 入口本身也有同款兜底（契约 ② 对直调引擎的出口），
        // 这里是同一顺序的显式前置 —— 两处都幂等，不互为唯一防线。
        self.leave_symbol_browse_if_needed();
        self.input_space()
    }

    /// 浏览态选中的实际落点（`select_from` 的浏览分支）：类目 = 下钻（空串 ——
    /// **不走**数字选词那条「空串 = 直通」判据，宿主在浏览分支里自消化）；符号 =
    /// 上屏（记学习，与既有 select 同一条）+ 回一级。**不碰 composer**（真缓冲留在
    /// 原地等退出），也不记类目的学习（类目不是可上屏文本）。
    /// 非浏览态 `select` 一律 `Miss` → 空串；引擎门面已按 active 分派，这里是兜底。
    pub(crate) fn symbol_browse_select(&mut self, index: usize) -> String {
        match self.symbol_browse.select(index) {
            Selection::Commit(text) => {
                self.learner.record_selection(&text);
                text
            }
            Selection::Drill | Selection::Miss => String::new(),
        }
    }

    // ---- 符号表查询的转发出口（从 engine.rs 机械搬来，见文件头理由①）----

    pub fn symbol_blocks(&self) -> Vec<crate::symbols::Block> {
        self.symbols.common_blocks()
    }

    pub fn symbols_in_block(
        &self,
        id: crate::symbols::BlockId,
    ) -> Vec<crate::symbols::SymbolEntry> {
        self.symbols.entries_in_block(id)
    }

    pub fn search_symbols(&self, keyword: &str) -> Vec<crate::symbols::SymbolEntry> {
        self.symbols.search(keyword)
    }
}
