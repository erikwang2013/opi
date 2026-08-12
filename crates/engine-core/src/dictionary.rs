use crate::trie::{Entry, Trie};

/// 词典抽象。engine 只依赖此 trait，M2 换成 mmap 二进制实现。
pub trait Dictionary {
    /// 按拼音查候选，返回按词频降序、长度不超过 limit 的条目。
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Default)]
pub struct InMemoryDictionary {
    trie: Trie,
}

impl InMemoryDictionary {
    pub fn new() -> Self {
        InMemoryDictionary { trie: Trie::new() }
    }

    pub fn insert(&mut self, pinyin: &str, word: &str, freq: u32) {
        self.trie.insert(pinyin, word, freq);
    }
}

impl Dictionary for InMemoryDictionary {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        self.trie.query_prefix(pinyin, limit)
    }

    fn len(&self) -> usize {
        self.trie.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_query_works() {
        let mut d = InMemoryDictionary::new();
        d.insert("hao", "好", 5000);
        d.insert("hao", "号", 1200);
        let got = d.query("hao", 8);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "好");
    }

    #[test]
    fn trait_default_is_empty() {
        let d = InMemoryDictionary::new();
        assert!(d.is_empty());
        assert_eq!(d.len(), 0);
    }
}
