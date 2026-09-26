/// 词典条目：词语 + 静态词频 + 拼音字节长度。
/// `pinyin_len` 供候选排序区分「精确等长匹配」与「前缀扩展」（见 candidates.rs）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub word: String,
    pub freq: u32,
    pub pinyin_len: usize,
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
        // 用「键原本是否已存在」判断新增，而不是拿 `*entry == 0` 当新建的代理：
        // freq=0 是合法词频（TSV 的 f 列可为 0，`parse_freq("0")` 返回 Some(0)），
        // 首次以 0 插入时 `0 > 0` 为假 → 不计数，但条目已进 map、query_prefix 查得到，
        // 于是 len()/is_empty() 与查询结果自相矛盾。
        let is_new = !node.entries.contains_key(word);
        let entry = node.entries.entry(word.to_owned()).or_insert(0);
        if is_new {
            self.len += 1;
        }
        if freq > *entry {
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
        collect(node, pinyin.len(), &mut acc);
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

/// 深度（= 已走拼音的字节数）随递归下传，作为条目的 `pinyin_len`。
fn collect(node: &Node, depth: usize, acc: &mut Vec<Entry>) {
    for (word, freq) in &node.entries {
        acc.push(Entry { word: word.clone(), freq: *freq, pinyin_len: depth });
    }
    for (c, child) in &node.children {
        collect(child, depth + c.len_utf8(), acc);
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

    /// freq=0 是合法词频，首次以 0 插入也必须计数 —— 否则 len() 与 query 结果矛盾。
    #[test]
    fn zero_freq_entry_is_counted() {
        let mut t = Trie::new();
        t.insert("hao", "好", 0);
        assert_eq!(t.query_prefix("hao", 10).len(), 1, "freq=0 的词条应查得到");
        assert_eq!(t.len(), 1, "len() 必须与 query 结果一致");
        assert!(!t.is_empty());
        // 重复插入不重复计数，且更高频才覆盖
        t.insert("hao", "好", 0);
        assert_eq!(t.len(), 1);
        t.insert("hao", "好", 5);
        assert_eq!(t.len(), 1);
        assert_eq!(t.query_prefix("hao", 1)[0].freq, 5);
    }
}
