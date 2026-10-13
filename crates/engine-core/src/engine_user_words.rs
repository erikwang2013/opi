// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 用户词与开关一族（2026-10-11 从 `engine.rs` 拆出 —— 9 键无候选修复在那里又加了
//! 8 行、越了 500 行门禁；先例：engine_punct.rs / engine_readings.rs /
//! engine_segment.rs / engine_symbol_browse.rs）。
//!
//! 拆的是三类只有一行转发的口子：学习开关（[`Engine::set_learner`]）、排序开关
//! （fuzzy / jianpin 各自一对读写）、用户词管理（删 / 清 / 导出 / 导入）。语义
//! 一字未动；落盘仍是调用方的事（engine 不做 IO 是本 crate 的不变量，Android 侧
//! 启动时读文件后经 [`Engine::import_user_words`] 传入）。
//!
//! 字段可见性：`Engine::jianpin` 升 `pub(crate)`（本文件要读写它；**不泄漏给依赖方**）
//! —— 同 engine_readings.rs / engine_punct.rs 头记的拆分裂代价。

use crate::engine::Engine;

impl Engine {
    pub fn set_learner(&mut self, enabled: bool) {
        self.learner.set_enabled(enabled);
    }

    /// 模糊音开关（「属性设置」的旋钮之一）。关掉 = `rank_and_pick_with` 跳过
    /// 整串变体与逐音节兜底**两个**入口（只关一个会让 `zongguo` 这类输入仍从
    /// 逐音节拿到模糊结果）。
    /// 全开/全关的**包装**（2026-10-13 起真源是 [`Self::set_fuzzy_pairs`]；旧入口纪律：
    /// 这是新入口的默认包装，四端配置不写 FuzzyPairs 键时走的还是这条）。
    pub fn set_fuzzy(&mut self, enabled: bool) {
        self.fuzzy = if enabled { crate::fuzzy::FUZZY_ALL } else { 0 };
    }

    /// 模糊音**逐对**设置（位掩码，位序 = `fuzzy.rs::FUZZY_PAIRS`）。
    /// 越界位（> 8）不去动 —— 未来加组时旧宿主传的掩码不该把新组误开/误关。
    pub fn set_fuzzy_pairs(&mut self, mask: crate::fuzzy::FuzzyMask) {
        self.fuzzy = mask & crate::fuzzy::FUZZY_ALL;
    }

    /// 逐对读侧（写侧见 [`Self::set_fuzzy_pairs`]）。
    pub fn fuzzy_pairs(&self) -> crate::fuzzy::FuzzyMask {
        self.fuzzy
    }

    /// 旧读侧（布尔）：**任一组开着就是 true**（旧 UI 的复选框语义）。
    pub fn fuzzy_enabled(&self) -> bool {
        self.fuzzy != 0
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
    /// engine 不做 IO）。合并语义、幂等性与失败原子性见
    /// [`crate::learner::Learner::import_json`]。
    /// 返回导入条数；非法输入返回 Err 且不改动内存状态。
    pub fn import_user_words(&mut self, json: &str) -> Result<usize, String> {
        self.learner.import_json(json)
    }
}
