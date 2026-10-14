// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `extra_dicts.rs` 的主机单测（Linux 可跑 —— 正是把判定从 Windows 胶水里搬出来的理由）。
//!
//! 两条腿：**名单解析 + 装载判定**（纯函数级，夹具 = 临时目录里的真文件）与**端到端**
//! （走 `TsfLogic::apply_settings` 这唯一入口，断言词真的进了引擎的候选）。
//! 后者是承重的：前一条只证"算出了该装哪些"，不证"引擎真收到了"。

use super::*;

/// 一次性临时目录（每个用例一个名字，避免并发互踩）。
fn tmpdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("opi-extra-dicts-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 写一个词库文件（列序 = **词<TAB>拼音[<TAB>频次]**，即引擎解析器的顺序）。
fn write(dir: &std::path::Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).unwrap();
}

#[test]
fn parse_disabled_splits_trims_and_drops_empties() {
    // 与 Dicts.kt 的 parseDisabledDicts、opi_dicts.h 的 opiParseDisabledDicts 同口径。
    assert_eq!(
        parse_disabled(" a , b ,,  c  "),
        vec!["a", "b", "c"],
        "逗号切 + 去空白 + 丢空段"
    );
    assert!(parse_disabled("").is_empty(), "空串 = 空表 = 全部启用");
    assert!(
        parse_disabled(" , , ").is_empty(),
        "只有分隔符/空白也 = 全启用"
    );
    assert_eq!(parse_disabled("a,b"), vec!["a", "b"], "正常形态原样");
}

#[test]
fn loads_every_tsv_and_only_tsv_regular_files() {
    let d = tmpdir("scan");
    write(&d, "one.tsv", "测试\tceshi\t100\n");
    write(&d, "two.tsv", "好词\thao\t100\n坏行没有TAB\n");
    write(&d, "notes.txt", "不是词库\n");
    std::fs::create_dir_all(d.join("sub.tsv")).unwrap(); // 目录名叫 *.tsv：不是普通文件

    let r = load(&d, "");
    let names: Vec<&str> = r.dicts.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["one", "two"],
        "只收 *.tsv 的普通文件，按名字排序"
    );
    assert_eq!(r.dicts[0].1.len(), 1, "one 有 1 条");
    assert_eq!(r.dicts[1].1.len(), 1, "two 的好行进了库，坏行没进");
    assert_eq!(r.lines.len(), 2, "一行日志一个词库");
    assert!(r.lines[0].contains("one") && r.lines[0].contains("已装载 1 条"));
    assert!(
        r.lines[1].contains("跳过 1 行坏行"),
        "坏行如实报数（本仓的坏词库静默失效前车之鉴），实际：{}",
        r.lines[1]
    );

    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn disabled_names_are_skipped_quietly() {
    let d = tmpdir("disabled");
    write(&d, "one.tsv", "测试\tceshi\t100\n");
    write(&d, "two.tsv", "好词\thao\t100\n");

    let r = load(&d, " one , 没这个库 "); // 名单里有目录里没有的名字：不算错
    let names: Vec<&str> = r.dicts.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["two"], "停用的不装，别的照装");
    assert_eq!(r.lines.len(), 1, "停用不记日志（按名单跳过不是错误）");

    let r = load(&d, ""); // 名单空 = 全部启用
    assert_eq!(r.dicts.len(), 2, "名单去掉 ⇒ 自己回来（名单是唯一真源）");

    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn failures_are_reported_not_swallowed() {
    let d = tmpdir("fail");
    write(&d, "bad.tsv", "全是坏行\n另一行也坏\n"); // 一条都解析不出 ⇒ Err
    write(&d, "empty.tsv", ""); // 空文件 ⇒ Ok 但 0 条 ⇒ 按「没装上」处理
    write(&d, "ok.tsv", "好词\thao\t100\n");

    let r = load(&d, "");
    let names: Vec<&str> = r.dicts.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["ok"], "坏的两个不装，好的照装");
    let all = r.lines.join("\n");
    assert!(
        r.lines
            .iter()
            .any(|l| l.contains("bad") && l.contains("未装载")),
        "全坏行：带原因报出来，实际：{all}"
    );
    assert!(
        r.lines
            .iter()
            .any(|l| l.contains("empty") && l.contains("没有一条有效条目")),
        "空壳层按未装载处理（与 fcitx5/opi-ffi 的 count == 0 同裁决），实际：{all}"
    );

    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn missing_dir_is_not_an_error() {
    let r = load(std::path::Path::new("/nonexistent/opi/dicts"), "");
    assert!(
        r.dicts.is_empty() && r.lines.is_empty(),
        "目录不在 = 空报告、零噪音"
    );
}

/// 端到端：走**唯一入口** `apply_settings`（服务创建 / 每次 Activate 的那条），
/// 断言词真的进了引擎候选；再把名字写进 `DisabledDicts` ⇒ 下次套用后词消失。
#[test]
fn apply_settings_loads_and_disables_through_the_engine() {
    use crate::logic::TsfLogic;
    let d = tmpdir("e2e");
    write(&d, "smoke.tsv", "测词\tce\t999999999\n");

    let mut s = TsfLogic::load(None).expect("内置回退词库");
    let candidates = |s: &mut TsfLogic| {
        s.clear();
        for c in ['c', 'e'] {
            s.input_key(c as u32, 0);
        }
        s.candidates()
    };
    assert!(
        !candidates(&mut s).iter().any(|w| w == "测词"),
        "前置：没套设置前引擎里没有这个词（否则下面的断言没有分辨力）"
    );

    s.apply_settings(crate::settings::Settings::default(), &d);
    assert!(
        candidates(&mut s).iter().any(|w| w == "测词"),
        "默认名单（空 = 全部启用）⇒ 词库真进了引擎的候选"
    );

    let disabled = crate::settings::Settings {
        disabled_dicts: "smoke".to_string(),
        ..Default::default()
    };
    s.apply_settings(disabled, &d);
    assert!(
        !candidates(&mut s).iter().any(|w| w == "测词"),
        "名字进 DisabledDicts ⇒ 下次套用后这层不在引擎里"
    );

    // 名单清空 ⇒ 自己回来（零状态重算，不是"停用一次就永久停用"）。
    s.apply_settings(crate::settings::Settings::default(), &d);
    assert!(
        candidates(&mut s).iter().any(|w| w == "测词"),
        "名单清空 ⇒ 回来"
    );

    // 目录里没有的名单项（用户删了文件但名单还留着）：不报错、别的层不受影响。
    let stale = crate::settings::Settings {
        disabled_dicts: "smoke, 没这个库".to_string(),
        ..Default::default()
    };
    s.apply_settings(stale, &d);
    assert!(
        !candidates(&mut s).iter().any(|w| w == "测词"),
        "smoke 仍被停用"
    );

    let _ = std::fs::remove_dir_all(&d);
}
