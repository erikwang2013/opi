// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **方言词字可达门禁**（2026-10-17 用户裁决「只要有词库输入就要同步匹配，特别是
//! 方言词字的匹配，并组合到一起显示出来」）。
//!
//! 走 **C ABI 面**，与 Android 的真实调用序列同构（install → 写词库文件 →
//! `opi_import_dict` → 逐键 → `opi_candidates(64)`）。钉的是产品判据：
//! 导入的方言字/词**按读音打得出来**，且与自带词库的结果**同表**出现（不是分栏、
//! 不是另一种模式）。抢位的排序代数在 engine-core 侧另有三条门禁
//! （`tests/dict_layers.rs`），本文件只钉「端到端可达 + 同表」。
//!
//! 夹具读音是**本夹具自定的映射**（冇=mou / 咁=gan / 嘅=ge / 乜=mie / 唔该=wugai）：
//! 真值随方言词典来源定；本门禁钉的是通路，不是某种方言的罗马化标准。

use opi_ffi::api;

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn read(s: opi_ffi::cabi::OpiString) -> String {
    let out = if s.ptr.is_null() {
        String::new()
    } else {
        let units = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        String::from_utf16(units).expect("导出必须返回合法 UTF-16")
    };
    unsafe { opi_ffi::cabi::opi_ffi_free_string(s) };
    out
}

fn import_dict(path: &std::path::Path) -> i32 {
    let p = to_units(path.to_str().expect("临时路径是 UTF-8"));
    unsafe { opi_ffi::cabi::opi_import_dict(p.as_ptr(), p.len()) }
}

/// 敲一串按键，返回候选 JSON 解析出的词表。
fn candidates_for(keys: &str, limit: usize) -> Vec<String> {
    unsafe { opi_ffi::cabi::opi_clear() };
    for ch in keys.chars() {
        let unit = ch as u16;
        read(unsafe { opi_ffi::cabi::opi_input_key(&unit, 1) });
    }
    let json = read(unsafe { opi_ffi::cabi::opi_candidates(limit) });
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("候选应是 JSON 文本数组：{e} / {json}"))
}

/// 临时目录（与 `dict_import.rs` 同一命名纪律：进程 id 防并发撞名）。
fn temp_dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("opi_dialect_reach_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("建临时目录");
    d
}

/// 列序「词在前」（`engine-data/src/dict_text.rs` 的 cols[0]=word）。
/// 词频写真实小值（50–2000）：抢位靠机制加成而不是数据 —— 这正是本门禁的验点。
const FIXTURE: &str = "\
冇\tmou\t50
咁\tgan\t120
嘅\tge\t80
乜\tmie\t60
唔该\twugai\t2000
";

#[test]
fn imported_dialect_chars_and_words_are_reachable_and_merged() {
    api::install(None).expect("内置回退词库必装得上");

    // 前置：不带导入层时这些方言字/词**不在**候选里（否则「有/没有」两面空转）。
    for (keys, word) in [("mou", "冇"), ("ge", "嘅"), ("wugai", "唔该")] {
        assert!(
            !candidates_for(keys, 64).iter().any(|w| w == word),
            "前置：回退词库不该有「{word}」（有的话这个判据测不出东西）"
        );
    }

    let dir = temp_dir();
    let tsv = dir.join("dialect.tsv");
    std::fs::write(&tsv, FIXTURE).expect("写夹具");
    assert!(
        import_dict(&tsv) >= 5,
        "导入应至少 5 条（返回 -1 = 失败，夹具列序/读音得先查）"
    );

    // 判据一：**方言字**按读音可达（单字，数据词频只有 50–120）。
    for (keys, word) in [("mou", "冇"), ("gan", "咁"), ("ge", "嘅"), ("mie", "乜")] {
        let cands = candidates_for(keys, 64);
        assert!(
            cands.iter().any(|w| w == word),
            "打 {keys} 应出方言字「{word}」—— 实得：{cands:?}"
        );
    }

    // 判据二：**方言词**（两字）同样可达 —— 「词字」两个层面都要。
    let wugai = candidates_for("wugai", 64);
    assert!(
        wugai.iter().any(|w| w == "唔该"),
        "打 wugai 应出方言词「唔该」—— 实得：{wugai:?}"
    );

    // 判据三：**同表合并** —— 与自带词库的候选出现在同一个列表里（「ge」在回退
    // 词库里本来有候选，导入后两边同一张表），且方言字靠前可见（抢位机制，
    // 数据词频只有 80）。
    let ge = candidates_for("ge", 64);
    let pos = ge.iter().position(|w| w == "嘅");
    assert!(
        ge.len() > 1,
        "前置：`ge` 应与自带词库的候选同表（多于一条）—— 实得：{ge:?}"
    );
    assert!(
        pos.is_some_and(|p| p < 8),
        "方言字「嘅」应在前 8 内可见（机制加成抢位，数据词频仅 80）—— 实得：{ge:?}"
    );

    // 证据：把几条读数的前 12 打出来（跑的时候看得见排序）。
    for keys in ["mou", "ge", "wugai"] {
        let c = candidates_for(keys, 12);
        println!("[{keys}] 前 12：{c:?}");
    }
}
