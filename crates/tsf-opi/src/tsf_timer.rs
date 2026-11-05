// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 直插文本的即时落地：按键线程上的 message-only 窗口（从 state.rs 拆出守 500 行）。
//!
//! `state.rs` 的线程模型决定了直插只能在宿主 UI 线程上走 COM；读线程入队后原先
//! 只能等下一次按键/焦点。语音每 3 秒提交一段，那条路径变成「停止后才出字」。
//! 读线程现在同步 `SendMessage(DRAIN_MESSAGE)` 到这个窗口（`state_text.rs`），本文件
//! 在 UI 线程上把队列取空 —— 与按键线程的 drain 同一套 `insert_text`，不新增
//! 插入实现。同步而不是 Post：宿主按窗口过滤消息泵时也能落地，且能如实回报结果。
//!
//! 窗口只在 Activate..Deactivate 之间存在；销毁时先摘登记再 DestroyWindow，
//! Deactivate 的竞态投递只会失败，不会碰到已释放的状态（见 `wake_drain`）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicPtr, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleHandleExW,
};
use windows::Win32::UI::TextServices::ITfContext;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetWindowLongPtrW, HWND_MESSAGE,
    PostMessageW, RegisterClassW, SetWindowLongPtrW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_NCDESTROY, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

use crate::state::{DRAIN_MESSAGE, TsfSharedState};

const CLASS_NAME: PCWSTR = w!("OpiTsfDrainWindow");

/// 窗口与 state 之间的稳定间接层：窗口用 userdata 持它，state 释放前先把
/// `state` 置空。这样即使 Drop 发生在非窗口线程（DestroyWindow 失败，只能
/// PostMessage(WM_CLOSE) 让窗口稍后销毁），迟到的 DRAIN_MESSAGE 也解引用不到
/// 已释放的 state。
struct DrainCtx {
    state: AtomicPtr<TsfSharedState>,
}

impl TsfSharedState {
    /// **按键线程**调（Activate）：建窗口并登记；重复 Activate 不重建。
    /// 失败只记一行：语音仍会退回到「下一次按键/焦点」的旧路径，不影响打字。
    pub(super) fn start_drain_window(&self) {
        if self.drain_hwnd().is_some() {
            return;
        }
        // SAFETY: 全部 Win32 调用都在创建窗口的线程（Activate 所在 UI 线程）上。
        let hwnd = unsafe {
            // GetModuleHandleW(None) 给的是宿主 EXE —— 窗口类要登记在本 DLL 名下。
            let mut module = HMODULE::default();
            let proc =
                drain_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;
            if GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR(proc as *const u16),
                &mut module,
            )
            .is_err()
            {
                return;
            }
            let instance = HINSTANCE::from(module);
            let class = WNDCLASSW {
                lpfnWndProc: Some(drain_proc),
                hInstance: instance,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            // 同名类已注册时返回 0；那是重复 Activate，不是失败。
            RegisterClassW(&class);
            match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS_NAME,
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance),
                None,
            ) {
                Ok(hwnd) => hwnd,
                Err(e) => {
                    tlog!("tsf-opi: 建语音上屏窗口失败: {e}");
                    return;
                }
            }
        };
        // SAFETY: ctx 在 WM_NCDESTROY 释放；state 在 stop 时先置空再销毁窗口。
        unsafe {
            let ctx = Box::into_raw(Box::new(DrainCtx {
                state: AtomicPtr::new(self as *const TsfSharedState as *mut _),
            }));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx as isize);
        }
        self.set_drain_hwnd(Some(hwnd));
    }

    /// **按键线程**调（Deactivate/Drop）：先摘登记再销毁，读线程随后投递只会失败。
    pub(super) fn stop_drain_window(&self) {
        let Some(hwnd) = self.take_drain_hwnd() else {
            return;
        };
        // 先让窗口过程不再解引用 state（可能正有一个 DRAIN_MESSAGE 排在队列里）。
        // SAFETY: userdata 在 WM_NCDESTROY 之前一直指向有效的 DrainCtx。
        let ctx = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DrainCtx;
        if !ctx.is_null() {
            unsafe { (*ctx).state.store(std::ptr::null_mut(), Ordering::Release) };
        }
        // SAFETY: 正常路径与创建同线程；非所属线程时 DestroyWindow 失败，改投
        // WM_CLOSE 让所属线程销毁（DefWindowProc 的默认行为），不会泄漏裸指针。
        if unsafe { DestroyWindow(hwnd) }.is_err() {
            let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }

    /// 窗口消息：取空直插队列，返回真正落进文档的段数。
    /// 目标为空（用户还没敲过键）时先从线程焦点补取，否则 `insert_text` 会因为
    /// 无 context 静默丢弃 —— 正是「点了话筒直接说」的场景。
    fn drain_speech(&self) -> usize {
        if !self.has_pending_text() {
            return 0; // 空队列不碰 COM
        }
        if !self.target_present()
            && let Some(pic) = self.focus_context()
        {
            self.remember_target(Some(&pic));
        }
        let mut inserted = 0;
        while let Some((text, seq)) = self.take_pending_text() {
            if self.insert_text(&text, seq) {
                inserted += 1;
            }
        }
        inserted
    }

    /// 当前焦点文档的 base context（`ITfThreadMgr::GetFocus`）。
    fn focus_context(&self) -> Option<ITfContext> {
        let manager = self.thread_mgr.lock().ok()?.clone()?;
        // SAFETY: GetFocus/GetBase 只能在所属线程调；本函数由窗口过程或按键线程触发。
        let document = unsafe { manager.GetFocus().ok()? };
        unsafe { document.GetBase().ok() }
    }
}

impl Drop for TsfSharedState {
    fn drop(&mut self) {
        // Deactivate 正常已销毁；这里是服务对象未走 Deactivate 就释放的兜底，
        // 防止窗口里的裸指针在对象释放后仍被定时/唤醒消息解引用。
        self.stop_drain_window();
    }
}

/// 只有 `DRAIN_MESSAGE` 有行为；其余交默认处理。
///
/// SAFETY: 本函数由 user32 在创建窗口的线程上调用；panic 不得越过 FFI 边界，
/// 故队列处理整体包在 catch_unwind 里（与 TSF 各 COM 出口同一策略）。
unsafe extern "system" fn drain_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: userdata 在窗口创建后由 start_drain_window 写入，WM_NCDESTROY 前有效。
    let ctx = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DrainCtx;
    if msg == DRAIN_MESSAGE {
        let mut inserted = 0usize;
        if !ctx.is_null() {
            // SAFETY: stop_drain_window 先把 state 置空再销毁窗口，Acquire 保证看到空值。
            let state = unsafe { (*ctx).state.load(Ordering::Acquire) };
            if !state.is_null() {
                inserted = catch_unwind(AssertUnwindSafe(|| unsafe { (*state).drain_speech() }))
                    .unwrap_or(0);
            }
        }
        // +1：让 SendMessageTimeout 能区分「处理了但没插入(1)」和「超时(0)」。
        return LRESULT(inserted as isize + 1);
    }
    if msg == WM_NCDESTROY {
        // SAFETY: 同线程、窗口仍有效；清 userdata 并释放 ctx（此后再无本窗口消息）。
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !ctx.is_null() {
                drop(Box::from_raw(ctx));
            }
        }
    }
    // SAFETY: 参数原样交给默认窗口过程。
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
