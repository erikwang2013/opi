// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想收起（2026-10-27，Spec §5）：清引擎联想态，并作废一次抓取缓存 ——
//! 缓存键只有 (缓冲, 逐字片段)，联想开关不在键里，留着会让紧随其后的
//! `snapshot()` 取回已收起的联想候选。
//!
//! 挂在 `logic_snapshot.rs`（`logic.rs` 已 498 行）；逐行镜像 fcitx5 轨
//! `candidate_association.rs`。

use crate::logic::TsfLogic;

impl TsfLogic {
    // 生产调用方只在 Windows 胶水（`key_sink.rs` 的 OnTestKeyDown）；Linux 主机上
    // 只有测试用它 —— 不加 allow 会让 CI 的 clippy -D warnings 因 dead_code 变红。
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn dismiss_association(&mut self) -> bool {
        self.invalidate_fetch_cache();
        self.engine.dismiss_association()
    }
}
