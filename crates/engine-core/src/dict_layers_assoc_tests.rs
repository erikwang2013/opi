// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use super::*;

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
