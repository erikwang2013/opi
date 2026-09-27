// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::candidates::{Candidate, DEFAULT_TOP_N, USER_BOOST, rank_and_pick};
use crate::composer::{Composer, KeyEffect, Mode};
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::punctuation::{self, Quotes};
use crate::symbols::{Block, BlockId, SymbolEngine, SymbolEntry};

/// 引擎门面：输入法 UI 层（经 FFI）交互的唯一入口。
pub struct Engine {
    dict: Box<dyn Dictionary>,
    /// 繁体词典（trad.opid）；None 时 Traditional 模式回退 dict（spec 错误处理）。
    trad_dict: Option<Box<dyn Dictionary>>,
    composer: Composer,
    symbols: SymbolEngine,
    learner: Learner,
    /// 用户词频权重，按词典最大静态词频动态缩放（一次选词即压过所有静态词）。
    user_boost: u64,
    /// 全角开关：`true` = ASCII 标点出全角（中文模式出中文标点），`false` = 直通半角。
    /// 切模式重置为该模式的默认值（[`Mode::default_fullwidth`]）。
    fullwidth: bool,
    /// 成对引号的交替状态（复位时机见 [`Quotes`]）。
    quotes: Quotes,
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
            composer: Composer::new(),
            symbols,
            learner: Learner::new(learner_enabled),
            user_boost,
            fullwidth: Mode::default().default_fullwidth(),
            quotes: Quotes::default(),
        }
    }

    /// 换装繁体词典（FFI install_trad 用）。None = 清除（回退简体）。
    /// 重算 user_boost：trad.opid 静态最大词频 4e9，保持"一次选词压过全部静态词"。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        let max_freq = self
            .dict
            .max_freq()
            .max(dict.as_ref().map_or(0, |d| d.max_freq()));
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

    /// 标点键 → 需要上屏的文本；`None` = 本模式/本状态下不归标点表管，交调用方直通。
    ///
    /// **四端的单一判定点**：三轨（fcitx5 / TSF / 平台中立路由）在可打印分支开头问它，
    /// [`Self::input_key`] 也问它。加进来的是引擎层，不是某一端 —— 用户裁决
    /// 2026-09-27：「引擎层加标点表，四端走同一条路」。
    pub fn input_punct(&mut self, ch: char) -> Option<String> {
        let text = self.punct_text(ch)?;
        // 缓冲非空：先走与空格**同一条**收尾（有候选提首候选、无候选提原文），
        // 否则 `ni,` 上屏成 `，你` —— 标点抢在待提交的拼音前面。
        // 半角态不映射 ⇒ 不走这里：那是「直通」，由调用方自己插入，与三轨的 PassThrough 同形。
        let pending = if self.composer.session().buffer.is_empty() {
            String::new()
        } else {
            self.input_space()
        };
        Some(pending + &text)
    }

    /// 标点映射本尊。`None` = 直通。
    fn punct_text(&mut self, ch: char) -> Option<String> {
        // 半角：一个都不映射（全角开关的关状态）。
        if !self.fullwidth {
            return None;
        }
        // 撇号在拼音/繁体是**音节分隔符**（`xi'an`）：只有缓冲空时它没有分隔语义，
        // 这时才当引号 —— 两个需求都要，代价只有这一个条件。
        if ch == '\'' && !self.composer.session().buffer.is_empty() {
            return None;
        }
        let mapped = match self.composer.session().mode {
            // 中文标点：标准中文标点优先、机械全角兜底（见 `punctuation.rs`）
            Mode::Pinyin | Mode::Traditional => punctuation::chinese(ch, &mut self.quotes)?,
            // 西文模式：机械全角，`.` 得 `．`(U+FF0E) 而不是中文句号 `。`
            Mode::English | Mode::Number => punctuation::ascii_fullwidth(ch)?,
            // 符号模式：**不映射**（我的裁决，依据是数据不是口味）：生产符号表的全部
            // 关键字一律 `^[a-z0-9]+$`（tests/punctuation.rs 的 every_symbol_keyword_is_alnum
            // 钉住），标点在那里既搜不出候选、也不是待上屏的中文文本。交调用方原样插入
            // （Android 面板 / [`Self::input_key`]）或直通客户端（三轨），与 Number 同档。
            // 全角开关在符号模式因此不生效：它是搜索模式，不是文本模式。
            Mode::Symbol => return None,
        };
        Some(mapped.to_string())
    }

    /// 全角开关的当前值（`toggle_fullwidth` 的读侧，供状态栏/测试）。
    pub fn fullwidth(&self) -> bool {
        self.fullwidth
    }

    /// 触发键：全角 ↔ 半角，返回**切换后的状态**（状态栏显示用）。
    ///
    /// 语义只在本层，**键位不在本层**（与 [`Self::toggle_symbol`] 同一条：各端键位
    /// 未定，本层只出语义）。调用方拿到状态后自行刷新 UI；不需要插入通道，故比
    /// `toggle_symbol` 更容易接线。
    pub fn toggle_fullwidth(&mut self) -> bool {
        self.fullwidth = !self.fullwidth;
        self.fullwidth
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
    /// 各端各写一份必漂）；**键位不在本层**（计划 B5 仍未定，TSF 侧还要动 `vk.rs`
    /// 的映射表），本层只出语义。调用方负责把非空返回插入文档；拿不到插入通道的端
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

    pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
        let s = self.composer.session();
        rank_and_pick(
            self.active_dict(),
            &self.symbols,
            &self.learner,
            &s.buffer,
            s.mode,
            limit,
            self.user_boost,
        )
    }

    /// 选中候选项。越界返回空串。记录学习（若开启）。
    /// limit 512：rank_and_pick 仍全量排序（正确性），这里只限 FFI 载荷。
    pub fn select(&mut self, index: usize) -> String {
        let cands = self.candidates(512);
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
