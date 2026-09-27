// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! rime 文本解析器（dict.yaml / 项目 tsv → .opid 条目）。
//!
//! 解析规则见 M2 plan Task 5：跳过空行、`#` 注释、front-matter（文件开头的
//! `---` … `---`/`...` 整段，未闭合则按格式错报）；
//! `\t` 切分 2 列默认 freq=1000、3 列整数直接取、`NN.NN%` 按 round(percent × 1000)；
//! 非 ASCII 或 >255 字节的 pinyin、>255 字节的 word、空 word/pinyin 跳过；
//! 重复 (pinyin, word) 取最大 freq。文件首字节的 UTF-8 BOM 先剥离。

use engine_data::format::{OpDict, RawEntry, serialize};
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
pub fn parse_dict(text: &str) -> Vec<RawEntry> {
    parse_dict_report(text).entries
}

/// 同 [`parse_dict`]，但附带被跳过行的统计（CLI 据此区分「空词库」与「格式错」）。
pub fn parse_dict_report(text: &str) -> Parsed {
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

/// 排序 + 计算 pinyin_total，产出可序列化的 OpDict。
pub fn compile(entries: Vec<RawEntry>) -> OpDict {
    let mut entries = entries;
    entries.sort_by(|a, b| {
        a.pinyin
            .as_bytes()
            .cmp(b.pinyin.as_bytes())
            .then_with(|| a.word.as_bytes().cmp(b.word.as_bytes()))
    });
    entries.dedup_by(|a, b| a.pinyin == b.pinyin && a.word == b.word);
    let pinyin_total = entries.iter().map(|e| e.pinyin.len()).sum();
    OpDict {
        entries,
        pinyin_total,
    }
}

/// 把解析结果写成 `.opid`。
///
/// 两种拒绝，都以 `Err` 返回、且**不碰输出文件**：
/// 1. `input` 与 `output` 指向同一个文件 —— 无条件 `fs::write` 会把源 TSV
///    原地换成二进制，源数据不可逆地没了（往往是用户唯一的那份词库）；
/// 2. 一条都没解析出来、却有列数不合法的行 —— 那说明列分隔符用错了
///    （空格 / 逗号），不是「词库本来就是空的」，不能 exit 0 悄悄写出空壳。
pub fn compile_file(report: Parsed, input: &Path, output: &Path) -> Result<(), String> {
    if same_file(input, output) {
        return Err(format!(
            "拒绝编译：输入与输出是同一个文件（{}）—— 那会把源 TSV 覆盖成二进制、\
             且不可逆，请换一个输出路径",
            output.display()
        ));
    }
    if report.entries.is_empty() && !report.malformed.is_empty() {
        let (line, snippet) = &report.malformed[0];
        return Err(format!(
            "拒绝写出空词库：{} 有 {} 行不是 2/3 列（首个在第 {line} 行：{snippet:?}），\
             一条都没解析出来 —— 本工具要的是「TAB 分隔的 2 或 3 列」；\
             分隔符用错（空格/逗号）与列数不对（如 5 列）都会落到这里",
            input.display(),
            report.malformed.len()
        ));
    }
    let bytes = serialize(&compile(report.entries));
    std::fs::write(output, &bytes).map_err(|e| format!("write {}: {e}", output.display()))?;
    Ok(())
}

/// 两个路径是否指向同一个文件。`(dev, ino)` 相等即同一文件 —— 覆盖符号链接
/// （`metadata` 跟进链接）与硬链接两条路，后者 `canonicalize` 是看不出来的。
#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

/// 非 unix 退化：路径规范化后比对（覆盖符号链接与相对/绝对写法，不覆盖硬链接）。
#[cfg(not(unix))]
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_column_defaults_to_1000() {
        let entries = parse_dict("好\thao\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].freq, 1000);
        assert_eq!(entries[0].pinyin, "hao");
        assert_eq!(entries[0].word, "好");
    }

    #[test]
    fn three_column_percent_rounds() {
        let entries = parse_dict("丁\tding\t99.93%\n");
        assert_eq!(entries[0].freq, 99930);
    }

    #[test]
    fn three_column_integer_taken_directly() {
        let entries = parse_dict("我\two\t100000\n");
        assert_eq!(entries[0].freq, 100000);
    }

    #[test]
    fn skips_comments_blanks_and_front_matter() {
        let text = "# 注释\n\n---\nname: luna_pinyin\nuse_preset_vocabulary: true\n...\n好\thao\n";
        let entries = parse_dict(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].word, "好");
    }

    #[test]
    fn duplicate_takes_max_freq() {
        let text = "好\thao\t500\n好\thao\t3000\n";
        let entries = parse_dict(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].freq, 3000);
    }

    #[test]
    fn non_ascii_pinyin_skipped() {
        let entries = parse_dict("好\thāo\n");
        assert!(entries.is_empty());
    }

    #[test]
    fn empty_word_or_pinyin_skipped() {
        let entries = parse_dict("\thao\n好\t\n");
        assert!(entries.is_empty());
    }

    #[test]
    fn bom_is_stripped_from_the_first_line() {
        let entries = parse_dict("\u{FEFF}好\thao\n号\thao\n");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.word == "好" || e.word == "号"));
    }

    #[test]
    fn overlong_word_skipped_like_overlong_pinyin() {
        // 两侧都是 255 字节为界：serialize 的 `expect` 就在这条线上。
        let ok = parse_dict(&format!("{}\thao\n", "x".repeat(255)));
        assert_eq!(ok.len(), 1);
        let too_long = parse_dict_report(&format!("{}\thao\n", "x".repeat(256)));
        assert!(too_long.entries.is_empty());
        assert_eq!((too_long.unusable, too_long.malformed.len()), (1, 0));
    }

    /// 「该报的」与「正常跳过」的分界：列数不符 = 这行不像表的一行（多半是列分隔符
    /// 用错了，整份文件都这样就是全错）；字段不合规 = 行是表的一行，只是词条不可用
    /// （rime 的带声调拼音行、空字段），属文档写明要跳过的。
    #[test]
    fn malformed_lines_are_column_errors_not_field_skips() {
        let spaces = parse_dict_report("好 hao\n号 hao\n");
        assert!(spaces.entries.is_empty());
        assert_eq!(
            spaces.malformed,
            vec![(1, "好 hao".into()), (2, "号 hao".into())]
        );
        assert_eq!(spaces.unusable, 0);

        // 两条都是货真价实的 2/3 列行，只是词条不可用：非 ASCII pinyin、词频非数。
        let fields = parse_dict_report("好\thāo\n坏\thao\tabc\n");
        assert!(fields.entries.is_empty());
        assert!(fields.malformed.is_empty(), "字段问题不算列数问题");
        assert_eq!(fields.unusable, 2);

        // 记录一个既有语义（别按直觉改）：`trim()` 连 TAB 一起裁，所以**空字段的行到不了
        // 「空 word/pinyin」那条字段规则** —— `好\t`（缺第二列）与 `\thao`（缺第一列）
        // 裁完都只剩 1 列，归 malformed。丢的行没变，只是分类如此。
        assert_eq!(
            parse_dict_report("好\t\n").malformed,
            vec![(1, "好".into())]
        );
        assert_eq!(
            parse_dict_report("\thao\n").malformed,
            vec![(1, "hao".into())]
        );

        // front-matter 是**整段**跳过（文档写明要跳过的输入），不进任何统计：它里面的行
        // 在解析器眼里都是「一列」，按列数报就会变成「注释说跳过、诊断说格式错」。
        let front = parse_dict_report("---\nname: luna\n# 注释\n\n...\n好\thao\n");
        assert_eq!(
            (front.malformed.len(), front.unusable),
            (0, 0),
            "front-matter 区间内的行不报、不计数"
        );
        assert_eq!(front.entries.len(), 1, "front-matter 不挡条目");

        // 但**没闭合**的头不是 front-matter，是格式错：否则一个手滑的 `---` 会把整份
        // 文件静默吃完（0 条产出、exit 0），正是本模块在防的那种故障。
        let unterminated = parse_dict_report("---\nname: luna\n好 hao\n");
        assert_eq!(
            unterminated.malformed,
            vec![(2, "name: luna".into()), (3, "好 hao".into())],
            "未闭合的 front-matter 必须落地成格式错"
        );

        // 中间的 `---` 不当开启：否则一份 TSV 里一个手滑的 `---` 会吃掉它后面所有词。
        let mid = parse_dict_report("好\thao\n---\n号\thao\n");
        assert!(mid.malformed.is_empty(), "中间的 --- 只是被跳过的一行");
        assert_eq!(mid.entries.len(), 2, "它后面的词必须照常解析");
    }
}
