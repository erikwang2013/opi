// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! IMM32 胶水：`.ime` 导出、HIMC 状态、`WM_IME_*` 组合串、候选窗。
//!
//! 纯判定在 `session.rs`；这里只做 Win32 翻译。所有导出都 catch_unwind：
//! 本 DLL 被加载进 Chrome/PowerShell 的进程，panic 逃过 FFI = 宿主崩溃。

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use windows::Win32::Foundation::{GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::UI::Input::Ime::{
    GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR, HIMC, IME_CMODE_NATIVE, IME_PROP_AT_CARET,
    IME_PROP_UNICODE, IME_SENTENCE_MODE, IMEINFO, ISC_SHOWUIALL, ImmSetCompositionStringW,
    ImmSetConversionStatus, ImmSetOpenStatus, SCS_CAP_COMPSTR, SCS_SETSTR, TRANSMSG, TRANSMSGLIST,
    UI_CAP_2700,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_IME, DefWindowProcW, IDC_ARROW, LoadCursorW, RegisterClassW, WM_IME_COMPOSITION,
    WM_IME_CONTROL, WM_IME_ENDCOMPOSITION, WM_IME_SETCONTEXT, WM_IME_STARTCOMPOSITION, WNDCLASSW,
};
use windows::core::PCWSTR;

use tsf_opi::dict_path::{self, load_dict, load_trad};
use tsf_opi::logic::{
    KEY_STATE_ALT, KEY_STATE_CAPS_LOCK, KEY_STATE_CTRL, KEY_STATE_RELEASED, KEY_STATE_REPEAT,
    KEY_STATE_SHIFT, TsfLogic,
};
use tsf_opi::vk::vk_to_engine_keycode;

use crate::session::{ImmAction, ImmSession};
use ui::{hide_candidates, show_candidates};

mod ui;

const IME_CLASS: &str = "OpiImm32Wnd";
const IME_CLASS_W: &[u16] = &[79, 112, 105, 73, 109, 109, 51, 50, 87, 110, 100, 0];
pub(crate) const CAND_CLASS_W: &[u16] = &[79, 112, 105, 73, 109, 109, 51, 50, 67, 97, 110, 100, 0];

struct Host {
    session: ImmSession,
    himc: isize,
    /// `ImeProcessKey` 的键状态（含释放位）；ImeToAsciiEx 没有 lParam，必须从这里取。
    pending_states: u32,
}

static HOST: Mutex<Option<Host>> = Mutex::new(None);
static CLASSES: OnceLock<bool> = OnceLock::new();

fn lock_host() -> std::sync::MutexGuard<'static, Option<Host>> {
    HOST.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// 安装目录：安装器写 `HKLM\Software\opi` 的 `InstallDir`；缺省回落 Program Files。
fn install_dir() -> Option<PathBuf> {
    let sub = wide("Software\\opi");
    let val = wide("InstallDir");
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * std::mem::size_of::<u16>()) as u32;
    let err = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(sub.as_ptr()),
            PCWSTR(val.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast::<c_void>()),
            Some(&mut size),
        )
    };
    if err.0 == 0 && size >= 2 {
        let units = (size as usize / 2).saturating_sub(1);
        return Some(PathBuf::from(String::from_utf16_lossy(&buf[..units])));
    }
    std::env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join("opi"))
}

fn build_session() -> ImmSession {
    let lap = std::env::var_os("LOCALAPPDATA");
    let app = install_dir().map(|p| p.join("app"));
    let dict = load_dict(None, app.as_deref(), lap.as_deref());
    let trad = load_trad(None, app.as_deref(), lap.as_deref());
    let mut logic = TsfLogic::with_dicts(dict.dict, trad.dict);
    let dicts = dict_path::dicts_dir(lap.as_deref());
    logic.apply_settings(tsf_opi::settings::load(lap.as_deref()), &dicts);
    logic.apply_custom_phrases(tsf_opi::custom_phrases::load(&dicts));
    logic.apply_english_custom_phrases(tsf_opi::custom_phrases::load_english(&dicts));
    ImmSession::new(logic)
}

fn ensure_host(himc: HIMC) -> bool {
    let mut g = lock_host();
    if g.is_none() {
        *g = Some(Host {
            session: build_session(),
            himc: himc.0 as isize,
            pending_states: 0,
        });
    } else if let Some(h) = g.as_mut() {
        h.himc = himc.0 as isize;
    }
    true
}

fn register_classes() -> bool {
    let module = match unsafe { GetModuleHandleW(None) } {
        Ok(m) => m,
        Err(_) => return false,
    };
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();
    let ime = WNDCLASSW {
        style: CS_IME,
        lpfnWndProc: Some(ime_wnd_proc),
        hInstance: HINSTANCE(module.0),
        hCursor: cursor,
        lpszClassName: PCWSTR(IME_CLASS_W.as_ptr()),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&ime) } == 0 && unsafe { GetLastError() }.0 != 1410 {
        return false;
    }
    ui::register()
}

fn classes_ok() -> bool {
    *CLASSES.get_or_init(register_classes)
}

unsafe extern "system" fn ime_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            // 组合串由宿主按 GCS_COMPSTR 画；候选窗是我们自己的窗口，关掉 IMM 默认 UI。
            WM_IME_SETCONTEXT => {
                let masked = LPARAM(lparam.0 & !(ISC_SHOWUIALL as isize));
                DefWindowProcW(hwnd, msg, wparam, masked)
            }
            WM_IME_CONTROL => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn key_states(lparam: LPARAM, ks: &[u8; 256]) -> u32 {
    let lp = lparam.0 as u32;
    let mut s = 0u32;
    if lp & 0x8000_0000 != 0 {
        s |= KEY_STATE_RELEASED;
    }
    if lp & 0x4000_0000 != 0 {
        s |= KEY_STATE_REPEAT;
    }
    let down = |vk: u16| ks[vk as usize] & 0x80 != 0;
    if down(VK_SHIFT.0) || down(VK_LSHIFT.0) || down(VK_RSHIFT.0) {
        s |= KEY_STATE_SHIFT;
    }
    if down(VK_CONTROL.0) || down(VK_LCONTROL.0) || down(VK_RCONTROL.0) {
        s |= KEY_STATE_CTRL;
    }
    if down(VK_MENU.0) || down(VK_LMENU.0) || down(VK_RMENU.0) {
        s |= KEY_STATE_ALT;
    }
    if unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0 {
        s |= KEY_STATE_CAPS_LOCK;
    }
    s
}

fn engine_keycode(vk: u32, scan: u32, ks: &[u8; 256]) -> u32 {
    let mut buf = [0u16; 8];
    let n = unsafe { ToUnicodeEx(vk, scan, ks, &mut buf, 0x4, None) };
    vk_to_engine_keycode(vk, n, buf[0])
}

fn set_comp_string(himc: HIMC, text: &str) {
    let units: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let _ = ImmSetCompositionStringW(
            himc,
            SCS_SETSTR,
            Some(units.as_ptr().cast::<c_void>()),
            (units.len() * 2) as u32,
            None,
            0,
        );
    }
}

fn transmsg(message: u32, lparam: isize) -> TRANSMSG {
    TRANSMSG {
        message,
        wParam: WPARAM(0),
        lParam: LPARAM(lparam),
    }
}

/// 执行动作，把需要 IMM 转发的消息写进 `out`（容量由 `uMsgCount` 给）。
fn apply_actions(himc: HIMC, actions: Vec<ImmAction>, out: &mut Vec<TRANSMSG>) {
    for action in actions {
        match action {
            ImmAction::StartComposition => out.push(transmsg(WM_IME_STARTCOMPOSITION, 0)),
            ImmAction::SetComposition(text) => {
                set_comp_string(himc, &text);
                let flags = (GCS_COMPSTR.0 | GCS_CURSORPOS.0) as isize;
                out.push(transmsg(WM_IME_COMPOSITION, flags));
            }
            ImmAction::EndComposition => out.push(transmsg(WM_IME_ENDCOMPOSITION, 0)),
            ImmAction::Result(text) => {
                set_comp_string(himc, &text);
                out.push(transmsg(WM_IME_COMPOSITION, GCS_RESULTSTR.0 as isize));
            }
            ImmAction::ShowCandidates {
                items,
                page,
                page_count,
            } => show_candidates(items, page, page_count),
            ImmAction::HideCandidates => hide_candidates(),
        }
    }
}

// ---------- `.ime` 导出 ----------

#[unsafe(no_mangle)]
pub extern "system" fn ImeInquire(info: *mut IMEINFO, class: *mut u16, _flags: u32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        if info.is_null() || class.is_null() {
            return 0;
        }
        if !classes_ok() {
            return 0;
        }
        *info = IMEINFO {
            dwPrivateDataSize: 0,
            fdwProperty: IME_PROP_UNICODE | IME_PROP_AT_CARET,
            fdwConversionCaps: IME_CMODE_NATIVE.0,
            fdwSentenceCaps: 0,
            fdwUICaps: UI_CAP_2700,
            fdwSCSCaps: SCS_CAP_COMPSTR,
            fdwSelectCaps: 0,
        };
        let name: Vec<u16> = IME_CLASS.encode_utf16().chain([0]).collect();
        std::ptr::copy_nonoverlapping(name.as_ptr(), class, name.len().min(32));
        if name.len() < 32 {
            *class.add(name.len()) = 0;
        }
        1
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeSelect(himc: HIMC, select: i32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        if himc.0.is_null() {
            return 0;
        }
        if select != 0 {
            if !ensure_host(himc) {
                return 0;
            }
            if let Some(h) = lock_host().as_mut() {
                h.session.reset();
            }
            let _ = ImmSetOpenStatus(himc, true);
            let _ = ImmSetConversionStatus(himc, IME_CMODE_NATIVE, IME_SENTENCE_MODE(0));
        } else {
            hide_candidates();
            if let Some(h) = lock_host().as_mut() {
                h.session.reset();
            }
        }
        1
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeSetActiveContext(himc: HIMC, focus: i32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if himc.0.is_null() {
            return 0;
        }
        if focus != 0 {
            ensure_host(himc);
        } else {
            hide_candidates();
        }
        1
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeProcessKey(
    himc: HIMC,
    vk: u32,
    lparam: LPARAM,
    key_state: *const u8,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        if himc.0.is_null() || key_state.is_null() {
            return 0;
        }
        let ks = &*key_state.cast::<[u8; 256]>();
        let states = key_states(lparam, ks);
        let code = engine_keycode(vk, ((lparam.0 >> 16) & 0xff) as u32, ks);
        if !ensure_host(himc) {
            return 0;
        }
        let claimed = {
            let mut g = lock_host();
            g.as_mut()
                .map(|h| {
                    h.pending_states = states;
                    h.session.claim(code, states)
                })
                .unwrap_or(false)
        };
        if claimed { 1 } else { 0 }
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeToAsciiEx(
    vk: u32,
    scan: u32,
    key_state: *const u8,
    trans: *mut TRANSMSGLIST,
    _state: u32,
    himc: HIMC,
) -> u32 {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        if himc.0.is_null() || key_state.is_null() || trans.is_null() {
            return 0;
        }
        let ks = &*key_state.cast::<[u8; 256]>();
        let code = engine_keycode(vk, scan, ks);
        if !ensure_host(himc) {
            return 0;
        }
        // ImeToAsciiEx 没有 lParam：释放/重复位取 ImeProcessKey 刚存下的那一份。
        let actions = {
            let mut g = lock_host();
            g.as_mut()
                .map(|h| {
                    let states = h.pending_states;
                    h.pending_states = 0;
                    h.session.process_at(code, states, Instant::now())
                })
                .unwrap_or_default()
        };
        let mut msgs = Vec::new();
        apply_actions(himc, actions, &mut msgs);
        let cap = (*trans).uMsgCount as usize;
        let n = msgs.len().min(cap);
        for (i, m) in msgs.into_iter().take(n).enumerate() {
            (*trans).TransMsg[i] = m;
        }
        n as u32
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn NotifyIME(himc: HIMC, action: u32, _index: u32, _value: u32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if himc.0.is_null() {
            return 0;
        }
        // NI_COMPOSITIONSTR = 21; CPS_CANCEL/COMPLETE 都收尾当前组合。
        if action == 21 {
            hide_candidates();
            if let Some(h) = lock_host().as_mut() {
                h.session.reset();
            }
        }
        1
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeSetCompositionString(
    _himc: HIMC,
    _index: u32,
    _comp: *const c_void,
    _comp_len: u32,
    _read: *const c_void,
    _read_len: u32,
) -> i32 {
    1
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeConfigure(
    _hkl: isize,
    _hwnd: HWND,
    _mode: u32,
    _data: *mut c_void,
) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeConversionList(
    _himc: HIMC,
    _source: *const u16,
    _dest: *mut c_void,
    _len: u32,
    _flag: u32,
) -> u32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeRegisterWord(_read: *const u16, _style: u32, _word: *const u16) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeUnregisterWord(_read: *const u16, _style: u32, _word: *const u16) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeGetRegisterWordStyle(_n: u32, _buf: *mut c_void) -> u32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeEnumRegisterWord(
    _proc: *const c_void,
    _read: *const u16,
    _style: u32,
    _data: *mut c_void,
) -> u32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn ImeEscape(_himc: HIMC, _escape: u32, _data: *mut c_void) -> isize {
    0
}
