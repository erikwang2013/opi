// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use super::*;
use crate::dict_budget::QueryBudget;
use std::time::{Duration, Instant};

#[derive(Default)]
struct StubDict {
    prefix: Vec<(String, u32)>,
    suffixes: Vec<(String, u32)>,
}

impl Dictionary for StubDict {
    fn query(&self, _pinyin: &str, _limit: usize) -> Vec<Entry> {
        Vec::new()
    }

    fn words_with_prefix(&self, _prefix: &str) -> Vec<(String, u32)> {
        self.prefix.clone()
    }

    fn association_suffixes(&self, _root: &str) -> Vec<(String, u32)> {
        self.suffixes.clone()
    }

    fn len(&self) -> usize {
        0
    }
}

#[test]
fn layered_dictionary_delegates_association_to_base_only() {
    let base = StubDict {
        prefix: vec![("今天晚上".into(), 7)],
        suffixes: vec![("回家".into(), 9)],
    };
    let mut d = LayeredDictionary::new(Box::new(base));
    d.set_extras(vec![("导入".into(), {
        let mut e = InMemoryDictionary::new();
        e.insert("jintian", "今天导入", 99);
        e
    })]);
    assert_eq!(d.words_with_prefix("今天"), vec![("今天晚上".into(), 7)]);
    assert_eq!(d.association_suffixes("今天"), vec![("回家".into(), 9)]);
}

#[test]
fn in_memory_dictionary_has_no_association() {
    let mut d = InMemoryDictionary::new();
    d.insert("jintian", "今天", 1);
    assert!(d.words_with_prefix("今天").is_empty());
    assert!(d.association_suffixes("今天").is_empty());
}

/// 2026-11-02 性能项 6：base 查询的耗时必须计入导入层总账。
///
/// 手工把预算槽置成「后处理余量已耗尽」：固定导入截止还在 60s 之后，旧实现
/// （只看固定截止）会照常扫导入层；动态截止必须当场把整层丢弃、只回 base。
/// 用状态注入而不是 sleep，避免负载下变成偶发绿。
#[test]
fn visit_paths_count_base_time_before_scanning_import_layer() {
    let mut base = InMemoryDictionary::new();
    base.insert("ni", "内置", 5);
    let mut extra = InMemoryDictionary::new();
    extra.insert("nihao", "导入", 99);
    let mut d = LayeredDictionary::new(Box::new(base));
    d.set_extras(vec![("extra".into(), extra)]);

    // 前置：没有活动预算（Inactive）时导入层照常并入 —— 证明下面不是「本来就没装」。
    let mut merged = Vec::new();
    d.visit_prefix_budgeted("ni", &mut |_py, w, f| merged.push((w.to_string(), f)));
    assert!(
        merged.iter().any(|(w, _)| w == "导入"),
        "前置：导入层应可见，实际 {merged:?}"
    );

    // base 已把「总账 − 20ms 后处理余量」花光，但固定导入截止仍很远。
    let now = Instant::now();
    *d.query_budget.lock().unwrap() = QueryState::Limited(QueryBudget {
        import_deadline: now + Duration::from_secs(60),
        total_deadline: now - dict_budget::QUERY_POST_RESERVE,
    });

    let mut prefix = Vec::new();
    d.visit_prefix_budgeted("ni", &mut |_py, w, f| prefix.push((w.to_string(), f)));
    assert_eq!(
        prefix,
        vec![("内置".to_string(), 5)],
        "前缀访问：base 耗时用尽总账 ⇒ 导入层整层丢弃"
    );

    let mut all = Vec::new();
    d.visit_entries_budgeted(&mut |_py, w, f| all.push((w.to_string(), f)));
    assert_eq!(
        all,
        vec![("内置".to_string(), 5)],
        "全量访问：base 耗时用尽总账 ⇒ 导入层整层丢弃"
    );
}
