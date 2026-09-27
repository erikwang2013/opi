// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `learner.rs` 的单元测试（`#[path]` 引入，见该文件尾部）：单测独立成文件以保持
//! 源文件 <500 行，与 `candidates_tests.rs` / `router_tests.rs` 同惯例。

use super::*;

// ---------------------------------------------------------------- 内存语义

#[test]
fn disabled_learner_ignores_selections() {
    let mut l = Learner::new(false);
    l.record_selection("好");
    assert_eq!(l.freq_of("好"), 0);
    assert_eq!(l.export_json(), r#"{"version":1,"words":[]}"#);
}

#[test]
fn records_and_counts_selections() {
    let mut l = Learner::new(true);
    l.record_selection("好");
    l.record_selection("好");
    l.record_selection("号");
    assert_eq!(l.freq_of("好"), 2);
    assert_eq!(l.freq_of("号"), 1);
}

#[test]
fn remove_word_drops_freq() {
    let mut l = Learner::new(true);
    l.record_selection("好");
    l.remove_word("好");
    assert_eq!(l.freq_of("好"), 0);
}

#[test]
fn clear_empties_everything() {
    let mut l = Learner::new(true);
    l.record_selection("好");
    l.clear();
    assert_eq!(l.freq_of("好"), 0);
    assert_eq!(l.export_json(), r#"{"version":1,"words":[]}"#);
}

// ---------------------------------------------------------------- 导入语义

/// 导入 = 与内存状态**合并**，重复词取 max（不是累加）：同一份 JSON 重复导入
/// （Android 每次启动都会读同一个文件）必须幂等，否则频次无界膨胀。
#[test]
fn import_json_merges_by_max_freq() {
    let mut l = Learner::new(true);
    l.record_selection("好"); // 内存里已有 1 次
    let json = r#"{"version":1,"words":[{"text":"好","freq":5},{"text":"号","freq":3}]}"#;
    assert_eq!(l.import_json(json).unwrap(), 2);
    assert_eq!(l.freq_of("好"), 5, "取 max(1,5)");
    assert_eq!(l.freq_of("号"), 3, "新词按文件频次进内存");
    // 幂等：再导入一次不变
    assert_eq!(l.import_json(json).unwrap(), 2);
    assert_eq!(l.freq_of("好"), 5);
    // 两棵树一致：user_words 也含导入词，导出才有它们
    let parsed: UserWordExport = serde_json::from_str(&l.export_json()).unwrap();
    assert_eq!(parsed.words.len(), 2);
}

/// 导入的数据可比内存新（手机上删了自造词又恢复备份）：取 max 而非只增不减。
#[test]
fn import_json_never_lowers_existing_freq() {
    let mut l = Learner::new(true);
    for _ in 0..9 {
        l.record_selection("好");
    }
    l.import_json(r#"{"version":1,"words":[{"text":"好","freq":2}]}"#)
        .unwrap();
    assert_eq!(l.freq_of("好"), 9);
}

/// enabled=false 只关「记录新选词」，不关「载入用户自己的数据」：
/// freq_of 本来就参与排序（与 enabled 无关），导入被判为 no-op 会让用户
/// 关掉学习后再打开时词库凭空消失。
#[test]
fn import_json_applies_even_when_disabled() {
    let mut l = Learner::new(false);
    assert_eq!(
        l.import_json(r#"{"version":1,"words":[{"text":"好","freq":2}]}"#)
            .unwrap(),
        1
    );
    assert_eq!(l.freq_of("好"), 2);
    l.record_selection("好"); // 仍不记录
    assert_eq!(l.freq_of("好"), 2);
}

#[test]
fn import_json_rejects_malformed_and_wrong_version() {
    let mut l = Learner::new(true);
    for bad in [
        "",
        "not json",
        "{}",                                              // 缺 version/words
        r#"{"version":1}"#,                                // 缺 words
        r#"{"version":2,"words":[]}"#,                     // 版本不符
        r#"{"version":1,"words":[{"text":"好"}]}"#,        // 词条缺 freq
        r#"{"version":1,"words":[{"freq":3}]}"#,           // 词条缺 text
        r#"{"version":1,"words":[{"text":"","freq":3}]}"#, // 空词
    ] {
        assert!(l.import_json(bad).is_err(), "应拒绝：{bad}");
    }
    assert_eq!(
        l.export_json(),
        r#"{"version":1,"words":[]}"#,
        "失败不得部分导入"
    );
}

/// 超大词表直接拒绝：导入是信任边界（文件来自设备存储/恢复备份），
/// 上限挡住畸形文件把内存打爆。
#[test]
fn import_json_rejects_oversized_word_list() {
    let mut l = Learner::new(true);
    let one = r#"{"text":"好","freq":1}"#;
    let body = std::iter::repeat_n(one, MAX_IMPORT_WORDS + 1)
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(r#"{{"version":1,"words":[{body}]}}"#);
    assert!(l.import_json(&json).is_err());
    // 恰好等于上限则接受
    let body = std::iter::repeat_n(one, MAX_IMPORT_WORDS)
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(r#"{{"version":1,"words":[{body}]}}"#);
    assert_eq!(l.import_json(&json).unwrap(), MAX_IMPORT_WORDS);
    assert_eq!(l.freq_of("好"), 1);
}

/// 导入频次可能已是 u32::MAX（外部数据），此后再选词不得溢出 panic。
#[test]
fn record_after_max_freq_import_saturates() {
    let mut l = Learner::new(true);
    l.import_json(r#"{"version":1,"words":[{"text":"好","freq":4294967295}]}"#)
        .unwrap();
    l.record_selection("好");
    assert_eq!(l.freq_of("好"), u32::MAX);
}

#[test]
fn export_json_has_version_and_words() {
    let mut l = Learner::new(true);
    l.record_selection("好");
    l.record_selection("号");
    let json = l.export_json();
    let parsed: UserWordExport = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.words.len(), 2);
}
