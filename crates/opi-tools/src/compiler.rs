// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! rime 文本解析器（dict.yaml / 项目 tsv → .opid 条目）的 **CLI 侧薄包装**。
//!
//! 解析规则（跳过空行 / `#` 注释 / front-matter、`\t` 切 2..=3 列、词频百分比、
//! BOM 剥离、坏行统计）与实现本体已于 2026-10-14（M3 计划 Task 2a）搬去
//! `engine-data::dict_text`：导入词库要在不打 CLI 的情况下解析同一份文本，
//! 两份实现必然漂移。这里只保留原名字（`parse_dict` / `parse_dict_report` /
//! `Parsed`）与 re-export，**行为一字不变** —— 本文件的既有测试原样跑，
//! 锁的就是这条「搬家没改行为」。

use engine_data::format::{OpDict, RawEntry, serialize};
use std::path::Path;

pub use engine_data::dict_text::{Parsed, parse_report as parse_dict_report};

/// 解析 rime dict.yaml / 项目 tsv 文本为条目。规则见 [`engine_data::dict_text`]。
pub fn parse_dict(text: &str) -> Vec<RawEntry> {
    parse_dict_report(text).entries
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

/// 路径规范化后比对（覆盖符号链接与相对/绝对写法，不覆盖硬链接）。
#[cfg(not(unix))]
fn canonical_paths_equal(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Windows：`GetFileInformationByHandle` 的（卷序列号, 文件索引）就是 unix 的
/// （dev, ino）—— 硬链接指向同一份 MFT 记录，故这个判据能拦住
/// `mklink /H` 造出来的别名。std 的 `MetadataExt::file_index` 仍是 unstable
/// （`windows_by_handle`），所以这里直接调 Win32。
#[cfg(windows)]
fn same_file(a: &Path, b: &Path) -> bool {
    if canonical_paths_equal(a, b) {
        return true;
    }
    match (windows_file_id(a), windows_file_id(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// 非 unix / 非 Windows 的退化：只能比规范化路径。
#[cfg(not(any(unix, windows)))]
fn same_file(a: &Path, b: &Path) -> bool {
    canonical_paths_equal(a, b)
}

#[cfg(windows)]
fn windows_file_id(path: &Path) -> Option<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    struct Filetime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct ByHandleFileInformation {
        attributes: u32,
        creation: Filetime,
        last_access: Filetime,
        last_write: Filetime,
        volume_serial: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            file: *mut core::ffi::c_void,
            info: *mut ByHandleFileInformation,
        ) -> i32;
    }

    let file = std::fs::File::open(path).ok()?;
    // SAFETY: 全零是该结构合法的初始值（全是标量）；file 的句柄在调用期间
    // 有效，info 是独占的、大小形状与 Win32 的 BY_HANDLE_FILE_INFORMATION 相同。
    let mut info: ByHandleFileInformation = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle() as *mut _, &mut info) };
    if ok == 0 {
        return None;
    }
    Some((
        info.volume_serial,
        (u64::from(info.index_high) << 32) | u64::from(info.index_low),
    ))
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
