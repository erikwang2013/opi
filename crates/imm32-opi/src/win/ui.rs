// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! IMM32 候选窗（从 win.rs 拆出守 500 行）：无激活顶层窗口 + GDI 绘制。
//!
//! 不调用 IMM 的候选列表 API；IMM32 只负责把键送来，候选窗由 IME 自己拥有。
//! 数字/空格选词走 `TsfLogic`，所以窗口先做成显示层，不接管鼠标焦点。

use std::ffi::c_void;
use std::sync::Mutex;

use windows::Win32::Foundation::{COLORREF, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, ClientToScreen, CreateSolidBrush, DEFAULT_GUI_FONT, DeleteObject, EndPaint,
    FillRect, FrameRect, GetStockObject, InvalidateRect, PAINTSTRUCT, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT, TextOutW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetCaretPos, GetClientRect, GetCursorPos, GetSystemMetrics,
    HWND_TOPMOST, IDC_ARROW, LoadCursorW, MA_NOACTIVATE, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN,
    SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow,
    WM_MOUSEACTIVATE, WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};
use windows::core::PCWSTR;

use super::CAND_CLASS_W;

struct Cand {
    hwnd: isize,
    items: Vec<String>,
    page: usize,
    page_count: usize,
}

static CAND: Mutex<Cand> = Mutex::new(Cand {
    hwnd: 0,
    items: Vec::new(),
    page: 0,
    page_count: 0,
});

fn lock_cand() -> std::sync::MutexGuard<'static, Cand> {
    CAND.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn candidate_hwnd() -> HWND {
    let existing = { lock_cand().hwnd };
    if existing != 0 {
        return HWND(existing as *mut c_void);
    }
    let module = match unsafe { GetModuleHandleW(None) } {
        Ok(m) => m,
        Err(_) => return HWND::default(),
    };
    let title: Vec<u16> = "opi".encode_utf16().chain([0]).collect();
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            PCWSTR(CAND_CLASS_W.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            10,
            10,
            None,
            None,
            Some(windows::Win32::Foundation::HINSTANCE(module.0)),
            None,
        )
    };
    let Ok(hwnd) = hwnd else {
        return HWND::default();
    };
    lock_cand().hwnd = hwnd.0 as isize;
    hwnd
}

fn caret_point() -> POINT {
    let focus = unsafe { GetFocus() };
    if !focus.0.is_null() {
        let mut pt = POINT::default();
        if unsafe { GetCaretPos(&mut pt) }.is_ok()
            && unsafe { ClientToScreen(focus, &mut pt) }.as_bool()
        {
            return pt;
        }
    }
    let mut pt = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pt) };
    pt
}

pub(crate) fn show_candidates(items: Vec<String>, page: usize, page_count: usize) {
    let hwnd = candidate_hwnd();
    if hwnd.0.is_null() {
        return;
    }
    {
        let mut c = lock_cand();
        c.items = items.clone();
        c.page = page;
        c.page_count = page_count;
    }
    let lines = items.len() as i32 + if page_count > 1 { 1 } else { 0 };
    let width = items
        .iter()
        .map(|s| s.chars().count() as i32)
        .max()
        .unwrap_or(6)
        .max(6)
        * 18
        + 40;
    let height = lines.max(1) * 22 + 12;
    let pt = caret_point();
    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let x = pt.x.min((screen_w - width - 4).max(0));
    let y = (pt.y + 22).min((screen_h - height - 4).max(0));
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

pub(crate) fn hide_candidates() {
    let hwnd = { lock_cand().hwnd };
    if hwnd != 0 {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut c_void), SW_HIDE);
        }
    }
}

pub(crate) unsafe extern "system" fn cand_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let bg = CreateSolidBrush(COLORREF(0x00F7F7F7));
                FillRect(hdc, &rc, bg);
                let _ = DeleteObject(bg.into());
                let border = CreateSolidBrush(COLORREF(0x00808080));
                FrameRect(hdc, &rc, border);
                let _ = DeleteObject(border.into());
                let old = SelectObject(hdc, GetStockObject(DEFAULT_GUI_FONT));
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, COLORREF(0x00202020));
                let (items, page, page_count) = {
                    let c = lock_cand();
                    (c.items.clone(), c.page, c.page_count)
                };
                for (i, item) in items.iter().enumerate() {
                    let line = format!("{}. {item}", i + 1);
                    let w: Vec<u16> = line.encode_utf16().collect();
                    let _ = TextOutW(hdc, 8, 6 + i as i32 * 22, &w);
                }
                if page_count > 1 {
                    let hint = format!("{}/{}  - / = 翻页", page + 1, page_count);
                    let w: Vec<u16> = hint.encode_utf16().collect();
                    let _ = TextOutW(hdc, 8, 6 + items.len() as i32 * 22, &w);
                }
                let _ = SelectObject(hdc, old);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// 候选窗类注册（`win.rs` 的 `register_classes` 调）。
pub(crate) fn register() -> bool {
    let module = match unsafe { GetModuleHandleW(None) } {
        Ok(m) => m,
        Err(_) => return false,
    };
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();
    let wc = WNDCLASSW {
        style: Default::default(),
        lpfnWndProc: Some(cand_wnd_proc),
        hInstance: windows::Win32::Foundation::HINSTANCE(module.0),
        hCursor: cursor,
        lpszClassName: PCWSTR(CAND_CLASS_W.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) != 0 || GetLastError().0 == 1410 }
}
