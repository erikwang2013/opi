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
    /// 换成**首条候选的拼音**（「64」→ ni、「64426」→ nihao），即搜狗 9 键候选栏
    /// 左侧那格「输入时显示的拼音」。无候选时为空串。
    /// 其余方案 = 实际缓冲（26/14 键的字母串本来就是拼音；英文/数字/符号模式的
    /// 缓冲也不是拼音，一律回本体）。FFI 的 buffer 出口读这里 —— 别的端没有 9 键
    /// 方案，读到的与原值逐字节相同。
    pub fn display_buffer(&self) -> String {
        if matches!(self.mode(), Mode::Pinyin | Mode::Traditional) && self.scheme() == Scheme::Nine
        {
            // **带分词的**读数（搜狗「拼音9键显示分词」同款）：音节用 `'` 连起来，
            // 「nihao」显示成「ni'hao」—— 一眼看到引擎怎么切的音节。
            let cached = self.scheme_readings.borrow();
            // 缓冲变了（提交/退格/清空）而读数还是旧缓冲那一次算的 ⇒ 作废，报空串 ——
            // 界面按「读不到拼音」处理（左列回标点列）。下次 candidates() 会按新缓冲重算。
            if cached.0 != self.buffer() {
                return String::new();
            }
            return cached.1.first().cloned().unwrap_or_default();
        }
        self.buffer().to_string()
    }

    /// **逐条**候选拼音读数（已分词）：`readings[i]` = 最近一次 `candidates(limit)` 的
    /// 第 i 条候选的拼音（两串同序等长）。9 键左列按候选逐条显示用（2026-10-09 用户
    /// 裁决「一列多个、可滑动查看后面候选/下一页」）。
    ///
    /// 与 [`Self::display_buffer`] 是**同一对缓存的两个视图**：非 9/14 键方案、
    /// 或缓冲已变（提交/退格/清空）⇒ 空数组。调用方先 `candidates()` 再读它
    /// （同一次刷新里相邻调用 = 同一次排名，不会再扫一遍词库）。
    pub fn candidate_readings(&self) -> Vec<String> {
        if !matches!(self.mode(), Mode::Pinyin | Mode::Traditional) {
            return Vec::new();
        }
        if !matches!(self.scheme(), Scheme::Nine | Scheme::Fourteen) {
            return Vec::new();
        }
        let cached = self.scheme_readings.borrow();
        if cached.0 != self.buffer() {
            return Vec::new();
        }
        cached.1.clone()
    }
}
