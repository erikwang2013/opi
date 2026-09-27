// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 的**会话与生命周期**边界：键事件极端值、重复装载、跨装载的句柄、
//! 装载失败后的状态、学习/用户词库、无参导出的幂等。
//!
//! 与 `cabi_boundary.rs` 的分工：那边管**入参形状**（null 指针、长度不符、
//! 非法 UTF-16、下标/上限极端值），这边管**状态与时间**（装载前后、装载失败、
//! 反复调用）。拆开是为了两边都 < 500 行 —— 一个文件塞不下这么多导出的全部边界。
//! 两个文件都是独立测试二进制（独立进程），各自持一把 `SERIAL` 串行化单例访问。
//! 未装载态在 `cabi_unloaded.rs`（那个文件里谁都不调 `opi_load`）。

use opi_ffi::cabi::*;

use std::sync::Mutex;

use engine_core::keys::{
    KEY_BACK_SPACE, KEY_DELETE, KEY_ESCAPE, KEY_PAGE_DOWN, KEY_PAGE_UP, KEY_RETURN, KEY_SHIFT,
    KEY_SPACE, KEY_STATE_ALT, KEY_STATE_CTRL, KEY_STATE_LONG_PRESSED, KEY_STATE_META,
    KEY_STATE_RELEASED, KEY_STATE_REPEAT, KEY_TAB,
};

static SERIAL: Mutex<()> = Mutex::new(());

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// 读句柄内容并释放（ptr 为 null → 空串）。
fn read(s: OpiString) -> String {
    let out = if s.ptr.is_null() {
        String::new()
    } else {
        let units = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        String::from_utf16(units).expect("导出必须返回合法 UTF-16")
    };
    unsafe { opi_ffi_free_string(s) };
    out
}

fn read_texts(s: OpiString) -> Vec<String> {
    serde_json::from_str(&read(s)).expect("多串导出必须是合法 JSON 数组")
}

/// 装载 luna 词库（存在时真加载，缺失则内置回退，两者都必须返回 true）。
fn load_any() {
    let p = to_units(LUNA_OPID);
    let ok = unsafe { opi_load(p.as_ptr(), p.len()) };
    assert!(
        ok || unsafe { opi_load(std::ptr::null(), 0) },
        "两条装载路径都失败：仓库资产缺失或加载器坏了"
    );
}

/// 权威键路由结果 → `(action, text)`。同时校验两个不变量：
/// ① `action` 只能是 0/1/2（未知值 = ABI 契约外的东西）；
/// ② `action == 2` ⟺ 句柄非空 —— 调用方可以无条件 `opi_ffi_free_string`，
///    但 action=2 却给空句柄会让「提交了空串」与「没提交」无从分辨。
fn key_event(keyval: u32, states: u32) -> (i32, String) {
    let r: OpiKeyEventResult = unsafe { opi_key_event(keyval, states) };
    assert!(
        (0..=2).contains(&r.action),
        "action 越出 0/1/2 三档：{keyval:#x}/{states:#x} → {}",
        r.action
    );
    assert_eq!(
        r.action == 2,
        !r.text.ptr.is_null(),
        "action=2 必须携带非空句柄、action≠2 必须是空句柄（{keyval:#x}/{states:#x}）"
    );
    (r.action, read(r.text))
}

// ---------- 键事件极端值：值域、修饰位、释放事件 ----------

#[test]
fn key_event_extreme_values_stay_in_contract() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
    key_event('n' as u32, 0); // 缓冲非空：最容易被误判成提交的状态
    for keyval in [0, 0x1f, 0x7f, 0xD800, 0xDFFF, 0xFFFF, 0x10FFFF, u32::MAX] {
        for states in [
            0,
            KEY_STATE_RELEASED,
            KEY_STATE_REPEAT,
            KEY_STATE_LONG_PRESSED,
            KEY_STATE_CTRL,
            KEY_STATE_ALT,
            KEY_STATE_META,
            u32::MAX,
        ] {
            let (action, text) = key_event(keyval, states);
            if states & (KEY_STATE_CTRL | KEY_STATE_ALT | KEY_STATE_META) != 0 {
                assert_eq!(action, 0, "修饰键组合一律直通（{keyval:#x}/{states:#x}）");
            }
            if action == 2 {
                assert!(!text.is_empty(), "action=2 不许提交空串");
            }
        }
    }
    assert_eq!(read(unsafe { opi_buffer() }), "n", "极端事件不得改缓冲");
    unsafe { opi_clear() };
}

/// 全部 9 个特殊键**抬起**事件：不得产生提交、不得二次改状态。
/// 这是「按下与抬起同判」的 ABI 面复核 —— 老 bug 是抬起时把同一个字符
/// 第二次送进引擎（`ni`→`nnii`）或英文模式重复提交（`a`→`aa`）。
#[test]
fn special_key_release_events_never_commit_or_double_apply() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
    key_event('n' as u32, 0);
    key_event('i' as u32, 0);
    assert_eq!(read(unsafe { opi_buffer() }), "ni", "前置");
    let page_before = unsafe { opi_page() };
    let shift_before = unsafe { opi_shift_state() };
    for key in [
        KEY_BACK_SPACE,
        KEY_TAB,
        KEY_RETURN,
        KEY_ESCAPE,
        KEY_PAGE_UP,
        KEY_PAGE_DOWN,
        KEY_DELETE,
        KEY_SHIFT,
        KEY_SPACE,
    ] {
        let (action, text) = key_event(key, KEY_STATE_RELEASED);
        assert_ne!(action, 2, "抬起事件不许提交：{key:#x} → {text:?}");
    }
    assert_eq!(read(unsafe { opi_buffer() }), "ni", "抬起不得改缓冲");
    assert_eq!(unsafe { opi_page() }, page_before, "抬起不得翻页");
    assert_eq!(
        unsafe { opi_shift_state() },
        shift_before,
        "抬起不得动 ⇧ 三态"
    );
    unsafe { opi_clear() };
}

// ---------- 3. 装载与句柄生命周期 ----------

/// 重复装载：第二次装载必须把**全部**会话状态重置（缓冲、页码、⇧ 三态、
/// 候选），而不是只换词库。漏掉的后果是「换了词库但 UI 还停在第 2 页」——
/// 页码属于路由状态，`opi_load` 走的是整对象替换，这条钉住它不被改成
/// 「只换 engine」。
#[test]
fn reload_resets_all_session_state() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED); // ⇧ Lock
    assert_eq!(unsafe { opi_shift_state() }, 2, "前置：LOCK");
    key_event(KEY_PAGE_DOWN, 0);

    load_any(); // 第二次装载

    assert_eq!(read(unsafe { opi_buffer() }), "", "重载必须清缓冲");
    assert_eq!(unsafe { opi_page() }, 0, "重载必须归零页码");
    assert_eq!(unsafe { opi_shift_state() }, 0, "重载必须清 ⇧ 三态");
    assert_eq!(read(unsafe { opi_candidates_page() }), "[]", "重载后无候选");
}

/// 装载前拿到的句柄在装载后仍然有效（句柄是独立分配，不指向引擎内存），
/// 且必须**恰好释放一次**。这条守的是「换代/重载后旧句柄还能不能读」——
/// 若哪天句柄改成借用引擎内存，这里就是 use-after-free。
#[test]
fn string_handles_outlive_a_reload() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    let before = unsafe { opi_export_user_words() };
    assert!(!before.ptr.is_null(), "导出必须非空");
    load_any(); // 替换引擎
    let text = read(before); // 读 + 释放旧句柄
    assert!(text.contains("\"version\""), "旧句柄内容仍应可读：{text}");
    // 空句柄可无条件释放（调用方不必按 action 分支）
    unsafe { opi_ffi_free_string(OpiString::empty()) };
}

/// 装载坏路径 → `false`，且**不得**把已经装好的词库换掉（调用方看到 false
/// 就会继续用旧的；若实现先清空再装载，用户会突然掉进 35 词内置库）。
#[test]
fn failed_load_keeps_the_previous_dictionary() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    let before = read_texts(unsafe { opi_candidates(8) });
    assert!(
        before.len() >= 2,
        "前置：luna 下 wo 有多候选，实际 {before:?}"
    );
    unsafe { opi_clear() };
    let missing = to_units("/nonexistent/opi.opid");
    assert!(
        !unsafe { opi_load(missing.as_ptr(), missing.len()) },
        "坏路径必须 false"
    );
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    assert_eq!(
        read_texts(unsafe { opi_candidates(8) }),
        before,
        "装载失败不得替换词库（换掉就是静默掉进内置 35 词库）"
    );
    unsafe { opi_clear() };
}

/// 学习开关与用户词：非法入参不得改动学习状态或词表。
#[test]
fn learner_and_user_words_survive_invalid_input() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_clear_user_words() };
    assert!(unsafe { opi_learner_enabled() }, "默认开");
    unsafe { opi_set_learner(false) };
    let bad = [0xD800u16, 0xDC00];
    assert_eq!(
        unsafe { opi_import_user_words(bad.as_ptr(), bad.len()) },
        -1
    );
    unsafe { opi_remove_user_word(bad.as_ptr(), bad.len()) };
    assert!(!unsafe { opi_learner_enabled() }, "非法导入不得改学习开关");
    assert_eq!(
        read(unsafe { opi_export_user_words() }),
        r#"{"version":1,"words":[]}"#,
        "非法导入不得留下词条"
    );
    unsafe { opi_set_learner(true) };
}

/// 无入参的出口（backspace / clear / set_shift / input_space / 各查询）在
/// 空缓冲、重复调用下必须幂等且不 panic。这 9 个导出没有外部内存参数，
/// 所以它们的边界只有「未装载」与「重复调用」两种 —— 未装载那份在
/// `cabi_unloaded.rs`，这里管重复调用。
#[test]
fn no_arg_exports_are_idempotent_and_sentinel_typed() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
    for _ in 0..3 {
        unsafe {
            opi_backspace();
            opi_clear();
            opi_set_shift(false);
        }
        assert_eq!(read(unsafe { opi_buffer() }), "");
        assert_eq!(read(unsafe { opi_input_space() }), "", "空缓冲空格无提交");
    }
    assert_eq!(unsafe { opi_page() }, 0);
    assert_eq!(unsafe { opi_page_count() }, 0);
    assert_eq!(read(unsafe { opi_candidates_page() }), "[]");
    assert_eq!(unsafe { opi_shift_state() }, 0);
    assert_eq!(unsafe { opi_mode() }, 0);
    // shift 只影响大小写，不改模式/缓冲（改了就是状态机串线）
    unsafe { opi_set_shift(true) };
    assert_eq!(unsafe { opi_mode() }, 0);
    assert_eq!(read(unsafe { opi_buffer() }), "");
    unsafe { opi_set_shift(false) };
}

/// 装载路径含**非法 UTF-16**（不是 null、也不是坏路径，而是坏调用方）：
/// 契约要求「坏路径 → false」，且**不许换掉已装好的词库**。
///
/// 已修（v1.0.15+）：`read_utf16` 现在把「null / 零长」与「非法 UTF-16」分开
/// —— 前者返回 `Some("")`（→ 内置库，返回 true，既有契约），
/// 后者返回 `None` → `opi_load` 返回 false 且不碰词库。
/// 修前：`read_utf16` 对两者都返回 None，`opi_load` 把 None 解释成「没给路径」
/// → 装内置 35 词回退库 → **返回 true**。于是调用方拿到「成功」，引擎却从完整
/// 词库**静默掉进 35 词内置库**（候选从一屏变成个位数），与本项目记录过的
/// fcitx5 `loadDictionary` 静默失效同族：失败被编码成成功。
#[test]
fn load_invalid_utf16_must_not_silently_swap_dictionary() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // 先装真词库并记下候选
    let luna = to_units(LUNA_OPID);
    assert!(unsafe { opi_load(luna.as_ptr(), luna.len()) });
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    let before = read_texts(unsafe { opi_candidates(64) });
    assert!(before.len() > 8, "前置：真词库下 wo 候选多于一页");
    unsafe { opi_clear() };

    let broken = [0xD800u16, 0x0041];
    let ok = unsafe { opi_load(broken.as_ptr(), broken.len()) };
    assert!(!ok, "非法 UTF-16 路径必须 false（现状返回 {ok}）");
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    assert_eq!(
        read_texts(unsafe { opi_candidates(64) }),
        before,
        "失败路径不得替换词库"
    );
    unsafe { opi_clear() };
}

/// 空串与 `null` 在 `opi_load` 上**同义**（都走内置回退），在
/// `opi_import_user_words` 上**不同结果但都失败**（空串不是合法 JSON → -1）。
/// 这条把两个出口的「空」语义并排钉住：并发改一侧时另一侧会红。
#[test]
fn empty_string_and_null_agree_per_export() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    assert!(unsafe { opi_load(std::ptr::null(), 0) });
    let empty: [u16; 0] = [];
    assert!(unsafe { opi_load(empty.as_ptr(), 0) }, "空串 → 内置回退");
    assert_eq!(unsafe { opi_import_user_words(std::ptr::null(), 0) }, -1);
    assert_eq!(unsafe { opi_import_user_words(empty.as_ptr(), 0) }, -1);
    unsafe { opi_clear_user_words() };
}
