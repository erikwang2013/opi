// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 两轨的**常量级**不变量：键状态位、特殊键码、以及唯一一处合法重叠（裸 0x20）。
//!
//! 与 `two_track_parity.rs` 分开是因为 500 行硬规矩；两者共用 `common` 里的机械。
//! 行为对照在本文件里只当「钉子」用：`space_keyval_0x20_...` 钉住那处设计差异，
//! 免得将来被人顺手「修」成一致。

mod common;

use common::*;

/// 键状态位约定必须逐位相同 —— 两轨共用同一套位（内部契约），
/// 这是「逐行同构」纪律里最容易悄悄漂移的一条。
#[test]
fn key_state_bits_are_identical_across_tracks() {
    use fcitx5_opi::input_method as f;
    use tsf_opi::logic as t;
    for (name, a, b) in [
        ("SHIFT", f::KEY_STATE_SHIFT, t::KEY_STATE_SHIFT),
        ("CAPS_LOCK", f::KEY_STATE_CAPS_LOCK, t::KEY_STATE_CAPS_LOCK),
        ("CTRL", f::KEY_STATE_CTRL, t::KEY_STATE_CTRL),
        ("ALT", f::KEY_STATE_ALT, t::KEY_STATE_ALT),
        ("RELEASED", f::KEY_STATE_RELEASED, t::KEY_STATE_RELEASED),
        ("REPEAT", f::KEY_STATE_REPEAT, t::KEY_STATE_REPEAT),
        (
            "LONG_PRESSED",
            f::KEY_STATE_LONG_PRESSED,
            t::KEY_STATE_LONG_PRESSED,
        ),
    ] {
        assert_eq!(a, b, "{name} 状态位两轨不同：{a:#x} vs {b:#x}");
    }
    // 位必须互不相同（复用同一个位会让「按住 shift 的抬起」变成「长按」）。
    let bits = [
        f::KEY_STATE_SHIFT,
        f::KEY_STATE_CAPS_LOCK,
        f::KEY_STATE_CTRL,
        f::KEY_STATE_ALT,
        f::KEY_STATE_RELEASED,
        f::KEY_STATE_REPEAT,
        f::KEY_STATE_LONG_PRESSED,
    ];
    for (i, a) in bits.iter().enumerate() {
        for b in &bits[i + 1..] {
            assert_ne!(a, b, "状态位重复：{a:#x}");
        }
    }
}

/// 折算函数本身不能退化：两轨的特殊键码必须是**不同的数字**。
///
/// 防的是「对照测试恒定通过」这一种假绿：若 `K::fcitx5()` 与 `K::tsf()` 给出同一个
/// 数字，两边跑的就是同一套键码，比对再严也只是自证。可打印段则是**共享**约定
/// （两轨都是「字符 = Unicode 码点」），那部分必须相同。
#[test]
fn special_key_codes_differ_between_tracks_but_printable_codes_agree() {
    let specials = [
        K::Back,
        K::Del,
        K::Tab,
        K::Esc,
        K::Return,
        K::Space,
        K::PageUp,
        K::PageDown,
        K::Shift,
    ];
    for k in &specials {
        assert_ne!(
            k.fcitx5(),
            k.tsf(),
            "{k:?} 两轨键码撞成同一个数字 —— 对照测试会退化成自证"
        );
    }
    for c in ['a', '1', '\'', '.', ' '] {
        assert_eq!(
            K::Ch(c).fcitx5(),
            K::Ch(c).tsf(),
            "可打印段两轨共用「码点即键码」"
        );
    }
}

/// 空格键值是两轨**唯一**一处合法重叠，因此单独钉住它的确切差异。
///
/// - 空格**按键**（fcitx5 keysym 0x20 / TSF `SPECIAL_BASE|0x20`）两轨一致 —— 见上面的脚本；
/// - 裸 `0x20` 作为**可打印字符**进来时：fcitx5 的 `KEY_SPACE` 就是 0x20 → 走空格分支；
///   TSF 的 `KEY_SPACE` 是 0x10020 → 0x20 走可打印分支 → 拼音模式下直通。
///
/// 两种解释在宿主侧**用户可见的结果相同**（都产出一个空格）：一边自己提交，
/// 一边交给宿主插入。差别只在「谁负责插入」，所以这不是缺陷；
/// 但必须钉住 —— 若哪天有人把 TSF 的 `KEY_SPACE` 改成裸 0x20（或反之），
/// 用户可见行为会变（拼音打一半按空格键变成「插入一个空格且 composition 不提交」），
/// 那时这条会红，由人决定哪种对。
#[test]
fn space_keyval_0x20_is_the_one_documented_divergence() {
    let mut f = CandidateState::load(Some(LUNA)).expect("装载");
    let mut t = TsfLogic::load(Some(LUNA)).expect("装载");
    // 空缓冲、拼音模式。
    let fa = fcitx5_snap(&mut f, 0x20, 0);
    let ta = tsf_snap(&mut t, 0x20, 0);
    assert_eq!(fa.mode, ta.mode);
    assert_eq!(fa.buffer, ta.buffer, "两侧都不该把空格吃进拼音缓冲");
    assert_eq!(
        fa.act,
        Act::Commit(" ".to_string()),
        "fcitx5 侧自己提交空格"
    );
    assert_eq!(ta.act, Act::Pass, "TSF 侧交宿主插入（宿主插入的也是空格）");
    assert_ne!(fa.act, ta.act, "这条用例存在的理由就是两者不同");

    // 而「空格按键」（各自的键码）在空缓冲下用户可见结果相同：各提交一个空格。
    let mut f2 = CandidateState::load(Some(LUNA)).expect("装载");
    let mut t2 = TsfLogic::load(Some(LUNA)).expect("装载");
    let fs = fcitx5_snap(&mut f2, K::Space.fcitx5(), 0);
    let ts = tsf_snap(&mut t2, K::Space.tsf(), 0);
    assert_eq!(fs.act, ts.act);
    assert_eq!(
        fs.act,
        Act::Commit(" ".to_string()),
        "空格键 = 提交一个空格"
    );
}
