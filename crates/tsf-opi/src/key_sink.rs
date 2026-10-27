// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `ITfKeyEventSink` 的五个回调（从 `tsf.rs` 拆出守 500 行）。
//!
//! `OnTestKeyDown` 是 TSF 的**最终吞键决定**（TRUE 之后 `OnKeyDown` 再答 FALSE
//! 不会把键还给应用），所以认领判定必须与引擎分流一致，见 `logic_claim.rs`。
//! 其余回调只是把键转给 `TsfTextService::handle_key` 或维护 Shift 轻按状态。

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_LWIN, VK_RWIN};
use windows::Win32::UI::TextServices::{ITfContext, ITfKeyEventSink_Impl};
use windows::core::{BOOL, GUID, Ref, Result};

use crate::key_state::{map_key_state, to_engine_keycode};
use crate::logic::{KEY_STATE_ALT, KEY_STATE_CTRL};
use crate::tsf::TsfTextService_Impl;
use crate::vk::{fullwidth_hotkey, is_shift_vk, mode_hotkey};

impl ITfKeyEventSink_Impl for TsfTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        // 焦点切换撤销 Shift 轻按判定：按住 Shift 切走/切回后，抬起不得切模式
        // （镜像 fcitx5 轨 `OpiShiftToggle::disarm` 的 reset 语义）。
        self.shift_tap_disarm();
        Ok(())
    }

    fn OnTestKeyDown(&self, _pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // TSF 只在 OnTestKeyDown 返回 TRUE 时才调用 OnKeyDown，而且这个 TRUE 是
        // **最终决定**：OnKeyDown 再答 FALSE 不会把键还给应用。2026-10-21 旧实现
        // 只认领热键、普通字母全放行，导致中文永远不出候选；2026-10-22 的另一端
        // （一律认领、等 OnKeyDown 回 Unhandled）又吞掉了 F1–F12/媒体键和空缓冲
        // 的退格/回车/Esc/Tab。正确形状是**测试阶段就与引擎分流一致**：
        //   · 模式热键 / 全角热键 → TRUE（必须在应用看到之前认领）；
        //   · 带 Ctrl/Alt 的组合键（除热键外）与 Win 键/Win 组合键 → FALSE；
        //   · 引擎不会消费的键（功能键、空缓冲特殊键）→ FALSE，交给应用/系统；
        //   · 其余键 → TRUE；OnKeyDown 里引擎保证消费。
        //
        // 联想态（Spec §5）：**按下**先收起再走上面的分流 —— 点选候选排回本线程
        // 的队列也要先落地（点选可能刚把引擎带进联想态）。Ctrl/Alt 直通同样收起。
        // 点选建出的新链在这里又被收起：TSF 排队架构的已知边界（Spec §11）。
        self.drain_pending();
        self.dismiss_association_and_push();
        let states = map_key_state(lparam);
        let vk = wparam.0 as u32;
        // Shift 轻按判定第一拍（镜像 fcitx5 轨 `OpiShiftToggle::handle`）：
        // Shift 按下 → 武装；**任何别的键**按下 → 撤销。必须在所有提前 return
        // 之前 —— Ctrl/Alt/Win 组合在这下面返回 FALSE、不会进 OnKeyDown，但它们
        // 同样不是轻按（Shift+Ctrl+C、Shift+Win+S…），漏掉会在 Shift 抬起时误切。
        self.shift_tap_key_down(vk);
        if mode_hotkey(vk, states).is_some() || fullwidth_hotkey(vk, states) {
            return Ok(BOOL(1));
        }
        if states & (KEY_STATE_CTRL | KEY_STATE_ALT) != 0 {
            return Ok(BOOL(0));
        }
        // Win 组合键（Win+R、Win+Space、Win+数字…）：Win 键本身与「Win 正被按住」
        // 都放行。`map_key_state` 没有 Win 位，所以这里单独查 GetKeyState —— 不查的话
        // Win+R 的 R 会被认领并进拼音缓冲，系统热键还能同时触发。
        // SAFETY: GetKeyState 只读当前线程输入状态队列，无参数约束。
        let win_down =
            unsafe { GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0 };
        if win_down || vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
            return Ok(BOOL(0));
        }
        // 与 OnKeyDown 共用同一映射（`to_engine_keycode`），再交给引擎的只读分流
        // （`TsfLogic::claim_on_test`）：NO_KEYVAL 的功能/导航/媒体键、空缓冲的
        // 退格/回车/翻页、非浏览态的 Esc、Tab 都在这里答 FALSE。锁中毒 → 放行，
        // 宁可让应用收到一次键，也不在未知状态下继续吞。
        let keycode = to_engine_keycode(vk);
        let claim = self
            .state
            .lock_logic()
            .is_some_and(|logic| logic.claim_on_test(keycode));
        Ok(BOOL(if claim { 1 } else { 0 }))
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        // OnKeyUp **只在 OnTestKeyUp 答 TRUE 时才被调用**。Shift 抬起是「轻按切中英」
        // 的判定点，必须认领（Windows 抬起事件的 wParam 是 VK_LSHIFT/VK_RSHIFT，
        // 不是 VK_SHIFT —— 见 `vk::is_shift_vk`）。
        // **ShiftToggle=False 时也认领**：抬起分支还要清掉引擎里的 Single（见
        // handle_key_inner），不认领就清不掉。其余键的抬起本服务没有用途。
        let claim = is_shift_vk(wparam.0 as u32);
        Ok(BOOL(if claim { 1 } else { 0 }))
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // pic 不再丢弃：提交文本要插进的正是这个 context（见 insert_into）。
        Ok(self.handle_key(pic.as_ref(), wparam, lparam))
    }

    fn OnKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // 释放状态由 lParam bit31（转换状态）映射，与按下同路由
        // （logic 对释放事件返回 Consumed/Unhandled，见 input_key）。
        Ok(self.handle_key(pic.as_ref(), wparam, lparam))
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}
