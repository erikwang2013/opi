// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `api::install` 重装词库时必须**保留自定义短语表与启用位**（2026-10-23）。
//!
//! install 整体换 Engine；用户词一直有回放，短语最初漏了 —— 设置页导入/切换词库后
//! 短语会凭空消失，直到宿主下次启动重推。本文件单独一个测试（独立进程），
//! 避免与其它用全局单例的测试并行互踩。

use opi_ffi::api;

const PHRASES: &str = r#"{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"#;
const PHRASES_EN: &str = r#"{"version":1,"phrases":[{"trigger":"dz","text":"address"}]}"#;

#[test]
fn install_replays_custom_phrases_and_enabled_flag() {
    api::install(None).expect("内置回退词库必装得上");
    let n = api::with_engine(|a| a.set_custom_phrases(PHRASES.to_string()))
        .expect("引擎已装载")
        .expect("合法 JSON");
    assert_eq!(n, 1);
    api::with_engine(|a| a.set_custom_phrases_enabled(false)).expect("引擎已装载");
    // 英文套（2026-10-24）：同一安装路径也必须回放，且开关独立。
    let n_en = api::with_engine(|a| a.set_english_custom_phrases(PHRASES_EN.to_string()))
        .expect("引擎已装载")
        .expect("合法 JSON");
    assert_eq!(n_en, 1);
    api::with_engine(|a| a.set_english_custom_phrases_enabled(false)).expect("引擎已装载");

    // 重装（设置页导入词库 / 换词库走的是同一条 install）。
    api::install(None).expect("重装必成功");

    let (json, on, json_en, on_en) = api::with_engine(|a| {
        (
            a.export_custom_phrases(),
            a.custom_phrases_enabled(),
            a.export_english_custom_phrases(),
            a.english_custom_phrases_enabled(),
        )
    })
    .expect("引擎已装载");
    assert!(json.contains("地址"), "短语表必须回放：{json}");
    assert!(!on, "启用位必须回放（装前已关）：{on}");
    assert!(json_en.contains("address"), "英文套必须回放：{json_en}");
    assert!(!on_en, "英文套启用位必须回放：{on_en}");
}
