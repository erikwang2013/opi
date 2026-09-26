// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 生僻字（CJK 扩展 A/B）覆盖门禁 —— 项目缘起第一条的正面回答。
//!
//! 缺陷：`scripts/hanzi_freq.py:86,106`、`scripts/gen_trad_dict.py:107`、
//! `scripts/gen_luna_dict.py:65` 四处 `0x4E00..=0x9FFF` 过滤把 **CJK 扩展 A
//! （U+3400–4DBF）与扩展 B（U+20000–2A6DF）整段丢弃** —— 词库里根本没有这些字，
//! 再多的候选翻页也翻不出来（README 缘起：「想输入一个生僻字，翻了三页候选词都找不到」）。
//!
//! 本门禁锁**存在性**：扩展区代表字必须能由拼音查到。名次高低属排序质量问题，由
//! `ranking_quality.rs` 覆盖，此处不断言名次。
//!
//! 代表字读音取自 Unihan `kMandarin`（data/raw/LICENSES.md 记录的同一份上游），
//! 与 luna_pinyin.dict.yaml 的标注在这几个字上一致（qiu/tian/kua/wu/yin/he/qi）。
//!
//! 上游 pin 与数据来源同 `hanzi_freq.py` 头注释；本测试只读不联网。

use engine_core::dictionary::Dictionary;
use engine_data::load_mmap;
use std::path::Path;

const TRAD_OPID: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/generated/trad.opid");
const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

// ---- 符号面板数据（scripts/gen_symbols.py 产出，提交入库）----
// 路径与 engine-core/src/symbols.rs::builtin() 的 include_str! 一致（data/raw/，非任务书
// 写的 data/generated/）：消费侧是**编译期**读取，路径不一致直接编不过 —— 见该处注释。
const SYMBOL_BLOCKS_TSV: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/symbol_blocks.tsv");
const SYMBOLS_TSV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/symbols.tsv");

/// common=1 区块的条目下限。板面是 8 列网格，32 条 = 4 行。
/// 取值依据：实测最小块是 CJK 符号 60 条，32 留了约 2 倍余量 —— 真实要拦的是
/// 「声明了区块但几乎没内容」：改动前 `SymbolEngine::builtin()` 的 5 个常用区块
/// 合计只有 **8 条**（60 → 3 条的实际水平），另外还声明了「CJK 扩展 A」整段
/// 6592 个码位、**0 条**（该块 common=0，故不在常用页，但声明了空区间同样是坏数据）。
/// 任何把区块砍掉一半的笔误（十六进制范围少写一位：3000–303F → 3000–300F 只剩 16 条）
/// 都会红，不是「永远能过」的阈值。
const MIN_COMMON_BLOCK: usize = 32;

/// (字, 拼音, 来源码位)：扩展 A 取 U+3400–3406，扩展 B 取 U+20000–20001。
const RARE: &[(&str, &str, u32)] = &[
    ("㐀", "qiu", 0x3400), // 扩展 A 首字
    ("㐁", "tian", 0x3401),
    ("㐄", "kua", 0x3404),
    ("㐅", "wu", 0x3405),
    ("㐆", "yin", 0x3406),
    ("𠀀", "he", 0x20000), // 扩展 B 首字（UTF-8 四字节 / UTF-16 代理对）
    ("𠀁", "qi", 0x20001),
];

/// 扩展区代表字在该前缀查询中的名次（1 起，去重）；0 = 无候选。
fn rank_of<D: Dictionary + ?Sized>(dict: &D, pinyin: &str, expected: &str) -> usize {
    let mut seen = std::collections::HashSet::new();
    let mut rank = 0;
    for e in dict.query(pinyin, usize::MAX) {
        if !seen.insert(e.word.clone()) {
            continue;
        }
        rank += 1;
        if e.word == expected {
            return rank;
        }
    }
    0
}

fn gate(label: &str, path: &str) {
    let dict = load_mmap(Path::new(path)).unwrap_or_else(|e| panic!("加载 {path} 失败：{e:?}"));
    let mut fails = Vec::new();
    for (ch, py, cp) in RARE {
        let rank = rank_of(&dict, py, ch);
        if rank == 0 {
            fails.push(format!("U+{cp:05X} {ch}（{py}）→ 无候选：该扩展区字未进词库"));
        } else {
            println!("[{label}] U+{cp:05X} {ch} ← {py} 第 {rank} 位");
        }
    }
    assert!(
        fails.is_empty(),
        "[{label}] 生僻字覆盖不达标（{}/{}）：\n  {}",
        fails.len(),
        RARE.len(),
        fails.join("\n  ")
    );
}

#[test]
fn trad_rare_chars_reachable() {
    gate("trad", TRAD_OPID);
}

#[test]
fn luna_rare_chars_reachable() {
    gate("luna", LUNA_OPID);
}

// ---- 任务 2：符号面板数据门禁 ----

/// TSV → 行×列，列数不符即报错（生成器少写一列会当场炸，而不是静默错位）。
fn read_tsv(path: &str, cols: usize) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("读 {path} 失败：{e}\n（由 scripts/gen_symbols.py 生成，应提交入库）"));
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let f: Vec<String> = line.split('\t').map(str::to_string).collect();
        assert_eq!(f.len(), cols, "{path}:{} 列数 {} ≠ {cols}：{line:?}", i + 1, f.len());
        rows.push(f);
    }
    assert!(!rows.is_empty(), "{path} 无数据行");
    rows
}

#[test]
fn symbol_data_wellformed() {
    // 区块：id / start / end / name / common
    let mut blocks: Vec<(u32, u32, u32, String, bool)> = Vec::new();
    for b in read_tsv(SYMBOL_BLOCKS_TSV, 5) {
        let id: u32 = b[0].parse().unwrap_or_else(|_| panic!("区块 id {:?} 非十进制", b[0]));
        let (start, end) = (
            u32::from_str_radix(&b[1], 16).unwrap_or_else(|_| panic!("start {:?} 非十六进制", b[1])),
            u32::from_str_radix(&b[2], 16).unwrap_or_else(|_| panic!("end {:?} 非十六进制", b[2])),
        );
        assert!(start <= end, "区块 {id} 范围倒置 U+{start:04X}-{end:04X}");
        let common = match b[4].as_str() {
            "0" => false,
            "1" => true,
            o => panic!("区块 {id} common={o:?} 非 0/1"),
        };
        // 交叠检查：SymbolEngine::new 的 debug_assert! 只在 debug 生效，数据侧在此锁死
        for (oid, os, oe, oname, _) in &blocks {
            assert!(
                end < *os || start > *oe,
                "区块 {id}「{}」U+{start:04X}-{end:04X} 与区块 {oid}「{oname}」U+{os:04X}-{oe:04X} 交叠",
                b[3]
            );
        }
        assert!(!b[3].is_empty(), "区块 {id} 名字为空");
        blocks.push((id, start, end, b[3].clone(), common));
    }

    // 条目：text / name / keywords / block_id / emoji
    let mut counts: Vec<usize> = vec![0; blocks.len()];
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, r) in read_tsv(SYMBOLS_TSV, 5).iter().enumerate() {
        let (text, name, keys, bid, emoji) = (&r[0], &r[1], &r[2], &r[3], &r[4]);
        let line = i + 1;
        assert_eq!(
            text.chars().count(),
            1,
            "第 {line} 行 text {text:?} 不是单字符（面板一格一条）"
        );
        assert!(!name.is_empty(), "第 {line} 行 {text} 名字为空");
        let cp = text.chars().next().unwrap() as u32;
        let bid: u32 = bid.parse().unwrap_or_else(|_| panic!("第 {line} 行 block_id {bid:?} 非十进制"));
        let idx = blocks
            .iter()
            .position(|(id, ..)| *id == bid)
            .unwrap_or_else(|| panic!("第 {line} 行 {text} 指向未声明的区块 id={bid}"));
        let (_, start, end, bname, _common) = &blocks[idx];
        assert!(
            cp >= *start && cp <= *end,
            "第 {line} 行 {text} U+{cp:04X} 不在区块 {bid}「{bname}」U+{start:04X}-{end:04X} 内"
        );
        // emoji ⟺ 非 BMP：Android SymbolCatalog.isEmoji() 就是「含代理项」，口径必须一致
        let want_emoji = if cp > 0xFFFF { "1" } else { "0" };
        assert_eq!(emoji, want_emoji, "第 {line} 行 {text} U+{cp:04X} emoji={emoji}，应为 {want_emoji}");
        // 关键字：引擎按字节前缀匹配，非小写 ASCII 永远搜不到
        let keys: Vec<&str> = keys.split(',').filter(|k| !k.is_empty()).collect();
        assert!(!keys.is_empty(), "第 {line} 行 {text} 无关键字（搜不到）");
        for k in &keys {
            assert!(
                k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "第 {line} 行 {text} 关键字 {k:?} 非小写 ASCII 字母数字"
            );
        }
        // 重复 text：SymbolEngine::search 按 text 去重，重复条目会静默丢一条
        if let Some(prev) = seen.insert(text.clone(), line) {
            panic!("{text} 在第 {prev} 行与第 {line} 行重复");
        }
        counts[idx] += 1;
    }

    // 每个声明区块必须有内容（旧 builtin() 的「CJK 扩展 A」声明 6592 码位、0 条 —— 正是这条要拦的）
    for (i, (id, start, end, name, common)) in blocks.iter().enumerate() {
        assert!(counts[i] > 0, "区块 {id}「{name}」U+{start:04X}-{end:04X} 声明了却一条数据都没有");
        if *common {
            assert!(
                counts[i] >= MIN_COMMON_BLOCK,
                "常用区块 {id}「{name}」只有 {} 条（下限 {MIN_COMMON_BLOCK}）",
                counts[i]
            );
        }
        println!(
            "[symbols] 区块 {id} {name} U+{start:04X}-{end:04X} {} 条 common={}",
            counts[i],
            if *common { 1 } else { 0 }
        );
    }
    println!("[symbols] 合计 {} 条 / {} 区块", counts.iter().sum::<usize>(), blocks.len());
}
