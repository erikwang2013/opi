// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 由 lib.rs 搬出（`#[path]` 引入）以保持本文件 <500 行，与 input_method_tests.rs 同惯例。

use super::*;

/// 读取 OpString 内容（null ptr → 空串）并释放。
fn read_and_free(s: OpString) -> String {
    let out = if s.ptr.is_null() {
        String::new()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    unsafe { opi_ffi_free_string_utf8(s) };
    out
}

#[test]
fn mode_int_roundtrip() {
    assert_eq!(mode_from_int(0), Some(Mode::Pinyin));
    assert_eq!(mode_from_int(3), Some(Mode::Symbol));
    assert_eq!(mode_from_int(4), Some(Mode::Traditional));
    assert_eq!(mode_from_int(-1), None);
    assert_eq!(mode_to_int(Mode::English), 1);
    assert_eq!(mode_to_int(Mode::Number), 2);
    assert_eq!(mode_to_int(Mode::Traditional), 4);
}

#[test]
fn opstring_roundtrip() {
    let s = OpString::from_utf8("你好");
    let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
    assert_eq!(std::str::from_utf8(bytes).unwrap(), "你好");
    unsafe { opi_ffi_free_string_utf8(s) };
    let e = OpString::from_utf8("");
    assert!(e.ptr.is_null());
    assert_eq!(e.len, 0);
}

#[test]
fn install_singleton_fallback_and_path() {
    let _g = serial_install();
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
    // 坏路径 → Err（load_or_fallback 原样语义，不回退）
    assert!(install(Some("/nonexistent/opi.dict")).is_err());
    // 单例保持上一次成功装载可用
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
}

#[test]
fn input_key_invalid_utf8_is_lossy_and_safe() {
    let _g = serial_install();
    // 单字节无效 UTF-8 → lossy 替换为 U+FFFD，非 ASCII → 引擎忽略 → 空串
    let raw = [0xffu8];
    let out = unsafe { opi_fcitx5_input_key(raw.as_ptr(), raw.len()) };
    assert_eq!(read_and_free(out), "");
    // 多字节无效序列 → lossy 后为多字符 → 边界拒绝 → 空串
    let raw = [0xc3u8, 0x28u8];
    let out = unsafe { opi_fcitx5_input_key(raw.as_ptr(), raw.len()) };
    assert_eq!(read_and_free(out), "");
    // 不 panic、不返回垃圾
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
}

#[test]
fn input_key_null_ptr_with_len_returns_empty() {
    let _g = serial_install();
    // read_utf8 先判 null（lib.rs read_utf8 首行），len>0 也不触碰内存
    let out = unsafe { opi_fcitx5_input_key(std::ptr::null(), 5) };
    assert_eq!(read_and_free(out), "");
}

#[test]
fn opstring_empty_roundtrip() {
    let e = OpString::from_utf8("");
    assert!(e.ptr.is_null());
    assert_eq!(e.len, 0);
    // 空句柄 free 为无操作且安全；再读也为空
    unsafe { opi_ffi_free_string_utf8(e) };
    assert_eq!(read_and_free(OpString::empty()), "");
}

#[test]
fn candidates_without_buffer_is_empty_json() {
    let _g = serial_install();
    let out = unsafe { opi_fcitx5_candidates(8) };
    assert_eq!(read_and_free(out), "[]");
}

#[test]
fn key_event_english_letter_buffers_and_switch_clears() {
    let _g = serial_install();
    // 切英文：字母进引擎缓冲（2026-10-18 单词联想），不再直传提交
    unsafe { opi_fcitx5_switch_mode(1) };
    let r = unsafe { opi_fcitx5_key_event(97, 0) };
    assert_eq!(r.action, 1); // EngineHandled
    assert_eq!(with_state(|s| s.buffer()), Some("a".to_string()));
    // 切回拼音（清缓冲），英文缓冲不残留
    unsafe { opi_fcitx5_switch_mode(0) };
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
}

#[test]
fn key_event_pinyin_letter_handled_and_space_commits() {
    let _g = serial_install();
    unsafe { opi_fcitx5_switch_mode(0) };
    let r = unsafe { opi_fcitx5_key_event(104, 0) }; // 'h'
    assert_eq!(r.action, 1); // EngineHandled
    assert_eq!(with_state(|s| s.buffer()), Some("h".to_string()));
    // 缓冲非空空格 → 提交并清空（提交文本取决于词库，只断言非空）
    let r = unsafe { opi_fcitx5_key_event(32, 0) };
    assert_eq!(r.action, 2);
    assert!(!read_and_free(r.text).is_empty());
    let r = unsafe { opi_fcitx5_key_event(97, 0) }; // 'a'
    assert_eq!(r.action, 1);
    let r = unsafe { opi_fcitx5_key_event(32, 0) }; // buffer 非空 → 空格提交
    assert_eq!(r.action, 2);
    assert!(!read_and_free(r.text).is_empty());
}

#[test]
fn key_event_ctrl_passes_through_and_shift_consumed() {
    let _g = serial_install();
    // Ctrl+C → 直通
    let r = unsafe { opi_fcitx5_key_event(99, 1 << 2) };
    assert_eq!(r.action, 0);
    // 切英文后再按 shift（无修饰）→ EngineHandled（切模式清引擎 ⇧，故再按一次）
    unsafe { opi_fcitx5_switch_mode(1) };
    let r = unsafe { opi_fcitx5_key_event(0xffe1, 0) };
    assert_eq!(r.action, 1);
    // 英文空缓冲 + single shift → 字母进缓冲、大写且消费（2026-10-18 起走引擎）
    let r = unsafe { opi_fcitx5_key_event(97, 0) };
    assert_eq!(r.action, 1);
    assert_eq!(with_state(|s| s.buffer()), Some("A".to_string()));
    let r = unsafe { opi_fcitx5_key_event(97, 0) };
    assert_eq!(r.action, 1);
    assert_eq!(with_state(|s| s.buffer()), Some("Aa".to_string()));
}

/// 双拼两张设置出口（2026-10-13/14）：C++ 的枚举下标/方案码 → 引擎的布局与方案。
/// 这条是 Linux 端的**唯一**程序化取证点（C++ 的 applyOpiSettings 在配置变更时走
/// 同一条；那边要真 fcitx5 才能跑）。
#[test]
fn shuangpin_layout_and_scheme_exports_land_on_the_engine() {
    use engine_core::Scheme;
    let _g = serial_install();
    // 布局：C++ 枚举下标原样进引擎（钳制在引擎侧）。
    assert_eq!(with_state(|s| s.engine.shuangpin_layout()), Some(0));
    unsafe { opi_fcitx5_set_shuangpin_layout(4) };
    assert_eq!(
        with_state(|s| s.engine.shuangpin_layout()),
        Some(4),
        "紫光（下标 4）该原样进引擎"
    );
    unsafe { opi_fcitx5_set_shuangpin_layout(99) };
    assert_eq!(
        with_state(|s| s.engine.shuangpin_layout()),
        Some(5),
        "过大钳到末位（同 opi_set_shuangpin_layout）"
    );
    // 方案：枚举值 = 方案整数码（0=全拼 4=双拼）。
    assert_eq!(
        with_state(|s| s.engine.scheme()),
        Some(Scheme::Full),
        "缺省全拼"
    );
    unsafe { opi_fcitx5_set_scheme(4) };
    assert_eq!(
        with_state(|s| s.engine.scheme()),
        Some(Scheme::Shuangpin),
        "码 4 该切到双拼"
    );
    unsafe { opi_fcitx5_set_scheme(99) };
    assert_eq!(
        with_state(|s| s.engine.scheme()),
        Some(Scheme::Shuangpin),
        "越界码 = 空操作（不静默跳回全拼）"
    );
    unsafe { opi_fcitx5_set_scheme(0) };
    assert_eq!(
        with_state(|s| s.engine.scheme()),
        Some(Scheme::Full),
        "码 0 切回全拼"
    );
    // 读侧出口（2026-10-14 加，与 set_scheme 配对）：C 面的 opi_fcitx5_scheme() 必须
    // 与引擎同一值 —— opi-ctl 的 `mode` 动作就靠它派生状态 token（无本地镜像）。
    // 逐值对一遍写侧刚设过的两个码，外加「越界码是空操作」这条在读侧也看得见。
    unsafe { opi_fcitx5_set_scheme(4) };
    assert_eq!(unsafe { opi_fcitx5_scheme() }, 4, "读侧该回双拼码 4");
    unsafe { opi_fcitx5_set_scheme(99) };
    assert_eq!(
        unsafe { opi_fcitx5_scheme() },
        4,
        "越界写不动引擎 ⇒ 读侧也不动"
    );
    unsafe { opi_fcitx5_set_scheme(0) };
    assert_eq!(unsafe { opi_fcitx5_scheme() }, 0, "读侧该回全拼码 0");
}

/// 生效中文标点读侧（2026-10-15）：状态栏 `punct=` 那一格读的就是它。
/// 中文类模式跟设置值、非中文模式恒 false —— 与 `punct_text` 同一条线
/// （引擎侧逐模式真值在 engine-core 的 punctuation_switches.rs，这条验**出口**接对了）。
#[test]
fn effective_chinese_punct_export_follows_mode() {
    let _g = serial_install();
    unsafe { opi_fcitx5_set_chinese_punct(true) };
    unsafe { opi_fcitx5_switch_mode(0) }; // Pinyin
    assert!(
        unsafe { opi_fcitx5_effective_chinese_punct() },
        "拼音：跟设置（on）"
    );
    unsafe { opi_fcitx5_set_chinese_punct(false) };
    assert!(
        !unsafe { opi_fcitx5_effective_chinese_punct() },
        "拼音：跟设置（off）—— 是读数不是常量"
    );
    unsafe { opi_fcitx5_set_chinese_punct(true) };
    unsafe { opi_fcitx5_switch_mode(4) }; // Traditional
    assert!(
        unsafe { opi_fcitx5_effective_chinese_punct() },
        "繁体同拼音档（设置仍为 on）"
    );
    for mode in [1, 2, 3] {
        // English / Number / Symbol
        unsafe { opi_fcitx5_switch_mode(mode) };
        assert!(
            !unsafe { opi_fcitx5_effective_chinese_punct() },
            "模式 {mode}：恒 false（设置仍是 on —— 切中英文的显示联动就靠这条）"
        );
    }
    unsafe { opi_fcitx5_switch_mode(0) }; // 还原：不把单例留在别的模式
}

/// 五笔旋钮族（2026-10-14，M4 第二波）：六个出口真调 + 真码表端到端。
/// 这条是 Linux 端五笔的**唯一**程序化取证点（C++ 的 applyOpiSettings 走同一批出口）。
#[test]
fn wubi_exports_land_on_the_engine() {
    use engine_core::{Scheme, WubiMixedEntry, WubiReturnStyle, WubiStyle};
    let _g = serial_install();
    // 坏路径哨兵（空路径也 false；不回落内置）。
    assert!(!unsafe { opi_fcitx5_load_wubi(std::ptr::null(), 0) });
    let bad = b"/nonexistent/wubi86.opid";
    assert!(!unsafe { opi_fcitx5_load_wubi(bad.as_ptr(), bad.len()) });

    // 真表装载（路径按本仓布局推导；表缺失时跳过装载断言，旋钮照测）。
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/generated/wubi86.opid");
    let bytes = path.to_string_lossy().into_owned();
    let loaded = unsafe { opi_fcitx5_load_wubi(bytes.as_ptr(), bytes.len()) };
    if loaded {
        // 真打一个码：wqvb（86 的四码），方案切五笔后应有候选。
        unsafe { opi_fcitx5_set_scheme(Scheme::Wubi.code()) };
        assert_eq!(
            with_state(|s| s.mode()),
            Some(engine_core::composer::Mode::Pinyin)
        );
        for ch in ['w', 'q', 'v', 'b'] {
            unsafe { opi_fcitx5_input_key(&(ch as u8), 1) };
        }
        let buf = with_state(|s| s.buffer()).unwrap_or_default();
        assert_eq!(buf, "wqvb", "四码应进缓冲");
        let cands = with_state(|s| s.page_count()).unwrap_or(0);
        assert!(cands > 0, "五笔四码该有候选（页数 > 0）");
        unsafe { opi_fcitx5_clear() };
    } else {
        eprintln!("（wubi86.opid 不在 data/generated/ ⇒ 跳过真表段；旋钮段照测）");
    }

    // 旋钮：缺省 → 写回环 → 越界忽略 / 钳制（读侧经引擎回读）。
    assert_eq!(
        with_state(|s| s.engine.wubi_style()),
        Some(WubiStyle::Pure),
        "缺省纯五笔"
    );
    unsafe { opi_fcitx5_set_wubi_style(WubiStyle::Mixed.code()) };
    assert_eq!(
        with_state(|s| s.engine.wubi_style()),
        Some(WubiStyle::Mixed)
    );
    unsafe { opi_fcitx5_set_wubi_style(99) };
    assert_eq!(
        with_state(|s| s.engine.wubi_style()),
        Some(WubiStyle::Mixed),
        "越界风格忽略"
    );
    unsafe { opi_fcitx5_set_wubi_style(0) };

    unsafe { opi_fcitx5_set_wubi_mixed_entry(WubiMixedEntry::ZKey.code()) };
    assert_eq!(
        with_state(|s| s.engine.wubi_mixed_entry()),
        Some(WubiMixedEntry::ZKey)
    );
    unsafe { opi_fcitx5_set_wubi_mixed_entry(0) };

    unsafe { opi_fcitx5_set_wubi_candidate_limit(99) };
    assert_eq!(with_state(|s| s.engine.wubi_candidate_limit()), Some(9));
    unsafe { opi_fcitx5_set_wubi_candidate_limit(0) };
    assert_eq!(with_state(|s| s.engine.wubi_candidate_limit()), Some(1));
    unsafe { opi_fcitx5_set_wubi_candidate_limit(5) };

    unsafe { opi_fcitx5_set_wubi_return_style(WubiReturnStyle::CommitRaw.code()) };
    assert_eq!(
        with_state(|s| s.engine.wubi_return_style()),
        Some(WubiReturnStyle::CommitRaw)
    );
    unsafe { opi_fcitx5_set_wubi_return_style(0) };

    unsafe { opi_fcitx5_set_wubi_auto_commit(false) };
    assert_eq!(with_state(|s| s.engine.wubi_auto_commit()), Some(false));
    unsafe { opi_fcitx5_set_wubi_auto_commit(true) };
    assert_eq!(with_state(|s| s.engine.wubi_auto_commit()), Some(true));
}
