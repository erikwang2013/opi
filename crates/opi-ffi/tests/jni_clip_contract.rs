// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **opi-clip 的 13 条 JNI 签名单独钉死**（`jni.rs` 同一张 RegisterNatives 表）。
//!
//! `JNI_CLIP` 已并入 `JNI_ALL`，整表 ↔ 活文件 `OpiEngine.kt` 的集合相等由
//! `jni_contract.rs` 负责。本文件额外做两件事：
//! 1. 把这一族的 13 条**同名同描述符**独立钉住（[`FIX_KT`] 是期望形态，
//!    即使活 Kotlin 被误改，这里也能指出「计划签名」是什么）；
//! 2. 证明 `JNI_OnLoad` 真的调用了 `clip_native_methods()`，且这 13 条没有
//!    塞进 `jni.rs` 里那张大字面量表（保持文件行数、按族 extend）。

#[allow(dead_code)]
#[path = "jni/checker.rs"]
mod checker;

use checker::*;

/// 宿主补齐时的目标声明（方法名/签名以此为准；改这里 = 改 Kotlin 契约）。
const FIX_KT: &str = r#"
object OpiEngineClipPlan {
    external fun clipCreate(configJson: String?): Long
    external fun clipDestroy(handle: Long)
    external fun clipStart(handle: Long): Boolean
    external fun clipStop(handle: Long)
    external fun clipLocalText(handle: Long, text: String?): Int
    external fun clipPollEvent(handle: Long, timeoutMs: Int): String?
    external fun clipPairStart(handle: Long, peerId: String): Boolean
    external fun clipPairConfirm(handle: Long, peerId: String, accept: Boolean): Boolean
    external fun clipPairCancel(handle: Long, peerId: String): Boolean
    external fun clipPeers(handle: Long): String?
    external fun clipForgetPeer(handle: Long, peerId: String): Boolean
    external fun clipStatus(handle: Long): String?
    external fun clipLastError(): String
}
"#;

#[test]
fn clip_registration_parser_is_not_blind() {
    let src = read(JNI_CLIP);
    let reg = parse_rust_registrations(&src).expect("解析 jni_clip.rs 注册表");
    assert_eq!(reg.len(), 13, "clip 注册表应为 13 条，实为 {}", reg.len());

    for (name, desc) in [
        ("clipCreate", "(Ljava/lang/String;)J"),
        ("clipPollEvent", "(JI)Ljava/lang/String;"),
        ("clipPairConfirm", "(JLjava/lang/String;Z)Z"),
        ("clipLastError", "()Ljava/lang/String;"),
    ] {
        let got = reg.iter().find(|m| m.name == name);
        assert_eq!(
            got.map(|m| m.desc.as_str()),
            Some(desc),
            "解析器没解析出锚点 `{name}` / `{desc}`（解析器坏了，不是契约坏了）"
        );
    }

    // 本文件没有 JNI_OnLoad，故 no_mangle 条数应恰好等于注册条数。
    let no_mangle = count_no_mangle(&src);
    assert_eq!(
        no_mangle,
        reg.len(),
        "jni_clip.rs 的 #[unsafe(no_mangle)] 有 {no_mangle} 条，注册表解析出 {} 条",
        reg.len()
    );
}

#[test]
fn clip_registration_matches_planned_kotlin() {
    let reg = parse_rust_registrations(&read(JNI_CLIP)).expect("解析 jni_clip.rs");
    let host = parse_kotlin_externals(FIX_KT).expect("解析计划声明");
    assert_eq!(host.len(), 13, "计划声明应为 13 条");
    let diffs = check_exact(&reg, &host, "计划中的 OpiEngine.kt");
    assert!(
        diffs.is_empty(),
        "jni_clip.rs 与计划 Kotlin 声明漂移（{} 条）：\n  {}",
        diffs.len(),
        report(diffs)
    );
}

#[test]
fn clip_kotlin_signature_mutant_is_reported() {
    let reg = parse_rust_registrations(&read(JNI_CLIP)).expect("解析 jni_clip.rs");
    let mutant = FIX_KT.replace("timeoutMs: Int", "timeoutMs: Long");
    assert_ne!(mutant, FIX_KT, "变异没生效");
    let host = parse_kotlin_externals(&mutant).expect("解析变异体");
    let diffs = check_exact(&reg, &host, "变异 OpiEngine.kt");
    assert_eq!(diffs.len(), 1, "应恰好报 1 条：{diffs:?}");
    assert!(diffs[0].contains("clipPollEvent"), "{}", diffs[0]);
}

/// 注册必须真的发生在 `JNI_OnLoad` 路径上；13 条按族 extend，不塞进大字面量表。
#[test]
fn jni_onload_registers_clip_table() {
    let jni_rs = read(JNI_RS);
    assert!(
        jni_rs.contains("jni_clip::clip_native_methods()"),
        "jni.rs 没有调用 clip_native_methods()：这批出口永远不会注册"
    );
    let engine_reg = parse_rust_registrations(&jni_rs).expect("解析 jni.rs 引擎表");
    assert!(
        engine_reg.iter().all(|m| !m.name.starts_with("clip")),
        "clip 方法塞进了 jni.rs 的大字面量表：应按族 extend（保持 500 行门禁）"
    );
    assert!(
        JNI_ALL.contains(&JNI_CLIP),
        "JNI_CLIP 应并入 JNI_ALL（OpiEngine.kt 已声明这 13 条，整表门禁必须看见）"
    );
}
