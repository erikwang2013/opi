// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **文本输入准确率基准**（2026-11-01）：中英文长句 / 短句 / 词组的 top-1 / top-3。
//!
//! 数据来源全部在仓库内，不联网：
//! - 中文词条与拼音：`data/raw/extra_phrases.tsv`（`词\t拼音\torder`，已编入
//!   `android/app/src/main/assets/luna.opid`）；
//! - 英文词条：`data/raw/en_words.tsv`（`词形\t查询键\t词频`）。
//!
//! 采样是**确定性等距抽样**：先按桶计数，再取 `k * total / n` 行 —— 同一份数据
//! 永远得到同一组用例，跨机器可复现。桶按词长分：两字词 / 三到四字短语 /
//! 五到六字长词 / 七字以上长句（拼音键 ≤ `MAX_BUFFER`，否则真机一次打不完）；
//! 另有「合成短句」= 两个词条拼音直连（检验组合/分词层）与英文单词。
//!
//! 断言只锁**下界**（当前基线的向下取整）：基准的意义是防止回退，不是锁死单条
//! 名次。真正的前后对比看 `--nocapture` 打印的百分比与失败分类。

use engine_core::composer::MAX_BUFFER;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};
use engine_data::{Dictionary, load_mmap};
use std::collections::HashMap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);
const EXTRA_PHRASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/raw/extra_phrases.tsv"
);
const EN_WORDS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/en_words.tsv");

/// 每桶用例数。总量 7 个中文桶（其中「常用长句」只有 7 条）+ 英文桶 ≈ 287 条，
/// debug 下也在可接受范围。
const PER_BUCKET: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Bucket {
    ZhShort,
    ZhPhrase,
    ZhLongPhrase,
    ZhLong,
    ZhSentence,
    ZhSentence5,
    ZhSentenceReal,
    English,
}

impl Bucket {
    fn label(self) -> &'static str {
        match self {
            Bucket::ZhShort => "中文两字词",
            Bucket::ZhPhrase => "中文3-4字短语",
            Bucket::ZhLongPhrase => "中文5-6字长词",
            Bucket::ZhLong => "中文7字以上长句",
            Bucket::ZhSentence => "合成长句(两词直连)",
            Bucket::ZhSentence5 => "合成长句(五词直连)",
            Bucket::ZhSentenceReal => "中文常用长句(5-6段)",
            Bucket::English => "英文单词",
        }
    }
}

#[derive(Debug, Clone)]
struct Case {
    bucket: Bucket,
    input: String,
    expected: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    total: usize,
    top1: usize,
    top3: usize,
    absent: usize,
    not_in_dict: usize,
    segmented: usize,
}

/// 读取 `word\tpinyin\torder`，返回按桶分好的候选词条（文件序）。
fn phrase_rows() -> Vec<(String, String)> {
    std::fs::read_to_string(EXTRA_PHRASES)
        .expect("读 extra_phrases.tsv")
        .lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let word = cols.next()?;
            let pinyin = cols.next()?;
            Some((word.to_string(), pinyin.to_string()))
        })
        .collect()
}

fn bucket_of(len: usize, pinyin_len: usize) -> Option<Bucket> {
    match len {
        2 => Some(Bucket::ZhShort),
        3..=4 => Some(Bucket::ZhPhrase),
        5..=6 => Some(Bucket::ZhLongPhrase),
        7.. if pinyin_len <= MAX_BUFFER => Some(Bucket::ZhLong),
        _ => None,
    }
}

/// 按桶取等距样本。桶的成员可能极不均匀（7 字以上只有几千条），所以先数再抽。
fn sample_chinese(rows: &[(String, String)]) -> Vec<Case> {
    let mut per_bucket: HashMap<Bucket, Vec<usize>> = HashMap::new();
    for (i, (word, pinyin)) in rows.iter().enumerate() {
        if let Some(b) = bucket_of(word.chars().count(), pinyin.len()) {
            per_bucket.entry(b).or_default().push(i);
        }
    }
    let mut out = Vec::new();
    for (bucket, idx) in per_bucket {
        let n = PER_BUCKET.min(idx.len());
        for k in 0..n {
            let pos = idx[k * idx.len() / n];
            let (word, pinyin) = &rows[pos];
            out.push(Case {
                bucket,
                input: pinyin.clone(),
                expected: word.clone(),
            });
        }
    }
    // 合成短句：取两个 2-3 字词条，拼音直连。两个词都来自同一份表，期望文本
    // 是它们的直连 —— 不引入外部语料，也不假设引擎懂语法。
    let small: Vec<&(String, String)> = rows
        .iter()
        .filter(|(w, _)| w.chars().count() <= 3 && !w.contains(['—', '；', '（', '）']))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let step = small.len() / (PER_BUCKET * 2).max(1);
    let mut i = 0usize;
    while seen.len() < PER_BUCKET && i * step + 1 < small.len() {
        let a = small.get(i * step).unwrap();
        let b = small.get(i * step + 1).unwrap();
        i += 1;
        let input_len = a.1.len() + b.1.len();
        if input_len > MAX_BUFFER || a.0 == b.0 {
            continue;
        }
        let key = format!("{}{}", a.0, b.0);
        if !seen.insert(key.clone()) {
            continue;
        }
        out.push(Case {
            bucket: Bucket::ZhSentence,
            input: format!("{}{}", a.1, b.1),
            expected: key,
        });
    }
    out.sort_by_key(|c| c.bucket as u8);
    out
}

/// 常用长句（5–6 个词段、拼音 ≤32）：词段全部取自 `extra_phrases.tsv`，
/// 期望句 = 词段直连。用来量「整句一次出候选」的覆盖与排序。
const REAL_SENTENCES: &[(&str, &str)] = &[
    ("womenmingtianyiqiqugongyuan", "我们明天一起去公园"),
    ("jintiantianqihenhaowomenchuqu", "今天天气很好我们出去"),
    ("wopengyouzaibeijinggongzuo", "我朋友在北京工作"),
    ("womenyiqixuexizhongguolishi", "我们一起学习中国历史"),
    ("tamenmingtianhuijiachifan", "他们明天回家吃饭"),
    ("zhegewentiwojuedehennan", "这个问题我觉得很难"),
    ("womenmingtianyiqiqukandianying", "我们明天一起去看电影"),
];

/// 五个两字词直连（10 字、≤32 键）：用来量组合层对 5 段的覆盖。
/// 随机词对之间大多不是词库词条 ⇒ 4 段切法不存在，正好落在 `MAX_CHUNKS` 边界外。
fn sample_five_chunk_sentences(rows: &[(String, String)]) -> Vec<Case> {
    let two: Vec<&(String, String)> = rows
        .iter()
        .filter(|(w, p)| {
            w.chars().count() == 2
                && p.len() >= 2
                && w.chars().all(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
        })
        .collect();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let step = (two.len() / (PER_BUCKET * 5)).max(1);
    let mut k = 0usize;
    while out.len() < PER_BUCKET && k * step + 4 < two.len() {
        let group: Vec<&&(String, String)> = (0..5).map(|j| &two[k * step + j]).collect();
        k += 1;
        let input: String = group.iter().map(|(_, p)| p.as_str()).collect();
        if input.len() > MAX_BUFFER
            || group.iter().map(|(w, _)| w.chars().count()).sum::<usize>() != 10
        {
            continue;
        }
        let expected: String = group.iter().map(|(w, _)| w.as_str()).collect();
        if !seen.insert(expected.clone()) {
            continue;
        }
        out.push(Case {
            bucket: Bucket::ZhSentence5,
            input,
            expected,
        });
    }
    out
}

fn sample_real_sentences() -> Vec<Case> {
    REAL_SENTENCES
        .iter()
        .map(|(input, expected)| Case {
            bucket: Bucket::ZhSentenceReal,
            input: input.to_string(),
            expected: expected.to_string(),
        })
        .collect()
}

fn sample_english() -> Vec<Case> {
    let text = std::fs::read_to_string(EN_WORDS).expect("读 en_words.tsv");
    let rows: Vec<(String, String)> = text
        .lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let word = cols.next()?;
            let key = cols.next()?;
            // 单字母有「原文优先」特殊分支（engine_candidates.rs），不在本基准的
            // 词形准确率范围内；空键/非 ASCII 也不收。
            (key.len() >= 2 && key.is_ascii()).then(|| (word.to_string(), key.to_string()))
        })
        .collect();
    let n = PER_BUCKET.min(rows.len());
    (0..n)
        .map(|k| {
            let (word, key) = &rows[k * rows.len() / n];
            Case {
                bucket: Bucket::English,
                input: key.clone(),
                expected: word.clone(),
            }
        })
        .collect()
}

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine() -> Engine {
    let dict =
        load_mmap(Path::new(LUNA_OPID)).unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}"));
    Engine::new(Box::new(dict), no_symbols(), false)
}

fn candidates_of(engine: &mut Engine, input: &str) -> Vec<String> {
    engine.clear();
    for ch in input.chars() {
        engine.input_key(ch);
    }
    engine.candidates(64).into_iter().map(|c| c.text).collect()
}

/// 期望词是否真的在编译产物里（区分「数据缺词」与「排序失败」）。
fn in_dict(dict: &dyn Dictionary, input: &str, expected: &str) -> bool {
    dict.query_prefix_top(input, 0)
        .into_iter()
        .any(|e| e.pinyin_len == input.len() && e.word == expected)
}

fn classify(case: &Case, cands: &[String], in_dict: bool) -> (&'static str, Option<usize>) {
    let rank = cands
        .iter()
        .position(|c| c == &case.expected)
        .map(|i| i + 1);
    if let Some(r) = rank {
        return if r == 1 { ("ok", rank) } else { ("rank", rank) };
    }
    if matches!(
        case.bucket,
        Bucket::ZhSentence | Bucket::ZhSentence5 | Bucket::ZhSentenceReal
    ) {
        return ("composition_absent", None);
    }
    if case.bucket == Bucket::English {
        let lower = case.expected.to_lowercase();
        if cands.iter().any(|c| c.to_lowercase() == lower) {
            return ("english_case", None);
        }
        if cands
            .iter()
            .any(|c| c.starts_with(&case.input) && c.len() > case.input.len())
        {
            return ("english_longer_prefix", None);
        }
        return ("english_absent", None);
    }
    if !in_dict {
        return ("data_missing", None);
    }
    // 期望词在词库里却不在前 64：看是否是「期望词的前缀片段」占了候选
    // （分词/组合层只给了前半截），否则归为被过滤或截断。
    let prefix_hit = cands.iter().any(|c| {
        case.expected.starts_with(c.as_str()) || (c.starts_with(&case.expected) && !c.is_empty())
    });
    if prefix_hit {
        ("segmented", None)
    } else {
        ("missing_or_low", None)
    }
}

fn run_bucket(
    dict: &dyn Dictionary,
    engine: &mut Engine,
    cases: &[Case],
    stats: &mut Stats,
    categories: &mut HashMap<&'static str, Vec<String>>,
) {
    for case in cases {
        let cands = candidates_of(engine, &case.input);
        let present = in_dict(dict, &case.input, &case.expected);
        let (kind, rank) = classify(case, &cands, present);
        stats.total += 1;
        if rank == Some(1) {
            stats.top1 += 1;
        }
        if rank.is_some_and(|r| r <= 3) {
            stats.top3 += 1;
        }
        if rank.is_none() {
            stats.absent += 1;
        }
        if !present {
            stats.not_in_dict += 1;
        }
        if kind == "segmented" {
            stats.segmented += 1;
        }
        if kind != "ok" {
            categories.entry(kind).or_default().push(format!(
                "{}: 期望「{}」→ {}（输入 {}，首位 {}）",
                case.bucket.label(),
                case.expected,
                rank.map(|r| format!("第 {r} 位")).unwrap_or("缺".into()),
                case.input,
                cands.first().map(String::as_str).unwrap_or("—")
            ));
        }
    }
}

#[test]
fn text_input_accuracy_report() {
    let dict =
        load_mmap(Path::new(LUNA_OPID)).unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}"));
    let rows = phrase_rows();
    let mut cases = sample_chinese(&rows);
    cases.extend(sample_five_chunk_sentences(&rows));
    cases.extend(sample_real_sentences());
    cases.extend(sample_english());

    let mut engine = engine();
    let mut stats = Stats::default();
    let mut categories: HashMap<&'static str, Vec<String>> = HashMap::new();
    let mut by_bucket: HashMap<&'static str, Stats> = HashMap::new();

    for bucket in [
        Bucket::ZhShort,
        Bucket::ZhPhrase,
        Bucket::ZhLongPhrase,
        Bucket::ZhLong,
        Bucket::ZhSentence,
        Bucket::ZhSentence5,
        Bucket::ZhSentenceReal,
        Bucket::English,
    ] {
        if bucket == Bucket::English {
            engine.switch_mode(Mode::English);
        }
        let subset: Vec<Case> = cases
            .iter()
            .filter(|c| c.bucket == bucket)
            .cloned()
            .collect();
        let mut bstats = Stats::default();
        let mut bcat = HashMap::new();
        run_bucket(&dict, &mut engine, &subset, &mut bstats, &mut bcat);
        for (kind, mut items) in bcat {
            eprintln!("  [{}] 失败 {kind}（{}）：", bucket.label(), items.len());
            for item in items.iter().take(20) {
                eprintln!("    {item}");
            }
            categories.entry(kind).or_default().append(&mut items);
        }
        eprintln!(
            "[{}] n={} top1={}/{} ({:.1}%) top3={}/{} ({:.1}%) 缺词={} 分词={}",
            bucket.label(),
            bstats.total,
            bstats.top1,
            bstats.total,
            100.0 * bstats.top1 as f64 / bstats.total.max(1) as f64,
            bstats.top3,
            bstats.total,
            100.0 * bstats.top3 as f64 / bstats.total.max(1) as f64,
            bstats.not_in_dict,
            bstats.segmented,
        );
        by_bucket.insert(bucket.label(), bstats);
        stats.total += bstats.total;
        stats.top1 += bstats.top1;
        stats.top3 += bstats.top3;
        stats.absent += bstats.absent;
        stats.not_in_dict += bstats.not_in_dict;
        stats.segmented += bstats.segmented;
    }

    eprintln!(
        "[合计] n={} top1={} ({:.1}%) top3={} ({:.1}%) 缺词={} 未进前64={} 疑似分词={}",
        stats.total,
        stats.top1,
        100.0 * stats.top1 as f64 / stats.total.max(1) as f64,
        stats.top3,
        100.0 * stats.top3 as f64 / stats.total.max(1) as f64,
        stats.not_in_dict,
        stats.absent,
        stats.segmented,
    );
    for (kind, items) in &categories {
        eprintln!("失败分类 {kind}（{}）：", items.len());
        for item in items.iter().take(8) {
            eprintln!("  {item}");
        }
    }

    // 下界断言：按当前实测值留余量（防止环境/数据漂移），改代码后应只升不降。
    // 数值见 `--nocapture` 报告；两字词 top-1 低是同音字歧义（不是词库缺词），
    // 所以中文词条的下界按四桶合计给，不逐桶锁死。
    let zh = by_bucket["中文两字词"].top1
        + by_bucket["中文3-4字短语"].top1
        + by_bucket["中文5-6字长词"].top1
        + by_bucket["中文7字以上长句"].top1;
    assert!(zh >= 130, "中文词条 top-1 回退：{zh}/160");
    assert!(
        by_bucket["英文单词"].top1 >= 38,
        "英文 top-1 回退（整词优先）：{}/40",
        by_bucket["英文单词"].top1
    );
    assert!(
        by_bucket["合成长句(五词直连)"].top1 >= 10,
        "五词直连 top-1 回退（MAX_CHUNKS）：{}/40",
        by_bucket["合成长句(五词直连)"].top1
    );
}
