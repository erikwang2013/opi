// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI **边界**测试：28 个 `opi_*` 导出在非法/极端入参下的行为。
//!
//! 与 `cabi_test.rs` 的分工：那边测**正常链路**（装载→输入→候选→选择），
//! 这边只测**边界**——null 指针、`len` 与实际不符、非法 UTF-16（孤立代理对、
//! `0xFFFE`/`0xFFFF`）、极端整数（`usize::MAX`、`i32::MIN`、`u32::MAX`）、
//! 重复装载、跨导出的句柄生命周期。
//!
//! 断言的判据是**契约**而不是「当前实现碰巧如此」：
//! - panic 不跨 `extern "C"`（除了 `catch_unwind` 的兜底，边界输入不该触发 panic）；
//! - 错误必须**显式**表达（空句柄 / `false` / `-1` / 哨兵 0），不许静默改状态；
//! - 未知/越界输入**不得**被静默解释成有效输入（`opi_select(usize::MAX)` 不许
//!   提交任何候选 —— 这类「负值当无符号」的入口在 C 侧就是 `-1`）。
//!
//! 引擎是否已装载的两条路分开测：本文件全程 `load_any()`（已装载），
//! 未装载那份在 `cabi_unloaded.rs`（**独立测试二进制 = 独立进程**，
//! 那个进程里谁都不调 `opi_load`，SINGLETON 恒为 None）。

use std::sync::Mutex;

use opi_ffi::cabi::{
    OpiKeyEventResult, OpiString, opi_buffer, opi_candidates, opi_candidates_page, opi_clear,
    opi_clear_user_words, opi_export_user_words, opi_ffi_free_string, opi_import_user_words,
    opi_input_key, opi_key_event, opi_load, opi_load_trad, opi_mode, opi_page,
    opi_remove_user_word, opi_search_symbols, opi_select, opi_select_page, opi_set_learner,
    opi_switch_mode, opi_symbol_blocks, opi_symbols_in_block,
};

use engine_core::keys::KEY_PAGE_DOWN;

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

// ---------- 1. 字符串入参：null / len 不符 / 非法 UTF-16 ----------

/// 六个收 `(const u16*, len)` 的导出，在 null 指针（`len` 为 0 与非 0）下
/// 一律按「无输入」处理：不 panic、不读内存、返回值走各自的错误哨兵。
/// `len > 0` 而 `ptr` 为 null 是 C 侧最常见的失误（长度算了、指针忘了），
/// 实现的第一行 `ptr.is_null()` 必须挡住它 —— 漏了就是段错误而不是错误码。
/// `len = usize::MAX`（C 侧的 `-1`）是同一族的极端值：先切片就是必然的 UB。
#[test]
fn null_ptr_with_nonzero_len_is_never_dereferenced() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    for len in [0usize, 1, 7, usize::MAX] {
        assert!(
            unsafe { opi_load(std::ptr::null(), len) },
            "null 入参必须走内置回退且成功（len={len}）"
        );
        assert!(
            !unsafe { opi_load_trad(std::ptr::null(), len) },
            "null 路径必须失败（len={len}）"
        );
        assert_eq!(read(unsafe { opi_input_key(std::ptr::null(), len) }), "");
        assert_eq!(unsafe { opi_import_user_words(std::ptr::null(), len) }, -1);
        unsafe { opi_remove_user_word(std::ptr::null(), len) };
    }
    unsafe {
        opi_clear();
        opi_clear_user_words();
    }
}

/// `opi_search_symbols` 的「什么算无关键字」分界：`null` 与**非法 UTF-16**
/// 都按空关键字处理（`read_utf16` 对两者都返回 None，出口 `unwrap_or_default()`
/// 成 ""），而**合法**的非字符不是空关键字。
///
/// 断言的是**等价关系**而不是某个具体结果：空关键字命中什么由符号引擎决定
/// （实测见下），断言具体值会随符号表增长而红；「null ≡ 非法 UTF-16 ≡ 空串」
/// 才是契约本身。后半段反过来钉住合法输入**不得**被当成空关键字 ——
/// 少了它，一个把 `from_utf16` 的 Ok/Err 一律 `unwrap_or_default` 的实现
/// 也能全绿。
#[test]
fn search_symbols_null_and_broken_utf16_mean_empty_keyword() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    let empty: [u16; 0] = [];
    let baseline = read(unsafe { opi_search_symbols(empty.as_ptr(), 0) });
    assert!(!baseline.is_empty(), "前置：空关键字实测返回全表");
    for (ptr, len) in [
        (std::ptr::null(), 0usize),
        (std::ptr::null(), 9),
        ([0xD800u16].as_ptr(), 1),
        ([0xDC00u16, 0xD800].as_ptr(), 2),
    ] {
        assert_eq!(
            read(unsafe { opi_search_symbols(ptr, len) }),
            baseline,
            "null / 非法 UTF-16 必须与空关键字同解（len={len}）"
        );
    }
    // 合法非字符：不是空关键字，命中结果必须与全表不同
    let nc = [0xFFFFu16];
    assert_ne!(
        read(unsafe { opi_search_symbols(nc.as_ptr(), nc.len()) }),
        baseline,
        "合法 UTF-16（U+FFFF）不得被当成空关键字"
    );
    // 真关键字必须真的过滤（否则上面的「等价」对一个什么都不做的实现也成立）
    let he = to_units("he");
    let hits = read(unsafe { opi_search_symbols(he.as_ptr(), he.len()) });
    assert!(hits.contains('♥'), "关键字搜索坏了：{hits:.80}");
    assert!(hits.len() < baseline.len(), "关键字必须缩小结果集");
}

/// 非法 UTF-16（孤立高/低代理对、反转配对）：`read_utf16` 走
/// `String::from_utf16` 失败 → None。各出口必须按**无输入**处理。
///
/// 这条同时钉住「不许 panic 跨 ABI」：`String::from_utf16` 的 Err 若被 `unwrap`
/// 就是一次穿过 `extern "C"` 的 unwind（UB），而孤立代理对是**冷门但合法**的
/// C 侧输入（`NSString` 派生缓冲、手写测试桩、网络来的路径都能造出来）。
#[test]
fn lone_surrogates_are_treated_as_no_input() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_set_learner(false) };
    let bad_cases: [&[u16]; 4] = [
        &[0xD800],         // 孤立高代理
        &[0xDC00],         // 孤立低代理
        &[0xD800, 0x0041], // 高代理后跟非低代理
        &[0xDC00, 0xD800], // 反转配对
    ];
    for units in bad_cases {
        assert_eq!(
            read(unsafe { opi_input_key(units.as_ptr(), units.len()) }),
            "",
            "非法 UTF-16 不得产生提交文本：{units:?}"
        );
        assert_eq!(
            unsafe { opi_import_user_words(units.as_ptr(), units.len()) },
            -1,
            "非法 UTF-16 必须按失败处理：{units:?}"
        );
        unsafe { opi_remove_user_word(units.as_ptr(), units.len()) };
    }
    // 全程不得改动引擎状态
    assert_eq!(read(unsafe { opi_buffer() }), "");
    assert_eq!(
        read(unsafe { opi_export_user_words() }),
        r#"{"version":1,"words":[]}"#
    );
    unsafe { opi_set_learner(true) };
}

/// 非字符 U+FFFE / U+FFFF 是**合法**的 UTF-16 单元（`String::from_utf16` 接受，
/// `unwrap_or_default` 那条路走不到），所以它们必须被当成**普通字符串**而不是
/// 「坏缓冲」：`input_key` 按非 ASCII 拒绝、`import` 按非法 JSON 失败。
/// 与上一条分开写是因为两者的**路径不同**：把 U+FFFF 归进「非法 UTF-16」
/// 会写出一个恰好也绿的断言（`input_key` 两者都返回空串），
/// 从而永远测不到 `from_utf16` 的 Ok/Err 分界 —— 本项目记过这种「断言观察点
/// 与变异点等价」的假绿。
#[test]
fn noncharacters_are_valid_utf16_units_not_a_broken_buffer() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_clear() };
    for units in [&[0xFFFEu16][..], &[0xFFFFu16][..], &[0xFFFFu16, 0xFFFE][..]] {
        let unit = String::from_utf16(units).expect("非字符是合法 UTF-16");
        assert_eq!(
            read(unsafe { opi_input_key(units.as_ptr(), units.len()) }),
            "",
            "非 ASCII 字符一律拒绝：{unit:?}"
        );
        assert_eq!(
            unsafe { opi_import_user_words(units.as_ptr(), units.len()) },
            -1,
            "非 JSON 内容必须失败：{unit:?}"
        );
    }
    assert_eq!(read(unsafe { opi_buffer() }), "", "拒绝路径不得改缓冲");
    assert_eq!(
        read(unsafe { opi_export_user_words() }),
        r#"{"version":1,"words":[]}"#
    );
}

/// 补充平面与 BMP 边界的**合法**字符：多字符 / 非 ASCII 一律拒绝（空串），
/// 且不得被特殊键码空间吃掉。`0x10000`（补充平面首字符）与 `0xFFFF` 相邻，
/// 后者正是 fcitx5 轨 Delete 的键值、前者是 Apple 层 `SPECIAL_BASE` ——
/// 本层的 `opi_input_key` 收的是**字符**，与键码空间无关，两者都必须返回空串。
#[test]
fn input_key_rejects_multi_and_non_ascii_without_touching_state() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_clear() };
    for s in ["ab", "你", "\u{10000}", "\u{FFFF}", "'a", "a'", "\u{0}"] {
        let u = to_units(s);
        assert_eq!(
            read(unsafe { opi_input_key(u.as_ptr(), u.len()) }),
            "",
            "非单 ASCII 字符必须拒绝：{s:?}"
        );
    }
    assert_eq!(read(unsafe { opi_buffer() }), "", "拒绝路径不得改缓冲");
}

// ---------- 2. 极端整数入参 ----------

/// 模式整数越界必须**忽略**（状态不变）。`i32::MIN`/`i32::MAX`/`-1` 是 C 侧
/// 把枚举当位域传时的常见取值；`mode_from_int` 的 `_ => None` 是唯一防线，
/// 漏掉就会静默切模式（表现是「打字出不了字」，与本项目历次静默失效同族）。
#[test]
fn switch_mode_extremes_are_ignored() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    assert_eq!(unsafe { opi_mode() }, 0);
    for m in [-1, 5, 100, i32::MIN, i32::MAX] {
        unsafe { opi_switch_mode(m) };
        assert_eq!(unsafe { opi_mode() }, 0, "越界模式 {m} 必须被忽略");
    }
    // 合法值仍然工作（防「一律忽略」的假实现把这条也吞了）
    unsafe { opi_switch_mode(1) };
    assert_eq!(unsafe { opi_mode() }, 1);
    unsafe { opi_switch_mode(0) };
}

/// `opi_select` / `opi_select_page` 的极端下标：C 侧的 `-1` 到这里是
/// `usize::MAX` / `u32::MAX`。契约是「越界 → 空串、不改状态」。
///
/// 两个出口走**不同**的下标空间，所以两个都要测、且都在第 1 页上测：
/// `opi_select` 是全局下标（直接 `Engine::select`，`cands.get(index)`），
/// `opi_select_page` 是页内下标（`KeyRouter::select` 里算 `page * 8 + index`）。
/// 后者在 64 位上 `u32::MAX` 仍不溢出（`u32` 只有 32 位），**32 位 target 上会**：
/// `page≥1` 时 `8 + 4294967295` 回绕成 7，于是「越界」变成「提交第 8 个候选」。
/// 出货的 armeabi-v7a 就是 32 位，本机无法复现 —— 这条断言在 64 位上只能守住
/// 「64 位语义正确」，32 位那半没有本机证据（见报告）。
#[test]
fn select_extreme_index_returns_empty_and_never_commits() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_set_learner(false);
        opi_clear();
    }
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    let global = read_texts(unsafe { opi_candidates(64) });
    let page = read_texts(unsafe { opi_candidates_page() });
    assert!(
        global.len() > page.len(),
        "前置：需要多于一页候选（luna 词库），全局 {} / 首页 {}",
        global.len(),
        page.len()
    );
    key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(unsafe { opi_page() }, 1, "前置：已翻到第 1 页");

    // usize::MAX（= C 里把 -1 传给 `size_t` 参数）。opi_select 是**全局**下标，
    // 直接 `cands.get(usize::MAX)`，无意外的算术。
    assert_eq!(
        read(unsafe { opi_select(usize::MAX) }),
        "",
        "usize::MAX 是越界下标 → 必须空串。返回非空说明某处做了回绕算术"
    );
    assert_eq!(
        read(unsafe { opi_buffer() }),
        "wo",
        "越界选择不得改动缓冲（回绕命中候选时缓冲会被提交清空）"
    );
    assert_eq!(
        read(unsafe { opi_select_page(u32::MAX) }),
        "",
        "u32::MAX 同样越界 → 空串"
    );
    assert_eq!(read(unsafe { opi_buffer() }), "wo", "越界不得改状态");
    // 合法下标仍工作（防「一律返回空串」的假实现）
    assert_eq!(
        read(unsafe { opi_select_page(0) }),
        global[page.len()],
        "第 1 页 k=0 仍应选中全局第 n 个"
    );
    unsafe {
        opi_set_learner(true);
        opi_clear();
    }
}

/// `opi_candidates(limit)` 的极端 limit：0 → 空数组（不是「默认 8」）；
/// `usize::MAX`（C 侧的 `-1`）**不得 panic、不得按 limit 预分配** ——
/// 实现里一句 `Vec::with_capacity(limit)` 就是一次必然失败的 16EB 分配（abort，
/// `catch_unwind` 拦不住）。这条不假设上限（该出口确实按调用方给的 limit 截断，
/// 实测 `usize::MAX` 拿到全量 139 条），只钉住「不崩、且是前 N 条的同一序列」。
#[test]
fn candidates_limit_extremes_never_abort_and_stay_prefix_consistent() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    assert!(
        read_texts(unsafe { opi_candidates(0) }).is_empty(),
        "limit=0 必须是空数组"
    );
    let all = read_texts(unsafe { opi_candidates(usize::MAX) });
    assert!(!all.is_empty(), "limit=usize::MAX 仍须返回候选");
    assert_eq!(
        read_texts(unsafe { opi_candidates(8) }),
        all[..8.min(all.len())],
        "limit 只截断尾部：前 N 条必须与全量同序（截断点错位 = 排序不稳）"
    );
    unsafe { opi_clear() };
}

/// `opi_symbols_in_block(i16)`：负值与超范围都必须是空数组（与 JNI 侧同语义），
/// 合法块仍是满的。`i16::MIN` 是「负 id 被当成 u16 回绕」的哨兵 ——
/// 一旦实现写成 `id as u16`，`i16::MIN` 会变成 32768 而落到某个真实块上。
#[test]
fn symbols_in_block_extremes_are_empty_not_wrapped() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    let blocks: Vec<serde_json::Value> =
        serde_json::from_str(&read(unsafe { opi_symbol_blocks() })).expect("合法 JSON");
    let ids: Vec<i16> = blocks
        .iter()
        .map(|b| b["id"].as_u64().expect("id 为数字") as i16)
        .collect();
    let id0 = ids[0];
    assert!(
        !read_texts(unsafe { opi_symbols_in_block(id0) }).is_empty(),
        "前置：块 {id0} 非空"
    );
    // 不存在的 id 要从**实际块表**里推（块 id 不是 0..n：实测是 1,2,3,4,6），
    // 硬编码「id0-1 越界」会撞上真实存在的邻块 —— 这条断言就变成假的。
    let missing = (0..=i16::MAX)
        .find(|id| !ids.contains(id))
        .expect("块表不可能占满 i16");
    for id in [-1, i16::MIN, i16::MAX, missing] {
        assert!(
            read_texts(unsafe { opi_symbols_in_block(id) }).is_empty(),
            "越界块 id {id} 必须返回空数组（不许回绕成别的块）"
        );
    }
}
