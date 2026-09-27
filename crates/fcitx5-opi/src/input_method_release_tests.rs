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
use engine_core::dictionary::InMemoryDictionary;

/// 每个测试文件自带状态构造（与 input_method_tests.rs 同惯例）。多数用例不需要
/// 候选，用内置回退词库即可。
fn pinyin_state() -> CandidateState {
    let mut s = CandidateState::load(None).expect("内置回退词库");
    s.switch_mode(Mode::Pinyin);
    s
}

/// 20 条 "hao" 词条（与 input_method_tests.rs 的 `state()` 同惯例）：**提交真的会
/// 发生**，回车按下会把缓冲清空 —— 特殊键用例需要「按下改动了缓冲」这个前提。
fn state_with_candidates() -> CandidateState {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    let mut s = CandidateState {
        engine: engine_core::Engine::new(Box::new(d), symbols, true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
    };
    s.buffer_snapshot = s.engine.buffer().to_string();
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

// ---- 特殊键抬起须与按下同判（按下会改缓冲，判「当前」缓冲必然不对称） ----
// 缺陷形态（v1.0.11 只修了可打印分支的那一半）：退格/回车的抬起分支判
// `buffer().is_empty()`，判的是**当前**缓冲，而按下分支 handle_backspace /
// handle_enter 会把它改空（退格删掉最后一个字符、回车提交并清缓冲）。
// 于是单字符退格、任意回车提交之后，抬起返回 PassThrough 而按下是 EngineHandled
// → 客户端收到**无 keydown 的 keyup**（依赖键状态的游戏/编辑器卡键）。
// 修法同可打印分支：按下记结论（`last_printable`），抬起按同一结论回复。
#[test]
fn backspace_release_matches_press_after_buffer_emptied() {
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, 'n' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(
        handle_key(&mut s, KEY_BACK_SPACE, 0),
        KeyAction::EngineHandled
    );
    assert_eq!(s.buffer(), "", "前置：退格已把缓冲删空");
    assert_eq!(
        handle_key(&mut s, KEY_BACK_SPACE, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "按下拦下、抬起放行 → 客户端只有 keydown"
    );
    assert_eq!(s.buffer(), "", "抬起事件不得改动缓冲");
}

#[test]
fn enter_release_matches_press_after_commit() {
    let mut s = state_with_candidates();
    for c in ['h', 'a', 'o'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(
        handle_key(&mut s, KEY_RETURN, 0),
        KeyAction::Input("词00".into())
    );
    assert_eq!(s.buffer(), "", "前置：回车已提交并清空缓冲");
    assert_eq!(
        handle_key(&mut s, KEY_RETURN, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "按下提交、抬起放行 → 客户端只有 keydown"
    );
}
