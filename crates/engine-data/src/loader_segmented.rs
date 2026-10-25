// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 分词输入（含 `'`）的按键类区间查询（2026-10-26 从 `loader.rs` 拆出：那边加
//! 这个方法与回归用例后越过 500 行门禁；同 `loader_lower_bound.rs` 的先例）。
//!
//! 旧路径：`input.contains('\'')` 时退回 `scheme_entries_scan`，逐条扫 93.7 万条
//! （桌面 ~60ms，手机秒级）。这里去掉分隔符做 `scheme_ranges`，再用
//! `Scheme::matches`（内部 `matches_segmented`）过滤音节边界；结果集/次序与旧扫描
//! 同（分词输入没有「精确」档，按静态词频取前 n）。

use super::*;

impl MmapDictionary {
    pub(super) fn scheme_entries_segmented(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        n: usize,
    ) -> Vec<(Entry, String)> {
        if n == 0 {
            return Vec::new();
        }
        let joined: String = input.chars().filter(|c| *c != '\'').collect();
        if joined.is_empty() {
            return Vec::new();
        }
        let data = self.data();
        let ps = self.pinyin_start();
        let word_start = self.word_start();
        let mut out: Vec<(Entry, String)> = Vec::new();
        for (lo, hi) in self.scheme_ranges(data, &joined, scheme) {
            for i in lo..hi {
                let (po, pl, wo, wl, freq) = self.record(data, i);
                let pinyin = std::str::from_utf8(&data[ps + po..ps + po + pl])
                    .expect("validate 已校验 UTF-8");
                if let Some(p) = pin
                    && pinyin != p
                {
                    continue;
                }
                if !scheme.matches(input, pinyin) {
                    continue;
                }
                let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                    .expect("validate 已校验 UTF-8")
                    .to_string();
                out.push((
                    Entry {
                        word,
                        freq,
                        pinyin_len: pl,
                    },
                    pinyin.to_string(),
                ));
            }
        }
        out.sort_by(|a, b| b.0.freq.cmp(&a.0.freq).then(a.0.word.cmp(&b.0.word)));
        out.truncate(n);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::format::{OpDict, RawEntry, serialize};
    use engine_core::dictionary::scheme_entries_scan;
    use engine_core::scheme::Scheme;

    #[test]
    fn segmented_entries_match_the_scan_fallback() {
        let d = load_bytes(serialize(&OpDict {
            entries: vec![
                RawEntry {
                    pinyin: "nian".into(),
                    word: "年".into(),
                    freq: 5000,
                },
                RawEntry {
                    pinyin: "nihao".into(),
                    word: "你好".into(),
                    freq: 6000,
                },
                RawEntry {
                    pinyin: "hao".into(),
                    word: "好".into(),
                    freq: 4000,
                },
            ],
            pinyin_total: 12,
        }))
        .unwrap();
        let got = d.scheme_entries("64'26", Scheme::Nine, None, 2048);
        let scan = scheme_entries_scan(&d, "64'26", Scheme::Nine, None, 2048);
        assert_eq!(
            got.iter()
                .map(|(e, p)| (e.word.as_str(), p.as_str()))
                .collect::<Vec<_>>(),
            scan.iter()
                .map(|(e, p)| (e.word.as_str(), p.as_str()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            got.iter().map(|(e, _)| e.word.as_str()).collect::<Vec<_>>(),
            ["年"]
        );
    }
}
