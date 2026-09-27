// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **键码空间**的边界：三套「特殊键」编码与「可打印 = 码点」那一段必须互不撞号。
//!
//! 三套空间：
//! - `engine_core::keys`：`SPECIAL_BASE(0x1_0000) | 低 16 位`，C ABI（`opi_key_event`）用；
//! - `tsf_opi::logic`：`SPECIAL_BASE(0x1_0000) | Windows VK`，TSF 胶水用；
//! - `fcitx5_opi::input_method`：fcitx5 keysym（`0xff00+`），fcitx5 胶水用。
//!
//! 这条纪律有过血的教训（见 tsf 轨 `SPECIAL_BASE` 的注释）：TSF 轨曾裸用 VK，
//! 于是 `VK_NEXT=0x22` 撞上 `"`、`VK_DELETE=0x2E` 撞上 `.` —— 表现为
//! **普通字符被吃掉**（拼音打一半敲 `.` 变退格）。所以这里不问「有没有撞」，
//! 而是把两件事都钉住：
//! 1. 特殊键码本身必须落在可打印段之外（`> 0x7f` 且 `char::from_u32` 得非 ASCII）；
//! 2. **同一个数字在两个入口下含义不同，且可打印入口永远不触发特殊行为** ——
//!    这是真正的「不相交」证明：`SPECIAL_BASE` 取 0x1_0000 之后，特殊键码本身就
//!    是**合法的 Unicode 字符**（补充平面），数学上并不与「所有可打印码点」不相交，
//!    安全完全依赖「字符走 `input_key`、键值走 `key_event`」这条入口分工。
//!    所以第 2 条必须测：把特殊键码当字符喂进去，缓冲一个字符都不能变。

use engine_core::keys as ek;
use fcitx5_opi::input_method as fim;
use fcitx5_opi::{opi_fcitx5_buffer, opi_fcitx5_input_key, opi_ffi_free_string_utf8};
use opi_ffi::cabi::{
    OpiString, opi_buffer, opi_candidates, opi_clear, opi_ffi_free_string, opi_input_key, opi_load,
    opi_set_learner, opi_switch_mode,
};
use tsf_opi::logic as tlg;

/// 取 android 部署副本：`data/generated/luna.opid` 被 gitignore（构建中间物），
/// 全新 clone 里不存在。两份内容逐字节相同。
const LUNA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

/// 三个模块各自的特殊键常量（不含空格 —— 空格在三套里的编码本就不同）。
fn all_special_codes() -> Vec<(&'static str, u32)> {
    let mut v: Vec<(&'static str, u32)> = vec![
        ("engine_core", ek::KEY_BACK_SPACE),
        ("engine_core", ek::KEY_TAB),
        ("engine_core", ek::KEY_RETURN),
        ("engine_core", ek::KEY_ESCAPE),
        ("engine_core", ek::KEY_PAGE_UP),
        ("engine_core", ek::KEY_PAGE_DOWN),
        ("engine_core", ek::KEY_DELETE),
        ("engine_core", ek::KEY_SHIFT),
        ("engine_core", ek::KEY_UP),
        ("engine_core", ek::KEY_DOWN),
        ("engine_core", ek::KEY_LEFT),
        ("engine_core", ek::KEY_RIGHT),
        ("tsf", tlg::KEY_BACK_SPACE),
        ("tsf", tlg::KEY_TAB),
        ("tsf", tlg::KEY_RETURN),
        ("tsf", tlg::KEY_ESCAPE),
        ("tsf", tlg::KEY_PAGE_UP),
        ("tsf", tlg::KEY_PAGE_DOWN),
        ("tsf", tlg::KEY_DELETE),
        ("tsf", tlg::KEY_SHIFT),
        ("tsf", tlg::KEY_SPACE),
        ("fcitx5", fim::KEY_BACK_SPACE),
        ("fcitx5", fim::KEY_TAB),
        ("fcitx5", fim::KEY_RETURN),
        ("fcitx5", fim::KEY_ESCAPE),
        ("fcitx5", fim::KEY_PAGE_UP),
        ("fcitx5", fim::KEY_PAGE_DOWN),
        ("fcitx5", fim::KEY_SHIFT_L),
        ("fcitx5", fim::KEY_SHIFT_R),
        ("fcitx5", fim::KEY_DELETE),
    ];
    v.sort_by_key(|(_, c)| *c);
    v
}

/// 不变量 1：每个特殊键码都 > 0x7f，且能解码成非 ASCII 字符。
///
/// 「能解码」不是形式主义：漏了基址/越界的键码会落进 `char::from_u32 → None`
/// 那一支，行为是「静默忽略」而不是报错 —— 键按下去了没反应，最难查的一类。
#[test]
fn special_codes_stay_out_of_the_printable_ascii_range() {
    let codes = all_special_codes();
    assert_eq!(codes.len(), 30, "三套空间的特殊键常量数");
    for (who, code) in &codes {
        assert!(
            *code > 0x7f,
            "{who} 的特殊键码 {code:#x} 落进 ASCII 段 —— 普通字符会被吃掉"
        );
        let c = char::from_u32(*code).unwrap_or_else(|| {
            panic!("{who} 的特殊键码 {code:#x} 不可解码，漏 match 臂时会静默忽略")
        });
        assert!(
            !c.is_ascii(),
            "{who} 的特殊键码 {code:#x} 解码成 ASCII 字符 {c:?}：落到可打印分支会变成垃圾字符"
        );
    }
    // 反过来问一遍（这才是「不相交」的正面表述）：**任何**可打印 ASCII 都不等于特殊键码。
    for c in 0x20u32..=0x7e {
        for (who, code) in &codes {
            assert_ne!(
                c, *code,
                "可打印 ASCII {c:#x} 撞上 {who} 的特殊键码 —— 敲这个字符会触发特殊行为"
            );
        }
    }
    // 同一套空间内部也不许重复（两个键码相同 → 一个键被另一个顶掉）。
    for i in 0..codes.len() {
        for j in i + 1..codes.len() {
            if codes[i].0 == codes[j].0 {
                assert_ne!(
                    codes[i].1, codes[j].1,
                    "{} 内特殊键码重复：{:#x}",
                    codes[i].0, codes[i].1
                );
            }
        }
    }
}

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn read(s: OpiString) -> String {
    let out = if s.ptr.is_null() {
        String::new()
    } else {
        let units = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        String::from_utf16(units).expect("导出必须返回合法 UTF-16")
    };
    unsafe { opi_ffi_free_string(s) };
    out
}

/// 不变量 2（C ABI 面）：**字符入口永不触发特殊键**。
///
/// 把每一个特殊键码当成**字符**送进 `opi_input_key`，缓冲必须一字不变 ——
/// 这些码点（`0x1_0008`…`0x1_0087`）都是合法的补充平面字符，与 `SPECIAL_BASE|VK`
/// 数值完全相同，所以这条是「同一个数字、两个入口、含义必须分开」的实测。
///
/// 正控在末尾：同一个数字走**键事件**入口必须真的删掉一个字符 ——
/// 没有它，「字符入口什么都不做」可能只是因为这个数字压根没被处理。
#[test]
fn the_character_entry_point_never_triggers_a_special_key() {
    let path = to_units(LUNA);
    assert!(unsafe { opi_load(path.as_ptr(), path.len()) }, "装载 luna");
    unsafe {
        opi_set_learner(false);
        opi_switch_mode(0);
        opi_clear();
    }
    for c in "ni".chars() {
        let one = to_units(&c.to_string());
        read(unsafe { opi_input_key(one.as_ptr(), one.len()) });
    }
    assert_eq!(read(unsafe { opi_buffer() }), "ni", "前置：拼音缓冲已建立");
    let cands = read(unsafe { opi_candidates(8) });
    assert!(!cands.is_empty(), "前置：有候选：{cands}");

    for (who, code) in all_special_codes() {
        let c = char::from_u32(code).expect("特殊键码是合法字符");
        let one = to_units(&c.to_string());
        let out = read(unsafe { opi_input_key(one.as_ptr(), one.len()) });
        assert_eq!(
            out, "",
            "{who} 的键码 {code:#x}（= 字符 {c:?}）从**字符**入口进来却提交了文本"
        );
        assert_eq!(
            read(unsafe { opi_buffer() }),
            "ni",
            "{who} 的键码 {code:#x}（= 字符 {c:?}）从字符入口改动了缓冲 —— 特殊键与可打印段撞号"
        );
        assert_eq!(
            read(unsafe { opi_candidates(8) }),
            cands,
            "{who} 的键码 {code:#x}（= 字符 {c:?}）从字符入口动了候选"
        );
    }

    // 正控：同一个数字（engine-core 的退格）走**键事件**入口必须真的删。
    let r = unsafe { opi_ffi::cabi::opi_key_event(ek::KEY_BACK_SPACE, 0) };
    assert_eq!(r.action, 1, "键事件入口的退格必须被引擎接管");
    assert_eq!(read(unsafe { opi_buffer() }), "n", "退格必须删掉一个码点");
}

/// 不变量 3（fcitx5 轨）：`opi_fcitx5_input_key` 的**文档契约**是
/// 「空串/多字符/**非 ASCII** → 空串」（`crates/fcitx5-opi/src/lib.rs` 该函数头注释）。
/// 实现却是 `handle_key(s, c as u32, 0)`：键值空间按**码点**解释，于是
/// U+FF00–U+FFFF 里那 9 个「数值恰好等于某个 keysym」的字符会被当成控制键。
///
/// 最小复现（本机实证）：
/// ```text
/// （先装好引擎、敲 n i 建出缓冲）
/// opi_fcitx5_input_key("－")   // U+FF0D，数值 = KEY_RETURN(0xff0d)
///   → 返回候选文本（≠ 空串，违反「非 ASCII → 空串」），且缓冲被提交清空
/// opi_fcitx5_input_key("（")   // U+FF08，数值 = KEY_BACK_SPACE(0xff08) → 删掉一个码点
/// ```
/// 受影响的字符（数值 = keysym 者）：`（` `）` `－` `；` `ｕ` `ｖ` `￡` `￠` `U+FFFF`。
/// 其中 `￡`（U+FFE1，全角英镑符）与 `ｕ`/`ｖ`（半角拉丁字母）是真实文本里会出现的字符。
///
/// **可达性**：本导出声明在 `crates/fcitx5-opi/cpp/opi_fcitx5.cpp:50`，但出货的
/// C++ 胶水**没有调用它**（按键走的是 `keyEvent` 那条路）。所以今天是潜在缺陷、
/// 不是线上故障 —— 但它是 `.so` 的公开导出，第三方或将来改胶水时按文档调用即中招。
/// 根因是「键值 = 码点」这个假设只在 ASCII 段成立（真实 fcitx5 的非 ASCII
/// keysym 是 `0x0100_0000 | 码点`）。
///
/// **已修**：入口加 `if !c.is_ascii() { return 空串 }`（贴文档那条；另一条「改按
/// keysym 解释」是**反向改契约**且牵动真实前端，未采纳）。改的是导出壳，不动
/// `input_method::handle_key`，故两轨行为对照不受影响。
#[test]
fn fcitx5_char_entry_must_not_treat_fullwidth_text_as_control_keys() {
    fcitx5_opi::install(Some(LUNA)).expect("装载 luna");
    let feed = |ch: char| -> String {
        let s = ch.to_string();
        let out = unsafe { opi_fcitx5_input_key(s.as_ptr(), s.len()) };
        let text = if out.ptr.is_null() {
            String::new()
        } else {
            String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(out.ptr, out.len) })
                .into_owned()
        };
        unsafe { opi_ffi_free_string_utf8(out) };
        text
    };
    let buffer = || -> String {
        let b = unsafe { opi_fcitx5_buffer() };
        let s = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(b.ptr, b.len) })
            .into_owned();
        unsafe { opi_ffi_free_string_utf8(b) };
        s
    };

    for c in "ni".chars() {
        assert_eq!(feed(c), "", "ASCII 字母入缓冲，不提交");
    }
    assert_eq!(buffer(), "ni", "前置：拼音缓冲已建立");

    // U+FF0D 的数值等于 KEY_RETURN(0xff0d)。
    let before = buffer();
    assert_eq!(
        feed('\u{ff0d}'),
        "",
        "文档：非 ASCII → 空串。现值回来了「提交文本」——它被当成了回车"
    );
    assert_eq!(
        buffer(),
        before,
        "文档：非 ASCII 不该改状态。现值把缓冲提交清空了（被当成回车）"
    );

    // 其余 8 个数值等于 keysym 的字符：一个都不许动状态。
    for (c, why) in [
        ('\u{ff08}', "= KEY_BACK_SPACE(0xff08)"),
        ('\u{ff09}', "= KEY_TAB(0xff09)"),
        ('\u{ff1b}', "= KEY_ESCAPE(0xff1b)"),
        ('\u{ff55}', "= KEY_PAGE_UP(0xff55)"),
        ('\u{ff56}', "= KEY_PAGE_DOWN(0xff56)"),
        ('\u{ffe1}', "= KEY_SHIFT_L(0xffe1)"),
        ('\u{ffe2}', "= KEY_SHIFT_R(0xffe2)"),
        ('\u{ffff}', "= KEY_DELETE(0xffff)"),
    ] {
        let before = buffer();
        assert_eq!(feed(c), "", "{c:?}（{why}）是非 ASCII，文档说返回空串");
        assert_eq!(buffer(), before, "{c:?}（{why}）不得改动缓冲");
    }
}
