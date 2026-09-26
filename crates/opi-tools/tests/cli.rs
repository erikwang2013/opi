// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

#[test]
fn cli_compile_roundtrip() {
    let dir = std::env::temp_dir().join(format!("opi-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tsv = dir.join("t.tsv");
    let opid = dir.join("t.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n").unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
        .args(["compile", tsv.to_str().unwrap(), opid.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // 编译成功后打印项目宠物「小欧」——整只键帽必须在 stdout 里完整出现。
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("OPI"), "小欧的刻字缺失：{stdout}");
    assert!(
        stdout.contains("╭────┴────╮"),
        "小欧的键帽正面缺失：{stdout}"
    );
    let bytes = std::fs::read(&opid).unwrap();
    let parsed = engine_data::parse(&bytes).unwrap();
    assert_eq!(parsed.entries.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 版本号必须来自 workspace.package.version（单一版本源），与发布 tag 一致。
#[test]
fn cli_version_prints_crate_version_and_pet() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with(&format!("opi-tools {}\n", env!("CARGO_PKG_VERSION"))),
        "版本行不符：{stdout}"
    );
    assert!(
        stdout.contains("│      OPI      │"),
        "版本输出应带上小欧：{stdout}"
    );
}
