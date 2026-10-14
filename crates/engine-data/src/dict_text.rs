// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! rime 文本解析器（dict.yaml / 项目 tsv → 条目）+ 导入词库的装载（M3 2a，2026-10-14）。
//!
//! 解析规则见 M2 plan Task 5：跳过空行、`#` 注释、front-matter（文件开头的
//! `---` … `---`/`...` 整段，未闭合则按格式错报）；
//! `\t` 切分 2 列默认 freq=1000、3 列整数直接取、`NN.NN%` 按 round(percent × 1000)；
//! 非 ASCII 或 >255 字节的 pinyin、>255 字节的 word、空 word/pinyin 跳过；
//! 重复 (pinyin, word) 取最大 freq。文件首字节的 UTF-8 BOM 先剥离。
//!
//! **搬迁说明（2026-10-14，M3 计划 Task 2a）**：本解析器的唯一实现原在
//! `opi-tools/src/compiler.rs`（`parse_dict` / `parse_dict_report` / `Parsed`），
//! 随导入词库需求搬进 engine-data —— opi-tools 改成 re-export，行为一字不变，
//! 它的既有测试原样跑（那边 `parse_dict_report` 这个名字保留）。
//! 搬家的理由：import 路径要在**不打 CLI**的情况下解析同一份 tsv 文本。

use crate::format::RawEntry;
use engine_core::InMemoryDictionary;
use std::collections::HashMap;
use std::path::Path;

/// 解析结果：条目 + **被跳过的行**。
///
/// 只报一个 `kept entries` 数字是分不清「词库本来就空」与「TSV 写错了」的，
/// 而后者写出一个空壳词库、一路 exit 0，正是本项目的经典故障形态。
pub struct Parsed {
    pub entries: Vec<RawEntry>,
    /// 列数不在 2..=3 的行（1-based 行号 + 原文摘要）：这行**不像表的一行**
    /// （空格 / 逗号分隔、少列、多列）。整份文件都是这种行 = 列分隔符用错了。
    pub malformed: Vec<(usize, String)>,
    /// 列数合法、但词条按文档规则不可用的行数（空 word/pinyin、非 ASCII 或超长
    /// pinyin、超长 word、第三列词频解析失败）。这类跳过是**正常**的 ——
    /// rime 词库里带声调的拼音行就靠它滤掉 —— 所以只统计，不报错。
    pub unusable: usize,
}

/// 解析 rime dict.yaml / 项目 tsv 文本为条目。见 plan Task 5 规则。
pub fn parse(text: &str) -> Vec<RawEntry> {
    parse_report(text).entries
}

/// 同 [`parse`]，但附带被跳过行的统计（CLI 据此区分「空词库」与「格式错」）。
pub fn parse_report(text: &str) -> Parsed {
    // BOM 只可能出现在文件首字节。U+FEFF 是 `Cf` 而不是空白，`trim()` 去不掉它，
    // 留着就让首行的 word 变成 "\u{FEFF}好"：条目数正常、校验和正常、exit 0，
    // 而终端里肉眼看不出前缀 —— 用户永远打不出「好」这个词。
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut best: HashMap<(String, String), u32> = HashMap::new();
    let mut malformed: Vec<(usize, String)> = Vec::new();
    let mut unusable = 0usize;
    // front-matter（rime 的 `---` … `...` 头）整段跳过、不进任何统计：它是本文件开头与
    // `skips_comments_blanks_and_front_matter` 都写明要跳过的输入，不该被报成「列数不符」。
    // 只在**首个条目之前**开：文件中间若也认 `---`，一份 TSV 里一个手滑的 `---` 就会把它
    // 后面的词全部静默吃掉 —— 那正是本模块在防的故障。
    let mut front_matter: Option<Vec<(usize, String)>> = None;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if let Some(mut pending) = front_matter.take() {
            if line == "---" || line == "..." {
                // 闭合：`pending` 直接丢掉 —— 区间内的行不报、不计数、不产条目。
            } else {
                // 先记账、不落地：没写收尾标记的 `---` 不是 front-matter 而是格式错，
                // 否则一个没闭合的头会让整份文件静默消失（0 条产出、exit 0）。只记
                // 「没有 front-matter 时也会被报的行」，空行/`#`/`-` 到哪儿都不算。
                if !line.is_empty() && !line.starts_with('#') && !line.starts_with('-') {
                    pending.push((i + 1, line.chars().take(40).collect()));
                }
                front_matter = Some(pending);
            }
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "---" && best.is_empty() {
            front_matter = Some(Vec::new());
            continue;
        }
        if line.starts_with('-') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if !(2..=3).contains(&cols.len()) {
            // 存摘要而不是全文：坏行可能是一整段没有 TAB 的文本。
            malformed.push((i + 1, line.chars().take(40).collect()));
            continue;
        }
        let word = cols[0].trim();
        let pinyin = cols[1].trim().to_lowercase();
        // word 与 pinyin 都得守 ≤255 字节：`serialize` 对两者都写死了这条不变式
        // 并 `expect`，只守一边就等于那一边的 panic（中文 3 字节/字 → 85 字短语即可触发）。
        if word.is_empty()
            || pinyin.is_empty()
            || !pinyin.is_ascii()
            || pinyin.len() > u8::MAX as usize
            || word.len() > u8::MAX as usize
        {
            unusable += 1;
            continue;
        }
        let freq = match cols.get(2).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => 1000,
            Some(s) => match parse_freq(s) {
                Some(f) => f,
                None => {
                    unusable += 1;
                    continue;
                }
            },
        };
        let key = (pinyin.clone(), word.to_string());
        let prev = best.get(&key).copied().unwrap_or(0);
        best.insert(key, freq.max(prev));
    }
    // 走到文件末尾还开着 = 这个头没闭合：按格式错落地，保住上面那条不静默的保证。
    if let Some(pending) = front_matter {
        malformed.extend(pending);
    }
    let mut entries: Vec<RawEntry> = best
        .into_iter()
        .map(|((pinyin, word), freq)| RawEntry { pinyin, word, freq })
        .collect();
    entries.sort_by(|a, b| a.pinyin.as_bytes().cmp(b.pinyin.as_bytes()));
    Parsed {
        entries,
        malformed,
        unusable,
    }
}

/// 3 列词频：整数直接取；`NN.NN%` 按 round(percent × 1000)。
fn parse_freq(s: &str) -> Option<u32> {
    if let Ok(n) = s.parse::<u32>() {
        return Some(n);
    }
    let pct = s.strip_suffix('%')?;
    Some((pct.parse::<f64>().ok()? * 1000.0).round() as u32)
}

/// 读一份导入词库文本（tsv / dict.yaml）并装成 [`InMemoryDictionary`]（M3 2a）。
///
/// 返回 `(库, 跳过的坏行数)` —— 坏行 = [`Parsed::malformed`]（列数不符，多半是
/// 列分隔符用错了）。`unusable` 那类**不计入**：rime 词库里带声调的拼音行是文档
/// 写明要跳过的输入，报成「坏行」只会让统计变噪音。
///
/// 拒绝条件与 `opi-tools` 的 `compile_file` 逐字同源：**一条都没解析出来、
/// 却有列数不合法的行** ⇒ `Err`。那说明分隔符用错（空格/逗号），不是「词库本来
/// 就是空的」—— 导入一个空壳层、界面照常显示成功，正是本项目在防的故障形态。
/// （空文件 = 条目 0 且没有坏行 ⇒ `Ok`：那是「空词库」，不是「写错了」。）
///
/// 本函数只做「文件 → 字符串 → 内存库」这一层的收发；写盘、目录扫描、引擎状态
/// 都不在这里（engine-data 不碰引擎）。
pub fn load_extra_dict(path: &Path) -> Result<(InMemoryDictionary, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "读导入词库 {} 失败（需要 UTF-8 编码的 tsv 文本）：{e}",
            path.display()
        )
    })?;
    let report = parse_report(&text);
    if report.entries.is_empty() && !report.malformed.is_empty() {
        let (line, snippet) = &report.malformed[0];
        return Err(format!(
            "拒绝导入空词库：{} 有 {} 行不是 2/3 列（首个在第 {line} 行：{snippet:?}），\
             一条都没解析出来 —— 导入词库要的是「TAB 分隔的 2 或 3 列」；\
             分隔符用错（空格/逗号）与列数不对（如 5 列）都会落到这里",
            path.display(),
            report.malformed.len()
        ));
    }
    let mut dict = InMemoryDictionary::new();
    for e in report.entries {
        dict.insert(&e.pinyin, &e.word, e.freq);
    }
    Ok((dict, report.malformed.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::Dictionary; // InMemoryDictionary 的 query/len 来自这个 trait

    // 以下四条是从 opi-tools/src/compiler.rs 的同名测试里挑的**子集**
    // （2026-10-14 搬迁时的门禁：那边原测试一条不删，这里保一份独立副本，
    // 防「re-export 断链」被两边的空白同时盖过）。

    #[test]
    fn two_column_defaults_to_1000() {
        let entries = parse("好\thao\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].freq, 1000);
        assert_eq!(entries[0].pinyin, "hao");
        assert_eq!(entries[0].word, "好");
    }

    #[test]
    fn bom_is_stripped_from_the_first_line() {
        let entries = parse("\u{FEFF}好\thao\n号\thao\n");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.word == "好" || e.word == "号"));
    }

    #[test]
    fn skips_comments_blanks_and_front_matter() {
        let text = "# 注释\n\n---\nname: luna_pinyin\nuse_preset_vocabulary: true\n...\n好\thao\n";
        let entries = parse(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].word, "好");
    }

    #[test]
    fn malformed_lines_are_column_errors_not_field_skips() {
        let spaces = parse_report("好 hao\n号 hao\n");
        assert!(spaces.entries.is_empty());
        assert_eq!(
            spaces.malformed,
            vec![(1, "好 hao".into()), (2, "号 hao".into())]
        );
        assert_eq!(spaces.unusable, 0);

        let fields = parse_report("好\thāo\n坏\thao\tabc\n");
        assert!(fields.entries.is_empty());
        assert!(fields.malformed.is_empty(), "字段问题不算列数问题");
        assert_eq!(fields.unusable, 2);

        // 未闭合的 front-matter 必须落地成格式错（否则一个手滑的 `---` 会把整份文件静默吃完）。
        let unterminated = parse_report("---\nname: luna\n好 hao\n");
        assert_eq!(
            unterminated.malformed,
            vec![(2, "name: luna".into()), (3, "好 hao".into())]
        );
    }

    // ---- 装载（M3 2a）：文件 IO 只在 engine-data 这一层 ----

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("opi-dict-text-{}-{name}", std::process::id()))
    }

    #[test]
    fn load_extra_dict_reads_tsv_and_counts_bad_rows() {
        let p = tmp("ok.tsv");
        std::fs::write(
            &p,
            "好\thao\t5000\n这行没有tab\n号\thao\t1000\n丁\tding\t99.93%\n坏\thao\tabc\n",
        )
        .unwrap();
        let (dict, bad) = load_extra_dict(&p).unwrap();
        // 坏行 = 列数不符的那一行；`abc` 词频是字段不可用（unusable），不计入。
        assert_eq!(bad, 1);
        assert_eq!(dict.len(), 3);
        let hits = dict.query("hao", 8);
        assert_eq!(hits[0].word, "好");
        assert_eq!(hits[0].freq, 5000);
        assert_eq!(hits[1].word, "号");
        assert_eq!(dict.query("ding", 8)[0].freq, 99930, "百分比词频照常生效");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_extra_dict_rejects_column_separator_mistake() {
        let p = tmp("bad.tsv");
        std::fs::write(&p, "好 hao\n号 hao\n").unwrap();
        // 不用 `unwrap_err`：`InMemoryDictionary` 没有 Debug（`Ok` 分支打不出来）。
        let err = match load_extra_dict(&p) {
            Err(e) => e,
            Ok(_) => panic!("列分隔符用错必须拒绝（不能静默导入空壳层）"),
        };
        assert!(err.contains("拒绝导入空词库"), "错误信息要指认成因：{err}");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_extra_dict_missing_file_is_err_not_panic() {
        let p = tmp("nope.tsv");
        assert!(load_extra_dict(&p).is_err());
    }
}
