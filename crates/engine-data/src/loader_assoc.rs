// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语义联想小表的二分查询（Task 3）。目录按 root 字节严格升序，
//! 命中后按 `(rank desc, len asc, bytes asc)` 返回后缀。

use super::MmapDictionary;
use crate::format_assoc::AssocView;

#[allow(dead_code)] // Task 4 把它接进 Dictionary trait 后移除
impl MmapDictionary {
    pub(super) fn association_suffixes_impl(&self, root: &str) -> Vec<(String, u32)> {
        if root.is_empty() {
            return Vec::new();
        }
        let Some(assoc) = self.assoc else {
            return Vec::new();
        };
        if assoc.root_count == 0 {
            return Vec::new();
        }
        let data = self.data();
        let needle = root.as_bytes();

        let mut lo = 0usize;
        let mut hi = assoc.root_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.assoc_root(data, &assoc, mid) < needle {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo >= assoc.root_count {
            return Vec::new();
        }

        let dir = assoc.start + 4 + lo * 6;
        let root_off = u32::from_le_bytes(data[dir..dir + 4].try_into().unwrap()) as usize;
        let suffix_count = data[dir + 4] as usize;
        let root_len = data[dir + 5] as usize;
        let root_start = assoc.start + root_off;
        if &data[root_start..root_start + root_len] != needle {
            return Vec::new();
        }

        let mut pos = root_start + root_len;
        let mut out = Vec::with_capacity(suffix_count);
        for _ in 0..suffix_count {
            let len = data[pos] as usize;
            pos += 1;
            let suffix = std::str::from_utf8(&data[pos..pos + len])
                .expect("validate 已校验 UTF-8")
                .to_string();
            pos += len;
            let rank = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            pos += 4;
            out.push((suffix, rank));
        }
        out.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(a.0.len().cmp(&b.0.len()))
                .then(a.0.as_bytes().cmp(b.0.as_bytes()))
        });
        out
    }

    fn assoc_root<'a>(&self, data: &'a [u8], assoc: &AssocView, i: usize) -> &'a [u8] {
        let dir = assoc.start + 4 + i * 6;
        let root_off = u32::from_le_bytes(data[dir..dir + 4].try_into().unwrap()) as usize;
        let root_len = data[dir + 5] as usize;
        let start = assoc.start + root_off;
        &data[start..start + root_len]
    }
}

#[cfg(test)]
mod tests {
    use super::super::load_bytes;
    use crate::format::{OpDict, RawEntry, serialize};
    use crate::format_assoc::{AssocRow, serialize_with_extras};

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

    #[test]
    fn suffixes_are_sorted_by_rank_then_length_then_bytes() {
        let rows = vec![
            AssocRow {
                root: "今天".into(),
                suffix: "晚上".into(),
                rank: 10,
            },
            AssocRow {
                root: "今天".into(),
                suffix: "回家".into(),
                rank: 90,
            },
            AssocRow {
                root: "今天".into(),
                suffix: "下午茶".into(),
                rank: 10,
            },
            AssocRow {
                root: "中国".into(),
                suffix: "人民".into(),
                rank: 80,
            },
        ];
        let d = load_bytes(serialize_with_extras(&sample(), true, &rows)).unwrap();
        assert_eq!(
            d.association_suffixes_impl("今天"),
            vec![
                ("回家".to_string(), 90),
                ("晚上".to_string(), 10),
                ("下午茶".to_string(), 10),
            ]
        );
        assert!(
            d.association_suffixes_impl("今天")
                .iter()
                .all(|(s, _)| s != "人民")
        );
    }

    #[test]
    fn empty_root_missing_root_and_missing_table_return_empty() {
        let rows = vec![AssocRow {
            root: "今天".into(),
            suffix: "回家".into(),
            rank: 90,
        }];
        let d = load_bytes(serialize_with_extras(&sample(), true, &rows)).unwrap();
        assert!(d.association_suffixes_impl("").is_empty());
        assert!(d.association_suffixes_impl("不存在").is_empty());
        let plain = load_bytes(serialize(&sample())).unwrap();
        assert!(plain.association_suffixes_impl("今天").is_empty());
    }

    #[test]
    fn assoc_without_word_index_is_queryable() {
        let rows = vec![AssocRow {
            root: "今天".into(),
            suffix: "回家".into(),
            rank: 90,
        }];
        let d = load_bytes(serialize_with_extras(&sample(), false, &rows)).unwrap();
        assert_eq!(
            d.association_suffixes_impl("今天"),
            vec![("回家".to_string(), 90)]
        );
    }
}
