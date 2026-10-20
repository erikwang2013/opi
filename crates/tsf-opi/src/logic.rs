// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! TSF 逻辑层：候选翻页状态机 + 键路由（语义对照 A4 行为表，即 Android
//! `KeyRouter`/`EngineController`，与 Linux 轨 fcitx5-opi 同源）。
//!
//! 纯 Rust、无 windows/COM 类型，可独立单测；C2 的 TSF 胶水做键码映射。
//! 键码约定（可打印字符 = Unicode 码点，如 'a'=97；特殊键 = `SPECIAL_BASE |
//! Windows VK`；键状态位沿用 fcitx5 轨位约定）与路由表见子模块 `input_method`
//! 头注释 —— 两段键空间不相交是硬约束，成因见 `input_method::SPECIAL_BASE`。
//!
//! TSF 现实的适配：TSF 下按键总是先经服务处理，无 fcitx5 式"直通客户端"
//! 概念；空缓冲退格/回车、Ctrl/Alt 组合等返回 `Unhandled`，由 C2 决定
//! 是否交应用（不拦截则键自然流入）。
//!
//! 划分与 Linux 轨对称（两轨逐行同构，测试文件不合并）：本文件 = fcitx5 轨的
//! `candidate.rs`（状态 + 分页），子模块 `input_method`（`logic_input_method.rs`）
//! = fcitx5 轨的 `input_method.rs`（键路由）。

use engine_core::Engine;
use engine_core::candidates::Candidate;
use engine_core::composer::Mode;

/// 每页候选数（与 Android 候选栏一致）。
pub const PAGE_SIZE: usize = 8;
/// 一次抓取的候选批量上限：**不设上限**。曾是 64（对齐 Android fetchLimit），后果见
/// `engine-core/src/router.rs` 同名常量：真词库下 `y` 前缀一次命中**八千余条**，用户只能
/// 翻到前 64 条（≈0.8%）—— 而第 64 与第 65 名的静态词频只差 0.01%，没有断崖可供察觉。
/// ⚠️ 「八千余」**别换成精确数字**（它会漂，实测差异见 `logic_limits_tests.rs` 头注释）。
/// 上限也不省成本：`rank_and_pick` 无论 limit 多大都全量收集 + 排序，截断只是扔掉算好的尾巴。
pub const FETCH_LIMIT: usize = usize::MAX;

// 与 `engine-core/src/router.rs` 的**同名常量**绑死 —— 期望值来自**另一份源码**，不是
// 本文件的字面量。三处各存一份、彼此零绑定的年代，v1.3.0 只抬了那一份，本轨留在 64 上，
// **没有任何门禁红**（2026-09-28 修，见 `logic_limits_tests.rs`）。
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

/// 引擎 + 候选分页 + ⇧ 状态机（TSF 逻辑层唯一状态对象）。
pub struct TsfLogic {
    pub(crate) engine: Engine,
    /// 当前页（0 起）。
    pub(crate) page: usize,
    /// 上次操作后的 buffer 快照；变化时页码归零。
    pub(crate) buffer_snapshot: String,
    /// ⇧ 状态机（镜像 Android EngineController.shiftState）。
    pub(crate) shift_state: ShiftState,
    /// 英文专业模式（2026-10-19，镜像 fcitx5 轨 `CandidateState::english_pro`）：
    /// false = en26 字母直出、不查词库；true = 前缀联想。**会话内状态、不落配置**
    /// （与 Linux 轨同一语义：每次插件/引擎启动都回 false）。
    pub(crate) english_pro: bool,
    /// 上一次按下的分流结论：(键值, 是否放行)。抬起按同一结论回复，使「按下放行 →
    /// 抬起也放行」成立（见 input_key 的可打印分支与退格/回车分支：那两个按下分支
    /// 会把缓冲改空，抬起再判「当前」缓冲就不对称了）。
    pub(crate) last_printable: Option<(u32, bool)>,
}

impl TsfLogic {
    /// 装载引擎。`None`/空串 → 内置回退词库（35 词）；非空路径 →
    /// load_or_fallback 原样语义（坏路径返回 Err）。与 opi-ffi 及
    /// fcitx5-opi 的 load 语义一致（学习默认开）。
    pub fn load(path: Option<&str>) -> Result<Self, String> {
        let dict: Box<dyn engine_core::dictionary::Dictionary> = match path {
            Some(p) if !p.is_empty() => {
                engine_data::load_or_fallback(Some(std::path::Path::new(p)))?
            }
            _ => Box::new(engine_data::fallback_dict()),
        };
        Ok(Self::with_dict(dict))
    }

    /// 用**已装好的**词库构建（`load` 的落点，也是 `dict_path::load_dict` 的落点）。
    /// Windows 侧走这条：词库解析（含逐候选回退）在 `dict_path` 里一次做完，
    /// 不必经历「先给路径 → 失败再回退」的两段式 —— 那条路的中间态是
    /// 「服务创建不出来」，用户看到的是输入法整个不在（见 `com_server.rs`）。
    /// 失败路径归零：能到这里的词库一定是装上了的（最差是内置 35 词）。
    pub fn with_dict(dict: Box<dyn engine_core::dictionary::Dictionary>) -> Self {
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        let mut s = TsfLogic {
            engine: Engine::new(dict, symbols, true),
            page: 0,
            buffer_snapshot: String::new(),
            shift_state: ShiftState::Off,
            english_pro: false,
            last_printable: None,
        };
        s.refresh_snapshot();
        s
    }

    fn refresh_snapshot(&mut self) {
        self.buffer_snapshot = self.engine.buffer().to_string();
    }

    /// buffer 与快照不一致 → 页码归零（与 Android 语义一致），再刷新快照。
    fn reset_page_if_buffer_changed(&mut self) {
        if self.engine.buffer() != self.buffer_snapshot {
            self.page = 0;
        }
        self.refresh_snapshot();
    }

    /// 有**待处理的输入** = 真缓冲或已确认的逐字片段非空（2026-10-17 逐字确认）：
    /// 路由的 ⌫/空格/回车分流都用它 —— 逐字态下缓冲可能已空，但片段还没进输入框。
    pub fn has_composing(&self) -> bool {
        !self.engine.buffer().is_empty() || !self.engine.pending_text().is_empty()
    }

    /// 候选窗**显示用**：已确认的逐字片段 + 真缓冲（2026-10-17 逐字确认）。
    /// `buffer()` 保持真缓冲语义不动（门禁钉着它；配对：fcitx5-opi candidate.rs 的
    /// 同名方法，逐行同构）。
    pub fn display_buffer(&self) -> String {
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

    /// 全角开关的读侧（直传点判据；语义与 `Engine::fullwidth` 同）。
    pub fn fullwidth(&self) -> bool {
        self.engine.fullwidth()
    }

    /// 清空 ⇧ 残留（状态栏 `lang`/`mode` 动作进英文前用）：与用户按 ⇧ 的「单次大写」
    /// 不是一回事，切换动作不该把大写带进新档位。
    pub fn reset_shift(&mut self) {
        self.shift_state = ShiftState::Off;
        self.set_shift(false);
    }

    /// 符号/表情搜索（设置页与状态栏符号页共用）：命中条目文本，保序。
    pub fn search_symbol_texts(&self, keyword: &str) -> Vec<String> {
        self.engine
            .search_symbols(keyword)
            .into_iter()
            .map(|e| e.text)
            .collect()
    }

    /// 全角化待上屏文本（宽档；见 `Engine::fullwidthize` 头注 —— 宿主直传点的唯一入口）。
    pub fn fullwidthize(&self, text: &str) -> String {
        self.engine.fullwidthize(text)
    }

    /// 单字符入引擎（路由内部用）。返回引擎输出（如英文模式已提交文本）。
    fn input_char(&mut self, ch: char) -> String {
        let out = self.engine.input_key(ch);
        self.reset_page_if_buffer_changed();
        out
    }

    /// 引擎层标点表：`Some` = 该键归标点表管（文本要上屏），`None` = 交回路由放行。
    /// 与 `input_char` 同一套收尾 —— 命中时引擎可能先 flush 缓冲，页码得跟着归零。
    fn input_punct(&mut self, ch: char) -> Option<String> {
        let out = self.engine.input_punct(ch);
        self.reset_page_if_buffer_changed();
        out
    }

    fn input_space(&mut self) -> String {
        let out = self.engine.input_space();
        self.reset_page_if_buffer_changed();
        out
    }

    fn backspace(&mut self) {
        self.engine.backspace();
        self.reset_page_if_buffer_changed();
    }

    pub fn clear(&mut self) {
        self.engine.clear();
        self.reset_page_if_buffer_changed();
    }

    /// 测试探针：四个开关的当前值。生产侧**没有读需求**（设置窗读的是配置文件，
    /// 不从引擎回读），故打成 `cfg(test)` —— 不在生产面上多开一个出口。
    #[cfg(test)]
    pub(crate) fn switches(&self) -> (bool, bool, bool, bool) {
        (
            self.engine.learner_enabled(),
            self.engine.chinese_punct(),
            self.engine.fuzzy_enabled(),
            self.engine.jianpin_enabled(),
        )
    }

    /// 把「属性设置」一次性套到引擎上（开关 + 导入词库）。**唯一入口**（两处调用：
    /// 服务创建 + 每次 `Activate`）—— 各处 setter 各调各的会漏，漏掉的那个是静默的。
    ///
    /// `dicts_dir` = 导入词库目录（`dict_path::dicts_dir`，调用方从 `%LOCALAPPDATA%` 推
    /// —— 本层不读环境，主机单测才能把它换成临时目录）。目录不在 ⇒ 零导入词库
    /// （还没导入过是常态，零日志）。
    pub fn apply_settings(&mut self, s: crate::settings::Settings, dicts_dir: &std::path::Path) {
        self.set_learner(s.learn);
        self.set_chinese_punct(s.chinese_punct);
        // 模糊音：列表非空 ⇒ 以列表为准；空 ⇒ 回落旧的布尔（老配置行为一字不变）。
        if s.fuzzy_pairs.is_empty() {
            self.set_fuzzy(s.fuzzy);
        } else {
            self.set_fuzzy_pairs(&s.fuzzy_pairs);
        }
        self.set_jianpin(s.jianpin);
        self.set_correction(s.correction);
        // 双拼布局：空串（未设置）⇒ 不动引擎（老配置行为一字不变）；**认不得的名字
        // 也不动**（配置面写坏一个名字不该静默把用户的布局跳回自然码）。
        if let Some(i) = engine_core::shuangpin::layout_index_of_name(&s.shuangpin_layout) {
            self.set_shuangpin_layout(i);
        }
        // 拼音方式：认不得的名字/空串都**不动**引擎（同上一条姿态）。
        if let Some(scheme) = engine_core::Scheme::from_name(&s.pinyin_method) {
            self.set_scheme(scheme);
        }
        // 五笔（2026-10-14，M4 第二波）：名字认不得/空串**不动**引擎（同 pinyin_method
        // 的姿态）；个数 0/负 = 未设置（引擎缺省 5），正数交给引擎钳到 1..=9。
        if let Some(st) = engine_core::WubiStyle::from_name(&s.wubi_style) {
            self.set_wubi_style(st);
        }
        if let Some(e) = engine_core::WubiMixedEntry::from_name(&s.wubi_mixed_entry) {
            self.set_wubi_mixed_entry(e);
        }
        if let Some(r) = engine_core::WubiReturnStyle::from_name(&s.wubi_return) {
            self.set_wubi_return_style(r);
        }
        if s.wubi_candidate_limit > 0 {
            self.set_wubi_candidate_limit(s.wubi_candidate_limit as usize);
        }
        self.set_wubi_auto_commit(s.wubi_auto_commit);
        // 码表：按 `WubiPlan` 选表，从**与 luna.opid 同目录**装载（`dicts_dir` 的父目录）。
        // 文件不在是常态（可选增强，与 fcitx5 轨 opiWubiSync 一字不差）：跳过 + 零日志，
        // 方案落普通路径；**文件在但装不上**才打日志（load_wubi 内部打）。
        let wubi = crate::dict_path::wubi_dict_path(dicts_dir, &s.wubi_plan);
        if wubi.is_file() {
            let _ = self.load_wubi(&wubi.to_string_lossy());
        }
        // 导入词库：名单是配置值、目录是环境量，两个都到手才成立 —— 与 fcitx5 轨
        // `applyOpiSettings → opiDictsSync` 同一条（那边目录由 C++ 现推）。
        self.sync_extra_dicts(dicts_dir, &s.disabled_dicts);
    }

    /// 扫词库目录 + **整体替换**引擎里的导入层（判定在 `extra_dicts`，这里只转发 + 日志）。
    /// 每次调用都从目录重算（零状态；后果与代价见 `extra_dicts` 头注）。返回装上的层数
    /// （生产侧不用，门禁/排障要看）。
    /// 配对：fcitx5-opi 的 `opi_fcitx5_import_dict` + `opi_fcitx5_set_dict_enabled` 那一对
    /// 出口（那边由 C++ 宿主逐条调 —— 不对称的理由见 `extra_dicts` 头注）。
    pub(crate) fn sync_extra_dicts(&mut self, dir: &std::path::Path, disabled_raw: &str) -> usize {
        let report = crate::extra_dicts::load(dir, disabled_raw);
        // 「装载结果必须接住」（本仓的坏词库静默失效前车之鉴）：没装上的在这里现形。
        for line in &report.lines {
            eprintln!("tsf-opi: {line}");
        }
        let count = report.dicts.len();
        self.engine.set_extra_dicts(report.dicts);
        count
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
        // 否则高亮着却打出小写。配对：fcitx5-opi/src/candidate.rs 的同名方法。
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
    /// 与 [`Self::switch_mode`] 的唯一差别就是这步收尾：那条路（本轨 Ctrl+' 的
    /// `toggle_mode`）`Composer::switch_mode` 只清缓冲，打到一半的拼音被静默丢弃。
    /// 配对：`fcitx5-opi/src/candidate.rs` 的同名方法（两轨同构，改一处必须改另一处）。
    /// ⚠️ **本回合无生产调用方**（2026-10-10 裁决「状态机语义先立住、如实注明」）：
    /// tsf 的切英文路径仍是 `toggle_mode` → `switch_mode`（丢缓冲）—— 接线 = 改调
    /// 本函数并把返回值插入文档（Windows 侧插入面未验证，不目测）。fcitx5 侧同语义的
    /// 另一个实现是 glue 的 C++（「先 `opi_fcitx5_symbol_browse(0)` 退浏览、再上
    /// 原文」）。
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

    fn set_shift(&mut self, on: bool) {
        self.engine.set_shift(on);
        // shift 不改变 buffer（reset_page_if_buffer_changed 不会触发），但
        // 可能改变候选集；将页码钳制到当前 page_count 边界，防 page 越界。
        self.set_page(self.page);
    }

    // ---- 符号浏览（「符号大全」）：转发 + 分页收敛。宿主键路由见
    // logic_input_method.rs 的浏览分支；契约（buffer 劫持 / 未处理键先退出 /
    // W1 先退出）在 engine-core/src/engine_symbol_browse.rs 头。
    // 配对：fcitx5-opi/src/candidate.rs 的同名方法（两轨同构，改一处必须改另一处）。
    // 【本回合 Windows 侧无装载接线】`load_symbol_categories` 暂无生产调用方
    // （2026-10-10 裁决：Windows 符号页由桌面 Compose 自渲染，点选走
    // candidate_io.rs 的 `commit_text` 直插）—— 无数据 ⇒ 入口恒 false 且
    // `enter_symbol_browse` 被拒，即「没接线」的唯一后果是惰性，不是坏状态。

    /// 装载类目数据（TSV 串；文件读取在宿主侧 —— engine-core 禁 IO）。
    /// 返回跳过的坏行数；**替换**数据集并复位浏览态。buffer 随之变化 → 页码归零。
    pub fn load_symbol_categories(&mut self, tsv: &str) -> usize {
        let skipped = self.engine.load_symbol_categories(tsv);
        self.reset_page_if_buffer_changed();
        skipped
    }

    /// 类目数据是否可用（入口闸门：无数据 ⇒ false ⇒ 宿主把入口禁用）。
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

    /// 退出浏览（幂等、**同步**：返回时 buffer() 已是真缓冲 —— 切走前的强制前置）。
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
    pub(crate) fn fetched(&self) -> Vec<Candidate> {
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
    pub fn page_count(&self) -> usize {
        self.fetched().len().div_ceil(PAGE_SIZE)
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

// 键路由（KeyOutcome + KEY_*/KEY_STATE_* 常量 + input_key/handle_* ）在子模块，
// 对应 fcitx5 轨的 input_method.rs；`pub use` 保持 `crate::logic::KEY_*` 等
// 既有路径不变（vk.rs / tsf.rs 直接引用）。
#[path = "logic_input_method.rs"]
pub mod input_method;
pub use input_method::*;

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：logic_candidate_tests.rs
// 为候选分页状态机测试（键路由测试见 input_method 模块）。
#[cfg(test)]
#[path = "logic_candidate_tests.rs"]
mod candidate_tests;

// 候选上限门禁独立成文件（`#[path]` 引入）以保持本文件 <500 行；两轨同源不合并，
// 配对：`candidate_limits_tests.rs`（Linux 轨）↔ `logic_limits_tests.rs`（Windows 轨）。
#[cfg(test)]
#[path = "logic_limits_tests.rs"]
mod limits_tests;
