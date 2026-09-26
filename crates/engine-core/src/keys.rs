// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键码表：平台中立键路由（[`crate::router`]）的输入编码。
//!
//! 独立成模块的理由与 TSF 轨的 `vk.rs` 同源：**键码映射是两端唯一的差异**，
//! 把它单独放一处，路由逻辑（router.rs）就与平台无关；两轨用的是各自的
//! keysym / VK，本模块不复制任何一端的平台常量，只保留中立的那套。
//!
//! # 键码约定
//!
//! - **可打印字符 = Unicode 码点**：空格 = `0x20`，'a' = 0x61，'A' = 0x41（大小写由
//!   码点本身携带；物理 ⇧ 已被平台应用，与两轨的键值来源同义）。
//! - **特殊键 = [`SPECIAL_BASE`] | 低 16 位码**（取值见下方常量，与 TSF 轨的
//!   `SPECIAL_BASE` 同一约定）。
//! - **键状态位**沿用两轨的位约定（见下方 `KEY_STATE_*`）。
//!
//! # Apple 侧的键位来源（胶水侧映射，本机无法编译验证，故只写规则不写魔数）
//!
//! 键码由 Swift 胶水按 `NSEvent.keyCode`（`kVK_*`，见 Events.h）或
//! `UIKey.keyCode`（HID usage）映射到本表常量；**不要按 `characters` 猜特殊键** ——
//! 退格在不同来源下可能是 BS(0x08) 也可能是 DEL(0x7F)，猜错的后果是该键在
//! 输入态下静默失效（退化成交系统）。可打印字符则直接取 `characters` 的 Unicode 码点。

// ---------- 特殊键码：SPECIAL_BASE | 低 16 位 ----------

/// 特殊键键码基址。特殊键空间与「可打印字符 = Unicode 码点」**必须不相交**。
///
/// 取 `0x1_0000` 的理由与 TSF 轨同源，那边是被真 bug 逼出来的：裸用平台键码会与
/// ASCII 撞号（VK_NEXT=0x22=`"`、VK_DELETE=0x2E=`.`），后果不是「多认了一个键」而是
/// **普通字符被吃掉** —— 拼音缓冲非空时敲 `.` 会走退格分支删掉拼音字母。
///
/// 取补充平面后，编码键码即便将来漏了 match 臂、掉进可打印分支，`char::from_u32`
/// 得到的也是**非 ASCII** 的补充平面字符，会被 `is_ascii()` 挡回交系统，
/// **不会静默变成垃圾字符**。不变式由 `special_key_space_stays_out_of_ascii` 钉住。
pub const SPECIAL_BASE: u32 = 0x1_0000;

// 低 16 位取值的两条规则：
// 1) 四大控制键取 ASCII 控制码 —— 这是**通用**约定（不是某家平台常量）：
//    Windows VK_BACK/TAB/RETURN/ESCAPE、fcitx5 keysym 0xffxx 的低字节、
//    Apple 的 BS/HT/CR/ESC 都落在同一段，将来收敛时零映射成本。
// 2) 无通用字符码的键（翻页/删除/⇧/方向键）取中立枚举 0x80 起 —— 不抄 TSF 的 VK、
//    也不抄 fcitx5 的 keysym。低 16 位取值本身不影响安全（SPECIAL_BASE 已保证整码
//    非 ASCII），避开 ASCII 段只是为了让常量表读起来不会被误认成字符码。

/// 退格（ASCII BS）。
pub const KEY_BACK_SPACE: u32 = SPECIAL_BASE | 0x08;
/// Tab（ASCII HT）。
pub const KEY_TAB: u32 = SPECIAL_BASE | 0x09;
/// 回车（ASCII CR）。
pub const KEY_RETURN: u32 = SPECIAL_BASE | 0x0D;
/// Esc（ASCII ESC）。
pub const KEY_ESCAPE: u32 = SPECIAL_BASE | 0x1B;
/// 上一页（候选翻页，对应 PageUp）。
pub const KEY_PAGE_UP: u32 = SPECIAL_BASE | 0x80;
/// 下一页（候选翻页，对应 PageDown）。
pub const KEY_PAGE_DOWN: u32 = SPECIAL_BASE | 0x81;
/// 向后删除（Delete / forward delete）。
pub const KEY_DELETE: u32 = SPECIAL_BASE | 0x82;
/// ⇧（左右 Shift 同一码；两轨的 SHIFT_L/SHIFT_R 二码在平台侧归一）。
pub const KEY_SHIFT: u32 = SPECIAL_BASE | 0x83;
/// ↑↓←→：本层不消费（见 `KeyRouter::key_event` 的直通分支），键码在此备胶水侧使用。
pub const KEY_UP: u32 = SPECIAL_BASE | 0x84;
/// ↓。
pub const KEY_DOWN: u32 = SPECIAL_BASE | 0x85;
/// ←。
pub const KEY_LEFT: u32 = SPECIAL_BASE | 0x86;
/// →。
pub const KEY_RIGHT: u32 = SPECIAL_BASE | 0x87;

/// 空格。**可打印段**（Unicode 码点 0x20），不是特殊键 —— 与 TSF 轨的
/// `SPECIAL_BASE | 0x20` 编码不同：本模块以「可打印 = 码点」为准，TSF 那种编码
/// 在本层会退化成交系统（非 ASCII），不会变成垃圾字符。
pub const KEY_SPACE: u32 = 0x20;

// ---------- 键状态位（与两轨、fcitx5 5.1.x fcitx::KeyState 的位约定一致） ----------

/// 物理 Shift 被按住（键值本身已含大小写时此位仅作参考）。
pub const KEY_STATE_SHIFT: u32 = 1 << 0;
/// CapsLock 锁定。位布局的一部分；路由不用它（两轨同样只定义不用 —— 大小写由
/// 平台给出的码点与 ⇧ 状态机决定）。
pub const KEY_STATE_CAPS_LOCK: u32 = 1 << 1;
/// 物理 Ctrl 被按住。
pub const KEY_STATE_CTRL: u32 = 1 << 2;
/// 物理 Alt（macOS 的 Option）被按住。
pub const KEY_STATE_ALT: u32 = 1 << 3;
/// 物理 Meta（macOS 的 Command）被按住。
///
/// 两轨的 Rust 侧没有这一位（它们的平台不产生），但 Apple 侧 Command 组合键
/// （⌘A/⌘C/⌘V）与 Ctrl/Alt 同类 —— 系统快捷键，必须直通，否则 ⌘A 会把 'a'
/// 吃进拼音缓冲。故本层把 META 并入直通掩码；这是相对两轨**唯一**的一处语义扩展。
pub const KEY_STATE_META: u32 = 1 << 4;
/// 键释放事件。
pub const KEY_STATE_RELEASED: u32 = 1 << 26;
/// 键重复事件。
pub const KEY_STATE_REPEAT: u32 = 1 << 27;
/// 长按事件（⇧ 长按 = Lock）。
pub const KEY_STATE_LONG_PRESSED: u32 = 1 << 28;

#[cfg(test)]
mod tests {
    use super::*;

    /// 全部特殊键码（不含空格 —— 空格在**可打印**段，见下方断言）。
    const SPECIAL_KEYS: [u32; 12] = [
        KEY_BACK_SPACE,
        KEY_TAB,
        KEY_RETURN,
        KEY_ESCAPE,
        KEY_PAGE_UP,
        KEY_PAGE_DOWN,
        KEY_DELETE,
        KEY_SHIFT,
        KEY_UP,
        KEY_DOWN,
        KEY_LEFT,
        KEY_RIGHT,
    ];

    /// 两轨的成因见 `SPECIAL_BASE` 注释（TSF 轨曾被 VK 与 ASCII 撞号咬过：
    /// VK_NEXT=0x22=`"`、VK_DELETE=0x2E=`.` → 打 `.` 变退格）。
    #[test]
    fn special_key_space_stays_out_of_ascii() {
        for k in SPECIAL_KEYS {
            assert!(
                k >= SPECIAL_BASE,
                "特殊键码必须带基址（{k:#x}）——漏了基址就会与 ASCII 撞号"
            );
            let c = char::from_u32(k).expect("特殊键码可解码");
            assert!(
                !c.is_ascii(),
                "特殊键码 {k:#x} 落进了 ASCII 段：将来漏了 match 臂会被当成可打印字符吃进缓冲"
            );
        }
        // 空格在可打印段（Unicode 码点 0x20）——与 TSF 轨的 SPECIAL_BASE|0x20 编码不同。
        assert_eq!(KEY_SPACE, 0x20);
        assert!(char::from_u32(KEY_SPACE).expect("空格可解码").is_ascii());
        assert!(!SPECIAL_KEYS.contains(&KEY_SPACE));
    }
}
