// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! IMM32 胶水：`.ime` 导出、HIMC 状态、`WM_IME_*` 组合串、候选窗。
//!
//! 纯判定在 `session.rs`；这里只做 Win32 翻译。所有导出都 catch_unwind：
//! 本 DLL 被加载进 Chrome/PowerShell 的进程，panic 逃过 FFI = 宿主崩溃。

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use windows::Win32::Foundation::{GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::UI::Input::Ime::{
    CPS_CANCEL, CPS_COMPLETE, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR, HIMC, IME_CMODE_NATIVE,
    IME_PROP_AT_CARET, IME_PROP_UNICODE, IME_SENTENCE_MODE, IMEINFO, ISC_SHOWUIALL, ImmLockIMC,
    ImmSetCompositionStringW, ImmSetConversionStatus, ImmSetOpenStatus, ImmUnlockIMC,
    NI_COMPOSITIONSTR, SCS_CAP_COMPSTR, SCS_SETSTR, TRANSMSG, TRANSMSGLIST, UI_CAP_2700,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
    VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_IME, DefWindowProcW, IDC_ARROW, LoadCursorW, PostMessageW, RegisterClassW,
    WM_IME_COMPOSITION, WM_IME_CONTROL, WM_IME_ENDCOMPOSITION, WM_IME_SETCONTEXT,
    WM_IME_STARTCOMPOSITION, WNDCLASSW,
};
use windows::core::PCWSTR;

use tsf_opi::dict_path::{self, load_dict, load_trad};
use tsf_opi::logic::{
    KEY_STATE_ALT, KEY_STATE_CAPS_LOCK, KEY_STATE_CTRL, KEY_STATE_RELEASED, KEY_STATE_REPEAT,
    KEY_STATE_SHIFT, TsfLogic,
};
use tsf_opi::vk::{
    ModeHotkey, fullwidth_hotkey, hotkey_should_act, is_shift_vk, mode_hotkey, vk_to_engine_keycode,
};

use crate::dict::shared;
use crate::hosts::HostMap;
use crate::session::{ImmAction, ImmSession};
use ui::{hide_candidates, show_candidates};

mod exports;
mod ui;

const IME_CLASS: &str = "OpiImm32Wnd";
const IME_CLASS_W: &[u16] = &[79, 112, 105, 73, 109, 109, 51, 50, 87, 110, 100, 0];
pub(crate) const CAND_CLASS_W: &[u16] = &[79, 112, 105, 73, 109, 109, 51, 50, 67, 97, 110, 100, 0];

struct Host {
    session: ImmSession,
    /// `ImeProcessKey` 的键状态（含释放位）；ImeToAsciiEx 没有 lParam，必须从这里取。
    pending_states: u32,
    /// 引擎前的热键：ImeProcessKey 判定，ImeToAsciiEx 执行。
    /// 这两项随 Host 一起按 HIMC 隔离 —— 旧实现进程级单槽，两个上下文的
    /// ImeProcessKey/ImeToAsciiEx 交错时会执行对方的热键。
    pending_hotkey: PendingHotkey,
}

#[derive(Clone, Copy)]
enum PendingHotkey {
    None,
    /// 认领但不动作（按住重复/抬起）。
    Ignore,
    Mode(ModeHotkey),
    Fullwidth,
}

/// 进程内同时保留的 HIMC 槽位上限。主词库已进程级共享；导入词库/五笔码表
/// 仍随每个 Host 的 `apply_settings` 各装一份，这里给它们一个上限（超限淘汰
/// 最久未用且不持焦点的 Host，见 `hosts.rs`）。正常 `ImeSelect(FALSE)` 会移除。
const MAX_HOSTS: usize = 8;

/// 进程级只读词库：同一进程里所有 HIMC 共用这一份（不再每槽位重装载/校验）。
///
/// 只覆盖主库/繁体库：导入词库与五笔码表由 `TsfLogic::apply_settings` 装进各
/// 自 Engine 的 `LayeredDictionary`（具体 `InMemoryDictionary`，外部注不进共享
/// 句柄），所以那两层仍按 Host 各一份 —— 由 `MAX_HOSTS` 封顶并随
/// `ImeSelect(FALSE)` 释放；主库（体积与校验成本的大头）不再重复。
struct DictBundle {
    dict: Arc<dyn engine_core::Dictionary>,
    /// `None` = 没装繁体词库（`load_trad` 的唯一空态），引擎按主库回退。
    trad: Option<Arc<dyn engine_core::Dictionary>>,
}

static BUNDLE: OnceLock<DictBundle> = OnceLock::new();
static HOSTS: OnceLock<Mutex<HostMap<Host>>> = OnceLock::new();
static CLASSES: OnceLock<bool> = OnceLock::new();

fn lock_hosts() -> std::sync::MutexGuard<'static, HostMap<Host>> {
    HOSTS
        .get_or_init(|| Mutex::new(HostMap::new(MAX_HOSTS)))
        .lock()
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

/// 进程级只读词库：装载一次，所有 HIMC 共享（`dict.rs` 的 `Arc` 外壳）。
/// `OPI_DICT_PATH` 只在宿主进程首次用到输入法时读一次，改它要重启宿主
/// —— 旧实现每次新建 Host 都重读，但那是实现细节，从未是文档承诺。
fn dict_bundle() -> &'static DictBundle {
    BUNDLE.get_or_init(|| {
        let lap = std::env::var_os("LOCALAPPDATA");
        let app = install_dir().map(|p| p.join("app"));
        let dict = load_dict(None, app.as_deref(), lap.as_deref());
        let trad = load_trad(None, app.as_deref(), lap.as_deref());
        DictBundle {
            dict: Arc::from(dict.dict),
            trad: trad.dict.map(Arc::from),
        }
    })
}

fn build_session() -> ImmSession {
    let b = dict_bundle();
    let mut logic = TsfLogic::with_dicts(shared(&b.dict), b.trad.as_ref().map(shared));
    let lap = std::env::var_os("LOCALAPPDATA");
    let dicts = dict_path::dicts_dir(lap.as_deref());
    logic.apply_settings(tsf_opi::settings::load(lap.as_deref()), &dicts);
    logic.apply_custom_phrases(tsf_opi::custom_phrases::load(&dicts));
    logic.apply_english_custom_phrases(tsf_opi::custom_phrases::load_english(&dicts));
    ImmSession::new(logic)
}

impl Host {
    fn new() -> Self {
        Self {
            session: build_session(),
            pending_states: 0,
            pending_hotkey: PendingHotkey::None,
        }
    }
}

/// 重新读设置/自定义短语（IMM32 没有 TSF 的 CtlOp::Reload 通道；每个宿主进程
/// 在 `ImeSelect(TRUE)` 时重读一次，改设置后切回输入法即可生效）。
fn reload_settings(h: &mut Host) {
    let lap = std::env::var_os("LOCALAPPDATA");
    let dicts = dict_path::dicts_dir(lap.as_deref());
    h.session
        .logic_mut()
        .apply_settings(tsf_opi::settings::load(lap.as_deref()), &dicts);
    h.session
        .logic_mut()
        .apply_custom_phrases(tsf_opi::custom_phrases::load(&dicts));
    h.session
        .logic_mut()
        .apply_english_custom_phrases(tsf_opi::custom_phrases::load_english(&dicts));
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

/// 热键要按**布局字符**判（AZERTY/Dvorak 上 ' 与 \ 不在 US 位置）：
/// 复制键盘状态、清掉 Ctrl/Alt（Shift 保留），再问 ToUnicodeEx。
fn layout_char(vk: u32, ks: &[u8; 256]) -> Option<char> {
    let mut copy = *ks;
    for m in [
        VK_CONTROL,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
    ] {
        copy[m.0 as usize] = 0;
    }
    let mut buf = [0u16; 8];
    let n = unsafe { ToUnicodeEx(vk, 0, &copy, &mut buf, 0x4, None) };
    (n >= 1).then(|| char::from_u32(buf[0] as u32)).flatten()
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
    // 候选窗只有一个：后台 HIMC 的动作不得显示/隐藏焦点上下文正在看的窗口。
    let owns_candidates = lock_hosts().owns_candidates(himc.0 as isize);
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
            } => {
                if owns_candidates {
                    show_candidates(items, page, page_count);
                }
            }
            ImmAction::HideCandidates => {
                if owns_candidates {
                    hide_candidates();
                }
            }
        }
    }
}

/// 结束当前 HIMC 的组合：`commit=true` 时把可见的拼音作为结果串交给宿主。
///
/// `NotifyIME(CPS_COMPLETE)` / `ImeSelect(FALSE)` 没有 TRANSMSGLIST 可填；用
/// `ImmLockIMC` 取宿主窗口后 **Post** 两条消息（不 Send，避免在 IME 回调里
/// 重入宿主消息循环）。失败时至少清掉本地状态，不让下次输入带旧缓冲。
fn finish_composition(himc: HIMC, commit: bool) {
    let key = himc.0 as isize;
    let text = {
        let mut g = lock_hosts();
        match g.get_mut(key) {
            Some(h) if commit => h.session.logic().display_buffer(),
            _ => String::new(),
        }
    };
    if !text.is_empty() {
        set_comp_string(himc, &text);
    }
    let hwnd = unsafe {
        let imc = ImmLockIMC(himc);
        if imc.is_null() {
            GetFocus()
        } else {
            let h = (*imc).hWnd;
            let _ = ImmUnlockIMC(himc);
            if h.0.is_null() { GetFocus() } else { h }
        }
    };
    if !hwnd.0.is_null() {
        unsafe {
            if !text.is_empty() {
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_IME_COMPOSITION,
                    WPARAM(0),
                    LPARAM(GCS_RESULTSTR.0 as isize),
                );
            }
            let _ = PostMessageW(Some(hwnd), WM_IME_ENDCOMPOSITION, WPARAM(0), LPARAM(0));
        }
    }
    if lock_hosts().owns_candidates(key) {
        hide_candidates();
    }
    if let Some(h) = lock_hosts().get_mut(key) {
        h.session.reset();
    }
}
