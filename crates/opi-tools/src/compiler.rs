//! rime 文本解析器（dict.yaml / 项目 tsv → .opid 条目）。
//!
//! 解析规则见 M2 plan Task 5：跳过空行、`#` 注释、front-matter；
//! `\t` 切分 2 列默认 freq=1000、3 列整数直接取、`NN.NN%` 按 round(percent × 1000)；
//! 非 ASCII 或 >255 字节的 pinyin、空 word/pinyin 跳过；重复 (pinyin, word) 取最大 freq。

use engine_data::format::{OpDict, RawEntry, serialize};
use std::collections::HashMap;
use std::path::Path;

/// 解析 rime dict.yaml / 项目 tsv 文本为条目。见 plan Task 5 规则。
pub fn parse_dict(text: &str) -> Vec<RawEntry> {
    let mut best: HashMap<(String, String), u32> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 || cols.len() > 3 {
            continue;
        }
        let word = cols[0].trim();
        let pinyin = cols[1].trim().to_lowercase();
        if word.is_empty() || pinyin.is_empty() || !pinyin.is_ascii() || pinyin.len() > u8::MAX as usize {
            continue;
        }
        let freq = match cols.get(2).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => 1000,
            Some(s) => match parse_freq(s) {
                Some(f) => f,
                None => continue,
            },
        };
        let key = (pinyin.clone(), word.to_string());
        let prev = best.get(&key).copied().unwrap_or(0);
        best.insert(key, freq.max(prev));
    }
    let mut out: Vec<RawEntry> = best
        .into_iter()
        .map(|((pinyin, word), freq)| RawEntry { pinyin, word, freq })
        .collect();
    out.sort_by(|a, b| a.pinyin.as_bytes().cmp(b.pinyin.as_bytes()));
    out
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
    OpDict { entries, pinyin_total }
}

pub fn compile_file(input: &Path, output: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(input).map_err(|e| format!("read {}: {e}", input.display()))?;
    let dict = compile(parse_dict(&text));
    let bytes = serialize(&dict);
    std::fs::write(output, &bytes).map_err(|e| format!("write {}: {e}", output.display()))?;
    Ok(())
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
}
