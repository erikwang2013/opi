// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Physical -> AWT logical pixels: the `\\.\pipe\opi-v2` protocol sends
//! positions in logical pixels, while TSF gives physical pixels.
//! Platform-neutral so the arithmetic is covered by host tests; Phase 2's
//! caret code calls it with `dpi::window_dpi`.

/// `dpi = 0` means "unknown", treated as 96 (100%).
#[allow(dead_code)] // Phase 2 caret positioning.
pub(crate) fn to_logical(value: i32, dpi: u32) -> i32 {
    let dpi = if dpi == 0 { 96 } else { i64::from(dpi) };
    (i64::from(value) * 96 / dpi) as i32
}

#[cfg(test)]
mod tests {
    use super::to_logical;

    #[test]
    fn identity_at_96_and_rounded_at_125() {
        assert_eq!(to_logical(200, 0), 200);
        assert_eq!(to_logical(200, 96), 200);
        // Integer division truncates like the Windows 10 crate: 200*96/120.
        assert_eq!(to_logical(200, 120), 160);
        assert_eq!(to_logical(150, 144), 100);
    }
}
