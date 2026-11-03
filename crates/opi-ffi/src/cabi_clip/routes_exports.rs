// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 文本/剪贴板的定向路由出口（从 `cabi_clip.rs` 拆出守 500 行）。
//!
//! 路由是内核状态：发送侧只投给指定会话；接收侧只落指定设备的事件。
//! 空十六进制串 = 该方向不过滤（保持原来的广播/全收行为）。

use std::panic::{AssertUnwindSafe, catch_unwind};

use opi_clip::InstanceId;

use super::OpiClip;
use crate::cabi::utf16::read_utf16;

impl OpiClip {
    pub(crate) fn set_text_routes(
        &self,
        send_hex: &str,
        receive_hex: &str,
    ) -> Result<bool, String> {
        self.check_owner()?;
        let send = parse_optional_peer(send_hex)?;
        let receive = parse_optional_peer(receive_hex)?;
        self.node()?.set_text_routes(send, receive);
        Ok(true)
    }
}

fn parse_optional_peer(hex: &str) -> Result<Option<InstanceId>, String> {
    let hex = hex.trim();
    if hex.is_empty() {
        Ok(None)
    } else {
        super::parse_peer(hex).map(Some)
    }
}

/// setTextRoutes(clip, send_hex, send_len, receive_hex, receive_len) -> bool。
/// 任一串为空 = 该方向全部设备；非法十六进制/未 start → false。
/// # Safety
/// 两个指针都必须指向对应的 UTF-16 字符串（或为 null 且长度为 0）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_set_text_routes(
    clip: *mut OpiClip,
    send_hex: *const u16,
    send_len: usize,
    receive_hex: *const u16,
    receive_len: usize,
) -> bool {
    super::settle(
        catch_unwind(AssertUnwindSafe(|| {
            let send =
                unsafe { read_utf16(send_hex, send_len) }.ok_or("send peer is not valid UTF-16")?;
            let receive = unsafe { read_utf16(receive_hex, receive_len) }
                .ok_or("receive peer is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.set_text_routes(&send, &receive)
        })),
        false,
    )
}
