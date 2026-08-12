/// Unicode 区块 ID（V1 用 u16 编号，M2 数据管线扩展）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u16);

/// Unicode 区块定义。common=true 表示进"常用"面板。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub common: bool,
}

/// 符号条目。keywords 为拼音/英文搜索词，emoji 标记表情。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntry {
    pub text: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub block: BlockId,
    pub emoji: bool,
}

pub struct SymbolEngine {
    blocks: Vec<Block>,
    entries: Vec<SymbolEntry>,
    keywords: Vec<(String, usize)>, // 排序后的 (小写 keyword, entry 下标)
    block_index: std::collections::HashMap<BlockId, Vec<usize>>,
}

impl SymbolEngine {
    /// blocks 与 entries 由调用方提供（M2 起来自数据文件），此处构建索引。
    pub fn new(mut blocks: Vec<Block>, entries: Vec<SymbolEntry>) -> Self {
        blocks.sort_by_key(|b| b.start);
        debug_assert!(
            blocks.windows(2).all(|w| w[1].start > w[0].end),
            "Unicode 区块不可重叠"
        );
        let mut block_index: std::collections::HashMap<BlockId, Vec<usize>> =
            std::collections::HashMap::new();
        let mut keywords: Vec<(String, usize)> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            block_index.entry(e.block).or_default().push(i);
            for kw in &e.keywords {
                keywords.push((kw.to_lowercase(), i));
            }
        }
        keywords.sort();
        keywords.dedup();
        SymbolEngine { blocks, entries, keywords, block_index }
    }

    /// 内置精简符号集（覆盖主要用例，M2 由编译产物替代）。
    pub fn builtin() -> Self {
        let blocks = vec![
            Block { id: BlockId(1), start: 0x3000, end: 0x303F, name: "CJK 符号".into(), common: true },
            Block { id: BlockId(2), start: 0x25A0, end: 0x25FF, name: "几何图形".into(), common: true },
            Block { id: BlockId(3), start: 0x2600, end: 0x26FF, name: "杂项符号".into(), common: true },
            Block { id: BlockId(4), start: 0x1F600, end: 0x1F64F, name: "表情符号".into(), common: true },
            Block { id: BlockId(5), start: 0x3400, end: 0x4DBF, name: "CJK 扩展 A".into(), common: false },
            Block { id: BlockId(6), start: 0x3040, end: 0x309F, name: "平假名".into(), common: true },
        ];
        let entries = vec![
            SymbolEntry { text: "、".into(), name: "顿号".into(), keywords: vec!["dun".into(), "comma".into()], block: BlockId(1), emoji: false },
            SymbolEntry { text: "。".into(), name: "句号".into(), keywords: vec!["ju".into(), "period".into()], block: BlockId(1), emoji: false },
            SymbolEntry { text: "〈".into(), name: "左书名号".into(), keywords: vec!["shu".into()], block: BlockId(1), emoji: false },
            SymbolEntry { text: "▲".into(), name: "上三角".into(), keywords: vec!["sjx".into(), "triangle".into()], block: BlockId(2), emoji: false },
            SymbolEntry { text: "♥".into(), name: "心形".into(), keywords: vec!["heart".into(), "ai".into(), "xin".into()], block: BlockId(3), emoji: false },
            SymbolEntry { text: "★".into(), name: "实心星".into(), keywords: vec!["star".into(), "xing".into()], block: BlockId(3), emoji: false },
            SymbolEntry { text: "😄".into(), name: "微笑".into(), keywords: vec!["xiao".into(), "smile".into(), "laugh".into()], block: BlockId(4), emoji: true },
            SymbolEntry { text: "あ".into(), name: "平假名a".into(), keywords: vec!["a".into()], block: BlockId(6), emoji: false },
        ];
        SymbolEngine::new(blocks, entries)
    }

    /// 查询字符所属区块。
    pub fn block_of(&self, ch: char) -> Option<Block> {
        let cp = ch as u32;
        self.blocks
            .iter()
            .find(|b| cp >= b.start && cp <= b.end)
            .cloned()
    }

    /// 常用区块列表（面板 Tab）。
    pub fn common_blocks(&self) -> Vec<Block> {
        self.blocks.iter().filter(|b| b.common).cloned().collect()
    }

    /// 区块内全部符号。
    pub fn entries_in_block(&self, id: BlockId) -> Vec<SymbolEntry> {
        self.block_index
            .get(&id)
            .map(|idx| idx.iter().map(|&i| self.entries[i].clone()).collect())
            .unwrap_or_default()
    }

    /// 关键字前缀搜索（拼音或英文小写；输入为完整/部分拼音）。
    pub fn search(&self, keyword: &str) -> Vec<SymbolEntry> {
        let kw = keyword.to_lowercase();
        let keys: Vec<&str> = self.keywords.iter().map(|(k, _)| k.as_str()).collect();
        let lo = keys.partition_point(|k| k.as_bytes() < kw.as_bytes());
        let hi = match byte_successor(kw.as_bytes()) {
            Some(succ) => keys.partition_point(|k| k.as_bytes() < succ.as_slice()),
            None => keys.len(),
        };
        let mut out: Vec<SymbolEntry> = self.keywords[lo..hi]
            .iter()
            .map(|(_, i)| self.entries[*i].clone())
            .collect();
        out.sort_by(|a, b| a.text.cmp(&b.text));
        out.dedup_by(|a, b| a.text == b.text);
        out
    }
}

/// 字节后继：末字节 +1（带进位）；全 0xFF 返回 None。
fn byte_successor(p: &[u8]) -> Option<Vec<u8>> {
    let mut b = p.to_vec();
    let mut i = b.len();
    while i > 0 {
        i -= 1;
        let (nb, overflow) = b[i].overflowing_add(1);
        b[i] = nb;
        if !overflow {
            return Some(b);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_block_lookup() {
        let e = SymbolEngine::builtin();
        let b = e.block_of('。').unwrap();
        assert_eq!(b.id, BlockId(1));
        let b = e.block_of('♥').unwrap();
        assert_eq!(b.id, BlockId(3));
    }

    #[test]
    fn builtin_common_blocks_sorted_by_start() {
        let e = SymbolEngine::builtin();
        let blocks = e.common_blocks();
        assert_eq!(blocks.len(), 5);
        assert!(blocks.windows(2).all(|w| w[0].start <= w[1].start));
    }

    #[test]
    fn entries_in_block_returns_members() {
        let e = SymbolEngine::builtin();
        let texts: Vec<String> =
            e.entries_in_block(BlockId(3)).iter().map(|s| s.text.clone()).collect();
        assert!(texts.contains(&"♥".to_string()));
    }

    #[test]
    fn search_by_pinyin_keyword() {
        let e = SymbolEngine::builtin();
        let got = e.search("xiao");
        assert!(got.iter().any(|s| s.text == "😄"));
    }

    #[test]
    fn search_by_english_keyword() {
        let e = SymbolEngine::builtin();
        let got = e.search("heart");
        assert!(got.iter().any(|s| s.text == "♥"));
    }

    #[test]
    fn search_no_match_empty() {
        let e = SymbolEngine::builtin();
        assert!(e.search("zzz").is_empty());
    }

    #[test]
    fn heart_suit_name_fixed() {
        let e = SymbolEngine::builtin();
        let hearts = e.search("heart");
        assert!(hearts.iter().any(|s| s.name == "心形"));
    }

    #[test]
    fn search_matches_keyword_prefix() {
        let e = SymbolEngine::builtin();
        assert!(e.search("x").iter().any(|s| s.text == "😄")); // xiao
        assert!(e.search("sm").iter().any(|s| s.text == "😄")); // smile
        assert!(e.search("he").iter().any(|s| s.text == "♥")); // heart
        assert!(e.search("a").iter().any(|s| s.text == "あ")); // 平假名 a
    }

    #[test]
    fn search_returns_deterministic_unique() {
        let e = SymbolEngine::builtin();
        let got = e.search("x");
        let mut texts: Vec<String> = got.iter().map(|s| s.text.clone()).collect();
        texts.sort();
        let mut uniq = texts.clone();
        uniq.dedup();
        assert_eq!(texts, uniq);
    }
}
