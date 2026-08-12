use opi_tools::compiler::parse_dict;
use proptest::prelude::*;

proptest! {
    /// 任意行输入不 panic
    #[test]
    fn never_panics_on_arbitrary_lines(
        lines in prop::collection::vec(prop::collection::vec(any::<String>(), 0..5), 0..50),
    ) {
        let text = lines.iter().map(|l| l.join("\t")).collect::<Vec<_>>().join("\n");
        let _ = parse_dict(&text);
    }

    /// 重复 (pinyin, word) 取最大 freq
    #[test]
    fn duplicate_takes_max(
        pinyin in "[a-z]{1,6}",
        word in "[\u{4e00}-\u{9fff}]{1,4}",
        a in 0u32..10_000,
        b in 0u32..10_000,
    ) {
        let text = format!("{word}\t{pinyin}\t{a}\n{word}\t{pinyin}\t{b}\n");
        let entries = parse_dict(&text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].freq, a.max(b));
    }
}
