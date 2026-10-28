// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗接缝（`TsfSink`）、状态推送与 COM 出口公用件（Windows 目标专属）。
//!
//! 从 `tsf.rs` 拆出只为守住 500 行；`tsf.rs` 用 re-export 保持原路径可用。
//! 接缝语义：**不负责文档写入** —— 提交文本的插入由 `state.rs` 的 `insert_into`
//! 用 COM 接口完成（那里才有 `ITfContext` 与 client id），本接缝只把"发生了什么"
//! 通知出去，供候选窗（C3 的 CMP 窗口）刷新。

use std::sync::Once;

use engine_core::composer::Mode;
use windows::core::HRESULT;

use crate::logic::StateSnapshot;

/// E_FAIL：引擎内部错误的统一返回码（panic 被兜住 / 锁中毒）。
pub(crate) const E_FAIL: HRESULT = HRESULT(0x80004005_u32 as i32);

/// 一次性 panic hook（对照 fcitx5-opi/src/lib.rs 的 `ensure_panic_hook`）。
/// 每个 COM 出口都要调：换掉默认 hook 后，宿主进程的事件日志里能看到是引擎炸了。
pub(crate) fn ensure_panic_hook() {
    static PANIC_HOOK: Once = Once::new();
    PANIC_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            tlog!("tsf-opi: engine panic caught: {info}");
        }));
    });
}

/// 观察接缝：Commit → 隐藏候选窗；CompositionChanged → 用候选数据刷新候选窗。
/// 无状态变化时不回调。
pub trait TsfSink: Send + Sync {
    /// 宿主 Activate：候选窗客户端**主动**连上候选窗（状态栏在用户第一次按键之前
    /// 就要能读到 `state`；只靠 show 惰性连接会让状态栏在未打字时一直不出现）。
    fn on_activate(&self) {}
    /// 立即提交文本（英文直传、空缓冲空格、缓冲/候选提交等）。
    fn on_commit(&self, _text: &str) {}
    /// composition/候选窗需刷新。
    fn on_composition_changed(
        &self,
        _buffer: &str,
        _candidates: &[String],
        _page: usize,
        _page_count: usize,
        _mode: Mode,
    ) {
    }
    /// caret 位置 → 候选窗左上角（**AWT 逻辑像素**，单位约定见 `candidate_io.rs`
    /// 协议头）。拿不到位置（非文档 context / 编辑会话被拒）时**不调** ——
    /// 窗口沿用上一次位置，不闪。
    fn on_caret(&self, _x: i32, _y: i32) {}
    /// 宿主失活：释放单实例候选窗管道。不释放则第一个输入过的应用占住连接，
    /// 其余应用永远连不上（「只在桌面/单个应用可用」）；窗口侧断开后自隐藏并重建。
    fn disconnect(&self) {}
    /// 语言/标点/全半角/模式等状态变了，但可能没有候选刷新（如 Shift 切语言）。
    /// 候选窗进程据此 `SetEvent` 唤醒状态栏（2026-10-25 响应 <10ms）。
    fn on_state_changed(&self) {}
}

/// 把快照推给候选窗。服务对象（按键/切模式）与候选窗回调（点选后刷新，
/// 见 candidate_io.rs 的 `SharedAction::refresh`）共用 —— 两处各写一遍必然漂移，
/// 症状是"切了模式/点了候选，窗口还显示旧内容"。
///
/// **只吃快照、不碰 `TsfLogic`**：调用方必须先在锁内 `TsfLogic::snapshot()`、
/// 放锁后再调本函数。管道写可能阻塞（候选窗进程卡住），持着 logic 锁跨它会
/// 冻住宿主 UI 线程（2026-10-22 真机卡死根因）。
pub(crate) fn push_state(sink: &dyn TsfSink, state: &StateSnapshot) {
    sink.on_composition_changed(
        &state.buffer, // 2026-10-17：候选窗显示带已确认的逐字片段
        &state.candidates,
        state.page,
        state.page_count,
        state.mode,
    );
}
