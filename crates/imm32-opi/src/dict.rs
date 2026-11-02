// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 只读词库共享：`Engine` 持 `Box<dyn Dictionary>`，每个 HIMC 各建一份引擎就
//! 会把主词库再装载一遍。外壳持 `Arc`，各 HIMC 的引擎各拿一个 `Box<Shell>`，
//! 底层仍是**同一份**词库（mmap 映射/校验只做一次）。
//!
//! **所有** trait 方法都显式转发：默认实现里 `query_prefix_top` 会退化成全量
//! `query_all`、`visit_entries` 是空实现 —— 漏转发不会报错，只会让大词库性能
//! 与 9/14 键方案静默失效。

use std::sync::Arc;

use engine_core::{Dictionary, Entry, Scheme};

struct SharedDictionary(Arc<dyn Dictionary>);

impl Dictionary for SharedDictionary {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        self.0.query(pinyin, limit)
    }

    fn query_all(&self, pinyin: &str) -> Vec<Entry> {
        self.0.query_all(pinyin)
    }

    fn query_prefix_top(&self, prefix: &str, n: usize) -> Vec<Entry> {
        self.0.query_prefix_top(prefix, n)
    }

    fn visit_entries(&self, f: &mut dyn FnMut(&str, &str, u32)) {
        self.0.visit_entries(f)
    }

    fn visit_prefix(&self, prefix: &str, f: &mut dyn FnMut(&str, &str, u32)) {
        self.0.visit_prefix(prefix, f)
    }

    fn scheme_entries(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        n: usize,
    ) -> Vec<(Entry, String)> {
        self.0.scheme_entries(input, scheme, pin, n)
    }

    fn scheme_find(
        &self,
        input: &str,
        scheme: Scheme,
        pin: Option<&str>,
        pred: &mut dyn FnMut(&str, &str) -> bool,
    ) -> Option<usize> {
        self.0.scheme_find(input, scheme, pin, pred)
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn max_freq(&self) -> u64 {
        self.0.max_freq()
    }

    fn words_with_prefix(&self, prefix: &str) -> Vec<(String, u32)> {
        self.0.words_with_prefix(prefix)
    }

    fn association_suffixes(&self, root: &str) -> Vec<(String, u32)> {
        self.0.association_suffixes(root)
    }
}

/// 从进程级共享句柄造一个引擎可接的 `Box<dyn Dictionary>`（只加一层指针）。
pub(crate) fn shared(dict: &Arc<dyn Dictionary>) -> Box<dyn Dictionary> {
    Box::new(SharedDictionary(Arc::clone(dict)))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use engine_core::InMemoryDictionary;

    use super::*;

    struct Counting {
        inner: InMemoryDictionary,
        calls: Arc<AtomicUsize>,
    }

    impl Dictionary for Counting {
        fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inner.query(pinyin, limit)
        }

        fn len(&self) -> usize {
            self.inner.len()
        }
    }

    #[test]
    fn handles_share_one_dictionary_allocation() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut inner = InMemoryDictionary::new();
        inner.insert("ni", "你", 1000);
        let original: Arc<dyn Dictionary> = Arc::new(Counting {
            inner,
            calls: calls.clone(),
        });

        let a = shared(&original);
        let b = shared(&original);
        assert_eq!(a.query("ni", 8)[0].word, "你");
        assert_eq!(b.query("ni", 8)[0].word, "你");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "两次查询都落到同一词库");
        assert_eq!(Arc::strong_count(&original), 3, "只多两个引用，不复制词库");
    }

    /// 转发必须走内层覆写：默认 `query_prefix_top` = 全量重排，默认 `visit_entries`
    /// 为空 —— 两者行为不同，用它证明确实转发到了。
    #[test]
    fn optional_methods_reach_inner_overrides() {
        struct Overrides(AtomicUsize);

        impl Dictionary for Overrides {
            fn query(&self, _pinyin: &str, _limit: usize) -> Vec<Entry> {
                Vec::new()
            }

            fn query_prefix_top(&self, _prefix: &str, _n: usize) -> Vec<Entry> {
                self.0.fetch_add(1, Ordering::SeqCst);
                vec![Entry {
                    word: "你".into(),
                    freq: 1,
                    pinyin_len: 2,
                }]
            }

            fn visit_entries(&self, f: &mut dyn FnMut(&str, &str, u32)) {
                self.0.fetch_add(1, Ordering::SeqCst);
                f("ni", "你", 1);
            }

            fn len(&self) -> usize {
                1
            }
        }

        let original: Arc<dyn Dictionary> = Arc::new(Overrides(AtomicUsize::new(0)));
        let d = shared(&original);
        assert_eq!(d.query_prefix_top("n", 8).len(), 1);
        let mut seen = 0;
        d.visit_entries(&mut |_, _, _| seen += 1);
        assert_eq!(seen, 1);
    }
}
