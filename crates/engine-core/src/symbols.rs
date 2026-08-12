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
    keyword_index: std::collections::HashMap<String, Vec<usize>>,
    block_index: std::collections::HashMap<BlockId, Vec<usize>>,
}

impl SymbolEngine {
    /// blocks 与 entries 由调用方提供（M2 起来自数据文件），此处构建索引。
    pub fn new(mut blocks: Vec<Block>, entries: Vec<SymbolEntry>) -> Self {
        blocks.sort_by_key(|b| b.start);
        let mut keyword_index: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut block_index: std::collections::HashMap<BlockId, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            block_index.entry(e.block).or_default().push(i);
            for kw in &e.keywords {
                keyword_index.entry(kw.to_lowercase()).or_default().push(i);
            }
        }
        SymbolEngine { blocks, entries, keyword_index, block_index }
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
            SymbolEntry { text: "♥".into(), name: "黑桃心".into(), keywords: vec!["heart".into(), "ai".into(), "xin".into()], block: BlockId(3), emoji: false },
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

    /// 关键字精确搜索（拼音或英文小写）。
    pub fn search(&self, keyword: &str) -> Vec<SymbolEntry> {
        self.keyword_index
            .get(&keyword.to_lowercase())
            .map(|idx| idx.iter().map(|&i| self.entries[i].clone()).collect())
            .unwrap_or_default()
    }
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
}
