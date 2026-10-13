// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 纠错档的**分档位次**门禁（2026-10-14 修，真词库探针抓到的产品回归）。
//!
//! ## 事故（探针原件）
//!
//! 真 luna 词库 + 产品默认（纠错开）+ 输入 `nh`（简拼「你好」）：
//! ```text
//! 纠错开：安徽 安徽省 那好 那会 呐喊 暗黑 恩惠 嗯哼 蒽环类 | 南航 内耗 你好 …   ← 第一页没有「你好」
//! 纠错关：南航 内耗 你好 女孩 女孩子 您好 你会 男孩 …                        ← 正常
//! ```
//! 机制：纠错命中曾被并入 `fuzzy` 档，而 `fuzzy` 档有「主档为空时我就是主结果」的
//! 晋升规则（`zongguo`→「中国」那条）。`nh` 的主档本来就空 ⇒ `merged = fuzzy`
//! （全是纠错噪声）⇒ 简拼档只能被**追加**，再被 `truncate(limit)` 砍掉 —— 第一页
//! 里没有任何简拼结果。纠错变体 `anh` 在真词库里键真有词（`query_all("anh")` 命中
//! 安哈尔特/俺还/暗害/…，含高频的「安徽」= 3787750734），这就是噪声来源。
//!
//! ## 修法与这条门禁
//!
//! 纠错独立成 `corr` 档，**排在最外**（模糊 → 简拼 → 纠错），晋升链一字不动；
//! `wide_hits`（回退「有候选就别接管」判据）仍把纠错命中算作候选。
//! 本文件用**夹具**复现同一形状（不依赖 luna.opid）：输入 `nh`，纠错变体 `anh`
//! 在词库里有一条命中 ⇒ 旧代码会把简拼的「你好」挤出 `limit`，新代码不会。

use engine_core::Engine;
use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::symbols::SymbolEngine;

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 夹具：`nihao` 一族（简拼 `nh` 够得着）+ `anh`（**只有纠错变体够得着**）。
/// 别的键与 `nh` 的任何简拼/模糊读法都不相干。
fn dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("ni", "你", 3_999_128_899u32),
        ("hao", "好", 3_997_635_583),
        ("nihao", "你好", 3_996_000_000),
        // 纠错变体 `anh` 的命中（真词库同形：`query_all("anh")` 非空）。两条词，
        // 好让旧代码的截断（limit=2）把简拼结果整个挤掉。
        ("anh", "锕", 3_780_000_000),
        ("anh", "锿", 3_770_000_000),
    ] {
        d.insert(py, w, f);
    }
    d
}

fn cands(limit: usize, correction: bool) -> Vec<String> {
    let mut e = Engine::new(Box::new(dict()), no_symbols(), false);
    e.set_correction(correction);
    e.set_jianpin(true);
    e.set_fuzzy(true);
    for ch in "nh".chars() {
        e.input_key(ch);
    }
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 前提自查（也给门禁失明兜底）：**纠错变体确实够得着** —— 没有这一条，下面
/// 两条断言在「纠错根本没跑」的实现下也会绿（假绿）。
#[test]
fn premise_the_correction_variant_has_hits() {
    let vs = engine_core::correction::input_corrections("nh");
    assert!(
        vs.contains(&"anh".to_string()),
        "夹具的前提是 `nh` 的单段纠错变体里有 `anh`（实得 {vs:?}）"
    );
    let d = dict();
    assert!(
        !d.query_all("anh").is_empty(),
        "夹具前提：`anh` 键在词库里真有词（真 luna 同形）"
    );
}

/// **主断言**：产品默认（纠错开）下，`nh` 的第一页首位 = 「你好」。
/// 旧代码在这里红：首位是「锕」（纠错档被晋升成主结果，简拼被截断）。
#[test]
fn jianpin_result_survives_the_correction_tier() {
    let got = cands(2, true);
    assert_eq!(
        got.first().map(String::as_str),
        Some("你好"),
        "纠错开着时 `nh` 的首位必须是简拼的「你好」（实得 {got:?}）"
    );
}

/// 纠错命中**仍参与**（用户裁决「混合参与排序」，只是排在最外）：放宽 limit 就能拿到。
/// 这条挡住「把纠错整档删掉」那种过修。
#[test]
fn correction_hits_still_participate_after_the_jianpin_tier() {
    let got = cands(20, true);
    assert!(got.contains(&"你好".to_string()), "简拼结果在（{got:?}）");
    assert!(
        got.iter().any(|t| t == "锕" || t == "锿"),
        "纠错命中该在列表里（最外层但可翻到）：{got:?}"
    );
    let jp = got.iter().position(|t| t == "你好").unwrap();
    let corr = got.iter().position(|t| t == "锕" || t == "锿").unwrap();
    assert!(jp < corr, "简拼结果必须排在纠错命中之前（{got:?}）");
}

/// 关掉纠错 ⇒ 列表里一条纠正来路都没有（双向：开关真的管这一段）。
#[test]
fn correction_off_drops_the_variant_hits() {
    let got = cands(20, false);
    assert!(got.contains(&"你好".to_string()), "简拼照旧（{got:?}）");
    assert!(
        !got.iter().any(|t| t == "锕" || t == "锿"),
        "纠错关了 ⇒ `anh` 的命中不许出现（{got:?}）"
    );
}
