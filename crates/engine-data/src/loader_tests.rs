// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use super::*;
use crate::format::{FormatError, OpDict, RawEntry, serialize};

fn sample() -> Vec<u8> {
    serialize(&OpDict {
        entries: vec![
            RawEntry {
                pinyin: "hao".into(),
                word: "好".into(),
                freq: 5000,
            },
            RawEntry {
                pinyin: "hao".into(),
                word: "号".into(),
                freq: 1200,
            },
            RawEntry {
                pinyin: "xiao".into(),
                word: "笑".into(),
                freq: 3000,
            },
        ],
        // pinyin blob = "hao"(3) + "hao"(3) + "xiao"(4) = 10 字节
        pinyin_total: 10,
    })
}

fn temp_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "opi-load-{}-{}.opid",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn load_bytes_query_prefix_ordered() {
    let d = load_bytes(sample()).unwrap();
    assert_eq!(d.len(), 3);
    let got = d.query("h", 8);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].word, "好");
    assert_eq!(got[1].word, "号");
    let x = d.query("xiao", 8);
    assert_eq!(x[0].word, "笑");
}

#[test]
fn query_partial_and_full_match() {
    let d = load_bytes(sample()).unwrap();
    assert_eq!(d.query("hao", 8).len(), 2);
    assert_eq!(d.query("ha", 8).len(), 2);
    assert_eq!(d.query("h", 8).len(), 2);
}

/// `query_all` 只少了排序，**不许**少条目（截断/丢词会让候选表静默变窄）。
#[test]
fn query_all_returns_same_set_as_unlimited_query() {
    let d = load_bytes(sample()).unwrap();
    let sorted: Vec<String> = d
        .query("h", usize::MAX)
        .into_iter()
        .map(|e| e.word)
        .collect();
    let mut all: Vec<String> = d.query_all("h").into_iter().map(|e| e.word).collect();
    let mut sorted_sorted = sorted;
    sorted_sorted.sort();
    all.sort();
    assert_eq!(all, sorted_sorted);
    assert!(d.query_all("").is_empty());
    assert!(d.query_all("zz").is_empty());
}

#[test]
fn query_limit_truncates() {
    let d = load_bytes(sample()).unwrap();
    let got = d.query("hao", 1);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].word, "好");
}

#[test]
fn query_no_match_or_empty() {
    let d = load_bytes(sample()).unwrap();
    assert!(d.query("zz", 8).is_empty());
    assert!(d.query("", 8).is_empty());
}

#[test]
fn load_mmap_reads_file() {
    let path = temp_path();
    std::fs::write(&path, sample()).unwrap();
    let d = load_mmap(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let got = d.query("hao", 8);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].word, "好");
}

#[test]
fn corrupt_file_rejected() {
    let mut bytes = sample();
    bytes[20] ^= 0xFF;
    assert!(matches!(
        load_bytes(bytes),
        Err(LoadError::Format(FormatError::ChecksumMismatch { .. }))
    ));
}

#[test]
fn missing_file_io_error() {
    assert!(matches!(
        load_mmap(Path::new("/nonexistent/opi.opid")),
        Err(LoadError::Io(_))
    ));
}
