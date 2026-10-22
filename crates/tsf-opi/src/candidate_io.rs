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
//!             （2026-10-22 起真发：每次 composition 变化取 caret 位置，窗口贴着
//!             输入处；发端见 `caret.rs`）
//!             x/y 单位 = **AWT 逻辑像素**（= Compose 的 `.dp` 数值）：收端直接
//!             `WindowPosition(x.dp, y.dp)`，而 CMP 1.11.1 的窗口定位**不乘 density**
//!             （反编译 `setPositionImpl` 确认），故发 200 就落在离屏幕左缘 200
//!             逻辑像素处。**发端不得预乘 density/缩放，收端不得按设备像素解读。**
//!             TSF 给的是物理像素 ⇒ `caret.rs` 按输入窗口 DPI 折回逻辑像素
//!             （100% = 恒等；125%/150% 必须折算，否则窗口偏出去一截）。
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
//! 下次发送时重连；读线程 `PeekNamedPipe` 轮询（有数据才 `ReadFile`，见
//! `READ_POLL_MS`）并分帧解析（它自己的重连循环才使用 `RETRY_SLEEP_MS`，
//! 不在按键线程上）。**宿主 Deactivate 时主动断开**
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
use windows::Win32::System::Pipes::{
    PIPE_NOWAIT, PIPE_READMODE_BYTE, PeekNamedPipe, SetNamedPipeHandleState,
};
use windows::core::{PCWSTR, w};

/// 管道句柄的 Send+Sync 包装：windows-rs 的 HANDLE 是裸指针（非 Sync），
/// 而读写跨线程使用（按键线程写 + 读线程阻塞读），此处显式承诺线程安全。
/// 句柄自身不可拷贝到别的进程，跨线程共享同一连接是安全的。
#[derive(Clone, Copy, PartialEq, Eq)]
struct PipeHandle(HANDLE);
unsafe impl Send for PipeHandle {}
unsafe impl Sync for PipeHandle {}

/// 候选窗管道名（窗口侧 = desktop/ 的 JNA 服务器端，两侧保持一致）。
const PIPE_NAME: PCWSTR = w!(r"\\.\pipe\opi");
/// 读线程重连间隔（窗口未启动/忙，**仅 active 时**）。
const RETRY_SLEEP_MS: u64 = 50;
/// 宿主失活后的读线程空闲间隔：不抢管道，也不 50ms 醒一次烧 CPU。
const IDLE_SLEEP_MS: u64 = 250;
/// 读线程「有数据才 ReadFile」的轮询间隔。**不能再用阻塞 ReadFile**：同一同步命名
/// 管道句柄上未完成的 ReadFile 会把 WriteFile 串行化 —— 按键线程的第二次 show
/// 会永久阻塞，宿主 UI 线程冻住（2026-10-21 真机：按 n 出候选窗后按 i 不再响应）。
/// 20ms 对候选/控制面足够（按键线程只写，不读）。
const READ_POLL_MS: u64 = 20;
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
    /// 本宿主当前是否激活了本输入法（Activate → true，Deactivate → false）。
    ///
    /// 读线程只在 active 时**主动**连候选窗（2026-10-21）：状态栏的 `state` 查询
    /// 需要这条连接，而连接原先只在第一次 show/hide 时惰性建立 —— 用户切到 opi、
    /// 还没敲第一个键时，状态栏读不到 `im=opi`，整条不显示（体感「没有状态栏」）。
    /// Deactivate 置 false，保证管线交还给下一个应用（单实例管道）。
    active: Arc<AtomicBool>,
    action: Arc<dyn CandidateAction>,
}

impl CandidateClient {
    /// `action`：候选窗回复回调（Arc 共享给读线程）。
    pub fn new(action: Arc<dyn CandidateAction>) -> Self {
        let client = Self {
            conn: Arc::new(Mutex::new(None)),
            reader_started: AtomicBool::new(false),
            reader_stop: Arc::new(AtomicBool::new(false)),
            active: Arc::new(AtomicBool::new(false)),
            action,
        };
        client.spawn_reader();
        client
    }

    /// 宿主 Activate：允许读线程主动连候选窗（让状态栏在首次按键前就能读到状态）。
    pub fn on_activate(&self) {
        self.active.store(true, Ordering::SeqCst);
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

    /// position：caret 位置（AWT 逻辑像素；发端 = `caret.rs` 的 `candidate_anchor`）。
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
            Ok(h) => {
                let handle = PipeHandle(h);
                // 非阻塞写：候选窗进程若一时不读，管道缓冲满时 WriteFile 会**无限
                // 阻塞**在宿主 UI 线程上（in-proc，用户的 Word/浏览器一起冻住）。
                // 写不进去就丢这一帧并断开，下一次 show 重连；候选窗下一帧会全量重发。
                let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
                if unsafe { SetNamedPipeHandleState(handle.0, Some(&mode), None, None) }.is_err() {
                    unsafe { CloseHandle(handle.0) }.ok();
                    return None;
                }
                Some(handle)
            }
            Err(_) => None, // 未启动/忙/不可重试：本次放弃
        }
    }

    /// 发送一条完整消息（自动补 '\n'）。连接失败 → 本次放弃（不睡眠）；写失败 → 断开待重连。
    ///
    /// **不在这里置 `active`**（2026-10-22 review）：`active` 的语义是「本宿主当前
    /// 是前台输入法」，唯一写入点是 `Activate`/`ThreadFocusSink::OnSetThreadFocus`。
    /// 旧代码在每次写消息时无条件置 true —— 失焦后若还有一次排队刷新（点候选后
    /// Alt-Tab），已失活的宿主会把读线程重新武装，50ms 内抢回单实例 `\\.\pipe\opi`，
    /// 新前台宿主永远连不上（候选窗/状态栏不出现）。
    ///
    /// 失活时**连惰性连接也不做**：排队刷新自己 connect 同样会把管道抢回来。检查
    /// 放在 conn 锁内 —— `disconnect()` 先置 false 再取同一把锁，锁内看到 true
    /// 的那次连接随后会被 disconnect 关掉；看到 false 则直接放弃。激活时
    /// `on_activate` 已先置 true，首帧 state 不受影响。
    fn send_json(&self, json: &str) {
        let bytes = [json.as_bytes(), b"\n"].concat();
        let mut conn = match self.conn.lock() {
            Ok(g) => g,
            Err(_) => return, // 中毒锁：吞掉（与 tsf.rs 的锁策略一致）
        };
        let handle = match *conn {
            Some(h) => h,
            None => {
                if !self.active.load(Ordering::SeqCst) {
                    return;
                }
                match Self::connect() {
                    Some(h) => {
                        *conn = Some(h);
                        h
                    }
                    None => return, // 窗口未启动：本次放弃，下次发送再试
                }
            }
        };
        let ok = write_all(handle, &bytes);
        if !ok {
            eprintln!("tsf-opi: candidate pipe write failed; disconnecting");
            // 写失败（管道另一端关闭等）：断开，下次发送重连。
            unsafe { CloseHandle(handle.0) }.ok();
            *conn = None;
        }
    }

    /// 断开当前连接（宿主 Deactivate 时调用）：关句柄让读线程退出本轮，
    /// 窗口服务端在 readLoop 的 finally 里隐藏并重建单实例管道。下次 show 惰性重连。
    ///
    /// 关闭方式与 `Drop` 相同（读线程每 20ms 醒一次，看到句柄没了就退本轮）；
    /// 不在这里 sleep/等确认 —— Deactivate 跑在宿主的 UI 线程上。
    pub fn disconnect(&self) {
        // 先停主动重连：否则读线程会在 Deactivate 后把单实例管道又抢回去，
        // 焦点切到的下一个应用就永远连不上（2026-10-21）。
        self.active.store(false, Ordering::SeqCst);
        if let Some(h) = self.conn.lock().ok().and_then(|mut g| g.take()) {
            // SAFETY: 句柄由本对象持有，take 后 write/show 不会再使用它。
            unsafe { CloseHandle(h.0) }.ok();
        }
    }

    /// 读线程（构造时启动一次）：轮询候选窗回复，'\n' 分帧解析，
    /// 回调 `CandidateAction`。连接失效 → 清句柄（仅当仍是本线程持有的那个），
    /// 下一轮退避重试 —— 骨架期容忍与写线程的句柄替换竞态。
    fn spawn_reader(&self) {
        if self.reader_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let conn = Arc::clone(&self.conn);
        let stop = Arc::clone(&self.reader_stop);
        let active = Arc::clone(&self.active);
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
                            // 激活期间主动连：第一次按键之前状态栏也能读到 state。
                            // 失败（窗口没起/被别的宿主占着）就退避，下一轮再试。
                            if active.load(Ordering::SeqCst)
                                && let Some(h) = Self::connect()
                            {
                                match conn.lock() {
                                    Ok(mut g) if g.is_none() => {
                                        // **装上句柄前再查一次 active**：disconnect() 先
                                        // 置 false 再取 conn 锁；若 connect 期间失焦，这里
                                        // 装回去就等于把管道从新前台宿主手里抢走。
                                        if active.load(Ordering::SeqCst) {
                                            *g = Some(h);
                                            continue;
                                        }
                                        drop(g);
                                        // SAFETY: 句柄刚由 connect 创建，尚未交给任何人。
                                        unsafe { CloseHandle(h.0) }.ok();
                                    }
                                    _ => {
                                        // 写线程抢先连上了：关掉多出来的这个。
                                        // SAFETY: 句柄刚由 connect 创建，尚未交给任何人。
                                        unsafe { CloseHandle(h.0) }.ok();
                                    }
                                }
                            }
                            // 失活时不必 50ms 醒一次：这条线程只在 active 时才该抢管道。
                            let ms = if active.load(Ordering::SeqCst) {
                                RETRY_SLEEP_MS
                            } else {
                                IDLE_SLEEP_MS
                            };
                            thread::sleep(Duration::from_millis(ms));
                            continue;
                        }
                    }
                };
                // 先 Peek：同步句柄上不能留一个阻塞 ReadFile，否则同句柄的
                // WriteFile 会被串行化（见 READ_POLL_MS）。
                let mut avail = 0u32;
                if unsafe { PeekNamedPipe(handle.0, None, 0, None, Some(&mut avail), None) }
                    .is_err()
                {
                    if !disconnect(&conn, handle) {
                        return;
                    }
                    pending.clear();
                    continue;
                }
                if avail == 0 {
                    thread::sleep(Duration::from_millis(READ_POLL_MS));
                    continue;
                }
                // 字节流模式读：Peek 已确认有数据，这次 ReadFile 不会阻塞。
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
        // 取走句柄（PipeHandle 是 Copy，take() 留 None）：读线程下一次轮询看到
        // 句柄为 None 或 stop 置位就退出（不会再有阻塞中的 ReadFile）。
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

// 候选窗回复的生产实现见 `state.rs` 的 `SharedAction`（随 ctl 处理一起搬过去，
// 那里已有引擎/文档写入的线程模型；本文件只管管道与分帧）。
// 候选窗接缝（TsfSink）的生产实现见 `candidate_sink.rs`：那边只管「引擎事件 →
// show/hide/position」的映射，连接与分帧仍在本文件。

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
