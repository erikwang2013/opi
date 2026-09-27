// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 可打印键「按下/抬起同判」的回归测试 + 键码空间的退化行为（`#[path]` 引入
//! router.rs，保持各文件 <500 行）。对标两轨的 `input_method_release_tests.rs` /
//! `logic_release_tests.rs`（同名同序）。
//!
//! 缺陷形态（两轨都栽过）：`key_event` 里每个特殊键分支都判了 `released`，可打印
//! 分支漏判 → 一个字符被送进引擎两次（拼音缓冲翻倍 "ni"→"nnii"、英文重复提交
//! "a"→"aa"）；改成「抬起一律拦下」后反向不对称 —— 拼音态的非字母符号（`.`、`-`）、
//! 无候选或越界的数字在按下时是 PassThrough，抬起却 EngineHandled，客户端收到
//! keydown 收不到 keyup（依赖键状态的控件卡键）。修法：记按下时的结论
//! （`last_printable`），抬起按同一结论回复。

use super::*;
use crate::symbols::SymbolEngine;

/// 本组用例都不需要候选，用内置回退之外的**空**词库即可（engine-core 不依赖
/// engine-data，故只造引擎不装载词表）。
fn pinyin_state() -> KeyRouter {
    let mut s = KeyRouter::new(Engine::new(
        Box::new(crate::dictionary::InMemoryDictionary::new()),
        SymbolEngine::builtin(),
        true,
    ));
    s.switch_mode(Mode::Pinyin);
    s
}

/// 20 条 "hao" 词条（与 router_invariants.rs 同惯例）：**提交真的会发生**，
/// 回车按下会把缓冲清空 —— 特殊键用例需要「按下改动了缓冲」这个前提。
fn state_with_candidates() -> KeyRouter {
    let mut d = crate::dictionary::InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let mut s = KeyRouter::new(Engine::new(Box::new(d), SymbolEngine::builtin(), true));
    s.switch_mode(Mode::Pinyin);
    s
}

// ---- 可打印字符抬起须与按下同判（直通字符的反向不对称） ----
#[test]
fn pinyin_symbol_release_matches_press() {
    let mut s = pinyin_state();
    // 半角态（用户按了全角切换键）才有直通的标点：全角态下 `.` 出 `。`、已不是直通。
    // 记结论的 `last_printable` 只在**按下直通**时才是 true，故这条必须以半角为背景。
    s.engine_mut().toggle_fullwidth();
    assert_eq!(s.key_event('.' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        s.key_event('.' as u32, KEY_STATE_RELEASED),
        KeyAction::PassThrough,
        "按下直通、抬起拦下 → 客户端只有 keydown"
    );
}

#[test]
fn number_mode_digit_release_matches_press() {
    let mut s = pinyin_state();
    s.switch_mode(Mode::Number);
    // Number 模式下整排可见 ASCII 直通；抬起同样直通（否则整排数字卡键）
    assert_eq!(s.key_event('2' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        s.key_event('2' as u32, KEY_STATE_RELEASED),
        KeyAction::PassThrough
    );
}

#[test]
fn digit_without_candidates_release_matches_press() {
    let mut s = pinyin_state();
    // 无候选：'1' 直通给客户端当数字输入；抬起亦直通
    assert_eq!(s.key_event('1' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        s.key_event('1' as u32, KEY_STATE_RELEASED),
        KeyAction::PassThrough
    );
}

#[test]
fn release_of_another_key_falls_back_to_handled() {
    // 单槽记录的天花板：另一键按下会顶掉记录（键盘 rollover 时才会遇到），
    // 键值不匹配时回落到旧行为（拦下），绝不误放行。
    let mut s = pinyin_state();
    s.engine_mut().toggle_fullwidth(); // 半角态：`.` 直通（全角态下它出 `。`）
    assert_eq!(s.key_event('.' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        s.key_event('n' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
}

// ---- 特殊键抬起须与按下同判（按下会改缓冲，判「当前」缓冲必然不对称） ----
// 缺陷形态（v1.0.11 只修了可打印分支的那一半）：退格/回车的抬起分支判
// `buffer().is_empty()`，判的是**当前**缓冲，而按下分支 handle_backspace /
// handle_enter 会把它改空（退格删掉最后一个字符、回车提交并清缓冲）。
// 于是单字符退格、任意回车提交之后，抬起返回 PassThrough 而按下是 EngineHandled
// → 客户端收到**无 keydown 的 keyup**（依赖键状态的控件卡键）。
// 修法同可打印分支：按下记结论（`last_printable`），抬起按同一结论回复。
#[test]
fn backspace_release_matches_press_after_buffer_emptied() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event('n' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.key_event(KEY_BACK_SPACE, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "", "前置：退格已把缓冲删空");
    assert_eq!(
        s.key_event(KEY_BACK_SPACE, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "按下拦下、抬起放行 → 客户端只有 keydown"
    );
    assert_eq!(s.buffer(), "", "抬起事件不得改动缓冲");
}

#[test]
fn enter_release_matches_press_after_commit() {
    let mut s = state_with_candidates();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(KEY_RETURN, 0), KeyAction::Input("词00".into()));
    assert_eq!(s.buffer(), "", "前置：回车已提交并清空缓冲");
    assert_eq!(
        s.key_event(KEY_RETURN, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "按下提交、抬起放行 → 客户端只有 keydown"
    );
}

// ---- 键码空间的退化行为：特殊键与「可打印 = Unicode 码点」不相交 ----
// 常量本身的不变式在 keys.rs 的 `special_key_space_stays_out_of_ascii`；
// 这里验的是**路由在漏匹配时的行为**（TSF 轨曾被 VK 与 ASCII 撞号咬过：
// VK_NEXT=0x22=`"`、VK_DELETE=0x2E=`.` → 打 `.` 变退格）。

#[test]
fn missed_special_key_degrades_to_pass_through() {
    // 将来加键时漏了 match 臂（此处用未定义码 0x90 模拟）：必须退化成「交系统」，
    // 而不是被当可打印字符吃进缓冲。两轨的硬要求。
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(SPECIAL_BASE | 0x90, 0), KeyAction::PassThrough);
    assert_eq!(
        s.key_event(SPECIAL_BASE | 0x90, KEY_STATE_RELEASED),
        KeyAction::PassThrough
    );
    assert_eq!(s.buffer(), "hao");
}

#[test]
fn ascii_collision_printables_not_stolen_by_special_keys() {
    // '.'=0x2E / '!'=0x21 / '"'=0x22 是 TSF 轨真出过 bug 的三个码位（与 VK_DELETE /
    // VK_PRIOR / VK_NEXT 同值）：**绝不能**被退格/翻页分支吃掉（`.` 变退格）。
    // 2026-09-27 起它们会出中文标点、并先把待提交的缓冲上屏 —— 旧版断言的是
    // 「直通且缓冲原样」，标点表落地后那条已不成立；这里改钉真正要保的东西：
    // 出的是标点文本、缓冲是**被 flush 上屏**而不是被退格吃掉、页码没被翻。
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    let out = s.key_event('.' as u32, 0);
    assert!(
        matches!(out, KeyAction::Input(_)),
        "'.' 必须是标点上屏，不是特殊键：{out:?}"
    );
    assert!(s.buffer().is_empty(), "缓冲上屏了，不是被退格吃掉");
    assert_eq!(s.page(), 0, "页码不得被这些字符改动");
    for c in ['!', '"'] {
        let out = s.key_event(c as u32, 0);
        assert!(
            matches!(out, KeyAction::Input(_)),
            "{c:?} 被特殊键抢走了码位：{out:?}"
        );
    }
}
