//! 引擎薄壳：类型转换与边界校验，内部持 engine_core::Engine。
//! 双 ABI（JNI + C）共享同一引擎单例（SINGLETON）与内部实现，避免双份逻辑。

use std::sync::Mutex;

use engine_core::candidates::{Candidate, CandidateKind};
use engine_core::composer::Mode;
use engine_core::dictionary::Dictionary;
use engine_core::symbols::{Block, BlockId, SymbolEntry};
use engine_core::Engine;

/// 引擎单例：load 后可供 JNI / C 出口共享。
pub static SINGLETON: Mutex<Option<Api>> = Mutex::new(None);

/// 装载引擎。`None`/空串 → 内置回退词库（35 词）；非空路径 → load_or_fallback
/// 原样语义（坏路径返回 Err，仅内置损坏时方为不可恢复）。成功即替换单例。
pub fn install(path: Option<&str>) -> Result<(), String> {
    let dict: Box<dyn Dictionary> = match path {
        Some(p) if !p.is_empty() => engine_data::load_or_fallback(Some(std::path::Path::new(p)))?,
        _ => Box::new(engine_data::fallback_dict()),
    };
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    // 毒化恢复：18 个 FFI 入口的 catch_unwind 吞 panic 时锁已毒化，
    // into_inner 取回数据，install 整体替换引擎，提供恢复路径。
    let mut guard = SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    *guard = Some(Api { engine: Engine::new(dict, symbols, true) });
    Ok(())
}

/// 装载繁体词典并挂到已安装引擎上（不替换主词典，简体模式不受影响）。
/// 严格加载（load_mmap，坏路径不回落内置——内置是简体 35 词，装成繁体语义错误）；
/// 引擎未 load 时返回 Err（调用方按 false 处理，繁体模式回退简体库）。
pub fn install_trad(path: &str) -> Result<(), String> {
    let dict = engine_data::load_mmap(std::path::Path::new(path))
        .map_err(|e| format!("load trad {}: {e:?}", path))?;
    let mut guard = SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    match guard.as_mut() {
        Some(api) => {
            api.set_trad_dict(Some(Box::new(dict)));
            Ok(())
        }
        None => Err("engine not loaded".into()),
    }
}

/// 在引擎单例上执行操作；未 load 时返回 None（调用方按哨兵处理）。
pub fn with_engine<R>(f: impl FnOnce(&mut Api) -> R) -> Option<R> {
    let mut g = SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    g.as_mut().map(f)
}

/// JNI/C 共用的 0..=4 模式整数 ↔ Mode 转换（0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional）。
pub fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::English),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        4 => Some(Mode::Traditional),
        _ => None,
    }
}

pub fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::English => 1,
        Mode::Number => 2,
        Mode::Symbol => 3,
        Mode::Traditional => 4,
    }
}

/// 候选文本列表（JNI/C 出口共用；kind/score UI 不用，仅文本）。
pub fn candidate_texts(api: &Api, limit: usize) -> Vec<String> {
    api.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 符号块内符号文本列表。
pub fn symbol_texts(api: &Api, id: u16) -> Vec<String> {
    api.symbols_in_block(id).into_iter().map(|s| s.text).collect()
}

/// 符号搜索命中文本列表。
pub fn search_symbol_texts(api: &Api, keyword: &str) -> Vec<String> {
    api.search_symbols(keyword.to_string()).into_iter().map(|s| s.text).collect()
}

/// 文本列表 → JSON 数组字符串。
pub fn texts_json(texts: &[String]) -> String {
    serde_json::to_string(texts).unwrap_or_default()
}

/// symbolBlocks JSON：`[{id,start,end,name,common}]`。
pub fn symbol_blocks_json(api: &Api) -> String {
    let blocks = api.symbol_blocks();
    let arr: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "start": b.start,
                "end": b.end,
                "name": b.name,
                "common": b.common,
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiMode {
    Pinyin,
    Traditional,
    English,
    Number,
    Symbol,
}

impl From<Mode> for ApiMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Pinyin => ApiMode::Pinyin,
            Mode::Traditional => ApiMode::Traditional,
            Mode::English => ApiMode::English,
            Mode::Number => ApiMode::Number,
            Mode::Symbol => ApiMode::Symbol,
        }
    }
}

impl From<ApiMode> for Mode {
    fn from(m: ApiMode) -> Self {
        match m {
            ApiMode::Pinyin => Mode::Pinyin,
            ApiMode::Traditional => Mode::Traditional,
            ApiMode::English => Mode::English,
            ApiMode::Number => Mode::Number,
            ApiMode::Symbol => Mode::Symbol,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiCandidateKind {
    Hanzi,
    English,
    Emoji,
    Symbol,
}

impl From<CandidateKind> for ApiCandidateKind {
    fn from(k: CandidateKind) -> Self {
        match k {
            CandidateKind::Hanzi => ApiCandidateKind::Hanzi,
            CandidateKind::English => ApiCandidateKind::English,
            CandidateKind::Emoji => ApiCandidateKind::Emoji,
            CandidateKind::Symbol => ApiCandidateKind::Symbol,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiCandidate {
    pub text: String,
    pub kind: ApiCandidateKind,
    pub score: u64,
}

impl From<Candidate> for ApiCandidate {
    fn from(c: Candidate) -> Self {
        ApiCandidate { text: c.text, kind: c.kind.into(), score: c.score }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiBlock {
    pub id: u16,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub common: bool,
}

impl From<Block> for ApiBlock {
    fn from(b: Block) -> Self {
        ApiBlock { id: b.id.0, start: b.start, end: b.end, name: b.name, common: b.common }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSymbolEntry {
    pub text: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub block: u16,
    pub emoji: bool,
}

impl From<SymbolEntry> for ApiSymbolEntry {
    fn from(s: SymbolEntry) -> Self {
        ApiSymbolEntry { text: s.text, name: s.name, keywords: s.keywords, block: s.block.0, emoji: s.emoji }
    }
}

/// 引擎句柄。同步核心；Rust 测试测同步核心。
pub struct Api {
    engine: Engine,
}

impl Api {
    pub fn load_fallback_sync() -> Result<Api, String> {
        let dict = engine_data::fallback_dict();
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        Ok(Api { engine: Engine::new(Box::new(dict), symbols, true) })
    }

    pub fn load_sync(path: String) -> Result<Api, String> {
        let dict = engine_data::load_or_fallback(Some(std::path::Path::new(&path)))?;
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        Ok(Api { engine: Engine::new(dict, symbols, true) })
    }

    pub fn input_key(&mut self, ch: String) -> String {
        let mut chars = ch.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return String::new(); // 边界：拒绝空串/多字符；非 ASCII 拒绝在引擎层（引擎只收 ASCII 键）
        };
        self.engine.input_key(c)
    }

    pub fn input_space(&mut self) -> String {
        self.engine.input_space()
    }

    pub fn backspace(&mut self) {
        self.engine.backspace();
    }

    pub fn clear(&mut self) {
        self.engine.clear();
    }

    pub fn switch_mode(&mut self, mode: ApiMode) {
        self.engine.switch_mode(mode.into());
    }

    /// 换装繁体词典（trad.opid）。None = 清除（繁体模式回退简体库）。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        self.engine.set_trad_dict(dict);
    }

    pub fn set_shift(&mut self, on: bool) {
        self.engine.set_shift(on);
    }

    pub fn buffer(&self) -> String {
        self.engine.buffer().to_string()
    }

    pub fn mode(&self) -> ApiMode {
        self.engine.mode().into()
    }

    pub fn candidates(&self, limit: usize) -> Vec<ApiCandidate> {
        self.engine.candidates(limit).into_iter().map(Into::into).collect()
    }

    pub fn select(&mut self, index: usize) -> String {
        self.engine.select(index)
    }

    pub fn set_learner(&mut self, enabled: bool) {
        self.engine.set_learner(enabled);
    }

    pub fn learner_enabled(&self) -> bool {
        self.engine.learner_enabled()
    }

    pub fn remove_user_word(&mut self, text: String) {
        self.engine.remove_user_word(&text);
    }

    pub fn clear_user_words(&mut self) {
        self.engine.clear_user_words();
    }

    pub fn export_user_words(&self) -> String {
        self.engine.export_user_words()
    }

    pub fn symbol_blocks(&self) -> Vec<ApiBlock> {
        self.engine.symbol_blocks().into_iter().map(Into::into).collect()
    }

    pub fn symbols_in_block(&self, id: u16) -> Vec<ApiSymbolEntry> {
        self.engine.symbols_in_block(BlockId(id)).into_iter().map(Into::into).collect()
    }

    pub fn search_symbols(&self, keyword: String) -> Vec<ApiSymbolEntry> {
        self.engine.search_symbols(&keyword).into_iter().map(Into::into).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 单例串行化：install/install_trad 与 install_singleton_fallback_and_path
    /// 共享 SINGLETON，并行执行时相互复位会制造竞态，加锁串行。
    static SINGLETON_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn api() -> Api {
        Api::load_fallback_sync().expect("fallback load")
    }

    #[test]
    fn load_fallback_sync_ok() {
        let a = api();
        assert_eq!(a.buffer(), "");
        assert_eq!(a.mode(), ApiMode::Pinyin);
        assert!(a.learner_enabled()); // M1 默认开
    }

    #[test]
    fn pinyin_input_produces_candidates() {
        let mut a = api();
        a.input_key("w".into());
        a.input_key("o".into());
        assert_eq!(a.buffer(), "wo");
        let cands = a.candidates(3);
        assert_eq!(cands[0].text, "我");
        assert_eq!(cands[0].kind, ApiCandidateKind::Hanzi);
        assert!(cands[0].score > 0);
    }

    #[test]
    fn select_commits_and_records_learner() {
        let mut a = api();
        for c in ["w", "o"] { a.input_key(c.into()); }
        let text = a.select(0);
        assert_eq!(text, "我");
        assert_eq!(a.buffer(), "");
        assert!(a.export_user_words().contains("我"));
        a.remove_user_word("我".into());
        assert!(!a.export_user_words().contains("我"));
        a.clear_user_words();
        assert_eq!(a.export_user_words(), r#"{"version":1,"words":[]}"#);
    }

    #[test]
    fn input_key_boundary_rejects_non_single_char() {
        let mut a = api();
        assert_eq!(a.input_key("".into()), "");
        assert_eq!(a.input_key("ab".into()), "");
        assert_eq!(a.input_key("你".into()), ""); // 非 ASCII 也拒绝
        assert_eq!(a.buffer(), "");
    }

    #[test]
    fn select_out_of_range_returns_empty() {
        let mut a = api();
        for c in ["w", "o"] { a.input_key(c.into()); }
        assert_eq!(a.select(999), "");
    }

    #[test]
    fn mode_and_shift_and_space() {
        let mut a = api();
        a.switch_mode(ApiMode::English);
        for c in ["a", "b", "c"] { a.input_key(c.into()); }
        assert_eq!(a.buffer(), "abc");
        assert_eq!(a.input_space(), "abc");
        assert_eq!(a.buffer(), "");
        a.set_shift(true);
        a.input_key("a".into());
        assert_eq!(a.buffer(), "A");
        a.set_shift(false);
        a.switch_mode(ApiMode::Pinyin);
    }

    #[test]
    fn backspace_and_clear() {
        let mut a = api();
        a.input_key("w".into());
        a.backspace();
        assert_eq!(a.buffer(), "");
        a.input_key("w".into());
        a.clear();
        assert_eq!(a.buffer(), "");
    }

    #[test]
    fn symbol_search_and_blocks() {
        let a = api();
        let blocks = a.symbol_blocks();
        assert!(!blocks.is_empty());
        let hits = a.search_symbols("he".into());
        assert!(hits.iter().any(|s| s.text == "♥"));
        let b0 = blocks[0].clone();
        assert!(!a.symbols_in_block(b0.id).is_empty());
    }

    #[test]
    fn set_learner_toggles() {
        let mut a = api();
        a.set_learner(false);
        assert!(!a.learner_enabled());
        a.set_learner(true);
        assert!(a.learner_enabled());
    }

    #[test]
    fn install_singleton_fallback_and_path() {
        let _g = SINGLETON_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        // 空串/None → 内置回退
        assert!(install(None).is_ok());
        assert!(with_engine(|a| a.buffer()).is_some());
        // 坏路径 → Err（load_or_fallback 原样语义，不回退）
        assert!(install(Some("/nonexistent/opi.dict")).is_err());
        // 单例保持上一次成功装载可用
        assert!(with_engine(|a| a.buffer()).is_some());
    }

    #[test]
    fn mode_int_roundtrip() {
        assert_eq!(mode_from_int(0), Some(Mode::Pinyin));
        assert_eq!(mode_from_int(3), Some(Mode::Symbol));
        assert_eq!(mode_from_int(4), Some(Mode::Traditional));
        assert_eq!(mode_from_int(5), None);
        assert_eq!(mode_from_int(-1), None);
        assert_eq!(mode_to_int(Mode::English), 1);
        assert_eq!(mode_to_int(Mode::Number), 2);
        assert_eq!(mode_to_int(Mode::Traditional), 4);
    }

    #[test]
    fn install_trad_hooks_trad_dict() {
        let _g = SINGLETON_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        // 引擎未加载 → Err
        *SINGLETON.lock().unwrap() = None;
        assert!(install_trad("/nonexistent/trad.opid").is_err());
        // 先装主库：坏路径 → Err 且简体模式不受影响
        install(None).unwrap();
        assert!(install_trad("/nonexistent/trad.opid").is_err());
        // 真路径：临时编译一个小词典（engine_data 序列化，opi-ffi 不依赖 opi-tools）
        let dict = engine_data::format::OpDict {
            entries: vec![engine_data::format::RawEntry {
                pinyin: "fa".into(),
                word: "發".into(),
                freq: 4000,
            }],
            pinyin_total: 2,
        };
        let tmp = std::env::temp_dir().join("opi_trad_test.opid");
        std::fs::write(&tmp, engine_data::serialize(&dict)).unwrap();
        install_trad(tmp.to_str().unwrap()).unwrap();
        // 繁体模式候选走 trad 词典
        let top = with_engine(|a| {
            a.switch_mode(ApiMode::Traditional);
            a.input_key("f".into());
            a.input_key("a".into());
            a.candidates(8)[0].text.clone()
        })
        .expect("engine loaded");
        assert_eq!(top, "發");
        std::fs::remove_file(&tmp).ok();
        with_engine(|a| a.switch_mode(ApiMode::Pinyin));
    }

    #[test]
    fn install_trad_recomputes_user_boost() {
        let _g = SINGLETON_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        // 高静态词频（4e9，trad.opid 同量级）安装后，learner 选词仍压过静态词。
        *SINGLETON.lock().unwrap() = None;
        install(None).unwrap();
        let dict = engine_data::format::OpDict {
            entries: vec![
                engine_data::format::RawEntry { pinyin: "fa".into(), word: "發".into(), freq: 4_000_000_000 },
                engine_data::format::RawEntry { pinyin: "fa".into(), word: "髮".into(), freq: 3_999_000_000 },
            ],
            pinyin_total: 4, // 两条 "fa" 拼音 blob 共 4 字节（serialize debug_assert 校验）
        };
        let tmp = std::env::temp_dir().join("opi_trad_boost.opid");
        std::fs::write(&tmp, engine_data::serialize(&dict)).unwrap();
        install_trad(tmp.to_str().unwrap()).unwrap();
        std::fs::remove_file(&tmp).ok();
        with_engine(|a| {
            a.set_learner(true);
            a.switch_mode(ApiMode::Traditional);
            a.input_key("f".into());
            a.input_key("a".into());
            assert_eq!(a.candidates(8)[0].text, "發");
            a.select(1); // 选 髮
            a.input_key("f".into());
            a.input_key("a".into());
            assert_eq!(a.candidates(8)[0].text, "髮", "learner 选词应压过 4e9 静态词");
            a.set_learner(false);
        });
        with_engine(|a| a.switch_mode(ApiMode::Pinyin));
    }
}
