//! .opid v1 二进制词库格式的 serialize/parse。
//!
//! 布局（全部 little-endian）：
//! - header 11B：`MAGIC` `b"OPID"`(4) + version u16=1(2) + flags u8=0(1) + count u32(4)
//! - entries 表 `[11, 11 + count*14)`：每条 14B =
//!   pinyin_blob_off u32 + pinyin_len u8 + word_blob_off u32 + word_len u8 + freq u32
//! - pinyin blob 紧随 entries 表；word blob 紧随 pinyin blob
//! - trailer 8B：FNV-1a64 覆盖 `[11, len-8)`
//!
//! pinyin_blob_off 相对 pinyin blob 起点，word_blob_off 相对 word blob 起点。
//! entries 必须按 pinyin 字节序升序。

use crate::checksum::fnv1a64;

pub const MAGIC: &[u8; 4] = b"OPID";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 11;
pub const ENTRY_LEN: usize = 14;
pub const TRAILER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    pub pinyin: String,
    pub word: String,
    pub freq: u32,
}

/// 排序后的词库（entries 按 pinyin 字节序升序）。pinyin_total 为 pinyin blob 字节数，
/// 由 compile/parse 维护，loader 据此定位 word blob。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpDict {
    pub entries: Vec<RawEntry>,
    pub pinyin_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    BadMagic,
    BadVersion(u16),
    Truncated,
    ChecksumMismatch { expected: u64, actual: u64 },
    Unsorted,
    BadOffsets,
}

/// 序列化为 .opid v1。条目先按 pinyin 字节序排序（不改动入参）。
/// 不变式：pinyin/word 均 ≤255 字节且为 ASCII pinyin（编译管线保证）。
pub fn serialize(dict: &OpDict) -> Vec<u8> {
    let mut sorted: Vec<&RawEntry> = dict.entries.iter().collect();
    sorted.sort_by(|a, b| a.pinyin.as_bytes().cmp(b.pinyin.as_bytes()));
    for e in &sorted {
        debug_assert!(e.pinyin.len() <= u8::MAX as usize);
        debug_assert!(e.word.len() <= u8::MAX as usize);
    }
    let mut pinyin_blob = Vec::new();
    let mut word_blob = Vec::new();
    let mut table = Vec::with_capacity(sorted.len() * ENTRY_LEN);
    for e in &sorted {
        let po = pinyin_blob.len() as u32;
        pinyin_blob.extend_from_slice(e.pinyin.as_bytes());
        let wo = word_blob.len() as u32;
        word_blob.extend_from_slice(e.word.as_bytes());
        table.extend_from_slice(&po.to_le_bytes());
        table.push(e.pinyin.len() as u8);
        table.extend_from_slice(&wo.to_le_bytes());
        table.push(e.word.len() as u8);
        table.extend_from_slice(&e.freq.to_le_bytes());
    }
    debug_assert_eq!(pinyin_blob.len(), dict.pinyin_total);
    let mut out = Vec::with_capacity(
        HEADER_LEN + table.len() + pinyin_blob.len() + word_blob.len() + TRAILER_LEN,
    );
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(0); // flags
    out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&pinyin_blob);
    out.extend_from_slice(&word_blob);
    let sum = fnv1a64(&out[HEADER_LEN..]);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

/// 解析并完整校验 .opid v1。
pub fn parse(data: &[u8]) -> Result<OpDict, FormatError> {
    if data.len() < HEADER_LEN + TRAILER_LEN {
        return Err(FormatError::Truncated);
    }
    if &data[0..4] != MAGIC {
        return Err(FormatError::BadMagic);
    }
    let version = u16::from_le_bytes(data[4..6].try_into().unwrap());
    if version != VERSION {
        return Err(FormatError::BadVersion(version));
    }
    let tail = data.len() - TRAILER_LEN;
    let expected = u64::from_le_bytes(data[tail..].try_into().unwrap());
    let actual = fnv1a64(&data[HEADER_LEN..tail]);
    if expected != actual {
        return Err(FormatError::ChecksumMismatch { expected, actual });
    }
    let count = u32::from_le_bytes(data[7..11].try_into().unwrap()) as usize;
    let table_end = HEADER_LEN + count * ENTRY_LEN;
    if table_end > tail {
        return Err(FormatError::Truncated);
    }
    let pinyin_start = table_end;
    let mut rows: Vec<[u8; ENTRY_LEN]> = Vec::with_capacity(count);
    for i in 0..count {
        let rec = HEADER_LEN + i * ENTRY_LEN;
        rows.push(data[rec..rec + ENTRY_LEN].try_into().unwrap());
    }
    // pinyin_total = 各条 pinyin 终点最大值；word blob 紧随其后
    let pinyin_total = rows
        .iter()
        .map(|row| {
            let po = u32::from_le_bytes(row[0..4].try_into().unwrap()) as usize;
            let pl = row[4] as usize;
            po + pl
        })
        .max()
        .unwrap_or(0);
    let word_start = pinyin_start + pinyin_total;
    if word_start > tail {
        return Err(FormatError::BadOffsets);
    }
    let word_total = tail - word_start;
    let mut entries = Vec::with_capacity(count);
    let mut prev: Option<String> = None;
    for row in rows {
        let po = u32::from_le_bytes(row[0..4].try_into().unwrap()) as usize;
        let pl = row[4] as usize;
        let wo = u32::from_le_bytes(row[5..9].try_into().unwrap()) as usize;
        let wl = row[9] as usize;
        let freq = u32::from_le_bytes(row[10..14].try_into().unwrap());
        if pl == 0 || wl == 0 || po + pl > pinyin_total || wo + wl > word_total {
            return Err(FormatError::BadOffsets);
        }
        let pinyin = std::str::from_utf8(&data[pinyin_start + po..pinyin_start + po + pl])
            .map_err(|_| FormatError::BadOffsets)?;
        let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
            .map_err(|_| FormatError::BadOffsets)?;
        if let Some(p) = &prev
            && p.as_bytes() > pinyin.as_bytes()
        {
            return Err(FormatError::Unsorted);
        }
        prev = Some(pinyin.to_string());
        entries.push(RawEntry { pinyin: pinyin.to_string(), word: word.to_string(), freq });
    }
    Ok(OpDict { entries, pinyin_total })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> OpDict {
        OpDict {
            entries: vec![
                RawEntry { pinyin: "hao".into(), word: "好".into(), freq: 5000 },
                RawEntry { pinyin: "hao".into(), word: "号".into(), freq: 1200 },
                RawEntry { pinyin: "xiao".into(), word: "笑".into(), freq: 3000 },
            ],
            // pinyin blob = "hao"(3) + "hao"(3) + "xiao"(4) = 10 字节
            pinyin_total: 10,
        }
    }

    #[test]
    fn layout_byte_by_byte() {
        let bytes = serialize(&sample());
        assert_eq!(&bytes[0..4], b"OPID");
        assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), 1);
        assert_eq!(bytes[6], 0);
        assert_eq!(u32::from_le_bytes(bytes[7..11].try_into().unwrap()), 3);
        // 第一条记录：hao/好/5000
        assert_eq!(u32::from_le_bytes(bytes[11..15].try_into().unwrap()), 0); // pinyin_blob_off
        assert_eq!(bytes[15], 3); // pinyin_len
        assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 0); // word_blob_off
        assert_eq!(bytes[20], 3); // word_len
        assert_eq!(u32::from_le_bytes(bytes[21..25].try_into().unwrap()), 5000);
        // pinyin blob 紧跟 entries 表（11 + 3*14 = 53）
        assert_eq!(&bytes[53..56], b"hao");
        assert_eq!(&bytes[56..59], b"hao");
        assert_eq!(&bytes[59..63], b"xiao");
        // trailer 校验和覆盖 [11, len-8)
        let expected = crate::checksum::fnv1a64(&bytes[11..bytes.len() - 8]);
        assert_eq!(u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap()), expected);
    }

    #[test]
    fn roundtrip_restores_entries_sorted() {
        let parsed = parse(&serialize(&sample())).unwrap();
        assert_eq!(parsed.entries.len(), 3);
        assert_eq!(parsed.entries[0].word, "好");
        assert_eq!(parsed.entries[1].word, "号");
        assert_eq!(parsed.entries[2].word, "笑");
        assert_eq!(parsed.entries[2].freq, 3000);
        assert_eq!(parsed.pinyin_total, 10);
    }

    #[test]
    fn empty_dict_roundtrips() {
        let d = OpDict { entries: vec![], pinyin_total: 0 };
        let parsed = parse(&serialize(&d)).unwrap();
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.pinyin_total, 0);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = serialize(&sample());
        bytes[0] = b'X';
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::BadMagic);
    }

    #[test]
    fn bad_version_rejected() {
        let mut bytes = serialize(&sample());
        bytes[4] = 2;
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::BadVersion(2));
    }

    #[test]
    fn truncated_rejected() {
        let bytes = serialize(&sample());
        assert_eq!(parse(&bytes[..10]).unwrap_err(), FormatError::Truncated);
    }

    #[test]
    fn payload_corruption_detected() {
        let mut bytes = serialize(&sample());
        bytes[20] ^= 0xFF;
        assert!(matches!(parse(&bytes), Err(FormatError::ChecksumMismatch { .. })));
    }

    #[test]
    fn checksummed_but_unsorted_rejected() {
        // 交换第 0 与第 2 条记录（hao ↔ xiao）并重算校验和，格式仍应拒绝无序表
        let mut bytes = serialize(&sample());
        for i in 0..ENTRY_LEN {
            bytes.swap(11 + i, 11 + 2 * ENTRY_LEN + i);
        }
        let sum = crate::checksum::fnv1a64(&bytes[11..bytes.len() - 8]);
        let tail = bytes.len() - TRAILER_LEN;
        bytes[tail..].copy_from_slice(&sum.to_le_bytes());
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::Unsorted);
    }

    #[test]
    fn out_of_bounds_offset_rejected() {
        let mut bytes = serialize(&sample());
        bytes[11] = 0xFF; // 第一条 pinyin_blob_off 越界（校验和重算后仍应被拒绝）
        let sum = crate::checksum::fnv1a64(&bytes[11..bytes.len() - 8]);
        let tail = bytes.len() - TRAILER_LEN;
        bytes[tail..].copy_from_slice(&sum.to_le_bytes());
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::BadOffsets);
    }
}
