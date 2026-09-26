// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

#[test]
fn cli_verify_ok_and_rejects_corruption() {
    let dir = std::env::temp_dir().join(format!("opi-verify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tsv = dir.join("t.tsv");
    let opid = dir.join("t.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n").unwrap();

    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
            .args(args)
            .output()
            .unwrap()
    };

    let ok = run(&["compile", tsv.to_str().unwrap(), opid.to_str().unwrap()]);
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    let v = run(&["verify", opid.to_str().unwrap()]);
    assert!(v.status.success(), "{}", String::from_utf8_lossy(&v.stderr));
    let out = String::from_utf8_lossy(&v.stdout);
    assert!(out.contains("checksum: ok"));
    assert!(out.contains("entries: 2"));
    // 校验成功也打印小欧（与 compile / --version 一致），别只测一半路径。
    assert!(
        out.contains("│      OPI      │"),
        "校验成功应带上小欧：{out}"
    );

    let mut bytes = std::fs::read(&opid).unwrap();
    bytes[20] ^= 0xFF;
    std::fs::write(&opid, &bytes).unwrap();
    let bad = run(&["verify", opid.to_str().unwrap()]);
    assert!(!bad.status.success());
    // 失败路径不摆小欧：报错旁边站一只笑呵呵的键帽，读起来像在嘲弄用户。
    assert!(
        !String::from_utf8_lossy(&bad.stdout).contains("╭"),
        "失败路径不该打印小欧"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
