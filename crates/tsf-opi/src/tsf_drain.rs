// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗回复（点选 / 直插文本）的执行点（从 `tsf.rs` 拆出守 500 行，2026-10-28）。
//!
//! 线程模型与延迟语义见 `state.rs` 模块头。调用点有三处：`handle_key_inner` 与
//! `key_sink.rs` 的 `OnTestKeyDown`（键事件开头）、`OnSetFocus`（焦点变化）。

use crate::sink::push_state;
use crate::tsf::TsfTextService;

impl TsfTextService {
    /// 读线程只把 index/文本排进队列（它拿不到 `ITfContext` —— 那是本线程的 COM
    /// 接口指针），真正走 COM 的插入在这里、在按键线程上跑。
    ///
    /// **已知代价：点到下一次按键/焦点变化之前才插入**。即时插入的两条升级路径
    /// （都要真机）写在 state.rs 模块头。一次按键最多清空队列（各上限 16 条）。
    ///
    /// 换文档的边界（2026-10-28 修）：每条排队动作带着入队时的目标世代，执行点
    /// 与当前世代不一致 = 点选之后换了文档 → 丢弃（`select_and_insert` 里判定）。
    /// 不会再把上一个文档的候选插进当前文档；代价是那次点击丢了 —— 它属于旧文档，
    /// 而旧文档此刻已没有合法插入目标。
    pub(super) fn drain_pending(&self) {
        while let Some((index, seq)) = self.state.take_pending_selection() {
            if self.state.select_and_insert(index, seq).is_none() {
                continue; // 越界/空缓冲/目标已换：引擎没变，窗口无需刷新
            }
            // 提交后缓冲已空 → sink 内部走 hide 分支，窗口自己收起来。
            // 锁只包住抓快照这一步：insert_into 早已返回，管道写在锁外。
            let snapshot = self.state.lock_logic().map(|logic| logic.snapshot());
            if let Some(snapshot) = snapshot {
                push_state(&*self.sink, &snapshot);
            }
        }
        // 候选窗「直接插入」（commit_text：符号页多选）：插文档但**不动引擎、不通知
        // sink**（`on_commit` 会收起窗口，而符号页要留在屏幕上连续点选；裁决
        // 2026-10-10，见 candidate_io.rs 协议头）。与上面同受"延迟到下一次按键"的约束。
        while let Some((text, seq)) = self.state.take_pending_text() {
            self.state.insert_text(&text, seq);
        }
    }
}
