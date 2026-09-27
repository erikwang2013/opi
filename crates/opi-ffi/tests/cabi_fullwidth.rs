// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **标点两档开关与符号开关的这组 C 出口**（`opi_toggle_fullwidth` /
//! `opi_fullwidth_state` / `opi_chinese_punct` / `opi_set_chinese_punct` /
//! `opi_toggle_chinese_punct` / `opi_toggle_symbol`）的集成测试。
//!
//! 本文件同时是两条**相反**的重读规则的对照门禁：全角随模式默认（切模式必须重读），
//! 中文标点是用户偏好（切模式**不**重置）—— 把后者当成前者的同构照抄，就会造出
//! 一条「切模式顺手把用户的中文标点偏好重置掉」的真 bug。
//!
//! 单独一个文件而不是塞进 `cabi_test.rs`：那个文件 405 行，逼近 500 行上限
//! （同 `cabi/key_event.rs` 拆出去的理由），且本文件自带 `SERIAL`（见下）。
//!
//! `SINGLETON` 是进程级的，同一二进制里的用例会互相串扰 —— 每个用例都串行执行
//! （照抄 `cabi_test.rs` 的做法）。
//!
//! 未装载态的哨兵**不在这里**：那需要冷进程，见 `cabi_unloaded.rs`（本文件的每个
//! 用例都以 `load_any()` 开头，跑过一次之后该进程再也观察不到未装载态）。

use std::sync::Mutex;

use opi_ffi::cabi::{
    OpiString, opi_buffer, opi_candidates, opi_chinese_punct, opi_ffi_free_string,
    opi_fullwidth_state, opi_input_key, opi_key_event, opi_load, opi_mode, opi_page,
    opi_page_count, opi_set_chinese_punct, opi_switch_mode, opi_toggle_chinese_punct,
    opi_toggle_fullwidth, opi_toggle_symbol,
};

use engine_core::keys::KEY_PAGE_DOWN;

static SERIAL: Mutex<()> = Mutex::new(());

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

/// 模式编码（`api::convert::mode_to_int`）：0 拼音 / 1 英文 / 2 数字 / 3 符号 / 4 繁体。
const MODE_PINYIN: i32 = 0;
const MODE_ENGLISH: i32 = 1;
const MODE_SYMBOL: i32 = 3;

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn read(s: OpiString) -> String {
    if s.ptr.is_null() {
        return String::new();
    }
    let units = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
    let out = String::from_utf16(units).expect("导出必须返回合法 UTF-16");
    unsafe { opi_ffi_free_string(s) };
    out
}

fn load_any() {
    let p = to_units(LUNA_OPID);
    let ok = unsafe { opi_load(p.as_ptr(), p.len()) };
    if !ok {
        assert!(unsafe { opi_load(std::ptr::null(), 0) }, "内置回退必须可用");
    }
}

fn key(ch: char) -> String {
    let u = to_units(&ch.to_string());
    read(unsafe { opi_input_key(u.as_ptr(), u.len()) })
}

fn type_str(s: &str) -> String {
    s.chars().map(key).collect()
}

/// 键路由出口：(action, text)。0=放行 1=已处理 2=提交。
fn route(ch: char) -> (i32, String) {
    let r = unsafe { opi_key_event(ch as u32, 0) };
    (r.action, read(r.text))
}

// ---------- 全角开关 ----------

/// 返回值是**切换后**的新状态（状态栏直接拿它刷新，不必再查一次），
/// 且与读侧 `opi_fullwidth_state()` 逐次一致 —— 两个出口不许各说各话。
#[test]
fn toggle_fullwidth_returns_the_state_it_just_switched_to() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_PINYIN) };

    // 拼音默认全角（`Mode::default_fullwidth`），首次切换必为 → 半角
    let mut expected = !unsafe { opi_fullwidth_state() };
    for i in 0..3 {
        let got = unsafe { opi_toggle_fullwidth() };
        assert_eq!(got, expected, "第 {i} 次切换返回值 ≠ 新状态");
        assert_eq!(
            unsafe { opi_fullwidth_state() },
            got,
            "第 {i} 次：切换返回值与读侧不一致"
        );
        expected = !got;
    }
}

/// **硬规则**：任何 `opi_switch_mode` 之后必须重读 `opi_fullwidth_state()` ——
/// 切模式把它重置为该模式的默认值（拼音 `true` / 英文 `false`）。
/// 这条钉的是「平台侧自己记一份全角状态」的做法：那样切完模式会漂。
#[test]
fn fullwidth_state_is_redefined_by_every_switch_mode() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();

    unsafe { opi_switch_mode(MODE_PINYIN) };
    assert!(unsafe { opi_fullwidth_state() }, "拼音默认全角");

    unsafe { opi_switch_mode(MODE_ENGLISH) };
    assert!(
        !unsafe { opi_fullwidth_state() },
        "英文默认半角（切模式重置）"
    );

    unsafe { opi_toggle_fullwidth() };
    assert!(unsafe { opi_fullwidth_state() }, "英文下也能手动切全角");

    unsafe { opi_switch_mode(MODE_PINYIN) };
    assert!(
        unsafe { opi_fullwidth_state() },
        "切回拼音 = 回到拼音的默认值"
    );
    unsafe { opi_switch_mode(MODE_ENGLISH) };
    assert!(
        !unsafe { opi_fullwidth_state() },
        "英文下开过的全角不跨模式残留"
    );
}

/// 英文模式的标点**默认半角**（引擎层映射规则的直接取证）：
/// 手动开全角后才转全角，且是**机械全角**（`.` → `．` U+FF0E），
/// 不是中文标点（`。` U+3002）—— 全角 ≠ 中文标点。
#[test]
fn english_punctuation_is_halfwidth_until_the_toggle_is_on() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_ENGLISH) };

    assert_eq!(key(','), ",", "英文默认半角：原样");
    assert_eq!(key('.'), ".");
    assert_eq!(key('?'), "?");

    unsafe { opi_toggle_fullwidth() };
    assert_eq!(key(','), "，", "全角后是 U+FF0C 机械全角");
    assert_eq!(key('.'), "．", "不是中文句号 U+3002");
    assert_eq!(key('?'), "？");
    assert_eq!(key('"'), "＂", "西文引号不成对交替（中文模式才交替）");

    // 全角只管标点：字母/数字照原样提交，全角字母表（ａ１）是另一件事，不归本开关。
    // 走**键路由**量 —— raw 出口 `opi_input_key` 对放行类字符返回空串（那是
    // 「引擎不产文本、宿主自己插入」的表达），拿它当「字符丢了」会读错。
    let (action, text) = route('1');
    assert_eq!((action, text.as_str()), (2, "1"), "全角开着也不动数字");
    let (action, text) = route('a');
    assert_eq!((action, text.as_str()), (2, "a"), "全角开着也不动字母");
}

// ---------- 中文标点那一档 ----------

/// **两档开关互不牵连**的真值表（用户裁决 2026-09-28）：拿 `.` 这一个键就能把两档
/// 分开量 —— 它在三档下出三个不同的字符（`。` U+3002 / `．` U+FF0E / `.`）。
/// 这条是「把 `chinese_punct` 与 `fullwidth` 焊成一个开关」那个旧 bug 的定点门禁：
/// 焊回去任意一半，这里至少一条断言会红。
#[test]
fn chinese_punct_and_fullwidth_are_two_independent_gates() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_PINYIN) };
    assert!(unsafe { opi_chinese_punct() }, "前置：拼音下中文标点默认开");
    assert!(unsafe { opi_fullwidth_state() }, "前置：拼音默认全角");

    assert_eq!(key('.'), "。", "两档都开：表命中 → 中文句号 U+3002");

    unsafe { opi_set_chinese_punct(false) };
    assert_eq!(
        key('.'),
        "．",
        "只关中文标点档：落到机械全角 U+FF0E —— **不是**半角，也不是 U+3002"
    );

    unsafe { opi_toggle_fullwidth() }; // → 半角
    assert!(!unsafe { opi_fullwidth_state() }, "前置：全角已关");
    assert_eq!(key('.'), ".", "两档都关：直通半角");

    // 复原。本档是**用户偏好**（进程级单例），泄漏给同二进制其它用例 = 假前提。
    unsafe { opi_set_chinese_punct(true) };
    unsafe { opi_toggle_fullwidth() }; // → 回全角
    assert!(unsafe { opi_fullwidth_state() }, "复原：全角回到默认");
}

/// 读侧如实反映写侧（`opi_chinese_punct` ↔ `opi_set_chinese_punct`），
/// **且不随模式重置** —— 与全角**相反**的那条规则。
///
/// 对照是刻意的：同一个用例里先证明「切模式确实重置了全角」（硬规则 1/4 为真），
/// 再证明本档没被重置。只断言后者的话，一个「切模式所有开关都不动」的退化实现
/// 也会绿 —— 那样全角那条硬规则本身就是坏的。
#[test]
fn chinese_punct_survives_switch_mode_and_toggle_symbol() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_PINYIN) };

    unsafe { opi_set_chinese_punct(false) };
    assert!(!unsafe { opi_chinese_punct() }, "读侧必须反映写侧");

    // 正对照：全角确实被切模式重置。
    unsafe { opi_switch_mode(MODE_ENGLISH) };
    assert!(
        !unsafe { opi_fullwidth_state() },
        "正对照失败：英文默认半角（切模式重置全角）"
    );
    unsafe { opi_switch_mode(MODE_PINYIN) };
    assert!(
        unsafe { opi_fullwidth_state() },
        "正对照失败：切回拼音 = 回到拼音的全角默认值"
    );

    // 反对照：本档是用户偏好，**不随模式重置**。
    assert!(
        !unsafe { opi_chinese_punct() },
        "中文标点档被 switch_mode 重置了 —— 它是用户偏好，不是模式默认值"
    );

    // 第二条路径：`toggle_symbol` 内部走了一次 `switch_mode`，同样不许重置本档。
    let _ = read(unsafe { opi_toggle_symbol() });
    assert_eq!(unsafe { opi_mode() }, MODE_SYMBOL, "前置：已切进符号模式");
    assert!(
        !unsafe { opi_chinese_punct() },
        "toggle_symbol 内部那次 switch_mode 把中文标点档重置了"
    );

    unsafe { opi_set_chinese_punct(true) };
    unsafe { opi_switch_mode(MODE_PINYIN) };
}

/// **触发键入口**：返回值是**切换后**的新状态，且与读侧逐次一致；并与设置项入口
/// **共用同一个 bit** —— 只证明「toggle 与自己的读侧一致」是不够的，一个翻转私有
/// 标志位的实现也能满足（那时设置页勾选框与触发键会各说各话）。
///
/// 「本档不随 `switch_mode` 重置」由上面那条用 `set` 入口钉住 + 本条钉住两个入口同源，
/// 两者合起来覆盖 toggle 路径 —— 故不重复断言。
#[test]
fn toggle_chinese_punct_returns_the_state_it_just_switched_to() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_PINYIN) };

    // 与设置项同源：`set(false)` 写进去的，`toggle` 必须翻得动
    unsafe { opi_set_chinese_punct(false) };
    let mut expected = !unsafe { opi_chinese_punct() };
    assert!(expected, "前置：set(false) 之后首次切换应得 true");
    for i in 0..3 {
        let got = unsafe { opi_toggle_chinese_punct() };
        assert_eq!(got, expected, "第 {i} 次切换返回值 ≠ 新状态");
        assert_eq!(
            unsafe { opi_chinese_punct() },
            got,
            "第 {i} 次：切换返回值与读侧不一致"
        );
        expected = !got;
    }

    unsafe { opi_set_chinese_punct(true) };
}

// ---------- 符号开关 ----------

/// 返回值 = **待插入文档的文本**。拿不到插入通道的端不许调它（引擎层的约定），
/// 所以这个出口必须把文本交出来，而不是 void。
#[test]
fn toggle_symbol_returns_the_pending_text_and_switches_mode() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_PINYIN) };

    type_str("ni");
    let pending = unsafe { opi_candidates(8) };
    let first = serde_json::from_str::<Vec<String>>(&read(pending))
        .expect("候选是 JSON 数组")
        .into_iter()
        .next()
        .expect("ni 有候选");

    let out = read(unsafe { opi_toggle_symbol() });
    assert_eq!(out, first, "有候选：提交首候选并把它交给调用方");
    assert_eq!(read(unsafe { opi_buffer() }), "", "缓冲已收尾");
    assert_eq!(unsafe { opi_mode() }, MODE_SYMBOL);

    assert_eq!(
        read(unsafe { opi_toggle_symbol() }),
        "",
        "空缓冲：没有待插入文本"
    );
    assert_eq!(unsafe { opi_mode() }, MODE_PINYIN, "再切回拼音");
}

/// 无候选的乱码缓冲（`qqq`）不上屏 —— 拼音原文塞进文档比丢掉更糟，
/// 所以返回空串（调用方不插入），且缓冲被清掉。
///
/// 用 `qqq` 而不是 `zzz`：luna.opid 里 `zzz` **有**候选（`😴`），拿它当乱码样本
/// 会得到一条恒真的假断言。无候选样本是实测定下来的。
#[test]
fn toggle_symbol_drops_a_buffer_with_no_candidate() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_PINYIN) };

    type_str("qqq");
    assert_eq!(
        read(unsafe { opi_toggle_symbol() }),
        "",
        "qqq 无候选 → 不交文本（拼音原文不上屏）"
    );
    assert_eq!(read(unsafe { opi_buffer() }), "");
    assert_eq!(unsafe { opi_mode() }, MODE_SYMBOL);
}

/// 收尾会改 buffer ⇒ **必须对齐页码**（本 crate 的既有不变式：改完 buffer 不
/// `reset_page_if_buffer_changed` 就会「翻页 → 切符号 → 按 1 选错候选」）。
/// 这条是 `api::toggle_symbol` 里那一行的定点变异测试：删掉它这里必红。
#[test]
fn toggle_symbol_resets_page_to_first() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(MODE_PINYIN) };

    type_str("hao");
    assert!(unsafe { opi_page_count() } > 1, "前置：hao 要多页");
    unsafe { opi_key_event(KEY_PAGE_DOWN, 0) };
    assert_eq!(unsafe { opi_page() }, 1, "前置：已翻到第 2 页");

    let _ = read(unsafe { opi_toggle_symbol() });
    assert_eq!(read(unsafe { opi_buffer() }), "");
    assert_eq!(unsafe { opi_page() }, 0, "缓冲被收尾 → 页码归零");
}
