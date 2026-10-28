// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `TsfLogic` 的构造（含可选繁体词库）。
//!
//! 拆出理由：`logic.rs` 顶到 500 行门禁；本模块是 `logic` 的子模块，可访问私有字段。
//! 双词典路由在 `engine-core`（`Engine::with_dictionaries`），本层只负责把 trad 装进去。

use std::cell::RefCell;

use engine_core::Engine;

use super::{ShiftState, TsfLogic};

impl TsfLogic {
    /// 用**已装好的**词库构建。`trad = None` 时繁体模式回退主库
    /// （`Engine::with_dictionaries` 的语义）；最差内置 35 词。
    pub fn with_dicts(
        dict: Box<dyn engine_core::dictionary::Dictionary>,
        trad: Option<Box<dyn engine_core::dictionary::Dictionary>>,
    ) -> Self {
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        let mut s = TsfLogic {
            engine: Engine::with_dictionaries(dict, trad, symbols, true),
            page: 0,
            buffer_snapshot: String::new(),
            shift_state: ShiftState::Off,
            english_pro: false,
            held_passthrough: Default::default(),
            shift_toggle: true,
            association_enabled: true,
            fetch_cache: RefCell::new(None),
        };
        s.refresh_snapshot();
        s
    }
}
