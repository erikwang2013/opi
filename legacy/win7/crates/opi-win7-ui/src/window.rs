// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Win32 candidate window skeleton: a popup that never takes focus
//! (`WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`) with two STATIC lines.
//!
//! Phase 1 has no caret anchoring, no candidate list rendering and no click
//! handling; it proves the window shape and the pipe -> window path compile and
//! run. The reader thread applies messages directly with cross-thread
//! `SetWindowTextW`/`ShowWindow` (both marshal to the owning thread), so no
//! custom message plumbing is needed yet.

use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    MoveWindow, PostQuitMessage, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, WM_CLOSE,
    WM_DESTROY, WM_SIZE, WNDCLASSW, WS_BORDER, WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_POPUP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use crate::pipe::PipeClient;
use crate::protocol::{Msg, parse_line};

const WIDTH: i32 = 420;
const HEIGHT: i32 = 52;
/// Child positions: buffer line then candidate line (two-row placeholder).
const ROWS: [(i32, i32); 2] = [(4, 20), (26, 20)];

static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
/// Buffer label and candidate label HWNDs (0 until created).
static LABELS: [AtomicIsize; 2] = [AtomicIsize::new(0), AtomicIsize::new(0)];

pub fn run() -> Result<(), String> {
    let hinstance =
        unsafe { GetModuleHandleW(None) }.map_err(|e| format!("GetModuleHandleW: {e}"))?;
    let hinstance = HINSTANCE(hinstance.0);

    let class_name = w!("OpiWin7CandidateWindow");
    let mut wc = WNDCLASSW::default();
    wc.lpfnWndProc = Some(wndproc);
    wc.hInstance = hinstance;
    wc.lpszClassName = class_name;
    if unsafe { RegisterClassW(&wc) } == 0 {
        return Err("RegisterClassW failed".into());
    }

    // WS_EX_NOACTIVATE: clicks/keys must not steal focus from the host editor.
    // WS_EX_TOOLWINDOW: keep it out of Alt+Tab and the taskbar.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name,
            w!("opi"),
            WS_POPUP | WS_BORDER,
            200,
            200,
            WIDTH,
            HEIGHT,
            None,
            None,
            Some(hinstance),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW: {e}"))?;
    MAIN_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

    let placeholder = ["opi (Win7) — 等待连接", "候选占位"];
    for (i, text) in placeholder.iter().enumerate() {
        let text_wide = wide(text);
        let label = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                PCWSTR(text_wide.as_ptr()),
                WS_CHILD | WS_VISIBLE,
                8,
                ROWS[i].0,
                WIDTH - 16,
                ROWS[i].1,
                Some(hwnd),
                None,
                Some(hinstance),
                None,
            )
        }
        .map_err(|e| format!("CreateWindowExW(STATIC {i}): {e}"))?;
        LABELS[i].store(label.0 as isize, Ordering::SeqCst);
    }
    layout(hwnd);

    spawn_reader();

    let mut msg = Default::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
    Ok(())
}

/// Build a NUL-terminated UTF-16 buffer for `CreateWindowExW`.
/// The temporary lives until the call returns (the API copies the text).
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn layout(hwnd: HWND) {
    let mut rc = RECT::default();
    // SAFETY: rc is a valid output buffer.
    if unsafe { GetClientRect(hwnd, &mut rc) }.is_err() {
        return;
    }
    let width = (rc.right - rc.left).max(1);
    for (i, (y, h)) in ROWS.iter().enumerate() {
        let label = LABELS[i].load(Ordering::SeqCst);
        if label != 0 {
            // SAFETY: handle was stored from a successful CreateWindowExW.
            let _ = unsafe { MoveWindow(HWND(label as *mut c_void), 8, *y, width - 16, *h, true) };
        }
    }
}

fn spawn_reader() {
    thread::spawn(|| {
        loop {
            if let Ok(mut client) = PipeClient::connect() {
                let mut line = String::new();
                loop {
                    line.clear();
                    match client.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            if let Some(msg) = parse_line(&line) {
                                apply(msg);
                            }
                        }
                    }
                }
            }
            // No server yet (or it restarted): retry quietly, never on the UI thread.
            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn apply(msg: Msg) {
    let raw = MAIN_HWND.load(Ordering::SeqCst);
    if raw == 0 {
        return;
    }
    // SAFETY: stored from a successful CreateWindowExW; process lifetime.
    let hwnd = HWND(raw as *mut c_void);
    match msg {
        Msg::Show {
            buffer, candidates, ..
        } => {
            set_label(0, &buffer);
            set_label(1, &candidates.join("  "));
            // SAFETY: the window is alive for the whole message loop.
            let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        }
        Msg::Hide => {
            // SAFETY: same handle.
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
        Msg::Position { x, y } => {
            // SAFETY: same handle; SWP_NOSIZE keeps the two-row layout.
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                )
            };
        }
        // state_changed is consumed by the Windows 10 status bar, not here.
        Msg::StateChanged
        | Msg::Select { .. }
        | Msg::NextPage
        | Msg::PrevPage
        | Msg::CommitText { .. } => {}
    }
}

fn set_label(index: usize, text: &str) {
    let raw = LABELS[index].load(Ordering::SeqCst);
    if raw == 0 {
        return;
    }
    let text_wide = wide(text);
    // SAFETY: handle stored after successful child creation; buffer is NUL-ended.
    let _ = unsafe { SetWindowTextW(HWND(raw as *mut c_void), PCWSTR(text_wide.as_ptr())) };
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            layout(hwnd);
            LRESULT(0)
        }
        WM_CLOSE => {
            // SAFETY: hwnd is the window receiving this message.
            let _ = unsafe { DestroyWindow(hwnd) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: forwarding the message unchanged is the default contract.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
