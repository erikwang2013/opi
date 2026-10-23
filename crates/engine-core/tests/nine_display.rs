// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 9 键拼音行（`display_buffer`）的显示语义（2026-10-23 从 `scheme_keys.rs` 拆出：
//! 那边随「完整显示」「手动分词符」两条用例越了 500 行门禁）。

use engine_core::symbols::SymbolEngine;
use engine_core::{Dictionary, Engine, InMemoryDictionary, Scheme};

fn dict(entries: &[(&str, &str, u32)]) -> Box<dyn Dictionary> {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    Box::new(d)
}

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine() -> Engine {
    Engine::new(
        dict(&[
            ("ni", "你", 5000),
            ("ni", "尼", 900),
            ("nihao", "你好", 6000),
        ]),
        no_symbols(),
        false,
    )
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

/// 9 键的引擎缓冲是数字串；**显示层**要的是首条候选的拼音（主流输入法 9 键候选栏左侧
/// 那格「输入时显示的拼音」）。FFI 的 buffer 出口读 `display_buffer`。
#[test]
fn nine_key_display_buffer_is_the_top_candidate_pinyin() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // ni
    let _ = e.candidates(8); // 取候选顺带刷新读数（宿主每键都这么走）
    assert_eq!(e.buffer(), "64", "引擎缓冲本体仍是数字串（路由/退格靠它）");
    assert_eq!(e.display_buffer(), "ni", "读数 = 首条候选「你」的拼音");

    type_str(&mut e, "426"); // nihao
    let _ = e.candidates(8);
    assert_eq!(e.buffer(), "64426");
    assert_eq!(e.display_buffer(), "ni'hao", "读数带分词");

    // 乱按（`999`）：宽松兜底档按末键首字母出字；显示补全为完整按键串。
    let mut e2 = engine();
    e2.set_scheme(Scheme::Nine);
    type_str(&mut e2, "999");
    let _ = e2.candidates(8);
    let shown = e2.display_buffer();
    assert_ne!(shown, "999", "数字串不许端上去（旧裁决的本意）");
}

/// 拼音行**完整显示**（2026-10-23 用户裁决）：「图片里显示省略号了，我需要完整显示
/// 出来」—— 读数只覆盖前缀时，未覆盖的键按**键面首字母**补上，不再出现「·」。
/// `64426` 只命中 `ni`（`你`）⇒ `ni` + 4→g、2→a、6→m。
#[test]
fn nine_key_display_fills_uncovered_keys_with_leading_letters() {
    let mut e = Engine::new(dict(&[("ni", "你", 5000)]), no_symbols(), false);
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "nigam", "未覆盖的键补键面首字母");
    assert_eq!(e.buffer(), "64426", "缓冲本体不动（行为判定读它）");
}

/// 首条读数比按键短、但后面有一条**等长**读数（前缀扩展词）⇒ 显示用等长的那条，
/// 不把短读数补成假尾巴。
#[test]
fn nine_key_display_prefers_an_exact_length_reading() {
    let mut e = Engine::new(
        dict(&[("ni", "你", 9000), ("nihao", "你好", 100)]),
        no_symbols(),
        false,
    );
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni'hao", "等长读数优先于补键");
}

/// 手动分词符要**看得见**（2026-10-23 用户报障「9 键分词键无效果」）：点分词键后
/// 缓冲末尾多一个 `'`，拼音行要把它显示出来，而不是因为候选读数没变而毫无反馈。
#[test]
fn nine_key_display_shows_a_manual_segment_mark() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni");
    e.input_key('\'');
    assert_eq!(e.buffer(), "64'");
    let _ = e.candidates(8);
    assert!(
        e.display_buffer().ends_with('\''),
        "手动分词符必须在拼音行可见：{:?}",
        e.display_buffer()
    );
}

/// 换方案把读数一起清掉（否则换方案后的一次读拿到上一条输入的拼音）。
#[test]
fn set_scheme_clears_the_display_reading() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni");
    e.set_scheme(Scheme::Nine); // 同值重设也清（等于「重新开始」）
    assert_eq!(e.display_buffer(), "", "换方案后读数必须为空");
}

/// 非 9 键方案：26 键显示按音节分词（单音节输入与缓冲逐字节相同），14 键仍是原文。
/// `buffer()` 本体不动 —— 别的端（fcitx5/TSF）的**行为判定**读它。
#[test]
fn display_buffer_is_raw_outside_nine() {
    let mut e = engine();
    type_str(&mut e, "ni");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni");
    assert_eq!(e.buffer(), "ni");

    let mut e14 = engine();
    e14.set_scheme(Scheme::Fourteen);
    type_str(&mut e14, "n");
    let _ = e14.candidates(8);
    assert_eq!(e14.display_buffer(), e14.buffer());
}
