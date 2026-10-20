// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C3：候选窗通信（Windows 目标专属；lib.rs 以 `#[cfg(target_os = "windows")]`
//! 引入本模块，Linux 主机不编译 —— 与 tsf.rs 同构的双重隔离）。
//!
//! 【线协议：NDJSON over named pipe】（与 desktop/src/main/kotlin/xyz/erik/opi/candidate/Main.kt
//! 头注释互为镜像，改协议须同步两处）
//!
//! ```text
//! 管道名：\\.\pipe\opi（单实例，字节流模式，'\n' 分帧，UTF-8）
//! 角色：候选窗（CMP desktop/）为 SERVER 监听管道；TSF 插件进程为 CLIENT，
//!       窗口启动后连接（窗口不在 → 本次发送放弃、下一条再试，不报错；
//!       读线程有自己的重连循环）。
//!
//! TSF(CLIENT) → 候选窗(SERVER)：
//!   show      {"type":"show","buffer":"ni","candidates":["你","尼",...],
//!              "page":1,"page_count":3,"mode":"pinyin"}
//!             （page/page_count 为 1 起；candidates 为当前页文本，页内 0 起）
//!   hide      {"type":"hide"}
//!   position  {"type":"position","x":120,"y":340}
//!             （可选项：caret 提示，窗口跟随光标；骨架无 caret 数据 → 固定位置）
//!             x/y 单位 = **AWT 逻辑像素**（= Compose 的 `.dp` 数值）：收端直接
//!             `WindowPosition(x.dp, y.dp)`，而 CMP 1.11.1 的窗口定位**不乘 density**
//!             （反编译 `setPositionImpl` 确认），故发 200 就落在离屏幕左缘 200
//!             逻辑像素处。**发端不得预乘 density/缩放，收端不得按设备像素解读。**
//!
//! 候选窗(SERVER) → TSF(CLIENT)：
//!   select    {"type":"select","index":0}    // 用户点击第 index（页内 0 起）候选
//!   next_page {"type":"next_page"} / {"type":"prev_page"}
//!   commit_text {"type":"commit_text","text":"，"}   // 直插文本（2026-10-10）：
//!             不经引擎选词 —— 缓冲/候选/学习都不动、也不通知 sink（多选不关窗，
//!             发端是桌面自渲染的符号页）。文本排回按键线程执行，与 select 同一条
//!             队列语义（见 state.rs 的线程模型）。
//!
//! 控制面（2026-10-20，状态栏/设置 → TSF）：候选窗进程把桌面请求原样转发进来，
//! TSF 在同一条管道上应答。请求/应答的**语义**在 `ctl.rs`（平台中立、有单测），
//! 这里只负责分帧与回写：
//!   ctl        {"type":"ctl","id":7,"op":"state"} 等（见 ctl.rs 头）
//!   ctl_result {"type":"ctl_result","id":7,"ok":true,"state":"..."}（按 id 关联）
//! ctl 在读线程上**同步**执行（只动引擎锁，不碰 COM）；上屏文本入既有队列。
//! 应答从读线程直接回写（`write_now`）：窗口侧读循环与读线程是两个线程，
//! 同一句柄上的写由 conn 锁串行化。
//! ```
//!
//! 本模块是纯胶水（镜像 tsf.rs 结构）：经 `TsfSink` 接缝（on_commit /
//! on_composition_changed）把候选数据序列化为 show/hide/position 发给候选窗；
//! 逻辑层（`TsfLogic`）零改动。`CandidateAction` 是候选窗回复（select/翻页）的
//! 回调接缝，生产实现是 `SharedAction`（B1 接线：select → `TsfLogic::select` →
//! 插进文档；翻页 → `next_page`/`prev_page`）。
//!
//! 连接语义：惰性连接；**窗口未启动时本次发送直接放弃、不睡眠**（下一条 show
//! 再试，2026-10-18 性能审计：旧行为在按键线程 4×50ms 空等 200ms）；写失败即断开、
//! 下次发送时重连；读线程阻塞读回复并分帧解析（它自己的重连循环才使用
//! `RETRY_SLEEP_MS`，不在按键线程上）。**宿主 Deactivate 时主动断开**
//! （`TsfSink::disconnect`）：管道是单实例，每个宿主进程一个客户端，谁先 show
//! 谁占住；不释放则其余应用永远连不上（「只在桌面/单个应用可用」）。
//! 生命周期：`CandidateClient` 的 Drop 关句柄 + 停读线程（否则句柄/线程/回调
//! 陪宿主进程跑到结束）；分帧残留有 1 MiB 上限（见 `MAX_PENDING`）。
//! 已知简化（验收可加固）：句柄替换与读线程的竞态以“仅清除仍指向自身句柄”
//! 的方式容忍（Drop 关闭句柄与读线程的 ReadFile 亦是同一容忍级别）；
//! 重连场景罕见（候选窗重启），骨架期不做 DuplicateHandle。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use engine_core::composer::Mode;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_MODE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows::core::{PCWSTR, w};

use crate::tsf::TsfSink;

/// 管道句柄的 Send+Sync 包装：windows-rs 的 HANDLE 是裸指针（非 Sync），
/// 而读写跨线程使用（按键线程写 + 读线程阻塞读），此处显式承诺线程安全。
/// 句柄自身不可拷贝到别的进程，跨线程共享同一连接是安全的。
#[derive(Clone, Copy, PartialEq, Eq)]
struct PipeHandle(HANDLE);
unsafe impl Send for PipeHandle {}
unsafe impl Sync for PipeHandle {}

/// 候选窗管道名（窗口侧 = desktop/ 的 JNA 服务器端，两侧保持一致）。
const PIPE_NAME: PCWSTR = w!(r"\\.\pipe\opi");
/// 读线程重连间隔（窗口未启动/忙）。
const RETRY_SLEEP_MS: u64 = 50;
/// 读线程缓冲区（单条消息远小于此；字节流模式按 '\n' 分帧）。
const READ_BUF: usize = 4096;
/// 分帧残留（pending）上限。对端只要发一条不含 '\n' 的超长消息，残留就会一直涨 ——
/// 而本模块跑在宿主输入法进程里，涨的是宿主的内存。1 MiB 远超单条消息（一页候选
/// JSON 至多几 KB）；超限即协议错乱/恶意对端 → 丢弃残留并断开（下次发送重连）。
const MAX_PENDING: usize = 1 << 20;

/// 候选窗回复回调接缝：select/翻页/直插 → TSF 文档操作（生产实现见 `SharedAction`）。
pub trait CandidateAction: Send + Sync {
    fn on_select(&self, _index: usize) {}
    fn on_next_page(&self) {}
    fn on_prev_page(&self) {}
    /// 候选窗要求**直接插入**一段文本（不经引擎选词；符号页多选，见协议头）。
    fn on_commit_text(&self, _text: &str) {}
    /// 控制请求（状态/动作/搜索/直插）。返回**应答行**（不含 '\n'）；None = 未处理。
    /// 读线程调用 —— 实现不得碰 COM（见 `SharedAction` 的实现）。
    fn on_ctl(&self, _request: &crate::ctl::CtlRequest) -> Option<String> {
        None
    }
}

/// 断开连接：仅当 `handle` 仍是当前持有的那个才关句柄并清空 conn
/// （写线程可能已换过句柄，关错就断了别人的连接）。返回 false = 锁中毒，
/// 调用方（读线程）应退出。
fn disconnect(conn: &Mutex<Option<PipeHandle>>, handle: PipeHandle) -> bool {
    match conn.lock() {
        Ok(mut guard) => {
            if *guard == Some(handle) {
                unsafe { CloseHandle(handle.0) }.ok();
                *guard = None;
            }
            true
        }
        Err(_) => false,
    }
}

/// named pipe 客户端：惰性连接 + 写消息 + 读线程解析候选窗回复。
pub struct CandidateClient {
    /// 当前连接句柄（None = 未连接）。Arc 共享给读线程。
    conn: Arc<Mutex<Option<PipeHandle>>>,
    reader_started: AtomicBool,
    /// 客户端已 Drop → 读线程退出（见 `Drop` 实现）。
    reader_stop: Arc<AtomicBool>,
    action: Arc<dyn CandidateAction>,
}

impl CandidateClient {
    /// `action`：候选窗回复回调（Arc 共享给读线程）。
    pub fn new(action: Arc<dyn CandidateAction>) -> Self {
        let client = Self {
            conn: Arc::new(Mutex::new(None)),
            reader_started: AtomicBool::new(false),
            reader_stop: Arc::new(AtomicBool::new(false)),
            action,
        };
        client.spawn_reader();
        client
    }

    /// show：缓冲 + 当前页候选 + 页码（1 起）+ 模式 → 候选窗渲染。
    pub fn show(
        &self,
        buffer: &str,
        candidates: &[String],
        page: usize,
        page_count: usize,
        mode: Mode,
    ) {
        let msg = serde_json::json!({
            "type": "show",
            "buffer": buffer,
            "candidates": candidates,
            "page": page,
            "page_count": page_count,
            "mode": mode_str(mode),
        });
        self.send_json(&msg.to_string());
    }

    /// hide：composition 结束/清空 → 隐藏候选窗。
    pub fn hide(&self) {
        self.send_json(r#"{"type":"hide"}"#);
    }

    /// position：caret 提示（骨架无 caret 数据时不调用；窗口降级固定位置）。
    pub fn position(&self, x: i32, y: i32) {
        let msg = serde_json::json!({"type": "position", "x": x, "y": y});
        self.send_json(&msg.to_string());
    }

    // ---------- 内部：连接 / 发送 / 读线程 ----------

    /// 尝试连接（**单次，不睡眠**）。
    ///
    /// 2026-10-18 性能审计：旧实现在**按键线程**上对 `ERROR_FILE_NOT_FOUND` 做
    /// 4×50ms 退避 —— 而「候选窗没启动」正是默认状态（全仓没有拉起候选窗的代码），
    /// 于是每键白白阻塞 200ms，直接击穿 50ms 键反馈预算。窗口不会在 200ms 内出现；
    /// 本次放弃，下一次 show 自然再试（状态每次全量重发，丢一次连接不丢状态）。
    /// 读线程有自己的重连循环（`spawn_reader` 的 `RETRY_SLEEP_MS`），不受影响。
    fn connect() -> Option<PipeHandle> {
        match unsafe {
            CreateFileW(
                PIPE_NAME,
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                FILE_SHARE_MODE(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0),
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        } {
            Ok(h) => Some(PipeHandle(h)),
            Err(_) => None, // 未启动/忙/不可重试：本次放弃
        }
    }

    /// 发送一条完整消息（自动补 '\n'）。连接失败 → 本次放弃（不睡眠）；写失败 → 断开待重连。
    fn send_json(&self, json: &str) {
        let bytes = [json.as_bytes(), b"\n"].concat();
        let mut conn = match self.conn.lock() {
            Ok(g) => g,
            Err(_) => return, // 中毒锁：吞掉（与 tsf.rs 的锁策略一致）
        };
        let handle = match *conn {
            Some(h) => h,
            None => match Self::connect() {
                Some(h) => {
                    *conn = Some(h);
                    h
                }
                None => return, // 窗口未启动：本次放弃，下次发送再试
            },
        };
        if !write_all(handle, &bytes) {
            // 写失败（管道另一端关闭等）：断开，下次发送重连。
            unsafe { CloseHandle(handle.0) }.ok();
            *conn = None;
        }
    }

    /// 断开当前连接（宿主 Deactivate 时调用）：关句柄让读线程退出本轮，
    /// 窗口服务端在 readLoop 的 finally 里隐藏并重建单实例管道。下次 show 惰性重连。
    ///
    /// 关闭方式与 `Drop` 相同（读线程可能正阻塞在 ReadFile 上，句柄一关即返回）；
    /// 不在这里 sleep/等确认 —— Deactivate 跑在宿主的 UI 线程上。
    pub fn disconnect(&self) {
        if let Some(h) = self.conn.lock().ok().and_then(|mut g| g.take()) {
            // SAFETY: 句柄由本对象持有，take 后 write/show 不会再使用它。
            unsafe { CloseHandle(h.0) }.ok();
        }
    }

    /// 读线程（构造时启动一次）：阻塞读候选窗回复，'\n' 分帧解析，
    /// 回调 `CandidateAction`。连接失效 → 清句柄（仅当仍是本线程持有的那个），
    /// 下一轮退避重试 —— 骨架期容忍与写线程的句柄替换竞态。
    fn spawn_reader(&self) {
        if self.reader_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let conn = Arc::clone(&self.conn);
        let stop = Arc::clone(&self.reader_stop);
        let action = Arc::clone(&self.action);
        thread::spawn(move || {
            let mut buf = vec![0u8; READ_BUF];
            let mut pending: Vec<u8> = Vec::new(); // '\n' 分帧残留（跨读合并）
            loop {
                if stop.load(Ordering::SeqCst) {
                    return; // 客户端已 Drop（Drop 会关句柄让下面的 ReadFile 返回）
                }
                // 取句柄（仅复制值，不持锁阻塞读）。
                let handle = {
                    let guard = match conn.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    match *guard {
                        Some(h) => h,
                        None => {
                            drop(guard);
                            thread::sleep(Duration::from_millis(RETRY_SLEEP_MS));
                            continue;
                        }
                    }
                };
                // 字节流模式阻塞读：0 字节或错误 → 另一端关闭/断开。
                let mut got = 0u32;
                let ok = unsafe { ReadFile(handle.0, Some(&mut buf), Some(&mut got), None) };
                if ok.is_err() || got == 0 {
                    if !disconnect(&conn, handle) {
                        return;
                    }
                    pending.clear();
                    continue;
                }
                pending.extend_from_slice(&buf[..got as usize]);
                if pending.len() > MAX_PENDING {
                    // 攒够 1 MiB 还没有 '\n'：丢弃残留 + 断开。只清残留不关连接的话，
                    // 对端接着发就接着涨 —— 等于把宿主内存交给对端。
                    pending.clear();
                    if !disconnect(&conn, handle) {
                        return;
                    }
                    continue;
                }
                while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = pending.drain(..pos).collect();
                    pending.drain(..1); // 去掉 '\n'
                    // 回调（select/翻页/ctl → 动作）是外部代码，panic 必须兜住：
                    // 线程一死，reader_started 已置位不会重启 → 回复永久失效。
                    let response =
                        catch_unwind(AssertUnwindSafe(|| dispatch_line(&line, action.as_ref())))
                            .ok()
                            .flatten();
                    if let Some(reply) = response {
                        write_now(&conn, &reply);
                    }
                }
            }
        });
    }
}

impl Drop for CandidateClient {
    /// 停读线程 + 关句柄。此前两者都不回收：正常路径永不 `CloseHandle`，读线程
    /// 阻塞在 `ReadFile` 上永不退出 → 句柄 + 线程 + `Arc<CandidateAction>` 一起
    /// 泄漏到宿主进程结束（插件重载时累积）。
    fn drop(&mut self) {
        self.reader_stop.store(true, Ordering::SeqCst);
        // 取走句柄（PipeHandle 是 Copy，take() 留 None）：关掉它同时让阻塞中的
        // ReadFile 返回，读线程本轮醒来后看到 stop 退出。
        self.disconnect();
    }
}

/// 写一段完整帧（调用方持有 conn 锁）。部分写循环（0.62 的 WriteFile 是切片签名）。
fn write_all(handle: PipeHandle, bytes: &[u8]) -> bool {
    let mut done = 0usize;
    while done < bytes.len() {
        let mut written = 0u32;
        let ok = unsafe { WriteFile(handle.0, Some(&bytes[done..]), Some(&mut written), None) };
        if ok.is_err() || written == 0 {
            return false;
        }
        done += written as usize;
    }
    true
}

/// 读线程回写应答（不惰性连接：能收到请求说明连接已在）。写失败 → 断开待重连。
fn write_now(conn: &Mutex<Option<PipeHandle>>, json: &str) {
    let bytes = [json.as_bytes(), b"\n"].concat();
    let Ok(mut guard) = conn.lock() else {
        return;
    };
    let Some(handle) = *guard else { return };
    if !write_all(handle, &bytes) {
        unsafe { CloseHandle(handle.0) }.ok();
        *guard = None;
    }
}

/// 解析一行并分发到 `CandidateAction`（未知 type 忽略）。返回应答行 = ctl 请求。
fn dispatch_line(line: &[u8], action: &dyn CandidateAction) -> Option<String> {
    let v = serde_json::from_slice::<serde_json::Value>(line).ok()?;
    match v.get("type").and_then(|t| t.as_str()) {
        Some("select") => {
            if let Some(i) = v.get("index").and_then(|i| i.as_u64()) {
                action.on_select(i as usize);
            }
        }
        Some("next_page") => action.on_next_page(),
        Some("prev_page") => action.on_prev_page(),
        Some("commit_text") => {
            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                action.on_commit_text(t);
            }
        }
        Some("ctl") => {
            let request = std::str::from_utf8(line).ok().and_then(crate::ctl::parse)?;
            return action.on_ctl(&request);
        }
        _ => {}
    }
    None
}

/// 候选窗回复的生产实现见 `state.rs` 的 `SharedAction`（随 ctl 处理一起搬过去，
/// 那里已有引擎/文档写入的线程模型；本文件只管管道与分帧）。

/// 候选窗接缝的生产实现：把 composition 变化映射为 show/hide。
pub struct CandidateSink {
    client: CandidateClient,
}

impl CandidateSink {
    pub fn new(action: Arc<dyn CandidateAction>) -> Self {
        Self {
            client: CandidateClient::new(action),
        }
    }
}

impl TsfSink for CandidateSink {
    /// 提交：composition 结束 → 隐藏候选窗。
    fn on_commit(&self, _text: &str) {
        self.client.hide();
    }

    /// composition/候选变化：缓冲非空 → show（页码 1 起）；空 → hide。
    fn on_composition_changed(
        &self,
        buffer: &str,
        candidates: &[String],
        page: usize,
        page_count: usize,
        mode: Mode,
    ) {
        if buffer.is_empty() {
            self.client.hide();
        } else {
            self.client
                .show(buffer, candidates, page + 1, page_count, mode);
        }
    }

    /// 宿主失活：断开管道，把单实例让给下一个应用（见 TsfSink::disconnect）。
    fn disconnect(&self) {
        self.client.disconnect();
    }
}

/// Mode → 协议字符串（与 desktop/ 解析端一致）。
/// Traditional 复用 "pinyin"：候选窗（desktop 的 `CandidateWindow.kt modeLabel`）无简繁概念，
/// 未知字符串一律回落 "拼音" 标签，新增 "traditional" 只会造成协议漂移。
fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Pinyin | Mode::Traditional => "pinyin",
        Mode::English => "english",
        Mode::Number => "number",
        Mode::Symbol => "symbol",
    }
}
