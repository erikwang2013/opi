// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入词库四条出口（`importDict` / `dictList` / `setDictEnabled` / `removeDict`，
//! 2026-10-14 M3）的**行为级**门禁。
//!
//! 为什么单独一个文件：本文档的用例要**自己装引擎 + 自己造词库文件**，而
//! `SINGLETON` 是进程级的 —— 与 `cabi_unloaded.rs` 同一个理由（那边是「冷进程」，
//! 这里是「自己控制装载态」）。**全部断言放在一个 `#[test]` 里**：同一个测试二进制
//! 的用例默认**并行**跑，两个用例抢同一个单例就会互相盖状态（`cabi_test.rs` 靠
//! 「每个用例自己 load 同一份词库」躲开这件事，这里躲不开 —— 导入层是累积状态）。
//!
//! 走 **C ABI** 这一面（JNI 面共用同一份 `api` 实现与同一份哨兵；它的宿主函数体
//! 另外有 `jni_smoke` 与 `consumer.c` 两处真调用）。
//!
//! 钉住的三件**非显然**事：
//! 1. **名字 = 文件名词干**，且 `dictList` 的每个字段值（条数 / 启用位 / system 位）；
//! 2. **数据留在 FFI 层内存里** —— 用例先把文件删掉再「停用 → 启用」，仍然打得出
//!    导入层的词（这是「再启用不依赖原文件还在」这条裁决的执行点）；
//! 3. **失败不改动既有导入层**（`-1` 那三条路径：读不到 / 全坏 / 一条有效条目都没有）。

use std::path::PathBuf;

use opi_ffi::api;
use opi_ffi::cabi::{
    OpiString, opi_candidates, opi_clear, opi_dict_list, opi_import_dict, opi_input_key,
    opi_remove_dict, opi_set_dict_enabled,
};

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

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

// ---- 薄封装：按 C ABI 的 UTF-16 (ptr, len) 调用四条出口 ----

fn import_dict(path: &std::path::Path) -> i32 {
    let p = to_units(path.to_str().expect("临时路径是 UTF-8"));
    unsafe { opi_import_dict(p.as_ptr(), p.len()) }
}

fn set_dict_enabled(name: &str, on: bool) -> bool {
    let n = to_units(name);
    unsafe { opi_set_dict_enabled(n.as_ptr(), n.len(), on) }
}

fn remove_dict(name: &str) -> bool {
    let n = to_units(name);
    unsafe { opi_remove_dict(n.as_ptr(), n.len()) }
}

fn dict_list() -> serde_json::Value {
    serde_json::from_str(&read(unsafe { opi_dict_list() })).expect("dictList 必须是合法 JSON 数组")
}

/// 列表里某个名字的那一项（**按键取**：JSON 键序是 serde 字典序，不许按下标）。
fn entry<'a>(list: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    let arr = list.as_array().expect("dictList 是数组");
    arr.iter()
        .find(|e| e["name"] == name)
        .unwrap_or_else(|| panic!("dictList 里应有 {name}：{list}"))
}

fn names(list: &serde_json::Value) -> Vec<String> {
    list.as_array()
        .expect("数组")
        .iter()
        .map(|e| e["name"].as_str().expect("name 是字符串").to_string())
        .collect()
}

/// 清缓冲后打一串 ASCII 键，返回 `candidates(limit)` 的文本数组。
/// （每次都从空缓冲起 —— 否则后一次调用是**追加**，候选取自累积缓冲。）
fn candidates_for(keys: &str, limit: usize) -> Vec<String> {
    unsafe { opi_clear() };
    for ch in keys.chars() {
        let unit = ch as u16;
        // 返回值必须释放：本坑的 ABI 契约是「每个返回的 OpiString 恰好 free 一次」
        // （`read` 的 from_raw_parts 释放，见它的实现）。
        read(unsafe { opi_input_key(&unit, 1) });
    }
    let json = read(unsafe { opi_candidates(limit) });
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("候选应是 JSON 文本数组：{e} / {json}"))
}

/// 临时目录（进程 id 防并发撞名；`engine-data` 的原子写同一条命名纪律）。
fn temp_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("opi_dict_import_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("建临时目录");
    d
}

/// 夹具：词库名 `smoke`，词 `测词`，pinyin `ce`（真音节，引擎切得开）—— 故意用
/// **两字新词**（回退词库 35 条里没有），好让「候选里有没有它」成为干净的判据。
///
/// ⚠️ **列序是「词在前」**（`测词<TAB>ce`）：真源在 `engine-data/src/dict_text.rs`
/// 的 `cols[0] = word` / `cols[1] = pinyin`，与 `data/raw/brand_terms.tsv`
/// （`腾讯<TAB>tengxun`）一致。M3 任务书里写的「`拼音<TAB>词<TAB>频次`」是**反的**
/// —— 照那个写，中文会落进 pinyin 槽、被 `!pinyin.is_ascii()` 整行判废。
const FIXTURE: &str = "测词\tce\t9000000\n这行没有TAB是坏行\n";
const WORD: &str = "测词";

#[test]
fn import_dict_round_trip() {
    // —— 前置：引擎装载（内置回退词库，不依赖 data/generated 的产物）——
    api::install(None).expect("内置回退词库必装得上");

    let dir = temp_dir();
    let tsv = dir.join("smoke.tsv");
    std::fs::write(&tsv, FIXTURE).expect("写夹具");

    // 前置：没有导入层时，`ce` 的候选里**没有**这个词（否则下面的「有/没有」两面
    // 都是空转 —— 「全量第 N 项存在」这条纪律）。
    let pre = candidates_for("ce", 64);
    assert!(
        !pre.is_empty(),
        "前置：回退词库对 `ce` 应本来就有候选（否则「没有它」这一档测不出什么）"
    );
    assert!(
        !pre.iter().any(|w| w == WORD),
        "前置失败：回退词库里本不该有 {WORD}（实得 {pre:?}）"
    );

    // —— [1] importDict：条目数 = 有效行数（坏行被跳过，不算失败）——
    let n = import_dict(&tsv);
    assert_eq!(n, 1, "1 条有效 + 1 行坏行 ⇒ 返回 1（不是 -1，也不是 2）");

    // —— [2] dictList：系统层 + 导入层的每个字段 ——
    let list = dict_list();
    assert_eq!(
        names(&list),
        vec!["luna", "trad", "stroke", "smoke"],
        "系统词典在前（固定序）+ 导入层按导入顺序"
    );
    let luna = entry(&list, "luna");
    assert_eq!(luna["system"], true, "luna 是系统词典");
    assert_eq!(luna["enabled"], true, "luna 就是当前主词典 ⇒ 恒启用");
    assert!(
        luna["count"].as_u64().unwrap() > 0,
        "luna 的条数是装载时刻采集的真条数：{luna}"
    );
    for sys in ["trad", "stroke"] {
        let e = entry(&list, sys);
        assert_eq!(e["system"], true, "{sys} 是系统词典");
        assert_eq!(e["enabled"], false, "本次没装 {sys} ⇒ enabled=false");
        assert_eq!(
            e["count"], 0,
            "没装 ⇒ 条数 0（不是「读不出」，是真的没这层）"
        );
    }
    let smoke = entry(&list, "smoke");
    assert_eq!(smoke["name"], "smoke", "名字 = 文件名词干");
    assert_eq!(smoke["count"], 1);
    assert_eq!(smoke["enabled"], true, "导入即启用");
    assert_eq!(smoke["system"], false);

    // —— [3] 真进了引擎：`ce` 的候选里有它 ——
    let on = candidates_for("ce", 64);
    assert!(
        on.iter().any(|w| w == WORD),
        "导入层的词应出现在候选里（实得 {on:?}）"
    );

    // —— [4] 停用 = 从引擎的额外层拿掉（候选里没有它了）——
    assert!(set_dict_enabled("smoke", false), "停用已导入的词库 → true");
    assert_eq!(entry(&dict_list(), "smoke")["enabled"], false);
    let off = candidates_for("ce", 64);
    assert!(
        !off.iter().any(|w| w == WORD),
        "停用后候选里不该再有它（实得 {off:?}）"
    );
    assert!(set_dict_enabled("smoke", false), "已是目标态 → 幂等 true");

    // —— [5] 数据留在内存：**先把文件删掉**再启用，照样打得出来 ——
    // 这一条是裁决 1 的执行点（宿主的删除键删的是文件，本层不依赖它还在）。
    std::fs::remove_file(&tsv).expect("删夹具文件");
    assert!(
        set_dict_enabled("smoke", true),
        "文件没了也要能再启用（数据在 FFI 层内存里）"
    );
    let back = candidates_for("ce", 64);
    assert!(
        back.iter().any(|w| w == WORD),
        "再启用后候选里应重新有它（实得 {back:?}）"
    );

    // —— [6] 系统词典 / 认不得的名字：一律 false（不改状态）——
    assert!(!set_dict_enabled("luna", false), "系统词典不可停用 → false");
    assert!(!set_dict_enabled("没这个名字", true), "名字认不得 → false");
    assert_eq!(
        entry(&dict_list(), "luna")["enabled"],
        true,
        "luna 不受影响"
    );

    // —— [7] 同名重导入 = 覆盖（条数跟着变，仍是启用态）——
    std::fs::write(&tsv, "测词\tce\t9000000\n测词二\tce\t8000000\n").expect("重写夹具");
    assert_eq!(import_dict(&tsv), 2, "同名重导入 = 原位覆盖，返回新条数");
    let l2 = dict_list();
    assert_eq!(entry(&l2, "smoke")["count"], 2, "覆盖后条数跟着变");
    assert_eq!(entry(&l2, "smoke")["enabled"], true, "重导入置为启用");
    assert_eq!(
        names(&l2).len(),
        4,
        "覆盖不是新增一条（实得 {:?}）",
        names(&l2)
    );

    // —— [8] removeDict：从内存删掉（文件早就不在了），双调第二次 false ——
    assert!(remove_dict("smoke"), "删已导入的词库 → true");
    assert!(!remove_dict("smoke"), "已经不在 ⇒ false");
    assert!(!remove_dict("luna"), "系统词典 ⇒ false");
    let after = dict_list();
    assert!(
        !names(&after).contains(&"smoke".to_string()),
        "删掉之后列表里没有它：{after}"
    );
    let gone = candidates_for("ce", 64);
    assert!(
        !gone.iter().any(|w| w == WORD || w == "测词二"),
        "删掉之后候选里不该再有它的词（实得 {gone:?}）"
    );

    // —— [9] 失败三条路：-1，且**不改动**既有导入层 ——
    let stale = dir.join("stale.tsv");
    std::fs::write(&stale, "残留词\tce\t100\n").expect("写第二个夹具");
    assert_eq!(import_dict(&stale), 1);
    let before_fail = dict_list();
    let missing = dir.join("does_not_exist.tsv");
    assert_eq!(import_dict(&missing), -1, "读不到 → -1");
    std::fs::write(&missing, "坏行一\n坏行二\n").expect("写全坏夹具");
    assert_eq!(import_dict(&missing), -1, "格式全坏 → -1");
    std::fs::write(&missing, "").expect("写空文件");
    assert_eq!(import_dict(&missing), -1, "一条有效条目都没有 → -1");
    assert_eq!(
        dict_list(),
        before_fail,
        "三次失败都不许改动既有导入层（连条数带启用位，逐字段相等）"
    );
    assert!(remove_dict("stale"), "收尾");
    assert_eq!(
        names(&dict_list()),
        vec!["luna", "trad", "stroke"],
        "收尾：只剩系统层"
    );
}
