// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按住键的「按下结论」表（2026-10-28 由单槽 `last_printable` 换成有界集合后拆出：
//! `logic.rs` / `logic_input_method.rs` 都贴着 500 行门禁）。同名方法镜像
//! engine-core `router_edit_keys.rs`；配对：fcitx5-opi/src/input_method_keys.rs。

use crate::logic::TsfLogic;

/// 同时按住的直通键上限。物理键盘 rollover 远低于此；上限只防宿主永不发 keyup
/// 时集合无限长。满时丢最久未按的一条（重复按会刷新）—— 它的抬起回落到消费，
/// 是安全方向。
pub(crate) const HOLD_LIMIT: usize = 8;

impl TsfLogic {
    /// 按下时记结论：**按下是权威**（直通插入、拦下移除）。先移除旧条目 —— 既是
    /// 去重，也让重复按下（自动重复）刷新为最新：淘汰按「最久未按」算，重按不得
    /// 挤掉别的按住键。2026-10-28 与 fcitx5/engine-core 两轨定型为同一形状。
    pub(crate) fn record_key_outcome(&mut self, keyval: u32, pass: bool) {
        self.held_passthrough.retain(|&k| k != keyval);
        if !pass {
            return;
        }
        if self.held_passthrough.len() >= HOLD_LIMIT {
            self.held_passthrough.pop_front();
        }
        self.held_passthrough.push_back(keyval);
    }

    /// 抬起：返回按下时是否直通。**集合**而非单槽 —— 键盘 rollover 下另一键按下
    /// 不再顶掉前一键的记录（旧单槽会让先按的直通键在抬起时被误拦，客户端卡键）。
    pub(crate) fn release_key(&mut self, keyval: u32) -> bool {
        let pos = self.held_passthrough.iter().position(|&k| k == keyval);
        pos.is_some_and(|i| self.held_passthrough.remove(i).is_some())
    }

    /// 清空按住表：clear / 切模式 / 换方案 / 失焦。这些点之后旧键的抬起结论不再
    /// 可信，缺失键的抬起回落到消费（安全方向）。
    pub(crate) fn clear_held_keys(&mut self) {
        self.held_passthrough.clear();
    }
}
