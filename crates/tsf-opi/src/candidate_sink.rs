// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗接缝的**生产实现**（`TsfSink`）：把引擎的 composition 变化映射成
//! show/hide/position 消息（Windows 目标专属）。
//!
//! 从 `candidate_io.rs` 拆出守 500 行：那边只管**管道与分帧**（连接/读写/协议），
//! 这里只管**事件 → 窗口消息**的映射。两边都以 `CandidateClient` 为界，没有第二份
//! 连接逻辑。
//!
//! 线程边界：`on_composition_changed`/`on_caret` 在**按键线程**（`push_state`），
//! `on_activate`/`disconnect` 来自 TSF 生命周期与线程焦点回调；`CandidateClient`
//! 内部用锁串行化句柄，不需要本层再加一层。

use std::sync::Arc;

use engine_core::composer::Mode;

use crate::candidate_io::{CandidateAction, CandidateClient};
use crate::state::EngineShared;
use crate::tsf::TsfSink;

/// 候选窗接缝的生产实现：把 composition 变化映射为 show/hide。
pub struct CandidateSink {
    client: CandidateClient,
    /// 引擎（与 `SharedAction` 同一份）：`on_activate` 顺带比对清词世代 ——
    /// 这里正是 TSF Activate / 线程焦点 / profile 重新激活三条路的共同出口。
    engine: Arc<EngineShared>,
}

impl CandidateSink {
    pub fn new(action: Arc<dyn CandidateAction>, engine: Arc<EngineShared>) -> Self {
        Self {
            client: CandidateClient::new(action),
            engine,
        }
    }
}

impl TsfSink for CandidateSink {
    /// 宿主激活：先比对清词世代（按宿主、下次焦点生效，见 `clear_generation.rs`），
    /// 再允许读线程主动连候选窗（见 `CandidateClient::on_activate`）。
    fn on_activate(&self) {
        self.engine.check_clear_generation();
        self.client.on_activate();
    }

    /// 提交：composition 结束 → 隐藏候选窗。
    fn on_commit(&self, _text: &str) {
        self.client.hide();
    }

    /// caret 位置 → 候选窗（发端见 `caret.rs`；收端把 `position` 直接当
    /// `WindowPosition(x.dp, y.dp)` 用）。先发位置后发 show（见 `tsf.rs` 的
    /// `CompositionChanged` 分支），窗口不会先出现在旧位置再跳。
    fn on_caret(&self, x: i32, y: i32) {
        self.client.position(x, y);
    }

    /// composition/候选变化：缓冲非空 → show（页码 1 起）；空 → hide。
    fn on_composition_changed(
        &self,
        buffer: &str,
        candidates: &[String],
        page: usize,
        page_count: usize,
        mode: Mode,
    ) {
        if buffer.is_empty() {
            self.client.hide();
        } else {
            self.client
                .show(buffer, candidates, page + 1, page_count, mode);
        }
    }

    /// 宿主失活：断开管道，把单实例让给下一个应用（见 TsfSink::disconnect）。
    fn disconnect(&self) {
        self.client.disconnect();
    }

    /// 状态变了 → 候选窗进程唤醒状态栏（不经过 1s 轮询）。
    fn on_state_changed(&self) {
        self.client.state_changed();
    }
}
