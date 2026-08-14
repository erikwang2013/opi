//! C3：候选窗通信（Windows 目标专属；lib.rs 以 `#[cfg(target_os = "windows")]`
//! 引入本模块，Linux 主机不编译 —— 与 tsf.rs 同构的双重隔离）。
//!
//! 【线协议：NDJSON over named pipe】（与 desktop/src/main/kotlin/io/opi/candidate/Main.kt
//! 头注释互为镜像，改协议须同步两处）
//!
//! ```text
//! 管道名：\\.\pipe\opi-candidates（单实例，字节流模式，'\n' 分帧，UTF-8）
//! 角色：候选窗（CMP desktop/）为 SERVER 监听管道；TSF 插件进程为 CLIENT，
//!       窗口启动后连接（窗口不在 → 客户端退避重试，不报错）。
//!
//! TSF(CLIENT) → 候选窗(SERVER)：
//!   show      {"type":"show","buffer":"ni","candidates":["你","尼",...],
//!              "page":1,"page_count":3,"mode":"pinyin"}
//!             （page/page_count 为 1 起；candidates 为当前页文本，页内 0 起）
//!   hide      {"type":"hide"}
//!   position  {"type":"position","x":120,"y":340}
//!             （可选项：caret 提示，窗口跟随光标；骨架无 caret 数据 → 固定位置）
//!
//! 候选窗(SERVER) → TSF(CLIENT)：
//!   select    {"type":"select","index":0}    // 用户点击第 index（页内 0 起）候选
//!   next_page {"type":"next_page"} / {"type":"prev_page"}
//! ```
//!
//! 本模块是纯胶水（镜像 tsf.rs 结构）：经 `TsfSink` 接缝（on_commit /
//! on_composition_changed）把候选数据序列化为 show/hide/position 发给候选窗；
//! 逻辑层（`TsfLogic`）零改动。`CandidateAction` 是候选窗回复（select/翻页）
//! 的回调接缝，默认 no-op —— 真机验收时接入 TSF 文档操作（logic.select /
//! next_page / prev_page，见 logic.rs 对应方法）。
//!
//! 连接语义（骨架）：惰性连接 + 静默退避重试（不阻塞 TSF 按键线程太久）；
//! 写失败即断开、下次发送时重连；读线程阻塞读回复并分帧解析。
//! 已知简化（验收可加固）：句柄替换与读线程的竞态以“仅清除仍指向自身句柄”
//! 的方式容忍；重连场景罕见（候选窗重启），骨架期不做 DuplicateHandle。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use engine_core::composer::Mode;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_MODE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

use crate::tsf::TsfSink;

/// 管道句柄的 Send+Sync 包装：windows-rs 的 HANDLE 是裸指针（非 Sync），
/// 而读写跨线程使用（按键线程写 + 读线程阻塞读），此处显式承诺线程安全。
/// 句柄自身不可拷贝到别的进程，跨线程共享同一连接是安全的。
#[derive(Clone, Copy, PartialEq, Eq)]
struct PipeHandle(HANDLE);
unsafe impl Send for PipeHandle {}
unsafe impl Sync for PipeHandle {}

/// 候选窗管道名（窗口侧 = desktop/ 的 JNA 服务器端，两侧保持一致）。
const PIPE_NAME: PCWSTR = w!(r"\\.\pipe\opi-candidates");
/// 每次发送前最多重试连接次数（窗口未启动时退避，避免阻塞按键线程）。
const CONNECT_RETRIES: u32 = 4;
/// ERROR_PIPE_BUSY / 窗口未启动时的重试间隔。
const RETRY_SLEEP_MS: u64 = 50;
/// 读线程缓冲区（单条消息远小于此；字节流模式按 '\n' 分帧）。
const READ_BUF: usize = 4096;

/// 候选窗回复回调接缝：select/翻页 → 真机验收接入 TSF 文档操作。
/// 默认全部 no-op（骨架期窗口可点击，但提交/翻页待验收接线）。
pub trait CandidateAction: Send + Sync {
    fn on_select(&self, _index: usize) {}
    fn on_next_page(&self) {}
    fn on_prev_page(&self) {}
}

/// 候选窗回复回调的默认 no-op 实现（骨架）。
struct NoopAction;
impl CandidateAction for NoopAction {}

/// named pipe 客户端：惰性连接 + 写消息 + 读线程解析候选窗回复。
pub struct CandidateClient {
    /// 当前连接句柄（None = 未连接）。Arc 共享给读线程。
    conn: Arc<Mutex<Option<PipeHandle>>>,
    reader_started: AtomicBool,
    action: Arc<dyn CandidateAction>,
}

impl CandidateClient {
    /// `action`：候选窗回复回调（Arc 共享给读线程）。
    pub fn new(action: Arc<dyn CandidateAction>) -> Self {
        let client = Self {
            conn: Arc::new(Mutex::new(None)),
            reader_started: AtomicBool::new(false),
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

    /// 尝试连接（窗口未启动/忙 → 退避重试；其他错误放弃）。
    fn connect() -> Option<PipeHandle> {
        for _ in 0..CONNECT_RETRIES {
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
                Ok(h) => return Some(PipeHandle(h)),
                Err(e) => {
                    // e.code() 为 HRESULT（i32），与 WIN32_ERROR（u32）比较需转 i32。
                    let code = e.code().0;
                    if code != ERROR_FILE_NOT_FOUND.0 as i32 && code != ERROR_PIPE_BUSY.0 as i32 {
                        return None; // 不可重试的错误：放弃本次
                    }
                    thread::sleep(Duration::from_millis(RETRY_SLEEP_MS));
                }
            }
        }
        None
    }

    /// 发送一条完整消息（自动补 '\n'）。连接失败 → 静默重试；写失败 → 断开待重连。
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
        let mut done = 0usize;
        while done < bytes.len() {
            let mut written = 0u32;
            // 0.62 的 WriteFile 为切片签名，返回 Result；部分写需循环。
            let ok = unsafe {
                WriteFile(
                    handle.0,
                    Some(&bytes[done..]),
                    Some(&mut written),
                    None,
                )
            };
            if ok.is_err() || written == 0 {
                // 写失败（管道另一端关闭等）：断开，下次发送重连。
                unsafe { CloseHandle(handle.0) }.ok();
                *conn = None;
                return;
            }
            done += written as usize;
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
        let action = Arc::clone(&self.action);
        thread::spawn(move || {
            let mut buf = vec![0u8; READ_BUF];
            let mut pending: Vec<u8> = Vec::new(); // '\n' 分帧残留（跨读合并）
            loop {
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
                let ok = unsafe {
                    ReadFile(handle.0, Some(&mut buf), Some(&mut got), None)
                };
                if ok.is_err() || got == 0 {
                    let mut guard = match conn.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    if *guard == Some(handle) {
                        unsafe { CloseHandle(handle.0) }.ok();
                        *guard = None;
                    }
                    pending.clear();
                    continue;
                }
                pending.extend_from_slice(&buf[..got as usize]);
                while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = pending.drain(..pos).collect();
                    pending.drain(..1); // 去掉 '\n'
                    dispatch_line(&line, action.as_ref());
                }
            }
        });
    }
}

/// 解析一行回复并分发到 `CandidateAction`（未知 type 忽略）。
fn dispatch_line(line: &[u8], action: &dyn CandidateAction) {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else {
        return;
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("select") => {
            if let Some(i) = v.get("index").and_then(|i| i.as_u64()) {
                action.on_select(i as usize);
            }
        }
        Some("next_page") => action.on_next_page(),
        Some("prev_page") => action.on_prev_page(),
        _ => {}
    }
}

/// 候选窗接缝的生产默认实现：把 composition 变化映射为 show/hide。
pub struct CandidateSink {
    client: CandidateClient,
}

impl CandidateSink {
    pub fn new(action: Arc<dyn CandidateAction>) -> Self {
        Self {
            client: CandidateClient::new(action),
        }
    }

    /// 默认 no-op 回复处理（骨架）。
    pub fn new_default() -> Self {
        Self::new(Arc::new(NoopAction))
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
            self.client.show(buffer, candidates, page + 1, page_count, mode);
        }
    }
}

/// Mode → 协议字符串（与 desktop/ 解析端一致）。
/// Traditional 复用 "pinyin"：候选窗（desktop/Main.kt modeLabel）无简繁概念，
/// 未知字符串一律回落 "拼音" 标签，新增 "traditional" 只会造成协议漂移。
fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Pinyin | Mode::Traditional => "pinyin",
        Mode::English => "english",
        Mode::Number => "number",
        Mode::Symbol => "symbol",
    }
}
