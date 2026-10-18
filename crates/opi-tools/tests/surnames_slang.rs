// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 百家姓 / 网络流行语并入 luna.opid 的语义门禁（2026-10-18）。
//!
//! 钉三件事：
//! 1. 姓氏特殊读音可达：单 shan、解 xie、仇 qiu、区 ou、查 zha、尉迟 yuchi、
//!    万俟 moqi 等；其中 能 nai、郗 xi、佴 nai 在 luna 单字层**没有**该读音，
//!    只能由新表提供 —— 这三条是本轮真正新增的能力，不是既有行为的复述。
//! 2. 流行语按拼音可达（名次不限；ASCII 字面键 yyds/栓Q/City不City/i人 同样走
//!    Engine::candidates，验证的是用户真能打出来的那条路）。
//! 3. 常用词不被挤：你好/中国/我们 仍第 1；且同键碰撞里常用词排在新表之前
//!    （dazi: 打字→搭子、eren: 恶人→e人）。新表放在批量段**队尾**正是为此
//!    （放置理由见 scripts/manual_tables.py 头注）。
//!
//! 断言一律走 `Engine::candidates`（candidates.rs 的 rank_and_pick），不走
//! dict.query —— 层序/去重/截断只在候选路径可见（同 phrase_quality.rs 注释）。

use engine_core::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::load_mmap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

fn engine_at(path: &str) -> Engine {
    let dict = load_mmap(Path::new(path)).unwrap_or_else(|e| panic!("加载 {path} 失败：{e:?}"));
    Engine::new(
        Box::new(dict),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    )
}

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

/// (拼音, 期望词)：候选里出现即可（名次不限）。单姓可能落在单字层或批量段尾；
/// 复姓与特殊读音单姓由新表提供。
const SURNAME_REACHABLE: &[(&str, &str)] = &[
    ("shan", "单"),
    ("xie", "解"),
    ("qiu", "仇"),
    ("ou", "区"),
    ("zha", "查"),
    ("piao", "朴"),
    ("qin", "覃"), // 姓氏读音取 qín
    ("yue", "乐"),
    ("nai", "能"), // luna 单字层只有 neng —— 新表新增读音
    ("xi", "郗"),  // luna 单字层只有 chi —— 新表新增读音
    ("nai", "佴"), // luna 单字层只有 er —— 新表新增读音
    ("moqi", "万俟"),
    ("yuchi", "尉迟"),
    ("zhangsun", "长孙"),
    ("tantai", "澹台"),
    ("chanyu", "单于"),
    ("lvqiu", "闾丘"),
    ("yuezheng", "乐正"),
    ("diwu", "第五"),
    ("sima", "司马"),
    ("ouyang", "欧阳"),
    ("zhuge", "诸葛"),
    ("huangfu", "皇甫"),
    ("linghu", "令狐"),
    ("murong", "慕容"),
    ("xuanyuan", "轩辕"),
    // 复姓里本轮真正净新增的 12 条（其余复姓已在上游层）。
    ("xianyu", "鲜于"),
    ("qiguan", "丌官"),
    ("zhangdu", "仉督"),
    ("ziju", "子车"),
    ("wuma", "巫马"),
    ("gongxi", "公西"),
    ("rangsi", "壤驷"),
    ("jiagu", "夹谷"),
    ("zaifu", "宰父"),
    ("guliang", "谷梁"),
    ("duangan", "段干"),
    ("guihai", "归海"),
];

/// (拼音, 期望词)。覆盖用户点名的代表项与 ASCII 字面键。
const SLANG_REACHABLE: &[(&str, &str)] = &[
    ("yyds", "yyds"),
    ("shuanq", "栓Q"),
    ("xianyanbao", "显眼包"),
    ("neijuan", "内卷"),
    ("tangping", "躺平"),
    ("pofang", "破防"),
    ("juejuezi", "绝绝子"),
    ("babiqiu", "芭比Q"),
    ("babiqiule", "芭比Q了"),
    ("tuituitui", "退退退"),
    ("nanie", "拿捏"),
    ("dazi", "搭子"),
    ("songchigan", "松弛感"),
    ("banwei", "班味"),
    ("citybucity", "City不City"),
    ("hajimi", "哈基米"),
    ("wayawa", "挖呀挖"),
    ("mingyundechilun", "命运的齿轮"),
    ("yaoyaolingxian", "遥遥领先"),
    ("tingquan", "听劝"),
    ("chunaizhanshi", "纯爱战士"),
    ("lianainao", "恋爱脑"),
    ("tezhongbinglvyou", "特种兵旅游"),
    ("duobaanchuanda", "多巴胺穿搭"),
    ("qingxujiazhi", "情绪价值"),
    ("iren", "i人"),
    ("eren", "e人"),
    ("shekong", "社恐"),
    ("sheniu", "社牛"),
    ("zuiti", "嘴替"),
    ("daican", "代餐"),
    ("xiatou", "下头"),
    ("shangtou", "上头"),
    ("chouxiang", "抽象"),
    ("zhenghuo", "整活"),
    ("leziren", "乐子人"),
    ("dianzizhacai", "电子榨菜"),
    ("fafengwenxue", "发疯文学"),
    ("feihuawenxue", "废话文学"),
    ("fanersai", "凡尔赛"),
    ("ningmengjing", "柠檬精"),
    ("dagongren", "打工人"),
    ("weikuanren", "尾款人"),
    ("gongjuren", "工具人"),
    ("ganfanren", "干饭人"),
    ("jimei", "集美"),
    ("wuyuzi", "无语子"),
    ("duojiojio", "跺jiojio"),
    ("shesi", "社死"),
    ("zundujiadu", "尊嘟假嘟"),
    ("taikula", "泰裤辣"),
    ("wolegedou", "我勒个豆"),
    ("potiandefugui", "泼天的富贵"),
    ("nanfangxiaotudou", "南方小土豆"),
    ("kemusan", "科目三"),
    ("erbin", "尔滨"),
    ("ziboshaokao", "淄博烧烤"),
    ("tianshuimalatang", "天水麻辣烫"),
    ("cunchao", "村超"),
    ("malou", "吗喽"),
    ("niuma", "牛马"),
    ("dacall", "打call"),
    ("kecp", "嗑CP"),
];

#[test]
fn surname_and_slang_entries_are_reachable_by_pinyin() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (py, word) in SURNAME_REACHABLE.iter().chain(SLANG_REACHABLE) {
        let cands = candidates_of(&mut engine, py);
        if !cands.iter().any(|c| c == word) {
            fails.push(format!(
                "{py}: 候选里没有「{word}」（共 {} 个候选）",
                cands.len()
            ));
        }
    }
    let total = SURNAME_REACHABLE.len() + SLANG_REACHABLE.len();
    assert!(
        fails.is_empty(),
        "姓氏/流行语不可达（{}/{}）：\n  {}",
        fails.len(),
        total,
        fails.join("\n  ")
    );
}

#[test]
fn common_words_keep_rank_and_curated_lane_stays_below_them() {
    let mut engine = engine_at(LUNA_OPID);
    let mut fails = Vec::new();
    for (py, word) in [("nihao", "你好"), ("zhongguo", "中国"), ("women", "我们")] {
        let cands = candidates_of(&mut engine, py);
        let got = cands.first().map(String::as_str).unwrap_or("—");
        if got != word {
            fails.push(format!("{py}: 期望第 1 是「{word}」，实得「{got}」"));
        }
    }
    // 同键碰撞：新表在批量段尾，常用词必须仍在前面。
    for (py, common, curated) in [("dazi", "打字", "搭子"), ("eren", "恶人", "e人")] {
        let cands = candidates_of(&mut engine, py);
        let common_pos = cands.iter().position(|c| c == common);
        let curated_pos = cands.iter().position(|c| c == curated);
        match (common_pos, curated_pos) {
            (Some(a), Some(b)) if a < b => {}
            _ => fails.push(format!(
                "{py}: 期望「{common}」在「{curated}」之前，实得 {common_pos:?} vs {curated_pos:?}"
            )),
        }
    }
    assert!(
        fails.is_empty(),
        "常用词排序被挤压或首选丢失（{}/5）：\n  {}",
        fails.len(),
        fails.join("\n  ")
    );
}
