// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::bytes::byte_successor;

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

    /// 表情**浏览**全量：`data/raw/emoji.tsv`（scripts/gen_symbols.py 从 UTS#51
    /// `emoji-test.txt` 生成，编译期嵌入，运行期无 IO）。
    ///
    /// ⚠️ 与表内 `SymbolEntry.emoji` 标志**是两条路，别合并**：
    ///
    /// - 标志（symbols.tsv 的 emoji 列）管**搜索**：单键反馈经 `search_emoji`
    ///   并入候选，受「命中按码位升序」的平局规则约束，故生成器**有意**不收
    ///   麻将/带圈等低码位块（见 gen_symbols.py 的说明）。
    /// - 本表管**浏览**：面板「表情」页要**全**（含旗帜/家庭/键帽等多码位序列，
    ///   共 3963 条完全限定条目）且按 UTS#51 官方分组序（文件序）排。
    ///
    /// 用户裁决来由：「表情中常用主流 emoji 不全」（2026-10-09）。
    ///
    /// 返回顺序 = 文件序 = 官方分组序（笑脸 → 人物 → 动物 → 食物 → … → 旗帜）。
    /// 名字列（第 2 列）暂无人消费，留在表里备查。
    pub fn builtin_emoji_texts() -> Vec<&'static str> {
        const EMOJI_TSV: &str = include_str!("../../../data/raw/emoji.tsv");
        EMOJI_TSV
            .lines()
            // ⚠️ 不滤 '#' 开头的行：tsv 是纯数据（生成器不写注释），而**键帽 #️⃣
            // 的序列正是以 '#' 开头** —— 首版按注释滤掉了它，条数少 1（测试当场抓到）。
            .filter_map(|l| l.split('\t').next())
            .filter(|t| !t.is_empty())
            .collect()
    }

    /// 表情**分类条**的分组（2026-10-12 用户裁决「中间是表情分类」）：按 `emoji.tsv`
    /// 第 3 列（中文组名）**顺序**切出 `(组名, 起始下标, 条数)`。
    ///
    /// - 顺序 = 文件序 = UTS#51 官方组序（笑脸在前、旗帜在后）；组名的中文标签由生成器
    ///   按 `EMOJI_GROUP_CN` 写进产物（翻译不在引擎层）。
    /// - `start` 与 [`Self::builtin_emoji_texts`] 的下标对齐 —— 面板点分类条就是
    ///   `grid.scrollToItem(start)`（引擎不替 UI 分页，只给锚点）。
    /// - 缺第 3 列直接 panic（同 `builtin()` 的姿态：产物损坏 = 仓库损坏，**响亮地**死；
    ///   静默退化成「一个没名字的组」才是最难查的形态）。
    pub fn builtin_emoji_groups() -> Vec<(&'static str, usize, usize)> {
        const EMOJI_TSV: &str = include_str!("../../../data/raw/emoji.tsv");
        let mut out: Vec<(&'static str, usize, usize)> = Vec::new();
        let mut idx = 0usize;
        for line in EMOJI_TSV.lines() {
            let mut cols = line.split('\t');
            let text = cols.next().unwrap_or("");
            if text.is_empty() {
                continue;
            }
            let group = cols
                .nth(1)
                .expect("emoji.tsv 缺第 3 列（组名）—— 重跑 scripts/gen_symbols.py");
            match out.last_mut() {
                Some((g, _, n)) if *g == group => *n += 1,
                _ => out.push((group, idx, 1)),
            }
            idx += 1;
        }
        out
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
        self.search_filtered(keyword, false)
    }

    /// 同 [`Self::search`]，但只保留 emoji 条目。单字符输入只并入 emoji
    /// （见 `candidates.rs`），过滤必须发生在这里：否则非 emoji 条目先被克隆再丢弃。
    pub fn search_emoji(&self, keyword: &str) -> Vec<SymbolEntry> {
        self.search_filtered(keyword, true)
    }

    /// 去重/排序在**引用**上做，最后才克隆幸存者：语义与「先全量克隆再筛」逐条相同，
    /// 但 183 条命中里只有 16 条 emoji 需要分配。
    fn search_filtered(&self, keyword: &str, only_emoji: bool) -> Vec<SymbolEntry> {
        let kw = keyword.to_lowercase();
        // 直接对 self.keywords 二分：此前先把全部关键字拷成 &str 数组只为调
        // partition_point，纯属白分配（(String, usize) 的序以 k 优先，谓词仍单调）。
        let lo = self
            .keywords
            .partition_point(|(k, _)| k.as_bytes() < kw.as_bytes());
        let hi = match byte_successor(kw.as_bytes()) {
            Some(succ) => self
                .keywords
                .partition_point(|(k, _)| k.as_bytes() < succ.as_slice()),
            None => self.keywords.len(),
        };
        let mut hits: Vec<&SymbolEntry> = self.keywords[lo..hi]
            .iter()
            .map(|(_, i)| &self.entries[*i])
            .collect();
        // 稳定排序 + 按 text 去重：等价于旧实现对克隆体做的事（保序 → 保首条）。
        hits.sort_by(|a, b| a.text.cmp(&b.text));
        hits.dedup_by(|a, b| a.text == b.text);
        hits.into_iter()
            .filter(|e| !only_emoji || e.emoji)
            .cloned()
            .collect()
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

#[cfg(test)]
#[path = "symbols_tests.rs"]
mod tests;
