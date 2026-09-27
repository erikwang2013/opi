// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Win32 虚拟键码（VK）→ 引擎键码的映射判定。
//!
//! 特殊键在这里换成 `logic` 的编码键码（`special_keycode`），可打印键换成
//! Unicode 码点，两者不相交；映射的常量表只有 `logic::SPECIAL_KEYS` 一份。
//!
//! 独立成模块（而非留在 `tsf.rs`）的唯一原因：`tsf.rs` 在
//! `#[cfg(target_os = "windows")]` 下，Linux 主机不编译它，判定写在那边就等于
//! 永远没有回归测试（本项目在 Linux 上开发/跑门禁）。故把**纯判定**放这里
//! （无 windows 类型，主机可编译可单测），Win32 调用留在 `tsf.rs` 的
//! `to_engine_keycode`。键码约定见 `logic` 模块头注释。

use engine_core::composer::Mode;

use crate::logic::{
    KEY_BACK_SPACE, KEY_DELETE, KEY_ESCAPE, KEY_PAGE_DOWN, KEY_PAGE_UP, KEY_RETURN, KEY_SHIFT,
    KEY_SPACE, KEY_STATE_ALT, KEY_STATE_CTRL, KEY_STATE_RELEASED, KEY_STATE_REPEAT,
    KEY_STATE_SHIFT, KEY_TAB, SPECIAL_BASE,
};

/// 特殊键表（唯一真源，放在本模块）：raw VK → 键码的查表与「键码与 ASCII 不相交」
/// 的不变式检查共用这一份。键码字面值只在 `logic` 的常量定义处出现一次，
/// 这里只列名字，不重复 VK 值（VK 由 `special_vk` 掩码取出）。
pub const SPECIAL_KEYS: [u32; 9] = [
    KEY_BACK_SPACE,
    KEY_TAB,
    KEY_RETURN,
    KEY_SHIFT,
    KEY_ESCAPE,
    KEY_SPACE,
    KEY_PAGE_UP,
    KEY_PAGE_DOWN,
    KEY_DELETE,
];

/// 无法从 VK 得到字符时的哨兵键码：`char::from_u32` 为 None 且不等于任何
/// `KEY_*` → `input_key` 必然返回 Unhandled，键自然流入应用（不接管）。
///
/// **绝不可回退成 VK 本身**：VK 空间与 ASCII 重叠，导航键/小键盘会变成可打印
/// 字符被吃进缓冲并吞掉该键 —— VK_RIGHT=0x27=`'`、NumLock 关时 VK_NUMPAD1..9
/// =0x61..0x69=`a`..`i`、VK_NUMPAD0=0x60=`\``。
/// `ToUnicodeEx` 对这些键本来就返回 0（无映射），必然走回退分支。
pub const NO_KEYVAL: u32 = u32::MAX;

/// 特殊键码低 16 位的 VK 原值（`logic::SPECIAL_BASE | vk` 的解码）。
/// 靠掩码取 VK：VK 字面值只写在 `logic` 的常量定义处，这里不重复一份。
pub const fn special_vk(key: u32) -> u32 {
    key & !SPECIAL_BASE
}

/// raw VK → `logic` 的特殊键码；非特殊键 → None（交给 `ToUnicodeEx` 那条路）。
///
/// **检测与转发是同一次查表的结果**：返回的就是要透传给 `input_key` 的键码，
/// 调用方不再自己判断"是不是特殊键"而转发另一个值 —— 这个 bug 的成因正是
/// 判定用 VK、转发也用 VK，两份含义共用同一段数值空间。
pub fn special_keycode(vk: u32) -> Option<u32> {
    SPECIAL_KEYS.into_iter().find(|&k| special_vk(k) == vk)
}

/// VK + `ToUnicodeEx` 的返回 → 引擎键码。`n` 为该 API 的返回值
/// （`<= 0` = 无映射/死键/取不到键盘状态），`first` 为转换出的首个 UTF-16 单元。
pub fn vk_to_engine_keycode(vk: u32, n: i32, first: u16) -> u32 {
    // 特殊键必须走编码后的键码、不可用映射结果：例如 VK_BACK=0x08 经
    // `ToUnicodeEx` 得到的退格控制符 0x08 仍然 `is_ascii()`，会被送到
    // `handle_printable` 当普通符号放行 —— 退格就废了。
    if let Some(keycode) = special_keycode(vk) {
        keycode
    } else if n >= 1 {
        first as u32
    } else {
        // 无映射（导航键/小键盘/取不到键盘状态）或死键：交给应用，别拿 VK 冒充字符
        NO_KEYVAL
    }
}

// ---------- 模式热键（B0/B5） ----------

/// US 布局的反斜杠键（`VK_OEM_5`）。值是 Win32 ABI 事实（winuser.h），冻结的。
const VK_OEM_5: u32 = 0xdc;
/// US 布局的撇号键（`VK_OEM_7`）。同上。
/// **曾错写成 `0xe2`（那是 `VK_OEM_102`）**：后果是 `Ctrl+'` 在 Windows 上按了
/// 没反应 —— 真键盘发的是 `0xde`，`mode_hotkey` 匹配不上就直落引擎，没有任何报错。
/// 那条错值能一直活着，是因为原来「与 crate 对齐」的单测两边都是本文件的字面量
/// （断言与被断言同源）—— 现在改由下面的 const 断言对着 crate 走，见那里。
const VK_OEM_7: u32 = 0xde;

// 与 windows crate 的**同名常量**对齐 —— 由**编译器**把关（`cargo check
// --target x86_64-pc-windows-msvc`），不是单测：那个 crate 是 target 作用域依赖
// （见 Cargo.toml），Linux 主机上根本不存在，写进 `#[cfg(test)]` 只能自己跟自己比
// —— 本模块原来就有一条这样的假闸（`assert_eq!(VK_OEM_5 as u32, 0xdc)`，两边同源），
// 已删。放这里而不是 `tsf.rs`：常量就在上面几行，改动时没法不看见它。
//
// 只在 Windows 目标存在，故整个块 cfg 掉，本模块在主机侧仍是纯 Rust（模块头那条）。
#[cfg(target_os = "windows")]
const _: () = {
    assert!(VK_OEM_5 == windows::Win32::UI::Input::KeyboardAndMouse::VK_OEM_5.0 as u32);
    assert!(VK_OEM_7 == windows::Win32::UI::Input::KeyboardAndMouse::VK_OEM_7.0 as u32);
};

/// 模式热键的语义。**不是目标模式**：切到哪儿取决于当前模式（见 `hotkey_target`），
/// 因为两个键都是「来回切」——只能进不能出等于换个地方卡住。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeHotkey {
    /// Ctrl+' → 英文 ⇄ 拼音（B0）。
    ToggleEnglish,
    /// Ctrl+\ → 符号 ⇄ 拼音（B5）。
    ToggleSymbol,
}

/// 热键此刻是否应当**动作**。`false` = 仍是热键（**要认领**），但这一次按键不改任何状态。
///
/// **按住不放的重复事件必须排除**：系统对按住的键持续补发重复（TSF 是 `lParam`
/// bit30 → `map_key_state` 的 `KEY_STATE_REPEAT`）。热键是**切换**语义，重复再切一次
/// 就是「按住 Ctrl+' 模式疯狂来回切」「按住 Shift+Space 全角位疯狂翻」。
///
/// **为什么是「认领但不动作」，不是「不是热键、放行」**：放行的重复事件会接着走到
/// 引擎 —— `logic_input_method.rs` 的 `KEY_SPACE` 分支**只看 `RELEASED`、不看 Shift 位**
/// （⇒ 被当成「提交缓冲 / 选首候选」，见 `tsf.rs` 里 `fullwidth_hotkey` 调用点上方那条
/// 注释：按下与重复同罪），`KEY_STATE_REPEAT` 整个引擎只有 `handle_shift` 认。
/// 与 `tsf.rs` 对**抬起**的处置同构（抬起也走引擎，靠 `handle_*` 里的 `released` 分支
/// 变成 Consumed；重复没有这一层保护，只能在热键这一层拦）。
///
/// 与 fcitx5 轨 `handleFullwidthHotkey` / `handleModeHotkey` 的 `isRepeatEvent` 同判，
/// 两轨必须一致。
pub fn hotkey_should_act(states: u32) -> bool {
    states & (KEY_STATE_REPEAT | KEY_STATE_RELEASED) == 0
}

/// VK + 修饰位 → 模式热键。`None` = 不是热键，照常走引擎路由。
/// **注意**：重复事件也返回 `Some`（键位已确认是我们的，要认领）——改状态前先过
/// `hotkey_should_act`，否则按住不放就会来回切。
///
/// 【本机（Linux）无法验证的部分，别当成已验】Windows 生态里这两个键位有没有
/// 被别的软件占用，本机核实不了 —— 没有 Windows，也没有那些软件。这里唯一的依据
/// 是「不与 Windows 自身的系统快捷键相撞」这一条常识判断，**没有实测**。
///
/// Linux 轨的 Ctrl+' / Ctrl+\ 是 `strings` + harness **实测**选出来的（见
/// `crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 的 handleModeHotkey：计划原本建议的
/// Ctrl+; 被 libclipboard 抢走，是实测推翻的）。两条轨键位相同是**约定**，
/// 不是同一份证据 —— 别把 Linux 侧那份实测当成这条的背书。
///
/// **为什么不改成可打印键**（2026-09-27 评估过，别再提）：裸键在中文模式下**已被
/// 标点层认领** —— `engine-core/src/punctuation.rs` 的 `CHINESE_PUNCT` 明写
/// `\`→`、`（注释：「主流 IME 都挂在反斜杠上」），`` ` ``→`｀` 走 `ascii_fullwidth`
/// 的 `is_ascii_punctuation()` 兜底，而**中文模式默认全角**（`Mode::default_fullwidth`）。
/// 裸 `'` 同样不行：`Engine::punct_text` 在缓冲为空时把它当引号。
/// ⇒ 拿这些键当模式触发＝与用户裁决的标点功能正面相撞（一个键不能既出 `、` 又切模式）。
/// Ctrl 组合键反而空着，且**两轨都在引擎之前截获**（本模块的 `mode_hotkey` 判在
/// `tsf.rs` 调 `to_engine_keycode` **之前**，用的是裸 VK + `GetKeyState`）——
/// 「Ctrl 到不了路由器」是对的，但热键本来就不走路由器。
///
/// VK_OEM_5 = 0xDC（US 布局反斜杠）、VK_OEM_7 = 0xDE（US 布局撇号）。此处用裸
/// 字面量是因为本模块**平台中立**（无 windows 类型 → 主机可编译可单测）；
/// 与 windows crate 同名常量的对齐由上面的 const 断言在 Windows 目标上把关
/// （**不是单测**：那个 crate 是 target 作用域依赖，主机侧根本没有它）。
///
/// 抬起（`KEY_STATE_RELEASED`）不返回热键：按下已经切过一次，抬起再切会切回去，
/// 用户看到的是模式纹丝不动。与 fcitx5 轨同判。
pub fn mode_hotkey(vk: u32, states: u32) -> Option<ModeHotkey> {
    if states & KEY_STATE_CTRL == 0 || states & KEY_STATE_RELEASED != 0 {
        return None;
    }
    match vk {
        VK_OEM_7 => Some(ModeHotkey::ToggleEnglish),
        VK_OEM_5 => Some(ModeHotkey::ToggleSymbol),
        _ => None,
    }
}

/// 当前模式 + 热键 → 目标模式。**来回切**：已在目标模式就回拼音。
///
/// 与 `opi_fcitx5.cpp` 的 `handleModeHotkey` 是同一个判定，两轨必须一致
/// （改一处就得改另一处）；这里是纯函数，主机可单测。
pub fn hotkey_target(hot: ModeHotkey, cur: Mode) -> Mode {
    match hot {
        ModeHotkey::ToggleEnglish if cur == Mode::English => Mode::Pinyin,
        ModeHotkey::ToggleSymbol if cur == Mode::Symbol => Mode::Pinyin,
        ModeHotkey::ToggleEnglish => Mode::English,
        ModeHotkey::ToggleSymbol => Mode::Symbol,
    }
}

// ---------- 全角 ⇄ 半角切换键（用户裁决 2026-09-27「除自动全角外，再加一个切换键」） ----------

/// `VK_SPACE`。与 `logic::KEY_SPACE`（0x20）**同值但不同空间**：那个是引擎键码，
/// 这个是 Win32 虚拟键码，别互相替换（同 `VK_OEM_5` 与 `FcitxKey_backslash` 的关系）。
const VK_SPACE: u32 = 0x20;

/// Shift+Space = 全角 ⇄ 半角开关。
///
/// **与 `mode_hotkey` 分开而不是并进那个枚举**：那两个键选的是**目标模式**，
/// 这个选的是**同一个模式内的一个布尔开关**；并进去会让 `hotkey_target` 多出一个
/// 没有目标模式的假分支（`ModeHotkey::ToggleFullwidth => ?`）。
///
/// **为什么是 Shift+Space**：CJK 生态里 Shift+Space 就是「全角空格」的通用约定，
/// 这里借它当**开关**（不是打全角空格）。fcitx5 轨实测无占用 —— 全部 fcitx5 库的
/// `^(Shift|Control|Super|Alt)\+` 默认组合里没有它，addon conf 里也搜不到，见
/// `opi_fcitx5.cpp` 的 `handleFullwidthHotkey` 占用表。
///
/// **带 Ctrl/Alt 的不算**：Ctrl+Space 是 fcitx5 的输入法切换键、也是 Windows 的
/// 输入法切换键，收窄条件避免与它撞；Alt+Space 是 Windows 的系统菜单键。
///
/// 【本机（Linux）无法验证】Windows 侧同键（`VK_SPACE` + Shift）会不会被别的软件
/// 先占、TSF 会不会先交给应用 —— **未验证**，与 `mode_hotkey` 那条是同一个未知。
///
/// 重复事件也返回 `true`（要认领）—— 改状态前先过 `hotkey_should_act`。
pub fn fullwidth_hotkey(vk: u32, states: u32) -> bool {
    if states & KEY_STATE_SHIFT == 0
        || states & (KEY_STATE_CTRL | KEY_STATE_ALT) != 0
        || states & KEY_STATE_RELEASED != 0
    {
        return false;
    }
    vk == VK_SPACE
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 input_method_tests.rs 同惯例。
#[cfg(test)]
#[path = "vk_tests.rs"]
mod tests;
