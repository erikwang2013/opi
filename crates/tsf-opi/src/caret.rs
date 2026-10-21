// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! caret（输入位置）→ 候选窗左上角（Windows 目标专属，从 `tsf.rs` 拆出守 500 行）。
//!
//! 2026-10-22 用户报障「候选词框要挨着输入的位置」：协议里早就有 `position` 消息，
//! 但**没有任何调用方**（`CandidateClient::position` 一直是死代码），窗口落在固定
//! 位置。本模块补上发端：每次 composition 变化时取当前 caret 的屏幕矩形，折算成
//! 候选窗能直接用的坐标。
//!
//! 【坐标空间】TSF 给的是**物理屏幕像素**；候选窗（Compose/AWT）的 `WindowPosition`
//! 用的是**逻辑像素**（= `.dp` 数值，见 `desktop/.../Main.kt` 协议头）。故这里按
//! 输入窗口的 DPI（`GetDpiForWindow`）折回逻辑像素 —— 100% 缩放时两者相等，
//! 125%/150% 下不折算就会偏出去一截。**发端不得预乘 density**（协议头那条），
//! 但**必须**把物理像素除掉缩放；这是同一件事的两面。
//!
//! 【为什么用编辑会话取选区】`GetSelection`/`GetTextExt` 的 `ec` 在文档锁内最可靠；
//! `TF_ES_READ | TF_ES_SYNC` 让 `DoEditSession` 在本线程、本栈同步跑完，返回时结果
//! 已经写好。**重入警告**：本模块在 `handle_key_inner` 持 `logic` 锁时被调用，
//! `CaretSession` 因此绝不能碰 `logic`/`sink`（`std::sync::Mutex` 不可重入）。
//!
//! 【拿不到就静默】宿主不支持（非文档 context）、选区为空、编辑会话被拒：都返回
//! `None`，候选窗沿用上一次位置（或窗口自己的默认落点）。位置只是体验，不该让
//! 一次按键失败。

use std::mem::{ManuallyDrop, take};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::RECT;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::TextServices::{
    ITfContext, ITfContextView, ITfEditSession, ITfEditSession_Impl, TF_ANCHOR_END,
    TF_DEFAULT_SELECTION, TF_ES_READ, TF_ES_SYNC, TF_SELECTION,
};
use windows::core::{BOOL, Result, implement};

use crate::sink::ensure_panic_hook;

/// caret 底边到候选窗顶边的间距（逻辑像素）。2 是「贴着但不压住光标」。
const CARET_GAP: i32 = 2;

/// 取 caret 位置 → 候选窗左上角（AWT 逻辑像素）。`tid` 为 `Activate` 记下的 client id；
/// 0（未激活）直接放弃 —— 没有合法 tid，`RequestEditSession` 必失败。
///
/// 必须在**按键线程**调用（`ITfContext` 是 STA 指针，跨线程未 marshal 是未定义行为）。
pub(crate) fn candidate_anchor(pic: &ITfContext, tid: u32) -> Option<(i32, i32)> {
    if tid == 0 {
        return None;
    }
    let slot: Arc<Mutex<Option<(i32, i32)>>> = Arc::new(Mutex::new(None));
    let session: ITfEditSession = CaretSession {
        pic: pic.clone(),
        anchor: Arc::clone(&slot),
    }
    .into();
    // SAFETY: 会话对象在本次调用期间存活；`TF_ES_SYNC` 下 `DoEditSession` 与本次
    // 调用同步完成（异步排队时下面读到 None）。
    let _ = unsafe { pic.RequestEditSession(tid, &session, TF_ES_READ | TF_ES_SYNC) };
    let from_session = slot.lock().ok().and_then(|mut g| g.take());
    // 回退（2026-10-22）：部分宿主在按键回调里不给同步读会话（返回 TF_E_SYNCHRONOUS
    // 或直接拒绝）。**默认选区**是 TSF 专门为「没有编辑会话时取选区」提供的特殊
    // cookie，直读一次；GetTextExt 在少数宿主上同样要 cookie，失败仍是 None ——
    // 不比现状差，只是多给一次机会（症状是候选窗一直停在默认落点）。
    from_session.or_else(|| read_anchor(pic, TF_DEFAULT_SELECTION))
}

/// 只读编辑会话：拿选区 → 选区末尾的屏幕矩形 → 折算逻辑像素。
#[implement(ITfEditSession)]
struct CaretSession {
    pic: ITfContext,
    /// 结果槽：`DoEditSession` 写、调用方读。`Arc` 是因为 `#[implement]` 生成的对象
    /// 只暴露接口、拿不回结构体字段（与 `state.rs` 的编辑会话同一套写法）。
    anchor: Arc<Mutex<Option<(i32, i32)>>>,
}

impl ITfEditSession_Impl for CaretSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        ensure_panic_hook();
        // COM 出口：panic 逃出去 = abort 宿主进程（同 state.rs 的编辑会话）。
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_anchor(&self.pic, ec)))
                .ok()
                .flatten();
        if let Some(anchor) = result
            && let Ok(mut slot) = self.anchor.lock()
        {
            *slot = Some(anchor);
        }
        Ok(())
    }
}

/// 选区末尾的屏幕矩形 → 逻辑像素坐标（caret 左下角）。失败 = None（静默）。
fn read_anchor(pic: &ITfContext, ec: u32) -> Option<(i32, i32)> {
    // SAFETY: 只读调用；`sel` 由本函数持有，取出的 range 在函数结束前释放。
    let mut sel: [TF_SELECTION; 1] = Default::default();
    let mut fetched = 0u32;
    unsafe { pic.GetSelection(ec, 0, &mut sel, &mut fetched) }.ok()?;
    if fetched == 0 {
        return None;
    }
    // 取走 range：`TF_SELECTION::range` 是 `ManuallyDrop`（原样丢弃会漏一次 Release），
    // 必须 `into_inner` 后按普通 `Option<ITfRange>` 释放。见 state.rs 里同一条备忘。
    let TF_SELECTION { range, .. } = take(&mut sel[0]);
    let range = ManuallyDrop::into_inner(range)?;
    // 折叠到选区**末尾**：拖选一段后开始输入，插入点在末尾，候选窗要跟末尾走。
    // `range` 是刚从 TF_SELECTION 里取出的自有引用，直接折叠即可 —— 不必先 Clone
    // 一份（review 2026-10-22：每次取位置都白做一次范围复制）。
    unsafe { range.Collapse(ec, TF_ANCHOR_END) }.ok()?;

    let view = unsafe { pic.GetActiveView() }.ok()?;
    let mut rc = RECT::default();
    let mut clipped = BOOL(0);
    // SAFETY: `range` 是本次编辑会话内取得的有效范围；`rc`/`clipped` 都是出参。
    unsafe { view.GetTextExt(ec, &range, &mut rc, &mut clipped) }.ok()?;
    // 完全空/无效的矩形（部分宿主对无焦点视图返回全 0）不当作位置：放弃本次，
    // 窗口沿用上一次位置。插入点本身是**零宽**的（right == left），这是合法值。
    if rc.right <= rc.left && rc.bottom <= rc.top {
        return None;
    }
    let (left, bottom) = (rc.left, rc.bottom);
    // 极端负值多半是宿主填的哨兵（正常屏幕坐标不会到 i32::MIN/2）：别把窗口甩飞。
    if left <= i32::MIN / 2 || bottom <= i32::MIN / 2 {
        return None;
    }
    Some(to_logical(left, bottom, view_dpi(&view)))
}

/// 输入窗口的 DPI。拿不到（无 HWND / 直接写屏应用）按 96 处理 —— 即不折算。
fn view_dpi(view: &ITfContextView) -> u32 {
    // SAFETY: GetWnd 只读视图属性；句柄无效时 GetDpiForWindow 返回 0，下面按 96 处理。
    let Ok(hwnd) = (unsafe { view.GetWnd() }) else {
        return 0;
    };
    if hwnd.is_invalid() {
        return 0;
    }
    unsafe { GetDpiForWindow(hwnd) }
}

/// 物理像素 → AWT 逻辑像素（`dpi=0`/96 = 恒等）。`bottom + CARET_GAP` 是候选窗顶边。
fn to_logical(left: i32, bottom: i32, dpi: u32) -> (i32, i32) {
    let dpi = if dpi == 0 { 96 } else { i64::from(dpi) };
    let scale = |v: i32| (i64::from(v) * 96 / dpi) as i32;
    (scale(left), scale(bottom) + CARET_GAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 96 DPI = 恒等（只加 2px 的贴边间距）—— 100% 缩放是绝大多数桌面。
    #[test]
    fn at_96_dpi_coordinates_are_unchanged_plus_the_gap() {
        assert_eq!(to_logical(120, 340, 96), (120, 342));
        assert_eq!(to_logical(120, 340, 0), (120, 342), "拿不到 DPI 按 96");
    }

    /// 150% 缩放：1500 物理像素 = 1000 逻辑像素；候选窗不折算就会偏出 50%。
    #[test]
    fn at_125_and_150_percent_physical_pixels_are_scaled_back() {
        assert_eq!(to_logical(250, 500, 120), (200, 402));
        assert_eq!(to_logical(300, 600, 144), (200, 402));
    }

    /// 负坐标（多屏左侧/上方的显示器）同样折算，别截成 0。
    #[test]
    fn negative_coordinates_survive_scaling() {
        assert_eq!(to_logical(-480, -240, 144), (-320, -158));
    }
}
