// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **引擎设置位一族**（2026-10-13/14 陆续加满，2026-10-14 收拢）：学习/中文标点/
//! 模糊音（含逐对）/简拼/纠错/双拼布局/方案 + **五笔一族**（装载 + 五个设置位）。
//! 从 `candidate.rs` 拆出
//! （500 行硬门禁：加完这批那里 549/500）。
//!
//! ⚠️ 拆出去**不等于脱离同构纪律**：两轨的这一族**逐字相同**（只有类型名与日志前缀不同），
//! 改了这边必须改那边（`tsf-opi/src/logic_wubi.rs` ↔ `fcitx5-opi/src/candidate_wubi.rs`）。
//! 与主文件的分工：主文件管键路由/候选状态，这里只管「配置值 → 引擎设置位」。

use crate::CandidateState;

impl CandidateState {
    /// 装载五笔码表（2026-10-14，M4 第二波；宿主按 `WubiPlan` 选 `wubi86.opid` /
    /// `wubi98.opid` 后把**绝对路径**传进来）。与 stroke 同一条严格装载：坏路径
    /// **不**回落内置（内置是拼音 35 词，装成码表语义错误）⇒ 返回 false，方案回退
    /// 普通路径。**装载不换方案**（要用五笔还得 `set_scheme(Wubi)`）。
    /// 配对：``tsf-opi/src/logic.rs`` 的同名方法（两轨同构）。
    pub fn load_wubi(&mut self, path: &str) -> bool {
        match engine_data::load_mmap(std::path::Path::new(path)) {
            Ok(d) => {
                self.engine.set_wubi_dict(Some(Box::new(d)));
                true
            }
            Err(e) => {
                eprintln!("fcitx5-opi: 五笔码表装载失败，方案回退普通路径: {e:?}");
                false
            }
        }
    }

    /// 五笔设置位（2026-10-14，M4 第二波）：风格 / 混输进入方式 / 候选个数 / 回车行为 /
    /// 自动上屏，五个都**不清缓冲**（只改「同一缓冲怎么装配候选」，与 `set_scheme` 相反）。
    /// 收**枚举**：整数码 ↔ 枚举的换算在 engine-core 的 `code()/from_code()` **一处**
    /// 完成（四端唯一真源，本层不写第二张表）。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构，改一处必须改另一处）。
    pub fn set_wubi_style(&mut self, style: engine_core::WubiStyle) {
        self.engine.set_wubi_style(style);
    }

    pub fn set_wubi_mixed_entry(&mut self, entry: engine_core::WubiMixedEntry) {
        self.engine.set_wubi_mixed_entry(entry);
    }

    pub fn set_wubi_candidate_limit(&mut self, n: usize) {
        self.engine.set_wubi_candidate_limit(n);
    }

    pub fn set_wubi_return_style(&mut self, style: engine_core::WubiReturnStyle) {
        self.engine.set_wubi_return_style(style);
    }

    pub fn set_wubi_auto_commit(&mut self, on: bool) {
        self.engine.set_wubi_auto_commit(on);
    }

    /// 学习开关（值来自 addon 配置的 `Learn` → `opi_fcitx5_set_learner`）。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构，改一处必须改另一处）。
    pub fn set_learner(&mut self, on: bool) {
        self.engine.set_learner(on);
    }

    /// 中文标点开关（值来自 addon 配置的 `ChinesePunct`）。配对同上。
    pub fn set_chinese_punct(&mut self, on: bool) {
        self.engine.set_chinese_punct(on);
    }

    /// 模糊音开关（值来自 fcitx5 的 addon 配置 → `opi_fcitx5_set_fuzzy`）。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构，改一处必须改另一处）。
    pub fn set_fuzzy(&mut self, on: bool) {
        self.engine.set_fuzzy(on);
    }

    /// 模糊音**逐对**（2026-10-13；配置键 `FuzzyPairs`，组名串见
    /// `engine_core::fuzzy::pair_names`，解析走 `fuzzy::mask_from_names` —— 唯一一份）。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构）。
    pub fn set_fuzzy_pairs(&mut self, names: &str) {
        self.engine
            .set_fuzzy_pairs(engine_core::fuzzy::mask_from_names(names));
    }

    /// 简拼开关。配对同上。
    pub fn set_jianpin(&mut self, on: bool) {
        self.engine.set_jianpin(on);
    }

    /// 拼音纠错开关（2026-10-13；配置键 `Correction`）。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构）。
    pub fn set_correction(&mut self, on: bool) {
        self.engine.set_correction(on);
    }

    /// 双拼布局（2026-10-13；配置键 `ShuangpinLayout`）。收**下标**（0=自然码…5=拼音加加，
    /// 表序见 `shuangpin_tables.rs`）—— C++ 侧把它的枚举值转成下标传进来；越界钳制在
    /// 引擎侧（`set_shuangpin_layout`）。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构）。
    pub fn set_shuangpin_layout(&mut self, index: usize) {
        self.engine.set_shuangpin_layout(index);
    }

    /// 输入方案（2026-10-14；配置键 `PinyinMethod`，C++ 侧传枚举值 = 方案整数码）。
    /// **清缓冲**（引擎侧语义，见 `Engine::set_scheme`）—— 冷路径（配置变更）调用，
    /// 没有「按键路径上换方案」这种用法。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构）。
    pub fn set_scheme(&mut self, scheme: engine_core::Scheme) {
        self.engine.set_scheme(scheme);
    }

    /// 导入一条词库（2026-10-14，M3；路径 = UTF-8 tsv，宿主扫目录或设置页给）。
    /// 返回条目数；失败 / 无有效条目 → **-1**（既有导入层不动）。冷路径，逐条语义与
    /// 三条裁决见 `dict_layer.rs` 头注（与 opi-ffi `api/dict.rs` 同源）。
    /// 配对：`tsf-opi/src/logic.rs` 的同名方法（两轨同构，改一处必须改另一处）。
    pub fn import_dict(&mut self, path: &str) -> i32 {
        self.dicts.import(&mut self.engine, path)
    }

    /// 装载 / 卸载一条**导入**词库（原子；`false` = 名字认不得，含系统词典与未导入
    /// 的名字）。停用后再启用**不依赖原文件还在**（数据在本层内存里）。配对同上。
    pub fn set_dict_enabled(&mut self, name: &str, on: bool) -> bool {
        self.dicts.set_enabled(&mut self.engine, name, on)
    }
}
