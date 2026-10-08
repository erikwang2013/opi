// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `langbar_icon.rs` 的主机单测（Linux 可跑 —— 这正是把脚从 langbar.rs 搬出来的理由：
//! 真正会错的是「哪条、多长、是不是 RT_ICON 形态」，那是**纯字节判定**，
//! 不该待在 `#[cfg(target_os = "windows")]` 里陪 CreateIconFromResourceEx 一起不可测）。
//!
//! 夹具分两半：**入库的真实资产**（漂了能发现）+ 手工拼的畸形 ico（把拒绝路径钉住）。

use super::*;

/// 拼一条合法形态的 32bpp DIB 条目（就是生成器 `ico_dib` 的形态）。
fn dib(w: u32) -> Vec<u8> {
    let stride = w.div_ceil(32) * 4;
    let mut v = Vec::new();
    v.extend_from_slice(&40u32.to_le_bytes()); // biSize
    v.extend_from_slice(&w.to_le_bytes()); // biWidth
    v.extend_from_slice(&(w * 2).to_le_bytes()); // biHeight = 2h
    v.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    v.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    v.extend_from_slice(&[0u8; 4]); // biCompression
    v.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
    v.extend_from_slice(&[0u8; 16]); // xppm/yppm/clrUsed/clrImportant
    v.extend_from_slice(&vec![0u8; (w * w * 4) as usize]); // XOR
    v.extend_from_slice(&vec![0u8; (stride * w) as usize]); // AND
    v
}

/// 拼一个 ico 容器。`entries` = (条目字节, ICONDIRENTRY 里声明的宽度)，**故意分开传**：
/// 宽度字节我们不信任（解析器只认 BITMAPINFOHEADER 里的），畸形夹具正要点这个。
fn ico(entries: &[(Vec<u8>, u32)]) -> Vec<u8> {
    let mut v = vec![0, 0, 1, 0];
    v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut off = 6 + 16 * entries.len();
    for (blob, w) in entries {
        v.push(*w as u8);
        v.push(*w as u8);
        v.extend_from_slice(&[0, 0]); // 颜色数 / 保留
        v.extend_from_slice(&1u16.to_le_bytes()); // planes
        v.extend_from_slice(&32u16.to_le_bytes()); // bpp
        v.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        v.extend_from_slice(&(off as u32).to_le_bytes());
        off += blob.len();
    }
    for (blob, _) in entries {
        v.extend_from_slice(blob);
    }
    v
}

// ---------- 入库资产（真字节） ----------

#[test]
fn real_asset_yields_the_32px_entry() {
    let e = entry(ICO, 32).expect("入库 ico 里必须能挑出 32px 条目");
    assert_eq!((e.w, e.h), (32, 32));
    assert_eq!(le32(e.dib, 0), Some(40), "必须是 BITMAPINFOHEADER 形态");
    // 长度自洽：40 + XOR(32²×4) + AND(4×32)
    assert_eq!(e.dib.len(), 40 + 32 * 32 * 4 + 4 * 32);
}

#[test]
fn real_asset_yields_the_16px_entry_when_asked() {
    // SM_CXSMICON 常态是 16：这一条就是为它准备的。
    let e = entry(ICO, 16).expect("入库 ico 里必须有 ≤16 的条目");
    assert_eq!((e.w, e.h), (16, 16));
}

#[test]
fn real_asset_falls_back_to_the_smallest_larger_entry() {
    // 要 8px：库里没有 ⇒ 退最小的一条（16），而不是 None。
    let e = entry(ICO, 8).expect("没有 ≤8 的条目时应退而取最小的一条");
    assert_eq!(e.w, 16);
}

// ---------- 畸形夹具：拒绝路径 ----------

#[test]
fn png_entry_is_skipped_in_favour_of_a_dib_entry() {
    // PNG 条目（首 4 字节是 PNG 签名的高位段）在前、DIB 在后：必须选中 DIB。
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(&[0u8; 40]);
    let good = dib(32);
    let bytes = ico(&[(png, 32), (good.clone(), 32)]);
    let e = entry(&bytes, 32).expect("DIB 条目必须被选中");
    assert_eq!(e.dib, &good[..]);
}

#[test]
fn png_only_ico_yields_none() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(&[0u8; 40]);
    assert!(entry(&ico(&[(png, 32)]), 32).is_none());
}

#[test]
fn truncated_entry_is_skipped_in_favour_of_a_good_one() {
    let good = dib(16);
    let mut bad = dib(32);
    bad.truncate(bad.len() - 8); // 长度不再自洽
    let bytes = ico(&[(bad, 32), (good.clone(), 16)]);
    let e = entry(&bytes, 32).expect("合法的那条必须被选中");
    assert_eq!(e.dib, &good[..]);
}

#[test]
fn entry_pointing_past_the_buffer_is_skipped() {
    let good = dib(16);
    let mut bytes = ico(&[(dib(32), 32), (good.clone(), 16)]);
    // 把第一条的 dwImageOffset 改到缓冲区外
    let off = 6 + 12;
    bytes[off..off + 4].copy_from_slice(&0xFFFF_0000u32.to_le_bytes());
    let e = entry(&bytes, 32).expect("越界条目应跳过、不应中止整次解析");
    assert_eq!(e.dib, &good[..]);
}

#[test]
fn wrong_biheight_or_square_mismatch_is_rejected() {
    // biHeight = h（少了 AND 层）→ 拒绝；只剩它 ⇒ None。
    let mut bad = dib(32);
    bad[8..12].copy_from_slice(&32u32.to_le_bytes());
    assert!(entry(&ico(&[(bad, 32)]), 32).is_none());
}

#[test]
fn garbage_header_yields_none() {
    assert!(entry(b"", 32).is_none());
    assert!(entry(b"\x00\x00\x02\x00\x01\x00", 32).is_none()); // type=2（光标，不是图标）
    assert!(entry(b"\x00\x00\x01\x00\x05\x00", 32).is_none()); // 声称 5 条、实际没有
}
