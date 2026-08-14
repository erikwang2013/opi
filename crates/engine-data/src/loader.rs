//! mmap 加载器：将 .opid v1 映射到只读内存，实现 engine_core 的 Dictionary。
//! 布局恢复只依赖 parse 校验过的 count 与 pinyin_total（三个区段边界）。

use std::path::Path;
use engine_core::dictionary::Dictionary;
use engine_core::Entry;
use crate::format::{parse, ENTRY_LEN, HEADER_LEN, OpDict};

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

/// 布局完全由 parse 校验过的参数恢复：count/pinyin_total 决定三个区段边界。
pub struct MmapDictionary {
    backing: Backing,
    count: usize,
    pinyin_total: usize,
    max_freq: u32,
}

impl MmapDictionary {
    fn from_parsed(backing: Backing, parsed: OpDict) -> Self {
        // 一次性扫描 freq 列（加载期 ~120k 条，毫秒级），供学习权重动态缩放。
        let data = match &backing {
            Backing::Map(m) => m.as_ref(),
            Backing::Bytes(v) => v.as_slice(),
        };
        let mut max_freq: u32 = 0;
        for i in 0..parsed.entries.len() {
            let off = HEADER_LEN + i * ENTRY_LEN;
            let freq = u32::from_le_bytes(data[off + 10..off + 14].try_into().unwrap());
            max_freq = max_freq.max(freq);
        }
        MmapDictionary {
            backing,
            count: parsed.entries.len(),
            pinyin_total: parsed.pinyin_total,
            max_freq,
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

    /// 读第 i 条记录（parse 已校验，可安全 unwrap）。
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
        if pinyin.is_empty() || self.count == 0 {
            return Vec::new();
        }
        let data = self.data();
        let needle = pinyin.as_bytes();
        let lo = lower_bound(data, self.table_start(), self.pinyin_start(), self.count, needle);
        let hi = match byte_successor(needle) {
            Some(succ) => lower_bound(data, self.table_start(), self.pinyin_start(), self.count, &succ),
            None => self.count,
        };
        let mut out: Vec<Entry> = Vec::new();
        let word_start = self.word_start();
        for i in lo..hi {
            let (_, _, wo, wl, freq) = self.record(data, i);
            let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                .expect("parse 已校验 UTF-8")
                .to_string();
            out.push(Entry { word, freq });
        }
        out.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.word.as_bytes().cmp(b.word.as_bytes())));
        out.truncate(limit);
        out
    }

    fn len(&self) -> usize {
        self.count
    }

    fn max_freq(&self) -> u64 {
        self.max_freq as u64
    }
}

/// 第 i 条记录的 pinyin 字节。
fn entry_pinyin(data: &[u8], table_start: usize, pinyin_start: usize, i: usize) -> &[u8] {
    let off = table_start + i * ENTRY_LEN;
    let po = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
    let pl = data[off + 4] as usize;
    &data[pinyin_start + po..pinyin_start + po + pl]
}

/// 前缀区间下界：第一个 pinyin >= needle 的下标。
fn lower_bound(data: &[u8], table_start: usize, pinyin_start: usize, count: usize, needle: &[u8]) -> usize {
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if entry_pinyin(data, table_start, pinyin_start, mid) < needle {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// 字节后继：末字节 +1（带进位）。全 0xFF 返回 None（上界 = 表尾）。
fn byte_successor(p: &[u8]) -> Option<Vec<u8>> {
    let mut b = p.to_vec();
    let mut i = b.len();
    while i > 0 {
        i -= 1;
        let (nb, overflow) = b[i].overflowing_add(1);
        b[i] = nb;
        if !overflow {
            return Some(b);
        }
    }
    None
}

/// 打开文件并以只读 mmap 加载 + 校验。
pub fn load_mmap(path: &Path) -> Result<MmapDictionary, LoadError> {
    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    // safety: 只读共享映射，解析先于任何读取（parse 全量校验），映射生命周期随 self。
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(LoadError::Io)?;
    let parsed = parse(&mmap).map_err(LoadError::Format)?;
    Ok(MmapDictionary::from_parsed(Backing::Map(mmap), parsed))
}

/// 从堆上字节加载（测试与内嵌 fallback 用）。
pub fn load_bytes(bytes: Vec<u8>) -> Result<MmapDictionary, LoadError> {
    let parsed = parse(&bytes).map_err(LoadError::Format)?;
    Ok(MmapDictionary::from_parsed(Backing::Bytes(bytes), parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{serialize, FormatError, OpDict, RawEntry};

    fn sample() -> Vec<u8> {
        serialize(&OpDict {
            entries: vec![
                RawEntry { pinyin: "hao".into(), word: "好".into(), freq: 5000 },
                RawEntry { pinyin: "hao".into(), word: "号".into(), freq: 1200 },
                RawEntry { pinyin: "xiao".into(), word: "笑".into(), freq: 3000 },
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
