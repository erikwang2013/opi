// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `compile` 的两个可选尾部开关（Task 9）：词排序索引与语义小表。

use engine_data::{serialize, validate};
use opi_tools::compiler::{compile, parse_dict};
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("opi-cli-extras-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn compile_with_index_and_assoc_verifies_and_default_stays_plain() {
    let d = dir("extras");
    let tsv = d.join("t.tsv");
    let assoc = d.join("a.tsv");
    let out = d.join("t.opid");
    let plain = d.join("plain.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n今天\tjintian\t900\n").unwrap();
    std::fs::write(&assoc, "今天\t回家\t600\n今天\t晚上好\t0\n").unwrap();

    let o = run(&[
        "compile",
        "--word-index",
        "--assoc",
        assoc.to_str().unwrap(),
        tsv.to_str().unwrap(),
        out.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let bytes = std::fs::read(&out).unwrap();
    let v = validate(&bytes).unwrap();
    assert_eq!(v.flags, 0b11);
    assert!(v.word_index.is_some());
    assert_eq!(v.assoc.expect("assoc").root_count, 1);

    let verify = run(&["verify", out.to_str().unwrap()]);
    assert!(
        verify.status.success(),
        "{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let o = run(&["compile", tsv.to_str().unwrap(), plain.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let expected = serialize(&compile(parse_dict(
        &std::fs::read_to_string(&tsv).unwrap(),
    )));
    assert_eq!(
        std::fs::read(&plain).unwrap(),
        expected,
        "默认 compile 字节必须不变"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn empty_assoc_file_does_not_enable_bit1() {
    let d = dir("empty");
    let tsv = d.join("t.tsv");
    let assoc = d.join("empty.tsv");
    let out = d.join("t.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n").unwrap();
    std::fs::write(&assoc, "").unwrap();

    let o = run(&[
        "compile",
        "--word-index",
        "--assoc",
        assoc.to_str().unwrap(),
        tsv.to_str().unwrap(),
        out.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(validate(&std::fs::read(&out).unwrap()).unwrap().flags, 0b01);
    let _ = std::fs::remove_dir_all(&d);
}
