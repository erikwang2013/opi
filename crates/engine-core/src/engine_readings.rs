// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 9 键「拼音读数」一族（2026-10-09 二期从 `engine.rs` 拆出 —— 那里加逐条读数与
//! 语言切换后越了 500 行门禁，与 engine_punct.rs / candidates_tests 同一套先例）。
//!
//! 拆的是**同一对缓存的两个视图**：[`Engine::display_buffer`]（首条，候选栏左格/读屏用）
//! 与 [`Engine::candidate_readings`]（逐条，9 键左列用）—— 都读 `scheme_readings`
//! 这一对 `(算它时的缓冲, 逐条读数)`。语义一字未动；写点在 `Engine::candidates()`
//! 的九键分支（仍在 engine.rs）。
//!
//! 字段可见性：本文件要用 `Engine::scheme_readings`，故它升成 `pub(crate)`
//! （**crate 内**，不泄漏给依赖方）—— 同 engine_punct.rs 头注记的拆分裂代价。

use crate::composer::Mode;
use crate::engine::Engine;
use crate::scheme::Scheme;

impl Engine {
    /// **显示用**缓冲（2026-10-09，用户裁决「输入的时候没在左侧显示拼音」）：
    /// 9 键方案的缓冲是**数字串**（键位序列，不是拼音）——直接摆出来用户不认；
    /// 换成**首条候选的拼音**（「64」→ ni、「64426」→ nihao），即主流输入法 9 键候选栏
    /// 左侧那格「输入时显示的拼音」。无候选时为空串。
    /// 其余方案 = 实际缓冲（26/14 键的字母串本来就是拼音；英文/数字/符号模式的
    /// 缓冲也不是拼音，一律回本体）。FFI 的 buffer 出口读这里 —— 别的端没有 9 键
    /// 方案，读到的与原值逐字节相同。
    pub fn display_buffer(&self) -> String {
        // 逐字确认态（2026-10-17）：已确认的字**留在候选区** —— 显示串以它们打头
        // （「中guo」），四端预编辑/拼音行都读这一份。
        let mut out = self.pending_text();
        if matches!(self.mode(), Mode::Pinyin | Mode::Traditional)
            && matches!(self.scheme(), Scheme::Nine | Scheme::Shuangpin)
        {
            // **带分词的**读数（主流输入法「拼音9键显示分词」同款）：音节用 `'` 连起来，
            // 「nihao」显示成「ni'hao」—— 一眼看到引擎怎么切的音节。
            let cached = self.scheme_readings.borrow();
            // 缓冲变了（提交/退格/清空）而读数还是旧缓冲那一次算的 ⇒ 作废，报空串 ——
            // 界面按「读不到拼音」处理（左列回标点列）。下次 candidates() 会按新缓冲重算。
            if cached.0 == self.buffer() {
                out.push_str(cached.1.first().map(String::as_str).unwrap_or_default());
            }
            return out;
        }
        out.push_str(self.buffer());
        out
    }

    /// 候选拼音读数（已分词），**去重 + 按长度短→长**（2026-10-12 晚用户裁决
    /// 「输入的拼音，在左侧排列从短到长」）。
    ///
    /// 两条规则叠在一起，次序固定：
    /// 1. **去重保序**（2026-10-12 白天的裁决）：同一读音只出一行，行序 = 该读音最好
    ///    那条候选的名次；真库 `426` 曾实测 8 行里 5 行重复。
    /// 2. **按长度升序**（同晚裁决）：短拼音在前、长的在后；**同长度保持第 1 步的
    ///    名次序**（稳定排序，不改成字典序 —— 用户要的是「短到长」，不是「字母序」）。
    ///    长度按**显示串**（含 `'` 分词符）的字符数算：`mo`(2) < `men`(3) < `mo'ni`(5)。
    ///    实测来由：真机左列当时是 `mo'ni / mo'mo / mo / o / men`（按名次），用户要
    ///    `o / mo / men / mo'ni / mo'mo`。
    ///
    /// ⚠️ 两条都发生在**本出口**，内部缓存（`scheme_readings`）与 [`Self::display_buffer`]
    /// 一字不动：候选↔读数的逐条对应不再是公共契约（左列不需要它；钉读音按
    /// **字符串**传动，见 [`Self::set_pinned_reading`]，不靠下标）。
    ///
    /// 与 [`Self::display_buffer`] 是**同一对缓存的两个视图**：非 9/14 键方案、
    /// 或缓冲已变（提交/退格/清空）⇒ 空数组。调用方先 `candidates()` 再读它
    /// （同一次刷新里相邻调用 = 同一次排名，不会再扫一遍词库）。
    pub fn candidate_readings(&self) -> Vec<String> {
        if !matches!(self.mode(), Mode::Pinyin | Mode::Traditional) {
            return Vec::new();
        }
        // 双拼同列（2026-10-13）：读数 = 各解析的展示形（左列给的是**解析候选**）。
        if !matches!(
            self.scheme(),
            Scheme::Nine | Scheme::Fourteen | Scheme::Shuangpin
        ) {
            return Vec::new();
        }
        let cached = self.scheme_readings.borrow();
        if cached.0 != self.buffer() {
            return Vec::new();
        }
        // 去重保序：n = 候选数（≤ 调用方的 limit，量级 8–64）—— 一趟遍历 + 集合判重，
        // 成本在字符级（读音串很短），相对本函数的调用点（一次全库排名）可忽略。
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::with_capacity(cached.1.len());
        for r in &cached.1 {
            if seen.insert(r.as_str()) {
                out.push(r.clone());
            }
        }
        // 短→长（稳定排序：同长度保持上面的名次序）。长度 = 显示串字符数（见函数注）。
        out.sort_by_key(|r| r.chars().count());
        out
    }

    /// 钉读音（9 键左列点一行拼音：**不确认**，候选列表收窄到**读音恰等**的条目 ——
    /// 2026-10-09 用户裁决「选择后所有候选词也跟着拼音变化显示出来，不要直接确认」；
    /// 2026-10-16 裁决「左侧选择拼音的字了，就不要联想词组了，选择的是字」把过滤从
    /// 「拼音以它开头」收紧为恰等，过滤实现见 `dictionary.rs::scheme_entries_scan` 的注）。
    /// `None`/空串 = 解除。入参允许带分词（界面给的是 `ni'hao`），这里归一为去 `'` 的原串。
    /// 与 `(缓冲, 读音)` 绑存：同一缓冲内有效，缓冲一走到别处即作废（不用谁去清）。
    /// ⚠️ 配对语义的边界：缓冲「变了又变回去」（如退格再打同一串）会复活旧钉 ——
    /// 调用侧（Android `EngineController`）在缓冲变化时显式 `pinReading("")` 清掉，
    /// 引擎这层只管「同缓冲内有效」。
    pub fn set_pinned_reading(&mut self, reading: Option<String>) {
        self.pinned_reading = match reading {
            Some(r) if !r.is_empty() => Some((self.buffer().to_string(), r.replace('\'', ""))),
            _ => None,
        };
    }

    /// 当前生效的钉读音（按缓冲校验过；不生效 ⇒ `None`）。`candidates()` 的九键分支用它。
    pub(crate) fn active_pin(&self) -> Option<&str> {
        match &self.pinned_reading {
            Some((buf, r)) if buf == self.buffer() => Some(r.as_str()),
            _ => None,
        }
    }
}
