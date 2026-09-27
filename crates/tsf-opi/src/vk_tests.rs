// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 由 vk.rs 搬出（`#[path]` 引入）以保持本文件 <500 行，与 input_method_tests.rs 同惯例。

use super::*;

// 缺陷形态：`ToUnicodeEx` 对导航键/小键盘返回 0（无映射），旧代码在此回退成
// VK 本身 —— 等于把「wParam 当码点」在错误路径上复活，方向键被吃进拼音缓冲、
// 小键盘凭空敲出字母。回退必须是放行哨兵。
#[test]
fn unmapped_vk_never_becomes_printable() {
    // VK_RIGHT = 0x27 = '\''：按下会被当撇号进缓冲，且方向键被吞
    assert_eq!(vk_to_engine_keycode(0x27, 0, 0), NO_KEYVAL);
    // VK_LEFT = 0x25 = '%'、VK_UP = 0x26 = '&'、VK_DOWN = 0x28 = '('
    for vk in [0x25, 0x26, 0x28] {
        assert_eq!(vk_to_engine_keycode(vk, 0, 0), NO_KEYVAL);
    }
}

#[test]
fn numpad_with_numlock_off_never_becomes_letters() {
    // NumLock 关时小键盘 1..9 = VK_NUMPAD1..9 = 0x61..0x69 = 'a'..'i'
    for vk in 0x61..=0x69 {
        let kc = vk_to_engine_keycode(vk, 0, 0);
        assert_eq!(kc, NO_KEYVAL, "VK {vk:#x} 不得变成字母");
        assert!(char::from_u32(kc).is_none());
    }
    // VK_NUMPAD0 = 0x60 = '`'，numpad Del（NumLock 关）= VK_DECIMAL=0x6E='n'
    assert_eq!(vk_to_engine_keycode(0x60, 0, 0), NO_KEYVAL);
    assert_eq!(vk_to_engine_keycode(0x6e, 0, 0), NO_KEYVAL);
}

#[test]
fn no_keyval_passes_through_engine() {
    // 哨兵在逻辑层的语义：不接管（TSF 侧 BOOL FALSE，键流入应用）
    use crate::logic::{KeyOutcome, TsfLogic};
    let mut s = TsfLogic::load(None).expect("内置回退词库");
    assert_eq!(s.input_key(NO_KEYVAL, 0), KeyOutcome::Unhandled);
    assert_eq!(s.buffer(), "");
}

#[test]
fn mapped_key_uses_unicode_codepoint() {
    // 正常可打印键：ToUnicodeEx 返回 1 → 取码点（'a' 而非 VK_A=0x41）
    assert_eq!(vk_to_engine_keycode(0x41, 1, b'a' as u16), b'a' as u32);
    // 死键（-1）同样放行，不退化成 VK
    assert_eq!(vk_to_engine_keycode(0x41, -1, 0), NO_KEYVAL);
}

#[test]
fn special_vks_pass_through_as_their_keycode() {
    // 特殊键走编码键码、不取 ToUnicodeEx 的字符：VK_BACK 的映射结果是退格
    // 控制符 0x08，它仍然 is_ascii()，会被 handle_printable 当普通符号放行，退格就废了
    assert_eq!(vk_to_engine_keycode(0x08, 8, 0x08), KEY_BACK_SPACE); // VK_BACK
    assert_eq!(vk_to_engine_keycode(0x0d, 1, 0x0d), KEY_RETURN); // VK_RETURN
    assert_eq!(vk_to_engine_keycode(0x20, 1, 0x20), KEY_SPACE); // VK_SPACE
    // 取不到键盘状态（n = -1）时特殊键仍可用
    assert_eq!(vk_to_engine_keycode(0x2e, -1, 0), KEY_DELETE); // VK_DELETE
}

// ---- 特殊键空间与「可打印字符 = Unicode 码点」不相交（第 6 条） ----

/// 拼音模式、缓冲非空（"ni"）。
fn pinyin_with_buffer() -> crate::logic::TsfLogic {
    use crate::logic::TsfLogic;
    use engine_core::composer::Mode;
    let mut s = TsfLogic::load(None).expect("内置回退词库");
    s.switch_mode(Mode::Pinyin);
    for k in ['n', 'i'] {
        s.input_key(k as u32, 0);
    }
    assert_eq!(s.buffer(), "ni");
    s
}

// 缺陷形态：可打印 ASCII 被特殊键常量占了码位 —— '.'=0x2E=VK_DELETE、
// '!'=0x21=VK_PRIOR、'"'=0x22=VK_NEXT。敲 '.' 在拼音缓冲非空时走退格分支
// 删掉拼音字母（'.' 还是最常用的符号之一），'!'/'"' 则翻候选页。
#[test]
fn printable_ascii_is_not_hijacked_by_special_keys() {
    // 只压**映射**这一层（本文件的责任）：可打印字符必须映射成它自己的码点，
    // 而不是特殊键空间里的码（后者才会被引擎当退格/翻页）。
    //
    // 这里**不再**压"引擎拿到它之后返回 Unhandled"：'.' '!' 这些如今是标点层
    // 的输入（引擎会提交并清空缓冲），那是标点层的既有设计 —— 断言它属于
    // logic 层的测试，压在这里只会随标点表的增删反复变红。
    // VK：'.' = VK_OEM_PERIOD 0xBE；'!' = Shift+'1'（'1' = 0x31）；
    // '"' = Shift+VK_OEM_7 0xDE。第三参 = ToUnicodeEx 已算好的字符。
    for (vk, ch) in [(0xbeu32, '.'), (0x31, '!'), (0xde, '"')] {
        let mapped = vk_to_engine_keycode(vk, 1, ch as u16);
        assert_eq!(mapped, ch as u32, "{ch:?}（VK {vk:#x}）没映射成自己的码点");
        assert!(
            !SPECIAL_KEYS.contains(&mapped),
            "{ch:?} 映射进了特殊键空间 {mapped:#x}：引擎会当退格/翻页，交不了应用"
        );
    }
}

// 分离的另一半：Delete 的删除功能没被"让出 '.'"弄丢，且物理 Delete 键仍是删除。
#[test]
fn vk_delete_deletes_but_period_key_does_not() {
    use crate::logic::KeyOutcome;
    // 物理 Delete 键：VK_DELETE=0x2E → 编码键码 → 真退格
    let mut s = pinyin_with_buffer();
    assert_eq!(
        s.input_key(vk_to_engine_keycode(0x2e, 0, 0), 0),
        KeyOutcome::CompositionChanged
    );
    assert_eq!(s.buffer(), "n");
    // 句点键：VK_OEM_PERIOD=0xBE，ToUnicodeEx 给 '.' → 映射成码点，**不是**
    // KEY_DELETE（否则这一下会再删掉一个字母）。同样只压映射层，理由同上。
    let period = vk_to_engine_keycode(0xbe, 1, '.' as u16);
    assert_ne!(period, KEY_DELETE, "句点键被映射成了删除键");
    assert_eq!(period, '.' as u32);
    // 直接给 KEY_DELETE 也一样能删（本轨内部键码的自洽）
    assert_eq!(s.input_key(KEY_DELETE, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "");
}

// 不变式：编码后的特殊键码永远落不进 ASCII 区 —— 将来新增特殊键忘了加
// match 臂时，最多掉进 `_ => Unhandled` 放行给应用，不会静默变成垃圾字符。
// 控制符也算 ASCII 区：0x08/0x09/0x0D/0x1B 在旧约定下漏匹配会让英文模式空缓冲
// `Commit` 一个控制符进文档（不止是吞掉一个键），见 logic.rs 的 SPECIAL_BASE 注释。
#[test]
fn special_key_space_stays_out_of_ascii() {
    assert!(
        SPECIAL_BASE.is_power_of_two(),
        "SPECIAL_BASE 必须是 2 的幂，special_vk 靠掩码取 VK"
    );
    let leaked: Vec<char> = SPECIAL_KEYS
        .into_iter()
        .filter_map(|k| char::from_u32(k).filter(char::is_ascii))
        .collect();
    assert!(
        leaked.is_empty(),
        "特殊键码落在 ASCII 区 {leaked:?}：将来漏了 match 臂会静默变成字符"
    );
    for k in SPECIAL_KEYS {
        assert!(special_vk(k) <= 0xFF, "特殊键 {k:#x} 的 VK 段越界");
    }
}

// 设计推理的直接验证：将来新增特殊键但漏了 match 臂时，键码会掉进可打印分支 ——
// 基址必须让这条路退化成"放行"，而不是凭空提交一个字符。
#[test]
fn unmatched_special_keycode_degrades_to_passthrough_not_garbage() {
    use crate::logic::{KeyOutcome, TsfLogic};
    use engine_core::composer::Mode;
    let mut s = TsfLogic::load(None).expect("内置回退词库");
    s.switch_mode(Mode::English); // 英文空缓冲的可打印分支会 Commit(c)：最能暴露"变成字符"
    // 对照：裸 VK 0x5B（'[' 键）走可打印分支被提交 —— 旧约定的危险就在这。
    // **断言只到「提交了某个字符」，不钉字形**：提交成 '[' 还是 '［' 归标点表层管
    // （中文模式 '[' 映射成 '［'，英文模式半角直传），把字形钉在这里 = 把别人的
    // 表钉进本模块，表一变就红，且红的地方与真实缺陷无关。
    assert!(matches!(s.input_key(0x5b, 0), KeyOutcome::Commit(_)));
    // 编码后同一个键（模拟"新增特殊键漏了 match 臂"）：char::from_u32 得到
    // 非 ASCII 补充平面字符 → 被 c.is_ascii() 挡回 _ => Unhandled，不提交不改缓冲
    assert_eq!(s.input_key(SPECIAL_BASE | 0x5b, 0), KeyOutcome::Unhandled);
    assert_eq!(s.buffer(), "");
}

// ---------- 模式热键（B0/B5） ----------

#[test]
fn hotkey_requires_ctrl_and_ignores_keyup() {
    // 无 Ctrl：OEM 键是普通字符（' 是拼音分隔符、\ 是可见 ASCII），不是热键
    assert_eq!(mode_hotkey(VK_OEM_7, 0), None);
    assert_eq!(mode_hotkey(VK_OEM_5, 0), None);
    // 只有 Alt/Shift 也不算
    assert_eq!(mode_hotkey(VK_OEM_7, crate::logic::KEY_STATE_ALT), None);
    // Ctrl + 按下：两个热键
    assert_eq!(
        mode_hotkey(VK_OEM_7, KEY_STATE_CTRL),
        Some(ModeHotkey::ToggleEnglish)
    );
    assert_eq!(
        mode_hotkey(VK_OEM_5, KEY_STATE_CTRL),
        Some(ModeHotkey::ToggleSymbol)
    );
    // Ctrl + **抬起**：不返回 —— 否则按下切过去、抬起切回来，等于没切
    assert_eq!(
        mode_hotkey(VK_OEM_7, KEY_STATE_CTRL | KEY_STATE_RELEASED),
        None
    );
    assert_eq!(
        mode_hotkey(VK_OEM_5, KEY_STATE_CTRL | KEY_STATE_RELEASED),
        None
    );
}

#[test]
fn shift_space_toggles_fullwidth_and_bare_space_does_not() {
    // 正例
    assert!(fullwidth_hotkey(VK_SPACE, KEY_STATE_SHIFT));
    // **裸空格必须不拦**：它是选首候选/提交缓冲的键，拦了整个输入法就废了。
    // 这条是本次改动里唯一会造成"打不了字"的方向，故单列。
    assert!(!fullwidth_hotkey(VK_SPACE, 0));
    // Ctrl+Space 是输入法切换键（fcitx5 与 Windows 都有）、Alt+Space 是系统菜单
    assert!(!fullwidth_hotkey(
        VK_SPACE,
        KEY_STATE_SHIFT | KEY_STATE_CTRL
    ));
    assert!(!fullwidth_hotkey(VK_SPACE, KEY_STATE_SHIFT | KEY_STATE_ALT));
    // 抬起不再切（同 mode_hotkey：按下切过去、抬起切回来 = 纹丝不动）
    assert!(!fullwidth_hotkey(
        VK_SPACE,
        KEY_STATE_SHIFT | KEY_STATE_RELEASED
    ));
    // Shift+别的键不算（Shift+a 是大写 A，Shift+Tab 是反向 Tab）
    for vk in [0x41u32, 0x09, 0xba, 0xc0] {
        assert!(
            !fullwidth_hotkey(vk, KEY_STATE_SHIFT),
            "VK {vk:#x} 不得是热键"
        );
    }
    // 键盘上真实存在的空格 VK 就是 0x20，别写成别的（写错 → 键静默失灵）
    assert_eq!(VK_SPACE, 0x20);
}

#[test]
fn hotkey_repeat_is_claimed_but_does_not_act() {
    // 按住不放：系统补发的重复事件仍**是**热键（要认领，别漏给应用/引擎）……
    assert_eq!(
        mode_hotkey(VK_OEM_7, KEY_STATE_CTRL | KEY_STATE_REPEAT),
        Some(ModeHotkey::ToggleEnglish)
    );
    assert!(fullwidth_hotkey(
        VK_SPACE,
        KEY_STATE_SHIFT | KEY_STATE_REPEAT
    ));
    // ……但**不许动作**：热键是切换语义，重复再切一次就是按住期间疯狂来回切。
    assert!(!hotkey_should_act(KEY_STATE_CTRL | KEY_STATE_REPEAT));
    assert!(!hotkey_should_act(KEY_STATE_SHIFT | KEY_STATE_REPEAT));
    // 首次按下才是动作的那一次
    assert!(hotkey_should_act(KEY_STATE_CTRL));
    assert!(hotkey_should_act(KEY_STATE_SHIFT));
    // 抬起也不动作（与 mode_hotkey/fullwidth_hotkey 里的 RELEASED 判定同向）
    assert!(!hotkey_should_act(KEY_STATE_SHIFT | KEY_STATE_RELEASED));
    // 干净的一次按下（前提是键位已由 mode_hotkey/fullwidth_hotkey 匹配上）
    assert!(hotkey_should_act(0));
    // 认领 + 不动作这条组合必须**两轨同判**：fcitx5 轨在
    // `handleModeHotkey`/`handleFullwidthHotkey` 里用 rawKey() 判同一个位
    // （key() 归一化后 Repeat 已被滤掉，见那里的注释）。
}

#[test]
fn hotkey_is_none_for_everything_else() {
    // Ctrl+字母/数字/空格一个都不能吃：那是应用的快捷键（Ctrl+C/V/A…），
    // 顺带确认没把 VK_OEM_1(0xBA)/VK_OEM_3(0xC0) 这些邻居算进来。
    // 0xE2 = VK_OEM_102（欧洲键盘那个多的键）**不是**撇号键 —— 它在列表里，
    // 0xDE（= VK_OEM_7，撇号键本体）不在：原来两处写反了，见 VK_OEM_7 的注释。
    for vk in [
        0x41u32, 0x43, 0x56, 0x31, 0x20, 0xba, 0xc0, 0xdb, 0xdd, 0xe2,
    ] {
        assert_eq!(
            mode_hotkey(vk, KEY_STATE_CTRL),
            None,
            "VK {vk:#x} 不得是热键"
        );
    }
}

/// 两个 VK 字面量的**值**。与上面的 const 断言分工不同，别删了上面那条：
/// - 这条在主机上跑，钉的是「本模块写下的数值」—— 挡住以后有人改错其中一个
///   而另一个没跟（比如 `VK_SPACE` 那条单测的同类）；
/// - 它**挡不住**「两处一起写错」（本文件原来那条 `assert_eq!(VK_OEM_5 as u32,
///   0xdc)` 就是这么绿的，而 0xE2 的错值一直在），真正的外部基准是 windows
///   crate 的同名常量，只有 `#[cfg(target_os = "windows")]` 的 const 断言够得着。
#[test]
fn oem_vks_are_the_win32_abi_values() {
    assert_eq!((VK_OEM_5, VK_OEM_7), (0xdc, 0xde));
}

#[test]
fn hotkey_toggles_in_and_out() {
    // 「能进能出」：同一个键按两次必须回到原模式。只进不出是 B0 的原缺陷形态。
    for (hot, want, other) in [
        (ModeHotkey::ToggleEnglish, Mode::English, Mode::Symbol),
        (ModeHotkey::ToggleSymbol, Mode::Symbol, Mode::English),
    ] {
        let in_once = hotkey_target(hot, Mode::Pinyin);
        assert_eq!(in_once, want, "{hot:?} 从拼音按下应进 {want:?}");
        assert_eq!(
            hotkey_target(hot, in_once),
            Mode::Pinyin,
            "{hot:?} 再按一次必须回拼音"
        );
        // 从**别的非拼音模式**按：也进目标模式（不是"只从拼音出发"）
        assert_eq!(hotkey_target(hot, other), want, "{hot:?} 从 {other:?} 按下");
    }
}

// raw VK → 键码的查表是检测与转发的唯一入口（不再有第二份 VK 表）。
#[test]
fn glue_maps_raw_vk_to_special_keycode() {
    assert_eq!(special_keycode(0x2e), Some(KEY_DELETE)); // VK_DELETE
    assert_eq!(special_keycode(0x21), Some(KEY_PAGE_UP)); // VK_PRIOR
    assert_eq!(special_keycode(0x22), Some(KEY_PAGE_DOWN)); // VK_NEXT
    assert_eq!(special_keycode(0x20), Some(KEY_SPACE)); // VK_SPACE
    assert_eq!(special_keycode(0x10), Some(KEY_SHIFT)); // VK_SHIFT
    // 可打印键不是特殊键：VK_OEM_PERIOD=0xBE → '.'，VK_1=0x31 → '!'/'1'
    assert_eq!(special_keycode(0xbe), None);
    assert_eq!(special_keycode(0x31), None);
}
