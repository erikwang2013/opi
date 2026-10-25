// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! mmap 加载器：将 .opid v1 映射到只读内存，实现 engine_core 的 Dictionary。
//! 布局恢复只依赖 `validate` 校验过的 count / pinyin_total / max_freq（三个区段边界），
//! **不物化任何条目** —— 冷启动只读文件与三条列。

use self::loader_lower_bound::lower_bound;
use crate::format::{ENTRY_LEN, HEADER_LEN, Validated, validate};
use engine_core::Entry;
use engine_core::bytes::byte_successor;
use engine_core::dictionary::{Dictionary, scheme_entries_scan, scheme_find_scan};
use engine_core::scheme::Scheme;
use std::path::Path;

#[path = "loader_lower_bound.rs"]
mod loader_lower_bound;
#[path = "loader_segmented.rs"]
mod loader_segmented;
/// 内存后盾：mmap 文件或堆上字节，避免自引用结构。
pub enum Backing {
    Map(memmap2::Mmap),
    Bytes(Vec<u8>),
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Format(crate::format::FormatError),
}

/// 布局完全由 validate 校验过的参数恢复：count/pinyin_total 决定三个区段边界。
pub struct MmapDictionary {
    backing: Backing,
    count: usize,
    pinyin_total: usize,
    max_freq: u32,
}

impl MmapDictionary {
    /// max_freq 由 `validate` 顺带测出（它本来就要逐条读 freq 列）。
    fn from_validated(backing: Backing, v: Validated) -> Self {
        MmapDictionary {
            backing,
            count: v.count,
            pinyin_total: v.pinyin_total,
            max_freq: v.max_freq,
        }
    }

    fn data(&self) -> &[u8] {
        match &self.backing {
            Backing::Map(m) => m.as_ref(),
            Backing::Bytes(v) => v.as_slice(),
        }
    }

    fn table_start(&self) -> usize {
        HEADER_LEN
    }

    fn pinyin_start(&self) -> usize {
        HEADER_LEN + self.count * ENTRY_LEN
    }

    fn word_start(&self) -> usize {
        self.pinyin_start() + self.pinyin_total
    }

    /// 读第 i 条记录（validate 已校验，可安全 unwrap）。
    fn record(&self, data: &[u8], i: usize) -> (usize, usize, usize, usize, u32) {
        let off = self.table_start() + i * ENTRY_LEN;
        let po = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
        let pl = data[off + 4] as usize;
        let wo = u32::from_le_bytes(data[off + 5..off + 9].try_into().unwrap()) as usize;
        let wl = data[off + 9] as usize;
        let freq = u32::from_le_bytes(data[off + 10..off + 14].try_into().unwrap());
        (po, pl, wo, wl, freq)
    }
}

impl Dictionary for MmapDictionary {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        let mut out = self.scan(pinyin);
        out.sort_by(|a, b| {
            b.freq
                .cmp(&a.freq)
                .then(a.word.as_bytes().cmp(b.word.as_bytes()))
        });
        out.truncate(limit);
        out
    }

    /// 生产路径（`rank_and_pick` 全量收集后按权威「分数」整个重排）专用：
    /// 省掉一次没人要的词频排序 —— 3625 条命中实测 `query` 1.35ms → 本方法 0.48ms。
    fn query_all(&self, pinyin: &str) -> Vec<Entry> {
        self.scan(pinyin)
    }

    /// 前缀层 top-N 的**流式**实现（见 trait 文档）：第一遍只读 freq 列做 O(m) 选择
    /// （`select_nth_unstable`），第二遍只对「exact 全量 + 词频 ≥ 阈值的恰好 n 条」
    /// 做字符串物化 —— 被截掉的尾巴一次 String 分配都不发生（单字母 80k 命中的
    /// 24.6ms 物化由此降到 ~1ms 量级，实测见 perf 报告）。
    ///
    /// .opid 的 freq 由行位置编码、逐行唯一（`gen_luna_dict` 的 spacing），故阈值法
    /// 无并列项；若将来 freq 允许并列，n 边界上的取舍不保证确定 —— 语义无损：
    /// 调用方按分数整体重排，并列 freq 在可见名次上的先后本就任意。
    fn query_prefix_top(&self, prefix: &str, n: usize) -> Vec<Entry> {
        if prefix.is_empty() || self.count == 0 {
            return Vec::new();
        }
        let data = self.data();
        let needle = prefix.as_bytes();
        let plen = needle.len();
        let (lo, hi) = self.range(data, needle);
        // 第一遍：切开 exact / 非 exact（只读列，不物化）。
        let mut exact_idx: Vec<usize> = Vec::new();
        let mut rest: Vec<(u32, usize)> = Vec::with_capacity(hi - lo);
        for i in lo..hi {
            let (_, pl, _, _, freq) = self.record(data, i);
            if pl == plen {
                exact_idx.push(i);
            } else {
                rest.push((freq, i));
            }
        }
        let take = n.min(rest.len());
        if take < rest.len() {
            rest.select_nth_unstable_by(take, |a, b| b.0.cmp(&a.0));
            rest.truncate(take);
        }
        // 第二遍：只物化保留下来的条目。
        let word_start = self.word_start();
        let mut out: Vec<Entry> = Vec::with_capacity(exact_idx.len() + rest.len());
        for i in exact_idx
            .into_iter()
            .chain(rest.into_iter().map(|(_, i)| i))
        {
            let (_, pl, wo, wl, freq) = self.record(data, i);
            let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                .expect("validate 已校验 UTF-8")
                .to_string();
            out.push(Entry {
                word,
                freq,
                pinyin_len: pl,
            });
        }
        out
    }

    /// 按键类匹配（9/14 键）的整档，见 trait 文档（pin = **读音恰等**，2026-10-16 裁决）。
    /// 实现 = `scheme_ranges` 区间枚举 + 与 `query_prefix_top` 相同的「exact 全量 + 非
    /// exact top-n」取舍；`'` 分词输入交回退实现（matches_segmented 按段长比）。
    fn scheme_entries(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        n: usize,
    ) -> Vec<(Entry, String)> {
        if input.is_empty() {
            return scheme_entries_scan(self, input, scheme, pin, n);
        }
        // 2026-10-26：分词输入改为去引号区间枚举 + 边界过滤，不再全表扫描。
        if input.contains('\'') {
            return self.scheme_entries_segmented(input, scheme, pin, n);
        }
        let data = self.data();
        let ps = self.pinyin_start();
        let plen = input.len();
        let mut exact_idx: Vec<usize> = Vec::new();
        let mut rest: Vec<(u32, usize)> = Vec::new();
        for (lo, hi) in self.scheme_ranges(data, input, scheme) {
            for i in lo..hi {
                let (po, pl, _, _, freq) = self.record(data, i);
                if let Some(p) = pin
                    && &data[ps + po..ps + po + pl] != p.as_bytes()
                {
                    continue;
                }
                if pl == plen {
                    exact_idx.push(i);
                } else {
                    rest.push((freq, i));
                }
            }
        }
        let take = n.min(rest.len());
        if take < rest.len() {
            rest.select_nth_unstable_by(take, |a, b| b.0.cmp(&a.0));
            rest.truncate(take);
        }
        let word_start = self.word_start();
        let mut out: Vec<(Entry, String)> = Vec::with_capacity(exact_idx.len() + rest.len());
        for i in exact_idx
            .into_iter()
            .chain(rest.into_iter().map(|(_, i)| i))
        {
            let (po, pl, wo, wl, freq) = self.record(data, i);
            let pinyin = std::str::from_utf8(&data[ps + po..ps + po + pl])
                .expect("validate 已校验 UTF-8")
                .to_string();
            let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                .expect("validate 已校验 UTF-8")
                .to_string();
            out.push((
                Entry {
                    word,
                    freq,
                    pinyin_len: pl,
                },
                pinyin,
            ));
        }
        out
    }

    /// 按键类匹配的存在性/见证者查询，见 trait 文档（pin 同 `scheme_entries`；区间枚举+早停；`'` 交回退）。
    fn scheme_find(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        pred: &mut dyn FnMut(&str, &str) -> bool,
    ) -> Option<usize> {
        if input.is_empty() || input.contains('\'') {
            return scheme_find_scan(self, input, scheme, pin, pred);
        }
        let data = self.data();
        let ps = self.pinyin_start();
        let word_start = self.word_start();
        for (lo, hi) in self.scheme_ranges(data, input, scheme) {
            for i in lo..hi {
                let (po, pl, wo, wl, _f) = self.record(data, i);
                let pinyin = std::str::from_utf8(&data[ps + po..ps + po + pl])
                    .expect("validate 已校验 UTF-8");
                if let Some(p) = pin
                    && pinyin != p
                {
                    continue;
                }
                let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                    .expect("validate 已校验 UTF-8");
                if pred(pinyin, word) {
                    return Some(pl);
                }
            }
        }
        None
    }

    /// 全量遍历（9/14 键方案的按键类展开专用，见 engine-core 的
    /// `Dictionary::visit_entries`）。表序扫描 —— blob 序即 pinyin 字节序，
    /// 零拷贝借用；按**行**回调而不先收集，省掉一次约 100k 条目的分配。
    fn visit_entries(&self, f: &mut dyn FnMut(&str, &str, u32)) {
        let data = self.data();
        let word_start = self.word_start();
        let pinyin_start = self.pinyin_start();
        for i in 0..self.count {
            let (po, pl, wo, wl, freq) = self.record(data, i);
            let pinyin = std::str::from_utf8(&data[pinyin_start + po..pinyin_start + po + pl])
                .expect("validate 已校验 UTF-8");
            let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                .expect("validate 已校验 UTF-8");
            f(pinyin, word, freq);
        }
    }

    /// 只扫拼音以 `prefix` 开头的区间（声母缩写匹配用，见 trait 文档）。
    /// 与 `visit_entries` 同一趟零拷贝读取，只是先用二分把区间收窄。
    fn visit_prefix(&self, prefix: &str, f: &mut dyn FnMut(&str, &str, u32)) {
        if prefix.is_empty() || self.count == 0 {
            return;
        }
        let data = self.data();
        let (lo, hi) = self.range(data, prefix.as_bytes());
        let word_start = self.word_start();
        let pinyin_start = self.pinyin_start();
        for i in lo..hi {
            let (po, pl, wo, wl, freq) = self.record(data, i);
            let pinyin = std::str::from_utf8(&data[pinyin_start + po..pinyin_start + po + pl])
                .expect("validate 已校验 UTF-8");
            let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                .expect("validate 已校验 UTF-8");
            f(pinyin, word, freq);
        }
    }

    fn len(&self) -> usize {
        self.count
    }

    fn max_freq(&self) -> u64 {
        self.max_freq as u64
    }
}

impl MmapDictionary {
    /// 前缀命中的全部条目，**未排序**（blob 序 = pinyin 字节序）。
    fn scan(&self, pinyin: &str) -> Vec<Entry> {
        if pinyin.is_empty() || self.count == 0 {
            return Vec::new();
        }
        let data = self.data();
        let (lo, hi) = self.range(data, pinyin.as_bytes());
        let mut out: Vec<Entry> = Vec::new();
        let word_start = self.word_start();
        for i in lo..hi {
            let (_, pl, wo, wl, freq) = self.record(data, i);
            let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                .expect("validate 已校验 UTF-8")
                .to_string();
            out.push(Entry {
                word,
                freq,
                pinyin_len: pl,
            });
        }
        out
    }

    /// 前缀 `needle` 的 `[lo, hi)` 条目区间（blob 序 = pinyin 字节序）。
    fn range(&self, data: &[u8], needle: &[u8]) -> (usize, usize) {
        let ts = self.table_start();
        let ps = self.pinyin_start();
        let lo = lower_bound(data, ts, ps, self.count, needle);
        let hi = match byte_successor(needle) {
            Some(succ) => lower_bound(data, ts, ps, self.count, &succ),
            None => self.count,
        };
        (lo, hi)
    }

    /// 按键类输入命中的**全部拼音区间**（未物化）：表按字节序排 ⇒ 每位按键类的每个
    /// 字母把「当前前缀」延长一字节、仍是一个连续区间（`lower_bound` ∈ `byte_successor`）
    /// —— 逐位枚举 + 空区间剪枝，取代全表逐条 `matches`。非法键 → 空。
    fn scheme_ranges(&self, data: &[u8], input: &str, scheme: Scheme) -> Vec<(usize, usize)> {
        let mut level: Vec<(String, usize, usize)> = vec![(String::new(), 0, self.count)];
        for ch in input.chars() {
            let Some(class) = scheme.key_class(ch) else {
                return Vec::new();
            };
            let mut next: Vec<(String, usize, usize)> = Vec::new();
            for (pfx, _lo, _hi) in &level {
                for &letter in class {
                    let mut np = pfx.clone();
                    np.push(letter);
                    let (lo, hi) = self.range(data, np.as_bytes());
                    if lo < hi {
                        next.push((np, lo, hi));
                    }
                }
            }
            if next.is_empty() {
                return Vec::new();
            }
            level = next;
        }
        level.into_iter().map(|(_, lo, hi)| (lo, hi)).collect()
    }
}

/// 打开文件并以只读 mmap 加载 + 校验。
pub fn load_mmap(path: &Path) -> Result<MmapDictionary, LoadError> {
    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    // safety: 只读共享映射，全量校验先于任何读取（validate），映射生命周期随 self。
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(LoadError::Io)?;
    let v = validate(&mmap).map_err(LoadError::Format)?;
    Ok(MmapDictionary::from_validated(Backing::Map(mmap), v))
}

/// 从堆上字节加载（测试与内嵌 fallback 用）。
pub fn load_bytes(bytes: Vec<u8>) -> Result<MmapDictionary, LoadError> {
    let v = validate(&bytes).map_err(LoadError::Format)?;
    Ok(MmapDictionary::from_validated(Backing::Bytes(bytes), v))
}

#[cfg(test)]
mod tests {
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
}
