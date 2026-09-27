// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 两轨 IME（fcitx5-opi / tsf-opi）的**行为**对照 —— 不是结构对照。
//!
//! 既有的两轨检查只比「函数名与顺序是否同名同序」（`two-ime-tracks-stay-line-parallel`
//! 记忆里那条纪律），那是**形状**；本文件比的是**形状变了会不会被发现**以外的另一面：
//! 同一串按键分别喂进去，两边的可观测状态必须逐步相同。逐行同构的两个文件里，
//! 只要有一边漏了一个 `released` 判定、少了一个 `checked_sub`、或把模式分流写反，
//! 这里就会在第 N 步上红，并打出是哪一步、哪个字段不同。
//!
//! 为什么放在 `crates/opi-ffi/tests/`：这个仓库里能同时依赖 fcitx5-opi 与 tsf-opi
//! 的地方只有测试目标（产品各 crate 互不依赖，且不该依赖）。dev-dependencies 即可，
//! 产品依赖图不变。
//!
//! 两轨**有意**不对称的三处，本文件显式编码、不当成 drift：
//! 1. 键码空间不同（fcitx5 keysym `0xff00+` vs TSF `SPECIAL_BASE|VK`）→ 由 `K::fcitx5/tsf` 折算；
//! 2. fcitx5 的 `EngineHandled` 一个枚举值覆盖 TSF 的 `CompositionChanged | Consumed`
//!    （TSF 多一档「消费但无需刷新」）→ 由 `Act` 归一；
//! 3. ⇧ 有两码（`KEY_SHIFT_L/R`）vs 一码（`KEY_SHIFT`）→ 两个入参折到同一个 `K::Shift`。
//!
//! 机械（`K`/`Act`/`Snap`/`run_script`）见 `common`；键码与状态位的常量级
//! 不变量见 `two_track_keycodes.rs`。

mod common;

use common::*;

/// 全分支脚本：三种模式 × 每类按键 × 按下/抬起/修饰键。
///
/// 长度是**故意的** —— 两轨的差异历史上就出现在 `released` 漏判、数字选词的
/// `checked_sub`、空缓冲的分流这三处，脚本必须把这三处的两侧都走到。
#[test]
fn routing_matches_event_by_event_on_a_shared_script() {
    use fcitx5_opi::input_method as f;
    let (rel, ctrl, alt, rep, long) = (
        f::KEY_STATE_RELEASED,
        f::KEY_STATE_CTRL,
        f::KEY_STATE_ALT,
        f::KEY_STATE_REPEAT,
        f::KEY_STATE_LONG_PRESSED,
    );
    let mut ev: Vec<Step> = Vec::new();
    fn push(ev: &mut Vec<Step>, k: K, rel: u32) {
        ev.push(Step::Key(k, 0));
        ev.push(Step::Key(k, rel));
    }

    // —— 拼音模式：空缓冲下的每一个特殊键 ——
    ev.push(Step::Mode(Mode::Pinyin));
    for k in [
        K::Back,
        K::Return,
        K::Space,
        K::Tab,
        K::Esc,
        K::PageUp,
        K::PageDown,
    ] {
        push(&mut ev, k, rel);
    }
    // 空缓冲的空格要提交一个空格（两轨同）
    // —— 组词：字母进缓冲、非字母直通、数字选词 ——
    for c in ['n', 'i', 'h', 'a', 'o'] {
        push(&mut ev, K::Ch(c), rel);
    }
    push(&mut ev, K::Ch('\''), rel);
    push(&mut ev, K::Ch('.'), rel);
    push(&mut ev, K::Ch(','), rel);
    push(&mut ev, K::Ch('0'), rel);
    for c in ['1', '5', '9'] {
        push(&mut ev, K::Ch(c), rel);
    }
    // 缓冲非空时的特殊键（这里才会真的动缓冲/翻页）。
    //
    // 每个键都**重新建立一次非空缓冲**，而不是串着按：串着按时，前一个键可能
    // 已经清空了缓冲（空格/回车提交、退格删空），后面的键就退化成「空缓冲分支」，
    // 而两轨在空缓冲下大多都直通 —— 分歧被掩盖。这个洞是变异测试拎出来的：
    // 把 fcitx5 的 `Key::Delete` 从特殊键分支摘掉（`KEY_BACK_SPACE | KEY_DELETE`
    // → `KEY_BACK_SPACE`），旧脚本**全绿**，因为按到 Delete 时缓冲已经空了。
    for k in [
        K::PageDown,
        K::PageUp,
        K::Back,
        K::Del,
        K::Tab,
        K::Esc,
        K::Space,
        K::Return,
    ] {
        for c in ['n', 'i'] {
            push(&mut ev, K::Ch(c), rel);
        }
        ev.push(Step::NonEmptyBuffer);
        push(&mut ev, k, rel);
    }
    // —— ⇧ 状态机：单击 / 长按 / 释放 / 重复 ——
    ev.push(Step::Key(K::Shift, 0));
    ev.push(Step::Key(K::Shift, rep));
    ev.push(Step::Key(K::Shift, long));
    ev.push(Step::Key(K::Shift, rel));
    push(&mut ev, K::Shift, rel);
    // —— 修饰键组合一律放行 ——
    for m in [ctrl, alt, ctrl | alt] {
        for k in [
            K::Ch('a'),
            K::Back,
            K::Return,
            K::Space,
            K::PageDown,
            K::Esc,
        ] {
            ev.push(Step::Key(k, m));
            ev.push(Step::Key(k, m | rel));
        }
    }

    // —— 英文模式：空缓冲直传、⇧ single 转大写、非字母 ——
    ev.push(Step::Mode(Mode::English));
    for k in [
        K::Ch('a'),
        K::Ch('Z'),
        K::Ch('5'),
        K::Ch('-'),
        K::Back,
        K::Return,
        K::Space,
    ] {
        push(&mut ev, k, rel);
    }
    push(&mut ev, K::Ch(' '), rel); // 裸 0x20：英文模式下两轨一致（见 space_keyval_0x20 用例）
    push(&mut ev, K::Shift, rel); // single
    push(&mut ev, K::Ch('b'), rel);
    push(&mut ev, K::Ch('c'), rel);
    push(&mut ev, K::Shift, rel); // 关
    push(&mut ev, K::Shift, rel);
    ev.push(Step::Key(K::Shift, long)); // lock
    push(&mut ev, K::Ch('d'), rel);
    push(&mut ev, K::Back, rel);
    push(&mut ev, K::Return, rel);

    // —— 数字模式：可见 ASCII 全直通；符号模式：可见 ASCII 进关键字缓冲（B3）——
    // 两者的结论不同但仍在同一条脚本里对照：本用例比的是**两轨逐步一致**，
    // 不是与某个期望表对齐 —— 期望表在各自的用例里（`number_mode_*` / `symbol_*`）。
    for m in [Mode::Number, Mode::Symbol] {
        ev.push(Step::Mode(m));
        for c in ['a', '1', '-', '\'', '.'] {
            push(&mut ev, K::Ch(c), rel);
        }
        for k in [K::Back, K::Return, K::Space, K::PageDown] {
            push(&mut ev, k, rel);
        }
    }

    // —— 回拼音再走一遍（模式切换后的状态残留）——
    ev.push(Step::Mode(Mode::Pinyin));
    for c in "nihao".chars() {
        push(&mut ev, K::Ch(c), rel);
    }
    push(&mut ev, K::Return, rel);

    assert!(
        ev.len() > 150,
        "脚本太短，覆盖不到两侧分支：{} 步",
        ev.len()
    );
    run_script(&ev);
}

/// 穷举**全部 ASCII 键值**（0x00..=0x7F）× 五种模式：两轨必须逐步一致。
///
/// 单个字符的用例靠人想；这一条靠机器把整个可打印段加控制段扫一遍。
/// ASCII 段是两轨唯一**共用键码**的区间（fcitx5 的 keysym 在 0xff00+、
/// TSF 在 0x10000+），因而是「同一数字在两轨里含义是否相同」最该扫的区间。
#[test]
fn every_ascii_keyval_behaves_identically_in_every_mode() {
    // Symbol 自 B3 起也消费可见 ASCII（进关键字缓冲）—— 两轨的新分支正是靠这一条
    // 全扫锁住；漏掉它的话，「两轨同构」在符号模式上就只剩人工抽查。
    for mode in [
        Mode::Pinyin,
        Mode::Traditional,
        Mode::English,
        Mode::Number,
        Mode::Symbol,
    ] {
        let mut f = CandidateState::load(Some(LUNA)).expect("装载");
        let mut t = TsfLogic::load(Some(LUNA)).expect("装载");
        f.switch_mode(mode);
        t.switch_mode(mode);
        for keyval in 0x00u32..=0x7f {
            // 0x20 是**唯一**一个两轨键空间合法重叠的值：fcitx5 的 `KEY_SPACE` 就是
            // 可打印段的 0x20（keysym 原值），TSF 的是 `SPECIAL_BASE|0x20=0x10020`。
            // 于是同一个 0x20 在两轨里含义不同 —— 这是设计，不是漂移，
            // 单独由 `space_keyval_0x20_is_the_one_documented_divergence` 钉住。
            if keyval == 0x20 {
                continue;
            }
            for states in [0u32, 1 << 26] {
                let fs = fcitx5_snap(&mut f, keyval, states);
                let ts = tsf_snap(&mut t, keyval, states);
                assert_eq!(
                    fs, ts,
                    "{mode:?} 模式下键值 {keyval:#04x}（states {states:#x}）两轨分叉：\n fcitx5: {fs:?}\n tsf   : {ts:?}"
                );
            }
        }
    }
}
