// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 英文模式的 C ABI 键路由用例（2026-10-18 从 `key_event.rs` 拆出 —— 单词联想
//! 落地后那里越过 500 行门禁；同 `key_event.rs` 的 `#[path]` 先例，共用同一测试
//! 二进制与 SERIAL）。

use super::*;

/// 调一次 opi_key_event，返回 (action, text)。
fn key_event(keyval: u32, states: u32) -> (i32, String) {
    let r = unsafe { opi_key_event(keyval, states) };
    (r.action, read(r.text))
}

/// 英文模式：字母进缓冲（action=1 + 无提交文本，2026-10-18 单词联想）；
/// ⇧ Lock 下进缓冲为大写；抬起不二次输入。
#[test]
fn cabi_key_event_english_buffers_letters_and_shift() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(1) };
    let (a, t) = key_event('a' as u32, 0);
    assert_eq!((a, t.as_str()), (1, ""), "英文空缓冲：字母入缓冲");
    assert_eq!(read(unsafe { opi_buffer() }), "a");
    let (a, _) = key_event('a' as u32, KEY_STATE_RELEASED);
    assert_eq!(a, 1, "抬起不得二次输入（否则 a → aa）");
    assert_eq!(read(unsafe { opi_buffer() }), "a", "抬起不动缓冲");

    let (a, _) = key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED);
    assert_eq!(a, 1, "⇧ 长按被消费");
    let (a, t) = key_event('a' as u32, 0);
    assert_eq!((a, t.as_str()), (1, ""), "Lock 下大写进缓冲");
    assert_eq!(read(unsafe { opi_buffer() }), "aA", "大小写由引擎 ⇧ 决定");

    unsafe { opi_switch_mode(0) };
    let (a, t) = key_event('a' as u32, 0);
    assert_eq!((a, t.as_str()), (1, ""), "拼音模式下字母进缓冲");
    unsafe { opi_clear() };
}

/// ⇧ Lock 不得跨模式残留：`opi_switch_mode` 必须清**前端** ⇧ 状态。
/// 引擎路径的缓冲大小写由引擎 shift 决定，两态仍各记各的 —— 前端不清，
/// 「⇧ 长按 → 切拼音 → 切回英文」后缓冲里会全是大写。对标两轨的
/// switch_mode_clears_frontend_shift_lock。
#[test]
fn cabi_key_event_switch_mode_clears_frontend_shift_lock() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(1) };
    key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED); // Lock
    unsafe { opi_switch_mode(0) }; // 离开 English → 清前端 ⇧
    unsafe { opi_switch_mode(1) }; // 回到 English
    let (a, _) = key_event('a' as u32, 0);
    assert_eq!(a, 1, "英文字母进缓冲");
    assert_eq!(
        read(unsafe { opi_buffer() }),
        "a",
        "Lock 不得跨模式残留（否则缓冲为 A）"
    );
    unsafe { opi_switch_mode(0) };
}

/// `opi_shift_state` 出口：0=OFF 1=SINGLE 2=LOCK。**前端**状态 —— Swift 的 ⇧ 高亮
/// 只能从这里读；没有出口 UI 只能猜，且永远错。
#[test]
fn cabi_shift_state_reflects_frontend_three_states() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(1) }; // English：缓冲大小写由引擎 ⇧（由前端三态同步）决定
    assert_eq!(unsafe { opi_shift_state() }, 0, "初始 OFF");
    key_event(KEY_SHIFT, 0); // tap
    assert_eq!(unsafe { opi_shift_state() }, 1, "tap → SINGLE");
    key_event('a' as u32, 0);
    assert_eq!(
        read(unsafe { opi_buffer() }),
        "A",
        "前置：SINGLE 下入缓冲转大写"
    );
    assert_eq!(unsafe { opi_shift_state() }, 0, "单次大写后自动复位");
    unsafe { opi_clear() };
    key_event(KEY_SHIFT, 0);
    key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED); // 长按 → LOCK
    assert_eq!(unsafe { opi_shift_state() }, 2, "长按 → LOCK");
    key_event('a' as u32, 0);
    assert_eq!(read(unsafe { opi_buffer() }), "A", "LOCK 下入缓冲大写");
    assert_eq!(unsafe { opi_shift_state() }, 2, "LOCK 不因一次输入复位");
    unsafe { opi_switch_mode(0) }; // 离开 English → 清前端 ⇧
    assert_eq!(
        unsafe { opi_shift_state() },
        0,
        "切走必须清（否则回到英文全大写）"
    );
}
