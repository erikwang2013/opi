// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 双拼（2026-10-13 用户裁决「搜狗同款全套」）。**解码前置**：键序 → 全拼串，
//! 下游复用既有的全拼候选管线（与 9/14 键同一条纪律：匹配在 engine-core，
//! 四端只负责把键送进来）。
//!
//! 键位表是生成物（[`crate::shuangpin_tables`]，源与 pin 见那个文件的头注）。
//! 本模块只实现**解码语义**，逐条镜像 libime 的编码空间（`shuangpinprofile.cpp`）：
//!
//! 一个音节的编码 = `声母键 + 韵母键`：
//! - 声母键：zh/ch/sh 查方案的 `initials` 表；单字母声母 = 字母自身；**零声母**
//!   （a/e/o 开头的音节）= 零声母标记键（方案 `zero` 里非 `*` 的字符）；
//! - 韵母键：查方案的 `finals` 表（**一个键可映射多个韵母** —— 自然码 `r` = uan/er、
//!   `d` = iang/uang、微软 `v` = ui/ve/v…解码时都要枚举）；
//! - `zero` 含 `*` ⇒ **裸韵母键**也是合法零声母音节（小鹤全是裸形；自然码 `oa` 与
//!   `a` 都行）。
//!
//! ⚠️ 单字符韵母（a/e/i/o/u/v）由**生成器**按 (键,韵母) 对补齐（libime 的
//! `singleCharFinal` 循环），本模块不另做兜底 —— 兜底两处写必漂。
//!
//! 解码 = 对键序做 DFS：每步吃 1 键（裸零声母）或 2 键（声母+韵母 / 标记+韵母），
//! 每个段都过 [`crate::pinyin::is_syllable`] 合法性；产物是按**确定性顺序**排的全拼
//! 串（含音节分隔 `'` 的展示形与无分隔的查询形）。条数封顶见 [`MAX_PARSES`]。

use crate::pinyin::{is_syllable, segment};
use crate::shuangpin_tables::{SHUANGPIN_TABLES, ShuangpinTable};

/// 一次解码最多给出几条全拼串（DFS 的分支数封顶）。双拼的歧义主要来自
/// 「一键多韵母」（自然码 `s` = ong/iong 这类），真实输入的解析数是个位数；
/// 封顶防病态键串把分支炸开（这条路在**每次击键**上）。
pub const MAX_PARSES: usize = 8;

/// 方案套数（设置面的下拉长度；FFI 的索引口径）。
pub fn layout_count() -> usize {
    SHUANGPIN_TABLES.len()
}

/// 方案显示名（FFI/设置面用它做下拉与校验）。
pub fn layout_name(index: usize) -> Option<&'static str> {
    SHUANGPIN_TABLES.get(index).map(|t| t.name)
}

fn table(index: usize) -> &'static ShuangpinTable {
    // 越界钳到 0（自然码）：FFI 侧另有钳制，这里是双保险 —— 与 fuzzy 掩码同纪律。
    &SHUANGPIN_TABLES[index.min(SHUANGPIN_TABLES.len() - 1)]
}

/// 单字母声母表（b…z 的辅音；元音 a/e/i/o/u 与 v 不是声母 —— v 是韵母键/zh 键）。
/// 用常量表而不是 `Box::leak` 拼串：这条路在每次击键上，起草版的 leak 会被门禁
/// 之外的人一眼看出来（也曾真写出来过）。
static CONSONANT_INITIALS: &[(char, &str)] = &[
    ('b', "b"), ('c', "c"), ('d', "d"), ('f', "f"), ('g', "g"), ('h', "h"), ('j', "j"),
    ('k', "k"), ('l', "l"), ('m', "m"), ('n', "n"), ('p', "p"), ('q', "q"), ('r', "r"),
    ('s', "s"), ('t', "t"), ('w', "w"), ('x', "x"), ('y', "y"), ('z', "z"),
];

/// 键 k 能否当声母键（含 zh/ch/sh 与单字母声母；零声母标记不算）。
fn initial_of<'a>(t: &'a ShuangpinTable, k: char) -> Option<&'a str> {
    for (key, ini) in t.initials {
        if *key == k {
            return Some(ini);
        }
    }
    CONSONANT_INITIALS
        .iter()
        .find(|(c, _)| *c == k)
        .map(|(_, s)| *s)
}

/// 键 k 对应的全部韵母（一键多韵母都要枚举）。
fn finals_of<'a>(t: &'a ShuangpinTable, k: char) -> Vec<&'a str> {
    t.finals
        .iter()
        .filter(|(key, _)| *key == k)
        .map(|(_, f)| *f)
        .collect()
}

/// 零声母标记键（`zero` 里非 `*` 的字符，如 'o'）。
fn zero_markers(t: &ShuangpinTable) -> Vec<char> {
    t.zero.chars().filter(|c| *c != '*').collect()
}

fn accepts_bare(t: &ShuangpinTable) -> bool {
    t.zero.contains('*')
}

/// 键序 → 全拼串候选（**无分隔**，供查询；展示形的分隔由调用方按 [`segment`] 重切）。
/// 顺序确定（DFS 最左优先、短段优先）；去重；封顶 [`MAX_PARSES`]。
pub fn decode_keys(buffer: &str, layout: usize) -> Vec<String> {
    let t = table(layout);
    let keys: Vec<char> = buffer.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    dfs(t, &keys, 0, &mut cur, &mut out);
    out
}

fn dfs(t: &ShuangpinTable, keys: &[char], i: usize, cur: &mut String, out: &mut Vec<String>) {
    if out.len() >= MAX_PARSES {
        return;
    }
    if i == keys.len() {
        if !cur.is_empty() && !out.contains(cur) {
            out.push(cur.clone());
        }
        return;
    }
    let k1 = keys[i];
    // 1 键段：裸零声母（仅当方案接受裸形；韵母键本身可多解）。
    if accepts_bare(t) && finals_of(t, k1).iter().any(|f| is_syllable(f)) {
        for f in finals_of(t, k1) {
            if !is_syllable(f) {
                continue;
            }
            let n = cur.len();
            cur.push_str(f);
            dfs(t, keys, i + 1, cur, out);
            cur.truncate(n);
        }
    }
    let Some(k2) = keys.get(i + 1).copied() else {
        return;
    };
    // 2 键段：零声母标记 + 韵母。
    if zero_markers(t).contains(&k1) {
        for f in finals_of(t, k2) {
            if !is_syllable(f) {
                continue;
            }
            let n = cur.len();
            cur.push_str(f);
            dfs(t, keys, i + 2, cur, out);
            cur.truncate(n);
        }
    }
    // 2 键段：声母 + 韵母（声母含 zh/ch/sh 与单字母）。
    if let Some(ini) = initial_of(t, k1) {
        for f in finals_of(t, k2) {
            let full = format!("{ini}{f}");
            if !is_syllable(&full) {
                continue;
            }
            let n = cur.len();
            cur.push_str(&full);
            dfs(t, keys, i + 2, cur, out);
            cur.truncate(n);
        }
    }
}

/// 展示形（音节用 `'` 分隔；非音节段原样保留：打一半的输入也要看得见）。
/// 与 `engine_readings` 的九键读数同为「显示用」出口 —— 绝不参与行为判定。
pub fn display(pinyin: &str) -> String {
    segment(pinyin).join("'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(name: &str) -> usize {
        SHUANGPIN_TABLES.iter().position(|t| t.name == name).unwrap()
    }

    /// 六套都在、名字与顺序是指定集合。
    #[test]
    fn six_layouts_in_order() {
        let names: Vec<&str> = SHUANGPIN_TABLES.iter().map(|t| t.name).collect();
        assert_eq!(names, ["自然码", "微软", "小鹤", "智能ABC", "紫光", "拼音加加"]);
    }

    /// 每套的**代表键**逐条钉（来自 libime 的 C 表，抽查会随生成物一起变红）。
    #[test]
    fn representative_keys_per_layout() {
        // nihao 的编码：ni = n+i、hao = h+?（各方案 ao 的键不同）。
        assert!(decode_keys("nihk", l("自然码")).contains(&"nihao".to_string()));
        assert!(decode_keys("nihk", l("微软")).contains(&"nihao".to_string()));
        assert!(decode_keys("nihc", l("小鹤")).contains(&"nihao".to_string()));
        assert!(decode_keys("nihk", l("智能ABC")).contains(&"nihao".to_string()));
        assert!(decode_keys("nihq", l("紫光")).contains(&"nihao".to_string()));
        assert!(decode_keys("nihd", l("拼音加加")).contains(&"nihao".to_string()));
        // zh/ch/sh：自然码 zhong = v+s（zh=v, ong=s）。
        assert!(decode_keys("vs", l("自然码")).contains(&"zhong".to_string()));
        // 智能ABC：zh=a、sh=v、ch=e。
        assert!(decode_keys("as", l("智能ABC")).contains(&"zhong".to_string()));
    }

    /// 零声母：自然码 o+j = an，且 `*` 让裸 `j` 也行；微软不认裸形。
    #[test]
    fn zero_initial_rules() {
        assert!(decode_keys("oj", l("自然码")).contains(&"an".to_string()));
        assert!(decode_keys("j", l("自然码")).contains(&"an".to_string()), "o* 接受裸形");
        assert!(decode_keys("oj", l("微软")).contains(&"an".to_string()));
        assert!(
            !decode_keys("j", l("微软")).contains(&"an".to_string()),
            "微软 zero=o（无 *）不许裸形"
        );
        // 小鹤：zero = "*"，只有裸形。
        assert!(decode_keys("j", l("小鹤")).contains(&"an".to_string()));
        assert!(!decode_keys("oj", l("小鹤")).contains(&"an".to_string()));
    }

    /// 一键多韵母要枚举：自然码 s = ong/iong、r = uan/er。
    #[test]
    fn multi_final_keys_are_enumerated() {
        assert!(decode_keys("ds", l("自然码")).contains(&"dong".to_string()));
        assert!(decode_keys("ds", l("自然码")).contains(&"diong".to_string()) == false, "diong 非法（被合法性过滤）");
        assert!(decode_keys("dr", l("自然码")).contains(&"duan".to_string()));
        assert!(decode_keys("dr", l("自然码")).contains(&"der".to_string()) == false, "der 非法");
    }

    /// 多音节：候选里出现完整连写（同一次 DFS 跨段拼接）。
    #[test]
    fn multi_syllable_concatenates() {
        // 自然码 nihao = n+i h+k。
        assert!(decode_keys("nihk", l("自然码")).contains(&"nihao".to_string()));
        // 打一半（单键段）也要有解：n（无法单独成音节）⇒ 空 —— 不会造出垃圾。
        assert!(decode_keys("n", l("自然码")).is_empty());
    }

    /// 展示形带 `'` 分隔。
    #[test]
    fn display_separates_syllables() {
        assert_eq!(display("nihao"), "ni'hao");
        assert_eq!(display("zhongguo"), "zhong'guo");
    }
}
