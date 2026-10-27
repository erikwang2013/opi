// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `scheme` 的单测（2026-10-27 从源文件拆出：源文件原本 500 行，加
//! `key_for_letter` 后 521 行，撞 500 行门禁）。与 `candidates_tests.rs` /
//! `learner_tests.rs` 同一套 `#[path]` 先例，语义一字未动。

use super::*;

/// 整数码是**对外协议**（JNI/C ABI 两张整数表）：整表逐值钉，改一个数先在这里红。
#[test]
fn code_table_is_the_protocol() {
    let table: [(Scheme, i32); 6] = [
        (Scheme::Full, 0),
        (Scheme::Nine, 1),
        (Scheme::Fourteen, 2),
        (Scheme::Stroke, 3),
        (Scheme::Shuangpin, 4),
        (Scheme::Wubi, 5),
    ];
    for (s, c) in table {
        assert_eq!(s.code(), c, "{s:?} 的码");
        assert_eq!(Scheme::from_code(c), Some(s), "码 {c} 该读回 {s:?}");
    }
    // 越界 → None（写侧忽略、读侧按 Full —— 由 FFI 层落成各自哨兵）。
    assert_eq!(Scheme::from_code(6), None);
    assert_eq!(Scheme::from_code(-1), None);
}

/// 名字表（配置面）：往返 + 认不得 → None（**不是**回默认）。
#[test]
fn name_table_round_trips() {
    for s in [
        Scheme::Full,
        Scheme::Nine,
        Scheme::Fourteen,
        Scheme::Stroke,
        Scheme::Shuangpin,
        Scheme::Wubi,
    ] {
        assert_eq!(Scheme::from_name(s.name()), Some(s), "{s:?} 的名字往返");
    }
    assert_eq!(
        Scheme::from_name(" 双拼 "),
        Some(Scheme::Shuangpin),
        "两侧空白容错"
    );
    assert_eq!(
        Scheme::from_name("全拼含简拼"),
        None,
        "认不得 ⇒ None（简拼是另一个开关）"
    );
    assert_eq!(Scheme::from_name(""), None);
}

#[test]
fn nine_key_class_table() {
    assert_eq!(Scheme::Nine.key_class('2'), Some(&['a', 'b', 'c'][..]));
    assert_eq!(Scheme::Nine.key_class('6'), Some(&['m', 'n', 'o'][..]));
    assert_eq!(Scheme::Nine.key_class('7'), Some(&['p', 'q', 'r', 's'][..]));
    // 0/1 不是输入键；字母是「单字母类」（2026-10-26 弹层精确选择）
    assert_eq!(Scheme::Nine.key_class('0'), None);
    assert_eq!(Scheme::Nine.key_class('1'), None);
    assert_eq!(Scheme::Nine.key_class('a'), Some(&['a'][..]));
    assert_eq!(Scheme::Nine.key_class('z'), Some(&['z'][..]));
    assert_eq!(Scheme::Nine.key_leading_letter('a'), Some('a'));
}

#[test]
fn fourteen_key_class_table_is_pinned() {
    // 这张表是**协议**（宿主画键盘、发符号都用它）：钉住整表，改一个字母
    // 都要在这里先红。
    let table: Vec<(char, &[char])> = vec![
        ('q', &['q', 'w']),
        ('e', &['e', 'r']),
        ('t', &['t', 'y']),
        ('u', &['u', 'i']),
        ('o', &['o', 'p']),
        ('a', &['a', 's']),
        ('d', &['d', 'f']),
        ('g', &['g', 'h']),
        ('j', &['j', 'k']),
        ('l', &['l']),
        ('z', &['z', 'x']),
        ('c', &['c', 'v']),
        ('b', &['b', 'n']),
        ('m', &['m']),
    ];
    // 14 个键，且并集恰好是全部 26 个字母（无遗漏、无重复）
    assert_eq!(table.len(), 14);
    let mut all: Vec<char> = table
        .iter()
        .flat_map(|(_, cs)| cs.iter().copied())
        .collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 26, "字母并集不是 26 个（有重复或遗漏）：{all:?}");
    for (k, cs) in table {
        assert_eq!(Scheme::Fourteen.key_class(k), Some(cs), "键 {k} 的类不符");
    }
    assert_eq!(
        Scheme::Fourteen.key_class('x'),
        None,
        "x 不是 14 键的键（属 z 键的类）"
    );
    assert_eq!(Scheme::Fourteen.key_class('2'), None);
}

#[test]
fn key_leading_letter_is_the_first_of_the_class() {
    // 宽松兜底档（用户裁决 2026-10-15）只认这一个来源 —— 表动的字母就是它出的字母。
    assert_eq!(Scheme::Nine.key_leading_letter('2'), Some('a'));
    assert_eq!(Scheme::Nine.key_leading_letter('4'), Some('g'));
    assert_eq!(Scheme::Nine.key_leading_letter('7'), Some('p'));
    assert_eq!(Scheme::Nine.key_leading_letter('9'), Some('w'));
    assert_eq!(Scheme::Nine.key_leading_letter('0'), None);
    assert_eq!(Scheme::Nine.key_leading_letter('1'), None);
    // 2026-10-26：具体小写字母也有单字母类，首字母就是它自己。
    assert_eq!(Scheme::Nine.key_leading_letter('a'), Some('a')); // was None before
    // 14 键：键类首字母 == 键自身（2026-10-27 起宽松兜底档也用它，与 9 键同语义）
    assert_eq!(Scheme::Fourteen.key_leading_letter('b'), Some('b'));
    assert_eq!(Scheme::Fourteen.key_leading_letter('q'), Some('q'));
    // 非键方案恒 None
    assert_eq!(Scheme::Full.key_leading_letter('a'), None);
    assert_eq!(Scheme::Wubi.key_leading_letter('a'), None);
}

#[test]
fn nine_matches_prefix_semantics() {
    assert!(Scheme::Nine.matches("64", "ni"));
    assert!(
        Scheme::Nine.matches("64", "nihao"),
        "前缀：ni 的扩展 nihao 也该命中"
    );
    assert!(Scheme::Nine.matches("64426", "nihao"));
    assert!(!Scheme::Nine.matches("64", "li"), "l 不在 6 的类里");
    assert!(!Scheme::Nine.matches("64426", "ni"), "输入比键长 → 不匹配");
    assert!(!Scheme::Nine.matches("", "ni"));
    assert!(!Scheme::Nine.matches("64", ""), "空键不匹配");
    // 2026-10-26：弹层选定具体小写字母 = 单字母类，不再按整格 T9 类匹配。
    assert!(Scheme::Nine.matches("a", "ai"));
    assert!(Scheme::Nine.matches("c", "ci"));
    assert!(!Scheme::Nine.matches("a", "bi"), "选 a 不能匹配 b 开头的词");
    assert!(!Scheme::Nine.matches("a", "ci"), "选 a 不能匹配 c 开头的词");
    assert!(Scheme::Nine.matches("ab", "abao"));
    assert!(!Scheme::Nine.matches("ab", "acao"));
}

#[test]
fn fourteen_matches_prefix_semantics() {
    // n→b 键、i→u 键 ⇒ "bu" 命中 "ni"
    assert!(Scheme::Fourteen.matches("bu", "ni"));
    assert!(Scheme::Fourteen.matches("bu", "nihao"));
    // q 键类 {q,w}：输入 q 命中 qiang/wang
    assert!(Scheme::Fourteen.matches("q", "qiang"));
    assert!(Scheme::Fourteen.matches("q", "wang"));
    assert!(!Scheme::Fourteen.matches("q", "ni"));
    assert!(!Scheme::Fourteen.matches("", "ni"));
}

#[test]
fn full_and_stroke_are_plain_prefix() {
    assert!(Scheme::Full.matches("ni", "nihao"));
    assert!(Scheme::Stroke.matches("hsp", "hspnz"));
    assert!(!Scheme::Stroke.matches("hsp", "hs"));
}

/// `key_for_letter` 是 `key_class` 的反向查询（补读音层 / PINNED 编码共用）。
/// 9 键取 T9 数字、14 键取类里的规范符号；非按键方案恒 None。
#[test]
fn key_for_letter_reverses_the_class_table() {
    assert_eq!(Scheme::Nine.key_for_letter('a'), Some('2'));
    assert_eq!(Scheme::Nine.key_for_letter('n'), Some('6'));
    assert_eq!(Scheme::Nine.key_for_letter('z'), Some('9'));
    assert_eq!(Scheme::Fourteen.key_for_letter('n'), Some('b'));
    assert_eq!(Scheme::Fourteen.key_for_letter('w'), Some('q'));
    assert_eq!(Scheme::Fourteen.key_for_letter('x'), Some('z'));
    assert_eq!(Scheme::Fourteen.key_for_letter('m'), Some('m'));
    assert_eq!(Scheme::Full.key_for_letter('a'), None);
    assert_eq!(Scheme::Stroke.key_for_letter('h'), None);
    assert_eq!(Scheme::Wubi.key_for_letter('w'), None);
}

/// 五笔与 Full/Stroke 同为**纯前缀**（键是普通字母，无按键类展开、无解码）。
/// 这条是协议级判据：改 semantics 先在这里红（生产候选走词典前缀查询，
/// 与 matches 同判据，见 engine_wubi.rs）。
#[test]
fn wubi_matches_like_plain_prefix() {
    assert!(Scheme::Wubi.matches("wq", "wqvb"));
    assert!(!Scheme::Wubi.matches("wqvb", "wq"));
    assert!(!Scheme::Wubi.matches("", "wqvb"));
    assert_eq!(Scheme::Wubi.key_class('a'), None, "键是普通字母");
    assert_eq!(Scheme::Wubi.code(), 5);
    assert_eq!(Scheme::from_name("五笔"), Some(Scheme::Wubi));
}
