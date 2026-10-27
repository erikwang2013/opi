// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 词典换装一族（2026-10-14 从 `engine.rs` 拆出 —— 加导入词库层后那里越了 500 行
//! 门禁；先例：engine_punct.rs / engine_readings.rs / engine_segment.rs /
//! engine_symbol_browse.rs / engine_user_words.rs）。
//!
//! 拆的是**一族自洽的方法**：三个换装口（trad / stroke / M3 的导入层）、当前模式
//! 生效词典的路由（[`Engine::active_dict`]），以及 `user_boost` 的唯一重算处 ——
//! 三者同一条不变式：**换了任何一份词频表，boost 都要跟着重算**。语义一字未动。
//!
//! 字段可见性：本文件要用 `Engine` 的三个词典字段（`dict` / `trad_dict` /
//! `stroke_dict`），故它们升成 `pub(crate)`（**crate 内**，不泄漏给依赖方）。
//! 这是拆分的代价，别再为此往 engine.rs 塞回方法。

use crate::candidates::USER_BOOST;
use crate::composer::Mode;
use crate::dictionary::{Dictionary, InMemoryDictionary};
use crate::engine::Engine;

impl Engine {
    /// `user_boost` 的**唯一**算法：`max(USER_BOOST, 全库最大静态词频 × 2)`。
    ///
    /// 取四个词典的**全局**最大词频（含 stroke，2026-10-09 起；导入层，2026-10-14 起
    /// —— `self.dict` 是分层库，它的 `max_freq` 已经把导入层算进去，见 `dict_layers.rs`
    /// 裁决 4；含 wubi，2026-10-14 M4 3b 起）。调用点：构造 + 四个换装口，**每个都要调**：
    /// 漏一个就会让某个更大的词频表静默改变「一次选词压过全部静态词」的档位
    /// （那是既有不变量）。
    pub(crate) fn recompute_user_boost(&mut self) {
        let max_freq = std::iter::once(&self.dict as &dyn Dictionary)
            .chain(self.trad_dict.as_deref())
            .chain(self.stroke_dict.as_deref())
            .chain(self.wubi_dict.as_deref())
            .map(|d| d.max_freq())
            .max()
            .unwrap_or(0);
        self.user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
    }

    /// 换装繁体词典（FFI install_trad 用）。None = 清除（回退简体）。
    /// 重算 user_boost：trad.opid 静态最大词频 4e9，保持"一次选词压过全部静态词"。
    /// ⚠️ 取三个词典的**全局**最大词频（含 stroke，2026-10-09 起）：只按「dict+trad」
    /// 算的话，后装的 stroke 若词频更高，boost 会悄悄偏小 —— 学习者压不过静态词。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        self.dismiss_association(); // 换词典 = 联想态结束（候选集合会变）
        self.trad_dict = dict;
        self.recompute_user_boost();
    }

    /// 当前模式生效的词典（`pub(crate)`：engine_segment.rs 的回退判定要用同一个词典）。
    /// 导入层只叠主库（`dict_layers.rs` 裁决 3）：繁体模式走 trad，trad 缺失时回退主库
    /// ——那时导入层自然可见。
    pub(crate) fn active_dict(&self) -> &dyn Dictionary {
        match self.composer.session().mode {
            Mode::Traditional => self.trad_dict.as_deref().unwrap_or(&self.dict),
            _ => &self.dict,
        }
    }

    /// 换装笔画词典（FFI install_stroke 用）。None = 清除（笔画方案回退普通路径）。
    /// 与 [`Self::set_trad_dict`] 同一形状（含重算 user_boost —— 别让更大的词频表
    /// 静默改变「精确加成」的档位）。
    pub fn set_stroke_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        self.dismiss_association(); // 换词典 = 联想态结束（候选集合会变）
        self.stroke_dict = dict;
        self.recompute_user_boost();
    }

    /// 换装五笔码表（M4 3b，2026-10-14）。None = 清除（五笔方案回退普通路径）。
    /// 与 [`Self::set_stroke_dict`] 同一形状（含重算 user_boost —— 别让更大的词频表
    /// 静默改变「精确加成」的档位）。换装**不清缓冲**（与两个 style 设置同：只改
    /// 之后怎么装配，缓冲里的码按新表重解）。
    pub fn set_wubi_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        self.dismiss_association(); // 换词典 = 联想态结束（候选集合会变）
        self.wubi_dict = dict;
        self.recompute_user_boost();
    }

    /// 换装**导入词库层**（M3 计划 Task 2b，2026-10-14）：**全集替换** ——
    /// 启用/停用/删除都表达成「新集合里有/没有它」，不追加、不嵌套（同一层集合
    /// 再传一次 = 幂等替换）。空集合 = 回到纯系统库。
    ///
    /// **必须重算 user_boost**（见 [`Self::recompute_user_boost`]）：导入一个词频高于
    /// `USER_BOOST/2` 的词会把 boost 抬到它的两倍 —— 不重算，学习过的词就再也压不过
    /// 这个静态词，「一次选词压过全部静态词」当场失效。门禁：
    /// tests/dict_layers.rs::import_high_freq_word_recomputes_user_boost（变异自检里
    /// 去掉本行调用，这条门禁必须红）。
    ///
    /// 合并在词条层完成（`dict_layers.rs`），**不另造排序**：候选仍是同一条
    /// `rank_and_pick` 管线；导入层只叠主库（繁体模式仍查 trad 库，裁决 3）。
    pub fn set_extra_dicts(&mut self, dicts: Vec<(String, InMemoryDictionary)>) {
        self.dismiss_association(); // 换导入层 = 联想态结束（候选集合会变）
        self.dict.set_extras(dicts);
        self.recompute_user_boost();
    }

    /// 当前导入层：名字 + 条数（列表页数据；顺序 = 传入顺序）。条数按层相加，
    /// 跨层同键算两次（只用于展示与「空不空」，不是精确去重计数，见 `dict_layers.rs`）。
    pub fn extra_dicts(&self) -> Vec<(String, usize)> {
        self.dict.extras_list()
    }

    /// 改导入层扫描预算（2026-10-17 裁决「超时就优先使用自带词库」的口子；2026-10-22 总账 30ms）：
    /// `None` = 不设限，缺省 = `dict_layers::IMPORT_SCAN_BUDGET`（20ms，代数见那里）。
    /// **不重算 boost** —— 预算与词频表无关。
    pub fn set_import_scan_budget(&mut self, budget: Option<std::time::Duration>) {
        self.dict.set_import_scan_budget(budget);
    }
}
