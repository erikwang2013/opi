// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号浏览（「符号大全」）：候选面板式的两级浏览状态。
//!
//! 一级 = 类目列表（显示 `序号.名称`），二级 = 该类目的符号。选中类目**下钻**、
//! 选中符号**上屏并回一级**（不是退出）。分页不入本模块：`candidates(limit)`
//! 一次性给全量，宿主按既有 `PAGE_SIZE` 切（`.skip().take()`），翻页键走宿主的
//! 既有 paging 接线 —— 本模块只出语义（与 `Engine::select_from` 同一条分工）。
//!
//! 数据由宿主把 TSV 串传进来（engine-core 禁 IO，见仓库不变量）。格式与
//! `android/app/src/main/assets/symbol_categories.tsv` 一致，解析语义对齐 Kotlin
//! 侧 `SymbolCategories.parse`：**坏行跳过而不抛** —— 资产是生成物，但用户设备上的
//! 文件可能被替换/截断，页面该降级成少几个类目而不是崩 IME。
//!
//! 与 `Mode::Symbol`（符号**搜索**模式，缓冲 = 关键字）的关系：那是模式，这是
//! 叠加在任意模式上的**浏览态**（入口/出口见 `Engine::enter_symbol_browse`），
//! 两者互不替代 —— 别把浏览并进那个模式的缓冲语义里。

use crate::candidates::{Candidate, CandidateKind};

/// 一级的预编辑标题（面板显示的「正在浏览什么」）。
pub const BROWSE_TITLE: &str = "符号";

/// 一个类目：序号（1 起，与展示的「1.常用」一致）、名称、符号表（表序 = 生成序）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolCategory {
    pub index: u32,
    pub label: String,
    pub symbols: Vec<String>,
}

/// 解析 TSV `序号 \t 名称 \t 符号 符号 …`（符号按**空格分隔的 token**，不按码点切
/// —— 组合序列如 `o͘`、`╮(╯▽╰)╭` 必须整块；与 Kotlin 侧逐条同义）。
///
/// 返回 `(类目, 跳过的坏行数)`：坏行 = 非空但缺列 / 序号非数字 / 名称空 / 无符号。
/// 空行跳过且**不计数**（Kotlin 侧 `isBlank → continue` 同义）；计数只为宿主日志
/// （「别让解析失败静默」）—— Kotlin 侧不计数，两边的**加载结果**逐条相同。
pub fn parse_categories(tsv: &str) -> (Vec<SymbolCategory>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for line in tsv.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // 列数 < 3 → 坏行；多余列忽略（Kotlin 侧同样只取前三列）。
        let mut cols = line.split('\t');
        let (Some(index), Some(label), Some(symbols)) = (cols.next(), cols.next(), cols.next())
        else {
            skipped += 1;
            continue;
        };
        let symbols: Vec<String> = symbols
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        match index.parse::<u32>() {
            Ok(index) if !label.is_empty() && !symbols.is_empty() => out.push(SymbolCategory {
                index,
                label: label.to_string(),
                symbols,
            }),
            _ => skipped += 1,
        }
    }
    (out, skipped)
}

/// 一次选中的结论。宿主只关心 `Commit` 的文本；下钻与未命中都不上屏。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// 一级选中类目：进入二级，无提交文本。
    Drill,
    /// 二级选中符号：返回上屏文本（已回一级）。
    Commit(String),
    /// 越界（或空表）：状态不变。
    Miss,
}

/// 符号浏览的全部状态：**数据**（装载进来的类目）与**浏览态**（active + 游标）
/// 是两件事，别并成一个 `Option` —— 并了的话「装载完还没进浏览」与「没数据」
/// 分不开：前者会被 input_key 的「未处理键先退出浏览」误当成正在浏览而清掉，
/// 表现为**装好数据后打第一个键入口就永久禁用**（有测试钉住：
/// tests/symbol_browse.rs 的 `loaded_data_survives_typing_until_enter`）。
///
/// 预编辑串**存字段**（不是每次算）：`Engine::buffer` 返回 `&str`，面板/候选窗按
/// 「buffer 非空」决定显示（fcitx5 的 `refreshingPanelFor` 与 tsf 的 sink 都是这个
/// 判据）—— 浏览态必须有一个非空标题串，否则面板根本不显示。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SymbolBrowse {
    categories: Vec<SymbolCategory>,
    /// `Some(i)` = 二级（第 i 个类目）；`None` = 一级。
    selected: Option<usize>,
    /// 是否正在浏览（`enter`/`exit` 翻）。**非 active 时 `select`/`candidates`
    /// 一律空答** —— 引擎侧的读侧分派都以它为准（见 `Engine::buffer` 一族）。
    active: bool,
    preedit: String,
}

impl SymbolBrowse {
    /// 由已解析的类目建浏览态（**未激活**：入口在 `enter`，空表由
    /// [`crate::Engine::load_symbol_categories`] 挡在门外 —— 空数据集入口禁用）。
    pub fn new(categories: Vec<SymbolCategory>) -> Self {
        let mut b = SymbolBrowse::default();
        b.load(categories);
        b
    }

    /// 装载/替换数据集：退回一级、退出浏览（旧下标对新数据无意义；数据在，
    /// active 不在 —— 重新进入由 `enter` 决定，见字段注释）。
    pub fn load(&mut self, categories: Vec<SymbolCategory>) {
        self.categories = categories;
        self.selected = None;
        self.active = false;
        self.refresh_preedit();
    }

    /// 数据集是否为空（入口闸门的判据：空 ⇒ 入口禁用）。
    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// 进入浏览：无数据 → false（入口禁用），状态不动。**已在浏览态 = no-op**
    /// （保持当前层级：宿主热键是 toggle，重复按不该把用户从二级弹回一级）；
    /// 退出/装载都会复位游标，故「退出后再进」一定从一级开始。
    pub fn enter(&mut self) -> bool {
        if self.categories.is_empty() {
            return false;
        }
        if !self.active {
            self.selected = None;
            self.active = true;
            self.refresh_preedit();
        }
        true
    }

    /// 退出浏览（幂等；**数据留着** —— 再次 `enter` 即可，不必重装载）。
    pub fn exit(&mut self) {
        self.active = false;
        self.selected = None;
        self.refresh_preedit();
    }

    /// 二级类目下标；`None` = 一级（测试探针与宿主可能的 UI 提示）。
    pub fn level2(&self) -> Option<usize> {
        self.selected
    }

    /// 预编辑串（浏览态的「缓冲」）：一级 = [`BROWSE_TITLE`]，二级 = `符号·名称`；
    /// 非浏览态 = 空串（此时 `Engine::buffer` 给的是真缓冲，轮不到它）。
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    fn refresh_preedit(&mut self) {
        self.preedit = if !self.active {
            String::new()
        } else {
            match self.selected {
                Some(i) => format!("{BROWSE_TITLE}·{}", self.categories[i].label),
                None => BROWSE_TITLE.to_string(),
            }
        };
    }

    /// 选中当前层第 `index`（全局下标，与宿主 `page * PAGE_SIZE + idx` 同义）。
    /// 非浏览态一律 `Miss`（引擎门面之外再堵一道：类目展示串没有任何返回路径）。
    pub fn select(&mut self, index: usize) -> Selection {
        if !self.active {
            return Selection::Miss;
        }
        match self.selected {
            None => match self.categories.get(index) {
                Some(_) => {
                    self.selected = Some(index);
                    self.refresh_preedit();
                    Selection::Drill
                }
                None => Selection::Miss,
            },
            Some(cat) => match self.categories[cat].symbols.get(index).cloned() {
                Some(text) => {
                    self.selected = None;
                    self.refresh_preedit();
                    Selection::Commit(text)
                }
                None => Selection::Miss,
            },
        }
    }

    /// 当前层**全量**候选（分页归宿主：`.skip(page * PAGE_SIZE).take(PAGE_SIZE)`）；
    /// 非浏览态 = 空表。类目条款式为 `序号.名称`（序号取数据列，不是行号）；
    /// `kind` 一律 `Symbol` —— 类目不可上屏，宿主按浏览态路由，不存在
    /// 「把它当符号提交」的路径。
    pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
        let mut out = Vec::new();
        if !self.active {
            return out;
        }
        match self.selected {
            None => out.extend(self.categories.iter().map(|c| Candidate {
                text: format!("{}.{}", c.index, c.label),
                kind: CandidateKind::Symbol,
                score: 0,
            })),
            Some(i) => out.extend(self.categories[i].symbols.iter().map(|s| Candidate {
                text: s.clone(),
                kind: CandidateKind::Symbol,
                score: 0,
            })),
        }
        out.truncate(limit);
        out
    }
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 symbols_tests.rs 同惯例。
#[cfg(test)]
#[path = "symbol_browse_tests.rs"]
mod tests;
