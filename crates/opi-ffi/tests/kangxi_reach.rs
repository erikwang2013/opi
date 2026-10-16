// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **康熙字「手机抓取窗内可达」门禁**（2026-10-17 用户报障
//! 「为打不出『馫蘒』二字，主流输入法就可以」的修复执行点）。
//!
//! 走 **C ABI 面**（与 Android 的真实调用序列逐字同构：`opi_load(luna)` →
//! `opi_import_dict(kangxi.tsv)`（= 首启种子后的词库目录装载）→ 逐键输入 →
//! `opi_candidates(64)`（= `EngineController.fetchLimit`，平铺态也吃同一个窗））。
//!
//! 钉的是**抓取窗内可见**这个用户判据，不是「库里有」那级：改前实测（FREQ=4000）
//! 馫 在 xin 第 124 位、蘒 在 qiu 第 108 位 —— 库里有、窗里没有，等于打不出。
//! 改 FREQ（脚本 `gen_kangxi_dict.py`，现值 3.98e9）或换词库数据，先过这条。
//!
//! 单例是进程级的：全部门禁放一个 `#[test]`（同 `dict_import.rs` 的纪律）。

use opi_ffi::cabi::{
    OpiString, opi_candidates, opi_clear, opi_import_dict, opi_input_key, opi_load,
};

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
    unsafe { opi_ffi::cabi::opi_ffi_free_string(s) };
    out
}

fn call(path: &std::path::Path) -> (Vec<u16>, usize) {
    let u = to_units(&path.to_string_lossy());
    let len = u.len();
    (u, len)
}

#[test]
fn rare_kangxi_chars_land_within_the_mobile_fetch_window() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let luna = base.join("../../android/app/src/main/assets/luna.opid");
    let kx = base.join("../../data/raw/kangxi.tsv");
    assert!(luna.is_file(), "缺 luna 资产：{}", luna.display());
    assert!(kx.is_file(), "缺 kangxi.tsv：{}", kx.display());

    let (u, len) = call(&luna);
    assert!(unsafe { opi_load(u.as_ptr(), len) }, "luna 装载");
    let (u, len) = call(&kx);
    let count = unsafe { opi_import_dict(u.as_ptr(), len) };
    assert!(count >= 20_000, "kangxi 导入条数 {count}（应 ≥ 2 万）");

    for (py, target) in [("xin", "馫"), ("qiu", "蘒")] {
        for c in py.chars() {
            let (u, len) = call(std::path::Path::new(&c.to_string()));
            opi_input_key_patched(u.as_ptr(), len);
        }
        // 出口是 JSON 数组串（`texts_to_json`）——按带引号的整词断言，防子串假阳。
        let got = read(unsafe { opi_candidates(64) });
        assert!(
            got.contains(&format!("\"{target}\"")),
            "打 {py} 应在手机抓取窗（前 64）内出「{target}」—— 实得前 120 字符：{:?}",
            got.chars().take(120).collect::<String>()
        );
        unsafe { opi_clear() };
    }
}

/// `opi_input_key` 的薄封装：按 C ABI 的 UTF-16 (ptr, len) 传单字符。
fn opi_input_key_patched(ptr: *const u16, len: usize) {
    unsafe { opi_input_key(ptr, len) };
}
