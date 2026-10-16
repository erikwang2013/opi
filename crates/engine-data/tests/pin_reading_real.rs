// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **钉读音「读音恰等」 · 真库组**（2026-10-16 用户裁决）。
//!
//! 裁决原文：「移动端，输入拼音后，左侧选择拼音的字了，就不要联想词组了，选择的是字」。
//! 语义：钉住左列一行读音后，候选**只保留读音恰等于它的条目** —— 读音更长的词组
//! （钉 `me` 的 `美俄/每个`、钉 `ni'hao` 的 `你好啊/你好吗`）与延伸读音的字（钉 `nian`
//! 的 `娘`）一律不出。此前是「拼音以钉住读音**开头**」（`starts_with`）。
//!
//! 为什么真库组落在 `engine-data`：`engine-core` 夹具词库走全表扫描那条路，
//! **真库走 `MmapDictionary` 的区间枚举**（另一份过滤实现，四处必须同语义）；
//! 且「读得多的读音下词组就在前几条」这个报障形态只有真库数据出得来
//! （`engine-core/tests/scheme_readings.rs` 是同裁决的夹具组）。
//!
//! 真库是**入库资产**（`android/app/src/main/assets/luna.opid`）—— 缺文件直接红。

use engine_core::Engine;
use engine_core::Scheme;
use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

fn real_engine() -> Engine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../android/app/src/main/assets/luna.opid");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "读不到真库 {}（{e}）—— 它是入库资产，不该缺；缺了本门禁就没在测东西",
            path.display()
        )
    });
    let dict = engine_data::load_bytes(bytes).expect("luna.opid 解析");
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), true);
    e.switch_mode(Mode::Pinyin);
    e
}

/// 钉 `me`（打 `63`）：me 音的字全留、`mei/men` 一族的词全出局。
///
/// 报障形态（未钉时的实测）：`63` 一屏候选里 mei 族的词与字混在 me 音的字后面 ——
/// 钉住 `me` 本是「我要这个音的字」，词却继续跟进来。
#[test]
fn pinned_me_only_keeps_me_readings() {
    let mut e = real_engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "63");
    let all = texts(&e, 64);
    assert!(
        all.contains(&"美俄".to_string()) || all.contains(&"每个".to_string()),
        "前置：未钉时 me 族的词可达（报障形态）—— 若词库数据换了，这条前提要重取：{all:?}"
    );

    e.set_pinned_reading(Some("me".into()));
    let got = texts(&e, 64);
    assert_eq!(got[0], "么", "钉 me 的首位仍是 么：{got:?}");
    for bad in ["美俄", "每个", "美丽", "媒体"] {
        assert!(
            !got.contains(&bad.to_string()),
            "读音更长（mei）的词不得进：{bad} 在 {got:?}"
        );
    }
    assert!(
        !got.contains(&"那".to_string()),
        "那 的词库读音是 na、补读音是 ne ⇒ 钉 me 时不出：{got:?}"
    );
    // 补读音表里 幺(MĒ)/末(MÈ) 的**缺读音恰是 me**（`data/raw/polyphone.tsv`）⇒ 读音恰等
    // ⇒ 留。这条把「补读音层与钉过滤的交互」钉死：钉住时只有**缺的正是这个读音**的才进。
    assert!(
        got.contains(&"幺".to_string()) && got.contains(&"末".to_string()),
        "读音恰为 me 的补读音字保留（幺/末）：{got:?}"
    );
    assert_eq!(
        e.candidate_readings(),
        vec!["me".to_string()],
        "钉住后左列读数只剩这一行（补读音条目带进来的是输入数字串「63」的旧读数不许再出现）"
    );
}

/// 钉 `ne`（打 `63`）：`那` 留（补读音 né 就是 ne）、mo/me 外的补读音字出局、ne 族的词出局。
#[test]
fn pinned_ne_keeps_the_ne_polyphone_char() {
    let mut e = real_engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "63");
    let _ = e.candidates(64);

    e.set_pinned_reading(Some("ne".into()));
    let got = texts(&e, 64);
    assert!(
        got.contains(&"那".to_string()),
        "那 的补读音恰是 ne（polyphone.tsv）⇒ 留：{got:?}"
    );
    assert!(
        !got.contains(&"幺".to_string()) && !got.contains(&"末".to_string()),
        "幺/末 的补读音是 me（不是 ne）⇒ 出局：{got:?}"
    );
    for bad in ["呢个", "讷河", "哪吒"] {
        assert!(
            !got.contains(&bad.to_string()),
            "读音更长的词不得进：{bad} 在 {got:?}"
        );
    }
    assert_eq!(e.candidate_readings(), vec!["ne".to_string()]);
}

/// 钉 `nian`（打 `6426`）：只剩 nian 音的字；`年度/年级`（nianji 一族）与 `娘`（niang）出局。
///
/// ⚠️ 抓 **256** 而不是宿主的一屏 64：nian 的精确字有 96 个，泄漏点（词组）在 87 位 ——
/// 只抓 64 时这条门禁对旧语义**无分辨力**（实测：改前 64 内全是字、改后同）。
#[test]
fn pinned_nian_drops_words_and_extension_reading() {
    let mut e = real_engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "6426");
    e.set_pinned_reading(Some("nian".into()));
    let got = texts(&e, 256);
    assert!(
        got.contains(&"年".to_string()) && got.contains(&"念".to_string()),
        "{got:?}"
    );
    for bad in ["年度", "年级", "娘"] {
        assert!(
            !got.contains(&bad.to_string()),
            "读音更长（nianji/niang）的不得进：{bad} 在 {got:?}"
        );
    }
    assert_eq!(e.candidate_readings(), vec!["nian".to_string()]);
}

/// 钉词组读音 `ni'hao`（打 `64426`）：只留读音恰为 nihao 的条目，`你好啊/你好吗` 等出局。
#[test]
fn pinned_word_reading_keeps_only_exact_entries() {
    let mut e = real_engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426");
    let all = texts(&e, 64);
    assert!(
        all.contains(&"你好啊".to_string()),
        "前置：未钉时延伸词可达：{all:?}"
    );

    e.set_pinned_reading(Some("ni'hao".into()));
    let got = texts(&e, 64);
    assert_eq!(
        got,
        vec!["你好".to_string(), "拟好".to_string()],
        "钉 nihao 只剩读音恰为 nihao 的条目"
    );
    assert_eq!(e.candidate_readings(), vec!["ni'hao".to_string()]);
}
