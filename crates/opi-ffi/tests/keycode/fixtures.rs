// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 第五道门禁的**自检 fixture**：故意改坏的值 + 迷你真源。
//!
//! 这些字符串**不参与活体对账**（活体在 `keycode_contract.rs`），它们的唯一用途是
//! 证明「解析器 + 对账器」在值错时**真的会红**、在写法不同但值相同时**真的不红**。
//! 用同一个 `parse_*` / `reconcile` 跑，走的就是活体那条代码路径 —— 这是这些 fixture
//! 算证据、而不算自说自话的原因。用例在 `keycode_fixtures.rs`。

// ---------- 故意改坏的 fixture（证明这套解析/对账真的会红）----------

/// 迷你真源：形状与 `keys.rs` 相同，只有 6 条 —— fixture 不该被真实条数绑住。
pub const FIX_RUST: &str = r#"
pub const SPECIAL_BASE: u32 = 0x1_0000;
pub const KEY_SPACE: u32 = 0x20;
pub const KEY_BACK_SPACE: u32 = SPECIAL_BASE | 0x08;
pub const KEY_SHIFT: u32 = SPECIAL_BASE | 0x83;
pub const KEY_STATE_SHIFT: u32 = 1 << 0;
pub const KEY_STATE_REPEAT: u32 = 1 << 27;
"#;

pub const FIX_C_OK: &str = r#"
#define OPI_KEY_SPECIAL_BASE 0x10000u
#define OPI_KEY_SPACE 0x20u
#define OPI_KEY_BACK_SPACE (OPI_KEY_SPECIAL_BASE | 0x08u)
#define OPI_KEY_SHIFT (OPI_KEY_SPECIAL_BASE | 0x83u)
#define OPI_KEY_STATE_SHIFT (1u << 0)
#define OPI_KEY_STATE_REPEAT (1u << 27)
"#;

/// 只改**低字节**：`| 0x83u` → `| 0x38u`。位数没动、形状没动，只有值错。
pub const FIX_C_WRONG_LOW: &str = r#"
#define OPI_KEY_SPECIAL_BASE 0x10000u
#define OPI_KEY_SPACE 0x20u
#define OPI_KEY_BACK_SPACE (OPI_KEY_SPECIAL_BASE | 0x08u)
#define OPI_KEY_SHIFT (OPI_KEY_SPECIAL_BASE | 0x38u)
#define OPI_KEY_STATE_SHIFT (1u << 0)
#define OPI_KEY_STATE_REPEAT (1u << 27)
"#;

/// ETS 侧：`1 << 27` → `1 << 25`（位值错一格，形状完全没变）。
pub const FIX_ETS_WRONG_SHIFT: &str = r#"
const SPECIAL_BASE: number = 0x10000;
export const KEY_SPACE: number = 0x20;
export const KEY_BACK_SPACE: number = SPECIAL_BASE | 0x08;
export const KEY_SHIFT: number = SPECIAL_BASE | 0x83;
export const STATE_SHIFT: number = 1 << 0;
export const STATE_REPEAT: number = 1 << 25;
"#;

/// Swift 侧：`1 << 26` 冒充 `1 << 27` —— 正是 iOS `stateRepeat` 那颗雷的形状。
pub const FIX_SWIFT_WRONG_BIT: &str = r#"
enum OpiKey {
    static let specialBase: UInt32 = 0x1_0000
    static let backspace: UInt32 = specialBase | 0x08
    static let shift: UInt32 = specialBase | 0x83
    static let space: UInt32 = 0x20
    static let stateShift: UInt32 = 1 << 0
    static let stateRepeat: UInt32 = 1 << 26
}
"#;

/// 同一个值写成四种样子：**必须绿**（按字面比会假红）。
pub const FIX_SWIFT_EQUAL_OTHER_FORM: &str = r#"
enum OpiKey {
    static let specialBase: UInt32 = 65536
    static let backspace = 0x10008
    static let shift: UInt32 = 65667
    static let space: UInt32 = 32
    static let stateShift: UInt32 = 0x1
    static let stateRepeat: UInt32 = 134217728
}
"#;

pub const FIX_SWIFT_MAP: &[(&str, &str)] = &[
    ("specialBase", "SPECIAL_BASE"),
    ("backspace", "KEY_BACK_SPACE"),
    ("shift", "KEY_SHIFT"),
    ("space", "KEY_SPACE"),
    ("stateShift", "KEY_STATE_SHIFT"),
    ("stateRepeat", "KEY_STATE_REPEAT"),
];

/// 本端多一个真源里没有的名字：必须报「未登记」，不许静默跳过。
/// 其余 6 条**故意照抄正确值**，好让这条 fixture 只差「多出来的那一个」。
pub const FIX_SWIFT_UNKNOWN: &str = r#"
enum OpiKey {
    static let specialBase: UInt32 = 0x1_0000
    static let backspace: UInt32 = 0x1_0008
    static let shift: UInt32 = 0x1_0083
    static let space: UInt32 = 0x20
    static let stateShift: UInt32 = 1 << 0
    static let stateRepeat: UInt32 = 1 << 27
    static let stateFoo: UInt32 = 1 << 29
}
"#;

/// 映射表全给、本端只声明了一半：反方向也要能报「登记了却没解析到」。
pub const FIX_SWIFT_MISSING: &str = r#"
enum OpiKey {
    static let specialBase: UInt32 = 0x1_0000
    static let stateRepeat: UInt32 = 1 << 27
}
"#;

/// 空壳：一条常量都没有。失明的解析器必须在这里红，而不是「零差异 = 绿」。
pub const FIX_SWIFT_EMPTY: &str = r#"
enum OpiKey {
    static func printable(_ s: UInt32) -> UInt32 {
        return s
    }
}
"#;
