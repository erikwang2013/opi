// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 文档写入：提交文本真正落进文档的唯一实现（从 `state.rs` 拆出守 500 行，2026-10-28）。
//!
//! 两条路径（键事件提交、候选窗点选/直插）都汇到 `TsfSharedState::insert_into` ——
//! 两份实现必然漂移，一处补了下面的保活另一处就会漏。线程模型见 `state.rs` 模块头：
//! 本文件全部只在按键线程上跑（`ITfContext` 不在读线程那一半）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::Ordering;

use windows::Win32::UI::TextServices::{
    INSERT_TEXT_AT_SELECTION_FLAGS, ITfContext, ITfEditSession, ITfEditSession_Impl,
    ITfInsertAtSelection, TF_ES_READWRITE, TF_ES_SYNC,
};
use windows::core::{Interface, Result, implement};

use crate::state::TsfSharedState;
use crate::tsf::{E_FAIL, ensure_panic_hook};

impl TsfSharedState {
    /// 把提交文本真正写进文档 —— 提交的**唯一下沉路径**（键事件与候选窗点选
    /// 都走这里；两份实现必然漂移，一处补了下面的保活另一处就会漏）。
    ///
    /// 两种「静默放弃」都是**正常情况**，不是错误路径：
    /// - `client_id` 为 0：尚未 `Activate`，`RequestEditSession` 必失败；
    /// - `pic` 转不出 `ITfInsertAtSelection`：该 context 不支持插入。
    ///
    /// 放弃后按键仍被吞（调用方返 `BOOL(1)`）：用户看到「这一下没出字」，而不是崩溃。
    /// 这两种不发声（它们是「本就没有目标」）；`RequestEditSession` 的失败则**要**
    /// 记一行 —— 那一类才是「本该出字却没出」，成因全在宿主进程里，没日志查不动。
    ///
    /// **重入警告**：`TF_ES_SYNC` 会让 `DoEditSession` 在**本线程、本栈**上被回调，
    /// 且此刻调用方（`tsf.rs` 的 `handle_key_inner`）正持有 `logic` 的锁。
    /// `InsertTextSession` 因此绝不能回头碰 `logic` / `sink`：
    /// `std::sync::Mutex` 不可重入，一碰就死锁在用户的 Word 里。
    /// 它只持有文本与插入接口，故安全 —— 改动它时务必守住这条。
    /// 返回是否已把插入交给 TSF（false = 无目标/请求被拒）。调用方（切模式/Shift
    /// 切语言的 W1 收尾）据此在文字无处可去时至少刷新候选窗，别停在旧模式。
    pub fn insert_into(&self, pic: &ITfContext, text: &str) -> bool {
        let tid = self.client_id.load(Ordering::Relaxed);
        if tid == 0 {
            return false; // 未 Activate：没有合法 tid，RequestEditSession 必失败
        }
        let Ok(session) = InsertTextSession::new(pic, text) else {
            tlog!("tsf-opi: InsertTextSession::new failed (no ITfInsertAtSelection?)");
            return false;
        };
        let session: ITfEditSession = session.into();
        // 2026-10-21 真机：提交 "你好" 时 notepad.exe 崩在 textinputframework.dll。
        // 先把会话引用存进 keepalive（不依赖 RequestEditSession 的返回码），
        // 排除「异步回调时 session 已释放」这一条；下一次插入才替换。
        if let Ok(mut keep) = self.keepalive.lock() {
            *keep = Some(session.clone());
        }
        // SAFETY: pic 是 TSF 交来的活动 context；session 在本次调用期间存活。
        // 返回 `Result<HRESULT>` 是两层：外层 = 调用本身成不成，
        // 内层 = `phrSession`，也就是 **DoEditSession 的返回值**（0.62 把它当出参映射）。
        // 失败不改行为（"这次不出字"总好过"崩掉宿主进程"），但**必须留下痕迹**：
        // "字插不进去"的成因全在宿主进程里，没有这一行，用户与我们都只剩
        // "输入法就是不工作"。TF_E_SYNCHRONOUS（另有编辑会话在跑）就落在这里。
        match unsafe { pic.RequestEditSession(tid, &session, TF_ES_SYNC | TF_ES_READWRITE) } {
            Ok(hr) if hr.is_ok() => true,
            Ok(hr) => {
                tlog!(
                    "tsf-opi: RequestEditSession async hr=0x{:08X}，保留引用待异步回调",
                    hr.0
                );
                // 这一支就是 TF_E_SYNCHRONOUS：按 TSF 文档是"转为异步排队"，
                // DoEditSession 在**本次调用返回之后**才被回调。keepalive 已在
                // 调用前无条件存了一份，这里不再重复。
                true
            }
            Err(e) => {
                tlog!("tsf-opi: RequestEditSession 失败: {e}");
                false
            }
        }
    }
}

/// 「把一段文本插到当前选区」的编辑会话。
///
/// 为什么必须绕这一道：TSF 不允许文本服务直接改文档。服务得先用
/// `ITfContext::RequestEditSession` 请求一个编辑会话，TSF 回调
/// `ITfEditSession::DoEditSession` 并把 edit cookie（`ec`）交给它 ——
/// 只有拿着 `ec` 才允许调用插入 API。`TF_ES_SYNC` 让这次回调同步发生
/// （否则 `RequestEditSession` 返回而我们还没插入，文本就丢了）。
///
/// 生命周期：本对象在 `insert_into` 里构造；同步会话在调用返回后即可释放，
/// 异步会话（`TF_E_SYNCHRONOUS`）则可能在返回之后才被回调，故 `insert_into`
/// 无条件把最新一份存进 `keepalive`。同步路径下那只是一份引用，下一次插入或
/// 服务对象销毁时释放 —— 回调期间宿主一定还在我们的调用栈上，且服务对象
/// 自己持有模块锁，DLL 不会被抽掉。
///
/// 用 `ITfInsertAtSelection` 而不是 `GetSelection` + `ITfRange::SetText`：
/// 前者是 TSF 为此场景提供的 API（换行、选区替换、插入点后移都由它处理），
/// 而 `TF_SELECTION.range` 是 `ManuallyDrop<Option<ITfRange>>` —— 手工取用
/// 时漏一次 `into_inner` 就是一次引用计数泄漏，正是本项目备忘里那条
/// "COM 生命周期漏了会崩在用户 Word 里"。
#[implement(ITfEditSession)]
struct InsertTextSession {
    /// 目标 context 的插入接口（构造时 QueryInterface 得到，随本对象一起释放）。
    insert: ITfInsertAtSelection,
    /// UTF-16 文本（TSF 全线 UTF-16）。**不带结尾 NUL** —— 长度由切片传。
    text: Vec<u16>,
}

impl InsertTextSession {
    /// `pic` 不支持 `ITfInsertAtSelection`（非文档 context）→ Err，调用方放弃。
    fn new(pic: &ITfContext, text: &str) -> Result<Self> {
        Ok(Self {
            insert: pic.cast()?,
            text: text.encode_utf16().collect(),
        })
    }
}

impl ITfEditSession_Impl for InsertTextSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        ensure_panic_hook();
        // COM 出口（TSF 经 vtable 回调）：panic 逃出去 = abort 宿主进程。
        catch_unwind(AssertUnwindSafe(|| {
            // 2026-10-21 真机：TF_IAS_NOQUERY 下 notepad 崩在 textinputframework.dll。
            // 改为查询范围（flags=0）：API 写出非空 ppRange，走 windows-rs 的正常路径
            // （NOQUERY 时 out 参数为 NULL，0.62 的 from_abi 会把它转成 Err）。
            match unsafe {
                self.insert
                    .InsertTextAtSelection(ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), &self.text)
            } {
                Ok(_) => Ok(()),
                Err(e) if e.code().is_ok() => Ok(()),
                Err(e) => {
                    tlog!("tsf-opi: InsertTextAtSelection failed: {e}");
                    Err(e)
                }
            }
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }
}
