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

/// **补上一版的残留缺口**：上面那条只让两轨**互比**，真源 `keys.rs` 没参与 ——
/// 两边**一起**把 `REPEAT` 写成 `1 << 26` 它会照绿，而这正是「门禁在、真源不在」。
/// 这条把两轨的声明面**逐个对着真源**钉死（直接引用常量：少一个名字就编译不过，
/// 比文本解析强）。
///
/// 两轨各只声明 **7** 个位、**都没有 `KEY_STATE_META`**（真源有 8 个）——
/// 这是**子集**不是缺陷：两轨逻辑里 `META` 零命中，谁也不读它。缺的那个**显式
/// 写在下面的计数护栏里**，谁哪天补上 `META`，这条会红，由人确认后改清单。
#[test]
fn state_bits_are_anchored_to_the_true_source_not_to_each_other() {
    use engine_core::keys as e;
    use fcitx5_opi::input_method as f;
    use tsf_opi::logic as t;
    let all: [(&str, u32, u32, u32); 7] = [
        (
            "SHIFT",
            f::KEY_STATE_SHIFT,
            t::KEY_STATE_SHIFT,
            e::KEY_STATE_SHIFT,
        ),
        (
            "CAPS_LOCK",
            f::KEY_STATE_CAPS_LOCK,
            t::KEY_STATE_CAPS_LOCK,
            e::KEY_STATE_CAPS_LOCK,
        ),
        (
            "CTRL",
            f::KEY_STATE_CTRL,
            t::KEY_STATE_CTRL,
            e::KEY_STATE_CTRL,
        ),
        ("ALT", f::KEY_STATE_ALT, t::KEY_STATE_ALT, e::KEY_STATE_ALT),
        (
            "RELEASED",
            f::KEY_STATE_RELEASED,
            t::KEY_STATE_RELEASED,
            e::KEY_STATE_RELEASED,
        ),
        (
            "REPEAT",
            f::KEY_STATE_REPEAT,
            t::KEY_STATE_REPEAT,
            e::KEY_STATE_REPEAT,
        ),
        (
            "LONG_PRESSED",
            f::KEY_STATE_LONG_PRESSED,
            t::KEY_STATE_LONG_PRESSED,
            e::KEY_STATE_LONG_PRESSED,
        ),
    ];
    for (name, fv, tv, ev) in all {
        assert_eq!(
            fv, ev,
            "fcitx5 轨 KEY_STATE_{name} = {fv:#x}，真源 keys.rs = {ev:#x}"
        );
        assert_eq!(
            tv, ev,
            "tsf 轨 KEY_STATE_{name} = {tv:#x}，真源 keys.rs = {ev:#x}"
        );
    }
    // 非空转护栏：条数钉成**实测**的 7。少了 = 上面表里少一行（编译不过，够不着这里）；
    // 多了 = 轨里新加了一个位而这份清单没跟 —— 那才是这条护栏真正要拦的。
    // ponytail: 数文本子串，不做语法分析；有人在这些文件里写注释提到 `const KEY_STATE_` 会假红，届时按实测值改。
    for (src, side) in [
        (
            include_str!("../../tsf-opi/src/logic_input_method.rs"),
            "tsf",
        ),
        (
            include_str!("../../fcitx5-opi/src/input_method.rs"),
            "fcitx5",
        ),
    ] {
        // 判据用**子串**而不是行首匹配：`pub(crate) const` / 裸 `const` / 同行带属性前缀
        // 都是合法写法，行首匹配对它们全盲。2026-09-28 与 `key_space_boundary.rs` 的键码
        // 扫描面一起收紧 —— 那边按名排除 `KEY_STATE_*`，**兜底责任实际落在这里**，
        // 两边都行首匹配的话同一个盲区会被两道门禁同时漏过（实测 `KEY_STATE_ZZZ` 走查）。
        let n = src.matches("const KEY_STATE_").count();
        assert_eq!(
            n, 7,
            "{side} 轨的状态位声明条数变了（实测 7）：增删必须显式改这份清单"
        );
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
