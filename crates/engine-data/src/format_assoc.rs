// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! .opid v1 的可选尾部：词排序索引（flags bit0）与语义小表（flags bit1）。
//!
//! 布局见 `docs/superpowers/specs/2026-10-27-smart-association-design.md` §3.2：
//! `word blob` 之后依次是 `word_index`（count × u32）、`assoc`、`assoc_len u32`，
//! 最后是原有 8B trailer。两个 bit 相互独立。

use crate::checksum::fnv1a64;
use crate::format::{FormatError, HEADER_LEN, MAGIC, OpDict, TRAILER_LEN, VERSION, build_parts};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocRow {
    pub root: String,
    pub suffix: String,
    pub rank: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssocView {
    pub start: usize,
    pub len: usize,
    pub root_count: usize,
    pub records_start: usize,
}

pub(crate) struct TailInfo {
    pub word_end: usize,
    pub word_index: Option<usize>,
    pub assoc: Option<AssocView>,
}

/// 写带可选尾部的 v1 文件。两个 bit 按存在性独立置位：
/// `word_index=false` 且 `assoc_rows` 为空时与 `serialize` 逐字节相同。
pub fn serialize_with_extras(dict: &OpDict, word_index: bool, assoc_rows: &[AssocRow]) -> Vec<u8> {
    if !word_index && assoc_rows.is_empty() {
        return crate::format::serialize(dict);
    }
    let (table, pinyin_blob, word_blob, sorted) = build_parts(dict);
    let mut flags = 0u8;
    if word_index {
        flags |= 0b01;
    }
    if !assoc_rows.is_empty() {
        flags |= 0b10;
    }

    let assoc = if assoc_rows.is_empty() {
        None
    } else {
        Some(build_assoc(assoc_rows))
    };
    let mut out = Vec::with_capacity(
        HEADER_LEN
            + table.len()
            + pinyin_blob.len()
            + word_blob.len()
            + if word_index { sorted.len() * 4 } else { 0 }
            + assoc.as_ref().map_or(0, |a| a.len() + 4)
            + TRAILER_LEN,
    );
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(flags);
    out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&pinyin_blob);
    out.extend_from_slice(&word_blob);

    if word_index {
        let mut order: Vec<usize> = (0..sorted.len()).collect();
        order.sort_by(|&a, &b| {
            sorted[a]
                .word
                .as_bytes()
                .cmp(sorted[b].word.as_bytes())
                .then(sorted[b].freq.cmp(&sorted[a].freq))
                .then(a.cmp(&b))
        });
        debug_assert!(
            order
                .windows(2)
                .all(|w| sorted[w[0]].word.as_bytes() <= sorted[w[1]].word.as_bytes()),
            "词索引必须按词字节序非降"
        );
        for i in order {
            out.extend_from_slice(&(i as u32).to_le_bytes());
        }
    }

    if let Some(assoc) = assoc {
        let len = u32::try_from(assoc.len()).expect("assoc 段 > u32::MAX");
        out.extend_from_slice(&assoc);
        out.extend_from_slice(&len.to_le_bytes());
    }

    let sum = fnv1a64(&out[HEADER_LEN..]);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

/// 从 `tail` 反推尾部边界并做结构校验。调用前 `validate` 已校验 checksum/flags。
pub(crate) fn parse_tail(
    data: &[u8],
    count: usize,
    word_start: usize,
    tail: usize,
) -> Result<TailInfo, FormatError> {
    let flags = data[6];
    let mut index_start = tail;
    let mut assoc = None;
    if flags & 0b10 != 0 {
        let assoc_end = tail.checked_sub(4).ok_or(FormatError::Truncated)?;
        let assoc_len = u32::from_le_bytes(data[assoc_end..tail].try_into().unwrap()) as usize;
        if assoc_len < 4 {
            return Err(FormatError::BadOffsets);
        }
        let assoc_start = assoc_end
            .checked_sub(assoc_len)
            .ok_or(FormatError::BadOffsets)?;
        if word_start > assoc_start {
            return Err(FormatError::BadOffsets);
        }
        assoc = Some(parse_assoc(data, assoc_start, assoc_end)?);
        index_start = assoc_start;
    }

    let index_len = if flags & 0b01 != 0 {
        count.checked_mul(4).ok_or(FormatError::Truncated)?
    } else {
        0
    };
    let word_end = index_start
        .checked_sub(index_len)
        .ok_or(FormatError::BadOffsets)?;
    if word_start > word_end {
        return Err(FormatError::BadOffsets);
    }

    let word_index = if flags & 0b01 != 0 {
        for i in 0..count {
            let off = word_end + i * 4;
            let idx = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
            if idx >= count {
                return Err(FormatError::BadOffsets);
            }
        }
        Some(word_end)
    } else {
        None
    };

    Ok(TailInfo {
        word_end,
        word_index,
        assoc,
    })
}

fn build_assoc(rows: &[AssocRow]) -> Vec<u8> {
    let mut groups: BTreeMap<&str, Vec<&AssocRow>> = BTreeMap::new();
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for row in rows {
        assert!(!row.root.is_empty(), "assoc root 为空");
        assert!(!row.suffix.is_empty(), "assoc suffix 为空");
        assert!(
            seen.insert((row.root.as_str(), row.suffix.as_str())),
            "assoc 重复 root+suffix: {} + {}",
            row.root,
            row.suffix
        );
        groups.entry(row.root.as_str()).or_default().push(row);
    }
    let root_count = groups.len();
    let records_start = root_count
        .checked_mul(6)
        .and_then(|n| n.checked_add(4))
        .expect("assoc 目录过大");
    let mut directory = Vec::with_capacity(root_count * 6);
    let mut records = Vec::new();
    for (root, mut group) in groups {
        group.sort_by(|a, b| {
            b.rank
                .cmp(&a.rank)
                .then(a.suffix.len().cmp(&b.suffix.len()))
                .then(a.suffix.as_bytes().cmp(b.suffix.as_bytes()))
        });
        let root_off = records_start
            .checked_add(records.len())
            .expect("assoc 偏移溢出");
        let root_len = u8::try_from(root.len()).expect("assoc root > 255 bytes");
        let suffix_count = u8::try_from(group.len()).expect("assoc suffix 数 > 255");
        directory.extend_from_slice(
            &u32::try_from(root_off)
                .expect("assoc root_off > u32::MAX")
                .to_le_bytes(),
        );
        directory.push(suffix_count);
        directory.push(root_len);
        records.extend_from_slice(root.as_bytes());
        for row in group {
            let len = u8::try_from(row.suffix.len()).expect("assoc suffix > 255 bytes");
            records.push(len);
            records.extend_from_slice(row.suffix.as_bytes());
            records.extend_from_slice(&row.rank.to_le_bytes());
        }
    }

    let mut out = Vec::with_capacity(records_start + records.len());
    out.extend_from_slice(
        &u32::try_from(root_count)
            .expect("assoc root 数 > u32::MAX")
            .to_le_bytes(),
    );
    out.extend_from_slice(&directory);
    out.extend_from_slice(&records);
    out
}

fn parse_assoc(data: &[u8], start: usize, end: usize) -> Result<AssocView, FormatError> {
    if end < start || end - start < 4 {
        return Err(FormatError::BadOffsets);
    }
    let root_count = u32::from_le_bytes(data[start..start + 4].try_into().unwrap()) as usize;
    let dir_len = root_count.checked_mul(6).ok_or(FormatError::Truncated)?;
    let records_start = start
        .checked_add(4)
        .and_then(|n| n.checked_add(dir_len))
        .ok_or(FormatError::Truncated)?;
    if records_start > end {
        return Err(FormatError::BadOffsets);
    }

    let mut expected = records_start;
    let mut prev_root: Option<&str> = None;
    for i in 0..root_count {
        let dir = start + 4 + i * 6;
        let root_off = u32::from_le_bytes(data[dir..dir + 4].try_into().unwrap()) as usize;
        let suffix_count = data[dir + 4] as usize;
        let root_len = data[dir + 5] as usize;
        if root_len == 0 {
            return Err(FormatError::BadOffsets);
        }
        let abs = start.checked_add(root_off).ok_or(FormatError::BadOffsets)?;
        if abs != expected {
            return Err(FormatError::BadOffsets);
        }
        let root_end = expected
            .checked_add(root_len)
            .ok_or(FormatError::BadOffsets)?;
        if root_end > end {
            return Err(FormatError::BadOffsets);
        }
        let root =
            std::str::from_utf8(&data[expected..root_end]).map_err(|_| FormatError::BadOffsets)?;
        if root.is_empty() || prev_root.is_some_and(|p| p.as_bytes() >= root.as_bytes()) {
            return Err(FormatError::BadOffsets);
        }
        prev_root = Some(root);

        let mut pos = root_end;
        for _ in 0..suffix_count {
            if pos >= end {
                return Err(FormatError::BadOffsets);
            }
            let suffix_len = data[pos] as usize;
            if suffix_len == 0 {
                return Err(FormatError::BadOffsets);
            }
            let suffix_start = pos.checked_add(1).ok_or(FormatError::BadOffsets)?;
            let suffix_end = suffix_start
                .checked_add(suffix_len)
                .ok_or(FormatError::BadOffsets)?;
            if suffix_end > end {
                return Err(FormatError::BadOffsets);
            }
            if std::str::from_utf8(&data[suffix_start..suffix_end]).is_err() {
                return Err(FormatError::BadOffsets);
            }
            pos = suffix_end.checked_add(4).ok_or(FormatError::BadOffsets)?;
            if pos > end {
                return Err(FormatError::BadOffsets);
            }
        }
        expected = pos;
    }
    if expected != end {
        return Err(FormatError::BadOffsets);
    }
    Ok(AssocView {
        start,
        len: end - start,
        root_count,
        records_start,
    })
}

#[cfg(test)]
#[path = "format_assoc_tests.rs"]
mod tests;
