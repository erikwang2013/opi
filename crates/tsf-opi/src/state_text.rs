// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 直插文本队列的「入队 + 唤醒」半（从 state.rs 拆出守 500 行，2026-11-05）。
//!
//! 语音每 3 秒一段提交，若仍沿用「下一次按键/焦点才 drain」的旧语义，用户看到的是
//! 停掉录音之后才一次性出字。这里在入队后同步 SendMessage 到按键线程的 message-only
//! 窗口（`tsf_timer.rs`），让 UI 线程立刻取走 —— COM 插入仍只在 UI 线程发生。
//! 用 SendMessage 而非 PostMessage：部分宿主按窗口过滤消息泵，投递消息可能永远
//! 不被取走；同步消息在宿主下一次 Peek/GetMessage 时必然先被处理，且能拿到
//! 「这段到底插没插进文档」的结果，用来如实回答控制面。
//!
//! 本模块是 `state` 的子模块：可以直接用父模块的私有字段，不额外开口子。

use std::sync::atomic::Ordering;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW};

use super::{DRAIN_MESSAGE, EngineShared, MAX_PENDING_TEXT, TsfSharedState};

impl EngineShared {
    /// **读线程**调（候选窗 `commit_text` / 控制面 `commit`）：收下这段要直插
    /// 文档的文本并**同步**等 UI 线程落地。返回 false = 队列满/锁中毒/无目标/
    /// 目标窗口超时 —— 调用方据此报失败，不再假装「已提交」。
    /// 真正的 COM 调用仍只在 UI 线程发生。
    pub fn enqueue_commit_text(&self, text: String) -> bool {
        let seq = self.target_seq();
        let queued = match self.pending_text.lock() {
            Ok(mut q) if q.len() < MAX_PENDING_TEXT => {
                q.push_back((text, seq));
                true
            }
            _ => false,
        };
        queued && self.wake_drain()
    }

    /// **按键线程**调：取一段待直插的文本（文本 + 入队时的目标世代）。None = 队列空。
    pub fn take_pending_text(&self) -> Option<(String, u32)> {
        self.pending_text.lock().ok()?.pop_front()
    }

    /// 直插队列非空？（唤醒处理先查，空队列不做 COM 取焦点。）
    pub fn has_pending_text(&self) -> bool {
        self.pending_text.lock().is_ok_and(|q| !q.is_empty())
    }

    /// message-only 窗口的登记/摘除（`tsf_timer.rs`，均在按键线程跑）。
    pub fn set_drain_hwnd(&self, hwnd: Option<HWND>) {
        self.drain_hwnd
            .store(hwnd.map_or(0, |h| h.0 as isize), Ordering::Relaxed);
    }

    pub fn drain_hwnd(&self) -> Option<HWND> {
        match self.drain_hwnd.load(Ordering::Relaxed) {
            0 => None,
            h => Some(HWND(h as *mut core::ffi::c_void)),
        }
    }

    pub fn take_drain_hwnd(&self) -> Option<HWND> {
        match self.drain_hwnd.swap(0, Ordering::Relaxed) {
            0 => None,
            h => Some(HWND(h as *mut core::ffi::c_void)),
        }
    }

    /// **读线程**调：阻塞等 UI 线程取走刚入队的直插文本。返回 true = UI 线程已经
    /// 处理了唤醒（窗口过程返回 `已插入段数 + 1`，>0 即处理过）。
    /// 2s 超时是防宿主 UI 线程卡死；窗口已销毁（Deactivate 竞态）直接 false。
    ///
    /// 不用「插入段数 > 0」当成功判据：按键线程的 drain 可能先一步把这段取走，
    /// 窗口过程随后看到空队列（返回 1），那仍是成功，不能误报上屏失败。
    fn wake_drain(&self) -> bool {
        let hwnd = self.drain_hwnd.load(Ordering::Relaxed);
        if hwnd == 0 {
            return false;
        }
        let mut result = 0usize;
        // SAFETY: hwnd 是按键线程创建的 message-only 窗口；SendMessageTimeout 跨线程
        // 只投递消息，不让我们在错误线程碰 COM。
        let sent = unsafe {
            SendMessageTimeoutW(
                HWND(hwnd as *mut core::ffi::c_void),
                DRAIN_MESSAGE,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                2000,
                Some(&mut result),
            )
        };
        sent.0 != 0 && result > 0
    }
}

impl TsfSharedState {
    /// 唤醒窗口的登记/查询（`tsf_timer.rs` 不直接碰 `engine`）。
    pub fn drain_hwnd(&self) -> Option<HWND> {
        self.engine.drain_hwnd()
    }

    pub fn set_drain_hwnd(&self, hwnd: Option<HWND>) {
        self.engine.set_drain_hwnd(hwnd);
    }

    pub fn take_drain_hwnd(&self) -> Option<HWND> {
        self.engine.take_drain_hwnd()
    }

    /// **按键线程**调：取一段待直插的文本（见 `EngineShared::take_pending_text`）。
    pub fn take_pending_text(&self) -> Option<(String, u32)> {
        self.engine.take_pending_text()
    }

    /// **按键线程**调：直插队列是否还有待落地文本（`tsf_timer.rs` 先查再取焦点）。
    pub fn has_pending_text(&self) -> bool {
        self.engine.has_pending_text()
    }

    /// **按键线程**调：是否已记下插入目标（无键事件时为空，需从线程焦点补取）。
    pub fn target_present(&self) -> bool {
        self.target.lock().is_ok_and(|g| g.is_some())
    }
}
