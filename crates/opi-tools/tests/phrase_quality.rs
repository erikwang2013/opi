// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 外部词库并入的语义门禁（2026-10-11，用户裁决「主流开源词库去重整合」）。
//!
//! 钉**用户要的那件事**：品牌/新词/成语/歇后语/专业术语/大词库词组打得出、排在
//! 该在的位置；**且繁体词形没有回潮**（当年 21759 条繁体成语段的老病）。
//!
//! **先证伪纪律**：本文件每颗钉（除繁体负对照的 trad 正控外）都在**并入前的
//! asset** 上跑过一次、全部 0 候选/不在候选里（证据是当时的红输出）；在新 asset
//! 上才改断言。没有这一步，钉子可能因「旧库本来就有」而通过 —— 那是观察点等价，
//! 钉的就不是本次整合。同理，断言一律走 `Engine::candidates`（不是 `dict.query`）：
//! 「层序/去重/截断」全在 candidates.rs，查 query 会让这些对门禁不可见
//! （同 `simplified_phrases.rs` / `ranking_quality.rs` 的注释）。
//!
//! 关于两档断言：
//! - **第 1 档**给品牌/新词层（人工表，层序在 base 之前）——同键无更高层竞争者；
//! - **可达档**给其余各层 —— 名次不限：层深只影响同键排序，可达性才是用户承诺。
//!
//! 繁体负对照用**字节扫描**而非 candidates：`電話` 的两个单字都在 luna 单字层
//! （電/話 是 luna 既有字形，文档写明「单字层的繁体字形一个不动」），拿候选栏判
//! 会与那条规定打架；词形级别的「0 条」用整词字节扫描最准。

use engine_core::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::load_mmap;
use std::path::Path;
use std::time::Instant;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);
const TRAD_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/trad.opid"
);

/// (拼音, 期望词)：必须是该拼音的**第 1** 个候选。品牌与网络用语两张人工表是
/// 词组三段序的**前置段**（人工 → simp → 批量，见 gen_luna_dict 头注释）——
/// 前置的理由就是本用例：`威信` 在 simp 层权重 3.71e9，坐落在批量段（`威信`
/// 实测会把 `微信` 压到第 3）。前置后同键无更高层竞争者（simp/批量段的同键
/// (key,word) 副本被去重让位，若同键还有别的词则按词频竞争 —— 目前这些键没有）。
const FIRST: &[(&str, &str)] = &[
    ("weixin", "微信"),
    ("pinduoduo", "拼多多"),
    ("neijuan", "内卷"),
    ("tangping", "躺平"),
    ("dagongren", "打工人"),
    ("xianyanbao", "显眼包"),
];

/// (拼音, 期望词)：**可达**即可（出现于候选，名次不限）。每层各选代表：
/// ext 层（短视频）、base 层（芯片/区块链）、成语层（阿鼻地狱）、歇后语层
/// （整条为词，键只由 CJK 生成）、essay 层（然後/由於 —— GB2312 成员按既有
/// 判据保留的繁体字面，判据见 gen_extra_phrases 的 `is_trad`）、THUOCL 术语层
/// （肺结核）、品牌层长词（呷哺呷哺 —— 也曾是 `吒`/OpenCC 误杀的回归钉）。
const REACHABLE: &[(&str, &str)] = &[
    ("duanshipin", "短视频"),
    ("xinpian", "芯片"),
    ("qukuailian", "区块链"),
    ("abidiyu", "阿鼻地狱"),
    ("adoudejiangshanbaisong", "阿斗的江山——白送"),
    ("ranhou", "然後"),
    ("youyu", "由於"),
    ("feijiehe", "肺结核"),
    ("xiabuxiabu", "呷哺呷哺"),
    ("nezha", "哪吒"),
];

/// 声母缩写 (缩写, 期望词)：`abbr.rs` 的整串匹配必须把它放在**第 1**。
/// 覆盖成语 / 歇后语 / 短语 / 俚语；`zmkhjjg` 与 `hslgjbnmahx` 命中的是整条
/// 歇后语词形（前缀档），断言用库里的完整词。
const ABBR_FIRST: &[(&str, &str)] = &[
    ("yffs", "一帆风顺"),
    ("hstz", "画蛇添足"),
    ("szdt", "守株待兔"),
    ("wybl", "亡羊补牢"),
    ("jdzw", "井底之蛙"),
    ("yedl", "掩耳盗铃"),
    ("zxmd", "自相矛盾"),
    ("kzqj", "刻舟求剑"),
    ("bgsy", "杯弓蛇影"),
    ("abdy", "阿鼻地狱"),
    ("bhys", "不好意思"),
    ("mmqm", "莫名其妙"),
    ("ssqs", "实事求是"),
    ("jjz", "绝绝子"),
    ("addjsbs", "阿斗的江山——白送"),
    ("zmkhjjg", "芝麻开花——节节高；节节上升"),
    ("hslgjbnmahx", "黄鼠狼给鸡拜年——没安好心；来者不善"),
];

/// 全拼 (拼音, 期望词)：成语 / 歇后语 / 短语 / 俚语各取代表，必须在**第 1**。
/// 后两条的输入是整条歇后语的**前半截**（词形还长着「；后半截」）：它验证
/// `candidates_segment` 的「音节边界对齐的长词前缀」档 —— 没有它时长词会被第一个
/// 音节（`zhi` 有 9813 条）的整档替换，甚至因前缀档截断而**整个消失**。
const FULL_PHRASE_FIRST: &[(&str, &str)] = &[
    ("yifanfengshun", "一帆风顺"),
    ("huashetianzu", "画蛇添足"),
    ("shouzhudaitu", "守株待兔"),
    ("adoudejiangshanbaisong", "阿斗的江山——白送"),
    ("zhimakaihuajiejiegao", "芝麻开花——节节高；节节上升"),
    (
        "huangshulanggeijibainianmeianhaoxin",
        "黄鼠狼给鸡拜年——没安好心；来者不善",
    ),
    ("buhaoyisi", "不好意思"),
    ("momingqimiao", "莫名其妙"),
    ("shishiqiushi", "实事求是"),
    ("juejuezi", "绝绝子"),
];

/// 高频两字母缩写 (缩写, 期望词, 允许名次)：同声母的词多，不苛求第 1，但必须在
/// 首屏内（用户打 `nh`/`zg` 的意图就是这两个词）。
const ABBR_VISIBLE: &[(&str, &str, usize)] = &[
    ("nh", "你好", 3),
    ("zg", "中国", 2),
    // 混合输入不是缩写（`nih` 是 `nihao` 的拼音前缀）⇒ 走既有前缀路径；这里只钉
    // 「本档没有把它挤掉」的可见性下界（实测第 6）。
    ("nih", "你好", 8),
];

/// 非缩写负对照：(输入, 期望词, 允许名次)。这些键在主查询里有前缀命中，
/// `abbr.rs` 必须一步不跑 —— 常用字词的名次与本轮之前一致。
const PREFIX_CONTROLS: &[(&str, &str, usize)] = &[
    ("ni", "你", 1),
    ("nihao", "你好", 1),
    ("zhongguo", "中国", 1),
    ("women", "我们", 1),
    ("xian", "先", 3),
];

/// 繁体回潮负对照：(拼音, 繁体词形)。luna.opid **字节扫描 0 条**，
/// **trad.opid 字节扫描 > 0**（正控：否则「0」可能只是扫描/编码写错）。
const TRAD_CONTROL: &[(&str, &str)] = &[
    ("dianhua", "電話"),
    ("xuesheng", "學生"),
    ("shijian", "時間"),
    ("women", "我們"),
    ("shuofa", "說法"),
];

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine_at(path: &str) -> Engine {
    let dict = load_mmap(Path::new(path)).unwrap_or_else(|e| panic!("加载 {path} 失败：{e:?}"));
    Engine::new(Box::new(dict), no_symbols(), false)
}

/// 敲入拼音并取候选（`Engine::candidates` = candidates.rs `rank_and_pick`）。
fn candidates_of(engine: &mut Engine, pinyin: &str) -> Vec<String> {
    engine.clear();
    for ch in pinyin.chars() {
        engine.input_key(ch);
    }
    engine
        .candidates(usize::MAX)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

/// 当前**线程**已用 CPU 毫秒（Linux `/proc/thread-self/stat`，USER_HZ=100）。
/// 时延门禁用 CPU 而不是墙钟：开发机/CI 可能被别的构建占满，墙钟会把「被调度走」
/// 记成我们的成本（本机 load ~50/8 核时同一调用墙钟 30–90ms、CPU 只有几毫秒）；
/// 空闲机单线程上两者同量级。读不到 `/proc` 时返回 `None`，回退墙钟。
#[cfg(target_os = "linux")]
fn thread_cpu_ms() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/thread-self/stat").ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ut: u64 = f.get(11)?.parse().ok()?;
    let st: u64 = f.get(12)?.parse().ok()?;
    Some((ut + st) as f64 * 10.0)
}

/// 敲前缀并取 8 条候选（只计装配成本，不物化整页）。
fn candidates_head(engine: &mut Engine, prefix: &str) -> usize {
    engine.clear();
    for ch in prefix.chars() {
        engine.input_key(ch);
    }
    engine.candidates(8).len()
}

/// 单次「敲到该前缀 + 取候选」的成本（ms）。Linux 用线程 CPU 时间（5 次批量摊薄
/// 10ms 的 tick 粒度）；其他平台用墙钟中位数。
fn prefix_cost(engine: &mut Engine, prefix: &str) -> f64 {
    #[cfg(target_os = "linux")]
    {
        const REPS: usize = 5;
        if let Some(t0) = thread_cpu_ms() {
            for _ in 0..REPS {
                std::hint::black_box(candidates_head(engine, prefix));
            }
            if let Some(t1) = thread_cpu_ms() {
                return (t1 - t0) / REPS as f64;
            }
        }
    }
    let mut samples = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        std::hint::black_box(candidates_head(engine, prefix));
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

#[test]
fn merged_brand_and_slang_words_rank_first() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (py, word) in FIRST {
        let cands = candidates_of(&mut engine, py);
        let got = cands.first().map(String::as_str).unwrap_or("—");
        if got != *word {
            let pos = cands
                .iter()
                .position(|c| c == word)
                .map(|i| (i + 1).to_string())
                .unwrap_or_else(|| "缺".into());
            fails.push(format!(
                "{py}: 期望第 1 是「{word}」，实得「{got}」（期望词名次 {pos}）"
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "并入层品牌/新词缺失或不在首位（{}/{}）：\n  {}",
        fails.len(),
        FIRST.len(),
        fails.join("\n  ")
    );
}

#[test]
fn merged_layers_are_reachable() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (py, word) in REACHABLE {
        let cands = candidates_of(&mut engine, py);
        if !cands.iter().any(|c| c == word) {
            fails.push(format!(
                "{py}: 候选里没有「{word}」（共 {} 个候选）",
                cands.len()
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "并入层条目不可达（{}/{}）：\n  {}",
        fails.len(),
        REACHABLE.len(),
        fails.join("\n  ")
    );
}

#[test]
fn full_pinyin_phrases_rank_first() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (py, word) in FULL_PHRASE_FIRST {
        let cands = candidates_of(&mut engine, py);
        let got = cands.first().map(String::as_str).unwrap_or("—");
        if got != *word {
            let pos = cands
                .iter()
                .position(|c| c == word)
                .map(|i| (i + 1).to_string())
                .unwrap_or_else(|| "缺".into());
            fails.push(format!(
                "{py}: 期望第 1 是「{word}」，实得「{got}」（期望词名次 {pos}）"
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "全拼长词没有排到首位（{}/{}）：\n  {}",
        fails.len(),
        FULL_PHRASE_FIRST.len(),
        fails.join("\n  ")
    );
}

#[test]
fn abbreviation_phrases_rank_first() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (abbr, word) in ABBR_FIRST {
        let cands = candidates_of(&mut engine, abbr);
        let got = cands.first().map(String::as_str).unwrap_or("—");
        if got != *word {
            let pos = cands
                .iter()
                .position(|c| c == word)
                .map(|i| (i + 1).to_string())
                .unwrap_or_else(|| "缺".into());
            fails.push(format!(
                "{abbr}: 期望第 1 是「{word}」，实得「{got}」（期望词名次 {pos}）"
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "声母缩写没有排到首位（{}/{}）：\n  {}",
        fails.len(),
        ABBR_FIRST.len(),
        fails.join("\n  ")
    );

    let mut fails = Vec::new();
    for (abbr, word, max_pos) in ABBR_VISIBLE {
        let cands = candidates_of(&mut engine, abbr);
        let pos = cands.iter().position(|c| c == word);
        if pos.is_none_or(|i| i + 1 > *max_pos) {
            fails.push(format!(
                "{abbr}: 「{word}」名次 {pos:?}（要求 ≤{max_pos}），head={:?}",
                cands.iter().take(8).collect::<Vec<_>>()
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "高频缩写/混合输入不可见（{}/{}）：\n  {}",
        fails.len(),
        ABBR_VISIBLE.len(),
        fails.join("\n  ")
    );
}

#[test]
fn prefix_inputs_are_not_displaced_by_abbr() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (input, word, max_pos) in PREFIX_CONTROLS {
        let cands = candidates_of(&mut engine, input);
        let pos = cands.iter().position(|c| c == word);
        if pos.is_none_or(|i| i + 1 > *max_pos) {
            fails.push(format!(
                "{input}: 「{word}」名次 {pos:?}（要求 ≤{max_pos}），head={:?}",
                cands.iter().take(8).collect::<Vec<_>>()
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "常用字词被缩写档挤出（{}/{}）：\n  {}",
        fails.len(),
        PREFIX_CONTROLS.len(),
        fails.join("\n  ")
    );
}

/// **release 时延门禁**：缩写键从第 2 击起，逐键 <30ms（用户「所有响应<30ms」）。
/// 第 1 击是既有的单字母前缀路径（`abbr.rs` 不接手），不归本档；debug 档位无优化，
/// 只打印不判红（与本仓 `wubi.rs` 的成本证据同一惯例）。
#[test]
fn abbreviation_keys_stay_under_30ms_release() {
    let mut engine = engine_at(LUNA_OPID);
    let keys = [
        "yffs",
        "hstz",
        "zmkhjjg",
        "nh",
        "zg",
        "addjsbs",
        "hslgjbnmahx",
        "qqqq",
    ];
    let mut over = Vec::new();
    let mut report = Vec::new();
    for k in keys {
        for pos in 2..=k.chars().count() {
            let prefix: String = k.chars().take(pos).collect();
            std::hint::black_box(candidates_head(&mut engine, &prefix)); // 预热
            let ms = prefix_cost(&mut engine, &prefix);
            report.push(format!("{prefix}:{ms:.2}ms"));
            if !cfg!(debug_assertions) && ms >= 30.0 {
                over.push(format!("{prefix}: {ms:.2}ms"));
            }
        }
    }
    println!("[abbr] 逐键成本：{}", report.join(" "));
    assert!(
        over.is_empty(),
        "缩写键超出 30ms（release）：\n  {}",
        over.join("\n  ")
    );
}

/// 解析出的**词条**（不是原始字节）：2026-10-27 起 luna.opid 带 assoc 尾部，
/// 对整文件做字节扫描会把小表里的后缀误当词条。
fn words(path: &str) -> Vec<String> {
    let bytes = std::fs::read(path).expect("读 opid");
    engine_data::parse(&bytes)
        .expect("opid 必须通过校验")
        .entries
        .into_iter()
        .map(|e| e.word)
        .collect()
}

#[test]
fn no_traditional_word_forms_leaked_into_luna() {
    let luna = words(LUNA_OPID);
    let trad = words(TRAD_OPID);
    let mut fails = Vec::new();
    for (py, word) in TRAD_CONTROL {
        // 正控先行：trad.opid 里扫不到 = 扫描本身有问题，先红在正控上。
        assert!(
            trad.iter().any(|w| w.as_str() == *word),
            "正控失败：trad.opid 里扫不到「{word}」—— 扫描/编码写错了，否定断言无意义"
        );
        if luna.iter().any(|w| w.as_str() == *word) {
            fails.push(format!("{py}: luna.opid 里出现了繁体词形「{word}」"));
        }
    }
    assert!(
        fails.is_empty(),
        "并入源把繁体词形带回了简洁库（{}/{}）：\n  {}",
        fails.len(),
        TRAD_CONTROL.len(),
        fails.join("\n  ")
    );
}
