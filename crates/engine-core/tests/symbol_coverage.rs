// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号面板覆盖面 + `SymbolEngine::builtin()` 装载开销（`--nocapture` 打印）。
//!
//! 只断言**用户可见行为**「这个输入搜得到这个符号」，不钉条数/区块编号 ——
//! 表内容归 `scripts/gen_symbols.py`，钉死内容等于把测试绑在别人的数据上
//! （与 `symbols.rs` 内联样例的取向一致）。条数与耗时只打印，不 assert：
//! 共享 CI 机器上的时间断言只会变成 flaky。
//!
//! 例外是 `emoji_flag_follows_uts51_property`：emoji 列不是「别人的数据」而是判据
//! 本身（决定「表情」页与单键反馈的内容），判错就是用户可见的错，故在这里钉规则。

use engine_core::symbols::SymbolEngine;

/// (输入关键字, 期望候选里必须有的字符, 该字符的 U+ 码位)：每个新增区块至少一条。
/// 关键字取自 Unicode 名分词（生成器对未手写条目的兜底路径），故这些用例钉的是
/// 「新块确实进了关键字索引」，不只是「进了文件」。
const SMOKE: &[(&str, &str, u32)] = &[
    ("dun", "、", 0x3001),            // 1 CJK 符号（旧块防回归）
    ("ballot", "☑", 0x2611),          // 3 杂项符号（旧块防回归）
    ("heart", "♥", 0x2665),           // 3 旧表必须活着的 8 条之一
    ("arrow", "←", 0x2190),           // 7 箭头
    ("watch", "⌚", 0x231A),          // 8 杂项技术符号
    ("circled", "①", 0x2460),         // 9 带圈字母数字
    ("scissors", "✂", 0x2702),        // 10 装饰符号（关键字取自 Unicode 名，非块名）
    ("curving", "⤴", 0x2934),         // 11 补充箭头-B
    ("star", "⭐", 0x2B50),           // 12 杂项符号与箭头
    ("congratulation", "㊗", 0x3297), // 13 带圈中日韩字母
    ("scroll", "📜", 0x1F4DC),        // 14 杂项符号与图形
    ("rocket", "🚀", 0x1F680),        // 15 交通与地图符号
    ("orange", "🟠", 0x1F7E0),        // 16 几何图形扩展
    ("soap", "🧼", 0x1F9FC),          // 17 补充符号与图形（词干靠名字分词，别按中文语义猜）
    ("maracas", "🪇", 0x1FA87),       // 18 符号与图形扩展-A（Unicode 15.0 新增）
];

/// UTS#51 `Emoji` 属性区间快照 —— Unicode **18.0.0** `emoji-data.txt` 里 `Emoji` 行的
/// 第一列**逐行照抄**（424 段，段序与上游文件相同；`A..B` 是闭区间，单码位只写一个）。
///
/// 生成器 `scripts/gen_symbols.py` 用**同一份文件、同一个 SHA-256 pin** 算 emoji 列，
/// 这里抄一份独立副本，于是三种漂移都会红：判据被改回代理指标、`symbols.tsv` 被手改、
/// 上游换版而两边没同步。**`crates/opi-tools/tests/rare_coverage.rs` 末尾有一份逐字节
/// 相同的拷贝**（那边守数据形状，两边都对着同一份 symbols.tsv 断言 ⇒ 漂移必红；
/// 换版时两处 + 生成器 pin 一起改）。复算（升级 Unicode 时照跑，输出直接换掉下面这段）：
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

/// UTS#51 `Emoji_Modifier` ∪ `Emoji_Component`（**去重并集 10 段**；两个属性直接拼会得到
/// 11 段，因为 1F3FB..1F3FF 两个属性都有 —— 比对上游时别按「拼起来」比）。
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

#[test]
fn new_blocks_are_searchable() {
    let e = SymbolEngine::builtin();
    let mut missing = Vec::new();
    for (kw, ch, cp) in SMOKE {
        let hits = e.search(kw);
        if !hits.iter().any(|h| h.text == *ch) {
            let got: Vec<&str> = hits.iter().take(6).map(|h| h.text.as_str()).collect();
            missing.push(format!("{kw:10} → 找不到 U+{cp:04X} {ch}（命中 {got:?}）"));
        }
    }
    assert!(
        missing.is_empty(),
        "新增区块未真正进关键字索引：\n  {}",
        missing.join("\n  ")
    );
}

/// **emoji 列 ⟺ UTS#51 `Emoji` 属性** —— 钉规则，不钉码位段。
///
/// 为什么值得一条门禁：`candidates.rs` 单键走 `search_emoji`，emoji 标记同时决定
/// 「表情」页与单键反馈的内容，判错就是用户可见的错。旧判据是**代理指标**
/// 「码位 > 0xFFFF」（与 Android `SymbolCatalog.isEmoji()` 的「含代理对」同构），
/// 两头都错：1F780–1F7FF（几何图形扩展）120 条方框/斜叉/三角全被当成表情
/// （打 x 的前 8 槽曾是 🖾🗙🗴🗵🗶🗷 六个几何字形），而 BMP 的真 emoji
/// （☺ U+263A、♥ U+2665、⌚ U+231A、⭐ U+2B50、✂ U+2702）反而不进表情页。
///
/// 换成属性判据后**低码位不再是风险来源**：进不进 emoji 通道只看 Unicode 属性。
/// 旧版这条门禁钉的是「1F000–1F2FF 这 5 个块」的清单，钉不住规则 ——
/// 生成器只要收第 6 个非图形块它就失效（1F780 正是这么漏过去的）。
#[test]
fn emoji_flag_follows_uts51_property() {
    let all = SymbolEngine::builtin().search("");
    assert!(!all.is_empty(), "builtin() 一条都没装进来，本门禁失去意义");
    let (mut wrong, mut comps) = (Vec::new(), Vec::new());
    for e in &all {
        let cp = e.text.chars().next().unwrap() as u32;
        if is_uts51_emoji(cp) != e.emoji {
            wrong.push(format!(
                "U+{cp:04X} {} emoji={} 应为 {}",
                e.text,
                e.emoji,
                is_uts51_emoji(cp)
            ));
        }
        if in_snapshot(UTS51_COMPONENTS, cp) {
            comps.push(format!("U+{cp:04X} {}", e.text));
        }
    }
    // 不符项可能成百（判据整体换过一次），只列前 10 条，数量另报。
    let brief = |v: &Vec<String>| -> String {
        let head: Vec<&str> = v.iter().take(10).map(String::as_str).collect();
        if v.len() > head.len() {
            format!(
                "{}\n  … 其余 {} 条",
                head.join("\n  "),
                v.len() - head.len()
            )
        } else {
            head.join("\n  ")
        }
    };
    assert!(
        wrong.is_empty(),
        "emoji 列与 UTS#51 Emoji 属性不符 {} 条（表由 scripts/gen_symbols.py 生成；\
         判据若有意改动，两边同步后再改本测试的 UTS51_EMOJI 快照）：\n  {}",
        wrong.len(),
        brief(&wrong)
    );
    assert!(
        comps.is_empty(),
        "修饰符/组件混进了符号表 {} 条（单独占一格渲染不完整，见 UTS51_COMPONENTS 注释）：\n  {}",
        comps.len(),
        brief(&comps)
    );
}

/// 单键 emoji 通道（`search_emoji`，`candidates.rs` 单字符输入走这条）只回
/// UTS#51 `Emoji` 属性的字符 —— **含 BMP 区**。这与 Android
/// `SymbolCatalog.isEmoji()`（「含代理对」）是同一契约的两端：旧启发式在 BMP emoji
/// 上必然漏（☺ U+263A、♥ U+2665 这些数据里 emoji=1、那边判 false），
/// 故 Android 侧同时改走 emoji 标记（见 fix-android）。
#[test]
fn emoji_channel_follows_the_property_including_bmp() {
    let e = SymbolEngine::builtin();
    for kw in ["a", "b", "c", "face", "heart", "ballot", "star", "sm"] {
        for hit in e.search_emoji(kw) {
            let cp = hit.text.chars().next().unwrap() as u32;
            assert!(
                hit.emoji,
                "{kw}: {:?} 进了 emoji 通道但 emoji 标记为假",
                hit.text
            );
            assert!(
                is_uts51_emoji(cp),
                "{kw}: {:?} U+{cp:04X} 进了 emoji 通道但不是 UTS#51 Emoji —— \
                 单键反馈/表情页会被非图形字符占满，见本文件 UTS51_EMOJI",
                hit.text
            );
        }
    }
    // 新口径的**用户可见收益**：BMP 的真 emoji 够得到了（旧口径下 emoji=0，
    // 单键与表情页都取不到）。♥ 走关键字 ai/xin、☺ 走 xiaolian。
    for (kw, ch) in [("a", "♥"), ("x", "☺")] {
        assert!(
            e.search_emoji(kw).iter().any(|h| h.text == ch),
            "单键 {kw} 取不到 BMP emoji {ch}（U+{:04X}）—— 新判据在 BMP 区的收益丢了",
            ch.chars().next().unwrap() as u32
        );
    }
}

/// 装载开销：`builtin()` 编译期嵌入、无 IO，构造发生在进程启动/面板首次打开。
/// 打印中位数（5 次，含一次预热丢弃惰性页错误）。
#[test]
fn builtin_build_time_is_reported() {
    let _ = SymbolEngine::builtin();
    let mut samples: Vec<f64> = (0..5)
        .map(|_| {
            let t = std::time::Instant::now();
            let e = SymbolEngine::builtin();
            std::hint::black_box(&e);
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let all = SymbolEngine::builtin().search("");
    let emoji = all.iter().filter(|s| s.emoji).count();
    println!(
        "[symbols] builtin(): 中位 {:.2}ms（样本 {}）",
        samples[2],
        samples
            .iter()
            .map(|d| format!("{d:.2}"))
            .collect::<Vec<_>>()
            .join("/")
    );
    println!("[symbols] 合计 {} 条 / emoji {} 条", all.len(), emoji);
}
