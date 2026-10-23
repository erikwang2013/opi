// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **9 键宽松兜底档**（用户裁决 2026-10-15）的语义门禁。
//!
//! 报障原话：「输入 ni 拼音后不停点 i 所在的按钮（9 键 = 连按 `4`），然后确认第一个候选词，
//! 再次输入其他拼音无反应，候选词显示无匹。……**没匹配的用按钮上最前的开始匹配字、词**。」
//!
//! 口径（按裁决时间）：
//! · **先段后键**：尾巴**整段**当一个音（取**最后一个**键的首字母）出字与词，之后**逐键**
//!   起音出字（同一个池里继续取）；
//! · **尾巴不分长短**：单键尾巴也走本档（拼音行因此把尾巴显示出来，不再隐形）；
//! · **位置**：产物排在**词组之后、联想单字之前**（用户裁决 2026-10-16，实测判据见
//!   `candidates_tiers::Meta::tail`）。
//!
//! 触发面见 `candidates_scheme::rank_and_pick_scheme` 的尾档：**严格档**（主查询 + 补读音 +
//! `scheme_fallback_len` 回退）读完余下的那段键 = 尾巴；整串严格命中 ⇒ 没有尾巴 ⇒ 本档一行
//! 不做。夹具是**内联词库 + 空符号表**（计数/收录断言不该被符号表掺料）；真库验收在
//! `crates/opi-tools/tests/m2_integration.rs`。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};

fn dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    // 前缀档（ni 一族）：一个精确单字 + 一个词组 + 一个**联想单字**（`nian` 是 `64` 的
    // 拼音延伸 ⇒ 第 3 档「联想单字」）。联想单字是**证明兜底档插在联想之前**的探针：
    // 它必须排在尾巴那批之后（用户裁决 2026-10-16：尾巴在词组之后、联想之前）。
    d.insert("ni", "你", 7_000);
    d.insert("nihao", "你好", 9_000);
    d.insert("nian", "年", 5_000);
    // 尾巴首字母 g 一族：4 个字（前段槽位）+ 2 个词（前段槽位）+ 2 个字（后段池里继续取）
    d.insert("ge", "个", 6_000);
    d.insert("gei", "给", 5_000);
    d.insert("guo", "过", 4_000);
    d.insert("guo", "国", 3_000);
    d.insert("ganjue", "感觉", 2_000);
    d.insert("gongzuo", "工作", 1_500);
    d.insert("gai", "该", 900);
    d.insert("gan", "干", 800);
    // 整串都是尾巴时用的 w 一族
    d.insert("wo", "我", 5_000);
    d
}

/// 只留 `ni` 的稀疏夹具：尾巴字母**一条都没有** ⇒ 本档产出为空 ⇒ 读数不许被改写
/// （这正是既有稀疏夹具断言保绿的那条机制）。
fn sparse_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 7_000);
    d
}

fn engine_with(dict: InMemoryDictionary) -> Engine {
    let mut e = Engine::new(
        Box::new(dict),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    );
    e.set_scheme(Scheme::Nine);
    e
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// **先段后键**的主用例（用户确认过的候选栏逐条）：
/// `6444444` = `64`（ni，前缀档照旧） + 尾巴 `44444`（读不出来）。
/// 尾巴整段当 `g` 一个音 ⇒ 个 给 过 国（字，前 4）+ 感觉 工作（词，前 2）；再逐键起音 ⇒
/// 从同一个池里继续取 该 干。逐条读数 = `ni'g`（前缀首条读音 + 尾字母）；
/// 拼音行显示串再按 2026-10-23 裁决把**未覆盖的键**逐个补成键面首字母 ⇒ `ni'ggggg`。
#[test]
fn junk_tail_falls_back_to_leading_letters_section_then_keys() {
    let mut e = engine_with(dict());
    type_str(&mut e, "6444444");
    assert_eq!(e.buffer(), "6444444", "预编辑还是用户按的整串");
    assert_eq!(
        texts(&e, 16),
        vec![
            "你", "你好", "个", "给", "过", "国", "感觉", "工作", "该", "干", "年"
        ],
        "前缀档（字→词）→ 尾巴（前段字→前段词→后段字）→ 联想单字「年」\
         （用户裁决 2026-10-16：尾巴档插在词组之后、联想之前）"
    );
    assert_eq!(
        e.display_buffer(),
        "ni'ggggg",
        "拼音行完整显示：读数 `ni'g` + 未覆盖的 4 个键（4→g）"
    );
    assert_eq!(
        e.candidate_readings(),
        vec!["ni'g"],
        "本档产出后逐条读数统一成「前缀首条读音 + 尾字母」⇒ 去重后一条"
    );
}

/// 整串都是尾巴（连回退都没落点）：`999` ⇒ 尾巴首字母 `w` ⇒ 出 w 一族的字，读数 = `w`。
/// 这条同时钉住旧裁决：拼音行显示的是**字母**，不是数字串。
#[test]
fn whole_tail_without_any_prefix_still_yields_candidates() {
    let mut e = engine_with(dict());
    type_str(&mut e, "999");
    assert_eq!(texts(&e, 8), vec!["我"], "w 一族的字");
    assert_eq!(
        e.display_buffer(),
        "www",
        "整串都是尾巴：每个键补一个首字母"
    );
    assert_ne!(
        e.display_buffer(),
        "999",
        "数字串不许端上去（2026-10-11 旧裁决）"
    );
}

/// 不启用面（三条，都是**判据**不是巧合）：
/// · **14 键**：键类首字母 == 键自身 ⇒ 本档本版不开（缓冲读不出来就还是没候选）；
/// · **含 `'`**：分词键是用户的**显式**音节边界主张，本档不越过去；
/// · **无类键**（字母混进 9 键缓冲）：取不到首字母 ⇒ 整档不启用（与 `Scheme::matches` 同姿态）。
#[test]
fn tier_is_off_for_fourteen_key_separator_and_classless_keys() {
    let mut e14 = engine_with(dict());
    e14.set_scheme(Scheme::Fourteen);
    type_str(&mut e14, "wqxwqx");
    assert!(texts(&e14, 8).is_empty(), "14 键本版不启用本档");

    let mut sep = engine_with(dict());
    type_str(&mut sep, "64'444");
    assert!(texts(&sep, 8).is_empty(), "含 `'` ⇒ 不启用");

    // 无类键（`a` 混进 9 键缓冲）：整档**不启用** —— 前缀档（严格档，`64` 的你我你好）照旧，
    // 但一个 g 字都不许进来（尾巴 `4a` 里那个 `a` 取不到首字母 ⇒ 本档整条不做）。
    let mut cls = engine_with(dict());
    type_str(&mut cls, "64a");
    assert_eq!(
        texts(&cls, 8),
        vec!["你", "你好", "年"],
        "无类键 ⇒ 整档不启用（只剩严格档的前缀与联想）"
    );
}

/// **产出为空不改写读数**：稀疏夹具（尾巴字母一条没有）下，前缀档照旧、读数本身照旧是
/// `ni` —— 不许因为「有过尾巴」就把读数改成 `ni'g`。（既有稀疏夹具的绿就靠这条。）
/// 显示串另算：2026-10-23 起它把未覆盖的键补成首字母（完整显示，`ni` + 5 个 `g`），
/// 与「读数改写」是两件事。
#[test]
fn readings_stay_untouched_when_the_tier_yields_nothing() {
    let mut e = engine_with(sparse_dict());
    type_str(&mut e, "6444444");
    assert_eq!(
        texts(&e, 8),
        vec!["你"],
        "前缀档照旧（尾巴字母无条目 ⇒ 本档空）"
    );
    assert_eq!(
        e.display_buffer(),
        "niggggg",
        "读数（缓存）不改；显示串把 5 个未覆盖键补成首字母"
    );
}
