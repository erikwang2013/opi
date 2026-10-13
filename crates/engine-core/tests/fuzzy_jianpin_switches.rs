// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」的两个排序开关：模糊音 / 简拼，**关掉必须真的关掉**。
//!
//! 走 `Engine::candidates`（= `rank_and_pick_with`）：分档规则全长在那一层，绕到
//! `dict.query` 就让缺陷对门禁不可见（记忆：ranking-gate-bypasses-ranking-code）。
//!
//! **每个用例都用「只有那条通路能查到」的词**（可控小词库 + 真实量级词频）：
//! - 模糊音：`zongguo` 的整串变体 `zhongguo` —— 逐音节兜底查 `zong`/`guo` 都是空，
//!   关掉模糊就必须**一条不剩**（这个「一条不剩」正是本文件盯的东西：只关整串、
//!   漏关逐音节那一档时，`zongguo` 仍会从别处冒出来）。
//! - 简拼：`nh` —— 词库里没有任何以 `nh` 开头的键，精确档必空。
//!
//! 开关的状态在 `Engine` 上（`set_fuzzy` / `set_jianpin`），与「学习开关」同一层；
//! 默认**两个都开**（= 本层既有行为，`RankOptions::default()`）。

use engine_core::Engine;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 两个词组各只有**一条**来路：`zhongguo` 只有模糊音够得着，`nihao` 只有简拼够得着。
///
/// ⚠️ 简拼那两 **单音节条目不能省**（`ni`/`hao`）：简拼展开是「按位置枚举有证据的音节」
/// 建网格（`jianpin.rs`），词库里没有音节条目就没有网格 ⇒ `nh` 什么都展开不出。
/// 只放整词会把这个用例变成「简拼没生效」的假红（本文件第一版就是这么红的）。
fn dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("ni", "你", 3_999_128_899u32),
        ("hao", "好", 3_997_635_583),
        ("zhongguo", "中国", 3_996_000_000),
        ("nihao", "你好", 47_594),
        // 只有 n→l 够得着的第二条来路（`nan` → `lan`）：逐对门禁用。
        ("lan", "兰", 3_990_000_000),
    ] {
        d.insert(py, w, f);
    }
    d
}

fn engine() -> Engine {
    Engine::new(Box::new(dict()), no_symbols(), false)
}

fn cands(e: &mut Engine, input: &str) -> Vec<String> {
    e.clear();
    for ch in input.chars() {
        e.input_key(ch);
    }
    e.candidates(usize::MAX)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[test]
fn defaults_are_both_on() {
    // 既有行为不许被这两个开关改掉：新装的引擎两个都开。
    let e = engine();
    assert!(e.fuzzy_enabled());
    assert!(e.jianpin_enabled());
}

#[test]
fn fuzzy_off_kills_the_variant_only_word() {
    let mut e = engine();
    assert!(
        cands(&mut e, "zongguo").contains(&"中国".to_string()),
        "默认（模糊开）：zongguo 应当经整串变体 zhongguo 拿到「中国」"
    );
    e.set_fuzzy(false);
    assert!(
        cands(&mut e, "zongguo").is_empty(),
        "模糊关掉后 zongguo 必须一条不剩（逐音节兜底那档也得一起关）"
    );
    e.set_fuzzy(true);
    assert!(
        cands(&mut e, "zongguo").contains(&"中国".to_string()),
        "再打开应当恢复 —— 开关不许粘住"
    );
}

#[test]
fn jianpin_off_kills_the_abbreviation_word() {
    let mut e = engine();
    assert!(
        cands(&mut e, "nh").contains(&"你好".to_string()),
        "默认（简拼开）：nh 应当展开到「你好」"
    );
    e.set_jianpin(false);
    assert!(
        cands(&mut e, "nh").is_empty(),
        "简拼关掉后 nh 必须一条不剩（词库里没有以 nh 开头的键）"
    );
    e.set_jianpin(true);
    assert!(
        cands(&mut e, "nh").contains(&"你好".to_string()),
        "再打开应当恢复"
    );
}

#[test]
fn switches_are_independent() {
    // 模糊关、简拼开
    let mut e = engine();
    e.set_fuzzy(false);
    assert!(cands(&mut e, "zongguo").is_empty(), "模糊关 ⇒ zongguo 空");
    assert!(
        cands(&mut e, "nh").contains(&"你好".to_string()),
        "简拼没被牵连"
    );
    // 简拼关、模糊开
    let mut e = engine();
    e.set_jianpin(false);
    assert!(cands(&mut e, "nh").is_empty(), "简拼关 ⇒ nh 空");
    assert!(
        cands(&mut e, "zongguo").contains(&"中国".to_string()),
        "模糊没被牵连"
    );
}


// ---------- 2026-10-13：模糊音逐对多选（用户裁决「可多选类似 z=zh」） ----------

/// 只开 `n=l`（第 6 位）时：`nan → lan` 的模糊要生效，`zongguo → zhongguo` 的**不许**生效。
#[test]
fn only_the_selected_pair_is_active() {
    use engine_core::fuzzy::{FUZZY_ALL, FUZZY_PAIRS};
    // 位序契约：`FUZZY_PAIRS` 数组序 = 位序（对外 ABI 同此）。
    let n_l = 1u8 << FUZZY_PAIRS.iter().position(|p| *p == ("n", "l")).unwrap();
    let zh_z = 1u8 << FUZZY_PAIRS.iter().position(|p| *p == ("zh", "z")).unwrap();

    let mut e = engine();
    e.set_fuzzy_pairs(n_l);
    assert!(
        cands(&mut e, "nan").contains(&"兰".to_string()),
        "n=l 开着 ⇒ nan → lan 要能出「兰」"
    );
    assert!(
        !cands(&mut e, "zongguo").contains(&"中国".to_string()),
        "zh=z 关着 ⇒ zongguo 不许从模糊拿「中国」"
    );

    e.set_fuzzy_pairs(zh_z);
    assert!(
        cands(&mut e, "zongguo").contains(&"中国".to_string()),
        "zh=z 开着 ⇒ 中国 要回来"
    );
    assert!(
        !cands(&mut e, "nan").contains(&"兰".to_string()),
        "n=l 关着 ⇒ nan 不许出「兰」"
    );
}

/// 旧布尔入口 = 新掩码的**包装**：`false` ≡ 0（一条模糊不剩）、`true` ≡ `FUZZY_ALL`。
#[test]
fn boolean_wrapper_equals_all_or_none() {
    use engine_core::fuzzy::FUZZY_ALL;

    let mut e = engine();
    e.set_fuzzy(false);
    assert_eq!(e.fuzzy_pairs(), 0);
    assert!(!e.fuzzy_enabled(), "读侧：任一组开着才为 true");
    assert!(!cands(&mut e, "zongguo").contains(&"中国".to_string()));
    assert!(!cands(&mut e, "nan").contains(&"兰".to_string()));

    e.set_fuzzy(true);
    assert_eq!(e.fuzzy_pairs(), FUZZY_ALL);
    assert!(e.fuzzy_enabled());
    assert!(cands(&mut e, "zongguo").contains(&"中国".to_string()));
    assert!(cands(&mut e, "nan").contains(&"兰".to_string()));
}

/// ABI 契约钉：`FUZZY_ALL` = 8 位全 1；越界位写不进读侧（防未来加组时旧宿主误开新组）。
#[test]
fn mask_contract_is_eight_bits() {
    use engine_core::fuzzy::FUZZY_ALL;
    assert_eq!(FUZZY_ALL, 0xFF, "8 组的全开掩码是对外契约（setFuzzyPairs 的入参口径）");

    // 引擎侧类型就是 u8（8 位 = 8 组）；「int 入参的超范围钳制」在 FFI 层
    // （`setFuzzyPairs(i32)` 的钳制与它的门禁见 jni/c_abi 契约测试）。
    let mut e = engine();
    assert_eq!(e.fuzzy_pairs(), FUZZY_ALL, "缺省 = 全开（既有行为）");
    e.set_fuzzy_pairs(0xFE);
    assert_eq!(e.fuzzy_pairs(), 0xFE, "掩码按位原样存读");
}
