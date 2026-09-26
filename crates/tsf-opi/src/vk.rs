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

use crate::logic::{
    KEY_BACK_SPACE, KEY_DELETE, KEY_ESCAPE, KEY_PAGE_DOWN, KEY_PAGE_UP, KEY_RETURN, KEY_SHIFT,
    KEY_SPACE, KEY_TAB, SPECIAL_BASE,
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

#[cfg(test)]
mod tests {
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
        use crate::logic::{KeyOutcome, TsfLogic};
        let mut hijacked = Vec::new();
        for c in ['.', '!', '"'] {
            let mut s = TsfLogic::load(None).expect("内置回退词库");
            s.switch_mode(engine_core::composer::Mode::Pinyin);
            for k in ['n', 'i'] {
                s.input_key(k as u32, 0);
            }
            if s.input_key(c as u32, 0) != KeyOutcome::Unhandled || s.buffer() != "ni" {
                hijacked.push(c);
            }
        }
        assert!(
            hijacked.is_empty(),
            "可打印字符 {hijacked:?} 被特殊键抢走码位：交不了应用 / 还会改缓冲"
        );
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
        // 句点键：VK_OEM_PERIOD=0xBE，ToUnicodeEx 给 '.' → 可打印字符，不删
        assert_eq!(
            s.input_key(vk_to_engine_keycode(0xbe, 1, '.' as u16), 0),
            KeyOutcome::Unhandled
        );
        assert_eq!(s.buffer(), "n");
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
        // 对照：裸 VK 0x5B（'[' 键）走可打印分支被提交 —— 旧约定的危险就在这
        assert_eq!(s.input_key(0x5b, 0), KeyOutcome::Commit("[".into()));
        // 编码后同一个键（模拟"新增特殊键漏了 match 臂"）：char::from_u32 得到
        // 非 ASCII 补充平面字符 → 被 c.is_ascii() 挡回 _ => Unhandled，不提交不改缓冲
        assert_eq!(s.input_key(SPECIAL_BASE | 0x5b, 0), KeyOutcome::Unhandled);
        assert_eq!(s.buffer(), "");
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
}
