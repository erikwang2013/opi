// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! TSF 键事件 → 引擎键码/键状态（Windows 目标专属，从 `tsf.rs` 拆出守 500 行）。
//!
//! 这里只做两件事：把 `wParam`（VK）按当前键盘布局换算成引擎约定的键码，
//! 把 TSF `lParam` + `GetKeyState` 换算成 `logic` 的 `KEY_STATE_*` 位。
//! 判定本体（VK 表、特殊键编码）在平台中立的 `crate::vk`，本文件是 Win32 取数。

use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_MENU,
    VK_SHIFT,
};

use crate::logic::{
    KEY_STATE_ALT, KEY_STATE_CAPS_LOCK, KEY_STATE_CTRL, KEY_STATE_RELEASED, KEY_STATE_REPEAT,
    KEY_STATE_SHIFT,
};
use crate::vk::vk_to_engine_keycode;

/// `ToUnicodeEx` 的 wFlags bit 2：本次调用不改变键盘状态。
/// 不加这个标志时，连续调用会在按下死键（重音等）时吞掉/推进死键状态，
/// 而那些键我们多半要放行给应用 —— 应用侧的组合键输入就会被吃掉。
const TO_UNICODE_NO_STATE_CHANGE: u32 = 0x4;

/// TSF 的 `wParam` 是**虚拟键码**，而 `TsfLogic::input_key` 的约定是
/// 「可打印字符 = Unicode 码点，特殊键 = VK 码」（见 logic.rs 头注释）。
/// 此前直接把 `wparam` 当码点透传，导致：
///   - 英文模式敲 'a' 得到 VK_A=0x41，被当成大写 'A' 提交 → "hello" 变 "HELLO"；
///   - 撇号 `'`（拼音分隔符）的 VK_OEM_7=0xDE 非 ASCII，永远 Unhandled，打不出来。
///
/// 拼音模式只是被 `Composer` 的「字母转小写」兜住才看起来正常，故一直未被发现。
///
/// 用 `ToUnicodeEx` 而非 `MapVirtualKeyW`：前者按**当前键盘布局**与修饰键状态
/// 换算，非 US 布局（AZERTY/Dvorak）下依然正确；后者只给未加修饰的字符，
/// 自行补 Shift/CapsLock 逻辑等于重造键盘布局。
pub(crate) fn to_engine_keycode(vk: u32) -> u32 {
    let mut key_state = [0u8; 256];
    let mut buf = [0u16; 8];
    // SAFETY: key_state 为 256 字节（Win32 要求的键盘状态数组大小）；
    // buf 长度经 cchbuff 如实传入，ToUnicodeEx 不会写出界。
    let n = unsafe {
        if GetKeyboardState(&mut key_state).is_err() {
            -1 // 取不到键盘状态：当"无映射"处理（判定与回退策略全在 vk_to_engine_keycode）
        } else {
            ToUnicodeEx(
                vk,
                0,
                &key_state,
                &mut buf,
                TO_UNICODE_NO_STATE_CHANGE,
                Some(GetKeyboardLayout(0)),
            )
        }
    };
    // 映射判定全在 crate::vk::vk_to_engine_keycode（主机可单测）：特殊键换成
    // 编码键码（SPECIAL_BASE|VK，查一次表即检测+转发）、有映射取码点、
    // 无映射/死键（n <= 0）返回放行哨兵而非 VK。
    vk_to_engine_keycode(vk, n, buf[0])
}

/// lParam 键状态位 → logic 的 KEY_STATE_*（位约定见 logic.rs 头注释）。
/// TSF lParam：bit30 = 按下前状态（1 = 重复），bit31 = 转换状态（1 = 释放）；
/// 修饰键（⇧/Ctrl/Alt/CapsLock）TSF 键事件不携带，用 GetKeyState 移位查询。
pub(crate) fn map_key_state(lparam: LPARAM) -> u32 {
    let lp = lparam.0 as u32;
    let mut s = (lp >> 3) & KEY_STATE_REPEAT; // bit30 → 1<<27
    s |= (lp >> 5) & KEY_STATE_RELEASED; // bit31 → 1<<26
    // GetKeyState 返回 i16：高位为 1 = 按下（负数）。
    if unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0 {
        s |= KEY_STATE_SHIFT;
    }
    if unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0 {
        s |= KEY_STATE_CTRL;
    }
    if unsafe { GetKeyState(VK_MENU.0 as i32) } < 0 {
        s |= KEY_STATE_ALT;
    }
    if unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0 {
        s |= KEY_STATE_CAPS_LOCK;
    }
    s
}
