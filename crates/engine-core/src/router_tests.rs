// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 平台中立键路由的语义测试：**逐条对标两轨**（`crates/fcitx5-opi/src/input_method_tests.rs`
//! 与 `crates/tsf-opi/src/logic_tests.rs`，两份同名同序）。用词对照：本层 `PassThrough`
//! = fcitx5 `PassThrough` = tsf `Unhandled`；本层 `EngineHandled` = fcitx5 `EngineHandled`
//! = tsf `CompositionChanged`/`Consumed`；本层 `Input` = fcitx5 `Input` = tsf `Commit`。
//!
//! 键码用中立编码：可打印字符 = Unicode 码点，特殊键 = `SPECIAL_BASE | 低 16 位`
//! （两轨的 keysym/VK 差异在各自的测试里，本层不出现）。

use super::*;
use crate::dictionary::InMemoryDictionary;
use crate::symbols::SymbolEngine;

/// 20 个 "hao" 词条 + 引擎：确定性的 3 页候选。
fn state() -> KeyRouter {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    KeyRouter::new(Engine::new(Box::new(d), SymbolEngine::builtin(), true))
}

fn pinyin_state() -> KeyRouter {
    let mut s = state();
    s.switch_mode(Mode::Pinyin);
    s
}

// ---- 拼音模式：字母入缓冲 ----
#[test]
fn pinyin_letter_goes_to_engine_buffer() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "a");
    assert_eq!(s.key_event('b' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "ab");
}

#[test]
fn pinyin_uppercase_codepoint_lowercased_into_buffer() {
    // 物理 shift 已由平台应用：键值为 'A'（两轨分别来自 xkb keysym / ToUnicodeEx）；
    // composer 转小写入缓冲。
    let mut s = pinyin_state();
    assert_eq!(s.key_event('A' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "a");
}

/// 标点表（2026-09-27）：ASCII 标点在中文模式出中文标点 —— 旧版这条叫
/// `pinyin_symbol_passes_through`，断言 `,` 直通；用户裁决加了引擎层标点表后，
/// 直通只剩**非标点**字符（这里用非 ASCII 的 `，`：它不是一个键，是已经成形的文本）。
#[test]
fn pinyin_ascii_punctuation_maps_to_chinese() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event('，' as u32, 0), KeyAction::PassThrough);
    assert_eq!(s.key_event(',' as u32, 0), KeyAction::Input("，".into()));
    assert_eq!(s.buffer(), "");
}

/// 撇号：缓冲空时是引号（标点表），缓冲非空时是**音节分隔符**（`xi'an`）。
/// 旧版这条叫 `pinyin_apostrophe_goes_to_buffer`，只钉了后半段。
#[test]
fn pinyin_apostrophe_is_separator_only_while_composing() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event('\'' as u32, 0), KeyAction::Input("‘".into()));
    assert_eq!(s.buffer(), "", "缓冲空：撇号没有分隔语义，当引号");
    for c in ['x', 'i'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event('\'' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "xi'", "缓冲非空：音节分隔符，入缓冲");
}

// ---- 空格 ----
#[test]
fn space_with_buffer_commits_top_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(KEY_SPACE, 0), KeyAction::Input("词00".into()));
    assert_eq!(s.buffer(), "");
}

#[test]
fn space_with_empty_buffer_commits_space() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event(KEY_SPACE, 0), KeyAction::Input(" ".into()));
    assert_eq!(s.buffer(), "");
}

// ---- 回车 ----
#[test]
fn enter_with_buffer_selects_first_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(KEY_RETURN, 0), KeyAction::Input("词00".into()));
    assert_eq!(s.buffer(), "");
}

#[test]
fn enter_with_empty_buffer_passes_through() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event(KEY_RETURN, 0), KeyAction::PassThrough);
}

// ---- 退格 ----
#[test]
fn backspace_with_buffer_deletes_codepoint() {
    let mut s = pinyin_state();
    for c in ['a', 'b'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(KEY_BACK_SPACE, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "a");
    assert_eq!(s.key_event(KEY_BACK_SPACE, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "");
    // 空缓冲 → 直通
    assert_eq!(s.key_event(KEY_BACK_SPACE, 0), KeyAction::PassThrough);
}

#[test]
fn backspace_release_event_consumed() {
    let mut s = pinyin_state();
    s.key_event('a' as u32, 0);
    assert_eq!(
        s.key_event(KEY_BACK_SPACE, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.buffer(), "a");
}

// ---- Delete（与退格同一路由分支） ----
#[test]
fn delete_mirrors_backspace() {
    let mut s = pinyin_state();
    for c in ['a', 'b'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(KEY_DELETE, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "a");
    s.key_event(KEY_DELETE, 0);
    assert_eq!(s.buffer(), "");
    assert_eq!(s.key_event(KEY_DELETE, 0), KeyAction::PassThrough);
    // 释放事件被消费，不删字符
    s.key_event('a' as u32, 0);
    assert_eq!(
        s.key_event(KEY_DELETE, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.buffer(), "a");
}

// ---- Tab / Esc / 方向键：一律直通（不拦截，不消费） ----
#[test]
fn tab_and_esc_pass_through() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(KEY_TAB, 0), KeyAction::PassThrough);
    assert_eq!(s.key_event(KEY_ESCAPE, 0), KeyAction::PassThrough);
    assert_eq!(s.buffer(), "hao"); // 缓冲未被吞
}

#[test]
fn arrow_keys_pass_through_press_and_release() {
    // 两轨里方向键靠「非 ASCII 键值」落到默认分支直通；本层为它们定了显式键码，
    // 按下与抬起都必须直通（否则客户端只有 keydown，依赖键状态的控件卡键）。
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    for key in [KEY_UP, KEY_DOWN, KEY_LEFT, KEY_RIGHT] {
        assert_eq!(
            s.key_event(key, 0),
            KeyAction::PassThrough,
            "按下直通（{key}）"
        );
        assert_eq!(
            s.key_event(key, KEY_STATE_RELEASED),
            KeyAction::PassThrough,
            "抬起同判（{key}）"
        );
    }
    assert_eq!(s.buffer(), "hao");
}

// ---- 英文模式直传（镜像 handleKey） ----
fn english_state() -> KeyRouter {
    let mut s = state();
    s.switch_mode(Mode::English);
    s
}

#[test]
fn english_empty_buffer_lowercase_passes_through() {
    let mut s = english_state();
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::Input("a".into()));
    assert_eq!(s.buffer(), "");
}

#[test]
fn english_empty_buffer_single_shift_commits_upper_and_consumes() {
    let mut s = english_state();
    s.shift_tap(); // Off → Single
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::Input("A".into()));
    // single 已消费：下个字母小写
    assert_eq!(s.key_event('b' as u32, 0), KeyAction::Input("b".into()));
    assert_eq!(s.shift_state(), ShiftState::Off);
    assert_eq!(s.buffer(), "");
}

#[test]
fn english_empty_buffer_lock_keeps_uppercase() {
    let mut s = english_state();
    s.shift_long_press(); // Lock
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::Input("A".into()));
    // Lock 不被消费
    assert_eq!(s.key_event('b' as u32, 0), KeyAction::Input("B".into()));
    assert_eq!(s.shift_state(), ShiftState::Lock);
    assert_eq!(s.buffer(), "");
}

#[test]
fn english_empty_buffer_physical_shift_codepoint_passes_upper() {
    // 平台已应用物理 shift：键值为 'A' + SHIFT 位，仍直传且消费 single
    let mut s = english_state();
    s.shift_tap();
    assert_eq!(
        s.key_event('A' as u32, KEY_STATE_SHIFT),
        KeyAction::Input("A".into())
    );
    assert_eq!(s.shift_state(), ShiftState::Off);
}

#[test]
fn english_letters_always_direct_commit_buffer_stays_empty() {
    // 镜像 KeyRouter.handleKey：英文模式空缓冲每次直传，缓冲永远为空，
    // 故字母永不入引擎缓冲（与 Android 行为一致）
    let mut s = english_state();
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::Input("a".into()));
    assert_eq!(s.key_event('b' as u32, 0), KeyAction::Input("b".into()));
    assert_eq!(s.buffer(), "");
    // 符号直传
    assert_eq!(s.key_event(',' as u32, 0), KeyAction::Input(",".into()));
}

#[test]
fn english_nonempty_buffer_letter_goes_to_engine() {
    // 防御路径：仅当缓冲意外非空（如经 engine 直入）时才入引擎，
    // 镜像 KeyRouter 的 else 分支
    let mut s = english_state();
    s.engine_mut().input_key('x'); // 绕过路由器直入引擎缓冲
    assert_eq!(s.buffer(), "x");
    assert_eq!(s.key_event('b' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "xb");
    // 缓冲非空时符号直通
    assert_eq!(s.key_event(',' as u32, 0), KeyAction::PassThrough);
}

#[test]
fn english_nonempty_buffer_shift_uppercases_in_engine() {
    // 引擎 shift 打开时，非空缓冲路径由 composer 大写（镜像同一分支）
    let mut s = english_state();
    s.engine_mut().input_key('x');
    s.shift_tap();
    assert_eq!(s.key_event('b' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "xB");
}

// ---- ⇧ 状态机（镜像 EngineController.shiftTap） ----
#[test]
fn shift_tap_cycles_off_single_off() {
    let mut s = english_state();
    assert_eq!(s.key_event(KEY_SHIFT, 0), KeyAction::EngineHandled);
    assert_eq!(s.shift_state(), ShiftState::Single);
    assert_eq!(s.key_event(KEY_SHIFT, 0), KeyAction::EngineHandled);
    assert_eq!(s.shift_state(), ShiftState::Off);
}

#[test]
fn shift_release_and_repeat_do_not_toggle() {
    let mut s = english_state();
    assert_eq!(
        s.key_event(KEY_SHIFT, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.shift_state(), ShiftState::Off);
    s.shift_tap();
    assert_eq!(
        s.key_event(KEY_SHIFT, KEY_STATE_REPEAT),
        KeyAction::EngineHandled
    );
    assert_eq!(s.shift_state(), ShiftState::Single);
}

#[test]
fn shift_long_press_locks() {
    let mut s = english_state();
    assert_eq!(
        s.key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.shift_state(), ShiftState::Lock);
}

#[test]
fn switch_mode_clears_frontend_shift_lock() {
    // 与 Android EngineController.resetShift 同源：切模式须清**前端** ⇧ 状态。
    // 英文模式空缓冲直传的大小写由 ShiftState 决定（不查 composer 的 shift），
    // 故 Lock 残留时切回英文仍全大写 —— 引擎侧清了不足以覆盖这条。
    let mut s = english_state();
    s.key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED);
    assert_eq!(s.shift_state(), ShiftState::Lock, "前置：长按已锁定");

    s.switch_mode(Mode::Pinyin);

    assert_eq!(
        s.shift_state(),
        ShiftState::Off,
        "切模式须清前端 ⇧（Lock 不得跨模式残留）"
    );
}

// ---- 候选选择与翻页 ----
#[test]
fn digit_selects_candidate_page_relative() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    // '2' → 页内索引 1
    assert_eq!(s.key_event('2' as u32, 0), KeyAction::Input("词01".into()));
    assert_eq!(s.buffer(), "");
}

#[test]
fn digit_one_selects_first_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    // '1' → 页内索引 0，选中首候选
    assert_eq!(s.key_event('1' as u32, 0), KeyAction::Input("词00".into()));
    assert_eq!(s.buffer(), "");
}

#[test]
fn digit_out_of_range_passes_through() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    s.key_event(KEY_PAGE_DOWN, 0);
    s.key_event(KEY_PAGE_DOWN, 0); // 末页 4 个候选（词16..词19）
    // '9' → 页内索引 8 越界 → 直通
    assert_eq!(s.key_event('9' as u32, 0), KeyAction::PassThrough);
    // '0' → 无索引 → 直通
    assert_eq!(s.key_event('0' as u32, 0), KeyAction::PassThrough);
}

#[test]
fn digit_without_candidates_passes_through() {
    let mut s = pinyin_state();
    // 无候选（缓冲空 / 无词条缓冲）
    assert_eq!(s.key_event('1' as u32, 0), KeyAction::PassThrough);
    s.key_event('x' as u32, 0);
    s.key_event('x' as u32, 0); // "xx" 无候选
    assert_eq!(s.key_event('1' as u32, 0), KeyAction::PassThrough);
}

#[test]
fn page_keys_navigate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.key_event(c as u32, 0);
    }
    assert_eq!(s.key_event(KEY_PAGE_DOWN, 0), KeyAction::EngineHandled);
    assert_eq!(s.page(), 1);
    assert_eq!(s.key_event(KEY_PAGE_DOWN, 0), KeyAction::EngineHandled);
    assert_eq!(s.key_event(KEY_PAGE_DOWN, 0), KeyAction::EngineHandled); // 钳制
    assert_eq!(s.page(), 2);
    assert_eq!(s.key_event(KEY_PAGE_UP, 0), KeyAction::EngineHandled);
    assert_eq!(s.page(), 1);
    // 翻页后数字选词是**页内**索引：第 2 页首位 = 词08
    assert_eq!(s.key_event('1' as u32, 0), KeyAction::Input("词08".into()));
}

// ---- 修饰键与模式 ----
#[test]
fn ctrl_and_alt_combos_pass_through() {
    let mut s = pinyin_state();
    s.key_event('a' as u32, 0);
    assert_eq!(
        s.key_event('c' as u32, KEY_STATE_CTRL),
        KeyAction::PassThrough
    );
    assert_eq!(
        s.key_event('x' as u32, KEY_STATE_CTRL | KEY_STATE_SHIFT),
        KeyAction::PassThrough
    );
    assert_eq!(
        s.key_event('a' as u32, KEY_STATE_ALT),
        KeyAction::PassThrough
    );
    assert_eq!(s.buffer(), "a"); // 未被吞
}

#[test]
fn command_combo_passes_through() {
    // 相对两轨的唯一位扩展（见 KEY_STATE_META 注释）：Apple 的 ⌘ 组合是系统快捷键，
    // 不拦下的话 ⌘A 会把 'a' 吃进拼音缓冲、⌘C 会吞掉复制。
    let mut s = pinyin_state();
    s.key_event('a' as u32, 0);
    assert_eq!(
        s.key_event('a' as u32, KEY_STATE_META),
        KeyAction::PassThrough
    );
    assert_eq!(
        s.key_event('c' as u32, KEY_STATE_META),
        KeyAction::PassThrough
    );
    assert_eq!(
        s.key_event(KEY_BACK_SPACE, KEY_STATE_META),
        KeyAction::PassThrough
    );
    assert_eq!(s.buffer(), "a");
}

/// Symbol 模式曾与 Number 同臂直通；B3 把它补成真模式后不再直通，
/// 符号模式的键路由见 router_symbol_tests.rs（本文件只留 Number 这一半）。
#[test]
fn number_mode_passes_through() {
    let mut s = pinyin_state();
    s.switch_mode(Mode::Number);
    assert_eq!(s.key_event('2' as u32, 0), KeyAction::PassThrough);
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::PassThrough);
    assert_eq!(s.buffer(), "");
}

#[test]
fn non_ascii_keyval_passes_through() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event(0x4e2d, 0), KeyAction::PassThrough); // 中
    assert_eq!(s.key_event(u32::MAX, 0), KeyAction::PassThrough);
    assert_eq!(s.buffer(), "");
}

#[test]
fn mode_switch_clears_buffer_then_english_direct() {
    // 拼音输入后切英文：缓冲清空，英文空缓冲直传
    let mut s = pinyin_state();
    s.key_event('a' as u32, 0);
    assert_eq!(s.buffer(), "a");
    s.switch_mode(Mode::English);
    assert_eq!(s.buffer(), "");
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::Input("a".into()));
}

// ---- 可打印字符的抬起不得二次入引擎（回归：拼音缓冲翻倍 / 英文重复提交） ----
// 缺陷形态：`_ =>` 可打印分支漏判 `released`，而同一函数里每个特殊键分支都判了。
#[test]
fn printable_release_does_not_refeed_pinyin_buffer() {
    let mut s = pinyin_state();
    assert_eq!(s.key_event('n' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(
        s.key_event('n' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.buffer(), "n", "抬起事件不得再次送入引擎（否则缓冲翻倍）");
    s.key_event('i' as u32, 0);
    s.key_event('i' as u32, KEY_STATE_RELEASED);
    assert_eq!(s.buffer(), "ni", "两键按下+抬起后应为 ni，而非 nnii");
}

#[test]
fn printable_release_does_not_double_commit_english() {
    let mut s = english_state();
    // 英文空缓冲直传：按下提交一次
    assert_eq!(s.key_event('a' as u32, 0), KeyAction::Input("a".into()));
    // 抬起不得再提交一次（否则 "a" → "aa"）
    assert_eq!(
        s.key_event('a' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
}
