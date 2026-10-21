// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 引擎状态 + 文档写入：候选窗回调（`CandidateAction`）与服务对象
//! （`TsfTextService`）**共用的那一份**。
//!
//! 【B1 的顺序问题在此解开】候选窗回复的回调 `Arc<dyn CandidateAction>` 必须在
//! `TsfTextService` **之前**建好（它要作为构造参数传进去），而回调要用到引擎状态
//! 与文档写入目标 —— 原先两者都是服务对象的私有字段，于是谁先建都不成立。
//! 解法：把状态摘出来，回调持 `Arc<EngineShared>`（引擎），服务对象另持
//! `TsfSharedState`（引擎 + 插入目标）；插入目标**不进** `Arc`，理由见下面的线程模型。
//!
//! 从 `tsf.rs` 拆出来还有第二个理由：那份文件按 500 行硬规矩已经贴线，而这一块
//! （状态 + 编辑会话）自成一体的 —— 只依赖 `logic` 与 COM 的插入 API，不碰 TSF 的
//! 键事件与生命周期。
//!
//! 【线程模型：候选窗的回复从哪来，插入在哪跑】
//! 候选窗回复来自 named pipe 的**读线程**（`candidate_io.rs` 的 `spawn_reader`），
//! 而文档写入要用 `ITfContext` —— 那是 TSF 在按键回调里交给我们的 COM 接口指针，
//! **不 Send**，按 COM 规矩只能在它所属的套间（宿主 UI 线程 = 按键线程）上用。
//!
//! 于是状态切成两半，让读线程**够不着**不该碰的那半：
//! - `EngineShared`：引擎 + 点选队列。字段只有 `Mutex`，`Send + Sync` 是编译器
//!   自己推出来的（**没有 `unsafe impl`**），读线程拿的就是这一半；
//! - `TsfSharedState` 里剩下的 `ITfContext` / 编辑会话只挂在按键线程持有的一份上。
//!
//! 读线程点选只 `enqueue_select(index)` 排队，按键线程在下一个键事件开头
//! `take_pending_selection()` 取走执行。**已知代价：插入延迟到下一次按键**，
//! 换来零跨线程 COM 调用。要即时插入时的升级路径（二选一，都得真机才验得了）：
//! (a) 按键线程建一个 message-only 窗口，读线程 `PostMessage` 过去在 UI 线程执行；
//! (b) 按 GIT（`IGlobalInterfaceTable`）注册 cookie，读线程取 marshal 过的代理。
//!
//! 【本机（Linux）无法验证的部分，别当成已验】上面整条排队路径本机一行都跑不了
//! （无 Windows）：真机验收**第一件事就是点一次候选、再敲一个键**，看文本有没有
//! 落进文档、宿主进程是否还在。没插入 → 看 `insert_into` 打出的那行 HRESULT。

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use windows::Win32::UI::TextServices::{
    INSERT_TEXT_AT_SELECTION_FLAGS, ITfContext, ITfEditSession, ITfEditSession_Impl,
    ITfInsertAtSelection, TF_ES_READWRITE, TF_ES_SYNC,
};
use windows::core::{Interface, Result, implement};

use crate::candidate_io::CandidateAction;
use crate::ctl::{CtlReply, CtlRequest};
use crate::logic::TsfLogic;
use crate::tsf::{E_FAIL, TsfSink, ensure_panic_hook, push_state};

/// 队列上限。对端是**另一个进程**（候选窗）：不设上限的话它连发 select 就能
/// 让宿主进程的内存一直涨。16 远超人手点击速度，超过即丢弃（读线程不阻塞）。
const MAX_PENDING_SELECT: usize = 16;

/// 「直接插入文本」队列上限（理由同 `MAX_PENDING_SELECT`：对端是另一个进程）。
const MAX_PENDING_TEXT: usize = 16;

/// 引擎 + 点选队列：**候选窗读线程够得着的那一半**。
///
/// `Send + Sync` 是编译器自己推的（字段只有 `Mutex`）—— 这个类型里**不允许**
/// 出现 COM 接口指针，见模块头的线程模型。想往这加字段前先确认它是 `Send`。
pub struct EngineShared {
    logic: Mutex<TsfLogic>,
    /// 读线程排下的点选（页内 index），由按键线程取走执行（见 `enqueue_select`）。
    pending: Mutex<VecDeque<usize>>,
    /// 读线程排下的「直接插入文本」（候选窗 `commit_text`，不经引擎选词），
    /// 由按键线程取走执行（见 `enqueue_commit_text`）。
    pending_text: Mutex<VecDeque<String>>,
}

impl EngineShared {
    /// 装载逻辑层（`TsfLogic::load`）。坏路径 → Err（**不回退**，回退由调用方决定）。
    pub fn load(path: Option<&str>) -> std::result::Result<Self, String> {
        Ok(Self::with_dict_inner(TsfLogic::load(path)?))
    }

    /// 用**已装好的**词库构建（`dict_path::load_dict` 的落点，调用点见 `com_server`）。
    /// 与 `load` 的差别是没有失败路径：词库解析与回退已在 `dict_path` 里做完，
    /// 这里拿到的词典一定可用（最差是内置 35 词）。
    pub fn with_dict(dict: Box<dyn engine_core::dictionary::Dictionary>) -> Self {
        Self::with_dict_inner(TsfLogic::with_dict(dict))
    }

    fn with_dict_inner(logic: TsfLogic) -> Self {
        Self {
            logic: Mutex::new(logic),
            pending: Mutex::new(VecDeque::new()),
            pending_text: Mutex::new(VecDeque::new()),
        }
    }

    /// 取引擎锁。中毒 → None（调用方吞掉这次操作，别经 COM vtable 泄漏 panic）。
    ///
    /// 调用方**不得在持锁期间碰 `target` 或走 COM**：见类型头的锁序说明。
    pub fn lock_logic(&self) -> Option<MutexGuard<'_, TsfLogic>> {
        self.logic.lock().ok()
    }

    /// 候选窗翻页箭头 → 引擎翻页（真源在引擎，窗口自己不存页码）。
    pub fn next_page(&self) {
        if let Some(mut l) = self.lock_logic() {
            l.next_page();
        }
    }

    /// 见 `next_page`。
    pub fn prev_page(&self) {
        if let Some(mut l) = self.lock_logic() {
            l.prev_page();
        }
    }

    // ---------- 候选窗点选：读线程入队，按键线程执行 ----------

    /// **读线程**调：收下这次点选，**不做任何 COM 调用**（理由见模块头）。
    /// 返回 false = 队列满（对端刷屏）或锁中毒 —— 都是丢这一次点选，不阻塞读线程。
    pub fn enqueue_select(&self, index: usize) -> bool {
        match self.pending.lock() {
            Ok(mut q) if q.len() < MAX_PENDING_SELECT => {
                q.push_back(index);
                true
            }
            _ => false,
        }
    }

    /// **按键线程**调：取一个待执行的点选。None = 队列空。
    pub fn take_pending_selection(&self) -> Option<usize> {
        self.pending.lock().ok()?.pop_front()
    }

    /// **读线程**调（候选窗 `commit_text`）：收下这段要直插文档的文本，**不做任何
    /// COM 调用**（理由同 `enqueue_select`：插入目标不在这半）。返回 false = 队列满
    /// 或锁中毒 —— 丢这一次，不阻塞读线程。
    pub fn enqueue_commit_text(&self, text: String) -> bool {
        match self.pending_text.lock() {
            Ok(mut q) if q.len() < MAX_PENDING_TEXT => {
                q.push_back(text);
                true
            }
            _ => false,
        }
    }

    /// **按键线程**调：取一段待直插的文本。None = 队列空。
    pub fn take_pending_text(&self) -> Option<String> {
        self.pending_text.lock().ok()?.pop_front()
    }
}

/// 按键线程独占的那一半：插入目标 + 编辑会话保活 + tid。
///
/// **锁序：先 `logic`，后 `target`。** `select_and_insert` 先取 logic（选词）
/// 再取 target（拿 context），反着取就是死锁。`keepalive` 只在 `insert_into`
/// 里即取即放，不参与嵌套。
///
/// 这个类型**不 Send**（`ITfContext` / `ITfEditSession` 是 COM 接口指针），
/// 而它按设计就该如此：跨线程用未 marshal 的 STA 接口指针在 COM 语义上是
/// 未定义的，所以它只跟着按键线程的服务对象走，读线程一行都碰不到。
pub struct TsfSharedState {
    /// 读线程那一半（`SharedAction` 也持一份，故是 `Arc`）。
    engine: Arc<EngineShared>,
    /// `RequestEditSession` 的第一个参数。0 = `TF_CLIENTID_NULL` = 尚未 Activate
    /// （TSF 分配的是非零 id），故 0 同时充当「未激活」哨兵。
    client_id: AtomicU32,
    /// 最后一次键事件带来的插入目标。候选窗点选排回按键线程执行时，
    /// `ITfContext` 只有键事件回调给得了 —— 不在这里存一份，点选就没有目标。
    target: Mutex<Option<ITfContext>>,
    /// 最近一次编辑会话的保活引用（唯一写入点在 `insert_into`，理由见那里：
    /// 异步回调可能在本次调用返回之后才到）。只保最后一个，下一次插入替换。
    keepalive: Mutex<Option<ITfEditSession>>,
}

impl TsfSharedState {
    /// `engine` 由调用方先建好：候选窗回调（读线程）要单独持一份，见 `com_server.rs`。
    pub fn new(engine: Arc<EngineShared>) -> Self {
        Self {
            engine,
            client_id: AtomicU32::new(0),
            target: Mutex::new(None),
            keepalive: Mutex::new(None),
        }
    }

    /// 见 `EngineShared::lock_logic`。
    pub fn lock_logic(&self) -> Option<MutexGuard<'_, TsfLogic>> {
        self.engine.lock_logic()
    }

    /// **按键线程**调：取一个待执行的点选（见 `EngineShared::take_pending_selection`）。
    pub fn take_pending_selection(&self) -> Option<usize> {
        self.engine.take_pending_selection()
    }

    /// **按键线程**调：取一段待直插的文本（见 `EngineShared::take_pending_text`）。
    pub fn take_pending_text(&self) -> Option<String> {
        self.engine.take_pending_text()
    }

    /// Activate 记下的 tid；Deactivate 传 0（旧 tid 不再有效，插入会据此静默放弃）。
    pub fn set_client_id(&self, tid: u32) {
        self.client_id.store(tid, Ordering::Relaxed);
    }

    /// 当前 tid（0 = 未激活）。Deactivate 用它成对摘掉按键监听，见 `tsf.rs`。
    pub fn client_id(&self) -> u32 {
        self.client_id.load(Ordering::Relaxed)
    }

    /// `settings.ini` 的 `ShiftToggle`（2026-10-22：Windows 读侧接通）。给胶水的
    /// `OnTestKeyUp`（关掉时**不认领** Shift 抬起）与三拍判定用。锁中毒 → false
    /// （不认领、不切换：宁可让键流入应用，也不在未知状态下改用户输入法模式）。
    pub fn shift_toggle(&self) -> bool {
        self.lock_logic().map(|l| l.shift_toggle()).unwrap_or(false)
    }

    /// 记下本次键事件的插入目标（每个非抬起键事件调一次）。
    pub fn remember_target(&self, pic: Option<&ITfContext>) {
        let next = pic.cloned();
        if let Ok(mut g) = self.target.lock() {
            *g = next;
        }
    }

    // ---------- 候选窗回复的执行点（**按键线程**，见模块头） ----------

    /// 点选第 `index`（**页内** 0 起）候选：选词 → 插文档。
    /// 返回提交的文本；`None` = 没提交（越界/空缓冲/锁中毒）。
    pub fn select_and_insert(&self, index: usize) -> Option<String> {
        // 先取文本、**先放锁**：`insert_into` 会阻塞在宿主的同步编辑会话上，
        // 攥着引擎锁等它，按键线程会跟着一起卡住。
        let text = self.lock_logic()?.select(index);
        if text.is_empty() {
            return None;
        }
        // 同样不持 target 锁去走 COM：克隆一份（AddRef）就够了。
        let pic = self.target.lock().ok().and_then(|g| g.clone());
        if let Some(pic) = pic {
            self.insert_into(&pic, &text);
        }
        Some(text)
    }

    /// 候选窗 `commit_text` 的**执行点**（按键线程）：把文本直插文档，**不经引擎** ——
    /// 符号页多选：缓冲/候选/学习都不动，故也**不通知 sink**（`on_commit` 会让候选窗
    /// 收起，而符号页要留在屏幕上供连续点选；裁决 2026-10-10，见 candidate_io.rs 协议头）。
    /// 与 `select_and_insert` 同一条下沉路径（`insert_into`），只是没有逻辑层那一步。
    /// 无插入目标（无焦点文档/未在键事件中记过 target）→ 静默放弃，与既有路径同策略。
    pub fn insert_text(&self, text: &str) {
        let pic = self.target.lock().ok().and_then(|g| g.clone());
        if let Some(pic) = pic {
            self.insert_into(&pic, text);
        }
    }

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
    pub fn insert_into(&self, pic: &ITfContext, text: &str) {
        let tid = self.client_id.load(Ordering::Relaxed);
        if tid == 0 {
            return; // 未 Activate：没有合法 tid，RequestEditSession 必失败
        }
        let Ok(session) = InsertTextSession::new(pic, text) else {
            eprintln!("tsf-opi: InsertTextSession::new failed (no ITfInsertAtSelection?)");
            return;
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
            Ok(hr) if hr.is_ok() => {}
            Ok(hr) => {
                eprintln!(
                    "tsf-opi: RequestEditSession async hr=0x{:08X}，保留引用待异步回调",
                    hr.0
                );
                // 这一支就是 TF_E_SYNCHRONOUS：按 TSF 文档是"转为异步排队"，
                // DoEditSession 在**本次调用返回之后**才被回调。keepalive 已在
                // 调用前无条件存了一份，这里不再重复。
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
                    eprintln!("tsf-opi: InsertTextAtSelection failed: {e}");
                    Err(e)
                }
            }
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }
}

/// 候选窗回复的**生产实现**（B1 接线）：点选 → 交给按键线程执行（选词 → 插文档）；
/// 翻页 → 引擎翻页（真源在引擎，窗口自己不存页码）；ctl → `ctl::execute`。
///
/// 两个引用为什么都得在这：
/// - `engine`：本对象跑在 named pipe 的**读线程**上，只能持有 `Send` 的那一半
///   （`EngineShared`）；`ITfContext` 在 `TsfSharedState` 里，读线程碰不到
///   —— 跨线程用未 marshal 的 STA 接口指针是未定义的，故点选/上屏只入队；
/// - `sink`：动作做完得让窗口**跟着变**（翻页后换页）。可
///   `CandidateSink::new` 又要本回调当参数 —— 互相等待的死结，用一次性槽位
///   （`OnceLock`）打破：先建本对象（槽位空着），再建 sink，最后回填。
///   接线三步在 `com_server.rs`。
///
/// 【本机（Linux）无法验证】点选/ctl 上屏的真正执行点在按键线程（`tsf.rs` 的
/// `drain_pending`），本机跑不了；代价是插入延迟到下一次按键。见 state.rs 模块头。
pub struct SharedAction {
    engine: Arc<EngineShared>,
    sink: OnceLock<Arc<dyn TsfSink>>,
}

impl SharedAction {
    pub fn new(engine: Arc<EngineShared>) -> Self {
        Self {
            engine,
            sink: OnceLock::new(),
        }
    }

    /// 回填 sink（`com_server.rs` 建好 `CandidateSink` 后立刻调，此后只读）。
    /// 已填过 → false：那只可能是重复初始化，覆盖会把正在用的一份换掉
    /// （读线程可能正读着它）。返回 false 让调用方看得见，不静默。
    pub fn attach_sink(&self, sink: Arc<dyn TsfSink>) -> bool {
        self.sink.set(sink).is_ok()
    }

    /// 动作改变了引擎状态 → 让候选窗跟上（翻页后换页）。sink 未回填 → 跳过：
    /// 只有启动那一瞬，那时也还没窗口。
    fn refresh(&self) {
        let Some(sink) = self.sink.get() else { return };
        let Some(logic) = self.engine.lock_logic() else {
            return;
        };
        push_state(sink.as_ref(), &logic);
    }
}

impl CandidateAction for SharedAction {
    /// **读线程**上跑：只入队，不碰引擎也不走 COM（理由见类型头）。
    /// 执行与随之而来的窗口刷新都在按键线程（`tsf.rs` 的 `drain_pending`）——
    /// 这里刷是白刷：此刻引擎还没选词，缓冲一个字节都没变。
    fn on_select(&self, index: usize) {
        self.engine.enqueue_select(index);
    }

    fn on_next_page(&self) {
        self.engine.next_page();
        self.refresh();
    }

    fn on_prev_page(&self) {
        self.engine.prev_page();
        self.refresh();
    }

    /// 同 `on_select`：读线程只入队（COM 插入在按键线程执行，见模块头的线程模型）。
    /// 执行后**不刷新窗口**：直插不动引擎状态，而符号页要留在屏幕上供连续点选。
    fn on_commit_text(&self, text: &str) {
        self.engine.enqueue_commit_text(text.to_string());
    }

    /// **读线程**上同步执行（只持引擎锁，不碰 COM）。上屏文本走既有队列，
    /// 由下一个按键事件落地——与点选同一条 W1 延迟语义。动作改了引擎态才刷新窗口。
    fn on_ctl(&self, request: &CtlRequest) -> Option<String> {
        let settings = crate::ctl::default_settings_path();
        let dicts = crate::ctl::default_dicts_dir();
        let (reply, changed) = {
            let mut logic = self.engine.lock_logic()?;
            let reply = crate::ctl::execute(&mut logic, &request.op, settings.as_deref(), &dicts);
            let changed = matches!(&reply, Ok(CtlReply::Action { .. } | CtlReply::Reload));
            (reply, changed)
        };
        if let Ok(
            CtlReply::Action {
                commit: Some(text), ..
            }
            | CtlReply::Commit(text),
        ) = &reply
        {
            self.engine.enqueue_commit_text(text.clone());
        }
        if changed {
            self.refresh();
        }
        Some(crate::ctl::reply_json(request.id, reply))
    }
}
