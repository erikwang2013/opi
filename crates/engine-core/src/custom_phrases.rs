// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语（2026-10-23）：`trigger -> text`，**精确**命中时排在所有候选之前。
//!
//! 语义（产品裁决）：
//! - **中英两套，完全独立**（2026-10-24）：中文套只在 Pinyin / Traditional 匹配，
//!   英文套只在 English 匹配；两套各有自己的开关、导入/导出，互不影响。
//! - 中文套只在方案 Full / Nine / Fourteen 匹配；其余方案恒不匹配。英文套的缓冲
//!   是字母原文，直接按大小写不敏感比较（不看方案）。
//! - 匹配 = 当前缓冲**恰好等于** trigger（先去掉分词符 `'`）。9/14 键把 trigger 的
//!   字母按 [`Scheme::key_class`] 折成按键类再比 —— 走 [`Scheme::matches`]，与词库
//!   匹配同一张表（四端不另抄）。
//! - 多条命中保持导入顺序；候选 kind 用 `Hanzi`（不新增 kind）。
//! - 短语在正常候选算完之后**前插**，整体再按 `limit` 截断 ⇒ limit=1 也能看到。
//!
//! 存储与持久化：引擎只持内存（本模块 **不碰 IO**，同 engine-core 的不变量）。
//! 宿主经 [`Engine::set_custom_phrases`] / [`Engine::export_custom_phrases`]（英文套
//! 用 `*_english_*` 一族）负责落盘。
//! JSON 契约：`{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}`（两套相同）。
//!
//! 校验（全有或全无：失败不改旧表）：version==1；trigger 非空、纯 ASCII 字母、
//! 长度 <= [`MAX_TRIGGER_LEN`]（大写归一为小写）；text 非空；条数 <=
//! [`MAX_CUSTOM_PHRASES`]；**归一后的 trigger 重复 ⇒ 整份拒绝**（不做 last-wins：
//! 静默丢条会让宿主保存的内容与引擎里的不一致，且无法在导出里看出被丢过）。

use serde::{Deserialize, Serialize};

use crate::candidates::{Candidate, CandidateKind};
use crate::composer::Mode;
use crate::engine::Engine;
use crate::scheme::Scheme;

/// 短语条数上限（导入是信任边界：畸形/超大文件不许把内存打爆）。
pub const MAX_CUSTOM_PHRASES: usize = 2000;
/// trigger 长度上限（字符；trigger 限 ASCII，故与字节数相同）。
pub const MAX_TRIGGER_LEN: usize = 32;

/// 导出 JSON 的顶层结构（字段序 = 导出键序）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PhrasesJson {
    version: u32,
    phrases: Vec<PhraseJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PhraseJson {
    trigger: String,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomPhrase {
    pub(crate) trigger: String,
    pub(crate) text: String,
}

pub(crate) struct CustomPhrases {
    /// 缺省**开**：列表为空时开关无副作用；宿主导入后即使还没恢复开关也应该立刻可用。
    enabled: bool,
    phrases: Vec<CustomPhrase>,
}

impl Default for CustomPhrases {
    fn default() -> Self {
        Self {
            enabled: true,
            phrases: Vec::new(),
        }
    }
}

impl CustomPhrases {
    fn set_json(&mut self, json: &str) -> Result<usize, String> {
        let parsed: PhrasesJson =
            serde_json::from_str(json).map_err(|e| format!("自定义短语解析失败: {e}"))?;
        if parsed.version != 1 {
            return Err(format!(
                "自定义短语版本不支持: {}（只认 1）",
                parsed.version
            ));
        }
        if parsed.phrases.len() > MAX_CUSTOM_PHRASES {
            return Err(format!(
                "自定义短语过多: {} 条（上限 {MAX_CUSTOM_PHRASES}）",
                parsed.phrases.len()
            ));
        }
        // 先校完整份再落内存 —— 中途失败不留半份表。
        let mut out = Vec::with_capacity(parsed.phrases.len());
        let mut seen = std::collections::HashSet::with_capacity(parsed.phrases.len());
        for p in parsed.phrases {
            let trigger = p.trigger.to_ascii_lowercase();
            if trigger.is_empty() || !trigger.bytes().all(|b| b.is_ascii_alphabetic()) {
                return Err(format!("短语 trigger 必须是 ASCII 字母: {:?}", p.trigger));
            }
            if trigger.len() > MAX_TRIGGER_LEN {
                return Err(format!(
                    "短语 trigger 过长: {} 个字符（上限 {MAX_TRIGGER_LEN}）",
                    trigger.len()
                ));
            }
            if p.text.is_empty() {
                return Err(format!("短语 {trigger:?} 的 text 为空"));
            }
            if !seen.insert(trigger.clone()) {
                return Err(format!("短语 trigger 重复: {trigger:?}"));
            }
            out.push(CustomPhrase {
                trigger,
                text: p.text,
            });
        }
        self.phrases = out;
        Ok(self.phrases.len())
    }

    fn export_json(&self) -> String {
        let phrases = self
            .phrases
            .iter()
            .map(|p| PhraseJson {
                trigger: p.trigger.clone(),
                text: p.text.clone(),
            })
            .collect();
        serde_json::to_string(&PhrasesJson {
            version: 1,
            phrases,
        })
        .unwrap()
    }

    /// 当前缓冲精确命中的**中文**短语（保持导入顺序）。`buffer` 为 composer 原文。
    fn matching_cn<'a>(&'a self, buffer: &str, scheme: Scheme) -> Vec<&'a CustomPhrase> {
        if !self.enabled || self.phrases.is_empty() {
            return Vec::new();
        }
        let key = strip_separators(buffer);
        self.phrases
            .iter()
            .filter(|p| phrase_matches(scheme, &p.trigger, &key))
            .collect()
    }

    /// 当前缓冲精确命中的**英文**短语。英文缓冲是字母原文（不经过方案类表），
    /// 故直接做大小写不敏感比较 —— 即便当时方案还停在 Nine 也不会误判。
    fn matching_en<'a>(&'a self, buffer: &str) -> Vec<&'a CustomPhrase> {
        if !self.enabled || self.phrases.is_empty() {
            return Vec::new();
        }
        let key = strip_separators(buffer);
        self.phrases
            .iter()
            .filter(|p| key.len() == p.trigger.len() && key.eq_ignore_ascii_case(&p.trigger))
            .collect()
    }
}

/// 分词符不参与比较（`d'z` 与 trigger `dz` 同义）。
fn strip_separators(buffer: &str) -> String {
    buffer.chars().filter(|c| *c != '\'').collect()
}

/// 缓冲（已去 `'`）是否**恰好**是该 trigger。
fn phrase_matches(scheme: Scheme, trigger: &str, buffer: &str) -> bool {
    if buffer.len() != trigger.len() {
        return false;
    }
    match scheme {
        Scheme::Full => buffer.eq_ignore_ascii_case(trigger),
        // 9/14 键：缓冲是按键串（数字 / 14 键符号），trigger 是字母 —— 用方案自己的
        // 类表逐位比，与词库匹配同一套语义；取不到类的字符即不匹配。
        Scheme::Nine | Scheme::Fourteen => scheme.matches(buffer, trigger),
        Scheme::Stroke | Scheme::Shuangpin | Scheme::Wubi => false,
    }
}

impl Engine {
    /// 导入自定义短语 JSON（全有或全无）。返回导入条数；失败时旧表原样保留。
    /// 语义与校验规则见模块头。
    pub fn set_custom_phrases(&mut self, json: &str) -> Result<usize, String> {
        self.custom_phrases.set_json(json)
    }

    /// 导出当前短语表（与 [`Self::set_custom_phrases`] 同一 schema、同一顺序）。
    pub fn export_custom_phrases(&self) -> String {
        self.custom_phrases.export_json()
    }

    pub fn set_custom_phrases_enabled(&mut self, on: bool) {
        self.custom_phrases.enabled = on;
    }

    pub fn custom_phrases_enabled(&self) -> bool {
        self.custom_phrases.enabled
    }

    /// 导入**英文**自定义短语（与中文套同 schema/同校验，表完全独立）。
    pub fn set_english_custom_phrases(&mut self, json: &str) -> Result<usize, String> {
        self.custom_phrases_en.set_json(json)
    }

    pub fn export_english_custom_phrases(&self) -> String {
        self.custom_phrases_en.export_json()
    }

    pub fn set_english_custom_phrases_enabled(&mut self, on: bool) {
        self.custom_phrases_en.enabled = on;
    }

    pub fn english_custom_phrases_enabled(&self) -> bool {
        self.custom_phrases_en.enabled
    }

    /// 英文短语是否**实际可命中**（开关开且表非空）。宿主据此决定英文普通模式要不要
    /// 把字母送进引擎缓冲 —— 直出模式下短语永远匹配不到（窗口已进文档，收不回来）。
    pub fn english_custom_phrases_active(&self) -> bool {
        self.custom_phrases_en.enabled && !self.custom_phrases_en.phrases.is_empty()
    }

    /// 公开候选入口：正常候选（[`Self::candidates_base`]）算完后，把**精确命中**的
    /// 自定义短语前插并整体按 `limit` 截断。没有短语时逐字节等同旧行为。
    pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
        let mut cands = self.candidates_base(limit);
        if limit == 0 {
            return cands;
        }
        let mut phrases = self.custom_phrase_candidates();
        if phrases.is_empty() {
            return cands;
        }
        phrases.append(&mut cands);
        phrases.truncate(limit);
        phrases
    }

    /// 当前缓冲命中的短语（按导入顺序）。模式/方案/浏览态/携带缓冲的门都在这里；
    /// 中文模式查中文套、英文模式查英文套 —— 两套互不串门。
    fn matching_custom_phrases(&self) -> Vec<&CustomPhrase> {
        let s = self.composer.session();
        if self.symbol_browse.is_active() {
            return Vec::new();
        }
        match s.mode {
            Mode::Pinyin | Mode::Traditional
                // 英→中保留的英文词不是拼音（与 `candidates_base` 同一判据）。
                if self.composer.carried_from() != Some(Mode::English) =>
            {
                self.custom_phrases.matching_cn(&s.buffer, self.scheme())
            }
            // 中→英保留的拼音不是英文 trigger：不出英文短语。
            Mode::English if self.composer.carried_from().is_none() => {
                self.custom_phrases_en.matching_en(&s.buffer)
            }
            _ => Vec::new(),
        }
    }

    /// 当前缓冲命中的短语候选（按导入顺序）。
    fn custom_phrase_candidates(&self) -> Vec<Candidate> {
        self.matching_custom_phrases()
            .into_iter()
            .map(|p| Candidate {
                text: p.text.clone(),
                kind: CandidateKind::Hanzi,
                // 前插已保证位次；分数只求不参与任何排序比较。
                score: u64::MAX,
            })
            .collect()
    }

    /// 选中的文本是否正是当前缓冲命中的某条短语（收尾层据此**整段消耗**缓冲）。
    ///
    /// 短语文本不是缓冲拼音的投影：若照分词回退切前缀，`zhangsan` 这类 trigger 会把
    /// 短语拆成两半、留在逐字态里。判据自带缓冲校验 ⇒ 不依赖调用方先取过候选。
    pub(crate) fn custom_phrase_selected(&self, selected: &str) -> bool {
        self.matching_custom_phrases()
            .iter()
            .any(|p| p.text == selected)
    }
}
