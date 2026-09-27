// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 第五道门禁的**自检**：把值改坏必须红、写法不同但值相同必须绿、认不出来必须报错。
//!
//! 与 `keycode_contract.rs`（活体对账）分两个测试目标，是为了守 500 行规矩；
//! 两边跑的是**同一套** `parse_*` / `reconcile`，所以自检的结论能迁移到活体路径上。

/// 本目标只用 `strip_comments`（解析器内部要用），其余条目天然是死代码。
#[allow(dead_code)]
#[path = "jni/checker.rs"]
mod checker;

/// 与 `keycode_contract.rs` 共用同一份解析机器；本目标只用其中的解析与对账，
/// 路径常量与 `missing_from` 在这里天然是死代码。
#[allow(dead_code)]
#[path = "keycode/constants.rs"]
mod constants;

#[path = "keycode/fixtures.rs"]
mod fixtures;

use constants::{Const, parse_c, parse_ets, parse_rust, parse_swift, reconcile};
use fixtures::{
    FIX_C_OK, FIX_C_WRONG_LOW, FIX_ETS_WRONG_SHIFT, FIX_RUST, FIX_SWIFT_EMPTY,
    FIX_SWIFT_EQUAL_OTHER_FORM, FIX_SWIFT_MAP, FIX_SWIFT_MISSING, FIX_SWIFT_UNKNOWN,
    FIX_SWIFT_WRONG_BIT,
};

// ---------- fixture 自检：证明这套解析/比对真的会红 ----------

/// 各端各改坏一处（形状不变、只有值变），必须红，且报文点名**哪一方 + 哪个常量**。
#[test]
fn fixtures_redden_on_a_single_wrong_value_and_name_the_constant() {
    let t = parse_rust(FIX_RUST, "FIX · rust", "FIX_RUST").unwrap();

    // 1) C 侧：低字节 `0x83` → `0x38`
    let c = parse_c(FIX_C_WRONG_LOW, "FIX · c", "FIX_C_WRONG_LOW").unwrap();
    let d = reconcile(&c, &t, &[]);
    assert_eq!(d.len(), 1, "应当只有 1 条差异：\n  {}", d.join("\n  "));
    assert!(
        d[0].contains("OPI_KEY_SHIFT") && d[0].contains("KEY_SHIFT") && d[0].contains("0x38u"),
        "报文没点名是哪一方哪个常量：{}",
        d[0]
    );

    // 2) ETS 侧：`1 << 27` → `1 << 25`
    let e = parse_ets(FIX_ETS_WRONG_SHIFT, "FIX · ets", "FIX_ETS_WRONG_SHIFT").unwrap();
    let d = reconcile(&e, &t, &[]);
    assert_eq!(d.len(), 1, "应当只有 1 条差异：\n  {}", d.join("\n  "));
    assert!(
        d[0].contains("STATE_REPEAT") && d[0].contains("KEY_STATE_REPEAT"),
        "报文没点名是哪一方哪个常量：{}",
        d[0]
    );

    // 3) Swift 侧：`1 << 26` 冒充 `1 << 27`（iOS `stateRepeat` 那颗雷的形状）
    let s = parse_swift(
        FIX_SWIFT_WRONG_BIT,
        "enum OpiKey {",
        "FIX · swift",
        "FIX_SWIFT_WRONG_BIT",
        FIX_SWIFT_MAP,
    )
    .unwrap();
    let d = reconcile(&s, &t, &[]);
    assert_eq!(d.len(), 1, "应当只有 1 条差异：\n  {}", d.join("\n  "));
    assert!(
        d[0].contains("stateRepeat")
            && d[0].contains("KEY_STATE_REPEAT")
            && d[0].contains("134217728"),
        "报文没点名是哪一方哪个常量：{}",
        d[0]
    );

    // 4) 绿基线：同一套代码路径，把改坏的值换回来立刻零差异
    let ok = parse_c(FIX_C_OK, "FIX · c-ok", "FIX_C_OK").unwrap();
    assert!(
        reconcile(&ok, &t, &[]).is_empty(),
        "绿基线都不绿，说明上面的红不是「值错」引起的"
    );
}

/// 值相同、写法不同（`1 << 27` / `134217728` / `0x8000000` / `(1u << 27)`）**必须绿**：
/// 按字面串比会制造假红，也会逼着各端不能换写法。
#[test]
fn fixtures_stay_green_when_equal_values_are_written_differently() {
    let t = parse_rust(FIX_RUST, "FIX · rust", "FIX_RUST").unwrap();
    let s = parse_swift(
        FIX_SWIFT_EQUAL_OTHER_FORM,
        "enum OpiKey {",
        "FIX · swift-other-form",
        "FIX_SWIFT_EQUAL_OTHER_FORM",
        FIX_SWIFT_MAP,
    )
    .unwrap();
    let d = reconcile(&s, &t, &[]);
    assert!(
        d.is_empty(),
        "写法不同但值相同，不该红：\n  {}",
        d.join("\n  ")
    );
    // 非空转：证明这些行**真被求值了**，而不是解析失败被静默跳过
    assert_eq!(s.consts["stateRepeat"].value, 1 << 27);
    assert_eq!(s.consts["stateRepeat"].expr, "134217728");
    assert_eq!(s.consts["backspace"].value, 0x1_0008);
    assert_eq!(s.consts["space"].expr, "32");
}

/// 认不出来、漏声明、空壳 —— 三种「静默跳过」的入口都必须报错，不许当零差异。
#[test]
fn fixtures_are_loud_when_the_parser_would_have_to_guess() {
    let t = parse_rust(FIX_RUST, "FIX · rust", "FIX_RUST").unwrap();

    // 未登记的名字（映射表里没有）
    let s = parse_swift(
        FIX_SWIFT_UNKNOWN,
        "enum OpiKey {",
        "FIX · swift-unknown",
        "FIX_SWIFT_UNKNOWN",
        FIX_SWIFT_MAP,
    )
    .unwrap();
    let d = reconcile(&s, &t, &[]);
    assert_eq!(d.len(), 1, "应当只报那一条未登记的：\n  {}", d.join("\n  "));
    assert!(
        d[0].contains("stateFoo") && d[0].contains("未登记"),
        "未知名字必须点名报出，不能静默跳过：{}",
        d[0]
    );

    // 映射表登记了、本端却没这条声明（反方向）
    let m = parse_swift(
        FIX_SWIFT_MISSING,
        "enum OpiKey {",
        "FIX · swift-missing",
        "FIX_SWIFT_MISSING",
        FIX_SWIFT_MAP,
    )
    .unwrap();
    let d = reconcile(&m, &t, &[]);
    assert_eq!(
        d.len(),
        4,
        "6 条映射里本端只声明 2 条：\n  {}",
        d.join("\n  ")
    );
    assert!(
        d.iter().all(|x| x.contains("没解析到")),
        "报文形状不对：\n  {}",
        d.join("\n  ")
    );

    // 空壳：解析器失明必须红，而不是「零差异 = 绿」
    let e = parse_swift(
        FIX_SWIFT_EMPTY,
        "enum OpiKey {",
        "FIX · swift-empty",
        "FIX_SWIFT_EMPTY",
        FIX_SWIFT_MAP,
    );
    match e {
        Ok(s) => panic!("空块解析成了 {} 条常量，这条路径本该报错", s.consts.len()),
        Err(msg) => assert!(msg.contains("一条常量都没解析出来"), "报错文案不对：{msg}"),
    }

    // 未登记常量也要如实进 `Const`（`expr` 原文留着，人才找得到那行）
    let c: &Const = &t.consts["KEY_SPACE"];
    assert_eq!(c.expr, "0x20");
}
