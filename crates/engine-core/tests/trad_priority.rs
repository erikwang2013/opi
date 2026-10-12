// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **简体优先**门禁（用户裁决 2026-10-13：「所有端切换简体就用简体数据，切换繁体就用
//! 繁体数据，默认简体，如果是混合就优先显示简体」）。
//!
//! 背景：模式→词典的路由本来就有（Pinyin → luna、Traditional → trad），但**数据里会混**
//! —— `gen_luna_dict.py` 的「简体库不得收繁体字形」守卫只覆盖 pinyin_simp 那一路，
//! 2026-10-11 词库整合后实测简体库候选里又出现 國/學/車/電。这一层保证四端次序一致。
//!
//! 三层各钉一次：
//! 1. **表**：繁体专形在表里，且**易误报的那批不在**（著/覆/乾/藉/瑙/阪 —— 那条更松的
//!    「变体与自身不同」判据把她们全判成繁体，`hanzi_freq.trad_only` 注明实测 35 条）；
//! 2. **分区**：混合列表简体在前，且同组内**稳定**（原名次不动）；
//! 3. **引擎**：简体模式混排简体在前；**繁体模式不动**（那边用的是繁体数据，
//!    繁体在前是那儿的既定行为，本裁决只管简体）。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::trad_chars::{contains_trad_only, is_trad_only};
use engine_core::{Engine, Mode, Scheme};

#[test]
fn table_marks_traditional_forms_and_spares_the_lookalikes() {
    // 真·繁体专形（都有对应简体、且自己不在 GB2312）
    for (t, s) in [("國", "国"), ("學", "学"), ("臺", "台"), ("發", "发")] {
        assert!(is_trad_only(t.chars().next().unwrap()), "{t}（简 {s}）应在表里");
        assert!(
            !is_trad_only(s.chars().next().unwrap()),
            "{s} 是简体形，不该被判成繁体"
        );
    }
    // ⚠️ 易误报对照组：她们有生僻异构简形，但本身在简体里照常用（著名/覆盖/乾隆/慰藉…）。
    // 这条是判据的分辨力所在 —— 换成「变体与自身不同」那条，这里当场红。
    for c in ["著", "覆", "乾", "藉", "瑙", "阪", "你", "的"] {
        assert!(
            !is_trad_only(c.chars().next().unwrap()),
            "{c} 不是繁体专形（误报对照组）—— 判据写松了"
        );
    }
    // 词组只要有一个繁体专形就算
    assert!(contains_trad_only("臺灣"));
    assert!(contains_trad_only("計算機"));
    assert!(!contains_trad_only("台湾"));
    assert!(!contains_trad_only("计算机"));
}

fn engine_with(pairs: &[(&str, &str, u32)]) -> Engine {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in pairs {
        d.insert(py, w, *f);
    }
    Engine::new(Box::new(d), SymbolEngine::builtin(), false)
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine) -> Vec<String> {
    e.candidates(8).into_iter().map(|c| c.text).collect()
}

/// 简体模式：混排时简体在前 —— 夹具里**繁体形词频更高**（900 > 100），没有这条分区它必在首位。
#[test]
fn simplified_mode_puts_simplified_first_even_when_traditional_is_frequent() {
    let mut e = engine_with(&[("guo", "国", 100), ("guo", "國", 900), ("guo", "过", 80)]);
    assert_eq!(e.mode(), Mode::Pinyin, "前置：默认即简体模式");
    type_str(&mut e, "guo");
    let got = texts(&e);
    assert_eq!(got[0], "国", "简体形必须排最前（实得 {got:?}）");
    assert!(
        got.iter().position(|t| t == "國") > Some(0),
        "繁体形仍在（只是排后）——「优先显示」不是删除（实得 {got:?}）"
    );
    // 稳定：简体组内原名次（国 100 > 过 80）；繁体组只有一条
    assert_eq!(got[2], "國", "简体组在前、繁体组在后（实得 {got:?}）");
}

/// 繁体模式：**不动**（那边用的是繁体数据，繁体在前是既定行为、另有裁决）。
#[test]
fn traditional_mode_is_left_alone() {
    let mut e = engine_with(&[("guo", "国", 100), ("guo", "國", 900)]);
    e.switch_mode(Mode::Traditional);
    type_str(&mut e, "guo");
    let got = texts(&e);
    assert_eq!(got[0], "國", "繁体模式仍按词频排（实得 {got:?}）");
}

/// 9 键同一条路（按键类展开那条）也走分区 —— 它是成对返回的，**读音要跟着候选一起搬**。
#[test]
fn nine_key_path_also_puts_simplified_first() {
    let mut e = engine_with(&[("guo", "国", 100), ("guo", "國", 900)]);
    e.set_scheme(Scheme::Nine);
    for c in "486".chars() {
        e.input_key(c);
    }
    let got = texts(&e);
    assert_eq!(got[0], "国", "9 键同样简体优先（实得 {got:?}）");
}
