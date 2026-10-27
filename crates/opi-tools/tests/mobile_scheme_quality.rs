// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **移动端 9/14 键真库匹配质量**（2026-10-27 用户报障）。
//!
//! 报障原文：9 键打 `ne` 显示的拼音成了 `63`；9/14/大 9 键的匹配「要么不完整、
//! 要么错误」；要像 26 键一样自动识别整词/整句。本文件用**部署版 luna.opid**
//! 钉住当轮修好的几条用户可观察路径：
//!
//! * 9 键读数不许出现按键数字（补读音条目曾把输入键序当读数）；
//! * 9/14 键的多字组合（长词前缀、整句、同长度碰撞的多种切法）与 26 键同档；
//! * 组合按段平均分排序，词库里的整词不被「更长但更冷」的切法顶掉。
//!
//! T9 键位本身的同码歧义（`92674264426` 既是「早上好」也是「晚上好」）不在本
//! 文件断言 —— 那需要上下文语言模型，见交付说明。

use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};
use engine_data::load_mmap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

fn engine() -> Engine {
    let dict = load_mmap(Path::new(LUNA_OPID)).expect("加载部署版 luna.opid（android assets）");
    Engine::new(
        Box::new(dict),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    )
}

fn top(e: &mut Engine, scheme: Scheme, input: &str, n: usize) -> Vec<String> {
    e.clear();
    e.set_scheme(scheme);
    for c in input.chars() {
        e.input_key(c);
    }
    e.candidates(n).into_iter().map(|c| c.text).collect()
}

fn assert_first(scheme: Scheme, input: &str, want: &str) {
    let mut e = engine();
    let got = top(&mut e, scheme, input, 8);
    assert_eq!(
        got.first().map(String::as_str),
        Some(want),
        "{scheme:?} {input} 的首选应为「{want}」：{got:?}"
    );
}

/// 9 键：用户报障的整词/整句（真库实测都曾失败或排在错误切法之后）。
#[test]
fn nine_key_matches_words_and_sentences() {
    // 我爱北京：旧行为只出「我查地精」（同为 11 键的另一种切法）。
    assert_first(Scheme::Nine, "96242345464", "我爱北京");
    // 这是谁：旧行为只出「这时期体」。
    assert_first(Scheme::Nine, "9437447484", "这是水");
    // 生日快乐的前缀（输入未打完）：旧行为回退成「生」。
    assert_first(Scheme::Nine, "74364745824", "生日快乐");
    // 整句：3 段组合（我们 + 一起 + 去吃饭 的切法）。
    assert_first(Scheme::Nine, "96636947478244326", "我们一起去吃饭");
    // 整句：他是 + 我的 + 好朋友（19 键，旧行为只剩「他」）。
    assert_first(Scheme::Nine, "8274496334267364968", "他是我的好朋友");
    // 长整句：中华 + 人民 + 共和国（23 键）。
    assert_first(Scheme::Nine, "94664482736646466443486", "中华人民共和国");
}

/// 14 键：同一批用户在 14 键上的报障（旧实现没有整句组合，长串只剩首字）。
#[test]
fn fourteen_key_matches_words_and_sentences() {
    // woaibeijing（qoaubeujubg）
    assert_first(Scheme::Fourteen, "qoaubeujubg", "我爱北京");
    // tashiwodehaopengyou（taaguqodegaooebgtou）
    assert_first(Scheme::Fourteen, "taaguqodegaooebgtou", "他是我的好朋友");
    // jintiantianqizhenhao（jubtuabtuabquzgebgao）
    assert_first(Scheme::Fourteen, "jubtuabtuabquzgebgao", "今天天气真好");

    // 多字组合本身（整词不在词库，两段拼出）：yanchunzheng → 厌蠢症。
    let mut e = engine();
    let got = top(&mut e, Scheme::Fourteen, "tabcgubzgebg", 64);
    assert!(
        got.contains(&"厌蠢症".to_string()),
        "14 键应组合出「厌蠢症」：{got:?}"
    );

    // 补读音缺口（词库只挂了 么/ma、me、yao，缺 mo）：14 键打 `mo` 也要补出「么」
    // —— 原先补读音索引只按 T9 数字建，14 键拿不到。
    let mut e = engine();
    let got = top(&mut e, Scheme::Fourteen, "mo", 512);
    assert!(
        got.contains(&"么".to_string()),
        "14 键 mo 应补出「么」：{got:?}"
    );
}

/// 9 键读数**只能是拼音**（2026-10-27 报障「打 ne 显示 63」）：遍历全部两键组合，
/// 显示串与左列逐条读数都不许出现 ASCII 数字。补读音层的条目曾把输入键序当读数。
#[test]
fn nine_key_readings_never_expose_digit_keys() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    let keys = ['2', '3', '4', '5', '6', '7', '8', '9'];
    for &a in &keys {
        for &b in &keys {
            e.clear();
            e.input_key(a);
            e.input_key(b);
            let _ = e.candidates(512);
            let shown = e.display_buffer();
            let readings = e.candidate_readings();
            assert!(
                !shown.chars().any(|c| c.is_ascii_digit()),
                "9 键 {a}{b} 的显示串出现数字：{shown:?}"
            );
            assert!(
                readings
                    .iter()
                    .all(|r| !r.chars().any(|c| c.is_ascii_digit())),
                "9 键 {a}{b} 的左列读数出现数字：{readings:?}"
            );
        }
    }
}
