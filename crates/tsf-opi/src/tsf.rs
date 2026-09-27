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
//!          （`state.rs` 的 `insert_into`：RequestEditSession + ITfEditSession::DoEditSession）。
//!   [骨架] composition（拼音缓冲）不做进文档 —— 缓冲显示在独立候选窗里
//!          （见 candidate_io.rs），故文档侧只有"插入"没有 composition 生命周期。
//!   [已接线] 候选窗点击选词/翻页 → `candidate_io.rs` 的 `CandidateAction`，
//!          插入目标由本文件每个键事件存进 `TsfSharedState`（B1）。
//!   [已补] COM 服务器导出与注册在 `com_server.rs`（类工厂 / DllRegisterServer）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, Once};

use engine_core::composer::Mode;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_MENU,
    VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfThreadMgr,
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
use crate::state::TsfSharedState;
use crate::vk::{
    ModeHotkey, fullwidth_hotkey, hotkey_should_act, hotkey_target, mode_hotkey,
    vk_to_engine_keycode,
};

/// E_FAIL：引擎内部错误的统一返回码（panic 被兜住 / 锁中毒）。
/// `pub(crate)`：`state.rs` 的编辑会话出口要用同一个码。
pub(crate) const E_FAIL: HRESULT = HRESULT(0x80004005_u32 as i32);

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

/// 观察接缝：**不负责文档写入** —— 提交文本的插入由 `state.rs` 的 `insert_into`
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

/// 把当前引擎状态推给候选窗。服务对象（按键/切模式）与候选窗回调（点选后刷新，
/// 见 candidate_io.rs 的 `SharedAction::refresh`）共用 —— 两处各写一遍必然漂移，
/// 症状是"切了模式/点了候选，窗口还显示旧内容"。
pub(crate) fn push_state(sink: &dyn TsfSink, logic: &TsfLogic) {
    sink.on_composition_changed(
        &logic.buffer(),
        &logic.candidates(),
        logic.page(),
        logic.page_count(),
        logic.mode(),
    );
}
/// TSF 服务对象：单对象实现两个 COM 接口，避免跨接口共享状态。
/// - `ITfTextInputProcessor`：TSF 核心入口（Activate/Deactivate 生命周期）。
/// - `ITfKeyEventSink`：按键转发（Activate 中经 ITfKeystrokeMgr::AdviseKeyEventSink 注册）。
#[implement(ITfTextInputProcessor, ITfKeyEventSink)]
pub struct TsfTextService {
    /// 引擎（与候选窗回调共用同一份 `EngineShared`）+ 插入目标 + 编辑会话保活。
    /// **本对象是它唯一的持有者**：后两样不 Send，只该待在按键线程上，
    /// 读线程那份引用走 `EngineShared`（见 `state.rs` 的线程模型）。
    pub state: TsfSharedState,
    /// Activate 时保存的线程管理器（骨架：仅持有引用，候选窗/文档操作需要）。
    pub thread_mgr: Mutex<Option<ITfThreadMgr>>,
    /// 候选窗接缝（见 `TsfSink`）。`Arc` 而非 `Box`：同一份还要交给候选窗回调
    /// （点选后得让窗口刷新/隐藏），两处各持一个引用，谁也不用等谁。
    pub sink: Arc<dyn TsfSink>,
    /// 模块级对象计数（见 `dll.rs`）：本对象活着 → `DllCanUnloadNow` 必须答
    /// S_FALSE。字段本身从不读写，作用全在 `Drop`（故下划线前缀）。
    _module_lock: DllLock,
}

impl TsfTextService {
    /// `state` 与 `sink` 都由 `com_server.rs` **先建好再传进来** —— 顺序理由见
    /// `state.rs` 模块头（B1：回调要 state，服务对象要回调，谁先建都不成立，
    /// 故两者都不再依赖对方先存在）。
    ///
    /// 词库装载失败（坏路径）由 `EngineShared::load` 报错，调用方决定回退。
    pub fn new(state: TsfSharedState, sink: Arc<dyn TsfSink>) -> Self {
        Self {
            state,
            thread_mgr: Mutex::new(None),
            sink,
            _module_lock: DllLock::new(),
        }
    }

    /// B0：模式热键。切完**立刻**刷一次候选窗 —— 不刷的话窗口还挂着上个模式的
    /// 候选（切到英文，窗口里还留着拼音的汉字），用户会以为键没生效。
    fn toggle_mode(&self, hot: ModeHotkey) -> BOOL {
        let Some(mut logic) = self.state.lock_logic() else {
            return BOOL(1); // 中毒锁：吞键，与 handle_key_inner 同策略
        };
        let target = hotkey_target(hot, logic.mode());
        logic.switch_mode(target);
        push_state(&*self.sink, &logic);
        BOOL(1)
    }

    /// 候选窗点选的**唯一执行点**：读线程只把 index 排进队列（它拿不到
    /// `ITfContext` —— 那是本线程的 COM 接口指针，见 state.rs 模块头），
    /// 真正走 COM 的插入在这里、在按键线程上跑。
    ///
    /// **已知代价：点到下一次按键之间才插入**。即时插入的两条升级路径（都要真机）
    /// 写在 state.rs 模块头。一次按键最多清空队列（上限 16 条）。
    ///
    /// 已接受的一个边缘：点了候选后**没敲键就换文档**，插入会落到上一个文档
    /// （目标取自上一次键事件）。这是"排回按键线程"的固有代价 —— 队列里只有一个
    /// index，没有可判定的新旧依据；换文档时的正确行为需要真机才能定。
    fn drain_pending(&self) {
        while let Some(index) = self.state.take_pending_selection() {
            if self.state.select_and_insert(index).is_none() {
                continue; // 越界/空缓冲：引擎没变，窗口无需刷新
            }
            // 提交后缓冲已空 → sink 内部走 hide 分支，窗口自己收起来。
            // 锁在这里即取即放：insert_into 早已返回（见它的重入警告）。
            if let Some(logic) = self.state.lock_logic() {
                push_state(&*self.sink, &logic);
            }
        }
    }

    /// 键事件统一入口（OnKeyDown/OnKeyUp 共用）。
    /// wParam = VK 或 Unicode 码点（见 logic.rs 头注释的键码约定）；
    /// lParam 位映射 KEY_STATE_*；KeyOutcome 分派见模块头注释。
    /// `pic` = TSF 交来的当前文档 context（无焦点文档时为 None），
    /// 一路传到 `insert_into` —— 提交文本要插进它，别处拿不到。
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
        // 候选窗点选排回本线程执行。位置是**两个约束夹出来的**，别挪：
        // - 必须在 `input_key` 之前：用户"点一下候选、再打字"的意图顺序才是这样，
        //   反过来新键先改了缓冲，排着的 index 就选到别的词上了；
        // - 必须在 `remember_target` 之前：那一句在 `pic` 为 None（焦点已不在文档，
        //   比如刚点过候选窗）时会**清空**目标，排在它后面就成了"点了没反应"。
        self.drain_pending();
        let states = map_key_state(lparam);
        // 记下插入目标：候选窗点选走的是 named pipe 读线程，那条路上没有任何
        // ITfContext，只有这里存下的一份可用。抬起事件不记（它带来的 context
        // 已不是当前焦点文档，记了会把文本插到上一个窗口里）。
        if states & KEY_STATE_RELEASED == 0 {
            self.state.remember_target(pic);
        }
        // B0 模式热键要判在引擎**之前**：引擎的按键路径里 Ctrl+符号多半被判
        // 放行给应用（对照 fcitx5 轨 input_method.rs 的 Ctrl/Alt 直通），
        // 进去就出不来了。
        if let Some(hot) = mode_hotkey(wparam.0 as u32, states) {
            // 按住不放的重复事件：**认领但不动作**（重复再切一次 = 按住期间模式
            // 疯狂来回切）。认领这一步不能省成"放行"：放回引擎后 Ctrl+' 会撞上
            // Ctrl 直通落到应用，而它的抬手已被本函数吃掉（应用收到无 keydown
            // 的 keydown）。同 fcitx5 轨 handleModeHotkey，判据见 hotkey_should_act。
            if !hotkey_should_act(states) {
                return BOOL(1);
            }
            return self.toggle_mode(hot);
        }
        // 全角 ⇄ 半角，同一条理由也判在引擎之前：Shift+Space 送进引擎就是普通空格
        // （逻辑层的 KEY_SPACE 分支不看 Shift 位），会被当成"选首候选"。
        if fullwidth_hotkey(wparam.0 as u32, states) {
            // 重复同理（见上）：Shift+Space 的重复放行会被引擎的 KEY_SPACE 分支
            // 当成"选首候选"（那个分支不看 Shift 位），所以只认领不翻转。
            if !hotkey_should_act(states) {
                return BOOL(1);
            }
            let mut logic = match self.state.lock_logic() {
                Some(g) => g,
                None => return BOOL(1), // 中毒锁：同下，吞键
            };
            logic.toggle_fullwidth();
            // 不刷候选面板：这个开关只改**后续**的标点映射，不动缓冲也不动候选
            // （`Engine::toggle_fullwidth` 只翻一个 bool）。刷了是白推一帧。
            return BOOL(1);
        }
        let mut logic = match self.state.lock_logic() {
            Some(g) => g,
            None => return BOOL(1), // 中毒锁：吞键，避免键流入应用造成死循环
        };
        let outcome = logic.input_key(to_engine_keycode(wparam.0 as u32), states);
        match outcome {
            KeyOutcome::Commit(text) => {
                // 顺序有意：先插入（此时 logic 锁还握着，但 insert_into 不碰 logic，
                // 见其重入警告），再通知接缝。反过来则"接缝炸了 → 字没插进去"。
                // pic 为 None = 无焦点文档，没有可插入的目标：静默跳过，照常吞键。
                if let Some(pic) = pic {
                    self.state.insert_into(pic, &text);
                }
                self.sink.on_commit(&text);
                BOOL(1)
            }
            KeyOutcome::CompositionChanged => {
                push_state(&*self.sink, &logic);
                BOOL(1)
            }
            KeyOutcome::Consumed => BOOL(1),
            KeyOutcome::Unhandled => BOOL(0), // 不拦截，键自然流入应用
        }
    }
}

// ---------- ITfTextInputProcessor：TSF 生命周期 ----------
// 注：0.62 的 #[implement] 生成 `TsfTextService_Impl` 包装（Deref 到原结构），
// `_Impl` trait 实现在包装类型上；字段经 Deref 访问（self.state 等）。

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
            self.state.set_client_id(tid);
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
            // 失活后旧 tid 不再有效：清零，insert_into 会据此静默放弃。
            self.state.set_client_id(0);
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

    fn OnTestKeyDown(&self, _pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // 模式热键必须在这里就认领。OnTestKeyDown 是 TSF 问"这键你要不要"的第一
        // 道关：答 FALSE，键会先流进应用（宿主里 Ctrl+' 是别的功能），随后我们在
        // OnKeyDown 里再吞已经晚了 —— 应用那一下已经发生。其余键仍不认领。
        Ok(BOOL(
            (mode_hotkey(wparam.0 as u32, map_key_state(lparam)).is_some()
                || fullwidth_hotkey(wparam.0 as u32, map_key_state(lparam))) as i32,
        ))
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // pic 不再丢弃：提交文本要插进的正是这个 context（见 insert_into）。
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
