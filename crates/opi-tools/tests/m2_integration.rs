//! M2 端到端集成测试：Engine + mmap 词典（编译 → 序列化 → 加载 → 输入 → 选词）。
//! 验证词典注入后引擎全链路可用，以及词库损坏时的回退路径。

use engine_core::engine::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::{fallback_dict, load_bytes, load_or_fallback, serialize, FormatError, LoadError};
use opi_tools::compiler::{compile, parse_dict};

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
    let mut eng = Engine::new(
        Box::new(fallback_dict()),
        SymbolEngine::builtin(),
        false,
    );
    for ch in "n".chars() {
        eng.input_key(ch);
    }
    assert!(eng.candidates(8).iter().any(|c| c.text == "你"));
}
