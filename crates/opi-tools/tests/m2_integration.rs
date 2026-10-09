// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! M2 端到端集成测试：Engine + mmap 词典（编译 → 序列化 → 加载 → 输入 → 选词）。
//! 验证词典注入后引擎全链路可用，以及词库损坏时的回退路径。

use engine_core::engine::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::{
    FormatError, LoadError, fallback_dict, load_bytes, load_mmap, load_or_fallback, serialize,
};
use opi_tools::compiler::{compile, parse_dict};
use std::path::Path;

/// 部署副本（`data/generated/luna.opid` 是 gitignore 的构建中间物），与
/// `rare_coverage.rs` / `simplified_phrases.rs` 同一取法。
const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

#[test]
fn full_pipeline_engine_typing() {
    let text = "好\thao\t5000\n号\thao\t1200\n笑\txiao\t3000\n";
    let bytes = serialize(&compile(parse_dict(text)));
    let mmap = load_bytes(bytes).expect("load");
    let mut eng = Engine::new(Box::new(mmap), SymbolEngine::builtin(), false);
    for ch in "hao".chars() {
        eng.input_key(ch);
    }
    assert_eq!(eng.buffer(), "hao");
    let cands = eng.candidates(8);
    assert_eq!(cands[0].text, "好");
    assert_eq!(eng.select(0), "好");
    assert_eq!(eng.buffer(), "");
}

#[test]
fn engine_with_fallback_dict() {
    let mut eng = Engine::new(Box::new(fallback_dict()), SymbolEngine::builtin(), true);
    for ch in "wo".chars() {
        eng.input_key(ch);
    }
    let cands = eng.candidates(8);
    assert_eq!(cands[0].text, "我");
}

#[test]
fn corrupt_file_engine_falls_back() {
    let mut bytes = serialize(&compile(parse_dict("好\thao\n")));
    let n = bytes.len() - 9; // payload 区域（非 trailer）
    bytes[n] ^= 0xFF;
    assert!(matches!(
        load_bytes(bytes),
        Err(LoadError::Format(FormatError::ChecksumMismatch { .. }))
    ));
    assert!(load_or_fallback(Some(std::path::Path::new("/nonexistent/opi.opid"))).is_err());
}

#[test]
fn corrupted_opid_engine_still_boots() {
    // 引擎在词库损坏时仍可用（回退内置），全链路冒烟
    let mut eng = Engine::new(Box::new(fallback_dict()), SymbolEngine::builtin(), false);
    for ch in "n".chars() {
        eng.input_key(ch);
    }
    assert!(eng.candidates(8).iter().any(|c| c.text == "你"));
}

/// 分词回退（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）在**真词库**上的验收。
/// 语义与边界的内联夹具版在 `engine-core/tests/segment_fallback.rs`，本条钉的是
/// **luna 简体真数据**下的三条：主用例（无命中 → 最长前缀的整档 → 选中只消耗前缀）、
/// 「整串模糊命中优先」（`zongguo` 不被回退接管）、「垃圾串维持死 + 逃生口」。
/// 符号表用**空表**：内建表会往 `hao` 这类拼音里掺料（memory「计数夹具要钉空符号表」）。
#[test]
fn segment_fallback_on_real_luna_dictionary() {
    let luna = || {
        Engine::new(
            Box::new(
                load_mmap(Path::new(LUNA_OPID))
                    .unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}")),
            ),
            SymbolEngine::new(Vec::new(), Vec::new()),
            false,
        )
    };
    let texts =
        |e: &Engine| -> Vec<String> { e.candidates(8).into_iter().map(|c| c.text).collect() };
    // 主用例：`nihaob` 整串（`query_all("nihaob")` 0 行）⇒ 最长前缀 `nihao` 的整档
    let mut eng = luna();
    for ch in "nihaob".chars() {
        eng.input_key(ch);
    }
    assert_eq!(eng.buffer(), "nihaob", "预编辑仍是整串");
    let got = texts(&eng);
    assert_eq!(got[0], "你好", "{got:?}");
    // 选中只消耗前缀 —— 旧语义（逐音节汤）在这里会把整个缓冲清掉
    assert_eq!(eng.select(0), "你好");
    assert_eq!(eng.buffer(), "b", "后缀留在缓冲里接着编");
    for ch in "ao".chars() {
        eng.input_key(ch);
    }
    assert_eq!(eng.buffer(), "bao");
    // 后缀是真缓冲：候选表与另开一台直接打 `bao` 的**逐条相同**
    let mut fresh = luna();
    for ch in "bao".chars() {
        fresh.input_key(ch);
    }
    assert_eq!(texts(&eng), texts(&fresh), "后缀接着编走的是同一条路径");
    // 边界②：「整串模糊命中优先」——`zongguo` 的变体 `zhongguo` 在 luna 有 81 行 ⇒
    // 回退不接管，逐音节汤照旧（`guo` 的单字在候选里；回退会把它当成后缀拿走）
    let mut eng = luna();
    for ch in "zongguo".chars() {
        eng.input_key(ch);
    }
    let got = texts(&eng);
    assert!(got.contains(&"中国".to_string()), "{got:?}");
    assert!(got.contains(&"国".to_string()), "{got:?}");
    // 边界①：垃圾串（段不是完整音节）维持死 —— 「打英文的逃生口」不动
    let mut eng = luna();
    for ch in "zxcvb".chars() {
        eng.input_key(ch);
    }
    assert!(texts(&eng).is_empty(), "`zxcvb` 不得被分词回退救活");
    assert_eq!(eng.input_key(' '), "zxcvb", "无候选 ⇒ 空格提交原文");
    assert_eq!(eng.buffer(), "");
}
