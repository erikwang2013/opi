// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **康熙字「手机抓取窗内可达」门禁**（2026-10-17 用户报障
//! 「为打不出『馫蘒』二字，主流输入法就可以」的修复执行点）。
//!
//! 走 **C ABI 面**（与 Android 的真实调用序列逐字同构：`opi_load(luna)` →
//! `opi_import_dict(kangxi.tsv)`（= 首启种子后的词库目录装载）→ 逐键输入 →
//! `opi_candidates(512)`（= 当前 `EngineController.fetchLimit`；2026-10-26 起 512，
//! 原 64 —— 本门禁 2026-10-17 写下时按 64，已随移动端窗口同步）。
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

/// 当前移动端候选抓取窗（Android `EngineController.fetchLimit`；2026-10-26 起 512）。
const MOBILE_FETCH_LIMIT: usize = 512;

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

    for (py, target, heads) in [("xin", "馫", ["心", "新"]), ("qiu", "蘒", ["求", "球"])] {
        for c in py.chars() {
            let (u, len) = call(std::path::Path::new(&c.to_string()));
            opi_input_key_patched(u.as_ptr(), len);
        }
        // 出口是 JSON 数组串（`texts_to_json`）——按带引号的整词断言，防子串假阳。
        let got = read(unsafe { opi_candidates(MOBILE_FETCH_LIMIT) });
        assert!(
            got.contains(&format!("\"{target}\"")),
            "打 {py} 应在手机抓取窗（前 512）内出「{target}」—— 实得前 120 字符：{:?}",
            got.chars().take(120).collect::<String>()
        );
        // 2026-10-17「抢位加成」后的对照面：**常用字头不受影响** —— 机制加成只给
        // 导入层独有条目（≤0.875×boost），主库自己的高分条目扫描不到它们。
        // 前 16 内必须找到这些头（阈值按实测取并留余量；这是旧「3.98e9 数据 hack」
        // 注释里点名的代价面 —— 回退数据后由本断言接手看住）。
        let top16: Vec<String> = got
            .trim_start_matches('[')
            .split(',')
            .take(16)
            .map(|t| t.trim_matches(['"', ']', ' ']).to_string())
            .collect();
        for h in heads {
            assert!(
                top16.iter().any(|t| t == h),
                "打 {py}：常用字头「{h}」必须在前 16 —— 实得：{top16:?}"
            );
        }
        // 证据：把前 20 打出来（跑的时候看得见 馫/蘒 落在哪一段）。
        println!(
            "[{py}] 前 20：{:?}",
            top16.iter().take(20).collect::<Vec<_>>()
        );
        println!(
            "[{py}] 馫/蘒 在窗口内 ✓（完整前 512 长度 {} 字符）",
            got.len()
        );
        unsafe { opi_clear() };
    }
}

/// `opi_input_key` 的薄封装：按 C ABI 的 UTF-16 (ptr, len) 传单字符。
fn opi_input_key_patched(ptr: *const u16, len: usize) {
    unsafe { opi_input_key(ptr, len) };
}
