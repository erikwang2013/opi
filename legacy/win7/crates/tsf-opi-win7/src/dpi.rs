// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Win7-safe DPI lookup: `GetDpiForWindow` is Windows 10 1607+, so resolve it
//! dynamically and fall back to the Win7/8 system DPI via `GetDeviceCaps`.
//!
//! Phase 1 has no caret positioning yet; Phase 2's caret code calls this
//! instead of linking `GetDpiForWindow` (which would add a hard import and
//! fail to load on Win7).

use std::sync::OnceLock;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, LOGPIXELSX, ReleaseDC};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::core::{s, w};

type GetDpiForWindowFn = unsafe extern "system" fn(HWND) -> u32;

fn dynamic_get_dpi() -> Option<GetDpiForWindowFn> {
    static F: OnceLock<Option<GetDpiForWindowFn>> = OnceLock::new();
    *F.get_or_init(|| unsafe {
        let user32 = GetModuleHandleW(w!("user32.dll")).ok()?;
        let p = GetProcAddress(user32, s!("GetDpiForWindow"))?;
        Some(std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            GetDpiForWindowFn,
        >(p))
    })
}

/// Window DPI. 0 = unknown; caller treats it as 96.
#[allow(dead_code)] // Phase 2 caret positioning (see module comment).
pub(crate) fn window_dpi(hwnd: HWND) -> u32 {
    if let Some(f) = dynamic_get_dpi() {
        let dpi = unsafe { f(hwnd) };
        if dpi != 0 {
            return dpi;
        }
    }
    unsafe {
        let hdc = GetDC(Some(hwnd));
        if !hdc.is_invalid() {
            let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSX);
            ReleaseDC(Some(hwnd), hdc);
            if dpi > 0 {
                return dpi as u32;
            }
        }
    }
    0
}
