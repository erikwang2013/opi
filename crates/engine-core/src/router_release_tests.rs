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

// ---- 可打印字符抬起须与按下同判（直通字符的反向不对称） ----
#[test]
fn pinyin_symbol_release_matches_press() {
    let mut s = pinyin_state();
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
    assert_eq!(s.key_event('.' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        s.key_event('n' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
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
    // VK_PRIOR / VK_NEXT 同值）：拼音缓冲非空时必须直通且缓冲原样，绝不能被
    // 退格/翻页分支吃掉。Apple 侧走的是 Unicode 码点，这层保护必须成立。
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    for c in ['.', '!', '"'] {
        assert_eq!(
            s.key_event(c as u32, 0),
            KeyAction::PassThrough,
            "可打印字符 {c:?} 被特殊键抢走了码位"
        );
    }
    assert_eq!(s.buffer(), "hao", "缓冲不得被吃掉");
    assert_eq!(s.page(), 0, "页码不得被这些字符改动");
}
