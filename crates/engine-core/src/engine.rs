// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::candidates::{
    Candidate, DEFAULT_TOP_N, RankOptions, USER_BOOST, rank_and_pick_scheme, rank_and_pick_with,
};
use crate::composer::{Composer, KeyEffect, Mode};
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::punctuation::Quotes;
use crate::scheme::Scheme;
use crate::symbols::{Block, BlockId, SymbolEngine, SymbolEntry};

/// 引擎门面：输入法 UI 层（经 FFI）交互的唯一入口。
pub struct Engine {
    dict: Box<dyn Dictionary>,
    /// 繁体词典（trad.opid）；None 时 Traditional 模式回退 dict（spec 错误处理）。
    trad_dict: Option<Box<dyn Dictionary>>,
    /// 笔画词典（stroke.opid，2026-10-09）：键是笔画串（`hspnz` 五个字母，
    /// 横竖撇捺折），词是单字。`Scheme::Stroke` 时改查它。None = 笔画方案回退
    /// 普通路径（可选增强，与 trad 同一姿态）。
    stroke_dict: Option<Box<dyn Dictionary>>,
    composer: Composer,
    symbols: SymbolEngine,
    learner: Learner,
    /// 用户词频权重，按词典最大静态词频动态缩放（一次选词即压过所有静态词）。
    user_boost: u64,
    /// 9 键方案的「拼音读数」（显示用，见 [Self::display_buffer] 与
    /// [Self::candidate_readings]）：由 `candidates()` 在九键分支里刷新 ——
    /// **逐条**（`readings[i]` = `candidates(limit)` 第 i 条的拼音，已分词；
    /// 同一次排名产出，见 `rank_and_pick_scheme`）。
    /// **存 `(算它时的缓冲, 逐条读数)` 一对** ——
    /// 缓冲一变（提交/退格/清空）旧读数立即作废，两个出口都返回空，
    /// **不依赖调用方「之后又取了一次候选」的次序**（2026-10-09 用户报障「确定候选词后
    /// 左列不恢复标点」：成因正是这个次序假设 —— 提交清缓冲时没清读数，而下一次
    /// 刷新先读 display_buffer 再取候选）。
    /// `RefCell` 是因为 `candidates()` 是 `&self`
    /// （读侧出口遍布 FFI，改 `&mut` 会连锁）；引擎单例在 Mutex 里，Send 足够。
    pub(crate) scheme_readings: std::cell::RefCell<(String, Vec<String>)>,
    /// 全角开关：`true` = ASCII 标点出**机械全角**（U+FF01..U+FF5E 平移），
    /// `false` = 直通半角。只管宽度 —— 中文标点表是另一档（[`Self::chinese_punct`]）。
    /// 切模式重置为该模式的默认值（[`Mode::default_fullwidth`]）。
    pub(crate) fullwidth: bool,
    /// 中文标点表开关（`,` → `，`、`\` → `、`，含成对引号交替）：`true` = 表生效。
    ///
    /// **全局布尔、不随模式重置**（与 [`Self::fullwidth`] 有意不同）：表只被
    /// Pinyin / Traditional 读，对它而言这是**用户偏好**而不是某个模式的默认值 ——
    /// 切模式重置一个没有 per-mode 含义的偏好是错的（切到英文再切回来，
    /// 用户关掉的表会自己打开）。默认 `true` = 保持既有行为。
    pub(crate) chinese_punct: bool,
    /// 成对引号的交替状态（复位时机见 [`Quotes`]）。
    pub(crate) quotes: Quotes,
    /// 模糊音开关（「属性设置」可改；排序层的门控见 `candidates::RankOptions`）。
    /// 默认 `true` = 既有行为。
    fuzzy: bool,
    /// 简拼开关。默认 `true` = 既有行为。
    jianpin: bool,
}

impl Engine {
    /// 单词典构造（trad=None），JVM/现有调用向后兼容。
    pub fn new(dict: Box<dyn Dictionary>, symbols: SymbolEngine, learner_enabled: bool) -> Self {
        Self::with_dictionaries(dict, None, symbols, learner_enabled)
    }

    /// 双词典构造：Traditional 模式查 trad（None 时回退 dict），其余模式查 dict。
    pub fn with_dictionaries(
        dict: Box<dyn Dictionary>,
        trad: Option<Box<dyn Dictionary>>,
        symbols: SymbolEngine,
        learner_enabled: bool,
    ) -> Self {
        let max_freq = dict
            .max_freq()
            .max(trad.as_ref().map_or(0, |d| d.max_freq()));
        let user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        Engine {
            dict,
            trad_dict: trad,
            stroke_dict: None,
            composer: Composer::new(),
            symbols,
            learner: Learner::new(learner_enabled),
            user_boost,
            scheme_readings: std::cell::RefCell::new((String::new(), Vec::new())),
            fullwidth: Mode::default().default_fullwidth(),
            chinese_punct: true,
            quotes: Quotes::default(),
            fuzzy: true,
            jianpin: true,
        }
    }

    /// 换装繁体词典（FFI install_trad 用）。None = 清除（回退简体）。
    /// 重算 user_boost：trad.opid 静态最大词频 4e9，保持"一次选词压过全部静态词"。
    /// ⚠️ 取三个词典的**全局**最大词频（含 stroke，2026-10-09 起）：只按「dict+trad」
    /// 算的话，后装的 stroke 若词频更高，boost 会悄悄偏小 —— 学习者压不过静态词。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        let max_freq = [dict.as_ref(), self.stroke_dict.as_ref()]
            .into_iter()
            .flatten()
            .chain(std::iter::once(&self.dict))
            .map(|d| d.max_freq())
            .max()
            .unwrap_or(0);
        self.user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        self.trad_dict = dict;
    }

    /// 当前模式生效的词典。
    fn active_dict(&self) -> &dyn Dictionary {
        match self.composer.session().mode {
            Mode::Traditional => self.trad_dict.as_deref().unwrap_or(&*self.dict),
            _ => &*self.dict,
        }
    }

    /// 换装笔画词典（FFI install_stroke 用）。None = 清除（笔画方案回退普通路径）。
    /// 与 [`Self::set_trad_dict`] 同一形状（含重算 user_boost —— 别让更大的词频表
    /// 静默改变「精确加成」的档位）。
    pub fn set_stroke_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        let max_freq = [self.trad_dict.as_ref(), dict.as_ref()]
            .into_iter()
            .flatten()
            .chain(std::iter::once(&self.dict))
            .map(|d| d.max_freq())
            .max()
            .unwrap_or(0);
        self.user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        self.stroke_dict = dict;
    }

    /// 当前输入方案（9 键 / 14 键 / 笔画 / 全键盘）。
    pub fn scheme(&self) -> Scheme {
        self.composer.scheme()
    }

    /// 换输入方案。**清缓冲**（与 `switch_mode` 同一条理由）：缓冲是按旧方案
    /// 打进去的串，留着会被新方案的匹配器读出个似是而非的结果（例如把 26 键的
    /// "ni" 拿 9 键规则去套，凑出一堆不相关的候选）。清掉 = 明确的重新开始。
    pub fn set_scheme(&mut self, scheme: Scheme) {
        self.composer.set_scheme(scheme);
        self.composer.clear();
        // 读数与缓冲同生命周期：不清的话换方案后的一次读会拿到上一条输入的拼音。
        *self.scheme_readings.borrow_mut() = (String::new(), Vec::new());
    }

    // ---- 标点表 / 全角一族：impl 体在 engine_punct.rs（500 行门禁拆出）----
    // 字段的 pub(crate) 理由见那个文件的头注释。

    /// 处理一次击键，返回需要提交的文本（空串 = 无提交）。
    /// 空格键在 Engine 层拦截：拼音模式选首候选，其他模式提交缓冲。
    ///
    /// **不吞标点**：既没进缓冲、也没有标点映射的 ASCII 标点，原样交回调用方。
    /// 调用方是「没有客户端可交」的那些端（Android 面板经 JNI、C ABI 直调）——
    /// 三轨的「直通」是把自己交还给客户端应用，Android 没有这一层，返回空串
    /// 就等于吞键（NumberPad 的 `,` `.` 正是靠这条活在引擎里）。
    pub fn input_key(&mut self, ch: char) -> String {
        if ch == ' ' {
            return self.input_space();
        }
        if let Some(text) = self.input_punct(ch) {
            return text;
        }
        let (effect, _session) = self.composer.input_key(ch);
        match effect {
            KeyEffect::Updated => String::new(),
            // 标点类**永远有归宿**：要么被标点表映射（上面那条），要么原样交回调用方。
            // 其余字符（字母/数字/控制符/非 ASCII）不在内 —— 它们各有去处（进缓冲，
            // 或这个模式明确不收），**且不能靠这条救**：缓冲满时 composer 同样返回
            // `Ignored`，那时交回一个字母会把「缓冲满」误当成「这个字符没处放」，
            // 顺手 flush 掉用户打到一半的拼音（实测：满缓冲再打一个字母会把 16 个
            // 字符全提交掉）。
            // 与标点表同一条收尾：先走空格的收尾，标点落在待提交的缓冲**后面**。
            KeyEffect::Ignored if ch.is_ascii_punctuation() => self.input_space() + &ch.to_string(),
            KeyEffect::Ignored => String::new(),
        }
    }

    /// 空格键：拼音/繁体/符号模式选中首候选；英文/数字模式提交缓冲。
    ///
    /// 无候选时的行为**有意分叉**：拼音/繁体提交原始缓冲（这是「打英文」的逃生口），
    /// 符号模式什么都不提交 —— 缓冲里是关键字（`dunx`），不是要上屏的文本。
    pub fn input_space(&mut self) -> String {
        match self.composer.session().mode {
            Mode::Pinyin | Mode::Traditional => {
                let buffer = self.composer.session().buffer.clone();
                let cands = self.candidates(DEFAULT_TOP_N);
                if cands.is_empty() {
                    self.composer.commit_buffer();
                    buffer
                } else {
                    // 走 select_from：候选提交的收尾（记学习 / 清缓冲 / 记页码）只此一份
                    self.select_from(&cands, 0)
                }
            }
            // 符号模式：候选就是符号（缓冲是关键字）。无候选 → 空串，路由层据此回
            // EngineHandled，**且不清缓冲**：关键字仍留在预编辑里，用户退格改一个
            // 字母就能重来（清了等于静默吞掉用户打的字）。
            Mode::Symbol => {
                let cands = self.candidates(DEFAULT_TOP_N);
                if cands.is_empty() {
                    String::new()
                } else {
                    self.select_from(&cands, 0)
                }
            }
            _ => {
                let buffer = self.composer.session().buffer.clone();
                self.composer.commit_buffer();
                buffer
            }
        }
    }

    pub fn backspace(&mut self) {
        self.composer.backspace();
    }

    pub fn clear(&mut self) {
        self.composer.clear();
    }

    /// 切模式：全角开关跟着模式默认值重置、引号交替复位（与「清 shift」同一条理由 ——
    /// 跨模式残留的粘滞态会让用户切回来发现「打字变成另一个样子」）。
    ///
    /// **`chinese_punct` 不在此列**：它是用户偏好而非模式默认值（理由见字段注释）。
    pub fn switch_mode(&mut self, mode: Mode) {
        self.fullwidth = mode.default_fullwidth();
        self.quotes.reset();
        self.composer.switch_mode(mode);
    }

    /// 触发键：在拼音与符号模式之间切换，返回**需要上屏的文本**（空串 = 无提交）。
    ///
    /// 切模式前先把未提交的缓冲处理掉 —— 照抄 Android
    /// `ImeState.commitPendingBuffer()`（`ImeState.kt:180-189`，`openSymbol` 开面板前先调它）：
    /// 有候选则提交首候选；无候选的乱码缓冲（如 `zzz`）直接清掉而**不上屏**
    /// ——把拼音原文塞进文档比丢掉更糟。`Composer::switch_mode` 只清缓冲，
    /// 拿到这里就是「打到一半的拼音被静默丢弃」，所以这层必须在它之前。
    ///
    /// 放 Engine 层是因为它同时被四端共用（与 [`Self::select_from`] 同一个理由：
    /// 各端各写一份必漂）；**键位不在本层**，本层只出语义。
    ///
    /// ⚠️ **桌面两轨目前没有任何按键指向本函数**（2026-09-28 核对的调用点）：
    /// `Ctrl+\` 两轨都走 [`Self::switch_mode`] —— `tsf-opi/src/vk.rs` 的 `mode_hotkey`
    /// → `tsf.rs` 的 `toggle_mode`、`fcitx5-opi/cpp/opi_fcitx5.cpp` 的
    /// `handleModeHotkey` —— 那条路**不收尾**：`Composer::switch_mode` 只清缓冲，
    /// 打到一半、甚至已经有候选的拼音都一起丢掉（本函数与 `switch_mode` 的语义差别
    /// 就是这个收尾）。它是给**有插入通道、且要保住待提交文本**的端用的，现有入口
    /// 只有两个出口（C ABI `opi_toggle_symbol`、JNI `toggleSymbol`）。Apple 两端接
    /// 热键时要在「照桌面丢缓冲」与「先上屏」之间**明选一个**，别照抄 `Ctrl+\`，
    /// 也别选可打印键（理由同上 [`Self::toggle_fullwidth`]）。
    /// 调用方负责把非空返回插入文档；拿不到插入通道的端
    /// 不要调它 —— 调了等于只记学习记录却不上屏。
    pub fn toggle_symbol(&mut self) -> String {
        let target = if self.composer.session().mode == Mode::Symbol {
            Mode::Pinyin
        } else {
            Mode::Symbol
        };
        let pending = if self.composer.session().buffer.is_empty() {
            String::new()
        } else {
            let cands = self.candidates(DEFAULT_TOP_N);
            if cands.is_empty() {
                self.composer.clear();
                String::new()
            } else {
                // 与 input_space 同一条收尾；符号模式下 select_from 顺带回拼音，
                // 下面的 switch_mode(target) 此时是幂等的
                self.select_from(&cands, 0)
            }
        };
        self.switch_mode(target);
        pending
    }

    pub fn set_shift(&mut self, on: bool) {
        self.composer.set_shift(on);
    }

    pub fn buffer(&self) -> &str {
        &self.composer.session().buffer
    }

    pub fn mode(&self) -> Mode {
        self.composer.session().mode
    }

    /// **语言切换**（中↔英 / 中↔繁，2026-10-09 用户裁决「中文输入的时候切换英文，
    /// 拼音保留可继续使用」）：换模式与方案，但**保留缓冲** —— 与 [`Self::switch_mode`] /
    /// [`Self::set_scheme`] 的区别只有这一步。
    ///
    /// 为什么这里可以不清：调用方保证**成对**切换（中→英换到英文方案、英→中换回原方案；
    /// 中→繁只动模式不动方案），缓冲在两头仍是同一套方案语义下的串；而清理的理由
    /// （见 [`Self::set_scheme`] 的注释）是「按旧方案打的串被新方案匹配器读出似是而非的
    /// 结果」——成对切换不产生这个处境。用户可见的「输入模式」面板切换（26/9/14/笔画）
    /// **不走这里**，仍走清缓冲路径（且切换前调用方已 commit pending）。
    ///
    /// 9 键的读数缓存随之作废（同一个缓冲换过方案/模式后，旧读数不再对应当前语境）；
    /// 切回来时下一次 `candidates()` 会重算。
    pub fn switch_language(&mut self, mode: Mode, scheme: Scheme) {
        self.fullwidth = mode.default_fullwidth();
        self.quotes.reset();
        self.composer.set_scheme(scheme);
        self.composer.switch_mode_keep_buffer(mode);
        *self.scheme_readings.borrow_mut() = (String::new(), Vec::new());
    }

    pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
        let s = self.composer.session();
        // 输入方案分流（2026-10-09）：只在「会打拼音缓冲」的两个模式下有意义
        // （English/Number/Symbol 的缓冲不是拼音串）。
        if matches!(s.mode, Mode::Pinyin | Mode::Traditional) {
            match self.scheme() {
                // 9/14 键：按键类展开 —— 前缀查询表达不了（数字串不是任何拼音键的
                // 前缀），走全量遍历 + 类匹配（candidates::rank_and_pick_scheme）。
                // 「精确加成」按**键长相等**给（与全键盘的 pinyin_len == 输入长度同义）。
                Scheme::Nine | Scheme::Fourteen => {
                    let (cands, readings) = rank_and_pick_scheme(
                        self.active_dict(),
                        &self.learner,
                        &s.buffer,
                        limit,
                        self.user_boost,
                        self.scheme(),
                    );
                    // 读数随每次取候选刷新（逐条、已分词），**并与当时的缓冲绑存**
                    // （见 display_buffer：缓冲一变这对就作废，不许把旧缓冲的读数报给新状态）。
                    *self.scheme_readings.borrow_mut() = (
                        s.buffer.clone(),
                        readings
                            .iter()
                            .map(|py| crate::pinyin::segment(py).join("'"))
                            .collect(),
                    );
                    return cands;
                }
                // 笔画：换词典 + **关掉模糊音/简拼**（两者对 hspnz 串没有意义，
                // 开着只会把不相干的音节捞进来）。其余与全键盘完全同路。
                Scheme::Stroke => {
                    if let Some(d) = &self.stroke_dict {
                        return rank_and_pick_with(
                            d.as_ref(),
                            &self.symbols,
                            &self.learner,
                            &s.buffer,
                            s.mode,
                            limit,
                            self.user_boost,
                            RankOptions {
                                fuzzy: false,
                                jianpin: false,
                            },
                        );
                    }
                    // 没装笔画词典：落到下面普通路径（候选基本为空）——
                    // 与 trad 缺失同一种「可选增强」姿态，不在引擎里报错。
                }
                Scheme::Full => {}
            }
        }
        rank_and_pick_with(
            self.active_dict(),
            &self.symbols,
            &self.learner,
            &s.buffer,
            s.mode,
            limit,
            self.user_boost,
            RankOptions {
                fuzzy: self.fuzzy,
                jianpin: self.jianpin,
            },
        )
    }

    /// 选中候选项。越界返回空串。记录学习（若开启）。
    ///
    /// **不设上限**（曾是 512，与 [`crate::router::FETCH_LIMIT`] 统一）：512 不省任何成本
    /// —— `rank_and_pick` 无论 limit 多大都全量收集 + 排序（正确性，见该模块「不能下推
    /// limit 到词库」），截断只是扔掉**已经算好**的尾巴 —— 却造出一条**静默失败的下标带**：
    /// `candidates(usize::MAX)` 里有第 513 项（真库 'y' 实测 8017 项），而这里返回空串，
    /// 不提交、不学习，调用方也无从分辨「没有这一项」与「我内部只取了 512 项」（返回值
    /// 只有一个字符串，没有地方承载这个区别）。下标空间与 [`crate::router::KeyRouter`]
    /// 的 `fetched()` 对齐后，同一份候选表算出的下标两条路必给同一个答案。
    /// 门禁：tests/select_index_bounds.rs 的 engine_select_index_space_agrees_with_unlimited_list。
    pub fn select(&mut self, index: usize) -> String {
        let cands = self.candidates(usize::MAX);
        self.select_from(&cands, index)
    }

    /// 在**调用方已算好的**候选表上选中第 `index` 项（全局下标）。
    /// 语义与 [`Self::select`] 逐字相同（越界 → 空串，命中 → 记学习 + 清缓冲），
    /// 唯一区别是不重算候选表 —— 数字选词在 `digit_select` 里已经抓过一次，
    /// 再排一遍整表是纯浪费。`Engine::select` 保留原签名（C 出口的全局索引用它）。
    pub fn select_from(&mut self, cands: &[Candidate], index: usize) -> String {
        match cands.get(index) {
            Some(c) => {
                let text = c.text.clone();
                self.learner.record_selection(&text);
                self.composer.commit_buffer();
                // 符号模式一次一符号（用户裁决 2026-09-27）：提交后回拼音，不跨键保持
                // ——下一次击键就是拼音，不需要先手动切回。放这一层是因为候选提交的收尾
                // 只此一处：四端（fcitx5 / TSF / Android / Apple）自动一致，
                // 前端各写一份必漂（`switch_mode` 顺带清 shift，与本层语义一致）。
                if self.composer.session().mode == Mode::Symbol {
                    self.switch_mode(Mode::Pinyin);
                }
                text
            }
            None => String::new(),
        }
    }

    pub fn set_learner(&mut self, enabled: bool) {
        self.learner.set_enabled(enabled);
    }

    /// 模糊音开关（「属性设置」的旋钮之一）。关掉 = `rank_and_pick_with` 跳过
    /// 整串变体与逐音节兜底**两个**入口（只关一个会让 `zongguo` 这类输入仍从
    /// 逐音节拿到模糊结果）。
    pub fn set_fuzzy(&mut self, enabled: bool) {
        self.fuzzy = enabled;
    }

    pub fn fuzzy_enabled(&self) -> bool {
        self.fuzzy
    }

    /// 简拼开关（同上）。
    pub fn set_jianpin(&mut self, enabled: bool) {
        self.jianpin = enabled;
    }

    pub fn jianpin_enabled(&self) -> bool {
        self.jianpin
    }

    pub fn learner_enabled(&self) -> bool {
        self.learner.is_enabled()
    }

    pub fn remove_user_word(&mut self, text: &str) {
        self.learner.remove_word(text);
    }

    pub fn clear_user_words(&mut self) {
        self.learner.clear();
    }

    pub fn export_user_words(&self) -> String {
        self.learner.export_json()
    }

    /// 导入 [`Engine::export_user_words`] 的产物（Android 侧启动时读文件后传入，
    /// engine 不做 IO）。合并语义、幂等性与失败原子性见 [`Learner::import_json`]。
    /// 返回导入条数；非法输入返回 Err 且不改动内存状态。
    pub fn import_user_words(&mut self, json: &str) -> Result<usize, String> {
        self.learner.import_json(json)
    }

    pub fn symbol_blocks(&self) -> Vec<Block> {
        self.symbols.common_blocks()
    }

    pub fn symbols_in_block(&self, id: BlockId) -> Vec<SymbolEntry> {
        self.symbols.entries_in_block(id)
    }

    pub fn search_symbols(&self, keyword: &str) -> Vec<SymbolEntry> {
        self.symbols.search(keyword)
    }
}
