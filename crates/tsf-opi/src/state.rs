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
use std::sync::{Arc, Mutex, MutexGuard};

use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection, TF_ES_READWRITE,
    TF_ES_SYNC, TF_IAS_NOQUERY,
};
use windows::core::{Interface, Result, implement};

use crate::logic::TsfLogic;
use crate::tsf::{E_FAIL, ensure_panic_hook};

/// 队列上限。对端是**另一个进程**（候选窗）：不设上限的话它连发 select 就能
/// 让宿主进程的内存一直涨。16 远超人手点击速度，超过即丢弃（读线程不阻塞）。
const MAX_PENDING_SELECT: usize = 16;

/// 引擎 + 点选队列：**候选窗读线程够得着的那一半**。
///
/// `Send + Sync` 是编译器自己推的（字段只有 `Mutex`）—— 这个类型里**不允许**
/// 出现 COM 接口指针，见模块头的线程模型。想往这加字段前先确认它是 `Send`。
pub struct EngineShared {
    logic: Mutex<TsfLogic>,
    /// 读线程排下的点选（页内 index），由按键线程取走执行（见 `enqueue_select`）。
    pending: Mutex<VecDeque<usize>>,
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
    /// 未能**同步**完成的编辑会话的保活引用（唯一写入点在 `insert_into`，
    /// 理由见那里）。只保最后一个。
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

    /// Activate 记下的 tid；Deactivate 传 0（旧 tid 不再有效，插入会据此静默放弃）。
    pub fn set_client_id(&self, tid: u32) {
        self.client_id.store(tid, Ordering::Relaxed);
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

    /// 把提交文本真正写进文档 —— 提交的**唯一下沉路径**（键事件与候选窗点选
    /// 都走这里；两份实现必然漂移，一处补了下面的保活另一处就会漏）。
    ///
    /// 三种「静默放弃」都是**正常情况**，不是错误路径：
    /// - `client_id` 为 0：尚未 `Activate`，`RequestEditSession` 必失败；
    /// - `pic` 转不出 `ITfInsertAtSelection`：该 context 不支持插入。
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
            return;
        };
        let session: ITfEditSession = session.into();
        // SAFETY: pic 是 TSF 交来的活动 context；session 在本次调用期间存活。
        // 返回 `Result<HRESULT>` 是两层：外层 = 调用本身成不成，
        // 内层 = `phrSession`，也就是 **DoEditSession 的返回值**（0.62 把它当出参映射）。
        // 失败不改行为（"这次不出字"总好过"崩掉宿主进程"），但**必须留下痕迹**：
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
                // 锁在这里即取即放：本分支按定义不是同步回调，不会重入（见上面的
                // 重入警告）；`keepalive` 也从不被 DoEditSession 碰到。
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
/// 生命周期：本对象在 `insert_into` 里临时构造，调用返回后即释放（无人长期
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
