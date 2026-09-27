// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号面板覆盖面 + `SymbolEngine::builtin()` 装载开销（`--nocapture` 打印）。
//!
//! 只断言**用户可见行为**「这个输入搜得到这个符号」，不钉条数/区块编号 ——
//! 表内容归 `scripts/gen_symbols.py`，钉死内容等于把测试绑在别人的数据上
//! （与 `symbols.rs` 内联样例的取向一致）。条数与耗时只打印，不 assert：
//! 共享 CI 机器上的时间断言只会变成 flaky。

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

/// `search_emoji`（单字符输入的路径）只回非 BMP 条目 —— 与 Android
/// `SymbolCatalog.isEmoji()`（含代理对）口径一致，两边不能各说各话。
#[test]
fn emoji_search_only_returns_non_bmp() {
    let e = SymbolEngine::builtin();
    for kw in ["a", "b", "c", "face", "heart", "ballot", "star", "sm"] {
        for hit in e.search_emoji(kw) {
            assert!(
                hit.emoji,
                "{kw}: {:?} 进了 emoji 通道但 emoji 标记为假",
                hit.text
            );
            assert!(
                hit.text.chars().next().unwrap() as u32 > 0xFFFF,
                "{kw}: {:?} 进了 emoji 通道但是 BMP",
                hit.text
            );
        }
    }
}

/// **单键 emoji 通道不得被 1F000–1F2FF 的「非图形」块灌满。**
///
/// 为什么值得单独一条：`candidates.rs` 单键走 `search_emoji`，命中按 text（码位）
/// 升序平局排序，于是低码位块**必然**占满前 8 槽。而 emoji 标记口径是「非 BMP」，
/// 所以任何非 BMP 块一进表就同时进 emoji 面板与单键反馈 —— 实测把
/// 麻将牌/多米诺/扑克牌/带圈字母数字补充/带圈表意文字补充（1F000–1F2FF）
/// 加进来后，打 s 的前 8 个候选从「😀😁😃😄😅😆😇😈」变成「🀁🀌🀍🀕🀖🀞🀟🀦」。
/// 生成器因此不收这 5 块（见 scripts/gen_symbols.py 的 BLOCKS 注释）。
///
/// 这是**决策**断言，不是数据形状断言：它不钉条数、不钉具体字符，只在
/// 「有人把那 5 块加回来」时红，并把理由留在断言消息里。要恢复那 5 块，
/// 得先解决上面那条降序问题（改 `candidates.rs` 的平局规则或给符号候选封顶），
/// 届时本断言应连同生成器注释一起改。
#[test]
fn single_key_emoji_channel_has_no_non_pictograph_low_codepoints() {
    let e = SymbolEngine::builtin();
    for c in 'a'..='z' {
        for hit in e.search_emoji(&c.to_string()) {
            let cp = hit.text.chars().next().unwrap() as u32;
            assert!(
                !(0x1F000..=0x1F2FF).contains(&cp),
                "单键 {c} 的 emoji 候选里出现 U+{cp:04X} {}（1F000–1F2FF 非图形块）—— \
                 emoji 面板与单键反馈会被它占满，见本测试文档注释",
                hit.text
            );
        }
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
