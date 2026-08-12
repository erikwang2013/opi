/// 词典条目：词语 + 静态词频。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub word: String,
    pub freq: u32,
}

use std::collections::BTreeMap;

/// Trie 节点：末端词频表 + 子节点表（BTreeMap 保证确定性顺序）。
/// 词频表以词为键，允许同一拼音路径下挂多个词。
#[derive(Default)]
struct Node {
    entries: BTreeMap<String, u32>,
    children: BTreeMap<char, Node>,
}

impl Node {
    fn new() -> Self {
        Node { entries: BTreeMap::new(), children: BTreeMap::new() }
    }
}

#[derive(Default)]
pub struct Trie {
    root: Node,
    len: usize,
}

impl Trie {
    pub fn new() -> Self {
        Trie { root: Node::new(), len: 0 }
    }

    /// 插入或更新 (pinyin, word) 条目。同词重复插入取更高频。
    pub fn insert(&mut self, pinyin: &str, word: &str, freq: u32) {
        let mut node = &mut self.root;
        for c in pinyin.chars() {
            node = node.children.entry(c).or_default();
        }
        let entry = node.entries.entry(word.to_owned()).or_insert(0);
        if freq > *entry {
            if *entry == 0 {
                self.len += 1;
            }
            *entry = freq;
        }
    }

    /// 查询前缀，按词频降序截断到 limit。空前缀不返回任何词。
    pub fn query_prefix(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        if pinyin.is_empty() {
            return Vec::new();
        }
        let mut node = &self.root;
        for c in pinyin.chars() {
            match node.children.get(&c) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut acc = Vec::new();
        collect(node, &mut acc);
        acc.sort_by_key(|e| std::cmp::Reverse(e.freq));
        acc.truncate(limit);
        acc
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn collect(node: &Node, acc: &mut Vec<Entry>) {
    for (word, freq) in &node.entries {
        acc.push(Entry { word: word.clone(), freq: *freq });
    }
    for child in node.children.values() {
        collect(child, acc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query() {
        let mut t = Trie::new();
        t.insert("hao", "好", 5000);
        t.insert("hao", "号", 1200);
        let got = t.query_prefix("hao", 8);
        assert_eq!(got[0].word, "好");
        assert_eq!(got[0].freq, 5000);
        assert_eq!(got[1].word, "号");
    }

    #[test]
    fn query_prefix_picks_longest_entry() {
        let mut t = Trie::new();
        t.insert("xiao", "笑", 3000);
        t.insert("xiang", "想", 4000);
        let got = t.query_prefix("xiang", 8);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].word, "想");
    }

    #[test]
    fn upsert_keeps_higher_freq() {
        let mut t = Trie::new();
        t.insert("hao", "好", 100);
        t.insert("hao", "好", 5000);
        let got = t.query_prefix("hao", 8);
        assert_eq!(got[0].freq, 5000);
    }

    #[test]
    fn limit_truncates_sorted() {
        let mut t = Trie::new();
        for (i, w) in ["甲", "乙", "丙", "丁"].iter().enumerate() {
            t.insert("jia", w, 100 - i as u32);
        }
        let got = t.query_prefix("jia", 2);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "甲");
        assert_eq!(got[1].word, "乙");
    }

    #[test]
    fn empty_prefix_returns_nothing() {
        let mut t = Trie::new();
        t.insert("hao", "好", 5000);
        assert!(t.query_prefix("", 8).is_empty());
    }

    #[test]
    fn len_counts_entries() {
        let mut t = Trie::new();
        t.insert("hao", "好", 1);
        t.insert("hao", "号", 1);
        t.insert("xiao", "笑", 1);
        assert_eq!(t.len(), 3);
    }
}
