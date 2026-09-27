// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 简体常用词组门禁（用户真机反馈：「常用词组无法拼音或模糊拼音显示」）。
//!
//! 根因**不在模糊拼音层**（模糊已实现并验证），在**词源**：`luna.opid` 的词组段全部
//! 来自 luna_pinyin.dict.yaml 的繁体文言成语段（中國內地、一不拗衆），且拼音**故意
//! 用空格分隔**（`scripts/gen_luna_dict.py` 的 parse_rime 注释写明「不要为激活词组去
//! 空格」）—— 引擎 buffer 是连续字母，整串查询永远不命中这些词。实测「你好」「中国」
//! 「谢谢」「什么」「我们」在 luna.opid 里 **0 条**（`data.count(w.encode())`）。
//!
//! 本门禁锁**用户要的那件事**：敲 `nihao` 出「你好」、敲 `zhongguo` 出「中国」。
//! 字形侧锁「不得回到繁体」：同拼音的繁体对照词不得出现在候选前列 —— 这正是当年
//! 「词组不激活」的理由，换简体词源后必须仍然成立。
//!
//! 产物：只读不联网。luna 取 android 部署副本（与 `ranking_quality.rs` 同源，
//! `data/generated/luna.opid` 是 gitignore 的构建中间物）。
//!
//! 注：luna 的「两份副本必须一致」没有门禁（trad 有 `ranking_quality.rs::trad_assets_in_sync`，
//! luna 比不了 —— `data/generated/luna.opid` 不入库，干净 checkout 上不存在）。本次改数据
//! 是人工同步两份，改完须自己 `sha256sum` 核对。

use engine_core::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::load_mmap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

/// (拼音, 期望简体词)：必须是该拼音的**第 1** 个候选。
///
/// 为什么敢要求第 1：这些拼音在简体词组表（rime-pinyin-simp，见 LICENSES.md）里都是
/// 同音词中词频最高者，且**没有同拼音的单字竞争者**（`nihao`/`zhongguo`/… 都不是合法
/// 音节）—— 精确等长匹配层里它们按词频降序，故必然第 1。
/// 同音词实测：xiexie 4 个（谢谢 47594 > 写写 556）、shijian 7 个（时间 92935 > 事件 13022）、
/// keyi 4 个（可以 304372 > 刻意 1693）。
const EXPECTED: &[(&str, &str)] = &[
    // 用户与 lead 点名的三个
    ("nihao", "你好"),
    ("zhongguo", "中国"),
    ("dianhua", "电话"),
    // 其余高频日常词（同一次换源应一并具备）
    ("xiexie", "谢谢"),
    ("shenme", "什么"),
    ("women", "我们"),
    ("shijian", "时间"),
    ("diannao", "电脑"),
    ("xuesheng", "学生"),
    ("taiwan", "台湾"),
    ("keyi", "可以"),
    ("zhidao", "知道"),
    ("pengyou", "朋友"),
];

/// (拼音, 繁体对照词)：不得出现在该拼音的**前 8** 个候选里。
///
/// 繁体词在 luna 里只能有两种来路：①旧的空格拼音词组段（打不出来，但会以短前缀
/// 命中混进候选尾巴）；②新词源若是繁体表（如 terra 繁→简前的原表），会被原样带进来。
/// ②是这条断言真正要拦的：一旦有人把词源换回繁体表，`zhongguo` 的首位就会是「中國」。
const TRADITIONAL_COUNTERPARTS: &[(&str, &str)] = &[
    ("nihao", "妳好"),
    ("zhongguo", "中國"),
    ("xiexie", "謝謝"),
    ("shenme", "什麼"),
    ("women", "我們"),
    ("dianhua", "電話"),
    ("shijian", "時間"),
    ("diannao", "電腦"),
    ("xuesheng", "學生"),
    ("taiwan", "臺灣"),
];

/// 空符号表：本门禁只测拼音候选，符号数据不参与这里的断言（同 ranking_quality.rs）。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 敲入拼音并取候选（`Engine::candidates` = candidates.rs `rank_and_pick`，排序规则所在层）。
/// 必须走 `Engine` 而不是 `dict.query`：词组能否排在单字之后、精确匹配能否分层，全在
/// candidates.rs，查 query 会让这些缺陷对门禁不可见（同 ranking_quality.rs 的注释）。
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

fn luna_engine() -> Engine {
    let dict =
        load_mmap(Path::new(LUNA_OPID)).unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}"));
    // learner 关闭：门禁锁静态排序（同 ranking_quality.rs，否则跑两次结果不同）。
    Engine::new(Box::new(dict), no_symbols(), false)
}

#[test]
fn luna_common_simplified_phrases_rank_first() {
    let mut engine = luna_engine();
    let mut fails = Vec::new();
    for (py, word) in EXPECTED {
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
        "luna 简体常用词组缺失或不在首位（{}/{}）：\n  {}",
        fails.len(),
        EXPECTED.len(),
        fails.join("\n  ")
    );
}

#[test]
fn luna_phrase_candidates_are_not_traditional() {
    let mut engine = luna_engine();
    let mut fails = Vec::new();
    for (py, trad) in TRADITIONAL_COUNTERPARTS {
        let cands = candidates_of(&mut engine, py);
        if let Some(i) = cands.iter().take(8).position(|c| c == trad) {
            fails.push(format!("{py}: 繁体「{trad}」出现在前 8 的第 {} 位", i + 1));
        }
    }
    assert!(
        fails.is_empty(),
        "luna 候选栏混入繁体字形（{}/{}）：\n  {}",
        fails.len(),
        TRADITIONAL_COUNTERPARTS.len(),
        fails.join("\n  ")
    );
}
