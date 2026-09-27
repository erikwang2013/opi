// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **第五道**契约门禁：常量「**值**」契约（前四道：JNI 名字/描述符、C 声明面、
//! 模式整数、Rust 内部两轨状态位）。
//!
//! ## 这道门禁补的是哪个盲区
//!
//! `abi_parity.py` / `c_abi_contract` / `jni_contract` 比的全是**名字与签名**；
//! 常量**值**不在它们的视野里。于是 `1 << 27` 写成 `1 << 26` 这类改动
//! **不报错、不崩溃**：`states & KEY_STATE_REPEAT` 恒假 → 那条分支静默死掉。
//! 本仓 iOS 侧 `OpiEngine.swift` 的 `stateRepeat` 就是这样一颗雷 —— 编译得过、
//! 跑得起来、功能不再发生。
//!
//! ## 对账方与真源
//!
//! 真源 = `crates/engine-core/src/keys.rs`（**只读**，本门禁一个字都不改它）。
//! 对账面 = 鸿蒙 C 头 / 鸿蒙 ETS / ios Swift / macos Swift（两个 enum）。
//! Android 侧**不在**本契约里：Kotlin 走的是 `inputKey(ch: String)` / `backspace()`，
//! 从没有把键码或状态位整数跨过去（grep `0x10000|shl 26|SPECIAL` 在 Kotlin 侧
//! 零命中）。这一条是覆盖边界，如实写在《覆盖边界》一节，不是遗漏。
//!
//! ## 与 `tests/jni/mode.rs` 的分工
//!
//! 那道管的是**模式整数**（Rust `convert.rs` ↔ Kotlin 三表互比）。同一类问题、
//! 不同的量：这里是**键码与状态位**。两份不重叠，别互相复制。
//!
//! ## 覆盖边界（如实列，见 `coverage_boundaries_are_honest`）
//!
//! * 鸿蒙 C / ETS **故意只抄 9 键**（无方向键）：子集合法，但清单被钉死在这份文件里。
//! * macos `OpiKeySpace.noKeyval = .max` 不是 `keys.rs` 的常量（本地哨兵），
//!   以 `locals` 形式登记并**钉死值**，不用「子集」蒙过去。
//! * 各端**取值点**（谁真的拿这些常量去比）不在本门禁里 —— 本门禁只保证
//!   「同名的值一致」。值一致而分支写错，是另一类问题（`states & REPEAT` 的
//!   `&` 写成 `|` 之类），不在本文件断言范围。

/// 与 `jni_contract` 共用同一份解析机器（`read` / `strip_comments`）；
/// 本目标只用其中两个函数，其余条目在本目标里天然是死代码。
#[allow(dead_code)]
#[path = "jni/checker.rs"]
mod checker;

#[path = "keycode/constants.rs"]
mod constants;

use constants::{
    HARMONY_C, HARMONY_ETS, IOS_SWIFT, KEYS_RS, MACOS_SWIFT, Surface, c_target, ets_target,
    missing_from, parse_c, parse_ets, parse_rust, parse_swift, reconcile,
};

// ---------- Swift 名 → 真源名（**整表**登记；少一条报「未登记」，多一条报「没解析到」） ----------

/// `ios/OpiEngine.swift` 的 `enum OpiKey`。`ret` / `delete` 这类不规则的拼写
/// 只能手写映射 —— 别改成「驼峰转下划线」的猜法，那种猜法在 `shift` 上会撞车。
const IOS_MAP: &[(&str, &str)] = &[
    ("specialBase", "SPECIAL_BASE"),
    ("backspace", "KEY_BACK_SPACE"),
    ("tab", "KEY_TAB"),
    ("ret", "KEY_RETURN"),
    ("escape", "KEY_ESCAPE"),
    ("pageUp", "KEY_PAGE_UP"),
    ("pageDown", "KEY_PAGE_DOWN"),
    ("delete", "KEY_DELETE"),
    ("shift", "KEY_SHIFT"),
    ("up", "KEY_UP"),
    ("down", "KEY_DOWN"),
    ("left", "KEY_LEFT"),
    ("right", "KEY_RIGHT"),
    ("space", "KEY_SPACE"),
    ("stateShift", "KEY_STATE_SHIFT"),
    ("stateCapsLock", "KEY_STATE_CAPS_LOCK"),
    ("stateCtrl", "KEY_STATE_CTRL"),
    ("stateAlt", "KEY_STATE_ALT"),
    ("stateMeta", "KEY_STATE_META"),
    ("stateReleased", "KEY_STATE_RELEASED"),
    ("stateRepeat", "KEY_STATE_REPEAT"),
    ("stateLongPressed", "KEY_STATE_LONG_PRESSED"),
];

const MACOS_SPACE_MAP: &[(&str, &str)] = &[
    ("specialBase", "SPECIAL_BASE"),
    ("backspace", "KEY_BACK_SPACE"),
    ("tab", "KEY_TAB"),
    ("ret", "KEY_RETURN"),
    ("escape", "KEY_ESCAPE"),
    ("pageUp", "KEY_PAGE_UP"),
    ("pageDown", "KEY_PAGE_DOWN"),
    ("del", "KEY_DELETE"),
    ("shift", "KEY_SHIFT"),
    ("up", "KEY_UP"),
    ("down", "KEY_DOWN"),
    ("left", "KEY_LEFT"),
    ("right", "KEY_RIGHT"),
    ("space", "KEY_SPACE"),
];

/// `noKeyval` 是 macOS 侧的**本地哨兵**（`.max`），真源里没有它。登记 + 钉死值。
const MACOS_SPACE_LOCALS: &[(&str, i64, &str)] = &[(
    "noKeyval",
    u32::MAX as i64,
    "本地哨兵：与 tsf 的 NO_KEYVAL 同义（u32::MAX），真源 keys.rs 不管这个数",
)];

const MACOS_STATE_MAP: &[(&str, &str)] = &[
    ("shift", "KEY_STATE_SHIFT"),
    ("capsLock", "KEY_STATE_CAPS_LOCK"),
    ("ctrl", "KEY_STATE_CTRL"),
    ("alt", "KEY_STATE_ALT"),
    ("meta", "KEY_STATE_META"),
    ("released", "KEY_STATE_RELEASED"),
    ("repeatKey", "KEY_STATE_REPEAT"),
    ("longPressed", "KEY_STATE_LONG_PRESSED"),
];

// ---------- 各面 ----------

type Locals = &'static [(&'static str, i64, &'static str)];

fn truth() -> Surface {
    parse_rust(
        &checker::read(KEYS_RS),
        "engine-core · keys.rs（真源）",
        KEYS_RS,
    )
    .expect("真源解析失败 —— 解析器失明比门禁红更糟，先修解析器")
}

fn surfaces() -> Vec<(Surface, Locals)> {
    vec![
        (
            parse_c(
                &checker::read(HARMONY_C),
                "harmony · cpp/opi_ffi.h",
                HARMONY_C,
            )
            .unwrap(),
            &[],
        ),
        (
            parse_ets(
                &checker::read(HARMONY_ETS),
                "harmony · ets/OpiEngine.ets",
                HARMONY_ETS,
            )
            .unwrap(),
            &[],
        ),
        (
            parse_swift(
                &checker::read(IOS_SWIFT),
                "enum OpiKey {",
                "ios · OpiEngine.swift#OpiKey",
                IOS_SWIFT,
                IOS_MAP,
            )
            .unwrap(),
            &[],
        ),
        (
            parse_swift(
                &checker::read(MACOS_SWIFT),
                "enum OpiKeySpace {",
                "macos · OpiEngine.swift#OpiKeySpace",
                MACOS_SWIFT,
                MACOS_SPACE_MAP,
            )
            .unwrap(),
            MACOS_SPACE_LOCALS,
        ),
        (
            parse_swift(
                &checker::read(MACOS_SWIFT),
                "enum OpiKeyState {",
                "macos · OpiEngine.swift#OpiKeyState",
                MACOS_SWIFT,
                MACOS_STATE_MAP,
            )
            .unwrap(),
            &[],
        ),
    ]
}

// ---------- 主门禁 ----------

/// 真源自身的锚点。**故意冗余**：5 条钉死的值对应 5 种「改坏一点」的形状，
/// 只要真源与各端被一起改歪，这条就会红（其余 17 条由下面的逐面比对覆盖）。
#[test]
fn keys_rs_true_source_is_pinned() {
    let t = truth();
    assert_eq!(
        t.declared, 22,
        "真源 keys.rs 的 `pub const` 条数变了：13 键码 + 8 状态位 + SPECIAL_BASE = 22"
    );
    let anchors: [(&str, i64, &str); 5] = [
        ("SPECIAL_BASE", 0x1_0000, "两个值空间的分界"),
        ("KEY_SPACE", 0x20, "可打印段的一员，不是漏套基址的特殊键"),
        ("KEY_SHIFT", 0x1_0000 | 0x83, "特殊键里低字节最大的那个"),
        ("KEY_STATE_REPEAT", 1 << 27, "iOS 侧那颗雷的位置"),
        ("KEY_STATE_LONG_PRESSED", 1 << 28, "最高的一个状态位"),
    ];
    for (name, want, why) in anchors {
        let got = &t.consts[name];
        assert_eq!(
            got.value, want,
            "真源 {name}（`{}`）= {}，锚点是 {want}（{why}）",
            got.expr, got.value
        );
    }
}

/// **主门禁**：五个面逐条与真源比值，零差异才绿。
#[test]
fn every_surface_matches_the_true_source_value_for_value() {
    let t = truth();
    let mut diffs = Vec::new();
    for (s, locals) in surfaces() {
        diffs.extend(reconcile(&s, &t, locals));
    }
    assert!(
        diffs.is_empty(),
        "常量值与真源不符（共 {} 条）：\n  {}",
        diffs.len(),
        diffs.join("\n  ")
    );
}

/// 非空转护栏：每条声明都必须被真解析到，条数还得是**实测**的那个数。
/// 少解析到一条 = 门禁对那一条失明，比红更危险。
#[test]
fn declaration_counts_are_the_measured_ones() {
    let t = truth();
    assert_eq!(t.declared, t.consts.len(), "真源有同名重复声明");
    for (s, _) in surfaces() {
        let want = match s.label.as_str() {
            "harmony · cpp/opi_ffi.h" => 18,
            "harmony · ets/OpiEngine.ets" => 18,
            "ios · OpiEngine.swift#OpiKey" => 22,
            "macos · OpiEngine.swift#OpiKeySpace" => 15,
            "macos · OpiEngine.swift#OpiKeyState" => 8,
            other => panic!("{other} 没钉条数：新出现的声明面必须显式登记，别让它悄悄溜进门禁"),
        };
        assert_eq!(
            s.declared, want,
            "[{}] 声明条数变了（实测 {want}）—— 增删常量必须同时改这份门禁",
            s.label
        );
        assert_eq!(
            s.declared,
            s.consts.len(),
            "[{}] 有同名重复：后一条会静默覆盖前一条",
            s.label
        );
    }
}

/// **有意的子集也要钉死**：鸿蒙两端故意不抄方向键（`OpiEngine.ets` 里写了理由），
/// Swift 两端是全抄。子集不算缺陷，但「悄悄地少抄一个」必须红。
#[test]
fn intended_subsets_are_pinned_exactly() {
    let t = truth();
    let mut got = Vec::new();
    for (s, _) in surfaces() {
        got.push((s.label.clone(), missing_from(&s, &t)));
    }
    let want: Vec<(&str, Vec<&str>)> = vec![
        (
            "harmony · cpp/opi_ffi.h",
            vec!["KEY_DOWN", "KEY_LEFT", "KEY_RIGHT", "KEY_UP"],
        ),
        (
            "harmony · ets/OpiEngine.ets",
            vec!["KEY_DOWN", "KEY_LEFT", "KEY_RIGHT", "KEY_UP"],
        ),
        ("ios · OpiEngine.swift#OpiKey", vec![]),
        (
            // ⚠️ 面 ≠ 文件：`OpiKeySpace` 只管键码，8 个状态位在隔壁 `OpiKeyState`
            // 里（同一个文件）。两个 enum 合起来才是全集 —— 见下一条用例。
            "macos · OpiEngine.swift#OpiKeySpace",
            vec![
                "KEY_STATE_ALT",
                "KEY_STATE_CAPS_LOCK",
                "KEY_STATE_CTRL",
                "KEY_STATE_LONG_PRESSED",
                "KEY_STATE_META",
                "KEY_STATE_RELEASED",
                "KEY_STATE_REPEAT",
                "KEY_STATE_SHIFT",
            ],
        ),
        (
            "macos · OpiEngine.swift#OpiKeyState",
            vec![
                "KEY_BACK_SPACE",
                "KEY_DELETE",
                "KEY_DOWN",
                "KEY_ESCAPE",
                "KEY_LEFT",
                "KEY_PAGE_DOWN",
                "KEY_PAGE_UP",
                "KEY_RETURN",
                "KEY_RIGHT",
                "KEY_SHIFT",
                "KEY_SPACE",
                "KEY_TAB",
                "KEY_UP",
                "SPECIAL_BASE",
            ],
        ),
    ];
    for ((label, missing), (wlabel, wmissing)) in got.iter().zip(want.iter()) {
        assert_eq!(label.as_str(), *wlabel, "面顺序变了，两份清单对不上");
        let m: Vec<&str> = missing.iter().map(String::as_str).collect();
        assert_eq!(
            m, *wmissing,
            "[{label}] 没抄的真源常量清单变了 —— 有意的子集可以，静默增删不行"
        );
    }
}

/// **面 ≠ 文件**：macOS 一个文件里两个 enum，各自只是半边；合起来才是全集。
/// iOS 一个 enum 装全部。这条补上「子集清单」看不到的那半边 —— 只看单面，
/// 「macOS 少抄了 8 个状态位」会被误读成缺陷。
#[test]
fn files_that_should_copy_everything_copy_everything() {
    use std::collections::BTreeSet;
    let t = truth();
    let all: BTreeSet<String> = t.consts.keys().cloned().collect();
    let mut ios: BTreeSet<String> = BTreeSet::new();
    let mut macos: BTreeSet<String> = BTreeSet::new();
    for (s, _) in surfaces() {
        let into = if s.label.starts_with("ios") {
            &mut ios
        } else if s.label.starts_with("macos") {
            &mut macos
        } else {
            continue;
        };
        into.extend(s.map.values().cloned());
    }
    assert_eq!(ios, all, "ios 的单 enum 应当把真源抄全");
    assert_eq!(
        macos, all,
        "macos 的两个 enum 合起来应当把真源抄全（单看一个 enum 只是半边）"
    );
}

/// 两个值空间不许相交（这就是 `SPECIAL_BASE` 存在的理由，也是 TSF 那次
/// 「`.` 被当退格吃掉」的病根）：特殊键 ≥ SPECIAL_BASE、低字节不撞空格、
/// 12 个键码互不相同、8 个状态位是互不相同的 2 的幂。
#[test]
fn the_two_value_spaces_stay_disjoint() {
    let t = truth();
    let space = t.consts["KEY_SPACE"].value;
    // 措辞按 team-lead 2026-09-28 裁示：说「可打印段的一员」，不说「唯一不套基址的键」——
    // 后者把 KEY_SPACE 框成**例外**，而例外最容易被「顺手补上基址」，会让 `0x20 → 0x10020` 那个变异看起来像修正。
    assert_eq!(
        space, 0x20,
        "KEY_SPACE 属可打印段（0x20 = 空格码点），不是漏套基址的特殊键"
    );
    assert!(space < t.consts["SPECIAL_BASE"].value, "空格属于可打印段");
    let mut keys = Vec::new();
    for (name, c) in &t.consts {
        if !name.starts_with("KEY_") || name.starts_with("KEY_STATE_") || name == "KEY_SPACE" {
            continue;
        }
        assert!(
            c.value >= t.consts["SPECIAL_BASE"].value,
            "特殊键 {name} = {}（`{}`）掉到可打印段里了",
            c.value,
            c.expr
        );
        assert_ne!(
            c.value & 0xFFFF,
            space,
            "特殊键 {name} 的低字节撞上了 KEY_SPACE（`{}`）—— 这就是那个 TSF 病根",
            c.expr
        );
        keys.push((name.clone(), c.value));
    }
    keys.sort_by_key(|(_, v)| *v);
    let n = keys.len();
    assert_eq!(n, 12, "特殊键应当 12 个（含方向键），实得 {keys:?}");
    keys.dedup_by_key(|(_, v)| *v);
    assert_eq!(n, keys.len(), "特殊键里有重复值：{keys:?}");
    let mut bits: Vec<(String, i64)> = t
        .consts
        .iter()
        .filter(|(n, _)| n.starts_with("KEY_STATE_"))
        .map(|(n, c)| (n.clone(), c.value))
        .collect();
    bits.sort_by_key(|(_, v)| *v);
    assert_eq!(bits.len(), 8, "状态位应当是 8 个");
    for (name, v) in &bits {
        assert_eq!(
            v.count_ones(),
            1,
            "状态位 {name} = {v} 不是单个 bit —— `&` 判断会连带命中别的状态"
        );
    }
    for (kname, kv) in &keys {
        for (sname, sv) in &bits {
            assert_ne!(kv, sv, "键码 {kname} 与状态位 {sname} 同值");
        }
    }
}

/// **非空转，看得见**：把逐面覆盖矩阵打出来（`cargo test -p opi_ffi --test
/// keycode_contract -- --nocapture`）。绿而空转是这批门禁唯一真正危险的失败模式，
/// 所以「解析到了多少、覆盖了哪些」要能被任何人一条命令复核，而不是靠一句话声明。
#[test]
fn coverage_matrix_is_printable() {
    let t = truth();
    let total = t.consts.len();
    let mut covered: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    println!("\n真源 keys.rs：{total} 条常量");
    for (s, _) in surfaces() {
        let missing = missing_from(&s, &t);
        covered.extend(s.map.values().cloned());
        println!(
            "  {:<38} 解析 {:>2} 条 · 覆盖 {:>2}/{total} · 未抄 [{}]",
            s.label,
            s.consts.len(),
            s.map.len(),
            missing.join(" ")
        );
    }
    println!("  全部端合起来覆盖 {}/{total}\n", covered.len());
    // 逐面：解析到的每一条都必须真的参与比对（`map` 是整表登记，不是抽样）
    for (s, _) in surfaces() {
        assert!(
            s.consts.len() >= 8,
            "[{}] 只解析到 {} 条 —— 少到不合理，先怀疑解析器",
            s.label,
            s.consts.len()
        );
    }
    assert_eq!(
        covered.len(),
        total,
        "所有端合起来都没盖满真源：说明有常量谁都没抄"
    );
}

/// 覆盖边界：本门禁**不**保证的事，写死在用例里，免得下次有人把它当已覆盖。
#[test]
fn coverage_boundaries_are_honest() {
    // 1. ETS 的目标名映射是机械的（`STATE_*` → `KEY_STATE_*`），C 的也是（剥 `OPI_`）。
    //    这里把它们钉住：改名会让映射静默错位。
    assert_eq!(ets_target("STATE_LONG_PRESSED"), "KEY_STATE_LONG_PRESSED");
    assert_eq!(ets_target("KEY_SPACE"), "KEY_SPACE");
    assert_eq!(ets_target("SPECIAL_BASE"), "SPECIAL_BASE");
    assert_eq!(c_target("OPI_KEY_SPECIAL_BASE"), "SPECIAL_BASE");
    assert_eq!(c_target("OPI_KEY_STATE_REPEAT"), "KEY_STATE_REPEAT");
    assert_eq!(c_target("OPI_KEY_BACK_SPACE"), "KEY_BACK_SPACE");

    // 2. Android 不在这份契约里：Kotlin 侧的入口是 `inputKey(ch: String)` /
    //    `backspace()`，键码与状态位从不跨语言。这条**不是**「已覆盖」，
    //    是「该端根本没有这个量」——哪天 Kotlin 开始传 keyval，这道门禁必须扩。
    //    （复现：`grep -rn '0x10000\|shl 26\|SPECIAL' android/app/src/main/kotlin` 零命中。）

    // 3. 两轨 Rust（tsf / fcitx5）**不在对账面**，但理由**不是**「它们直接用真源」——
    //    2026-09-28 实测：两个 crate 确实依赖 engine-core，但用的是引擎/词典/符号 API，
    //    **不是** `use engine_core::keys::*`；两处都**手抄**，而且是**另一套键码空间**：
    //    * `crates/tsf-opi/src/logic_input_method.rs:24-26` 明文写着键码约定
    //      = `SPECIAL_BASE | Windows VK`：`KEY_PAGE_UP = 0x1_0000|0x21`、
    //      `KEY_DELETE = 0x1_0000|0x2e`、`KEY_SHIFT = 0x1_0000|0x10`、
    //      `KEY_SPACE = 0x1_0000|0x20` —— 与真源的 `|0x80..|0x83` / `0x20` **故意不同**，
    //      理由也写在同处（裸用 VK 会撞 ASCII：`0x2E` 是 `.`）。
    //    * `crates/fcitx5-opi/src/input_method.rs:29-47` 用的是 X11 keysym
    //      （`0xff08` / `0xff55` …），第三套空间。
    //    ⇒ **拿真源来对它们会得到一堆假红**，那是把「有意的差异」报成缺陷（本会话已踩过三次的
    //    同一坑）。所以此处**不纳入**。
    //    * 真正共享的是**状态位**（`1<<0..4` / `1<<26..28`，两处都有手抄），由已有的
    //      `two_track_keycodes.rs` 管；26/27/28 另有 fcitx5 C++ 侧的 `static_assert`
    //      （`crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 里 `kOpiWireMask` 后面那三条
    //      `static_assert`；按符号 grep，别按行号 —— 行号引用在此处已错过一次）守线格式。
    //    ✅ **该缺口已闭合**（2026-09-28 订正：此处原写「本文件没做」，与现状相反）——
    //      `two_track_keycodes.rs::state_bits_are_anchored_to_the_true_source_not_to_each_other`
    //      把两轨这 7 个位**逐个对着真源** `engine_core::keys` 比（非两轨互比；`META` 两轨都没有，是子集）。

    // 4. ⚠️ **本门禁是文本层扫描（读文件 + 求值），所以 `cfg` 挡不住它** ——
    //    而 `cargo check` / clippy / 测试**会**被 `cfg` 挡住：主机（Linux）上
    //    `#[cfg(target_os = "windows")]` 的模块**不参与类型检查与 lint**。
    //    两面互补，各有一块看不见的地方：
    //    * 主机侧「全绿」对 Windows-only 代码是**零证据** —— `crates/tsf-opi/src/vk.rs`
    //      那条 `#[cfg(target_os = "windows")] const _: () = assert!(VK_OEM_7 ==
    //      windows::…::VK_OEM_7.0 as u32)` 的真源是 `windows` crate 的 `VK_*`，
    //      **不是** `keys.rs`，所以本门禁也**不覆盖它**；它只在
    //      `cargo check --target x86_64-pc-windows-msvc` 下才被求值。
    //    * 本门禁不受这条影响（读的是文件，不看 cfg），代价是它只认**文本**：
    //      按 cfg 条件生成/替换常量的构建脚本、宏展开出的值，它看不见。
    //    （2026-09-28 沙盒实测这个机制的两个方向：同形状的假断言
    //      `#[cfg(target_os = "windows")] const _: () = assert!(1u32 == 2u32);`
    //      在主机上 `cargo check` EXIT=0，在 `--target x86_64-pc-windows-msvc`
    //      下 `error[E0080]: evaluation panicked`。）
}
