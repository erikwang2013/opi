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
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));

    let v = run(&["verify", opid.to_str().unwrap()]);
    assert!(v.status.success(), "{}", String::from_utf8_lossy(&v.stderr));
    let out = String::from_utf8_lossy(&v.stdout);
    assert!(out.contains("checksum: ok"));
    assert!(out.contains("entries: 2"));

    let mut bytes = std::fs::read(&opid).unwrap();
    bytes[20] ^= 0xFF;
    std::fs::write(&opid, &bytes).unwrap();
    let bad = run(&["verify", opid.to_str().unwrap()]);
    assert!(!bad.status.success());

    let _ = std::fs::remove_dir_all(&dir);
}
