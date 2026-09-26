// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 排序质量门禁（spec 2026-08-15 验收偏差 #4）：拼音候选的首位必须是真实高频字，
//! 而不是 GB2312 码序（布局序）首字——旧数据下 wo→蜗、ni→呢、hao→镐、shi→匙。
//!
//! 数据来源：`scripts/gen_luna_dict.py` / `scripts/gen_trad_dict.py`，词频取自
//! Unihan `kHanyuPinlu`（逐读音语料计数，见 data/raw/LICENSES.md）；候选由 `Engine`
//! （= candidates.rs `rank_and_pick`）产出：**精确等长匹配整体排在前缀扩展之前**（精确
//! 加成 = boost/2 ≥ 词典最大静态词频），同一层内按 freq 降序，故层内「TSV 行序 == 候选栏顺序」。
//!
//! 三档断言：
//! - STRICT：期望字必须第 1；
//! - NEAR：期望字进前 3。用于同音字真实分布本身接近或前缀匹配带入更长音节的高频字
//!   （数字为 kHanyuPinlu 词次：guo 国 6416 / 过 6396 仅差 20 次；dian 点 3816 / 电 1775；
//!   zi 子 7967 / 自 4664；wan 完 1502 / 万 1335；ma 妈 2007 / 吗 1549（且 ma ⊂ mai/man）；
//!   li 里/裡/裏 同继承 8821，字形取向取 裏、Unihan 变体位次取 裡）。
//!   注：原属本档的 me（们 14950 / 么 8053）2026-09-26 随精确分层升入 STRICT。
//!
//! 已知不满足、刻意**不**断言的项（保留记录，勿悄悄放宽）：
//! - xie→谢/謝：些 3689 > 写 1171 > 谢 289，三者同属 "xie" 的真实分布；旧数据靠
//!   COMMON_TRAD 人工白名单把 寫/謝 抬到首位，白名单退役后回归数据事实（谢 名次仍从
//!   luna 第 22 位改善到第 4 位，只是不再是第 1）。
//! - xian→先：语料上 现 3900 > 先 1962（同为 "xian" 读音），先 实测第 3（第 2 是 現）。
//!   **旧注释记的「先 第 6、首位是想(xiang 4640)」已作废** —— 那是精确加成落地前的行为：
//!   加成分层后前缀扩展的 想 落到第 450 位（luna），先 由第 6 升到第 3；仍非第 1 是语料
//!   事实（现 > 先），不是引擎缺陷。
//! - 时→時：与 是/是 同音（shi，是 28130 / 時 6805），首位是 是。
//!
//! ## 读音可达性（REACH 档，2026-09-26 加）
//!
//! 缺陷：上游 luna 单字表把**常用读音标成 0% 份额、把罕用读音标成 100%**
//! （开 jian 95% / kai 0%；备 bei 0% / yuan 100%；广 guang 0% / yan 100%），
//! 而 `gen_luna_dict.py` 的 0% 过滤器把 0% 读音整条丢弃 —— 「开」于是只能由 jian
//! 打出来。**上游 0% 的含义是「该读音在语料里的份额为 0」，不等于「该读音不存在」；
//! kHanyuPinlu 逐读音词次才是读音是否存在的证据。** 现行规则：有词次证据的读音一律
//! 保留（0% 过滤器只对无证据读音生效）。
//!
//! 系统性检出（字 × 读音 与 kHanyuPinlu 逐读音词次比对，证据读音在产物中缺失即缺陷）：
//! luna 42 字 / 44 对，其中 38 字 38 对属「上游列了、被 0% 过滤」（下表 REACH 全覆盖），
//! 另 5 字 6 对属「上游根本没列该读音」（儿 r / 兒 r / 嗯 n,ng / 琢 zuo / 寻 xin）——
//! 后者不是 0% 过滤器造成，且含非音节形（r、n、ng 儿化/叹词），**刻意不自动补**
//! （补了会让 luna 的 "r" 直接出「儿」），记录在此不静默处理。
//! trad 23 字 / 23 对，全部是后者（0 例受 0% 过滤影响：trad 的读音全集来自 kMandarin，
//! 本就没有 0% 过滤），且多为简繁同字分工（长 chang 缺，但繁体侧 長 在），故不在本
//! 门禁断言范围内。
//!
//! 产物：本测试只读不联网。luna 取 android 部署副本（data/generated/luna.opid 是
//! gitignore 的构建中间物），trad 取入库产物 data/generated/trad.opid。

use engine_core::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::load_mmap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);
const TRAD_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/generated/trad.opid"
);
const TRAD_ASSET: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/trad.opid"
);

/// (拼音, 简体期望字, 繁体期望字)：期望字必须是该前缀查询的第 1 个候选。
const STRICT: &[(&str, &str, &str)] = &[
    ("wo", "我", "我"),
    ("ni", "你", "你"),
    ("hao", "好", "好"),
    ("de", "的", "的"),
    ("shi", "是", "是"),
    ("bu", "不", "不"),
    ("le", "了", "了"),
    ("zai", "在", "在"),
    ("you", "有", "有"),
    ("ren", "人", "人"),
    ("yi", "一", "一"),
    ("ta", "他", "他"),
    ("kan", "看", "看"),
    ("dao", "到", "到"),
    ("zhong", "中", "中"),
    ("da", "大", "大"),
    ("xiao", "小", "小"),
    ("shang", "上", "上"),
    ("jiu", "就", "就"),
    ("dou", "都", "都"),
    ("hen", "很", "很"),
    ("yao", "要", "要"),
    ("qu", "去", "去"),
    ("mei", "没", "沒"),
    ("hui", "会", "會"),
    ("ge", "个", "個"),
    ("lai", "来", "來"),
    ("dui", "对", "對"),
    ("zhe", "这", "這"),
    ("men", "们", "們"),
    ("shuo", "说", "說"),
    ("hua", "话", "話"),
    ("jian", "见", "見"),
    ("xin", "心", "心"),
    ("shou", "手", "手"),
    ("tian", "天", "天"),
    ("nian", "年", "年"),
    ("xiang", "想", "想"),
    ("neng", "能", "能"),
    ("duo", "多", "多"),
    ("ri", "日", "日"),
    ("chu", "出", "出"),
    ("xia", "下", "下"),
    ("nv", "女", "女"),
    ("san", "三", "三"),
    ("wei", "为", "為"),
    ("shui", "水", "水"),
    ("hou", "后", "後"),
    ("zhi", "只", "只"),
    ("mian", "面", "面"),
    ("cai", "才", "才"),
    ("fa", "发", "發"),
    ("xue", "学", "學"),
    ("hai", "还", "還"),
    ("xing", "行", "行"),
    ("zui", "最", "最"),
    // kai：开 词次 3483，同组次高 慨 33 / 揩 12 —— 由「读音权威来源」规则修复后方可达。
    ("kai", "开", "開"),
    // 以下四项锁**精确等长匹配分层**（candidates.rs：精确加成 boost/2 ≥ 词典最大静态词频）：
    // 期望字与该拼音等长，靠前的曾经/可能是更长的前缀扩展（men ⊂ me、xiang ⊂ xian、
    // shuo ⊂ shu、xia ⊂ xi），加成分层后必然第 1 —— 是确定性结论，故要求第 1 而非前 3。
    // me 原在 NEAR（们 14950 / 么 8053），2026-09-26 精确分层落地后两库实测均第 1。
    ("me", "么", "麼"),
    ("xian", "现", "現"), // 想 xiang 4640 曾是首位；分层后想 落到第 450 位（luna），现 第 1
    ("shu", "书", "書"),  // 说 shuo 曾是首位
    ("xi", "西", "西"),   // 下 xia 曾是首位
];

/// (拼音, 简体期望字, 繁体期望字)：期望字进前 3 即可（同音分布接近 / 前缀扩展）。
const NEAR: &[(&str, &str, &str)] = &[
    ("guo", "过", "過"),  // 國 6416 / 過 6396：差 20 次（0.3%），语料近乎并列
    ("dian", "电", "電"), // 點 3816 / 電 1775
    ("zi", "自", "自"),   // 子 7967 / 自 4664
    ("ma", "吗", "嗎"),   // 媽 2007 / 嗎 1549（且 ma ⊂ mai/man 前缀扩展）
    ("wan", "万", "萬"),  // 完 1502 / 萬 1335
    ("li", "里", "裡"),   // 裏/裡/里 同继承 8821：字形取向取 裏（terra 港式语料），裡 第 2
];

/// (拼音, 简体期望字, 繁体期望字)：期望字必须**能到达**（名次 ≥ 1 即可，不限前几）。
/// 表内 37 项 = 系统性检出的 38 个「0% 过滤器吃掉有证据读音」字，去掉 开（已进 STRICT）。
/// 名次高低是排序质量问题，此处锁的是**存在性**：这些读音曾整条从产物里消失。
const REACH: &[(&str, &str, &str)] = &[
    ("ye", "业", "业"),
    ("xiang", "乡", "乡"),
    ("kui", "亏", "亏"),
    ("qin", "亲", "亲"),
    ("jin", "仅", "仅"),
    ("er", "儿", "儿"),
    ("guan", "关", "关"),
    ("gang", "冈", "冈"),
    ("bo", "卜", "卜"),
    ("ye", "叶", "叶"),
    ("sheng", "圣", "圣"),
    ("huai", "坏", "坏"),
    ("ken", "垦", "垦"),
    ("bei", "备", "备"),
    ("ning", "宁", "宁"),
    ("xian", "宪", "宪"),
    ("guang", "广", "广"),
    ("huai", "怀", "怀"),
    ("lian", "怜", "怜"),
    ("jing", "惊", "惊"),
    ("rao", "扰", "扰"),
    ("di", "敌", "敌"),
    ("za", "杂", "杂"),
    ("quan", "权", "权"),
    ("han", "汉", "汉"),
    ("jie", "洁", "洁"),
    ("yong", "涌", "涌"),
    ("lie", "猎", "猎"),
    ("ji", "积", "积"),
    ("jian", "茧", "茧"),
    ("rong", "荣", "荣"),
    ("lv", "虑", "虑"),
    ("la", "蜡", "蜡"),
    ("chu", "触", "触"),
    ("shi", "适", "适"),
    ("you", "邮", "邮"),
    ("li", "隶", "隶"),
];

/// 空符号表：本门禁只测拼音候选，符号数据来自生成的文件，不参与这里的断言。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 敲入拼音并取候选（`Engine::candidates` = `rank_and_pick`，排序规则所在层）。
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

/// 期望字在**引擎候选**中的名次（1 起，按引擎顺序去重）；0 = 无候选。
///
/// 必须走 `Engine` 而不是 `dict.query`：prefix 检索只是词典层，排序规则（精确等长加成、
/// learner boost、去重、截断）全在 candidates.rs，查 query 会让这些缺陷对门禁**不可见**
/// —— v1.0.10 的「limit 下推」与 2026-09-26 的「精确 vs 前缀扩展」都栽在这条盲区上。
/// boost 由 `Engine::with_dictionaries` 按词典 max_freq 推出，与真机同源。
fn rank_of(engine: &mut Engine, pinyin: &str, expected: &str) -> usize {
    let mut seen = std::collections::HashSet::new();
    let mut rank = 0;
    for text in candidates_of(engine, pinyin) {
        if !seen.insert(text.clone()) {
            continue;
        }
        rank += 1;
        if text == expected {
            return rank;
        }
    }
    0
}

fn top_word(engine: &mut Engine, pinyin: &str) -> String {
    candidates_of(engine, pinyin)
        .into_iter()
        .next()
        .unwrap_or_else(|| "—".into())
}

/// 跑两档表；trad=true 取繁期待字。首位命中率一并打印（报告用）。
fn gate(label: &str, path: &str, trad: bool) {
    let dict = load_mmap(Path::new(path)).unwrap_or_else(|e| panic!("加载 {path} 失败：{e:?}"));
    // learner 关闭：门禁锁静态排序；学习后的反超由 engine-core 单测覆盖（否则跑两次结果不同）。
    let mut engine = Engine::new(Box::new(dict), no_symbols(), false);
    let (mut first, mut fails, mut total) = (0usize, Vec::new(), 0usize);
    for (tier, max_rank) in [(STRICT, 1usize), (NEAR, 3)] {
        for (py, simp, tr) in tier {
            let exp = if trad { tr } else { simp };
            let rank = rank_of(&mut engine, py, exp);
            total += 1;
            if rank == 1 {
                first += 1;
            }
            if rank == 0 || rank > max_rank {
                fails.push(format!(
                    "{py}: 期望 {exp} → 第 {} 位（当前首位 {}，要求前 {max_rank}）",
                    if rank == 0 {
                        "缺".into()
                    } else {
                        rank.to_string()
                    },
                    top_word(&mut engine, py)
                ));
            }
        }
    }
    // 可达性档：只要名次 ≥ 1。计入 fails、不计入首位命中率（分母与 STRICT+NEAR 不同源）。
    let (mut reach_ok, mut reach_total) = (0usize, 0usize);
    for (py, simp, tr) in REACH {
        let exp = if trad { tr } else { simp };
        reach_total += 1;
        if rank_of(&mut engine, py, exp) > 0 {
            reach_ok += 1;
        } else {
            fails.push(format!(
                "{py}: 期望 {exp} → 无候选（读音在产物中缺失，当前首位 {}）",
                top_word(&mut engine, py)
            ));
        }
    }
    println!("[{label}] 首位命中 {first}/{total}；读音可达 {reach_ok}/{reach_total}");
    assert!(
        fails.is_empty(),
        "[{label}] 排序质量不达标（{}/{}）：\n  {}",
        fails.len(),
        total + reach_total,
        fails.join("\n  ")
    );
}

#[test]
fn luna_ranking_quality() {
    gate("luna 简体", LUNA_OPID, false);
}

#[test]
fn trad_ranking_quality() {
    gate("trad 繁体", TRAD_OPID, true);
}

/// 两份 trad.opid（入库产物 + android 部署副本）必须逐字节一致：数据改了只更新一份
/// 会让真机与测试看到不同的词库。
#[test]
fn trad_assets_in_sync() {
    let committed = std::fs::read(TRAD_OPID).unwrap_or_else(|e| panic!("读 {TRAD_OPID} 失败：{e}"));
    let asset = std::fs::read(TRAD_ASSET).unwrap_or_else(|e| panic!("读 {TRAD_ASSET} 失败：{e}"));
    assert_eq!(
        committed.len(),
        asset.len(),
        "两份 trad.opid 大小不同：{} vs {}",
        committed.len(),
        asset.len()
    );
    assert_eq!(
        committed, asset,
        "两份 trad.opid 内容不同（重新生成并同步部署副本）"
    );
}
