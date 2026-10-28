// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按住键的「按下结论」表（2026-10-28 由单槽 `last_printable` 换成集合后拆出：
//! `candidate.rs` / `input_method.rs` 都贴着 500 行门禁）。同名方法镜像
//! engine-core `router_edit_keys.rs`；TSF 轨由 tsf-ux 同步。

use crate::candidate::CandidateState;

/// 按住键集合上限：覆盖常见键盘 rollover，又不给「只发 keydown 不发 keyup」的
/// 客户端无限长的表。与 engine-core / tsf 的 `HOLD_LIMIT` 同值。
const HOLD_LIMIT: usize = 8;

impl CandidateState {
    /// 按下时记结论：**按下是权威**（直通插入、拦下移除），同一键的陈旧残留随之清掉。
    /// 有界：满了挤掉最老的，那一键的抬起会回落 EngineHandled（宁可拦下不可误放）。
    pub(crate) fn record_key_outcome(&mut self, keyval: u32, pass: bool) {
        self.held_passthrough.retain(|&k| k != keyval);
        if !pass {
            return;
        }
        if self.held_passthrough.len() == HOLD_LIMIT {
            self.held_passthrough.pop_front();
        }
        self.held_passthrough.push_back(keyval);
    }

    /// 抬起：返回按下时是否直通。**集合**而非单槽 —— 键盘 rollover 下另一键按下
    /// 不再顶掉前一键的记录（旧单槽会让先按的直通键在抬起时被误拦，客户端卡键）。
    pub(crate) fn release_key(&mut self, keyval: u32) -> bool {
        match self.held_passthrough.iter().position(|&k| k == keyval) {
            Some(i) => self.held_passthrough.remove(i).is_some(),
            None => false,
        }
    }

    /// 清空/切模式/换方案/失焦时作废全部按下记录（tsf 同名方法同契约）。
    pub(crate) fn clear_held_keys(&mut self) {
        self.held_passthrough.clear();
    }
}
