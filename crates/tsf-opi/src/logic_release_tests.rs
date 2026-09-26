//! 可打印键「按下/抬起同判」的回归测试（`#[path]` 引入 logic.rs，保持各文件 <500 行）。
//!
//! 缺陷形态：本函数上面每个特殊键分支都判了 `released`，可打印分支此前漏判 ——
//! 一个字符被送进引擎两次（拼音缓冲翻倍 "ni"→"nnii"、英文重复提交 "a"→"aa"）；
//! 上一轮修掉了这一半，但修成「抬起一律拦下」，于是反向不对称：
//! 拼音/繁体的非字母符号（'-' 等）、无候选或越界的数字、Number/Symbol 模式下的
//! 全部可见 ASCII 在按下时是 Unhandled（不拦截，键流入应用），抬起却 Consumed
//! （吞键）→ 应用收到 keydown 收不到 keyup（依赖键状态的游戏/编辑器卡键）。
//! 上一轮的注释写着「抬起须与按下同判」，40 行后就被代码否认 —— 现在两者一致了。
//! 同源的 fcitx5 轨用例见 crates/fcitx5-opi/src/input_method_release_tests.rs。

use super::*;

/// 每个测试文件自带状态构造（与 logic_tests.rs / logic_candidate_tests.rs 同惯例）。
/// 本组用例都不需要候选，用内置回退词库即可。
fn pinyin_state() -> TsfLogic {
    let mut s = TsfLogic::load(None).expect("内置回退词库");
    s.switch_mode(Mode::Pinyin);
    s
}

// ---- 可打印字符抬起须与按下同判（直通字符的反向不对称） ----
// 缺陷形态与 fcitx5 轨同源（见 input_method.rs 对应分支的注释）：抬起一律
// Consumed，而按下走 handle_printable —— 拼音/繁体的非字母符号（'.'、'-'）、
// 无候选或越界的数字、Number/Symbol 模式下的全部可见 ASCII 都返回 Unhandled。
// TSF 下 Unhandled = 不拦截（BOOL FALSE，键流入应用），Consumed = 吞键：
// 不对称即「应用收到 keydown 收不到 keyup」。
#[test]
fn pinyin_symbol_release_matches_press() {
    let mut s = pinyin_state();
    // '.' 曾是本轨最典型的受害者：0x2E 与 KEY_DELETE 同值，缓冲非空时被当退格。
    // 特殊键改编码（SPECIAL_BASE|VK，见 logic.rs）后它才是普通的直通符号；
    // vk.rs 另有 printable_ascii_is_not_hijacked_by_special_keys 钉住码位不相交。
    assert_eq!(s.input_key('.' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(
        s.input_key('.' as u32, KEY_STATE_RELEASED),
        KeyOutcome::Unhandled,
        "按下放行、抬起拦下 → 应用只有 keydown"
    );
}

#[test]
fn number_mode_digit_release_matches_press() {
    let mut s = pinyin_state();
    s.switch_mode(Mode::Number);
    // Number 模式下整排可见 ASCII 放行（Unhandled）；抬起同样放行（否则整排数字卡键）
    assert_eq!(s.input_key('2' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(
        s.input_key('2' as u32, KEY_STATE_RELEASED),
        KeyOutcome::Unhandled
    );
}

#[test]
fn digit_without_candidates_release_matches_press() {
    let mut s = pinyin_state();
    // 无候选：'1' 放行给应用当数字输入；抬起同样放行
    assert_eq!(s.input_key('1' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(
        s.input_key('1' as u32, KEY_STATE_RELEASED),
        KeyOutcome::Unhandled
    );
}

#[test]
fn release_of_another_key_falls_back_to_consumed() {
    // 单槽记录的天花板：另一键按下会顶掉记录（键盘 rollover 时才遇到），
    // 键值不匹配时回落到旧行为（吞掉），绝不误放行。
    let mut s = pinyin_state();
    assert_eq!(s.input_key('.' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(
        s.input_key('n' as u32, KEY_STATE_RELEASED),
        KeyOutcome::Consumed
    );
}
