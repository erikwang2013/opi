// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

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

/// Debug 只为让 from_tsv 的 Err 分支可 unwrap_err（测试与调用方错误信息）。
#[derive(Debug)]
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
        SymbolEngine {
            blocks,
            entries,
            keywords,
            block_index,
        }
    }

    /// 从 scripts 生成的两个 TSV 构造（数据在编译期以 `include_str!` 嵌入，运行期无 IO）。
    ///
    /// 格式（冻结，见 data/raw/，由 scripts/gen_symbols.py 生成并入库）：
    /// - `symbol_blocks.tsv`：`id \t start(十六进制) \t end \t name \t common(0/1)`
    /// - `symbols.tsv`：`text \t name \t keywords(逗号分隔) \t block_id \t emoji(0/1)`
    ///
    /// 空行与 `#` 开头的注释行跳过；其余任何一行坏掉（列数不对、码位非十六进制、
    /// start > end、0/1 列非法、区块 id 重复或区间相交、引用不存在的区块、空 text）
    /// 都是整体 Err 并带行号 —— 不做部分加载：数据入库时已校验过，运行期读到坏数据
    /// 说明仓库损坏，该炸得早而响。返回 Err 而非 panic：调用方可能跨 FFI 边界。
    pub fn from_tsv(blocks_tsv: &str, entries_tsv: &str) -> Result<Self, String> {
        let mut blocks: Vec<Block> = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for (i, line) in blocks_tsv.lines().enumerate() {
            let no = i + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: [&str; 5] = tsv_cols(line, no, "区块")?;
            let [id, start, end, name, common] = cols;
            let id = BlockId(
                id.parse::<u16>()
                    .map_err(|e| format!("第 {no} 行区块 id 不是 u16 {id:?}: {e}"))?,
            );
            if !ids.insert(id) {
                return Err(format!("第 {no} 行区块 id {id:?} 重复"));
            }
            let (start, end) = (parse_hex(start, "start", no)?, parse_hex(end, "end", no)?);
            if start > end {
                return Err(format!("第 {no} 行 start({start:#X}) > end({end:#X})"));
            }
            blocks.push(Block {
                id,
                start,
                end,
                name: name.to_string(),
                common: parse_flag(common, "common", no)?,
            });
        }
        // block_of 是线性扫描，重叠区块会给出不确定结果：排序后相邻即覆盖全部相交对。
        blocks.sort_by_key(|b| b.start);
        for w in blocks.windows(2) {
            if w[1].start <= w[0].end {
                return Err(format!(
                    "区块区间相交：{:#X}..{:#X} 与 {:#X}..{:#X}",
                    w[0].start, w[0].end, w[1].start, w[1].end
                ));
            }
        }
        let mut entries: Vec<SymbolEntry> = Vec::new();
        for (i, line) in entries_tsv.lines().enumerate() {
            let no = i + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: [&str; 5] = tsv_cols(line, no, "符号")?;
            let [text, name, keywords, block, emoji] = cols;
            if text.is_empty() {
                return Err(format!("第 {no} 行符号为空"));
            }
            let block = BlockId(
                block
                    .parse::<u16>()
                    .map_err(|e| format!("第 {no} 行 block_id 不是 u16 {block:?}: {e}"))?,
            );
            if !ids.contains(&block) {
                return Err(format!("第 {no} 行引用了不存在的区块 {block:?}"));
            }
            entries.push(SymbolEntry {
                text: text.to_string(),
                name: name.to_string(),
                keywords: keywords
                    // 半角/全角逗号都收：中文文档里「逗号」两义，收窄只会让数据侧白踩一次。
                    .split([',', '，'])
                    .filter(|k| !k.is_empty())
                    .map(str::to_string)
                    .collect(),
                block,
                emoji: parse_flag(emoji, "emoji", no)?,
            });
        }
        Ok(SymbolEngine::new(blocks, entries))
    }

    /// 面板默认符号集：编译期嵌入 `data/raw/` 下两份由脚本生成的表（无运行期 IO）。
    ///
    /// 数据入库时已由生成器校验，运行期读到坏数据即仓库损坏 —— 直接 panic（FFI 出口层
    /// 有 catch_unwind，且此处 panic 只可能出现在首次构造）。名字保留 `builtin`：
    /// opi-ffi / fcitx5-opi / tsf-opi 的调用点暂不动。
    pub fn builtin() -> Self {
        // 本文件由 scripts/gen_symbols.py 生成并入库；改动需同时重跑脚本。
        const BLOCKS_TSV: &str = include_str!("../../../data/raw/symbol_blocks.tsv");
        const SYMBOLS_TSV: &str = include_str!("../../../data/raw/symbols.tsv");
        SymbolEngine::from_tsv(BLOCKS_TSV, SYMBOLS_TSV)
            .expect("符号数据表损坏（生成时已校验，损坏即仓库损坏）")
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

/// TSV 行的 5 列；列数不对即 Err（带行号与来源），避免用 `get(n)` 静默吞掉缺列。
fn tsv_cols<'a>(line: &'a str, no: usize, what: &str) -> Result<[&'a str; 5], String> {
    let f: Vec<&str> = line.split('\t').collect();
    let n = f.len();
    <[&str; 5]>::try_from(f)
        .map_err(|_| format!("第 {no} 行{what}应有 5 列（制表符分隔），实际 {n} 列"))
}

/// 0/1 列。
fn parse_flag(s: &str, what: &str, no: usize) -> Result<bool, String> {
    match s {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("第 {no} 行 {what} 只能是 0/1，实际 {s:?}")),
    }
}

/// 十六进制码位（接受 `0x` 前缀）。
fn parse_hex(s: &str, what: &str, no: usize) -> Result<u32, String> {
    u32::from_str_radix(s.trim_start_matches("0x"), 16)
        .map_err(|e| format!("第 {no} 行 {what} 不是十六进制码位 {s:?}: {e}"))
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

    /// 查询路径的通用断言一律走**内联样例**，不碰 `builtin()`/数据文件：
    /// 那两份表由 scripts 生成，内容（区块编号、关键字、emoji 标记）会随生成器变，
    /// 钉死内容等于把测试绑在别人的数据上。
    fn sample() -> SymbolEngine {
        SymbolEngine::new(
            vec![
                Block {
                    id: BlockId(1),
                    start: 0x3000,
                    end: 0x303F,
                    name: "CJK 符号".into(),
                    common: true,
                },
                Block {
                    id: BlockId(3),
                    start: 0x2600,
                    end: 0x26FF,
                    name: "杂项符号".into(),
                    common: true,
                },
                Block {
                    id: BlockId(4),
                    start: 0x1F600,
                    end: 0x1F64F,
                    name: "表情符号".into(),
                    common: false,
                },
            ],
            vec![
                SymbolEntry {
                    text: "♥".into(),
                    name: "心形".into(),
                    keywords: vec!["heart".into(), "xin".into()],
                    block: BlockId(3),
                    emoji: false,
                },
                SymbolEntry {
                    text: "😄".into(),
                    name: "微笑".into(),
                    keywords: vec!["xiao".into(), "smile".into()],
                    block: BlockId(4),
                    emoji: true,
                },
            ],
        )
    }

    /// 接线守卫：`builtin()` 走编译期嵌入的数据表，表空/路径错会让面板整个空掉。
    /// 只断言「非空」这类结构性质，不钉具体内容（内容归生成器）。
    #[test]
    fn builtin_panel_is_not_empty() {
        let e = SymbolEngine::builtin();
        assert!(
            !e.common_blocks().is_empty(),
            "常用区块为空：data/raw/symbol_blocks.tsv 内容可疑"
        );
        assert!(
            e.block_of('。').is_some() || e.block_of('♥').is_some(),
            "区块区间与常见符号对不上"
        );
    }

    #[test]
    fn block_lookup_returns_owning_block() {
        let e = sample();
        assert_eq!(e.block_of('♥').unwrap().id, BlockId(3));
        assert_eq!(e.block_of('😄').unwrap().id, BlockId(4));
        assert_eq!(e.block_of('中'), None, "区间外的字符没有所属区块");
    }

    #[test]
    fn common_blocks_sorted_by_start() {
        let e = sample();
        let blocks = e.common_blocks();
        assert_eq!(blocks.len(), 2);
        assert!(blocks.windows(2).all(|w| w[0].start <= w[1].start));
    }

    #[test]
    fn entries_in_block_returns_members() {
        let e = sample();
        let texts: Vec<String> = e
            .entries_in_block(BlockId(3))
            .iter()
            .map(|s| s.text.clone())
            .collect();
        assert_eq!(texts, vec!["♥".to_string()]);
        assert!(e.entries_in_block(BlockId(99)).is_empty());
    }

    #[test]
    fn search_by_pinyin_and_english_keyword() {
        let e = sample();
        assert!(e.search("xiao").iter().any(|s| s.text == "😄"));
        assert!(e.search("heart").iter().any(|s| s.name == "心形"));
    }

    #[test]
    fn search_matches_keyword_prefix() {
        let e = sample();
        assert!(e.search("x").iter().any(|s| s.text == "😄")); // xiao
        assert!(e.search("sm").iter().any(|s| s.text == "😄")); // smile
        assert!(e.search("he").iter().any(|s| s.text == "♥")); // heart
    }

    #[test]
    fn search_no_match_empty() {
        let e = sample();
        assert!(e.search("zzz").is_empty());
    }

    /// 数据文件格式（冻结，scripts 生成）：
    /// symbol_blocks.tsv: `id \t start(hex) \t end(hex) \t name \t common(0/1)`
    /// symbols.tsv:       `text \t name \t keywords(逗号分隔) \t block_id \t emoji(0/1)`
    const BLOCKS_TSV: &str = "\
# id\tstart\tend\tname\tcommon
1\t3000\t303F\tCJK 符号\t1
2\t1F600\t1F64F\t表情符号\t1
3\t0x25A0\t25FF\t几何图形\t0
";
    const SYMBOLS_TSV: &str = "\
。\t句号\tju,period\t1\t0
😄\t微笑\txiao,smile\t2\t1
▲\t上三角\tsjx，triangle\t3\t0
∅\t空集\t\t3\t0
";

    #[test]
    fn from_tsv_parses_blocks_and_entries() {
        let e = SymbolEngine::from_tsv(BLOCKS_TSV, SYMBOLS_TSV).unwrap();
        // 区块：解析、排序、common 标记
        assert_eq!(e.common_blocks().len(), 2);
        assert_eq!(e.block_of('。').unwrap().id, BlockId(1));
        assert_eq!(e.block_of('。').unwrap().name, "CJK 符号");
        assert_eq!(e.block_of('😄').unwrap().id, BlockId(2));
        assert_eq!(e.block_of('▲').unwrap().end, 0x25FF);
        // 条目：区块归属、emoji 标记、空 keywords 列
        let emoji = e.entries_in_block(BlockId(2));
        assert_eq!(emoji.len(), 1);
        assert_eq!(emoji[0].text, "😄");
        assert!(emoji[0].emoji);
        assert_eq!(
            emoji[0].keywords,
            vec!["xiao".to_string(), "smile".to_string()]
        );
        let geom = e.entries_in_block(BlockId(3));
        assert_eq!(geom.len(), 2);
        assert!(
            geom.iter()
                .find(|s| s.text == "∅")
                .unwrap()
                .keywords
                .is_empty()
        );
        // 全角逗号也当分隔符（▲ 的 keywords 用的是 "sjx，triangle"）
        assert_eq!(
            geom.iter().find(|s| s.text == "▲").unwrap().keywords,
            vec!["sjx".to_string(), "triangle".to_string()]
        );
        // 查询路径与 new() 构造的完全一致
        assert!(e.search("xi").iter().any(|s| s.text == "😄"));
        assert!(e.search("smile").iter().any(|s| s.text == "😄"));
        assert!(e.search("tri").iter().any(|s| s.text == "▲"));
        assert_eq!(e.search("heart"), Vec::new());
    }

    #[test]
    fn from_tsv_error_message_carries_line_number() {
        let err = SymbolEngine::from_tsv("1\t3000\tZ\tCJK\t1\n", "").unwrap_err();
        assert!(err.contains("第 1 行"), "{err}");
        let err = SymbolEngine::from_tsv(BLOCKS_TSV, "。\t句号\tju\t1\tX\n").unwrap_err();
        assert!(err.contains("第 1 行"), "{err}");
    }

    #[test]
    fn from_tsv_rejects_bad_input() {
        let cases: &[(&str, &str)] = &[
            ("1\t3000\tZ\tCJK\t1\n", ""),                           // end 非十六进制
            ("1\t3000\t303F\tCJK\t2\n", ""),                        // common 非 0/1
            ("1\t3040\t303F\tCJK\t1\n", ""),                        // start > end
            ("1\t3000\t303F\tCJK\n", ""),                           // 缺列
            ("1\t3000\t303F\tCJK\t1\textra\n", ""),                 // 多列
            ("1\t3000\t303F\tCJK\t1\n1\t3030\t3040\tX\t1\n", ""),   // 区块重叠
            ("1\t3000\t303F\tCJK\t1\n1\t3000\t303F\tCJK\t1\n", ""), // 区块 ID 重复
            ("65536\t3000\t303F\tCJK\t1\n", ""),                    // id 超 u16
            (BLOCKS_TSV, "。\t句号\tju\t9\t0\n"),                   // 引用不存在的区块
            (BLOCKS_TSV, "\t句号\tju\t1\t0\n"),                     // 空 text
            (BLOCKS_TSV, "。\t句号\tju\t1\n"),                      // 缺列
        ];
        for (blocks, entries) in cases {
            assert!(
                SymbolEngine::from_tsv(blocks, entries).is_err(),
                "应拒绝：blocks={blocks:?} entries={entries:?}"
            );
        }
    }

    #[test]
    fn from_tsv_allows_empty_data() {
        // 空符号表不是错误：面板空着好过崩。
        let e = SymbolEngine::from_tsv("", "").unwrap();
        assert!(e.common_blocks().is_empty());
        assert!(e.search("x").is_empty());
        assert_eq!(e.block_of('。'), None);
    }

    #[test]
    fn search_returns_deterministic_unique() {
        // 同一关键字挂在多个条目上（两个条目都带 x 前缀关键字）也不得重复输出。
        let e = SymbolEngine::new(
            vec![Block {
                id: BlockId(1),
                start: 0x1F600,
                end: 0x1F64F,
                name: "表情".into(),
                common: true,
            }],
            vec![
                SymbolEntry {
                    text: "😄".into(),
                    name: "微笑".into(),
                    keywords: vec!["xiao".into(), "x".into()],
                    block: BlockId(1),
                    emoji: true,
                },
                SymbolEntry {
                    text: "😆".into(),
                    name: "大笑".into(),
                    keywords: vec!["xiao".into()],
                    block: BlockId(1),
                    emoji: true,
                },
            ],
        );
        let got = e.search("xiao");
        let mut texts: Vec<String> = got.iter().map(|s| s.text.clone()).collect();
        texts.sort();
        let mut uniq = texts.clone();
        uniq.dedup();
        assert_eq!(texts, uniq);
        assert_eq!(texts, vec!["😄".to_string(), "😆".to_string()]);
    }
}
