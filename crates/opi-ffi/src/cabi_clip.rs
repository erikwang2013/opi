// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! opi-clip 的 C ABI 出口：不透明句柄 + `opi_clip_*`。语义与 JNI 面
//! （`jni_clip.rs`）**同源** —— 两边都调本模块的 `OpiClip` 方法，不各写一份。
//!
//! 契约（与各端薄壳）：
//! - **句柄不可跨线程共享**：create 记录创建线程，其余调用在该线程之外一律失败并在
//!   `opi_clip_last_error()` 里给出原因（destroy 也一样；错线程 destroy 会**泄漏**句柄，
//!   但不会 UB —— 宁可泄漏也不释放正在被别的线程使用的内存）。
//! - create → start：create 只解析配置，start 才起线程/绑端口；`start` 重复调用幂等。
//! - `stop` 幂等；`poll_event(timeout_ms=0)` 立即返回（非阻塞），>0 最多等 1 秒。
//! - 失败统一返回哨兵（NULL / false / -1 / 空 `OpiString`），原因读 `opi_clip_last_error()`。
//! - 事件 JSON 上限 **1 MiB**（超限的那条被丢弃并记 last_error）；文本本身仍受内核
//!   256 KiB 上限约束。
//!
//! 生命周期：`opi_clip_destroy` 释放句柄（内部 `stop` + join）；NULL 安全。
//!
//! ⚠️ 与 `cabi.rs` 一样关掉 clippy 的 `missing_safety_doc`，替代执行点见
//! `tests/cabi_safety_doc.rs`（它读的是合成语料 `read_cabi()`，**本文件已并入**）。

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use opi_clip::{ClipConfig, ClipNode, InstanceId, PublishOutcome};
use serde_json::json;

use crate::cabi::OpiString;
use crate::cabi::utf16::read_utf16;

mod encode;
#[cfg(test)]
mod tests;

use encode::{encode_event, peer_json, to_json};

/// `poll_event` 的单次最长等待（毫秒）。0 = 立即返回。
const MAX_POLL_WAIT_MS: u32 = 1000;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

pub(crate) fn set_last_error(message: impl Into<String>) {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = message.into());
}

pub(crate) fn last_error_string() -> String {
    LAST_ERROR.with(|slot| slot.borrow().clone())
}

/// `catch_unwind` 结果 → 出口哨兵；失败写 last_error。
pub(crate) fn settle<T>(result: thread::Result<Result<T, String>>, sentinel: T) -> T {
    match result {
        Ok(Ok(value)) => value,
        Ok(Err(message)) => {
            set_last_error(message);
            sentinel
        }
        Err(_) => {
            set_last_error("panic inside opi_clip_*");
            sentinel
        }
    }
}

/// 不透明句柄。字段不公开；C 侧只有 `OpiClip *`。
pub struct OpiClip {
    owner: ThreadId,
    config: ClipConfig,
    node: Option<ClipNode>,
}

impl OpiClip {
    fn new(config: ClipConfig) -> Self {
        Self {
            owner: thread::current().id(),
            config,
            node: None,
        }
    }

    /// 解析配置 JSON 并建句柄（不启动任何线程）。
    pub(crate) fn create(json: &str) -> Result<*mut Self, String> {
        if json.trim().is_empty() {
            return Err("clip config JSON is empty".into());
        }
        let config: ClipConfig =
            serde_json::from_str(json).map_err(|e| format!("clip config JSON: {e}"))?;
        if config.data_dir.as_os_str().is_empty() {
            return Err("clip config data_dir must not be empty".into());
        }
        Ok(Box::into_raw(Box::new(Self::new(config))))
    }

    fn check_owner(&self) -> Result<(), String> {
        if self.owner == thread::current().id() {
            Ok(())
        } else {
            Err("clip handle used from a non-owner thread (handles are not shareable across threads)"
                .into())
        }
    }

    fn node(&self) -> Result<&ClipNode, String> {
        self.node
            .as_ref()
            .ok_or_else(|| "clip node is not started".into())
    }

    pub(crate) fn start(&mut self) -> Result<bool, String> {
        self.check_owner()?;
        if self.node.is_some() {
            return Ok(true);
        }
        let node = ClipNode::start(self.config.clone()).map_err(|e| e.to_string())?;
        self.node = Some(node);
        Ok(true)
    }

    pub(crate) fn stop(&mut self) -> Result<(), String> {
        self.check_owner()?;
        if let Some(node) = self.node.take() {
            node.stop();
        }
        Ok(())
    }

    /// 0 = 已发布，1 = 被防回环抑制（正常，不上屏也不报错），-1 = 失败。
    pub(crate) fn publish(&self, text: &str) -> Result<i32, String> {
        self.check_owner()?;
        match self.node()?.publish_text(text).map_err(|e| e.to_string())? {
            PublishOutcome::Published { .. } => Ok(0),
            PublishOutcome::SuppressedLoop => Ok(1),
        }
    }

    /// 返回事件 JSON；空串 = 这段时间没有事件。`timeout_ms=0` 立即返回。
    pub(crate) fn poll(&self, timeout_ms: u32) -> Result<String, String> {
        self.check_owner()?;
        let node = self.node()?;
        let deadline =
            Instant::now() + Duration::from_millis(u64::from(timeout_ms.min(MAX_POLL_WAIT_MS)));
        loop {
            if let Some(event) = node.poll_event() {
                return encode_event(event);
            }
            if timeout_ms == 0 || Instant::now() >= deadline {
                return Ok(String::new());
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub(crate) fn peers_json(&self) -> Result<String, String> {
        self.check_owner()?;
        let Some(node) = &self.node else {
            return Ok("[]".into());
        };
        let peers: Vec<serde_json::Value> = node.peers().iter().map(peer_json).collect();
        to_json(&serde_json::Value::Array(peers))
    }

    pub(crate) fn pair_start(&self, peer_hex: &str) -> Result<bool, String> {
        self.check_owner()?;
        let peer = parse_peer(peer_hex)?;
        self.node()?.pair(peer).map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub(crate) fn pair_confirm(&self, peer_hex: &str, accept: bool) -> Result<bool, String> {
        self.check_owner()?;
        let peer = parse_peer(peer_hex)?;
        self.node()?
            .confirm_pairing(peer, accept)
            .map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub(crate) fn forget_peer(&self, peer_hex: &str) -> Result<bool, String> {
        self.check_owner()?;
        let peer = parse_peer(peer_hex)?;
        self.node()?.forget_peer(peer).map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub(crate) fn status_json(&self) -> Result<String, String> {
        self.check_owner()?;
        let (instance_id, fingerprint, peers) = match &self.node {
            Some(node) => (
                Some(node.local_instance_id().to_hex()),
                Some(node.local_fingerprint().to_hex()),
                node.peers().len(),
            ),
            None => (None, None, 0),
        };
        to_json(&json!({
            "running": self.node.is_some(),
            "instance_id": instance_id,
            "fingerprint": fingerprint,
            "device_name": self.config.device_name,
            "tcp_port": self.config.tcp_port,
            "discovery_port": self.config.discovery_port,
            "peers": peers,
        }))
    }

    /// 释放句柄。错线程调用只记 last_error 并返回（泄漏而非 UB）。
    pub(crate) unsafe fn free(handle: *mut Self) -> Result<(), String> {
        if handle.is_null() {
            return Ok(());
        }
        let clip = unsafe { &*handle };
        clip.check_owner()?;
        drop(unsafe { Box::from_raw(handle) });
        Ok(())
    }

    /// # Safety
    /// `handle` 必须是 `OpiClip::create` 返回的指针（可空）。
    pub(crate) unsafe fn borrow<'a>(handle: *mut Self) -> Result<&'a Self, String> {
        unsafe { handle.as_ref() }.ok_or_else(|| "clip handle is NULL".into())
    }

    /// # Safety
    /// 同 [`Self::borrow`]；调用方保证没有别名 `&mut`。
    pub(crate) unsafe fn borrow_mut<'a>(handle: *mut Self) -> Result<&'a mut Self, String> {
        unsafe { handle.as_mut() }.ok_or_else(|| "clip handle is NULL".into())
    }
}

fn parse_peer(hex: &str) -> Result<InstanceId, String> {
    InstanceId::from_hex(hex.trim()).map_err(|e| e.to_string())
}

// ---------- C 导出 ----------

/// create(config_json) -> 句柄。配置 JSON：`{"data_dir":"...","tcp_port":0,
/// "discovery_port":45888,"device_name":"..."}`；只解析、不启动。失败 → NULL。
/// # Safety
/// `config_json` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_create(config_json: *const u16, len: usize) -> *mut OpiClip {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let json =
                unsafe { read_utf16(config_json, len) }.ok_or("config JSON is not valid UTF-16")?;
            OpiClip::create(&json)
        })),
        std::ptr::null_mut(),
    )
}

/// destroy(clip)：停止并释放句柄；NULL 安全。必须在创建它的线程调用。
/// # Safety
/// `clip` 必须是本库返回且尚未释放的句柄（或 NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_destroy(clip: *mut OpiClip) {
    settle(
        catch_unwind(AssertUnwindSafe(|| unsafe { OpiClip::free(clip) })),
        (),
    );
}

/// start(clip) -> bool：绑定端口并起后台线程。重复调用幂等。
/// # Safety
/// `clip` 必须是本库返回且未释放的句柄（或 NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_start(clip: *mut OpiClip) -> bool {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow_mut(clip) }?.start()
        })),
        false,
    )
}

/// stop(clip)：幂等；未 start 或重复调用都返回成功。
/// # Safety
/// `clip` 必须是本库返回且未释放的句柄（或 NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_stop(clip: *mut OpiClip) {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow_mut(clip) }?.stop()
        })),
        (),
    );
}

/// localText(clip, text, len) -> int：0=已发布 1=防回环抑制 -1=失败。
/// 文本上限 256 KiB（内核约束）；超限/未 start → -1。
/// # Safety
/// `text` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_local_text(
    clip: *mut OpiClip,
    text: *const u16,
    len: usize,
) -> i32 {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let text =
                unsafe { read_utf16(text, len) }.ok_or("clipboard text is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.publish(&text)
        })),
        -1,
    )
}

/// pollEvent(clip, timeout_ms) -> JSON 事件；空句柄 = 无事件（或被丢弃，见
/// last_error）。timeout_ms=0 立即返回；>0 最多等 1 秒。
/// # Safety
/// `clip` 必须是本库返回且未释放的句柄（或 NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_poll_event(clip: *mut OpiClip, timeout_ms: u32) -> OpiString {
    let json = settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow(clip) }?.poll(timeout_ms)
        })),
        String::new(),
    );
    OpiString::from_utf16(&json)
}

/// pairStart(clip, peer_hex) -> bool：peer 是 32 位十六进制实例 ID。
/// # Safety
/// `peer_id` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_pair_start(
    clip: *mut OpiClip,
    peer_id: *const u16,
    len: usize,
) -> bool {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_utf16(peer_id, len) }.ok_or("peer id is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.pair_start(&peer)
        })),
        false,
    )
}

/// pairConfirm(clip, peer_hex, accept) -> bool：收到 PairingPrompt 后调用一次。
/// # Safety
/// `peer_id` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_pair_confirm(
    clip: *mut OpiClip,
    peer_id: *const u16,
    len: usize,
    accept: bool,
) -> bool {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_utf16(peer_id, len) }.ok_or("peer id is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.pair_confirm(&peer, accept)
        })),
        false,
    )
}

/// pairCancel(clip, peer_hex) -> bool：等价于 reject（`pairConfirm(..., false)`）。
/// # Safety
/// `peer_id` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_pair_cancel(
    clip: *mut OpiClip,
    peer_id: *const u16,
    len: usize,
) -> bool {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_utf16(peer_id, len) }.ok_or("peer id is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.pair_confirm(&peer, false)
        })),
        false,
    )
}

/// peers(clip) -> JSON 数组；未 start → `[]`。
/// # Safety
/// `clip` 必须是本库返回且未释放的句柄（或 NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_peers(clip: *mut OpiClip) -> OpiString {
    let json = settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow(clip) }?.peers_json()
        })),
        String::new(),
    );
    OpiString::from_utf16(&json)
}

/// forgetPeer(clip, peer_hex) -> bool：移除信任；不存在的对端也算成功。
/// # Safety
/// `peer_id` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_forget_peer(
    clip: *mut OpiClip,
    peer_id: *const u16,
    len: usize,
) -> bool {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_utf16(peer_id, len) }.ok_or("peer id is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.forget_peer(&peer)
        })),
        false,
    )
}

/// status(clip) -> JSON：`{running,instance_id,fingerprint,device_name,tcp_port,
/// discovery_port,peers}`；未 start 时 instance_id/fingerprint 为 null。
/// # Safety
/// `clip` 必须是本库返回且未释放的句柄（或 NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_status(clip: *mut OpiClip) -> OpiString {
    let json = settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow(clip) }?.status_json()
        })),
        String::new(),
    );
    OpiString::from_utf16(&json)
}

/// lastError() -> 最近一次失败原因；空串 = 没有错误。按线程记录（句柄不跨线程）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_last_error() -> OpiString {
    let message = settle(
        catch_unwind(AssertUnwindSafe(|| {
            Ok::<String, String>(last_error_string())
        })),
        String::new(),
    );
    OpiString::from_utf16(&message)
}
