// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想收起（2026-10-27，Spec §5）：清引擎联想态，并作废一次抓取缓存 ——
//! 缓存键只有 (缓冲, 逐字片段)，联想开关不在键里，留着会让紧随其后的
//! `candidates()` 取回已收起的联想候选。
//!
//! 单独成文件是 500 行门禁的拆分裂（candidate.rs 已 491 行），语义只有这一个方法。

use super::CandidateState;

impl CandidateState {
    pub(crate) fn dismiss_association(&mut self) -> bool {
        self.invalidate_fetch_cache();
        self.engine.dismiss_association()
    }
}
