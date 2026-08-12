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
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&opid).unwrap();
    let parsed = engine_data::parse(&bytes).unwrap();
    assert_eq!(parsed.entries.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
