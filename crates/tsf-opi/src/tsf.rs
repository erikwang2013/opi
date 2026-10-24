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
//!   [已接线] 候选窗点击选词/翻页/直插文本（符号页 `commit_text`）→ `candidate_io.rs`
//!          的 `CandidateAction`，插入目标由本文件每个键事件存进 `TsfSharedState`（B1）。
//!   [已补] COM 服务器导出与注册在 `com_server.rs`（类工厂 / DllRegisterServer）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfFnConfigure, ITfFnConfigure_Impl, ITfFunction, ITfFunction_Impl,
    ITfInputProcessorProfileActivationSink, ITfKeyEventSink, ITfKeystrokeMgr, ITfLangBarItem,
    ITfLangBarItemMgr, ITfSource, ITfTextInputProcessor, ITfTextInputProcessor_Impl,
    ITfThreadFocusSink, ITfThreadMgr,
};
use windows::core::{
    BOOL, BSTR, ComObjectInterface, GUID, HRESULT, IUnknown, Interface, InterfaceRef, Ref, Result,
    implement,
};

use self::profile_sink::ProfileSink;
use crate::dll::{DISPLAY_NAME, DllLock};
use crate::key_state::{map_key_state, to_engine_keycode};
use crate::logic::{KEY_STATE_RELEASED, KeyOutcome};
use crate::settings;
use crate::shift_tap::ShiftTap;
use crate::state::TsfSharedState;
use crate::thread_focus::ThreadFocusSink;
use crate::vk::{ModeHotkey, fullwidth_hotkey, hotkey_should_act, hotkey_target, mode_hotkey};

// 接缝/推送/出口公用件在 `sink.rs`（拆出守 500 行）；这里 re-export 保持原路径
// `crate::tsf::{TsfSink, E_FAIL, …}` 对其余模块不变。
pub use crate::sink::TsfSink;
pub(crate) use crate::sink::{E_FAIL, ensure_panic_hook, push_state};

/// TSF 服务对象：单对象实现两个 COM 接口，避免跨接口共享状态。
/// - `ITfTextInputProcessor`：TSF 核心入口（Activate/Deactivate 生命周期）。
/// - `ITfKeyEventSink`：按键转发（Activate 中经 ITfKeystrokeMgr::AdviseKeyEventSink 注册）。
// `ITfFunction` 是 `ITfFnConfigure` 的基接口，两个都要列（同 langbar.rs 的
// ITfLangBarItem/ITfLangBarItemButton 那一对）。
#[implement(ITfTextInputProcessor, ITfKeyEventSink, ITfFnConfigure, ITfFunction)]
pub struct TsfTextService {
    /// 引擎（与候选窗回调共用同一份 `EngineShared`）+ 插入目标 + 编辑会话保活。
    /// **本对象是它唯一的持有者**：后两样不 Send，只该待在按键线程上，
    /// 读线程那份引用走 `EngineShared`（见 `state.rs` 的线程模型）。
    pub state: TsfSharedState,
    /// Activate 时保存的线程管理器（骨架：仅持有引用，候选窗/文档操作需要）。
    pub thread_mgr: Mutex<Option<ITfThreadMgr>>,
    /// 语言栏按钮（`langbar.rs`）。挂上/摘下跟 Activate/Deactivate 配对 ——
    /// 不摘的话语言栏会持一份指向本 DLL 的 vtable。**挂不上不致命**：
    /// 语言栏没开（Win11 常态）就必然挂不上，那只意味着没那个图标，不该让
    /// 整个输入法激活不了 ⇒ 这里是 `Option`，失败只落空、不冒泡。
    pub langbar: Mutex<Option<(ITfLangBarItemMgr, ITfLangBarItem)>>,
    /// 候选窗接缝（见 `TsfSink`）。`Arc` 而非 `Box`：同一份还要交给候选窗回调
    /// （点选后得让窗口刷新/隐藏），两处各持一个引用，谁也不用等谁。
    pub sink: Arc<dyn TsfSink>,
    /// Shift 轻按判定（`crate::shift_tap`，镜像 fcitx5 轨 C++ 的 `OpiShiftToggle`）。
    /// **放在服务对象上而不是 `TsfLogic`**：Linux 轨的同一判定在 C++ glue 里，两轨的
    /// Rust 逻辑层必须逐事件同构（`crates/opi-ffi/tests/two_track_parity.rs` 压着）。
    /// 置位在 `OnTestKeyDown`，消费在 `OnKeyUp`（Shift 抬起）。
    shift_tap: Mutex<ShiftTap>,
    /// 线程焦点监听（`ITfThreadFocusSink`）：TSF 的 `Deactivate` **不随应用切换触发**，
    /// 只靠 Activate/Deactivate 的话，先激活过 opi 的后台宿主会一直占着单实例候选管道，
    /// 状态栏与候选窗都会认错前台应用（2026-10-21 报障：切走不消失、中文模式无候选）。
    /// 焦点进来 → 连接；焦点离开 → 断开，把管道交给下一个前台宿主。
    thread_focus: Mutex<Option<(ITfSource, u32, ITfThreadFocusSink)>>,
    /// 输入法**配置切换**监听（`profile_sink.rs`）：切到别的输入法时立即断开候选
    /// 管道，状态栏 1s 轮询读不到 `im=opi` 就会隐藏。`Deactivate` 之外的兜底。
    profile_focus: Mutex<Option<(ITfSource, u32, ITfInputProcessorProfileActivationSink)>>,
    /// 模块级对象计数（见 `dll.rs`）：本对象活着 → `DllCanUnloadNow` 必须答
    /// S_FALSE。字段本身从不读写，作用全在 `Drop`（故下划线前缀）。
    _module_lock: DllLock,
}

impl TsfTextService {
    /// `state` 与 `sink` 都由 `com_server.rs` **先建好再传进来** —— 顺序理由见
    /// `state.rs` 模块头（B1：回调要 state，服务对象要回调，谁先建都不成立，
    /// 故两者都不再依赖对方先存在）。
    ///
    /// 词库在这一步**之前**就解析完了（`dict_path::load_dict` 逐候选回退，最差是
    /// 内置 35 词）—— 故本构造没有失败路径：进到这里的一定是能用的引擎。
    /// （从前这里写的是"坏路径由 `EngineShared::load` 报错，调用方决定回退"，
    /// 那条路已随词库解析落地取消。）
    pub fn new(state: TsfSharedState, sink: Arc<dyn TsfSink>) -> Self {
        Self {
            state,
            thread_mgr: Mutex::new(None),
            langbar: Mutex::new(None),
            sink,
            shift_tap: Mutex::new(ShiftTap::new()),
            thread_focus: Mutex::new(None),
            profile_focus: Mutex::new(None),
            _module_lock: DllLock::new(),
        }
    }

    /// B0：模式热键。切完**立刻**刷一次候选窗 —— 不刷的话窗口还挂着上个模式的
    /// 候选（切到英文，窗口里还留着拼音的汉字），用户会以为键没生效。
    fn toggle_mode(&self, hot: ModeHotkey) -> BOOL {
        // 锁内只改引擎 + 抓快照；管道写在锁外（理由见 `push_state`）。
        let snapshot = {
            let Some(mut logic) = self.state.lock_logic() else {
                return BOOL(1); // 中毒锁：吞键，与 handle_key_inner 同策略
            };
            let target = hotkey_target(hot, logic.mode());
            // 切走/切英文的收尾（W1 订正版：拼音/繁体上原文、符号模式丢关键字）在
            // `logic.switch_mode_confirmed` —— 本函数走 `switch_mode`（丢缓冲）。
            // 接线 = 改调它并把返回值插入文档（2026-10-10 裁决：状态机语义先立住，
            // 插入面未接线如实注明）。
            logic.switch_mode(target);
            logic.snapshot()
        };
        push_state(&*self.sink, &snapshot);
        self.sink.on_state_changed(); // 模式/语言变了 → 状态栏立刻补读
        BOOL(1)
    }

    /// 候选窗回复（点选 / 直插文本）的**唯一执行点**：读线程只把 index/文本排进
    /// 队列（它拿不到 `ITfContext` —— 那是本线程的 COM 接口指针，见 state.rs 模块头），
    /// 真正走 COM 的插入在这里、在按键线程上跑。
    ///
    /// **已知代价：点到下一次按键之间才插入**。即时插入的两条升级路径（都要真机）
    /// 写在 state.rs 模块头。一次按键最多清空队列（各上限 16 条）。
    ///
    /// 已接受的一个边缘：点了候选后**没敲键就换文档**，插入会落到上一个文档
    /// （目标取自上一次键事件）。这是"排回按键线程"的固有代价 —— 队列里只有一个
    /// index/文本，没有可判定的新旧依据；换文档时的正确行为需要真机才能定。
    fn drain_pending(&self) {
        while let Some(index) = self.state.take_pending_selection() {
            if self.state.select_and_insert(index).is_none() {
                continue; // 越界/空缓冲：引擎没变，窗口无需刷新
            }
            // 提交后缓冲已空 → sink 内部走 hide 分支，窗口自己收起来。
            // 锁只包住抓快照这一步：insert_into 早已返回，管道写在锁外。
            let snapshot = self.state.lock_logic().map(|logic| logic.snapshot());
            if let Some(snapshot) = snapshot {
                push_state(&*self.sink, &snapshot);
            }
        }
        // 候选窗「直接插入」（commit_text：符号页多选）：插文档但**不动引擎、不通知
        // sink**（`on_commit` 会收起窗口，而符号页要留在屏幕上连续点选；裁决
        // 2026-10-10，见 candidate_io.rs 协议头）。与上面同受"延迟到下一次按键"的约束。
        while let Some(text) = self.state.take_pending_text() {
            self.state.insert_text(&text);
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
        let keycode = to_engine_keycode(wparam.0 as u32);
        // 记下插入目标：候选窗点选走的是 named pipe 读线程，那条路上没有任何
        // ITfContext，只有这里存下的一份可用。抬起事件不记（它带来的 context
        // 已不是当前焦点文档，记了会把文本插到上一个窗口里）。
        if states & KEY_STATE_RELEASED == 0 {
            self.state.remember_target(pic);
        }
        // Shift 抬起（2026-10-22）：轻按切中英 + 清掉引擎里的 Single；细节在
        // `tsf_shift.rs` 的 handle_shift_release_event（拆出去守 500 行）。
        if let Some(done) = self.handle_shift_release_event(states, keycode, pic) {
            return done;
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
            {
                let mut logic = match self.state.lock_logic() {
                    Some(g) => g,
                    None => return BOOL(1), // 中毒锁：同下，吞键
                };
                logic.toggle_fullwidth();
                // 不刷候选面板：这个开关只改**后续**的标点映射，不动缓冲也不动候选。
            }
            self.sink.on_state_changed(); // 状态栏的全/半角图标要立刻翻
            return BOOL(1);
        }
        // 锁内只跑引擎并抓快照，**立刻放锁**。后面三件都可能在宿主 UI 线程上阻塞：
        // `insert_into` / `candidate_anchor` 是同步 COM 编辑会话（会重入本线程），
        // `sink` 是管道写（候选窗卡住就写不动）。持着 logic 锁跨它们 = 真机卡死
        // （2026-10-22 报障「按功能键后卡死、只能输入英文」）。
        let (outcome, snapshot) = {
            let mut logic = match self.state.lock_logic() {
                Some(g) => g,
                None => return BOOL(1), // 中毒锁：吞键，避免键流入应用造成死循环
            };
            let outcome = logic.input_key(keycode, states);
            let snapshot = match outcome {
                KeyOutcome::CompositionChanged => Some(logic.snapshot()),
                _ => None,
            };
            (outcome, snapshot)
        };
        match outcome {
            KeyOutcome::Commit(text) => {
                // 顺序有意：先插入，再通知接缝。反过来则"接缝炸了 → 字没插进去"。
                // pic 为 None = 无焦点文档，没有可插入的目标：静默跳过，照常吞键。
                if let Some(pic) = pic {
                    self.state.insert_into(pic, &text);
                }
                self.sink.on_commit(&text);
                BOOL(1)
            }
            KeyOutcome::CompositionChanged => {
                // 2026-10-22 用户报障「候选词框要挨着输入的位置」：先发 caret 位置、
                // 再发 show —— 窗口在新位置出现，不会先画在旧位置再跳一下。
                // 拿不到位置就不发（窗口沿用上一次位置/默认落点），不阻断按键。
                //
                // 优化（2026-10-22）：缓冲为空（退格清空 / 切模式）时 `push_state`
                // 马上会 hide，取 caret 是白花一次同步 COM 编辑会话 —— 跳过。
                if let Some(snapshot) = snapshot {
                    if snapshot.composing
                        && let Some(pic) = pic
                        && let Some((x, y)) =
                            crate::caret::candidate_anchor(pic, self.state.client_id())
                    {
                        self.sink.on_caret(x, y);
                    }
                    push_state(&*self.sink, &snapshot);
                }
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
            // 让候选窗客户端主动连接：状态栏的 state 查询依赖这条连接，用户切到 opi
            // 后即使还没敲键，状态栏也应出现（2026-10-21 报障「没有状态栏」）。
            self.sink.on_activate();
            // 「属性设置」重读一次。在线重载走控制面 `CtlOp::Reload`（设置窗保存时），
            // 这里是候选进程不在时的兜底（下一次激活生效）。读不到 = 保持/回默认，
            // 见 settings.rs；导入词库在同一次里重装，见 extra_dicts.rs。
            if let Some(mut logic) = self.state.lock_logic() {
                let lap = std::env::var_os("LOCALAPPDATA");
                let dicts = crate::dict_path::dicts_dir(lap.as_deref());
                logic.apply_settings(settings::load(lap.as_deref()), &dicts);
                logic.apply_custom_phrases(crate::custom_phrases::load(&dicts));
                logic.apply_english_custom_phrases(crate::custom_phrases::load_english(&dicts));
            }
            // 注册按键监听：0.62 API 为 AdviseKeyEventSink（旧式 SetKeypressSink 已移除）。
            // fforeground=true：前台键盘事件也交本服务（输入法语义）。
            // 本对象同时实现 ITfKeyEventSink，as_interface_ref 取其 IUnknown 指针，
            // TSF 侧会 QueryInterface 到 ITfKeyEventSink。
            let tmgr = (*ptim).as_ref().ok_or_else(|| {
                windows::core::Error::from_hresult(HRESULT(0x80070057_u32 as i32))
            })?;
            let km: ITfKeystrokeMgr = tmgr.cast()?; // E_INVALIDARG：ptim 为空
            let sink: InterfaceRef<'_, IUnknown> = self.as_interface_ref();
            // cast = QueryInterface：对象支持 ITfKeyEventSink，取具体接口指针。
            let key_sink: ITfKeyEventSink = sink.cast()?;
            if let Err(e) = unsafe { km.AdviseKeyEventSink(tid, &key_sink, true) } {
                eprintln!(
                    "tsf-opi: AdviseKeyEventSink failed tid={tid} hr=0x{:08X}",
                    e.code().0 as u32
                );
                return Err(e);
            }
            // 线程焦点监听：候选管道跟随**前台线程**（见 thread_focus 字段注释）。
            if let Ok(source) = tmgr.cast::<ITfSource>() {
                let focus: ITfThreadFocusSink = ThreadFocusSink::new(Arc::clone(&self.sink)).into();
                match unsafe { source.AdviseSink(&ITfThreadFocusSink::IID, &focus) } {
                    Ok(cookie) => {
                        if let Ok(mut slot) = self.thread_focus.lock() {
                            // 重复 Activate：先摘掉上一份，绝不留悬挂的 Advise。
                            if let Some((old_src, old_cookie, _)) = slot.take() {
                                let _ = unsafe { old_src.UnadviseSink(old_cookie) };
                            }
                            *slot = Some((source, cookie, focus));
                        }
                    }
                    Err(e) => eprintln!("tsf-opi: AdviseSink(thread focus) failed: {e}"),
                }
            }
            // 输入法配置切换监听：切到别的输入法（同一应用内也算）时断开管道。
            // `Deactivate` 不保证每次切换都触发，只靠它状态栏会留在屏幕上。
            if let Ok(profile) = ProfileSink::new(Arc::clone(&self.sink))
                && let Ok(source) = tmgr.cast::<ITfSource>()
            {
                let profile: ITfInputProcessorProfileActivationSink = profile.into();
                match unsafe {
                    source.AdviseSink(&ITfInputProcessorProfileActivationSink::IID, &profile)
                } {
                    Ok(cookie) => {
                        if let Ok(mut slot) = self.profile_focus.lock() {
                            if let Some((old_src, old_cookie, _)) = slot.take() {
                                let _ = unsafe { old_src.UnadviseSink(old_cookie) };
                            }
                            *slot = Some((source, cookie, profile));
                        }
                    }
                    Err(e) => eprintln!("tsf-opi: AdviseSink(profile activation) failed: {e}"),
                }
            }
            // 语言栏按钮（宠物「小欧」在 Windows 上唯一新增的图标位，见 langbar.rs）。
            // 失败**只落空、不冒泡**：语言栏没开（Win11 默认）就挂不上，而那是常态 ——
            // 一个图标不值得让整个输入法激活不了。摘除在 Deactivate。
            if let Some(tm) = (*ptim).as_ref()
                && let Ok(pair) = crate::langbar::add(tm)
                && let Ok(mut slot) = self.langbar.lock()
            {
                *slot = Some(pair);
            }
            Ok(())
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }

    fn Deactivate(&self) -> Result<()> {
        ensure_panic_hook();
        catch_unwind(AssertUnwindSafe(|| {
            // Shift 轻按复位：按住 Shift 时失活/切走，抬起事件可能到不了本服务，
            // 不撤销就是回来后的「幽灵切换」（镜像 `OpiShiftToggle::disarm`）。
            self.shift_tap_disarm();
            // 语言栏按钮先摘（与 Activate 配对）：留着会持指向本 DLL 的 vtable —— 见 langbar::remove。
            if let Ok(mut slot) = self.langbar.lock()
                && let Some((mgr, item)) = slot.take()
            {
                crate::langbar::remove(&mgr, &item);
            }
            // 摘掉线程焦点监听（与 Activate 的 AdviseSink 配对）。
            if let Ok(mut slot) = self.thread_focus.lock()
                && let Some((source, cookie, _)) = slot.take()
            {
                let _ = unsafe { source.UnadviseSink(cookie) };
            }
            // 摘掉配置切换监听（配对 Activate；不留悬挂 vtable）。
            if let Ok(mut slot) = self.profile_focus.lock()
                && let Some((source, cookie, _)) = slot.take()
            {
                let _ = unsafe { source.UnadviseSink(cookie) };
            }
            self.sink.disconnect(); // 释放单实例管道，焦点切走后下一个应用才能连上。
            let mut tm = match self.thread_mgr.lock() {
                Ok(g) => g,
                Err(_) => return Ok(()), // 中毒锁：吞掉，避免经 COM vtable 泄漏 panic
            };
            // 成对摘掉按键监听（Activate 里 AdviseKeyEventSink 的配对）。TSF 不会替我们
            // 摘：不摘的话线程管理器仍持一份指向本对象的引用，下一次 Activate 再 Advice
            // 就是同一 tid 上两份 —— 一次按键回调两次（模式热键会连切两下）。
            let tid = self.state.client_id();
            if tid != 0
                && let Some(tmgr) = tm.as_ref()
                && let Ok(km) = tmgr.cast::<ITfKeystrokeMgr>()
            {
                // 0.62 的签名只收 tid（同一 ITfKeystrokeMgr 上、同一 tid 的那份监听）。
                // SAFETY: tid 正是 Activate 交给同一个 ITfKeystrokeMgr 的值；失败
                // （例如 TSF 已自行摘除）只需忽略。
                let _ = unsafe { km.UnadviseKeyEventSink(tid) };
            }
            *tm = None;
            // 失活后旧 tid 不再有效：清零，insert_into 会据此静默放弃。
            self.state.set_client_id(0);
            Ok(())
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }
}

// ITfKeyEventSink 的五个回调在 `key_sink.rs`（拆出守 500 行）；`OnTestKeyDown`
// 的认领判定在 `logic_claim.rs`，其 TRUE 是最终吞键决定。
//
// ITfThreadFocusSink 的实现（焦点 → 候选管道）在 `thread_focus.rs`：拆出守 500 行。
// 本文件只负责在 Activate/Deactivate 里配对 AdviseSink/UnadviseSink。

// ---------- ITfFunction / ITfFnConfigure：控制面板里的「属性设置」 ----------
// 取法见 configure.rs 的模块头（控制面板 `CoCreateInstance(CLSID, IID_ITfFnConfigure)`）。

impl ITfFunction_Impl for TsfTextService_Impl {
    /// 「属性」这一项在控制面板里显示的名字。
    fn GetDisplayName(&self) -> Result<BSTR> {
        Ok(DISPLAY_NAME.into())
    }
}

impl ITfFnConfigure_Impl for TsfTextService_Impl {
    /// 点「属性」→ 拉起设置窗。失败返回 E_FAIL：控制面板知道失败了，
    /// 而用户不会被弹一个空窗骗（宁可不响）。
    fn Show(&self, _hwndparent: HWND, _langid: u16, _rguidprofile: *const GUID) -> Result<()> {
        ensure_panic_hook();
        // COM 出口：panic 逃出去 = abort 宿主进程（同 handle_key）。
        match catch_unwind(AssertUnwindSafe(crate::configure::launch_settings)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                eprintln!("tsf-opi: 拉起设置窗失败: {e}");
                Err(E_FAIL.into())
            }
            Err(_) => Err(E_FAIL.into()),
        }
    }
}

// 键码/键状态映射（`to_engine_keycode` / `map_key_state`）在 `key_state.rs`：
// 从本文件拆出只为守住 500 行，二者都是 Win32 取数，逻辑判定仍在 `vk.rs`。

// Shift 轻按的三拍（OnTestKeyDown 武装 / OnKeyUp 消费 / 失活复位）在 `tsf_shift.rs`：
// 从本文件拆出守 500 行；与 Linux 轨 C++ 的 OpiShiftToggle 对称，见那边模块头。
#[path = "tsf_shift.rs"]
mod shift_glue;

#[path = "key_sink.rs"]
mod key_sink;

// 输入法配置切换监听（切走立即释放候选管道）在 profile_sink.rs。
#[path = "profile_sink.rs"]
mod profile_sink;

// COM 服务器导出（DllGetClassObject / DllCanUnloadNow / DllRegisterServer …）
// 在 `com_server.rs`：那里是"COM 如何加载与注册本 DLL"的整个面，本文件只管
// 服务对象自身的行为。
