// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **引擎未装载**时调全部 31 个 C 导出的行为。
//!
//! 为什么单独一个文件：`SINGLETON` 是进程级的，同一个测试二进制里只要有**一个**
//! 用例调过 `opi_load`，其余用例就再也观察不到「未装载」这个状态了
//! （`cabi_test.rs` 的每个用例都以 `load_any()` 开头）。集成测试文件各自编译成
//! **独立二进制、独立进程**，所以本文件是唯一能稳定复现「冷进程」的地方 ——
//! 因此**本文件里任何用例都不得调用 `opi_load` / `opi_load_trad`**。
//!
//! 这正是真实宿主的第一帧：Apple 侧或 Android 侧在 `load` 返回 false（词库缺失、
//! 校验和不符）后**照常接受按键**，此时每个出口都必须给一个明确的哨兵，
//! 而且**绝不能 panic**（panic 跨 `extern "C"` 是 UB；本 crate 另有
//! `catch_unwind` 兜底，但兜底本身也是「有 bug」的信号，不该被当作正常路径）。

use opi_ffi::cabi::{
    OpiString, opi_backspace, opi_buffer, opi_candidates, opi_candidates_page, opi_clear,
    opi_clear_user_words, opi_export_user_words, opi_ffi_free_string, opi_fullwidth_state,
    opi_import_user_words, opi_input_key, opi_input_space, opi_key_event, opi_learner_enabled,
    opi_load_trad, opi_mode, opi_page, opi_page_count, opi_remove_user_word, opi_search_symbols,
    opi_select, opi_select_page, opi_set_learner, opi_set_shift, opi_shift_state,
    opi_symbol_blocks, opi_symbols_in_block, opi_toggle_fullwidth, opi_toggle_symbol,
};

use engine_core::keys::{KEY_PAGE_DOWN, KEY_RETURN, KEY_SHIFT, KEY_SPACE, KEY_STATE_LONG_PRESSED};

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

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

fn key_event(keyval: u32, states: u32) -> (i32, String) {
    let r = unsafe { opi_key_event(keyval, states) };
    assert!(
        (0..=2).contains(&r.action),
        "未装载时 action 越界：{}",
        r.action
    );
    assert_eq!(
        r.action == 2,
        !r.text.ptr.is_null(),
        "action=2 ⟺ 非空句柄（未装载也不许例外）"
    );
    (r.action, read(r.text))
}

/// 冷进程（本文件第一个跑的用例即代表它）里，**每个**导出都必须给哨兵而不是
/// panic 或垃圾：字符串 → 空串、计数 → 0、数组 → `[]`、导入 → 负数、
/// 路由 → `action=0`（键交系统，绝不静默吞键）。
///
/// 逐条列出全部 31 个导出（含 `opi_load` 之外的 30 个）是本用例的主要价值：
/// 这份清单就是 ABI 面，将来新增导出忘了处理未装载态时，这里少一条就会红。
#[test]
fn every_export_returns_a_sentinel_when_engine_is_not_loaded() {
    unsafe {
        // —— 字符串查询类 ——
        assert_eq!(read(opi_buffer()), "");
        assert_eq!(read(opi_export_user_words()), "");
        assert_eq!(read(opi_symbol_blocks()), "");
        assert_eq!(read(opi_candidates(8)), "[]");
        assert_eq!(read(opi_candidates_page()), "[]");
        assert_eq!(read(opi_input_space()), "");
        let he = to_units("he");
        assert_eq!(read(opi_search_symbols(he.as_ptr(), he.len())), "[]");
        // 数组类出口的哨兵是 JSON 空数组（不是空句柄）—— `texts_to_json(vec![])`
        assert_eq!(read(opi_symbols_in_block(0)), "[]");
        let w = to_units("w");
        assert_eq!(read(opi_input_key(w.as_ptr(), w.len())), "");
        assert_eq!(read(opi_select(0)), "");
        assert_eq!(read(opi_select_page(0)), "");
        assert_eq!(read(opi_toggle_symbol()), "");

        // —— 计数值类（0 = 「没有」，与「未装载」共用哨兵）——
        assert_eq!(opi_mode(), 0);
        assert_eq!(opi_page(), 0);
        assert_eq!(opi_page_count(), 0);
        assert_eq!(opi_shift_state(), 0);
        assert!(!opi_learner_enabled());
        // 未装载 → 半角。这不是「报错码」：那时按键全走 action=0 交系统，
        // 宿主拿到的就是半角字符，读侧与可观测行为一致（见该导出注释）。
        assert!(!opi_fullwidth_state());

        // —— 键路由：未装载 → action=0（交系统），绝不静默吞键 ——
        for (k, s) in [
            (b'n' as u32, 0),
            (KEY_SPACE, 0),
            (KEY_RETURN, 0),
            (KEY_PAGE_DOWN, 0),
            (KEY_SHIFT, KEY_STATE_LONG_PRESSED),
        ] {
            assert_eq!(key_event(k, s).0, 0, "未装载必须放行：{k:#x}");
        }

        // —— 无返回值/写入类：必须可调用、不 panic ——
        opi_backspace();
        opi_clear();
        opi_set_shift(true);
        opi_set_learner(true);
        assert!(!opi_toggle_fullwidth(), "未装载：无引擎可切，返回半角");
        opi_clear_user_words();
        assert_eq!(opi_import_user_words(std::ptr::null(), 0), -1);
        let word = to_units("我");
        opi_remove_user_word(word.as_ptr(), word.len());
    }

    // 这一串调用之后仍未装载：状态没被「顺手」创建出来
    assert_eq!(read(unsafe { opi_buffer() }), "");
    assert_eq!(unsafe { opi_page_count() }, 0);
}

/// 未装载时 `opi_load_trad` 必须失败（它要求引擎已 load：繁体词典挂到已装引擎上）。
/// `opi_load_trad` 的实参路径即使**合法**也必须 false —— 否则就是把繁体词典
/// 挂到了一个不存在的引擎上（后续所有查询仍旧是哨兵，用户只会看到「明明返回
/// 成功却打不出字」）。
#[test]
fn load_trad_fails_while_engine_is_not_loaded() {
    let real = to_units(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/generated/trad.opid"
    ));
    assert!(
        !unsafe { opi_load_trad(real.as_ptr(), real.len()) },
        "引擎未装载时 loadTrad 必须 false"
    );
    assert_eq!(unsafe { opi_mode() }, 0, "失败不得改模式");
    assert_eq!(read(unsafe { opi_buffer() }), "");
}

/// 未装载态下反复调用不会「自愈」成可用状态，也不会毒化单例：
/// 之后的调用仍返回同一批哨兵（不是「第一次哨兵、第二次 panic」）。
/// 这条守的是 `with_engine` 的 `Option` 分支：一旦哪次改成 `expect`，
/// 第二次调用就会 panic 穿过 `extern "C"`。
#[test]
fn repeated_calls_while_unloaded_stay_sentinels() {
    for _ in 0..5 {
        assert_eq!(read(unsafe { opi_buffer() }), "");
        assert_eq!(unsafe { opi_page_count() }, 0);
        assert_eq!(read(unsafe { opi_candidates_page() }), "[]");
        assert_eq!(key_event(b'a' as u32, 0).0, 0);
    }
}
