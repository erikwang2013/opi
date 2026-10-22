// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选翻页状态：包装 engine_core::Engine，持有候选分页状态。
//!
//! 语义与 Android 侧一致：每页 8 个候选，一次抓取 FETCH_LIMIT 条（**不设上限**，见该
//! 常量注释；Android 的 fetchLimit 是 JNI 另一条通路，够不到这里的分页出口），页码越界
//! 钳制，buffer 变化时页码归零。纯逻辑结构体（无全局状态、无 FFI），可独立单测；
//! C 出口（lib.rs）经 Mutex 单例访问本状态。

use std::cell::RefCell;

use engine_core::Engine;
use engine_core::candidates::Candidate;
use engine_core::composer::Mode;

use crate::dict_layer::DictLayer;

/// 每页候选数（与 Android 候选栏一致）。
pub const PAGE_SIZE: usize = 8;
/// 一次抓取的候选批量上限：**不设上限**。曾是 64（对齐 Android fetchLimit），后果见
/// `engine-core/src/router.rs` 同名常量：真词库下 `y` 前缀一次命中**八千余条**，用户只能
/// 翻到前 64 条（≈0.8%）—— 而第 64 与第 65 名的静态词频只差 0.01%，没有断崖可供察觉。
/// ⚠️ 「八千余」**别换成精确数字**（它会漂，实测差异见 `candidate_limits_tests.rs` 头注释）。
/// 上限也不省成本：`rank_and_pick` 无论 limit 多大都全量收集 + 排序，截断只是扔掉算好的尾巴。
pub const FETCH_LIMIT: usize = usize::MAX;

// 与 `engine-core/src/router.rs` 的**同名常量**绑死 —— 期望值来自**另一份源码**，不是
// 本文件的字面量。三处各存一份、彼此零绑定的年代，v1.3.0 只抬了那一份，本轨留在 64 上，
// **没有任何门禁红**（2026-09-28 修，见 `candidate_limits_tests.rs`）。
// 编译期求值：漂移是 E0080，不是「某条测试恰好没跑到」。放在常量正下方是刻意的 ——
// 改常量的人没法不看见它（同 `vk.rs` 那条 const 断言的取舍）。
const _: () = {
    assert!(PAGE_SIZE == engine_core::router::PAGE_SIZE);
    assert!(FETCH_LIMIT == engine_core::router::FETCH_LIMIT);
};

/// ⇧ 状态机：off / single（下个字母大写后自动复位）/ lock（持续大写）。
/// 镜像 Android `EngineController.ShiftState` 的三态语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftState {
    #[default]
    Off,
    Single,
    Lock,
}

/// 引擎 + 候选分页状态。
pub struct CandidateState {
    pub(crate) engine: Engine,
    /// 当前页（0 起）。
    pub(crate) page: usize,
    /// 上次操作后的 buffer 快照；变化时页码归零。
    pub(crate) buffer_snapshot: String,
    /// ⇧ 状态机（镜像 Android EngineController.shiftState）。
    pub(crate) shift_state: ShiftState,
    /// 上一次按下的分流结论：(键值, 是否直通)。抬起按同一结论回复，使「按下放行 →
    /// 抬起也放行」成立（见 input_method::handle_key 的可打印分支与退格/回车分支：
    /// 那两个按下分支会把缓冲改空，抬起再判「当前」缓冲就不对称了）。
    pub(crate) last_printable: Option<(u32, bool)>,
    /// 导入词库层（2026-10-14，M3）。状态与语义见 `dict_layer.rs` 头注。
    pub(crate) dicts: DictLayer,
    /// 英文专业模式（2026-10-19）：false = en26 只直出字母（默认），true = 词库联想。
    pub(crate) english_pro: bool,
    /// 「一次抓取」缓存（2026-10-22 性能，30ms 总目标）：`page_count()` 已取过的
    /// 全量候选留给**紧随其后**的 `candidates()`，让一次翻页键只跑一次
    /// `Engine::candidates(MAX)`。实测 PageDown after `ni`：两次取回完全相同，
    /// 完整按键 30.6–32.3ms，其中一次取回 ~15ms。
    /// 键 = (真缓冲, 逐字片段)：两者任一变化都可能换候选集；设置位入口另行显式清
    /// （`candidate_settings.rs`）。只由 `page_count` 填充、被 `fetched` 消费一次 ——
    /// 不在候选窗刷新后继续持有全量表，避免跨键常驻内存与额外的失效面。
    pub(crate) fetch_cache: RefCell<Option<(String, String, Vec<Candidate>)>>,
}

impl CandidateState {
    /// 装载引擎。`None`/空串 → 内置回退词库（35 词）；非空路径 →
    /// load_or_fallback 原样语义（坏路径返回 Err）。语义与 opi-ffi
    /// api::install 一致（学习默认开）。
    pub fn load(path: Option<&str>) -> Result<Self, String> {
        Self::load_with_trad(path, None)
    }

    /// 双词典装载（简 + 繁，2026-10-09）。`path`（luna）的语义与 [`Self::load`]
    /// 一字不差；`trad` 是**可选增强**，坏法语义刻意与 luna 不同：
    ///   · None/空串    → 无繁体词典（Traditional 模式按 engine.rs 的既有约定回退 luna）；
    ///   · 有路径但坏   → **不**返回 Err：记一行 stderr、退回无繁体。繁体装坏了不该
    ///     让用户连拼音都用不了；而 luna 坏 = 整个插件没词库，那种必须 Err
    ///     （lib.rs 的 install 语义与 data_dir 的用例都钉着这一条）。
    /// 路由本身在 engine-core（`Engine::with_dictionaries`，tests/trad_mode.rs 已有
    /// 双词典路由的门禁），本层只负责「两条路径 → 一次构造」。
    pub fn load_with_trad(path: Option<&str>, trad: Option<&str>) -> Result<Self, String> {
        let dict: Box<dyn engine_core::dictionary::Dictionary> = match path {
            Some(p) if !p.is_empty() => {
                engine_data::load_or_fallback(Some(std::path::Path::new(p)))?
            }
            _ => Box::new(engine_data::fallback_dict()),
        };
        let trad_dict: Option<Box<dyn engine_core::dictionary::Dictionary>> = match trad {
            Some(p) if !p.is_empty() => {
                match engine_data::load_or_fallback(Some(std::path::Path::new(p))) {
                    Ok(d) => Some(d),
                    Err(e) => {
                        eprintln!("fcitx5-opi: 繁体词库装载失败，Traditional 回退简体: {e}");
                        None
                    }
                }
            }
            _ => None,
        };
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        let mut s = CandidateState {
            engine: Engine::with_dictionaries(dict, trad_dict, symbols, true),
            page: 0,
            buffer_snapshot: String::new(),
            shift_state: ShiftState::Off,
            last_printable: None,
            dicts: DictLayer::default(),
            english_pro: false,
            fetch_cache: RefCell::new(None),
        };
        s.refresh_snapshot();
        Ok(s)
    }

    fn refresh_snapshot(&mut self) {
        self.buffer_snapshot = self.engine.buffer().to_string();
    }

    /// buffer 与快照不一致 → 页码归零（与 Android 语义一致），再刷新快照。
    ///
    /// 所有编辑/提交/装载入口都以本方法收尾 ⇒ 一次抓取缓存也在这里统一作废：
    /// 即使 buffer 字符串恰好没变（提交后清空、再回到同一串），候选集也可能已换。
    fn reset_page_if_buffer_changed(&mut self) {
        self.invalidate_fetch_cache();
        if self.engine.buffer() != self.buffer_snapshot {
            self.page = 0;
        }
        self.refresh_snapshot();
    }

    /// 候选缓存键：候选集依赖真缓冲与已确认的逐字片段，两者一起作键。
    fn fetch_key(&self) -> (String, String) {
        (self.engine.buffer().to_string(), self.engine.pending_text())
    }

    /// 作废一次抓取缓存。除 `reset_page_if_buffer_changed` 外，`candidate_settings.rs`
    /// 的每个设置位入口也要调（它们改候选集但通常不动 buffer）。
    pub(crate) fn invalidate_fetch_cache(&mut self) {
        self.fetch_cache.get_mut().take();
    }

    /// 取走与当前键匹配的缓存（**一次性**：消费后即清，避免翻页键之外留存）。
    fn take_fetch_cache(&self, key: &(String, String)) -> Option<Vec<Candidate>> {
        let mut slot = self.fetch_cache.borrow_mut();
        let hit = slot
            .as_ref()
            .is_some_and(|(b, p, _)| b == &key.0 && p == &key.1);
        if hit {
            slot.take().map(|(_, _, list)| list)
        } else {
            // 键不匹配的旧表不再可能有用（编辑/设置位都走失效），别留着。
            slot.take();
            None
        }
    }

    /// 有**待处理的输入** = 真缓冲或已确认的逐字片段非空（2026-10-17 逐字确认）：
    /// 路由的 ⌫/空格/回车分流都用它 —— 逐字态下缓冲可能已空，但片段还没进输入框。
    pub fn has_composing(&self) -> bool {
        !self.engine.buffer().is_empty() || !self.engine.pending_text().is_empty()
    }

    /// 候选窗/预编辑**显示用**：已确认的逐字片段 + 真缓冲（2026-10-17 逐字确认）。
    /// `buffer()` 保持真缓冲语义不动（两轨的大量门禁钉着它；配对：tsf-opi logic.rs
    /// 的同名方法，逐行同构）。
    pub fn display_buffer(&self) -> String {
        let shown = self.engine.display_buffer();
        // 9/14 键的读数由 `candidates()` 填缓存，而 C++ 先读缓冲再取候选 ⇒ 新缓冲的
        // 首帧读不到读数（只剩逐字片段）。退回原始缓冲，数字/按键不得从候选栏消失。
        if shown.is_empty()
            || (!self.engine.buffer().is_empty() && shown == self.engine.pending_text())
        {
            return self.raw_buffer();
        }
        shown
    }

    /// **原文**（W1 直接确认 / 切输入法提交用）：已确认逐字片段 + 真缓冲。
    /// 与 [`Self::display_buffer`] 是两个出口：那个画在面板上（全键盘带 `'` 分词），
    /// 这个进文档 —— 分词符不能上屏（引擎 API「两个出口别合并」）。
    pub fn raw_buffer(&self) -> String {
        let mut out = self.engine.pending_text();
        out.push_str(self.engine.buffer());
        out
    }

    pub fn buffer(&self) -> String {
        self.engine.buffer().to_string()
    }

    pub fn mode(&self) -> Mode {
        self.engine.mode()
    }

    /// 英文专业模式（2026-10-19）：false = en26 只直出字母、不查词库；true = 前缀联想。
    pub fn english_pro(&self) -> bool {
        self.english_pro
    }

    /// 普通 ⇄ 专业：切换时清掉引擎缓冲/候选（旧串语义不同），并刷新页码快照。
    /// 调用方负责刷新 fcitx5 面板（见 `opi_engine.h` 的 englishpro 动作）。
    pub fn set_english_pro(&mut self, on: bool) {
        if self.english_pro == on {
            return;
        }
        self.english_pro = on;
        self.clear();
    }

    /// 全角开关的读侧（直传点判据；语义与 `Engine::fullwidth` 同）。
    pub fn fullwidth(&self) -> bool {
        self.engine.fullwidth()
    }

    /// 全角化待上屏文本（宽档；见 `Engine::fullwidthize` 头注 —— 宿主直传点的唯一入口）。
    pub fn fullwidthize(&self, text: &str) -> String {
        self.engine.fullwidthize(text)
    }

    /// 输入单字符键。空串/多字符由调用方（input_method）拒绝；
    /// 非 ASCII 由引擎层拒绝。返回引擎输出（如英文模式下已提交文本）。
    pub fn input_key(&mut self, ch: char) -> String {
        let out = self.engine.input_key(ch);
        self.reset_page_if_buffer_changed();
        out
    }

    /// 引擎层标点表：`Some` = 该键归标点表管（文本要上屏），`None` = 交回路由直通。
    /// 与 `input_key` 同一套收尾 —— 命中时引擎可能先 flush 缓冲，页码得跟着归零。
    pub fn input_punct(&mut self, ch: char) -> Option<String> {
        let out = self.engine.input_punct(ch);
        self.reset_page_if_buffer_changed();
        out
    }

    pub fn input_space(&mut self) -> String {
        let out = self.engine.input_space();
        self.reset_page_if_buffer_changed();
        out
    }

    pub fn backspace(&mut self) {
        self.engine.backspace();
        self.reset_page_if_buffer_changed();
    }

    pub fn clear(&mut self) {
        self.engine.clear();
        self.reset_page_if_buffer_changed();
    }

    /// 清空用户词（学习记忆）。缓冲没变但候选集会变（用户词参与排名）⇒ 作废
    /// 抓取缓存并回第一页，别停在一个已经不存在的页上。
    pub fn clear_user_words(&mut self) {
        self.engine.clear_user_words();
        self.page = 0;
        self.invalidate_fetch_cache();
    }

    /// 切模式（**不收尾**：待处理的缓冲由 `Composer::switch_mode` 直接清掉）。
    /// 需要「切走收尾（拼音/繁体上原文、符号模式丢关键字）」的用
    /// [`Self::switch_mode_confirmed`]（W1 订正版）。
    pub fn switch_mode(&mut self, mode: Mode) {
        self.engine.switch_mode(mode);
        // ⇧ 只在 English 有意义，判据与 Android `EngineController.switchMode` 的
        // `if (m != ENGLISH) resetShift()` 逐字一致：**离开** English 才清。
        // 前端 ⇧ 三态与引擎侧是两份状态，引擎清了不够 —— 英文空缓冲直传的大小写由
        // ShiftState 决定，Lock 残留会让再次进入 English 后打出的全是大写。
        // **进入** English 反过来：引擎 shift 刚被清，前端 SINGLE/LOCK 必须重推，
        // 否则高亮着却打出小写。配对：tsf-opi/src/logic.rs 的同名方法。
        if mode != Mode::English {
            self.shift_state = ShiftState::Off;
        } else if self.shift_state != ShiftState::Off {
            self.set_shift(true);
        }
        self.reset_page_if_buffer_changed();
    }

    /// 切走/切英文的**直接确认 + 切换**（W1 订正版，2026-10-10 深夜用户裁决
    /// 「切英文 = 默认是拼音，不要是候选词」）：先把待处理的缓冲收尾成待上屏文本
    /// （拼音/繁体**原样原文**、**不再**上首候选；**符号模式丢关键字**——清掉、
    /// 空串，同日裁决二；**先退符号浏览**恢复真缓冲 —— 顺序承重，契约见
    /// `engine-core/src/engine_symbol_browse.rs` 头），再切模式。返回**待上屏文本**
    /// （空缓冲 = 空串）—— 调用方负责插入文档。
    /// **先确认后切**也是承重的：顺序反了，`switch_mode` 会先把缓冲清掉，
    /// 返回值只能是空串（原文丢在清缓冲里）。
    /// 与 [`Self::switch_mode`] 的唯一差别就是这步收尾：那条路（`opi_fcitx5_switch_mode`
    /// 出口 ← 轻按 Shift / 模式热键）`Composer::switch_mode` 只清缓冲，打到一半的
    /// 拼音被静默丢弃。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构，改一处必须改另一处）。
    /// ⚠️ **本回合无 C 出口**（2026-10-10 出口面冻结在 `opi_fcitx5_symbol_browse` /
    /// `opi_fcitx5_load_symbols` 两个名字，`opi_rust_ffi.h` 归 glue）：fcitx5 的产品
    /// W1 归 glue 的 C++（「先 `opi_fcitx5_symbol_browse(0)` 退浏览、再上原文」），
    /// 本函数是同一语义的状态机实现 —— 接线时由 glue 加出口并由 C++ 调它取文本。
    pub fn switch_mode_confirmed(&mut self, mode: Mode) -> String {
        let text = self.engine.confirm_pending();
        self.switch_mode(mode); // 切换与 ⇧/页码收尾复用既有实现，不复制一份
        text
    }

    /// 全角 ↔ 半角，返回切换后的状态（状态栏显示用）。语义全在引擎层
    /// （[`Engine::toggle_fullwidth`]），本层只做转发 —— **键位不在本层**：
    /// 与 `toggle_symbol` 同一条（键位不在本层）：两轨一律 `Shift+Space`，且必须
    /// 在**调引擎之前**判掉 —— `router.rs` 对 Ctrl/Alt/Meta 在最前面就直通，而
    /// `Shift+Space` 会走到 `KEY_SPACE` 分支且**该分支不看 Shift 位**（表现为「选首
    /// 候选」）。键位占用表见 `vk.rs` 的 `fullwidth_hotkey` 与 C++ 侧同名函数。
    pub fn toggle_fullwidth(&mut self) -> bool {
        self.engine.toggle_fullwidth()
    }

    pub fn set_shift(&mut self, on: bool) {
        self.invalidate_fetch_cache();
        self.engine.set_shift(on);
        // shift 不改变 buffer（reset_page_if_buffer_changed 不会触发），但
        // 可能改变候选集；将页码钳制到当前 page_count 边界，防 page 越界。
        self.set_page(self.page);
    }

    // ---- 符号浏览（「符号大全」）：转发 + 分页收敛。宿主键路由见 input_method.rs
    // 的浏览分支；契约（buffer 劫持 / 未处理键先退出 / W1 先退出）在
    // engine-core/src/engine_symbol_browse.rs 头。配对：tsf-opi/src/logic.rs 同名方法。

    /// 装载类目数据（TSV 串；文件读取在 data_dir.rs —— engine-core 禁 IO）。
    /// 返回跳过的坏行数；**替换**数据集并复位浏览态。buffer 随之变化 → 页码归零。
    pub fn load_symbol_categories(&mut self, tsv: &str) -> usize {
        let skipped = self.engine.load_symbol_categories(tsv);
        self.reset_page_if_buffer_changed();
        skipped
    }

    /// 类目数据是否可用（入口闸门：文件缺失/坏 ⇒ false ⇒ 宿主把入口禁用）。
    pub fn symbol_browse_available(&self) -> bool {
        self.engine.symbol_browse_available()
    }

    pub fn is_symbol_browsing(&self) -> bool {
        self.engine.is_symbol_browsing()
    }

    /// 进入浏览（无数据 → false，入口禁用）；已在浏览态 = no-op。
    /// buffer 从真缓冲变标题 → 页码归零（reset）。
    pub fn enter_symbol_browse(&mut self) -> bool {
        let ok = self.engine.enter_symbol_browse();
        self.reset_page_if_buffer_changed();
        ok
    }

    /// 退出浏览（幂等、**同步**：返回时 buffer() 已是真缓冲 —— W1/切走前的强制前置）。
    pub fn exit_symbol_browse(&mut self) {
        self.engine.exit_symbol_browse();
        self.reset_page_if_buffer_changed();
    }

    // ---- ⇧ 状态机（镜像 Android EngineController.shiftTap/ShiftLongPress/consumeSingleShift）----

    pub fn shift_state(&self) -> ShiftState {
        self.shift_state
    }

    /// 单击：Off→Single（引擎 shift 开）；Single/Lock→Off（引擎 shift 关）。
    pub fn shift_tap(&mut self) {
        self.shift_state = if self.shift_state == ShiftState::Off {
            ShiftState::Single
        } else {
            ShiftState::Off
        };
        self.set_shift(self.shift_state != ShiftState::Off);
    }

    /// 长按：Lock（持续大写）。
    pub fn shift_long_press(&mut self) {
        self.shift_state = ShiftState::Lock;
        self.set_shift(true);
    }

    /// single 态消费后复位（lock 不受影响）。
    pub fn consume_single_shift(&mut self) {
        if self.shift_state == ShiftState::Single {
            self.shift_state = ShiftState::Off;
            self.set_shift(false);
        }
    }

    /// 提交当前页第 `index` 个候选（页内索引，0 起）。越界返回空串。
    pub fn select(&mut self, index: usize) -> String {
        let fetched = self.fetched();
        self.select_from(&fetched, index)
    }

    /// 页内索引 → 全局下标（**换算只此一份**），在调用方已抓好的候选表上提交。
    /// 数字选词（`digit_select`）复用自己那次抓取，不重排整表。
    ///
    /// **越界判据是页内的**（本页候选数）：全量判据会把页内越界（本页 8 项时点第 9 项）
    /// 换算成**次页**下标 ⇒ 提交用户看不见的候选（门禁：`candidate_tests.rs` /
    /// `logic_candidate_tests.rs` 的 `select_beyond_page_returns_empty`）。页内/全量越界一律空串。
    pub(crate) fn select_from(&mut self, fetched: &[Candidate], index: usize) -> String {
        // 本页候选数（口径与 `candidates()` 的 skip/take 逐字等价：饱和减法 + min）
        let page_len = fetched
            .len()
            .saturating_sub(self.page * PAGE_SIZE)
            .min(PAGE_SIZE);
        let out = if index < page_len {
            let global = self.page * PAGE_SIZE + index;
            self.engine.select_from(fetched, global)
        } else {
            String::new()
        };
        self.reset_page_if_buffer_changed();
        out
    }

    /// 批量抓取（FETCH_LIMIT 内；不设上限时 `truncate` 是空操作，排序仍全量）。
    /// 先消费 `page_count()` 留下的一次抓取缓存（同一 buffer/pending 才有效）。
    pub(crate) fn fetched(&self) -> Vec<Candidate> {
        if self.fetch_cache.borrow().is_none() {
            return self.engine.candidates(FETCH_LIMIT);
        }
        let key = self.fetch_key();
        if let Some(list) = self.take_fetch_cache(&key) {
            return list;
        }
        self.engine.candidates(FETCH_LIMIT)
    }

    /// 当前页候选文本（最多 PAGE_SIZE 条）。
    pub fn candidates(&self) -> Vec<String> {
        self.fetched()
            .iter()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|c| c.text.clone())
            .collect()
    }

    pub fn page(&self) -> usize {
        self.page
    }

    /// 总页数（无候选 → 0）。
    ///
    /// 命中缓存时只数条数；未命中则取回全量表并**留下**给紧接着的 `candidates()`
    /// （翻页键：`next_page` → 本方法 → 宿主刷新候选窗 → `candidates()`）。
    pub fn page_count(&self) -> usize {
        let key = self.fetch_key();
        if let Some((b, p, list)) = self.fetch_cache.borrow().as_ref()
            && b == &key.0
            && p == &key.1
        {
            return list.len().div_ceil(PAGE_SIZE);
        }
        let list = self.engine.candidates(FETCH_LIMIT);
        let count = list.len().div_ceil(PAGE_SIZE);
        *self.fetch_cache.borrow_mut() = Some((key.0, key.1, list));
        count
    }

    /// 下一页，越界钳制到最后一页；返回新页码。
    pub fn next_page(&mut self) -> usize {
        self.set_page(self.page + 1)
    }

    /// 上一页，越界钳制到首页；返回新页码。
    pub fn prev_page(&mut self) -> usize {
        self.page = self.page.saturating_sub(1);
        self.page
    }

    /// 直接跳到第 `p` 页（0 起），钳制到 [0, page_count-1]；返回实际页码。
    pub fn set_page(&mut self, p: usize) -> usize {
        let count = self.page_count();
        self.page = if count == 0 { 0 } else { p.min(count - 1) };
        self.page
    }
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行：候选分页状态机测试（键路由测试
// 见 `input_method` 模块）。配对：`candidate_tests.rs`（本轨）↔ `logic_candidate_tests.rs`
// （tsf 轨）；两轨**分层不同**（本轨的 buffer 编辑方法在本文件，tsf 在 `logic_input_method.rs`），
// 两份测试的调用面因此不同构 —— 别按「逐行同构」去"修"。
#[cfg(test)]
#[path = "candidate_tests.rs"]
mod candidate_tests;
// 候选上限门禁独立成文件（`#[path]` 引入）以保持本文件 <500 行；两轨同源不合并，
// 配对：`candidate_limits_tests.rs`（Linux 轨）↔ `logic_limits_tests.rs`（Windows 轨）。
#[cfg(test)]
#[path = "candidate_limits_tests.rs"]
mod limits_tests;
