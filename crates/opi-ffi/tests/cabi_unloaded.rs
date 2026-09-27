// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **引擎未装载**时调全部 C 导出的行为。**条数不写死**（写死的数会随下次扩容漂）：
//! 这份清单是不是真的覆盖了每个导出，由本文件末的
//! `every_export_is_actually_exercised_here` 从 `cabi.rs` **现场解析**后逐条核对
//! （少一条即红）—— 不是靠这段散文的承诺。
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

/// 本目标只用到其中几个函数，其余条目天然是死代码（照 keycode_contract.rs 的写法）。
#[allow(dead_code)]
#[path = "c_abi/checker.rs"]
mod checker;

use opi_ffi::cabi::{
    OpiString, opi_backspace, opi_buffer, opi_candidates, opi_candidates_page, opi_chinese_punct,
    opi_clear, opi_clear_user_words, opi_export_user_words, opi_ffi_free_string,
    opi_fullwidth_state, opi_import_user_words, opi_input_key, opi_input_space, opi_key_event,
    opi_learner_enabled, opi_load_trad, opi_mode, opi_page, opi_page_count, opi_remove_user_word,
    opi_search_symbols, opi_select, opi_select_page, opi_set_chinese_punct, opi_set_learner,
    opi_set_shift, opi_shift_state, opi_switch_mode, opi_symbol_blocks, opi_symbols_in_block,
    opi_toggle_chinese_punct, opi_toggle_fullwidth, opi_toggle_symbol,
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
/// 逐条列出全部导出（`opi_load` 那一条按设计除外，见文件头）是本用例的主要价值：
/// 这份清单就是 ABI 面。**「少一条就会红」不是这里的承诺** —— 兑现它的是本文件末的
/// `every_export_is_actually_exercised_here`：新增导出若不在这里被调用，那条会点名它。
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
        // 同理：未装载 → false（引擎侧默认值是 true，但那时没有任何按键会被映射，
        // 宿主拿到的是原样字符 ⇒ 读侧与可观测行为一致）。
        assert!(!opi_chinese_punct());

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
        assert!(
            !opi_toggle_chinese_punct(),
            "未装载：无引擎可切，返回 false（与「已装载且表关」同值，不是错误码）"
        );
        opi_set_chinese_punct(false);
        // 未装载时切模式：没有引擎可切 —— 不 panic，也不留下状态（随后重读仍是哨兵）。
        opi_switch_mode(1); // 1=English：编码见该导出注释，**别**照 `Mode` 枚举声明序推
        assert_eq!(opi_mode(), 0, "未装载时 switch_mode 不该改模式");
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

/// **本文件是不是真的覆盖了每个导出** —— 不是承诺，是现场解析出来的：从 `cabi.rs`
/// 取导出全集，减去按设计排除的那条，其余每条都必须在本文件的**代码**里被调用
/// （认 `name(` 这个调用形状）。
///
/// 为什么需要它：本文件头原先写着「将来新增导出忘了处理未装载态时，这里少一条就会
/// 红」，而**没有任何东西在核对这份清单** —— 2026-09-28 审计实测：导出全集里 32 条被
/// 调用，`opi_switch_mode` 从未被调用过、却也从不红。这条用例就是那个核对点：
/// 新增导出而没在这里调用 ⇒ **这条红**，逼作者二选一（补调用，或进排除表并写明理由）。
#[test]
fn every_export_is_actually_exercised_here() {
    // 按设计排除：本文件不得装载引擎（见文件头），故 `opi_load` 永不在此调用。
    const BY_DESIGN_EXCLUDED: &[&str] = &["opi_load"];

    // 只看**代码**：注释先剥掉，免得文档里提一句 `opi_xxx(` 就被算作已覆盖。
    let body = checker::strip_c_comments(include_str!("cabi_unloaded.rs"));
    let exports = checker::parse_rust_exports(&checker::read(checker::CABI));

    // 非空转护栏：解析器失明 ⇒ 下面的 `missing` 为空 = 假绿。锚点取刚补上的那条、
    // 按设计排除的那条，以及最普通的一条。
    for anchor in ["opi_switch_mode", "opi_load", "opi_candidates"] {
        assert!(
            exports.iter().any(|(n, _)| n == anchor),
            "解析器没解析出锚点 `{anchor}` —— 失明，下面的「全覆盖」不可信"
        );
    }

    let missing: Vec<&str> = exports
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| !BY_DESIGN_EXCLUDED.contains(n) && !body.contains(&format!("{n}(")))
        .collect();
    assert!(
        missing.is_empty(),
        "这些导出在本文件里从未被调用：{missing:?}\n\
         （补一条调用，或加进 BY_DESIGN_EXCLUDED 并写明理由）"
    );
    println!(
        "[未装载] 覆盖 {} 个导出，另有 {} 条按设计排除",
        exports.len() - BY_DESIGN_EXCLUDED.len(),
        BY_DESIGN_EXCLUDED.len()
    );
}
