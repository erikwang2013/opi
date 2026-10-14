// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `opi_fcitx5_import_dict` / `opi_fcitx5_set_dict_enabled` 的行为门禁（2026-10-14，M3）。
//!
//! 只一个 `#[test]`：两条出口都踩进程级 `SINGLETON`，而 `cargo test` 默认多线程并行
//! —— 同一测试二进制里再多一条踩单例的用例就会互踩（`serial_install` 的注释记着那次
//! 「12 次连跑约 40% 随机红」的事故）。一条用完就完，不需要第二把锁。
//!
//! ⚠️ **列序是「词在前」**：真源在 `engine-data/src/dict_text.rs` 的 `cols[0] = word`
//! / `cols[1] = pinyin`，与 `data/raw/brand_terms.tsv` 一致；任务书里写的
//! 「拼音<TAB>词」是**反的**（详见 opi-ffi `tests/dict_import.rs` 同一段）。
//!
//! 测不到的：宿主 C++ 侧扫目录 / `DisabledDicts` 的编排（那边有自己的 driver）。

use std::path::{Path, PathBuf};

use crate::{OpString, opi_ffi_free_string_utf8, serial_install};

fn call_import(path: &str) -> i32 {
    unsafe { crate::opi_fcitx5_import_dict(path.as_ptr(), path.len()) }
}

fn call_set(name: &str, on: bool) -> bool {
    unsafe { crate::opi_fcitx5_set_dict_enabled(name.as_ptr(), name.len(), on) }
}

/// 读一次 + 释放一次（与 C++ 的 take() 同一条契约）。
fn take(s: OpString) -> String {
    if s.ptr.is_null() {
        return String::new();
    }
    let v = unsafe { std::slice::from_raw_parts(s.ptr, s.len) }.to_vec();
    unsafe { opi_ffi_free_string_utf8(s) };
    String::from_utf8(v).expect("出口应是 UTF-8")
}

/// 清缓冲 → 逐字符敲 → 取当前页候选。**一次一个字符**：本出口对多字符入参整体忽略。
fn candidates_for(keys: &str) -> Vec<String> {
    unsafe { crate::opi_fcitx5_clear() };
    for ch in keys.chars() {
        let s = ch.to_string();
        let _ = take(unsafe { crate::opi_fcitx5_input_key(s.as_ptr(), s.len()) });
    }
    let raw = take(unsafe { crate::opi_fcitx5_candidates(64) });
    serde_json::from_str(&raw).expect("候选出口应是 serde_json 的字符串数组")
}

fn write(path: &Path, body: &str) {
    std::fs::write(path, body).expect("写夹具");
}

#[test]
fn import_and_toggle() {
    // 本用例全程持锁（`let _g =` 绑定活到结束）；`serial_install` 已装载内置回退词库。
    let _g = serial_install();

    // ---- 未装载哨兵：两条出口各给一个 ----（先测这一组，避免后面把单例弄丢）
    {
        let mut g = crate::SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
        *g = None;
    }
    assert_eq!(
        call_import("/nonexistent.tsv"),
        -1,
        "未装载 → import 哨兵 -1"
    );
    assert!(!call_set("whatever", true), "未装载 → set 哨兵 false");
    crate::install(None).expect("恢复内置回退词库");

    // ---- 夹具：一个文件目录（词库名 = 文件名词干，故文件名就是名字） ----
    let dir: PathBuf = std::env::temp_dir().join(format!("opi_fx5_dict_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // 上次残骸（同 pid 不可能，防御）
    std::fs::create_dir_all(&dir).expect("建临时目录");
    let tsv = dir.join("smoke.tsv");
    // 词在前；频次写满档把内置词压下去，保证它排进前 64 名。
    write(&tsv, "测词\tce\t999999999\n这行没有TAB是坏行\n");

    // ---- 导入：坏行如实跳过但**不影响条数**（1 条有效） ----
    assert_eq!(
        call_import(&tsv.to_string_lossy()),
        1,
        "坏行应被跳过、返回 1"
    );

    // 「词真进了引擎」—— 出口返回条数不等于引擎里有这个词。
    let cands = candidates_for("ce");
    assert!(
        cands.iter().any(|c| c == "测词"),
        "导入层的词应出现在 `ce` 的候选里（词没进引擎？）：{cands:?}"
    );

    // ---- 失败路径一律 -1，且既有导入层**不动** ----
    let bad = dir.join("bad.tsv");
    write(&bad, "全是坏行\n另一行也坏\n");
    assert_eq!(call_import(&bad.to_string_lossy()), -1, "全坏行 → -1");
    let empty = dir.join("empty.tsv");
    write(&empty, "");
    assert_eq!(call_import(&empty.to_string_lossy()), -1, "空文件 → -1");
    // 全坏 / 空文件都不许悄悄建出一层空壳把名字占了
    assert!(!call_set("bad", false), "被拒的导入不应留下名字");
    assert_eq!(
        call_import(&dir.join("missing.tsv").to_string_lossy()),
        -1,
        "读不到 → -1"
    );
    assert_eq!(call_import(""), -1, "空路径 → -1");
    let nul = unsafe { crate::opi_fcitx5_import_dict(std::ptr::null(), 0) };
    assert_eq!(nul, -1, "nullptr → -1");
    // 失败之后既有层仍在（上面那条「词在候选里」的断言此时仍成立）
    assert!(
        candidates_for("ce").iter().any(|c| c == "测词"),
        "失败路径不该动既有导入层"
    );

    // ---- 同名重导入 = 原位覆盖（不叠第二层） ----
    write(&tsv, "测词\tce\t999999999\n测词二\tce\t800000000\n");
    assert_eq!(call_import(&tsv.to_string_lossy()), 2, "重导入返回新条数");
    let cands = candidates_for("ce");
    assert!(
        cands.iter().any(|c| c == "测词二"),
        "新词应进引擎：{cands:?}"
    );

    // ---- 停用 / 启用 ----
    assert!(call_set("smoke", false), "停用已导入的词库 → true");
    assert!(
        !candidates_for("ce").iter().any(|c| c == "测词"),
        "停用后导入层的词不该再出现"
    );
    assert!(call_set("smoke", true), "再启用 → true");
    assert!(
        candidates_for("ce").iter().any(|c| c == "测词"),
        "启用后词应回来"
    );
    assert!(call_set("smoke", true), "已是目标态 → true（幂等）");

    // ---- 裁决 1 的执行点：**数据在内存里**，文件删了照样能再启用 ----
    std::fs::remove_file(&tsv).expect("删词库文件");
    assert!(call_set("smoke", false), "文件不在也能停用");
    assert!(
        call_set("smoke", true),
        "**文件不在也能再启用**（数据留内存）"
    );
    assert!(
        candidates_for("ce").iter().any(|c| c == "测词"),
        "文件删了但内存里的数据应当还在"
    );
    // 文件真不在时重新导入才是 -1（与上面那条对照，证明删除真的生效了）
    assert_eq!(
        call_import(&tsv.to_string_lossy()),
        -1,
        "文件已删 → 重导入 -1"
    );

    // ---- 系统词典不进这一层：两个方向都 false（不是「已是目标态」） ----
    for name in ["luna", "trad", "stroke"] {
        assert!(!call_set(name, true), "系统词典 {name} 不接受启用");
        assert!(!call_set(name, false), "系统词典 {name} 不接受停用");
    }
    assert!(!call_set("没这个名字", false), "认不得的名字 → false");

    let _ = std::fs::remove_dir_all(&dir);
}
