// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 引擎薄壳：类型转换与边界校验，内部持 engine_core::Engine。
//! 双 ABI（JNI + C）共享同一引擎单例（SINGLETON）与内部实现，避免双份逻辑。

use std::sync::Mutex;

use engine_core::Engine;
use engine_core::dictionary::Dictionary;
use engine_core::router::{KeyAction, KeyRouter, ShiftState};
use engine_core::symbols::BlockId;

mod convert;
#[cfg(test)]
mod tests;

pub use convert::{
    ApiBlock, ApiCandidate, ApiCandidateKind, ApiMode, ApiSymbolEntry, candidate_texts,
    emoji_symbol_texts, mode_from_int, mode_to_int, search_symbol_texts, symbol_blocks_json,
    symbol_texts, texts_json,
};

/// 引擎单例：load 后可供 JNI / C 出口共享。
pub static SINGLETON: Mutex<Option<Api>> = Mutex::new(None);

/// 装载引擎。`None`/空串 → 内置回退词库（35 词）；非空路径 → load_or_fallback
/// 原样语义（坏路径返回 Err，仅内置损坏时方为不可恢复）。
///
/// 成功即替换单例（缓冲 / 页码 / ⇧ 一并重置）。已有引擎会先导出用户词再导回，
/// 避免 Settings 与 IME 同进程再次 load 时把 Learner 冲掉。坏路径返回 Err，
/// 已装引擎保持不动。
pub fn install(path: Option<&str>) -> Result<(), String> {
    let dict: Box<dyn Dictionary> = match path {
        Some(p) if !p.is_empty() => engine_data::load_or_fallback(Some(std::path::Path::new(p)))?,
        _ => Box::new(engine_data::fallback_dict()),
    };
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    // 毒化恢复：某个 FFI 出口（`jni.rs` / `cabi.rs` 两面各自的 catch_unwind 包装）
    // 吞掉 panic 时锁已毒化，into_inner 取回数据；install 整体替换引擎，提供恢复路径。
    let mut guard = SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    let saved = guard.as_ref().map(|api| api.export_user_words());
    *guard = Some(Api {
        router: KeyRouter::new(Engine::new(dict, symbols, true)),
    });
    if let Some(json) = saved
        && let Some(api) = guard.as_mut()
    {
        let _ = api.import_user_words(json);
    }
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

/// 引擎句柄。同步核心；Rust 测试测同步核心。
///
/// 持 [`KeyRouter`] 而非裸 `Engine`：键路由的**状态**（页码、⇧ 三态、可打印键
/// 抬起结论）是引擎会话状态，与 composer 的 buffer 同类 —— 放在这里才能
/// ①与引擎同生命周期（`install` 换库时一并重置）、②不必再开第二把锁、
/// ③`switch_mode` 能顺手清前端 ⇧（否则 Apple 侧 ⇧ Lock 会跨模式残留）。
/// 引擎本体经 `router.engine()/engine_mut()` 取用，对两个 ABI 面透明。
pub struct Api {
    router: KeyRouter,
}

impl Api {
    pub fn load_fallback_sync() -> Result<Api, String> {
        let dict = engine_data::fallback_dict();
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        Ok(Api {
            router: KeyRouter::new(Engine::new(Box::new(dict), symbols, true)),
        })
    }

    pub fn load_sync(path: String) -> Result<Api, String> {
        let dict = engine_data::load_or_fallback(Some(std::path::Path::new(&path)))?;
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        Ok(Api {
            router: KeyRouter::new(Engine::new(dict, symbols, true)),
        })
    }

    /// raw 出口（绕过键路由）。改完 buffer 必须对齐页码 —— 否则「翻页 → 清空 →
    /// 重打 → 按 1」会用过期页码选错候选（见 `KeyRouter::engine_mut`）。
    pub fn input_key(&mut self, ch: String) -> String {
        let mut chars = ch.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return String::new(); // 边界：拒绝空串/多字符；非 ASCII 拒绝在引擎层（引擎只收 ASCII 键）
        };
        let out = self.router.engine_mut().input_key(c);
        self.router.reset_page_if_buffer_changed();
        out
    }

    /// raw 出口。空格在有缓冲时提交并清空 buffer → 必须对齐页码（同 `input_key`）。
    pub fn input_space(&mut self) -> String {
        let out = self.router.engine_mut().input_space();
        self.router.reset_page_if_buffer_changed();
        out
    }

    pub fn backspace(&mut self) {
        self.router.engine_mut().backspace();
        self.router.reset_page_if_buffer_changed();
    }

    /// raw 出口。清空 buffer → 必须对齐页码：否则 `opi_page()` 停在旧值而
    /// `opi_page_count()` 已是 0，UI 显示「第 2 页 / 共 0 页」。
    pub fn clear(&mut self) {
        self.router.engine_mut().clear();
        self.router.reset_page_if_buffer_changed();
    }

    /// 切模式走**路由**而非直接打引擎：切换须清前端 ⇧ 三态
    /// （见 `KeyRouter::switch_mode` 与两轨的 switch_mode_clears_frontend_shift_lock）。
    pub fn switch_mode(&mut self, mode: ApiMode) {
        self.router.switch_mode(mode.into());
    }

    /// 键路由（C ABI `opi_key_event` 的实现体）：平台中立键码 + 修饰位 → 动作。
    pub fn key_event(&mut self, keyval: u32, states: u32) -> KeyAction {
        self.router.key_event(keyval, states)
    }

    /// 换装繁体词典（trad.opid）。None = 清除（繁体模式回退简体库）。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        self.router.engine_mut().set_trad_dict(dict);
    }

    /// 引擎侧 ⇧（`opi_set_shift`，Android UI 用）。走路由：换 shift 会改候选集，
    /// 路由顺带把页码钳回边界（两轨的 `set_shift` 同样钳）。
    pub fn set_shift(&mut self, on: bool) {
        self.router.set_shift(on);
    }

    pub fn buffer(&self) -> String {
        self.router.engine().buffer().to_string()
    }

    pub fn mode(&self) -> ApiMode {
        self.router.engine().mode().into()
    }

    /// 当前候选页（0 起）。候选栏用它画页码 —— 自己维护会在末页（PageDown 被钳制）
    /// 与引擎漂移：高亮的页 ≠ 实际选词所在的页。
    pub fn page(&self) -> u32 {
        self.router.page() as u32
    }

    /// 候选总页数（UI 的「共 N 页」；无候选 → 0）。
    pub fn page_count(&self) -> u32 {
        self.router.page_count() as u32
    }

    /// **当前页**候选文本（已分页）。前端不要拿 `candidates(limit)` 自己按 8 切 ——
    /// 页大小是引擎侧常量，抄一份到 UI 就会在改动时静默错位。
    pub fn candidate_texts_page(&self) -> Vec<String> {
        self.router.candidates()
    }

    /// 前端 ⇧ 三态 → C ABI 整数 0=OFF 1=SINGLE 2=LOCK（薄壳的转换职责，同 `mode_to_int`）。
    /// 与 `set_shift` 不是一回事：这是 `ShiftState`（决定英文直传的大小写与 UI 高亮），
    /// 那个打的是引擎侧 shift 位。
    pub fn shift_state_int(&self) -> i32 {
        match self.router.shift_state() {
            ShiftState::Off => 0,
            ShiftState::Single => 1,
            ShiftState::Lock => 2,
        }
    }

    /// 全角开关读侧（`opi_fullwidth_state`）。**每次 `switch_mode` 之后必须重读**：
    /// 切模式把它重置为该模式的默认值（`Mode::default_fullwidth`：拼音/繁体全角、
    /// 英文/数字/符号半角），平台侧自己记一份必漂。
    pub fn fullwidth(&self) -> bool {
        self.router.engine().fullwidth()
    }

    /// 全角 ↔ 半角，返回**切换后的状态**（调用方直接拿去刷状态栏）。
    /// 键位不在本层（与 `toggle_symbol` 同一条：四端键位未定，引擎只出语义）。
    /// 不改 buffer、不改候选集 ⇒ 不需要对齐页码。
    pub fn toggle_fullwidth(&mut self) -> bool {
        self.router.engine_mut().toggle_fullwidth()
    }

    /// 中文标点表读侧（`opi_chinese_punct` / JNI `chinesePunct`）。
    ///
    /// **与全角是两档开关，互不牵连**：本档管中文标点表（`.。`、引号交替），
    /// `fullwidth` 管机械全角兜底。**切模式不重置本档** ——
    /// 它是用户偏好而非模式默认值，所以「每次 `switch_mode` 后必须重读」那条
    /// 硬规则**只对 `fullwidth` 成立**，别照抄过来。
    /// 仍然必须有读侧：设置项会被设置页/下次启动的持久化改动，平台侧自记一份必漂。
    /// 不改 buffer、不改候选集 ⇒ 不需要对齐页码。
    pub fn chinese_punct(&self) -> bool {
        self.router.engine().chinese_punct()
    }

    /// 设置中文标点表是否生效。**写入口之一**（设置项；UI 是勾选框，知道要设成什么值）——
    /// 另一个是 [`Self::toggle_chinese_punct`]（触发键）。与引擎层同一条，
    /// 见 `Engine::set_chinese_punct`。调用后不必重读 buffer/candidates（只翻一个 bool）。
    pub fn set_chinese_punct(&mut self, on: bool) {
        self.router.engine_mut().set_chinese_punct(on);
    }

    /// 中文标点表开关的**触发键入口**：翻转并返回**切换后的新状态**（调用方直接拿去
    /// 刷状态栏/勾选框）。键位不在本层（与 `toggle_fullwidth` / `toggle_symbol` 同一条：
    /// 键位在客户端侧、由客户端在引擎之前截获）。
    /// ⚠️ 与 `toggle_fullwidth` 的**重读规则不通用**：本档是用户偏好，`switch_mode`
    /// 不重置它。不改 buffer、不改候选集 ⇒ 不需要对齐页码。
    pub fn toggle_chinese_punct(&mut self) -> bool {
        self.router.engine_mut().toggle_chinese_punct()
    }

    /// 符号面板开关，返回**待插入文档的文本**（非空 = 调用方必须插入；拿不到插入
    /// 通道的端不要调它）。语义在引擎层（含「先收尾未提交缓冲」），键位不在本层。
    /// raw 出口：收尾会改 buffer ⇒ 必须对齐页码（同 `input_space`）。
    pub fn toggle_symbol(&mut self) -> String {
        let out = self.router.engine_mut().toggle_symbol();
        self.router.reset_page_if_buffer_changed();
        out
    }

    pub fn candidates(&self, limit: usize) -> Vec<ApiCandidate> {
        self.router
            .engine()
            .candidates(limit)
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// raw 出口（**全局**索引，JNI 与既有 `opi_select` 用）。选中即清空 buffer →
    /// 必须对齐页码；越界返回空串、buffer 不变，此时对齐也是 no-op。
    /// 页内索引请用 [`Api::select_page`]（`opi_select_page`），UI 不需要知道 PAGE_SIZE。
    pub fn select(&mut self, index: usize) -> String {
        let out = self.router.engine_mut().select(index);
        self.router.reset_page_if_buffer_changed();
        out
    }

    /// **页内**索引选词：与数字选词（`KeyRouter::digit_select`）、回车提交同源
    /// （都走 `KeyRouter::select` 那一份 `page * PAGE_SIZE + index` 换算），
    /// 所以这是最后一份 PAGE_SIZE 的载体 —— UI 不必自己算全局下标。
    pub fn select_page(&mut self, index: usize) -> String {
        self.router.select(index)
    }

    pub fn set_learner(&mut self, enabled: bool) {
        self.router.engine_mut().set_learner(enabled);
    }

    pub fn learner_enabled(&self) -> bool {
        self.router.engine().learner_enabled()
    }

    pub fn remove_user_word(&mut self, text: String) {
        self.router.engine_mut().remove_user_word(&text);
    }

    pub fn clear_user_words(&mut self) {
        self.router.engine_mut().clear_user_words();
    }

    /// 导入 [`Api::export_user_words`] 的产物（Android 启动时读文件后传入）。
    /// 返回导入条数；非法 JSON / 版本不符 → Err 且不改动既有状态。
    pub fn import_user_words(&mut self, json: String) -> Result<usize, String> {
        self.router.engine_mut().import_user_words(&json)
    }

    pub fn export_user_words(&self) -> String {
        self.router.engine().export_user_words()
    }

    pub fn symbol_blocks(&self) -> Vec<ApiBlock> {
        self.router
            .engine()
            .symbol_blocks()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn symbols_in_block(&self, id: u16) -> Vec<ApiSymbolEntry> {
        self.router
            .engine()
            .symbols_in_block(BlockId(id))
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn search_symbols(&self, keyword: String) -> Vec<ApiSymbolEntry> {
        self.router
            .engine()
            .search_symbols(&keyword)
            .into_iter()
            .map(Into::into)
            .collect()
    }
}
