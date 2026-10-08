// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语言栏按钮的图标载具：把入库的 `.ico` 字节解析成 RT_ICON 形态的 DIB 条目。
//!
//! **为什么是 `include_bytes!` 而不是 `.rc` 资源**：`langbar.rs` 的 `GetIcon` 要造
//! HICON，而 `.rc` → 资源编译器 → 链接期嵌 `.rsrc` 这条路**在本机与 CI 两处都没有
//! 执行点**（本机 rc.exe / windres / llvm-rc 零命中；CI 的 Windows 步骤是 Linux 上的
//! `cargo check`，不链接）。走 `include_bytes!` 后，"挑哪条、格式对不对"变成
//! **Linux 上可单测的纯函数** —— 只剩 `CreateIconFromResourceEx` 那一次调用
//! 本机无法验证（见 `langbar.rs` 头注释的验证边界）。
//!
//! **几何真源**：`crates/tsf-opi/assets/opi-langbar.ico` 是**生成物**
//! （`scripts/gen_pet_icons.py`，几何 = R4 图标简化版）。别手改：改了下次重生成
//! 就无声回退；而本文件的单测拿 `include_bytes!` 的**真实字节**当夹具，漂了会红。
//!
//! **两条格式约定**（生成器按此产出、这里按此校验，一处约定两处执行）：
//! 1. 条目必须是 **BMP(DIB)** 条目，不能是 PNG 条目 —— Vista 起 ico 允许 PNG 压缩，
//!    但 `CreateIconFromResourceEx` 要的是 RT_ICON 位图形态，PNG 条目是非法输入；
//! 2. 条目 = `BITMAPINFOHEADER(biHeight = 2h)` + XOR(32bpp，行**自下而上**) + AND(1bpp)。

/// 入库的 ico（16 + 32 两条）。编译期嵌入，与 DLL 同生共死。
pub const ICO: &[u8] = include_bytes!("../assets/opi-langbar.ico");

/// 一条可用的 ico 条目：指向 [`ICO`] 内部的一段（不复制）。
pub struct Entry<'a> {
    /// ICONIMAGE 字节 = RT_ICON 形态，直接喂 `CreateIconFromResourceEx`。
    pub dib: &'a [u8],
    /// 逻辑边长（从 BITMAPINFOHEADER 取 —— 不信任 ICONDIRENTRY 那个 1 字节宽度）。
    pub w: u32,
    pub h: u32,
}

/// 小端 u32 读取；越界 → None（所有 per-条目 的读取都经它，不会 panic）。
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// 在 ico 里挑一条**边长 ≤ `max`、尽量大**的 DIB 条目；没有 ≤ max 的则退而取最小的一条。
/// 任何一处不合法都只**跳过那一条**；整体不可解析（头截断等）返回 `None` ——
/// 调用方把 `None` 当"没有图标"（按钮只剩文字），不是崩。
pub fn entry(ico: &[u8], max: u32) -> Option<Entry<'_>> {
    // ICONDIR：reserved=0 · type=1(图标) · count。
    if !ico.starts_with(&[0, 0, 1, 0]) {
        return None;
    }
    let count = u16::from_le_bytes(ico.get(4..6)?.try_into().ok()?) as usize;
    let mut under: Option<Entry<'_>> = None; // 最大的一条 ≤ max
    let mut over: Option<Entry<'_>> = None; // 最小的一条 > max（兜底）
    for i in 0..count {
        // 某条 ICONDIRENTRY 缺失（count 撒谎）⇒ 后面的也读不到，直接收摊。
        let Some(e) = ico.get(6 + i * 16..6 + i * 16 + 16) else {
            break;
        };
        let (Some(size), Some(off)) = (le32(e, 8), le32(e, 12)) else {
            continue;
        };
        let Some(end) = (off as usize).checked_add(size as usize) else {
            continue;
        };
        let Some(dib) = ico.get(off as usize..end) else {
            continue;
        };
        // 只看 BITMAPINFOHEADER 形态的条目（PNG 条目首 4 字节是 PNG 签名 ⇒ 这里就落选）。
        let (Some(bi_size), Some(w), Some(bh)) = (le32(dib, 0), le32(dib, 4), le32(dib, 8)) else {
            continue;
        };
        if bi_size != 40 {
            continue;
        }
        // biHeight = 2h（XOR + AND 两层）；非方形本文件没有，长度算式也不成立 ⇒ 不猜。
        if w == 0 || bh % 2 != 0 || bh / 2 != w {
            continue;
        }
        let h = w;
        // 长度自洽：头部 40 + XOR(w·h·4) + AND(4 字节对齐的 1bpp 行 × h)。
        let stride = w.div_ceil(32) * 4;
        if dib.len() != (40 + w * h * 4 + stride * h) as usize {
            continue;
        }
        let cand = Entry { dib, w, h };
        if w <= max {
            if under.as_ref().is_none_or(|c| c.w < w) {
                under = Some(cand);
            }
        } else if over.as_ref().is_none_or(|c| c.w > w) {
            over = Some(cand);
        }
    }
    under.or(over)
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：langbar_icon_tests.rs
#[cfg(test)]
#[path = "langbar_icon_tests.rs"]
mod tests;
