// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **分词回退**的引擎侧收尾（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」；
//! 独立成文件是 500 行门禁 —— `engine.rs` 恰 500 行，见 `engine_symbol_browse.rs`
//! 同一套先例）。
//!
//! 回退本身分两半，本文件是第二半：
//! 1. **候选出哪一段**：`candidates::segment_fallback`（26 键/笔画的唯一判定）与
//!    `scheme::scheme_fallback_len`（9/14 键的数字空间版，2026-10-10 扩）——各自被
//!    对应候选层的回退档调用（整串无命中 ⇒ 候选换成最长可匹配前缀的整档）；
//! 2. **选完缓冲剩什么**：[`Engine::commit_buffer_after_select`] —— 只消耗那个前缀，
//!    剩下的后缀留在缓冲里**接着编**（`nihaob` 选「你好」→ 缓冲变 `b`；
//!    `644269` 选「你好」→ 缓冲变 `9`）。
//!
//! 为什么不另存「当前回退态」状态：`segment_fallback` 是纯函数（同一缓冲必同结论），
//! 收尾层重算一遍即可，于是**不依赖「先取候选、再选中」的调用次序** ——
//! 与 `scheme_readings` 那条 `(算它时的缓冲, 值)` 绑存同一条纪律（读取不许撒谎）。
//! 代价是每次选中多做几个词库前缀查询（`query_all`），对一次用户动作可忽略。
//!
//! 字段可见性：本文件要用 `Engine::composer` / `Engine::fuzzy` 与 `Engine::active_dict`，
//! 故它们升成 `pub(crate)`（**crate 内**，不泄漏给依赖方）—— 同 `engine_readings.rs`
//! 头注记的拆分裂代价。

use crate::candidates::segment_fallback;
use crate::composer::Mode;
use crate::engine::Engine;
use crate::scheme::{Scheme, scheme_fallback_len};

impl Engine {
    /// 候选选中后的缓冲收尾：**回退态只消耗前缀**，后缀留在缓冲里接着编；
    /// 其余情况照旧整段清空（[`crate::composer::Composer::commit_buffer`]）。
    ///
    /// 调用点在 `Engine::select_from` 的命中分支（**唯一**的收尾处，四端自动一致）。
    /// 三条边界与候选层逐字同源：方案只在全键盘/笔画（分词回退）与 9/14 键
    /// （数字空间回退）上做、模式只在拼音/繁体上做、单字符与「无回退落点」一律走
    /// 整段清空。9/14 键的钉读音（`pin`）按**当前生效值**传进判定 —— 候选层用的是
    /// 同一个，两侧必同结论（见 `scheme_fallback_len` 的判据 3）。笔画方案带笔画词典
    /// 时 `segment_fallback` 恒 `None`（笔画串的段不可能是完整音节），故这里用
    /// `active_dict()` 与候选层的键差无关紧要。
    pub(crate) fn commit_buffer_after_select(&mut self) {
        let fallback = if !matches!(self.mode(), Mode::Pinyin | Mode::Traditional) {
            None
        } else {
            // 缓冲用 composer 的原文（不是 `buffer()` —— 浏览态那个会返回标题，
            // 虽然浏览态的选中根本走不到这里，别把「走不到」当默认）。
            let buffer = &self.composer.session().buffer;
            match self.scheme() {
                Scheme::Full | Scheme::Stroke => {
                    segment_fallback(self.active_dict(), buffer, self.fuzzy)
                        .map(|fb| fb.suffix)
                        .filter(|suffix| !suffix.is_empty())
                }
                // 9/14 键：数字空间回退 —— 只消耗前缀的**键**，尾部键留缓冲接着编。
                // 字节切片安全：缓冲只收 ASCII（composer::input_key 的门）。
                Scheme::Nine | Scheme::Fourteen => scheme_fallback_len(
                    self.active_dict(),
                    buffer,
                    self.scheme(),
                    self.active_pin(),
                )
                .map(|k| buffer[k..].to_string())
                .filter(|suffix| !suffix.is_empty()),
            }
        };
        match fallback {
            Some(suffix) => {
                self.composer.commit_keep_suffix(&suffix);
            }
            None => {
                self.composer.commit_buffer();
            }
        }
    }
}
