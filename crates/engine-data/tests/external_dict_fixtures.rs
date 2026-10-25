// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **外部词库转换器产物 · 装载门禁**（2026-10-26）。
//!
//! 夹具是两万分之一量级的真实上游摘录（逐源署名与许可见
//! `tests/fixtures/external/README.md`），由 `scripts/convert_*` 生成。
//! 这里只验**输出契约**：`load_extra_dict` 能读、注释头不算词条、无调连写键
//! 能查到、两列默认 1000、同词不同读音都在、重复取最大词频。
//!
//! 转换器本身的逐字节一致性在 `scripts/tests/test_external_dict_converters.py`
//! （Rust 侧不调用 Python，避免把解释器变成 cargo test 的隐藏依赖）。

use engine_core::dictionary::Dictionary;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("external")
        .join(name)
}

fn words(dict: &impl Dictionary, pinyin: &str) -> Vec<String> {
    dict.query(pinyin, 8).into_iter().map(|e| e.word).collect()
}

#[test]
fn custom_pinyin_dictionary_fixture_loads() {
    let path = fixture("cpd_tiny.tsv");
    let (dict, bad) = match engine_data::load_extra_dict(&path) {
        Ok(v) => v,
        Err(e) => panic!("转换产物被装载器拒绝（{}）：{e}", path.display()),
    };
    assert_eq!(bad, 0, "生成的 TSV 不应有列数坏行");
    assert_eq!(dict.len(), 7, "注释头不能变成词条");
    assert!(words(&dict, "nihao").contains(&"你好".to_string()));
    assert!(words(&dict, "nver").contains(&"女儿".to_string()), "ü→v");
    assert!(words(&dict, "ngna").contains(&"嗯哪".to_string()));
    let baba = words(&dict, "baba");
    assert!(baba.contains(&"爸爸".to_string()));
    assert!(baba.contains(&"㞎㞎".to_string()), "同键不同词都保留");
    assert!(words(&dict, "babamama").contains(&"爸爸妈妈".to_string()));
    assert!(words(&dict, "beijingguanqu").contains(&"北京关区".to_string()));
    // 上游无可用词频 ⇒ 两列；装载器默认 1000。
    assert_eq!(dict.query("nihao", 1)[0].freq, 1000);
}

#[test]
fn rime_wanxiang_fixture_loads() {
    let path = fixture("wanxiang_tiny.tsv");
    let (dict, bad) = match engine_data::load_extra_dict(&path) {
        Ok(v) => v,
        Err(e) => panic!("转换产物被装载器拒绝（{}）：{e}", path.display()),
    };
    assert_eq!(bad, 0, "生成的 TSV 不应有列数坏行");
    assert_eq!(dict.len(), 13, "注释头不能变成词条；去重按 (词, 拼音)");

    // 声调剥离 + 音节空格连写。
    assert_eq!(dict.query("nihao", 1)[0].word, "你好");
    assert_eq!(dict.query("nihao", 1)[0].freq, 76730);
    assert_eq!(dict.query("beijing", 1)[0].word, "北京");
    assert_eq!(dict.query("nver", 1)[0].word, "女儿", "nǚ er → nver");

    // 同词不同读音：单库合法语义，合并层按 (拼音, 词) 判重，不该被折掉。
    assert_eq!(dict.query("xing", 1)[0].word, "行");
    assert_eq!(dict.query("xing", 1)[0].freq, 92900);
    assert_eq!(dict.query("hang", 1)[0].word, "行");
    assert_eq!(dict.query("zhong", 1)[0].word, "重");
    assert_eq!(dict.query("chong", 1)[0].word, "重");

    // 同拼音不同声调 → 同一个无调键，取最大权重（啊 a/ā）。
    assert_eq!(dict.query("a", 1)[0].word, "啊");
    assert_eq!(dict.query("a", 1)[0].freq, 759839);

    // 两列条目走装载器默认；非法权重 `6N` 在转换时降级为 1000。
    assert_eq!(dict.query("beike", 1)[0].freq, 1000);
    assert_eq!(dict.query("fufangdanshen", 1)[0].freq, 1000);
}
