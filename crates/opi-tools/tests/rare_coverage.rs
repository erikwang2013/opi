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

const TRAD_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/generated/trad.opid"
);
const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

// ---- 符号面板数据（scripts/gen_symbols.py 产出，提交入库）----
// 路径与 engine-core/src/symbols.rs::builtin() 的 include_str! 一致（data/raw/，非任务书
// 写的 data/generated/）：消费侧是**编译期**读取，路径不一致直接编不过 —— 见该处注释。
const SYMBOL_BLOCKS_TSV: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/raw/symbol_blocks.tsv"
);
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
            fails.push(format!(
                "U+{cp:05X} {ch}（{py}）→ 无候选：该扩展区字未进词库"
            ));
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
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("读 {path} 失败：{e}\n（由 scripts/gen_symbols.py 生成，应提交入库）")
    });
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let f: Vec<String> = line.split('\t').map(str::to_string).collect();
        assert_eq!(
            f.len(),
            cols,
            "{path}:{} 列数 {} ≠ {cols}：{line:?}",
            i + 1,
            f.len()
        );
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
        let id: u32 = b[0]
            .parse()
            .unwrap_or_else(|_| panic!("区块 id {:?} 非十进制", b[0]));
        let (start, end) = (
            u32::from_str_radix(&b[1], 16)
                .unwrap_or_else(|_| panic!("start {:?} 非十六进制", b[1])),
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
        let bid: u32 = bid
            .parse()
            .unwrap_or_else(|_| panic!("第 {line} 行 block_id {bid:?} 非十进制"));
        let idx = blocks
            .iter()
            .position(|(id, ..)| *id == bid)
            .unwrap_or_else(|| panic!("第 {line} 行 {text} 指向未声明的区块 id={bid}"));
        let (_, start, end, bname, _common) = &blocks[idx];
        assert!(
            cp >= *start && cp <= *end,
            "第 {line} 行 {text} U+{cp:04X} 不在区块 {bid}「{bname}」U+{start:04X}-{end:04X} 内"
        );
        // emoji ⟺ UTS#51 `Emoji` 属性（扣掉修饰符/组件）—— 与生成器 scripts/gen_symbols.py
        // 同一判据、同一份快照（见文件末尾）。旧口径 `cp > 0xFFFF` 是代理指标：非 BMP 的
        // 非 emoji（整个 U+1F780–1F7FF、自建块的 U+1D400…）会被**整类放行**。
        let want_emoji = if is_uts51_emoji(cp) { "1" } else { "0" };
        assert_eq!(
            emoji, want_emoji,
            "第 {line} 行 {text} U+{cp:04X} emoji={emoji}，应为 {want_emoji}"
        );
        // 关键字：引擎按字节前缀匹配，非小写 ASCII 永远搜不到
        let keys: Vec<&str> = keys.split(',').filter(|k| !k.is_empty()).collect();
        assert!(!keys.is_empty(), "第 {line} 行 {text} 无关键字（搜不到）");
        for k in &keys {
            assert!(
                k.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
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
        assert!(
            counts[i] > 0,
            "区块 {id}「{name}」U+{start:04X}-{end:04X} 声明了却一条数据都没有"
        );
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
    println!(
        "[symbols] 合计 {} 条 / {} 区块",
        counts.iter().sum::<usize>(),
        blocks.len()
    );
}

// ---- UTS#51 emoji 属性快照：**与 crates/engine-core/tests/symbol_coverage.rs 同一份** ----
// 判据只能有一个来源：这里不另写规则，而是**逐字节照抄**那份快照（同一上游文件、同一个
// SHA-256 pin，pin 记在 data/raw/LICENSES.md）。两份都对着同一份 data/raw/symbols.tsv
// 断言 ⇒ 任一份漂移都会立刻变红，不会静默；改一处必须改两处。
//
// ⚠️ 这一列原来钉的是 `cp > 0xFFFF` —— 一个**会整类放行**的代理指标：非 BMP 的非 emoji
// 全部放过（自建块 19 + U+1D400「𝐀」加进来照样全绿），而它守的正是数据形状。别改回去。
/// UTS#51 `Emoji` 属性区间快照 —— Unicode **18.0.0** `emoji-data.txt` 里 `Emoji` 行的
/// 第一列**逐行照抄**（424 段，段序与上游文件相同；`A..B` 是闭区间，单码位只写一个）。
///
/// 生成器 `scripts/gen_symbols.py` 用**同一份文件、同一个 SHA-256 pin** 算 emoji 列，
/// 这里抄一份独立副本，于是三种漂移都会红：判据被改回代理指标、`symbols.tsv` 被手改、
/// 上游换版而两边没同步。复算（升级 Unicode 时照跑，输出直接换掉下面这段）：
///
/// ```text
/// curl -s https://www.unicode.org/Public/18.0.0/ucd/emoji/emoji-data.txt \
///   | awk -F';' '/^[0-9A-F]/ {p=$2; gsub(/ |#.*/,"",p); if (p=="Emoji") print $1}'
/// ```
const UTS51_EMOJI: &str = "\
0023 002A 0030..0039 00A9 00AE 203C 2049 2122 2139 2194..2199 21A9..21AA 231A..231B
2328 23CF 23E9..23EC 23ED..23EE 23EF 23F0 23F1..23F2 23F3 23F8..23FA 24C2 25AA..25AB 25B6
25C0 25FB..25FE 2600..2601 2602..2603 2604 260E 2611 2614..2615 2618 261D 2620 2622..2623
2626 262A 262E 262F 2638..2639 263A 2640 2642 2648..2653 265F 2660 2663
2665..2666 2668 267B 267E 267F 2692 2693 2694 2695 2696..2697 2699 269B..269C
26A0..26A1 26A7 26AA..26AB 26B0..26B1 26BD..26BE 26C4..26C5 26C8 26CE 26CF 26D1 26D3 26D4
26E9 26EA 26F0..26F1 26F2..26F3 26F4 26F5 26F7..26F9 26FA 26FD 2702 2705 2708..270C
270D 270F 2712 2714 2716 271D 2721 2728 2733..2734 2744 2747 274C
274E 2753..2755 2757 2763 2764 2795..2797 27A1 27B0 27BF 2934..2935 2B05..2B07 2B1B..2B1C
2B50 2B55 3030 303D 3297 3299 1F004 1F0CF 1F170..1F171 1F17E..1F17F 1F18E 1F191..1F19A
1F1E6..1F1FF 1F201..1F202 1F21A 1F22F 1F232..1F23A 1F250..1F251 1F300..1F30C 1F30D..1F30E 1F30F 1F310 1F311 1F312
1F313..1F315 1F316..1F318 1F319 1F31A 1F31B 1F31C 1F31D..1F31E 1F31F..1F320 1F321 1F324..1F32C 1F32D..1F32F 1F330..1F331
1F332..1F333 1F334..1F335 1F336 1F337..1F34A 1F34B 1F34C..1F34F 1F350 1F351..1F37B 1F37C 1F37D 1F37E..1F37F 1F380..1F393
1F396..1F397 1F399..1F39B 1F39E..1F39F 1F3A0..1F3C4 1F3C5 1F3C6 1F3C7 1F3C8 1F3C9 1F3CA 1F3CB..1F3CE 1F3CF..1F3D3
1F3D4..1F3DF 1F3E0..1F3E3 1F3E4 1F3E5..1F3F0 1F3F3 1F3F4 1F3F5 1F3F7 1F3F8..1F407 1F408 1F409..1F40B 1F40C..1F40E
1F40F..1F410 1F411..1F412 1F413 1F414 1F415 1F416 1F417..1F429 1F42A 1F42B..1F43E 1F43F 1F440 1F441
1F442..1F464 1F465 1F466..1F46B 1F46C..1F46D 1F46E..1F4AC 1F4AD 1F4AE..1F4B5 1F4B6..1F4B7 1F4B8..1F4EB 1F4EC..1F4ED 1F4EE 1F4EF
1F4F0..1F4F4 1F4F5 1F4F6..1F4F7 1F4F8 1F4F9..1F4FC 1F4FD 1F4FF..1F502 1F503 1F504..1F507 1F508 1F509 1F50A..1F514
1F515 1F516..1F52B 1F52C..1F52D 1F52E..1F53D 1F549..1F54A 1F54B..1F54E 1F550..1F55B 1F55C..1F567 1F56F..1F570 1F573..1F579 1F57A 1F587
1F58A..1F58D 1F590 1F595..1F596 1F5A4 1F5A5 1F5A8 1F5B1..1F5B2 1F5BC 1F5C2..1F5C4 1F5D1..1F5D3 1F5DC..1F5DE 1F5E1
1F5E3 1F5E8 1F5EF 1F5F3 1F5FA 1F5FB..1F5FF 1F600 1F601..1F606 1F607..1F608 1F609..1F60D 1F60E 1F60F
1F610 1F611 1F612..1F614 1F615 1F616 1F617 1F618 1F619 1F61A 1F61B 1F61C..1F61E 1F61F
1F620..1F625 1F626..1F627 1F628..1F62B 1F62C 1F62D 1F62E..1F62F 1F630..1F633 1F634 1F635 1F636 1F637..1F640 1F641..1F644
1F645..1F64F 1F680 1F681..1F682 1F683..1F685 1F686 1F687 1F688 1F689 1F68A..1F68B 1F68C 1F68D 1F68E
1F68F 1F690 1F691..1F693 1F694 1F695 1F696 1F697 1F698 1F699..1F69A 1F69B..1F6A1 1F6A2 1F6A3
1F6A4..1F6A5 1F6A6 1F6A7..1F6AD 1F6AE..1F6B1 1F6B2 1F6B3..1F6B5 1F6B6 1F6B7..1F6B8 1F6B9..1F6BE 1F6BF 1F6C0 1F6C1..1F6C5
1F6CB 1F6CC 1F6CD..1F6CF 1F6D0 1F6D1..1F6D2 1F6D5 1F6D6..1F6D7 1F6D8 1F6D9 1F6DC 1F6DD..1F6DF 1F6E0..1F6E5
1F6E9 1F6EB..1F6EC 1F6F0 1F6F3 1F6F4..1F6F6 1F6F7..1F6F8 1F6F9 1F6FA 1F6FB..1F6FC 1F7E0..1F7EB 1F7F0 1F90C
1F90D..1F90F 1F910..1F918 1F919..1F91E 1F91F 1F920..1F927 1F928..1F92F 1F930 1F931..1F932 1F933..1F93A 1F93C..1F93E 1F93F 1F940..1F945
1F947..1F94B 1F94C 1F94D..1F94F 1F950..1F95E 1F95F..1F96B 1F96C..1F970 1F971 1F972 1F973..1F976 1F977..1F978 1F979 1F97A
1F97B 1F97C..1F97F 1F980..1F984 1F985..1F991 1F992..1F997 1F998..1F9A2 1F9A3..1F9A4 1F9A5..1F9AA 1F9AB..1F9AD 1F9AE..1F9AF 1F9B0..1F9B9 1F9BA..1F9BF
1F9C0 1F9C1..1F9C2 1F9C3..1F9CA 1F9CB 1F9CC 1F9CD..1F9CF 1F9D0..1F9E6 1F9E7..1F9FF 1FA70..1FA73 1FA74 1FA75..1FA77 1FA78..1FA7A
1FA7B..1FA7C 1FA80..1FA82 1FA83..1FA86 1FA87..1FA88 1FA89 1FA8A 1FA8B..1FA8D 1FA8E 1FA8F 1FA90..1FA95 1FA96..1FAA8 1FAA9..1FAAC
1FAAD..1FAAF 1FAB0..1FAB6 1FAB7..1FABA 1FABB..1FABD 1FABE 1FABF 1FAC0..1FAC2 1FAC3..1FAC5 1FAC6 1FAC8 1FACC 1FACD
1FACE..1FACF 1FAD0..1FAD6 1FAD7..1FAD9 1FADA..1FADB 1FADC 1FADD 1FADF 1FAE0..1FAE7 1FAE8 1FAE9 1FAEA 1FAEB
1FAEF 1FAF0..1FAF6 1FAF7..1FAF8 1FAF9..1FAFA
";

/// UTS#51 `Emoji_Modifier` ∪ `Emoji_Component`（11 段，其中 1F3FB..1F3FF 两个属性都有）。
/// 这些**不是 emoji 图形本身**，只在序列里有意义：肤色/发色是修饰符，ZWJ/VS16 是连接件，
/// 区域指示符只有成对才是国旗 —— 单独占面板一格渲染不完整、插入也无意义，生成器因此
/// **不入库**表内那 9 条（1F3FB–1F3FF、1F9B0–1F9B3）。只入库不标记是另一种选择，
/// 被否掉的理由同上：面板是「看到才想起」的浏览网格，9 个渲染不出来的格子是纯噪声。
const UTS51_COMPONENTS: &str = "\
0023 002A 0030..0039 200D 20E3 FE0F 1F1E6..1F1FF 1F3FB..1F3FF 1F9B0..1F9B3 E0020..E007F";

/// 快照查询：段之间用空白分隔，`A..B` 是闭区间，单码位就写一个十六进制数。
fn in_snapshot(snapshot: &str, cp: u32) -> bool {
    snapshot.split_whitespace().any(|seg| {
        let (a, b) = seg.split_once("..").unwrap_or((seg, seg));
        let (a, b) = (
            u32::from_str_radix(a, 16).expect("快照里有非十六进制段"),
            u32::from_str_radix(b, 16).expect("快照里有非十六进制段"),
        );
        (a..=b).contains(&cp)
    })
}

/// 判据本身：**emoji ⟺ UTS#51 `Emoji` 属性**（扣掉修饰符/组件）。
fn is_uts51_emoji(cp: u32) -> bool {
    in_snapshot(UTS51_EMOJI, cp) && !in_snapshot(UTS51_COMPONENTS, cp)
}
