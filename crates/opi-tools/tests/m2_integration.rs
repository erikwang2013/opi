// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! M2 端到端集成测试：Engine + mmap 词典（编译 → 序列化 → 加载 → 输入 → 选词）。
//! 验证词典注入后引擎全链路可用，以及词库损坏时的回退路径。

use engine_core::Scheme;
use engine_core::engine::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::{
    FormatError, LoadError, fallback_dict, load_bytes, load_mmap, load_or_fallback, serialize,
};
use opi_tools::compiler::{compile, parse_dict};
use std::path::Path;

/// 部署副本（`data/generated/luna.opid` 是 gitignore 的构建中间物），与
/// `rare_coverage.rs` / `simplified_phrases.rs` 同一取法。
const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

#[test]
fn full_pipeline_engine_typing() {
    let text = "好\thao\t5000\n号\thao\t1200\n笑\txiao\t3000\n";
    let bytes = serialize(&compile(parse_dict(text)));
    let mmap = load_bytes(bytes).expect("load");
    let mut eng = Engine::new(Box::new(mmap), SymbolEngine::builtin(), false);
    for ch in "hao".chars() {
        eng.input_key(ch);
    }
    assert_eq!(eng.buffer(), "hao");
    let cands = eng.candidates(8);
    assert_eq!(cands[0].text, "好");
    assert_eq!(eng.select(0), "好");
    assert_eq!(eng.buffer(), "");
}

#[test]
fn engine_with_fallback_dict() {
    let mut eng = Engine::new(Box::new(fallback_dict()), SymbolEngine::builtin(), true);
    for ch in "wo".chars() {
        eng.input_key(ch);
    }
    let cands = eng.candidates(8);
    assert_eq!(cands[0].text, "我");
}

#[test]
fn corrupt_file_engine_falls_back() {
    let mut bytes = serialize(&compile(parse_dict("好\thao\n")));
    let n = bytes.len() - 9; // payload 区域（非 trailer）
    bytes[n] ^= 0xFF;
    assert!(matches!(
        load_bytes(bytes),
        Err(LoadError::Format(FormatError::ChecksumMismatch { .. }))
    ));
    assert!(load_or_fallback(Some(std::path::Path::new("/nonexistent/opi.opid"))).is_err());
}

#[test]
fn corrupted_opid_engine_still_boots() {
    // 引擎在词库损坏时仍可用（回退内置），全链路冒烟
    let mut eng = Engine::new(Box::new(fallback_dict()), SymbolEngine::builtin(), false);
    for ch in "n".chars() {
        eng.input_key(ch);
    }
    assert!(eng.candidates(8).iter().any(|c| c.text == "你"));
}

/// 分词回退（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）在**真词库**上的验收。
/// 语义与边界的内联夹具版在 `engine-core/tests/segment_fallback.rs`，本条钉的是
/// **luna 简体真数据**下的三条：主用例（无命中 → 最长前缀的整档 → 选中只消耗前缀）、
/// 「整串模糊命中优先」（`zongguo` 不被回退接管）、「垃圾串维持死 + 逃生口」。
/// 符号表用**空表**：内建表会往 `hao` 这类拼音里掺料（memory「计数夹具要钉空符号表」）。
#[test]
fn segment_fallback_on_real_luna_dictionary() {
    let luna = || {
        Engine::new(
            Box::new(
                load_mmap(Path::new(LUNA_OPID))
                    .unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}")),
            ),
            SymbolEngine::new(Vec::new(), Vec::new()),
            false,
        )
    };
    let texts =
        |e: &Engine| -> Vec<String> { e.candidates(8).into_iter().map(|c| c.text).collect() };
    // 主用例：`tament` 整串（`query_all("tament")` 0 行）⇒ 最长前缀 `tamen` 的整档。
    // ⚠️ 前提标注（2026-10-11 词库整合，原用例 `nihaob` 的两次换血）：
    // ① `nihaob` 并入后成了 `你号被盗了`（nihaobeidaole，base 层）的前缀 ⇒ 整串有
    //    命中、回退不接管（分层规则的**正确**后果，不是回归）；
    // ② 换 `nihaot` 后又撞上**模糊判据**：它的变体 `lihaot`（n↔l）命中 `李昊桐`
    //    （lihaotong）⇒ 判据 4「整串模糊命中优先」拦下回退、落到逐音节汤。
    // 现用 `tament`（他们+t）：ta/men 两个音节都不在任何模糊音对里（变体集为空）、
    // `tament` 与 `tamen+L` 全系无前缀命中（对全库键扫描过），尾 `t` 续编 `tao` 有键。
    let mut eng = luna();
    for ch in "tament".chars() {
        eng.input_key(ch);
    }
    assert_eq!(eng.buffer(), "tament", "预编辑仍是整串");
    let got = texts(&eng);
    assert_eq!(got[0], "他们", "{got:?}");
    // 选中只消耗前缀 —— 旧语义（逐音节汤）在这里会把整个缓冲清掉
    assert_eq!(eng.select(0), "他们");
    assert_eq!(eng.buffer(), "t", "后缀留在缓冲里接着编");
    for ch in "ao".chars() {
        eng.input_key(ch);
    }
    assert_eq!(eng.buffer(), "tao");
    // 后缀是真缓冲：候选表与另开一台直接打 `tao` 的**逐条相同**
    let mut fresh = luna();
    for ch in "tao".chars() {
        fresh.input_key(ch);
    }
    assert_eq!(texts(&eng), texts(&fresh), "后缀接着编走的是同一条路径");
    // 边界②：「整串模糊命中优先」——`zongguo` 的变体 `zhongguo` 在 luna 有 81 行 ⇒
    // 回退不接管，逐音节汤照旧（`guo` 的单字在候选里；回退会把它当成后缀拿走）
    let mut eng = luna();
    for ch in "zongguo".chars() {
        eng.input_key(ch);
    }
    let got = texts(&eng);
    assert!(got.contains(&"中国".to_string()), "{got:?}");
    assert!(got.contains(&"国".to_string()), "{got:?}");
    // 边界①：垃圾串（段不是完整音节）维持死 —— 「打英文的逃生口」不动
    let mut eng = luna();
    for ch in "zxcvb".chars() {
        eng.input_key(ch);
    }
    assert!(texts(&eng).is_empty(), "`zxcvb` 不得被分词回退救活");
    assert_eq!(eng.input_key(' '), "zxcvb", "无候选 ⇒ 空格提交原文");
    assert_eq!(eng.buffer(), "");
}

/// 9/14 键**数字空间回退**（2026-10-10 裁决「回退扩到 9 键」）在**真词库**上的验收；
/// 内联夹具版在 `engine-core/tests/segment_fallback.rs`。用户现场：9 键打长句
/// 「打超到下一音节首键」恰好撞无候选（原用例 `644269` = nihao + [wxyz]；2026-10-11
/// 词库整合后该串被 `你好呀` 一族整串命中、回退不再接管，见证者换成品牌词
/// `比亚迪`——两处前提标注见用例内注释）。
/// 本条钉真 luna 数据下的四条：回退档与命中态整档**逐条相同**、选中只消耗前缀键
/// （尾部键留在缓冲、接着编与另开一台直打逐条相同）、14 键同一条判定自然覆盖
/// （`bugaoml` → 前缀 `bugao` 的整档 + 尾 `ml`；`bugaom` 是命中态不丢尾：真库里有
/// 7 字母的 `buhaoma`/`nihaoma` 把它吃成整串前缀）、垃圾串维持死；**9 键无候选
/// 不再走「空格提交原文」逃生口**（2026-10-11 用户报障「9 键没匹配的时候会变成数字
/// 输入」—— 缓冲是按键编码不是文本；14/26 键缓冲是字母，逃生口语义仍成立、不动）。
/// ⚠️ 本条此前断言 9 键提交 `"55"`，是旧语义的见证者，已随裁决翻面。
#[test]
fn scheme_fallback_on_real_luna_dictionary() {
    let luna = || {
        Engine::new(
            Box::new(
                load_mmap(Path::new(LUNA_OPID))
                    .unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}")),
            ),
            SymbolEngine::new(Vec::new(), Vec::new()),
            false,
        )
    };
    let texts =
        |e: &Engine| -> Vec<String> { e.candidates(8).into_iter().map(|c| c.text).collect() };
    let type_str = |e: &mut Engine, s: &str| {
        for ch in s.chars() {
            e.input_key(ch);
        }
    };
    // 9 键主用例：`8374529`（tesila=特斯拉 + 尾键 9）整串无命中 ⇒ 回退档 = 键前缀
    // `837452` 的整档。⚠️ 前提标注（2026-10-11 词库整合）：原用例是 `644269`
    // （nihao + 9）—— 并入后它被 `你好呀`（nihaoya）/`米高扬` 一族按 6 位前缀整串
    // 命中、回退不再接管（分层规则的**正确**后果），故换一条**天然不接 wxyz 音节**
    // 的见证者：品牌词 `特斯拉`（对全部品牌/新词表 835 条扫描，`8374529` 无任何键的
    // 数字前缀匹配 —— 品牌词极少被更长的词继续）。尾键仍是 `9`，下文 `96` → 我 的
    // 续编断言原样有效。
    let mut nine = luna();
    nine.set_scheme(Scheme::Nine);
    let mut hit = luna();
    hit.set_scheme(Scheme::Nine);
    type_str(&mut hit, "837452");
    type_str(&mut nine, "8374529");
    // ⚠️ **2026-10-15 改判（宽松兜底档）**：这两条的旧口径是「回退档与命中态**逐条相同**」
    // —— 现在**尾巴本身也会出候选**（尾键 `9` 按首字母 `w` 起音），故九键那边多出一截
    // w 一族的字/词。判据改成：**前缀那一截逐条相同**（锚定档在前），多出来的只能是尾巴的
    // 字母一族；读数 = 命中态读数 + 尾字母。
    // ⚠️ 顺序有讲究：读数缓存由 `candidates()` 写（`engine.rs` 那一处），**先各取一次候选**
    // 再读 `display_buffer`。原来这条断言排在取候选之前 —— 两边读到的都是**空缓存**，
    // `"" == ""` 恒真（2026-10-15 顺手发现的空转断言，改判时一并修）。
    let (n_all, h_all) = (texts(&nine), texts(&hit));
    assert_eq!(
        hit.display_buffer(),
        "te'si'la",
        "命中态读数 = 首条候选（特斯拉）的拼音"
    );
    assert_eq!(
        nine.display_buffer(),
        "te'si'la'w",
        "读数 = 命中态读数 + 尾字母（分词器把尾字母切成独立一段）"
    );
    assert_eq!(
        n_all[..h_all.len()],
        h_all[..],
        "前缀那一截逐条相同（尾档只往后追加）"
    );
    assert!(
        n_all.len() > h_all.len(),
        "尾巴该补出 w 一族的候选：{n_all:?}"
    );
    assert_eq!(nine.select(0), "特斯拉");
    assert_eq!(nine.buffer(), "9", "只消耗前缀键：尾部键留下接着编");
    // 接着编：`9` + `6` = 下一个音的键 → 与另开一台直打 `96` 逐条相同
    type_str(&mut nine, "6");
    assert_eq!(nine.buffer(), "96");
    let mut fresh = luna();
    fresh.set_scheme(Scheme::Nine);
    type_str(&mut fresh, "96");
    assert_eq!(texts(&nine)[0], "我", "{:?}", texts(&nine));
    assert_eq!(texts(&nine), texts(&fresh), "尾部键接着编走同一条路径");
    // 14 键同一条判定：`bugaoml`（bugao + 尾 `ml`）⇒ 前缀 `bugao` 的整档 + 尾留缓冲
    let mut fourteen = luna();
    fourteen.set_scheme(Scheme::Fourteen);
    type_str(&mut fourteen, "bugaoml");
    let mut hit14 = luna();
    hit14.set_scheme(Scheme::Fourteen);
    type_str(&mut hit14, "bugao");
    assert_eq!(
        texts(&fourteen),
        texts(&hit14),
        "14 键回退档 = 前缀 `bugao` 的整档，逐条相同"
    );
    assert_eq!(fourteen.select(0), "不好");
    assert_eq!(fourteen.buffer(), "ml", "两个尾部键都留下");
    // 14 键对照：打满到 `bugaom` **不是**回退态（真库里 7 字母的 buhaoma/nihaoma 命中，
    // 整串前缀语义）—— 选中后缓冲清空，证明没走「丢尾」那条腿
    let mut full = luna();
    full.set_scheme(Scheme::Fourteen);
    type_str(&mut full, "bugaom");
    assert_eq!(full.select(0), "不好吗");
    assert_eq!(full.buffer(), "", "命中态不丢尾：缓冲整段被消耗");
    // ⚠️ **2026-10-15 改判（宽松兜底档）**：`55` 不再是死串 —— 尾巴 `55` 整段按
    // **末键上最前的字母** `j` 起音（用户裁决「没匹配的用按钮上最前的开始匹配字、词」），
    // 于是「有候选」那条腿接管：空格 = 提交首候选 + 清缓冲。
    // **判据没丢、只是换了构造**：「9 键无候选时，空格不上屏也不清缓冲」现在只剩含 `'`
    // 或无类键的输入能造出来（夹具版见
    // `crates/engine-core/tests/nine_tail_guess.rs::tier_is_off_for_fourteen_key_separator_and_classless_keys`）。
    // 14/26 键的「打英文」逃生口仍是「提交原文」，见证者在 `segment_fallback.rs` /
    // 本文件上面的 `zxcvb`。
    let mut junk = luna();
    junk.set_scheme(Scheme::Nine);
    type_str(&mut junk, "55");
    let got = texts(&junk);
    assert!(!got.is_empty(), "宽松兜底档：`55` 按末键首字母 `j` 出字");
    assert!(
        got.iter().take(3).all(|t| t.chars().count() == 1),
        "前段出的是**字**（报障要的「字、词」里那半）：{got:?}"
    );
    assert!(
        junk.display_buffer().ends_with('j'),
        "拼音行把尾巴显示出来（尾字母）：{}",
        junk.display_buffer()
    );
    assert!(!junk.select(0).is_empty(), "选中首候选有文本");
    assert_eq!(junk.buffer(), "", "选中即清缓冲（既有语义）");
    // 报障那一串（真库）：前缀档照旧 + 尾巴出字，且读数带上尾字母
    let mut ni = luna();
    ni.set_scheme(Scheme::Nine);
    type_str(&mut ni, "6444444");
    let tail_texts = texts(&ni);
    assert_eq!(
        tail_texts.first().map(String::as_str),
        Some("你"),
        "前缀档照旧（`64` = ni 一族）：{tail_texts:?}"
    );
    assert!(
        tail_texts.len() > 3,
        "尾巴还该补出 g 一族的字/词：{tail_texts:?}"
    );
    assert!(
        ni.display_buffer().ends_with("'g"),
        "拼音行 = 前缀首条读音 + 尾字母：{}",
        ni.display_buffer()
    );
}
