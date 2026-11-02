// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `.ime` 导出（从 win.rs 拆出守 500 行）：每个入口 catch_unwind，panic 不逃过 FFI。

use super::*;

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
                reload_settings(h);
            }
            let _ = ImmSetOpenStatus(himc, true);
            let _ = ImmSetConversionStatus(himc, IME_CMODE_NATIVE, IME_SENTENCE_MODE(0));
        } else {
            finish_composition(himc, false);
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
        let released = states & KEY_STATE_RELEASED != 0;
        let shift = is_shift_vk(vk);
        // TSF 的引擎前分流：Win 组合放行；Ctrl/Alt 只放行热键；Shift+Space 先于空格
        // 进引擎；非 Shift 的未认领 keydown 也要撤销 ShiftTap（Shift+方向键）。
        let win_down = GetKeyState(VK_LWIN.0 as i32) < 0
            || GetKeyState(VK_RWIN.0 as i32) < 0
            || vk == VK_LWIN.0 as u32
            || vk == VK_RWIN.0 as u32;
        if win_down {
            return 0;
        }
        let claimed = {
            let mut g = lock_host();
            let Some(h) = g.as_mut() else { return 0 };
            h.pending_states = states;
            h.pending_hotkey = PendingHotkey::None;
            if !released && !shift {
                h.session.disarm_shift_tap();
            }
            let hot = layout_char(vk, ks).and_then(|ch| mode_hotkey(ch, states));
            let full = fullwidth_hotkey(vk, states);
            if full || hot.is_some() {
                h.pending_hotkey = if !hotkey_should_act(states) {
                    PendingHotkey::Ignore
                } else if full {
                    PendingHotkey::Fullwidth
                } else {
                    PendingHotkey::Mode(hot.expect("hot checked"))
                };
                true
            } else if states & (KEY_STATE_CTRL | KEY_STATE_ALT) != 0 {
                false
            } else {
                h.session.claim(code, states)
            }
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
        // ImeToAsciiEx 没有 lParam：释放/重复位与热键取 ImeProcessKey 刚存下的那一份。
        let actions = {
            let mut g = lock_host();
            let Some(h) = g.as_mut() else { return 0 };
            let states = h.pending_states;
            let hot = h.pending_hotkey;
            h.pending_states = 0;
            h.pending_hotkey = PendingHotkey::None;
            match hot {
                PendingHotkey::Ignore => return 0,
                PendingHotkey::Mode(hot) => h.session.toggle_mode_hotkey(hot),
                PendingHotkey::Fullwidth => {
                    h.session.toggle_fullwidth();
                    Vec::new()
                }
                PendingHotkey::None => h.session.process_at(code, states, Instant::now()),
            }
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
pub extern "system" fn NotifyIME(himc: HIMC, action: u32, index: u32, _value: u32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if himc.0.is_null() {
            return 0;
        }
        if action == NI_COMPOSITIONSTR.0 {
            match index {
                i if i == CPS_COMPLETE.0 => finish_composition(himc, true),
                i if i == CPS_CANCEL.0 => finish_composition(himc, false),
                _ => {}
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
