// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 五笔的**协议面**门禁（2026-10-14，M4 第二波）：整数码表与配置面名字表。
//! 从 `wubi.rs` 拆出（那个文件加完这两条 511/500 撞门禁）。
//!
//! 这里钉的是**跨语言契约**：整数码是 JNI/C ABI/fcitx5 配置枚举的取值，名字是四端
//! ini 里存的字面值 —— 改一处 = 改协议（四端要一起动，`SetupForm` 与
//! `desktop/.../SettingsLockstepTest` 现读这里的表比对）。

/// 协议码表（四端唯一真源 = `engine_wubi.rs` 的 `code()/from_code()`）—— 逐值钉住：
/// 改一个数 = 改协议（JNI/C ABI/fcitx5 配置枚举/桌面 ini 都指这一份）。
/// 越界判据从 2 起（今天只用到 0/1），别拿「还没用到的值」当越界样例。
#[test]
fn wubi_protocol_codes_are_pinned() {
    use engine_core::{WubiMixedEntry, WubiReturnStyle, WubiStyle};
    for (s, c) in [(WubiStyle::Pure, 0), (WubiStyle::Mixed, 1)] {
        assert_eq!(s.code(), c);
        assert_eq!(WubiStyle::from_code(c), Some(s));
    }
    for (m, c) in [
        (WubiMixedEntry::DirectLetters, 0),
        (WubiMixedEntry::ZKey, 1),
    ] {
        assert_eq!(m.code(), c);
        assert_eq!(WubiMixedEntry::from_code(c), Some(m));
    }
    for (r, c) in [(WubiReturnStyle::Clear, 0), (WubiReturnStyle::CommitRaw, 1)] {
        assert_eq!(r.code(), c);
        assert_eq!(WubiReturnStyle::from_code(c), Some(r));
    }
    assert_eq!(WubiStyle::from_code(2), None);
    assert_eq!(WubiMixedEntry::from_code(-1), None);
    assert_eq!(WubiReturnStyle::from_code(99), None);
}

/// 配置面名字表：往返 + 认不得 → None（**不是**回默认）—— 与 `Scheme::name/from_name`
/// 同一条纪律；四端 ini 里存的就是这些字面值（桌面/TSF 的读侧靠它换算）。
#[test]
fn wubi_names_round_trip_and_reject_unknown() {
    use engine_core::{WubiMixedEntry, WubiReturnStyle, WubiStyle};
    for s in [WubiStyle::Pure, WubiStyle::Mixed] {
        assert_eq!(WubiStyle::from_name(s.name()), Some(s));
    }
    for m in [WubiMixedEntry::DirectLetters, WubiMixedEntry::ZKey] {
        assert_eq!(WubiMixedEntry::from_name(m.name()), Some(m));
    }
    for r in [WubiReturnStyle::Clear, WubiReturnStyle::CommitRaw] {
        assert_eq!(WubiReturnStyle::from_name(r.name()), Some(r));
    }
    assert_eq!(
        WubiStyle::from_name(" 五笔拼音 "),
        Some(WubiStyle::Mixed),
        "两侧空白容错"
    );
    assert_eq!(
        WubiStyle::from_name("混输"),
        None,
        "认不得 ⇒ None（简写不是名字）"
    );
    assert_eq!(WubiMixedEntry::from_name("z"), None, "「z键」才是名字");
    assert_eq!(WubiReturnStyle::from_name(""), None);
}
