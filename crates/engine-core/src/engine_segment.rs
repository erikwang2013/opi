// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **分词回退**的引擎侧收尾（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」；
//! 独立成文件是 500 行门禁 —— `engine.rs` 恰 500 行，见 `engine_symbol_browse.rs`
//! 同一套先例）。
//!
//! 回退本身分两半，本文件是第二半：
//! 1. **候选出哪一段**：`candidates::segment_fallback`（26 键/笔画的唯一判定）与
//!    `scheme::scheme_fallback_len`（9/14 键的数字空间版，2026-10-10 扩）——各自被
//!    对应候选层的回退档调用（整串无精确命中 ⇒ 候选换成第一个音节的整档）；
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
    /// 候选选中后的缓冲收尾（2026-10-17 起带**逐字确认/延迟提交**）：
    /// **回退态只消耗前缀**，后缀留在缓冲里接着编 —— 且这一轮的选中文本**不提交**
    /// （返回 `None`，存进 `pending_segments` 留在候选区显示）；
    /// 其余情况整段清空并返回**待提交的整串**（已确认片段 + 本轮文本）。
    ///
    /// 用户裁决（2026-10-17）：「输入一个词组，匹配到的在最前面；到没有匹配，就要先
    /// 匹配前面的第一个字……确认一个字，依然在候选栏；然后依次匹配后面的字，直到最后
    /// 的字匹配确认后，才是最后确认到输入框。可参考同类输入法。」
    /// 门禁：`tests/segment_commit.rs`。
    ///
    /// 调用点在 `Engine::select_from` 的命中分支（**唯一**的收尾处，四端自动一致）。
    /// 三条边界与候选层逐字同源：方案只在全键盘/笔画（分词回退）与 9/14 键
    /// （数字空间回退）上做、模式只在拼音/繁体上做、单字符与「无回退落点」一律走
    /// 整段清空。9/14 键的钉读音（`pin`）按**当前生效值**传进判定 —— 候选层用的是
    /// 同一个，两侧必同结论（见 `scheme_fallback_len` 的判据 3）。笔画方案带笔画词典
    /// 时 `segment_fallback` 恒 `None`（笔画串的段不可能是完整音节），故这里用
    /// `active_dict()` 与候选层的键差无关紧要。
    pub(crate) fn commit_buffer_after_select(&mut self, selected: &str) -> Option<String> {
        let fallback = if self.custom_phrase_selected(selected) {
            // 自定义短语（2026-10-23）：文本不是缓冲的拼音投影，分词回退会把 `zhangsan`
            // 这类 trigger 切成「首音节 + 尾巴」⇒ 短语被拆开、留在逐字态。整段消耗。
            None
        } else if !matches!(self.mode(), Mode::Pinyin | Mode::Traditional) {
            None
        } else {
            // 缓冲用 composer 的原文（不是 `buffer()` —— 浏览态那个会返回标题，
            // 虽然浏览态的选中根本走不到这里，别把「走不到」当默认）。
            // **克隆**：9/14 键那一臂要临时换缓冲做探针（`&mut self`），不能同时借着它。
            let buffer = self.composer.session().buffer.clone();
            match self.scheme() {
                // 双拼不支持「切一半」：解码只在**整串**能解析时才出候选（DFS 要求
                // 到达键尾），选中即整段消耗 —— 回退臂给 None（走 commit_buffer）。
                // 五笔同（2026-10-14，M4 3b）：候选层的分词回退对它已关闸
                // （`RankOptions::segment_fallback`，理由见 engine_wubi.rs 头），
                // 收尾层若还按音节去切就是两层判据不同源 —— 整段消耗，与候选层一致。
                Scheme::Shuangpin | Scheme::Wubi => None,
                Scheme::Full | Scheme::Stroke => {
                    let suffix =
                        segment_fallback(self.active_dict(), &buffer, self.fuzzy, self.correction)
                            .map(|fb| fb.suffix)
                            .filter(|suffix| !suffix.is_empty());
                    // 多字组合（2026-10-23）：选中的组合**覆盖整个输入**，不是「先打
                    // 第一个字」—— 整段消耗，否则缓冲里会剩下半截拼音继续编。
                    // 单字选中的常见路径不进组合枚举（省一次全串 DP）。
                    match suffix {
                        Some(_)
                            if matches!(self.scheme(), Scheme::Full)
                                && selected.chars().count() > 1
                                && crate::composition::covers(
                                    self.active_dict(),
                                    &self.learner,
                                    self.user_boost,
                                    &buffer,
                                    selected,
                                ) =>
                        {
                            None
                        }
                        other => other,
                    }
                }
                // 9/14 键：数字空间回退 —— 只消耗前缀的**键**，尾部键留缓冲接着编
                // （尾巴长且整段读不出来时不留，见 [`Self::digit_tail_to_keep`]）。
                // 字节切片安全：缓冲只收 ASCII（composer::input_key 的门）。
                Scheme::Nine | Scheme::Fourteen => {
                    // 多字组合（2026-10-23）：选中的组合覆盖**整个**输入 ⇒ 整段消耗，
                    // 不按「第一个字」的回退留后缀键（同全键盘那条 `composition::covers`）。
                    let composed = matches!(self.scheme(), Scheme::Nine)
                        && self.active_pin().is_none()
                        && selected.chars().count() > 1
                        && !buffer.contains('\'')
                        && crate::composition_scheme::covers(
                            self.active_dict(),
                            &self.learner,
                            self.user_boost,
                            Scheme::Nine,
                            &buffer,
                            selected,
                        );
                    if composed {
                        None
                    } else {
                        self.digit_tail_to_keep(&buffer)
                    }
                }
            }
        };
        match fallback {
            Some(suffix) => {
                // 逐字态（2026-10-17）：被消耗的前缀（拼音）与确认文本一起入栈，
                // 缓冲只剩后缀 —— **本轮不提交**（None），显示由 display_buffer 带上。
                let buffer = self.composer.session().buffer.clone();
                let consumed = buffer[..buffer.len() - suffix.len()].to_string();
                self.pending_segments.push((consumed, selected.to_string()));
                self.composer.commit_keep_suffix(&suffix);
                None
            }
            None => {
                self.composer.commit_buffer();
                // 收尾：已确认片段 + 本轮文本 = 一次性进输入框的整串。
                let mut out: String = self.pending_segments.drain(..).map(|(_, t)| t).collect();
                out.push_str(selected);
                Some(out)
            }
        }
    }

    /// 9/14 键回退的尾巴**该不该留**（用户裁决 2026-10-15，折中口径）。
    ///
    /// - **1–2 键：照旧留**（不探）。那是「下一音节刚起头」的主场景 —— `644269` 选完
    ///   留 `9` 接着打下一个音，真词库里 `bugaoml` 的尾巴 `ml` 也靠这条活
    ///   （`crates/opi-tools/tests/m2_integration.rs` 钉着）。两键以内**不押在词库覆盖上**。
    /// - **≥3 键：自己出得来候选才留**，否则整段清空。
    ///
    /// ⚠️ 报障（用户原话「确认第一个候选词后，再次输入其他拼音无反应，候选词显示无匹」）：
    /// 9 键连按同一个键能按出 `6444444` 这类长串 → 回退只吃前缀 `64` → 尾巴 `44444`
    /// 是一段**整读不出来的死串**，而它在 9 键上**看不见**（拼音行按读数显示，读数空就
    /// 留空 —— 2026-10-11 裁决「不落数字」仍在）⇒ 之后每一次按键都喂给它，看着就是
    /// 「输入无反应」。判据用**候选层自己**回答（把缓冲临时换成尾巴问一遍
    /// `candidates()`）—— 数字串结构上分不出「还在打的下一音节」与「乱按出来的垃圾」，
    /// 长度分档 + 候选探测合起来才是这次裁决的口径。代价 = 一次候选计算（只在长尾巴被
    /// 选中时发生，与这条路上已有的几次词库前缀查询同档）。
    /// 26 键/笔画不走这里：它们的尾巴是**可见的**预编辑文本，留着不影响判断。
    fn digit_tail_to_keep(&mut self, buffer: &str) -> Option<String> {
        let suffix =
            scheme_fallback_len(self.active_dict(), buffer, self.scheme(), self.active_pin())
                .map(|k| buffer[k..].to_string())
                .filter(|suffix| !suffix.is_empty())?;
        if suffix.chars().count() < 3 {
            return Some(suffix); // 短尾巴：照旧留（见头注两条理由）
        }
        // 探针：**只看严格档**（`tail_guess = false`）—— 宽松兜底档（2026-10-15 同日裁决，
        // 见 `candidates_scheme::tail_guess`）会让任意尾巴都「出得来候选」，拿 `Engine::candidates`
        // 去问就把本条裁决悄悄废掉（那条尾巴会永远留着）。判据必须是「严格档有没有候选」：
        // 一条只有宽松档能读的尾巴仍然算死尾巴，整段清空。
        // 顺带不碰 `scheme_readings`（`Engine::candidates` 会写那份缓存，这里直接调下层）。
        let mode = self.mode();
        let matchable = !crate::candidates::rank_and_pick_scheme(
            self.active_dict(),
            &self.learner,
            &suffix,
            1,
            self.user_boost,
            self.scheme(),
            self.active_pin(),
            mode == Mode::Pinyin,
            false, // 宽松兜底档：**关**（本探针问的就是严格档）
        )
        .0
        .is_empty();
        matchable.then_some(suffix)
    }
}
