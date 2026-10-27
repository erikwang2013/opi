// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use super::{AssocRow, serialize_with_extras};
use crate::format::{self, FormatError, OpDict, RawEntry, TRAILER_LEN};

fn sample() -> OpDict {
    OpDict {
        entries: vec![
            RawEntry {
                pinyin: "jintian".into(),
                word: "今天".into(),
                freq: 900,
            },
            RawEntry {
                pinyin: "jintian".into(),
                word: "今天".into(),
                freq: 800,
            },
            RawEntry {
                pinyin: "jintianwanshang".into(),
                word: "今天晚上".into(),
                freq: 700,
            },
            RawEntry {
                pinyin: "huijia".into(),
                word: "回家".into(),
                freq: 600,
            },
        ],
        pinyin_total: "jintianjintianjintianwanshanghuijia".len(),
    }
}

fn reseal(bytes: &mut [u8]) {
    let tail = bytes.len() - TRAILER_LEN;
    let sum = crate::checksum::fnv1a64(&bytes[format::HEADER_LEN..tail]);
    bytes[tail..].copy_from_slice(&sum.to_le_bytes());
}

fn assoc_field_pos(bytes: &[u8]) -> usize {
    bytes.len() - TRAILER_LEN - 4
}

fn dir_root_off(bytes: &[u8], v: &format::Validated, i: usize) -> usize {
    let assoc = v.assoc.as_ref().expect("assoc");
    let off = assoc.start + 4 + i * 6;
    u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize
}

#[test]
fn extras_absent_is_byte_identical_to_serialize() {
    let d = sample();
    assert_eq!(serialize_with_extras(&d, false, &[]), format::serialize(&d));
}

#[test]
fn roundtrip_flags_index_and_assoc() {
    let d = sample();
    let rows = vec![
        AssocRow {
            root: "今天".into(),
            suffix: "回家".into(),
            rank: 600,
        },
        AssocRow {
            root: "今天".into(),
            suffix: "晚上好".into(),
            rank: 0,
        },
    ];
    let bytes = serialize_with_extras(&d, true, &rows);
    let v = format::validate(&bytes).expect("新文件必须通过校验");
    assert_eq!(v.flags, 0b11);
    assert!(v.word_index.is_some());
    let assoc = v.assoc.expect("assoc");
    assert_eq!(assoc.root_count, 1);
    assert_eq!(format::parse(&bytes).unwrap().entries.len(), 4);
}

#[test]
fn assoc_without_index_sets_only_bit1() {
    let rows = vec![AssocRow {
        root: "今天".into(),
        suffix: "回家".into(),
        rank: 600,
    }];
    let bytes = serialize_with_extras(&sample(), false, &rows);
    let v = format::validate(&bytes).expect("只开 assoc 也要通过校验");
    assert_eq!(v.flags, 0b10);
    assert!(v.word_index.is_none());
    assert_eq!(v.assoc.expect("assoc").root_count, 1);
    assert_eq!(format::parse(&bytes).unwrap().entries.len(), 4);
}

#[test]
fn unknown_flag_bit_is_rejected() {
    let mut bytes = serialize_with_extras(&sample(), true, &[]);
    // flags 在 offset 6，不在 checksum 覆盖的 [HEADER_LEN..tail) 内，改它不用重算 FNV。
    bytes[6] |= 0b100;
    assert!(matches!(
        format::validate(&bytes).unwrap_err(),
        FormatError::BadFlags(0b100)
    ));
}

#[test]
fn word_index_is_sorted_by_word_bytes() {
    let bytes = serialize_with_extras(&sample(), true, &[]);
    let v = format::validate(&bytes).unwrap();
    let start = v.word_index.expect("词索引起始偏移");
    let dict = format::parse(&bytes).unwrap();
    let words: Vec<&str> = (0..v.count)
        .map(|i| {
            let off = start + i * 4;
            let idx = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
            dict.entries[idx].word.as_str()
        })
        .collect();
    assert!(
        words.windows(2).all(|w| w[0] <= w[1]),
        "词索引必须按词字节序非降（Spec §9.2）：{words:?}"
    );
}

#[test]
fn assoc_root_out_of_order_rejected() {
    let rows = vec![
        AssocRow {
            root: "今天".into(),
            suffix: "回家".into(),
            rank: 600,
        },
        AssocRow {
            root: "中国".into(),
            suffix: "人民".into(),
            rank: 500,
        },
    ];
    let mut bytes = serialize_with_extras(&sample(), true, &rows);
    let v = format::validate(&bytes).unwrap();
    // 两个 root 都是 6 字节；交换记录里的 root 字节，目录仍是原顺序。
    let a = v.assoc.as_ref().unwrap().start + dir_root_off(&bytes, &v, 0);
    let b = v.assoc.as_ref().unwrap().start + dir_root_off(&bytes, &v, 1);
    for i in 0..6 {
        bytes.swap(a + i, b + i);
    }
    reseal(&mut bytes);
    assert!(format::validate(&bytes).is_err());
}

#[test]
fn assoc_root_offset_gap_rejected() {
    let rows = vec![AssocRow {
        root: "今天".into(),
        suffix: "回家".into(),
        rank: 600,
    }];
    let mut bytes = serialize_with_extras(&sample(), true, &rows);
    let v = format::validate(&bytes).unwrap();
    let dir = v.assoc.as_ref().unwrap().start + 4;
    let off = u32::from_le_bytes(bytes[dir..dir + 4].try_into().unwrap());
    bytes[dir..dir + 4].copy_from_slice(&(off + 1).to_le_bytes());
    reseal(&mut bytes);
    assert!(format::validate(&bytes).is_err());
}

#[test]
fn assoc_last_record_off_end_rejected() {
    let rows = vec![AssocRow {
        root: "今天".into(),
        suffix: "回家".into(),
        rank: 600,
    }];
    let mut bytes = serialize_with_extras(&sample(), true, &rows);
    let v = format::validate(&bytes).unwrap();
    let records_start = v.assoc.as_ref().unwrap().records_start;
    let suffix_len = records_start + "今天".len();
    bytes[suffix_len] += 1;
    reseal(&mut bytes);
    assert!(format::validate(&bytes).is_err());
}

#[test]
fn assoc_len_mismatch_rejected() {
    let rows = vec![AssocRow {
        root: "今天".into(),
        suffix: "回家".into(),
        rank: 600,
    }];
    let mut bytes = serialize_with_extras(&sample(), true, &rows);
    let pos = assoc_field_pos(&bytes);
    let n = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
    bytes[pos..pos + 4].copy_from_slice(&(n + 1).to_le_bytes());
    reseal(&mut bytes);
    assert!(format::validate(&bytes).is_err());
}

#[test]
fn word_index_out_of_range_rejected() {
    let mut bytes = serialize_with_extras(&sample(), true, &[]);
    let v = format::validate(&bytes).unwrap();
    let start = v.word_index.expect("词索引");
    let out_of_range = u32::try_from(v.count).unwrap();
    bytes[start..start + 4].copy_from_slice(&out_of_range.to_le_bytes());
    reseal(&mut bytes);
    assert!(format::validate(&bytes).is_err());
}

#[test]
fn truncated_index_rejected() {
    let mut bytes = serialize_with_extras(&sample(), true, &[]);
    let tail = bytes.len() - TRAILER_LEN;
    bytes.remove(tail - 1);
    reseal(&mut bytes);
    assert!(format::validate(&bytes).is_err());
}

#[test]
fn oversized_tail_lengths_do_not_panic() {
    // count 巨大：table_end 的 checked 算术必须先拒绝。
    let mut count_bytes = serialize_with_extras(&sample(), false, &[]);
    count_bytes[7..11].copy_from_slice(&u32::MAX.to_le_bytes());
    reseal(&mut count_bytes);
    assert!(format::validate(&count_bytes).is_err());

    // assoc_len 巨大：assoc_start = assoc_end - assoc_len 必须走 checked。
    let rows = vec![AssocRow {
        root: "今天".into(),
        suffix: "回家".into(),
        rank: 600,
    }];
    let mut assoc_len_bytes = serialize_with_extras(&sample(), true, &rows);
    let pos = assoc_field_pos(&assoc_len_bytes);
    assoc_len_bytes[pos..pos + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    reseal(&mut assoc_len_bytes);
    assert!(format::validate(&assoc_len_bytes).is_err());

    // root_off 巨大：目录偏移的 checked 加法/边界检查必须拒绝。
    let mut root_off_bytes = serialize_with_extras(&sample(), true, &rows);
    let v = format::validate(&root_off_bytes).unwrap();
    let dir = v.assoc.as_ref().unwrap().start + 4;
    root_off_bytes[dir..dir + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    reseal(&mut root_off_bytes);
    assert!(format::validate(&root_off_bytes).is_err());
}
