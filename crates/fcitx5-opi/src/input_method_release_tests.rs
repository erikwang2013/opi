// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 可打印键「按下/抬起同判」的回归测试（`#[path]` 引入 input_method.rs，保持各文件 <500 行）。
//!
//! 缺陷形态：handle_key 里每个特殊键分支都判了 `released`，可打印分支此前漏判 ——
//! 一个字符被送进引擎两次（拼音缓冲翻倍 "ni"→"nnii"、英文重复提交 "a"→"aa"）；
//! 上一轮修掉了这一半，但修成「抬起一律拦下」，于是反向不对称：
//! 拼音/繁体的非字母符号（'.'、'-' 等）、无候选或越界的数字、Number/Symbol 模式下的
//! 全部可见 ASCII 在按下时是 PassThrough（直通客户端），抬起却 EngineHandled
//! （拦下）→ 客户端收到 keydown 收不到 keyup（依赖键状态的游戏/编辑器卡键）。
//! 上一轮的注释写着「抬起须与按下同判」，40 行后就被代码否认 —— 现在两者一致了。
//! 两轨用词对照：tsf 轨的 Unhandled/Consumed = 本轨的 PassThrough/EngineHandled。
//! 同源的 tsf 轨用例见 crates/tsf-opi/src/logic_release_tests.rs（同名同序，逐条对应）。

use super::*;

/// 每个测试文件自带状态构造（与 input_method_tests.rs 同惯例）。本组用例都不需要
/// 候选，用内置回退词库即可。
fn pinyin_state() -> CandidateState {
    let mut s = CandidateState::load(None).expect("内置回退词库");
    s.switch_mode(Mode::Pinyin);
    s
}

// ---- 可打印字符抬起须与按下同判（直通字符的反向不对称） ----
// 缺陷形态：抬起一律 EngineHandled，而按下走 handle_printable —— 拼音/繁体的
// 非字母符号（'.'、'-'）、无候选或越界的数字、Number/Symbol 模式下的全部可见
// ASCII 都返回 PassThrough。于是客户端收到 keydown 收不到 keyup（依赖键状态的
// 游戏/编辑器卡键）。修法：记住按下的结论（last_printable），抬起按同一结论回复。
#[test]
fn pinyin_symbol_release_matches_press() {
    let mut s = pinyin_state();
    // 本轨没有 tsf 轨那类码位冲突：特殊键用 keysym 0xffxx，'.'=0x2E 不与任何特殊键同值。
    assert_eq!(handle_key(&mut s, '.' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        handle_key(&mut s, '.' as u32, KEY_STATE_RELEASED),
        KeyAction::PassThrough,
        "按下直通、抬起拦下 → 客户端只有 keydown"
    );
}

#[test]
fn number_mode_digit_release_matches_press() {
    let mut s = pinyin_state();
    s.switch_mode(Mode::Number);
    // Number 模式下整排可见 ASCII 直通；抬起同样直通（否则整排数字卡键）
    assert_eq!(handle_key(&mut s, '2' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        handle_key(&mut s, '2' as u32, KEY_STATE_RELEASED),
        KeyAction::PassThrough
    );
}

#[test]
fn digit_without_candidates_release_matches_press() {
    let mut s = pinyin_state();
    // 无候选：'1' 直通给客户端当数字输入；抬起亦直通
    assert_eq!(handle_key(&mut s, '1' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        handle_key(&mut s, '1' as u32, KEY_STATE_RELEASED),
        KeyAction::PassThrough
    );
}

#[test]
fn release_of_another_key_falls_back_to_handled() {
    // 单槽记录的天花板：另一键按下会顶掉记录（键盘 rollover 时才会遇到），
    // 键值不匹配时回落到旧行为（拦下），绝不误放行。
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, '.' as u32, 0), KeyAction::PassThrough);
    assert_eq!(
        handle_key(&mut s, 'n' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
}
