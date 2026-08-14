use crate::candidates::{rank_and_pick, Candidate, USER_BOOST, DEFAULT_TOP_N};
use crate::composer::{Composer, KeyEffect, Mode};
use crate::dictionary::Dictionary;
use crate::learner::Learner;
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
        let max_freq = dict.max_freq().max(trad.as_ref().map_or(0, |d| d.max_freq()));
        let user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        Engine {
            dict,
            trad_dict: trad,
            composer: Composer::new(),
            symbols,
            learner: Learner::new(learner_enabled),
            user_boost,
        }
    }

    /// 换装繁体词典（FFI install_trad 用）。None = 清除（回退简体）。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
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
    pub fn input_key(&mut self, ch: char) -> String {
        if ch == ' ' {
            return self.input_space();
        }
        let (effect, _session) = self.composer.input_key(ch);
        match effect {
            KeyEffect::Updated | KeyEffect::Ignored => String::new(),
        }
    }

    /// 空格键：拼音/繁体模式选中首候选（无候选则提交原始缓冲）；英文/数字模式提交缓冲。
    pub fn input_space(&mut self) -> String {
        match self.composer.session().mode {
            Mode::Pinyin | Mode::Traditional => {
                let buffer = self.composer.session().buffer.clone();
                let cands = self.candidates(DEFAULT_TOP_N);
                if cands.is_empty() {
                    self.composer.commit_buffer();
                    buffer
                } else {
                    let top = cands[0].text.clone();
                    self.learner.record_selection(&top);
                    self.composer.commit_buffer();
                    top
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

    pub fn switch_mode(&mut self, mode: Mode) {
        self.composer.switch_mode(mode);
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
        match cands.get(index) {
            Some(c) => {
                let text = c.text.clone();
                self.learner.record_selection(&text);
                self.composer.commit_buffer();
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
