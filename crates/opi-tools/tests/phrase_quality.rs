// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 外部词库并入的语义门禁（2026-10-11，用户裁决「主流开源词库去重整合」）。
//!
//! 钉**用户要的那件事**：品牌/新词/成语/歇后语/专业术语/大词库词组打得出、排在
//! 该在的位置；**且繁体词形没有回潮**（当年 21759 条繁体成语段的老病）。
//!
//! **先证伪纪律**：本文件每颗钉（除繁体负对照的 trad 正控外）都在**并入前的
//! asset** 上跑过一次、全部 0 候选/不在候选里（证据是当时的红输出）；在新 asset
//! 上才改断言。没有这一步，钉子可能因「旧库本来就有」而通过 —— 那是观察点等价，
//! 钉的就不是本次整合。同理，断言一律走 `Engine::candidates`（不是 `dict.query`）：
//! 「层序/去重/截断」全在 candidates.rs，查 query 会让这些对门禁不可见
//! （同 `simplified_phrases.rs` / `ranking_quality.rs` 的注释）。
//!
//! 关于两档断言：
//! - **第 1 档**给品牌/新词层（人工表，层序在 base 之前）——同键无更高层竞争者；
//! - **可达档**给其余各层 —— 名次不限：层深只影响同键排序，可达性才是用户承诺。
//!
//! 繁体负对照用**字节扫描**而非 candidates：`電話` 的两个单字都在 luna 单字层
//! （電/話 是 luna 既有字形，文档写明「单字层的繁体字形一个不动」），拿候选栏判
//! 会与那条规定打架；词形级别的「0 条」用整词字节扫描最准。

use engine_core::symbols::SymbolEngine;
use engine_core::Engine;
use engine_data::load_mmap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);
const TRAD_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/trad.opid"
);

/// (拼音, 期望词)：必须是该拼音的**第 1** 个候选。品牌与网络用语两张人工表是
/// 词组三段序的**前置段**（人工 → simp → 批量，见 gen_luna_dict 头注释）——
/// 前置的理由就是本用例：`威信` 在 simp 层权重 3.71e9，坐落在批量段（`威信`
/// 实测会把 `微信` 压到第 3）。前置后同键无更高层竞争者（simp/批量段的同键
/// (key,word) 副本被去重让位，若同键还有别的词则按词频竞争 —— 目前这些键没有）。
const FIRST: &[(&str, &str)] = &[
    ("weixin", "微信"),
    ("pinduoduo", "拼多多"),
    ("neijuan", "内卷"),
    ("tangping", "躺平"),
    ("dagongren", "打工人"),
    ("xianyanbao", "显眼包"),
];

/// (拼音, 期望词)：**可达**即可（出现于候选，名次不限）。每层各选代表：
/// ext 层（短视频）、base 层（芯片/区块链）、成语层（阿鼻地狱）、歇后语层
/// （整条为词，键只由 CJK 生成）、essay 层（然後/由於 —— GB2312 成员按既有
/// 判据保留的繁体字面，判据见 gen_extra_phrases 的 `is_trad`）、THUOCL 术语层
/// （肺结核）、品牌层长词（呷哺呷哺 —— 也曾是 `吒`/OpenCC 误杀的回归钉）。
const REACHABLE: &[(&str, &str)] = &[
    ("duanshipin", "短视频"),
    ("xinpian", "芯片"),
    ("qukuailian", "区块链"),
    ("abidiyu", "阿鼻地狱"),
    ("adoudejiangshanbaisong", "阿斗的江山——白送"),
    ("ranhou", "然後"),
    ("youyu", "由於"),
    ("feijiehe", "肺结核"),
    ("xiabuxiabu", "呷哺呷哺"),
    ("nezha", "哪吒"),
];

/// 繁体回潮负对照：(拼音, 繁体词形)。luna.opid **字节扫描 0 条**，
/// **trad.opid 字节扫描 > 0**（正控：否则「0」可能只是扫描/编码写错）。
const TRAD_CONTROL: &[(&str, &str)] = &[
    ("dianhua", "電話"),
    ("xuesheng", "學生"),
    ("shijian", "時間"),
    ("women", "我們"),
    ("shuofa", "說法"),
];

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine_at(path: &str) -> Engine {
    let dict = load_mmap(Path::new(path)).unwrap_or_else(|e| panic!("加载 {path} 失败：{e:?}"));
    Engine::new(Box::new(dict), no_symbols(), false)
}

/// 敲入拼音并取候选（`Engine::candidates` = candidates.rs `rank_and_pick`）。
fn candidates_of(engine: &mut Engine, pinyin: &str) -> Vec<String> {
    engine.clear();
    for ch in pinyin.chars() {
        engine.input_key(ch);
    }
    engine
        .candidates(usize::MAX)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[test]
fn merged_brand_and_slang_words_rank_first() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (py, word) in FIRST {
        let cands = candidates_of(&mut engine, py);
        let got = cands.first().map(String::as_str).unwrap_or("—");
        if got != *word {
            let pos = cands
                .iter()
                .position(|c| c == word)
                .map(|i| (i + 1).to_string())
                .unwrap_or_else(|| "缺".into());
            fails.push(format!(
                "{py}: 期望第 1 是「{word}」，实得「{got}」（期望词名次 {pos}）"
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "并入层品牌/新词缺失或不在首位（{}/{}）：\n  {}",
        fails.len(),
        FIRST.len(),
        fails.join("\n  ")
    );
}

#[test]
fn merged_layers_are_reachable() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (py, word) in REACHABLE {
        let cands = candidates_of(&mut engine, py);
        if !cands.iter().any(|c| c == word) {
            fails.push(format!(
                "{py}: 候选里没有「{word}」（共 {} 个候选）",
                cands.len()
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "并入层条目不可达（{}/{}）：\n  {}",
        fails.len(),
        REACHABLE.len(),
        fails.join("\n  ")
    );
}

#[test]
fn no_traditional_word_forms_leaked_into_luna() {
    let luna = std::fs::read(LUNA_OPID).expect("读 luna.opid");
    let luna = String::from_utf8_lossy(&luna);
    let trad = std::fs::read(TRAD_OPID).expect("读 trad.opid");
    let trad = String::from_utf8_lossy(&trad);
    let mut fails = Vec::new();
    for (py, word) in TRAD_CONTROL {
        // 正控先行：trad.opid 里扫不到 = 扫描本身有问题，先红在正控上。
        assert!(
            trad.contains(word),
            "正控失败：trad.opid 里扫不到「{word}」—— 扫描/编码写错了，否定断言无意义"
        );
        if luna.contains(word) {
            fails.push(format!("{py}: luna.opid 里出现了繁体词形「{word}」"));
        }
    }
    assert!(
        fails.is_empty(),
        "并入源把繁体词形带回了简洁库（{}/{}）：\n  {}",
        fails.len(),
        TRAD_CONTROL.len(),
        fails.join("\n  ")
    );
}
