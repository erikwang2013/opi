// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想态的两个推送点（2026-10-27，Spec §5，镜像 fcitx5 轨）：
//! 按键时收起、提交后补推快照。拆出守 500 行；锁内只碰引擎，管道写在锁外
//! （理由见 `sink.rs` 的 `push_state`）。
//!
//! 已知边界（2026-10-28 终审，Spec §11）：候选窗点选整段延迟到下一次按键
//! （`state.rs` 模块头），`drain_pending` 刚建的新链会在同一 keydown 被收起 ——
//! TSF 上联想项可点选落字，不能链式。

use windows::Win32::UI::TextServices::ITfContext;

use crate::logic::StateSnapshot;
use crate::tsf::{TsfTextService, push_state};

impl TsfTextService {
    /// 联想态下按下任意键：清联想并把收起后的快照推给候选窗（窗口随之隐藏）。
    /// 没有联想时什么都不做 —— 不白抓一次候选表。
    pub(crate) fn dismiss_association_and_push(&self) {
        let snapshot = self
            .state
            .lock_logic()
            .and_then(|mut logic| logic.dismiss_association().then(|| logic.snapshot()));
        if let Some(snapshot) = snapshot {
            push_state(&*self.sink, &snapshot);
        }
    }

    /// 提交后仍在组合/联想态时补位置并推快照。
    /// 联想态（Spec §5）缓冲是空的，不补推的话候选窗停在提交前的内容上。
    pub(crate) fn push_after_commit(&self, snapshot: &StateSnapshot, pic: Option<&ITfContext>) {
        if !(snapshot.composing || snapshot.association) {
            return;
        }
        if let Some(pic) = pic
            && let Some((x, y)) = crate::caret::candidate_anchor(pic, self.state.client_id())
        {
            self.sink.on_caret(x, y);
        }
        push_state(&*self.sink, snapshot);
    }
}
