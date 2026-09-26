// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C2：TSF（Text Services Framework）COM 胶水层（Windows 目标专属，
//! lib.rs 以 `#[cfg(target_os = "windows")]` 引入本模块；Linux 主机不编译）。
//!
//! 结构镜像 fcitx5 轨（crates/fcitx5-opi）：纯 Rust 逻辑层 + 平台胶水分离；
//! 本文件是 Windows 侧的胶水，完成 C1 契约的对接：
//!   - TSF 键事件 wParam/lParam → `TsfLogic::input_key` 的 keyval/KEY_STATE_*；
//!   - 按 `KeyOutcome` 分派：Commit → 提交接缝；CompositionChanged → 刷新接缝；
//!     Consumed → 吞键（BOOL TRUE）；Unhandled → 不拦截（BOOL FALSE，键流入应用）。
//!
//! 【骨架 vs 功能】本文件按"最小可编译骨架"编写（对照 windows-rs 0.62 TSF 示例）：
//!   [功能] ITfTextInputProcessor 生命周期、ITfKeyEventSink 按键转发与键码映射、
//!          KeyOutcome 分派、AdviseKeyEventSink 注册、**提交文本真正插入文档**
//!          （`insert_text`：RequestEditSession + ITfEditSession::DoEditSession）。
//!   [骨架] composition（拼音缓冲）不做进文档 —— 缓冲显示在独立候选窗里
//!          （见 candidate_io.rs），故文档侧只有"插入"没有 composition 生命周期。
//!          候选窗 UI 与 `CandidateAction`（点击选词/翻页）仍未接线。
//!   [已补] COM 服务器导出与注册在 `com_server.rs`（类工厂 / DllRegisterServer）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, Once};

use engine_core::composer::Mode;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_MENU,
    VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection, ITfKeyEventSink,
    ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfTextInputProcessor, ITfTextInputProcessor_Impl,
    ITfThreadMgr, TF_ES_READWRITE, TF_ES_SYNC, TF_IAS_NOQUERY,
};
use windows::core::{
    BOOL, ComObjectInterface, GUID, HRESULT, IUnknown, Interface, InterfaceRef, Ref, Result,
    implement,
};

use crate::dll::DllLock;
use crate::logic::{
    KEY_STATE_ALT, KEY_STATE_CAPS_LOCK, KEY_STATE_CTRL, KEY_STATE_RELEASED, KEY_STATE_REPEAT,
    KEY_STATE_SHIFT, KeyOutcome, TsfLogic,
};
use crate::vk::vk_to_engine_keycode;

/// E_FAIL：引擎内部错误的统一返回码（panic 被兜住 / 锁中毒）。
const E_FAIL: HRESULT = HRESULT(0x80004005_u32 as i32);

/// 一次性 panic hook（对照 fcitx5-opi/src/lib.rs 的 `ensure_panic_hook`）。
/// 每个 COM 出口都要调：换掉默认 hook 后，宿主进程的事件日志里能看到是引擎炸了。
pub(crate) fn ensure_panic_hook() {
    static PANIC_HOOK: Once = Once::new();
    PANIC_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            eprintln!("tsf-opi: engine panic caught: {info}");
        }));
    });
}

/// 观察接缝：**不负责文档写入** —— 提交文本的插入由 `insert_text` 在本文件内
/// 用 COM 接口完成（那里才有 `ITfContext` 与 client id），本接缝只把"发生了什么"
/// 通知出去，供候选窗（C3 的 CMP 窗口）刷新。语义（对照 C1 契约）：
/// Commit → 隐藏候选窗；CompositionChanged → 用候选数据刷新候选窗。
/// 无状态变化时不回调。
pub trait TsfSink: Send + Sync {
    /// 立即提交文本（英文直传、空缓冲空格、缓冲/候选提交等）。
    fn on_commit(&self, _text: &str) {}
    /// composition/候选窗需刷新。
    fn on_composition_changed(
        &self,
        _buffer: &str,
        _candidates: &[String],
        _page: usize,
        _page_count: usize,
        _mode: Mode,
    ) {
    }
}

/// TSF 服务对象：单对象实现两个 COM 接口，避免跨接口共享状态。
/// - `ITfTextInputProcessor`：TSF 核心入口（Activate/Deactivate 生命周期）。
/// - `ITfKeyEventSink`：按键转发（Activate 中经 ITfKeystrokeMgr::AdviseKeyEventSink 注册）。
#[implement(ITfTextInputProcessor, ITfKeyEventSink)]
pub struct TsfTextService {
    /// 引擎 + 候选分页 + ⇧ 状态机（C1 逻辑层）。
    pub logic: Mutex<TsfLogic>,
    /// Activate 时保存的线程管理器（骨架：仅持有引用，候选窗/文档操作需要）。
    pub thread_mgr: Mutex<Option<ITfThreadMgr>>,
    /// Activate 收到的 TfClientId。`RequestEditSession` 的第一个参数就是它 ——
    /// 此前被丢弃，于是"想插入也没有合法的 tid 可用"。0 = TF_CLIENTID_NULL =
    /// 尚未 Activate（TSF 分配的是非零 id），故 0 同时充当"未激活"哨兵。
    client_id: AtomicU32,
    /// 候选窗接缝（见 `TsfSink`）。
    pub sink: Box<dyn TsfSink>,
    /// 模块级对象计数（见 `dll.rs`）：本对象活着 → `DllCanUnloadNow` 必须答
    /// S_FALSE。字段本身从不读写，作用全在 `Drop`（故下划线前缀）。
    _module_lock: DllLock,
    /// 未能**同步**完成的编辑会话的保活引用（唯一写入点是 `insert_text` 的
    /// `TF_E_SYNCHRONOUS` 分支，理由见那里）。只保最后一个 —— 见该分支注释。
    keepalive: Mutex<Option<ITfEditSession>>,
}

impl TsfTextService {
    /// 装载逻辑层（`TsfLogic::load`，失败用内置回退词库语义由逻辑层处理）。
    pub fn new(path: Option<&str>, sink: Box<dyn TsfSink>) -> Result<Self> {
        // E_FAIL：词库装载失败（坏路径）→ 服务不可用。
        let logic = TsfLogic::load(path).map_err(|_| HRESULT(0x80004005_u32 as i32))?;
        Ok(Self {
            logic: Mutex::new(logic),
            thread_mgr: Mutex::new(None),
            client_id: AtomicU32::new(0),
            sink,
            _module_lock: DllLock::new(),
            keepalive: Mutex::new(None),
        })
    }

    /// 键事件统一入口（OnKeyDown/OnKeyUp 共用）。
    /// wParam = VK 或 Unicode 码点（见 logic.rs 头注释的键码约定）；
    /// lParam 位映射 KEY_STATE_*；KeyOutcome 分派见模块头注释。
    /// `pic` = TSF 交来的当前文档 context（无焦点文档时为 None），
    /// 一路传到 `insert_text` —— 提交文本要插进它，别处拿不到。
    ///
    /// 这是个 `extern "system"` 的 COM 出口（经 vtable 被宿主输入法进程调用）：
    /// panic 逃出去 = 宿主进程 abort（in-proc，用户的 Word/浏览器）。
    /// 故与另三端（jni.rs / cabi.rs / fcitx5-opi）一致，出口一律兜住；
    /// 兜住后返回 BOOL(1)（吞键）——与中毒锁同策略：宁可丢一次键，不炸宿主。
    fn handle_key(&self, pic: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> BOOL {
        ensure_panic_hook();
        catch_unwind(AssertUnwindSafe(|| {
            self.handle_key_inner(pic, wparam, lparam)
        }))
        .unwrap_or(BOOL(1))
    }

    fn handle_key_inner(&self, pic: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> BOOL {
        let mut logic = match self.logic.lock() {
            Ok(g) => g,
            Err(_) => return BOOL(1), // 中毒锁：吞键，避免键流入应用造成死循环
        };
        let outcome = logic.input_key(to_engine_keycode(wparam.0 as u32), map_key_state(lparam));
        match outcome {
            KeyOutcome::Commit(text) => {
                // 顺序有意：先插入（此时 logic 锁还握着，但 insert_text 不碰 logic，
                // 见其重入警告），再通知接缝。反过来则"接缝炸了 → 字没插进去"。
                self.insert_text(pic, &text);
                self.sink.on_commit(&text);
                BOOL(1)
            }
            KeyOutcome::CompositionChanged => {
                let candidates = logic.candidates();
                self.sink.on_composition_changed(
                    &logic.buffer(),
                    &candidates,
                    logic.page(),
                    logic.page_count(),
                    logic.mode(),
                );
                BOOL(1)
            }
            KeyOutcome::Consumed => BOOL(1),
            KeyOutcome::Unhandled => BOOL(0), // 不拦截，键自然流入应用
        }
    }

    /// 把提交文本真正写进文档 —— `KeyOutcome::Commit` 的落地处。
    ///
    /// 三种"静默放弃"都是**正常情况**，不是错误路径：
    /// - `pic` 为 None：TSF 没给 context（无焦点文档），没有可插入的目标；
    /// - `client_id` 为 0：尚未 `Activate`，`RequestEditSession` 必失败；
    /// - `pic` 转不出 `ITfInsertAtSelection`：该 context 不支持插入。
    /// 放弃后按键仍被吞（`BOOL(1)`）：用户看到"这一下没出字"，而不是崩溃。
    /// 这三种不发声（它们是"本就没有目标"）；`RequestEditSession` 的失败则**要**
    /// 记一行 —— 那一类才是"本该出字却没出"，成因全在宿主进程里，没日志就查不动。
    ///
    /// **重入警告**：`TF_ES_SYNC` 会让 `DoEditSession` 在**本线程、本栈**上被回调，
    /// 且此刻我们正持有 `self.logic` 的锁（`handle_key_inner` 的 guard 还活着）。
    /// `InsertTextSession` 因此绝不能回头碰 `self.logic` / `self.sink`：
    /// `std::sync::Mutex` 不可重入，一碰就死锁在用户的 Word 里。
    /// 它只持有文本与插入接口，故安全 —— 改动它时务必守住这条。
    fn insert_text(&self, pic: Option<&ITfContext>, text: &str) {
        let Some(pic) = pic else { return };
        let tid = self.client_id.load(Ordering::Relaxed);
        if tid == 0 {
            return; // 未 Activate：没有合法 tid，RequestEditSession 必失败
        }
        let Ok(session) = InsertTextSession::new(pic, text) else {
            return;
        };
        let session: ITfEditSession = session.into();
        // SAFETY: pic 是 TSF 交来的活动 context；session 在本次调用期间存活。
        // 返回 `Result<HRESULT>` 是两层：外层 = 调用本身成不成，
        // 内层 = `phrSession`，也就是 **DoEditSession 的返回值**（0.62 把它当出参映射）。
        // 失败不改行为（"这次按键不出字"总好过"崩掉宿主进程"），但**必须留下痕迹**：
        // "字插不进去"的成因全在宿主进程里，没有这一行，用户与我们都只剩
        // "输入法就是不工作"。TF_E_SYNCHRONOUS（另有编辑会话在跑）就落在这里。
        match unsafe { pic.RequestEditSession(tid, &session, TF_ES_SYNC | TF_ES_READWRITE) } {
            Ok(hr) if hr.is_ok() => {}
            Ok(hr) => {
                eprintln!("tsf-opi: 编辑会话未同步完成 hr={hr:?}，保留引用待异步回调");
                // 这一支就是 TF_E_SYNCHRONOUS：按 TSF 文档是"转为异步排队"，也就是
                // DoEditSession 在**本次调用返回之后**才被回调 —— 那时 `session`
                // 已出作用域。TSF 理应自己 AddRef，但这条本机（无 Windows）无法验证，
                // 赌错的代价是"回调打到已释放对象"→ 崩在用户 Word 里，而留下的代价
                // 只是一个引用（下一次插入或本对象销毁时释放）。故留。
                // 锁在这里即取即放：本分支按定义不是同步回调，不会重入（见 insert_text
                // 的重入警告）；`keepalive` 也从不被 DoEditSession 碰到。
                if let Ok(mut keep) = self.keepalive.lock() {
                    *keep = Some(session);
                }
            }
            Err(e) => eprintln!("tsf-opi: RequestEditSession 失败: {e}"),
        }
    }
}

// ---------- 文档写入：编辑会话 ----------

/// 「把一段文本插到当前选区」的编辑会话。
///
/// 为什么必须绕这一道：TSF 不允许文本服务直接改文档。服务得先用
/// `ITfContext::RequestEditSession` 请求一个编辑会话，TSF 回调
/// `ITfEditSession::DoEditSession` 并把 edit cookie（`ec`）交给它 ——
/// 只有拿着 `ec` 才允许调用插入 API。`TF_ES_SYNC` 让这次回调同步发生
/// （否则 `RequestEditSession` 返回而我们还没插入，文本就丢了）。
///
/// 生命周期：本对象在 `insert_text` 里临时构造，调用返回后即释放（无人长期
/// 持有），故不占模块锁 —— 回调期间宿主一定还在我们的调用栈上，且服务对象
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
            // TF_IAS_NOQUERY = "插入，但不要返回范围"。**成功时 ppRange 被置 NULL**，
            // 而 windows-rs 把 NULL 出参转成 `Err(Error::empty())`（空 HRESULT）——
            // 直接 `?` 会把**成功**判成失败。故按 HRESULT 定成败：非零才是真错误。
            // 这是 0.62 生成代码的实际行为（windows-core/src/type.rs 的 from_abi）。
            match unsafe {
                self.insert
                    .InsertTextAtSelection(ec, TF_IAS_NOQUERY, &self.text)
            } {
                Ok(_) => Ok(()),
                Err(e) if e.code().is_ok() => Ok(()),
                Err(e) => Err(e),
            }
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }
}

// ---------- ITfTextInputProcessor：TSF 生命周期 ----------
// 注：0.62 的 #[implement] 生成 `TsfTextService_Impl` 包装（Deref 到原结构），
// `_Impl` trait 实现在包装类型上；字段经 Deref 访问（self.logic 等）。

impl ITfTextInputProcessor_Impl for TsfTextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        ensure_panic_hook();
        // COM 出口：panic 逃出去 = abort 宿主进程（见 handle_key）。兜住 → E_FAIL。
        catch_unwind(AssertUnwindSafe(|| {
            // 中毒锁：跳过状态保存，避免经 COM vtable 泄漏 panic
            let mut tm = match self.thread_mgr.lock() {
                Ok(g) => g,
                Err(_) => return Err(E_FAIL.into()),
            };
            *tm = ptim.cloned();
            // client id 是 RequestEditSession 的必需参数：不存下来，插入就没有
            // 合法的 tid 可用（这就是"两处 on_commit 丢弃文本"之外的第二道墙）。
            self.client_id.store(tid, Ordering::Relaxed);
            // 注册按键监听：0.62 API 为 AdviseKeyEventSink（旧式 SetKeypressSink 已移除）。
            // fforeground=true：前台键盘事件也交本服务（输入法语义）。
            // 本对象同时实现 ITfKeyEventSink，as_interface_ref 取其 IUnknown 指针，
            // TSF 侧会 QueryInterface 到 ITfKeyEventSink。
            let km: ITfKeystrokeMgr = (*ptim)
                .as_ref()
                .ok_or_else(|| windows::core::Error::from_hresult(HRESULT(0x80070057_u32 as i32)))?
                .cast()?; // E_INVALIDARG：ptim 为空
            let sink: InterfaceRef<'_, IUnknown> = self.as_interface_ref();
            // cast = QueryInterface：对象支持 ITfKeyEventSink，取具体接口指针。
            let key_sink: ITfKeyEventSink = sink.cast()?;
            unsafe { km.AdviseKeyEventSink(tid, &key_sink, true) }
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }

    fn Deactivate(&self) -> Result<()> {
        ensure_panic_hook();
        catch_unwind(AssertUnwindSafe(|| {
            // 骨架：仅清状态；验收补全点：UnadviseKeyEventSink + 释放 composition/候选窗。
            let mut tm = match self.thread_mgr.lock() {
                Ok(g) => g,
                Err(_) => return Ok(()), // 中毒锁：吞掉，避免经 COM vtable 泄漏 panic
            };
            *tm = None;
            // 失活后旧 tid 不再有效：清零，insert_text 会据此静默放弃。
            self.client_id.store(0, Ordering::Relaxed);
            Ok(())
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }
}

// ---------- ITfKeyEventSink：按键转发 ----------

impl ITfKeyEventSink_Impl for TsfTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Ref<ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        // 测试阶段不认领：让系统走正常 OnKeyDown 路径。
        Ok(BOOL(0))
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // pic 不再丢弃：提交文本要插进的正是这个 context（见 insert_text）。
        Ok(self.handle_key(pic.as_ref(), wparam, lparam))
    }

    fn OnKeyUp(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // 释放状态由 lParam bit31（转换状态）映射，与按下同路由
        // （logic 对释放事件返回 Consumed/Unhandled，见 input_key）。
        Ok(self.handle_key(pic.as_ref(), wparam, lparam))
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}

/// lParam 键状态位 → logic 的 KEY_STATE_*（位约定见 logic.rs 头注释）。
/// TSF lParam：bit30 = 按下前状态（1 = 重复），bit31 = 转换状态（1 = 释放）；
/// 修饰键（⇧/Ctrl/Alt/CapsLock）TSF 键事件不携带，用 GetKeyState 移位查询。
/// `ToUnicodeEx` 的 wFlags bit 2：本次调用不改变键盘状态。
/// 不加这个标志时，连续调用会在按下死键（重音等）时吞掉/推进死键状态，
/// 而那些键我们多半要放行给应用 —— 应用侧的组合键输入就会被吃掉。
const TO_UNICODE_NO_STATE_CHANGE: u32 = 0x4;

/// TSF 的 `wParam` 是**虚拟键码**，而 `TsfLogic::input_key` 的约定是
/// 「可打印字符 = Unicode 码点，特殊键 = VK 码」（见 logic.rs 头注释）。
/// 此前直接把 `wparam` 当码点透传，导致：
///   - 英文模式敲 'a' 得到 VK_A=0x41，被当成大写 'A' 提交 → "hello" 变 "HELLO"；
///   - 撇号 `'`（拼音分隔符）的 VK_OEM_7=0xDE 非 ASCII，永远 Unhandled，打不出来。
///
/// 拼音模式只是被 `Composer` 的「字母转小写」兜住才看起来正常，故一直未被发现。
///
/// 用 `ToUnicodeEx` 而非 `MapVirtualKeyW`：前者按**当前键盘布局**与修饰键状态
/// 换算，非 US 布局（AZERTY/Dvorak）下依然正确；后者只给未加修饰的字符，
/// 自行补 Shift/CapsLock 逻辑等于重造键盘布局。
fn to_engine_keycode(vk: u32) -> u32 {
    let mut key_state = [0u8; 256];
    let mut buf = [0u16; 8];
    // SAFETY: key_state 为 256 字节（Win32 要求的键盘状态数组大小）；
    // buf 长度经 cchbuff 如实传入，ToUnicodeEx 不会写出界。
    let n = unsafe {
        if GetKeyboardState(&mut key_state).is_err() {
            -1 // 取不到键盘状态：当"无映射"处理（判定与回退策略全在 vk_to_engine_keycode）
        } else {
            ToUnicodeEx(
                vk,
                0,
                &key_state,
                &mut buf,
                TO_UNICODE_NO_STATE_CHANGE,
                Some(GetKeyboardLayout(0)),
            )
        }
    };
    // 映射判定全在 crate::vk::vk_to_engine_keycode（主机可单测）：特殊键换成
    // 编码键码（SPECIAL_BASE|VK，查一次表即检测+转发）、有映射取码点、
    // 无映射/死键（n <= 0）返回放行哨兵而非 VK。
    vk_to_engine_keycode(vk, n, buf[0])
}

fn map_key_state(lparam: LPARAM) -> u32 {
    let lp = lparam.0 as u32;
    let mut s = (lp >> 3) & KEY_STATE_REPEAT; // bit30 → 1<<27
    s |= (lp >> 5) & KEY_STATE_RELEASED; // bit31 → 1<<26
    // GetKeyState 返回 i16：高位为 1 = 按下（负数）。
    if unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0 {
        s |= KEY_STATE_SHIFT;
    }
    if unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0 {
        s |= KEY_STATE_CTRL;
    }
    if unsafe { GetKeyState(VK_MENU.0 as i32) } < 0 {
        s |= KEY_STATE_ALT;
    }
    if unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0 {
        s |= KEY_STATE_CAPS_LOCK;
    }
    s
}

// COM 服务器导出（DllGetClassObject / DllCanUnloadNow / DllRegisterServer …）
// 在 `com_server.rs`：那里是"COM 如何加载与注册本 DLL"的整个面，本文件只管
// 服务对象自身的行为。
