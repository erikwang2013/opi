// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 词排序索引查询（Task 2）。索引项是 entry 下标，按词字节序排列；
//! 同词按 freq 降序、entry 下标升序。查询结果不去重。

use super::MmapDictionary;

#[allow(dead_code)] // Task 4 把它接进 Dictionary trait 后移除
impl MmapDictionary {
    pub(super) fn words_with_prefix_impl(&self, prefix: &str) -> Vec<(String, u32)> {
        if prefix.is_empty() || self.count == 0 {
            return Vec::new();
        }
        let Some(index_start) = self.word_index_start else {
            return Vec::new();
        };
        let data = self.data();
        let word_start = self.word_start();
        let needle = prefix.as_bytes();

        // 索引已按词字节序排列；在索引上做 lower_bound，比较的是被指向条目的词。
        let mut lo = 0usize;
        let mut hi = self.count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let idx = self.index_entry(data, index_start, mid);
            if self.entry_word(data, word_start, idx) < needle {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        let mut out = Vec::new();
        for pos in lo..self.count {
            let idx = self.index_entry(data, index_start, pos);
            let (_, _, wo, wl, freq) = self.record(data, idx);
            let word = &data[word_start + wo..word_start + wo + wl];
            if !word.starts_with(needle) {
                break;
            }
            out.push((
                std::str::from_utf8(word)
                    .expect("validate 已校验 UTF-8")
                    .to_string(),
                freq,
            ));
        }
        out
    }

    /// 读词索引第 `pos` 项（validate 已校验边界与 `< count`）。
    fn index_entry(&self, data: &[u8], start: usize, pos: usize) -> usize {
        let off = start + pos * 4;
        u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize
    }

    fn entry_word<'a>(&self, data: &'a [u8], word_start: usize, idx: usize) -> &'a [u8] {
        let (_, _, wo, wl, _) = self.record(data, idx);
        &data[word_start + wo..word_start + wo + wl]
    }
}

#[cfg(test)]
mod tests {
    use super::super::load_bytes;
    use crate::format::{OpDict, RawEntry, serialize};
    use crate::format_assoc::serialize_with_extras;

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
    fn prefix_query_returns_word_and_weight_in_word_order() {
        let bytes = serialize_with_extras(&sample(), true, &[]);
        let d = load_bytes(bytes).expect("load");
        // sample() 里「今天」出现两次（900/800）：索引按 (词, freq 降序, entry 下标) 排，
        // 查询结果**不去重**，去重是联想层的事（Spec §3.3）。
        assert_eq!(
            d.words_with_prefix_impl("今天"),
            vec![
                ("今天".to_string(), 900),
                ("今天".to_string(), 800),
                ("今天晚上".to_string(), 700),
            ]
        );
    }

    #[test]
    fn empty_prefix_and_missing_index_return_empty() {
        let d = load_bytes(serialize_with_extras(&sample(), true, &[])).unwrap();
        assert!(d.words_with_prefix_impl("").is_empty());
        let plain = load_bytes(serialize(&sample())).unwrap();
        assert!(plain.words_with_prefix_impl("今天").is_empty());
    }

    #[test]
    fn no_match_returns_empty() {
        let d = load_bytes(serialize_with_extras(&sample(), true, &[])).unwrap();
        assert!(d.words_with_prefix_impl("不存在").is_empty());
    }

    #[test]
    fn full_hit_prefix_returns_every_matching_entry() {
        let d = load_bytes(serialize_with_extras(&sample(), true, &[])).unwrap();
        assert_eq!(
            d.words_with_prefix_impl("今"),
            vec![
                ("今天".to_string(), 900),
                ("今天".to_string(), 800),
                ("今天晚上".to_string(), 700),
            ]
        );
    }

    #[test]
    fn same_word_multiple_readings_returns_multiple() {
        let dict = OpDict {
            entries: vec![
                RawEntry {
                    pinyin: "hang".into(),
                    word: "行".into(),
                    freq: 700,
                },
                RawEntry {
                    pinyin: "xing".into(),
                    word: "行".into(),
                    freq: 900,
                },
            ],
            pinyin_total: "hangxing".len(),
        };
        let d = load_bytes(serialize_with_extras(&dict, true, &[])).unwrap();
        assert_eq!(
            d.words_with_prefix_impl("行"),
            vec![("行".to_string(), 900), ("行".to_string(), 700)]
        );
    }

    #[test]
    fn empty_indexed_dictionary_returns_empty() {
        let empty = OpDict {
            entries: vec![],
            pinyin_total: 0,
        };
        let d = load_bytes(serialize_with_extras(&empty, true, &[])).unwrap();
        assert!(d.words_with_prefix_impl("今").is_empty());
    }
}
