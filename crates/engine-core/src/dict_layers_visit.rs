// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入层的 `visit_entries`（2026-10-18 从 `dict_layers.rs` 拆出：加每键预算与可抢占
//! 扫描后那里 497 行，rustfmt 余量不足 500 行门禁；先例是 `engine_dicts.rs` 一族）。
//!
//! 语义与拆分前逐字相同：键 = (拼音串, 词)，两级 HashMap 让 base 的每条都拿 `&str`
//! 直查、不克隆；2026-10-18 起导入层扫描共享每键预算、逐条可抢占，超时**整层丢弃**
//! （只转发 base），rest 超过 `IMPORT_HIT_CAP` 时按最终排序键截断。

use crate::dict_index::{self, boosted_import_freq};
use crate::dict_layers::LayeredDictionary;
use std::collections::HashMap;
use std::time::Instant;

impl LayeredDictionary {
    pub(crate) fn visit_entries_budgeted(&self, f: &mut dyn FnMut(&str, &str, u32)) {
        if self.extras.is_empty() {
            return self.base.visit_entries(f);
        }
        // 键 = (拼音串, 词)：这里能拿到真拼音（不像查询族只有键长），与「一个大字典」
        // 的键逐字相同。索引只存**导入层**的条目（小），两级 HashMap 是为了 base 的
        // 每条都拿 `&str` 直查、不克隆 —— 单个 `HashMap<(&str, &str), _>` 会被
        // 生命周期间题卡住（闭包参数是 for<'x> 的高阶借用，装不进固定生命周期的键）。
        let deadline = self.import_deadline();
        let base_max = self.base_max();
        let mut idx: HashMap<String, HashMap<String, u32>> = HashMap::new();
        let mut seen = 0u32;
        for (_, dict) in &self.extras {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return self.base.visit_entries(f);
            }
            let mut stop = false;
            let ok = dict.for_each_entry_until(&mut |py, w, freq| {
                seen += 1;
                if seen.is_multiple_of(64) && deadline.is_some_and(|d| Instant::now() >= d) {
                    stop = true;
                    return false;
                }
                if seen as usize >= dict_index::IMPORT_SCAN_HIT_LIMIT {
                    stop = true;
                    return false;
                }
                let eff = boosted_import_freq(freq, base_max);
                idx.entry(py.to_string())
                    .or_default()
                    .entry(w.to_string())
                    .and_modify(|f| *f = (*f).max(eff))
                    .or_insert(eff);
                true
            });
            if !ok || stop {
                return self.base.visit_entries(f);
            }
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return self.base.visit_entries(f);
        }
        self.base.visit_entries(&mut |py, w, freq| {
            let merged = match idx.get_mut(py).and_then(|words| words.remove(w)) {
                Some(extra) => freq.max(extra),
                None => freq,
            };
            (*f)(py, w, merged);
        });
        // 剩余 = 没被 base 消费掉的条目（词频已在收集时加成）。先按最终排序键取
        // top-[IMPORT_HIT_CAP]，再按 (拼音, 词) 排 —— HashMap 迭代序不定，排序只为
        // 输出确定；超过上限的尾巴在可见窗口之外（2026-10-18 与查询族同一条取舍）。
        let mut rest: Vec<(String, String, u32)> = Vec::new();
        for (py, words) in idx {
            for (w, freq) in words {
                rest.push((py.clone(), w, freq));
            }
        }
        rest.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
        rest.truncate(dict_index::IMPORT_HIT_CAP);
        rest.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (py, w, freq) in rest {
            (*f)(&py, &w, freq);
        }
    }

    /// [`Dictionary::visit_prefix`] 的导入层合并：语义与 `visit_entries_budgeted` 逐条
    /// 相同（同键取 max、导入层加成、超预算整层丢弃），只是 base 与导入层都只扫
    /// `prefix` 区间 —— 不走这里的话默认实现会 `visit_entries` 全表 93 万条再过滤，
    /// 声母缩写每键实测 30–100ms（2026-10-22）。
    pub(crate) fn visit_prefix_budgeted(&self, prefix: &str, f: &mut dyn FnMut(&str, &str, u32)) {
        if self.extras.is_empty() {
            return self.base.visit_prefix(prefix, f);
        }
        let deadline = self.import_deadline();
        let base_max = self.base_max();
        let mut idx: HashMap<String, HashMap<String, u32>> = HashMap::new();
        let mut seen = 0u32;
        for (_, dict) in &self.extras {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return self.base.visit_prefix(prefix, f);
            }
            let mut stop = false;
            let ok = dict.for_each_prefix_until(prefix, &mut |py, w, freq| {
                seen += 1;
                if seen.is_multiple_of(64) && deadline.is_some_and(|d| Instant::now() >= d) {
                    stop = true;
                    return false;
                }
                if seen as usize >= dict_index::IMPORT_SCAN_HIT_LIMIT {
                    stop = true;
                    return false;
                }
                let eff = boosted_import_freq(freq, base_max);
                idx.entry(py.to_string())
                    .or_default()
                    .entry(w.to_string())
                    .and_modify(|f| *f = (*f).max(eff))
                    .or_insert(eff);
                true
            });
            if !ok || stop {
                return self.base.visit_prefix(prefix, f);
            }
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return self.base.visit_prefix(prefix, f);
        }
        self.base.visit_prefix(prefix, &mut |py, w, freq| {
            let merged = match idx.get_mut(py).and_then(|words| words.remove(w)) {
                Some(extra) => freq.max(extra),
                None => freq,
            };
            (*f)(py, w, merged);
        });
        let mut rest: Vec<(String, String, u32)> = Vec::new();
        for (py, words) in idx {
            for (w, freq) in words {
                rest.push((py.clone(), w, freq));
            }
        }
        rest.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
        rest.truncate(dict_index::IMPORT_HIT_CAP);
        rest.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (py, w, freq) in rest {
            (*f)(&py, &w, freq);
        }
    }
}
