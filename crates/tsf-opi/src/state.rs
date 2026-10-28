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
//! 读线程点选只 `enqueue_select(index)` 排队，按键线程取走执行。落地时机有两处：
//! 下一个键事件开头（`tsf.rs::handle_key_inner` / `key_sink.rs::OnTestKeyDown`）与
//! 焦点变化（`key_sink.rs::OnSetFocus`）。**已知代价：插入仍可能延迟** —— 候选窗是
//! 非激活窗口（desktop `Main.kt` 的 `focusable = false`），点它不产生焦点事件，
//! 「点完候选原地不动、不敲键」谁也叫不醒我们。要即时插入时的升级路径（二选一，
//! 都得真机才验得了）：(a) 按键线程建一个 message-only 窗口，读线程 `PostMessage`
//! 过去在 UI 线程执行；(b) 按 GIT（`IGlobalInterfaceTable`）注册 cookie，读线程取
//! marshal 过的代理。
//!
//! 排队动作带着入队时的**目标世代**（`EngineShared::target_seq`）：点选之后换了文档，
//! 世代由按键线程推进，旧动作在执行点被丢弃 —— 不会把 A 文档的候选插进 B 文档。
//!
//! 【本机（Linux）无法验证的部分，别当成已验】上面整条排队路径本机一行都跑不了
//! （无 Windows）：真机验收**第一件事就是点一次候选、再敲一个键**，看文本有没有
//! 落进文档、宿主进程是否还在。没插入 → 看 `insert_into` 打出的那行 HRESULT。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use windows::Win32::UI::TextServices::{ITfContext, ITfEditSession};
use windows::core::Interface;

use crate::candidate_io::CandidateAction;
use crate::ctl::{CtlReply, CtlRequest};
use crate::logic::TsfLogic;
use crate::tsf::{TsfSink, push_state};

/// 队列上限。对端是**另一个进程**（候选窗）：不设上限的话它连发 select 就能
/// 让宿主进程的内存一直涨。16 远超人手点击速度，超过即丢弃（读线程不阻塞）。
const MAX_PENDING_SELECT: usize = 16;

/// 「直接插入文本」队列上限（理由同 `MAX_PENDING_SELECT`：对端是另一个进程）。
const MAX_PENDING_TEXT: usize = 16;

/// 引擎 + 点选队列：**候选窗读线程够得着的那一半**。
///
/// `Send + Sync` 是编译器自己推的（字段只有 `Mutex` + 原子计数）—— 这个类型里
/// **不允许**出现 COM 接口指针，见模块头的线程模型。想往这加字段前先确认它是 `Send`。
pub struct EngineShared {
    logic: Mutex<TsfLogic>,
    /// 「清空用户词」世代：本引擎上次见过并清过的 token（见 `clear_generation.rs`）。
    /// 每个宿主进程各一份 ⇒ 设置窗写一次标记，各宿主在下次焦点时各自清。
    clear_seen: crate::clear_generation::SeenGeneration,
    /// 插入目标的世代号：按键线程发现本次键事件的 context 与上次不同时 +1
    /// （见 `TsfSharedState::note_target`）。读线程入队时记下当时的值，执行时
    /// 不一致 = 目标文档已换 → 丢弃这次点选/直插（宁可丢一次点击，也不能把
    /// 上一个文档的候选插进当前文档）。
    target_seq: AtomicU32,
    /// 读线程排下的点选（页内 index + 入队时的 `target_seq`），由按键线程取走执行
    /// （见 `enqueue_select`）。
    pending: Mutex<VecDeque<(usize, u32)>>,
    /// 读线程排下的「直接插入文本」（候选窗 `commit_text`，不经引擎选词；同样带
    /// 入队时的 `target_seq`），由按键线程取走执行（见 `enqueue_commit_text`）。
    pending_text: Mutex<VecDeque<(String, u32)>>,
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

    pub fn with_dicts(
        dict: Box<dyn engine_core::dictionary::Dictionary>,
        trad: Option<Box<dyn engine_core::dictionary::Dictionary>>,
    ) -> Self {
        Self::with_dict_inner(TsfLogic::with_dicts(dict, trad))
    }

    fn with_dict_inner(logic: TsfLogic) -> Self {
        Self {
            logic: Mutex::new(logic),
            clear_seen: Default::default(),
            target_seq: AtomicU32::new(0),
            pending: Mutex::new(VecDeque::new()),
            pending_text: Mutex::new(VecDeque::new()),
        }
    }

    /// 当前插入目标世代（读线程入队时快照一份，见 `enqueue_select`）。
    fn target_seq(&self) -> u32 {
        self.target_seq.load(Ordering::Relaxed)
    }

    /// 目标上下文换了（按键线程）或服务失活（Deactivate）→ 推进世代。
    fn bump_target_seq(&self) {
        self.target_seq.fetch_add(1, Ordering::Relaxed);
    }

    /// 取引擎锁。中毒 → None（调用方吞掉这次操作，别经 COM vtable 泄漏 panic）。
    ///
    /// 调用方**不得在持锁期间碰 `target` 或走 COM**：见类型头的锁序说明。
    pub fn lock_logic(&self) -> Option<MutexGuard<'_, TsfLogic>> {
        self.logic.lock().ok()
    }

    /// 比对「清空用户词」世代；变了就清本宿主的学习记忆（按宿主、下次焦点生效）。
    ///
    /// 调用点只有 `CandidateSink::on_activate`：TSF Activate、线程焦点进入、本
    /// profile 重新激活都会走到那里 —— 正是设置窗把焦点交还宿主的时刻。管道控制面
    /// 一次只能清一个宿主，这个标记让其余宿主各自清（见 `clear_generation.rs`）。
    /// 读文件在锁外；读不到 = 本次不清，下一次焦点再试。
    pub fn check_clear_generation(&self) {
        let lap = std::env::var_os("LOCALAPPDATA");
        let token = crate::clear_generation::read(lap.as_deref());
        if !self.clear_seen.take_if_changed(token.as_deref()) {
            return;
        }
        if let Some(mut logic) = self.lock_logic() {
            logic.clear_user_words();
        }
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
    ///
    /// 先读 `target_seq` 再入队：读与写之间有键事件换了文档时，这一条会带着旧世代
    /// 被丢弃 —— 保守方向（最多丢一次点击），不会插错文档。
    pub fn enqueue_select(&self, index: usize) -> bool {
        let seq = self.target_seq();
        match self.pending.lock() {
            Ok(mut q) if q.len() < MAX_PENDING_SELECT => {
                q.push_back((index, seq));
                true
            }
            _ => false,
        }
    }

    /// **按键线程**调：取一个待执行的点选（index + 入队时的目标世代）。None = 队列空。
    pub fn take_pending_selection(&self) -> Option<(usize, u32)> {
        self.pending.lock().ok()?.pop_front()
    }

    /// **读线程**调（候选窗 `commit_text`）：收下这段要直插文档的文本，**不做任何
    /// COM 调用**（理由同 `enqueue_select`：插入目标不在这半）。返回 false = 队列满
    /// 或锁中毒 —— 丢这一次，不阻塞读线程。目标世代语义同 `enqueue_select`。
    pub fn enqueue_commit_text(&self, text: String) -> bool {
        let seq = self.target_seq();
        match self.pending_text.lock() {
            Ok(mut q) if q.len() < MAX_PENDING_TEXT => {
                q.push_back((text, seq));
                true
            }
            _ => false,
        }
    }

    /// **按键线程**调：取一段待直插的文本（文本 + 入队时的目标世代）。None = 队列空。
    pub fn take_pending_text(&self) -> Option<(String, u32)> {
        self.pending_text.lock().ok()?.pop_front()
    }

    /// **按键线程**调（Deactivate）：丢弃全部排队动作 —— 失活后执行的插入只会
    /// 落到不再属于本服务的文档上。与 `clear_target` 一起收尾。
    pub fn clear_pending(&self) {
        if let Ok(mut q) = self.pending.lock() {
            q.clear();
        }
        if let Ok(mut q) = self.pending_text.lock() {
            q.clear();
        }
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
    pub fn take_pending_selection(&self) -> Option<(usize, u32)> {
        self.engine.take_pending_selection()
    }

    /// **按键线程**调：取一段待直插的文本（见 `EngineShared::take_pending_text`）。
    pub fn take_pending_text(&self) -> Option<(String, u32)> {
        self.engine.take_pending_text()
    }

    /// **按键线程**调（Deactivate）：丢弃排队动作，见 `EngineShared::clear_pending`。
    pub fn clear_pending(&self) {
        self.engine.clear_pending();
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

    /// **按键线程**调（每个非抬起键事件，**赶在 `drain_pending` 之前**）：本次键事件
    /// 带来的 context 与上次记的不是同一个 → 推进目标世代，之前排队的点选属于旧文档，
    /// 执行时按世代号丢弃（见 `select_and_insert`）。
    ///
    /// 仅 **Some** 的情况换代；`pic` 为 None（焦点离开文档）时不在这里清目标 ——
    /// 队列里排着的动作仍应落进上一次键事件的文档（那正是用户点选时所在的文档），
    /// 清空由 drain 之后的 `remember_target` 做。
    ///
    /// 线程模型依据：`OnTestKeyDown`/`OnKeyDown` 与候选窗读线程不同，后者只能入队；
    /// 本方法只在按键线程上跑，见模块头。
    pub fn note_target(&self, pic: Option<&ITfContext>) {
        let Some(pic) = pic else { return };
        let Ok(mut g) = self.target.lock() else {
            return; // 中毒锁：不换代（旧世代只会多保留一次排队动作，不插错文档）
        };
        // 指针相等是「同一个 context」的近似判据（TSF 对同一文档给同一接口指针）。
        // shortcut: 同一文档的代理指针若不同，只多丢一次排队点击，不会插错文档。
        if g.as_ref().is_some_and(|old| old.as_raw() == pic.as_raw()) {
            return;
        }
        *g = Some(pic.clone());
        self.engine.bump_target_seq();
    }

    /// 记下本次键事件的插入目标（每个非抬起键事件、drain 之后调一次）。
    /// 与 `note_target` 的分工：Some 的换代在 drain **之前**（否则旧文档的排队动作
    /// 会先执行）；这里补的是 None 的清除，以及 Some 的兜底存储。
    pub fn remember_target(&self, pic: Option<&ITfContext>) {
        let next = pic.cloned();
        if let Ok(mut g) = self.target.lock() {
            *g = next;
        }
    }

    /// **按键线程**调（Deactivate）：释放插入目标与编辑会话保活，并推进世代 ——
    /// 之后读线程再排进来的动作（目标为空）不会被当成当前文档的。
    pub fn clear_target(&self) {
        if let Ok(mut g) = self.target.lock() {
            *g = None;
        }
        if let Ok(mut g) = self.keepalive.lock() {
            *g = None;
        }
        self.engine.bump_target_seq();
    }

    // ---------- 候选窗回复的执行点（**按键线程**，见模块头） ----------

    /// 点选第 `index`（**页内** 0 起）候选：选词 → 插文档。
    /// 返回提交的文本；`None` = 没提交（越界/空缓冲/锁中毒/**目标已换**）。
    ///
    /// `seq` 是入队时 `target_seq` 的快照：与当前世代不同 = 用户在点选之后切了文档，
    /// 这次点选属于旧文档 → 丢弃（连引擎选词都不做，免得清掉当前文档的拼音缓冲）。
    pub fn select_and_insert(&self, index: usize, seq: u32) -> Option<String> {
        if seq != self.engine.target_seq() {
            tlog!("tsf-opi: 丢弃排队的候选点选 index={index}：目标文档已切换");
            return None;
        }
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
    /// `seq` 语义同 `select_and_insert`：目标文档换了就丢这段文本。
    pub fn insert_text(&self, text: &str, seq: u32) {
        if seq != self.engine.target_seq() {
            tlog!(
                "tsf-opi: 丢弃排队的直插文本（{} 字符）：目标文档已切换",
                text.chars().count()
            );
            return;
        }
        let pic = self.target.lock().ok().and_then(|g| g.clone());
        if let Some(pic) = pic {
            self.insert_into(&pic, text);
        }
    }

    // 文档写入（`insert_into` + `InsertTextSession`）在 `state_insert.rs`：
    // 从本文件拆出守 500 行（2026-10-28），且它自成一体的 —— 只依赖 COM 插入 API。
    // 这里保留 impl 的收尾括号；`insert_into` 的 impl 块在同 crate 的子模块里。
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
        // 锁内只抓快照；管道写在锁外 —— 本函数跑在读线程上，持锁写管道会让
        // 按键线程的 `input_key` 等它，候选窗一卡宿主 UI 就跟着卡。
        let snapshot = match self.engine.lock_logic() {
            Some(logic) => logic.snapshot(),
            None => return,
        };
        push_state(sink.as_ref(), &snapshot);
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

// 文档写入（`insert_into` + `InsertTextSession`）拆出守 500 行，见那里的模块头。
#[path = "state_insert.rs"]
mod insert;
