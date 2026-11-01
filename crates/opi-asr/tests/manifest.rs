use opi_asr::{ModelManifest, sha256_file, verify};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("opi-asr-test-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn manifest_for(dir: &Path, role: &str) -> String {
    let path = dir.join("model.bin");
    let sha = sha256_file(&path).unwrap();
    let size = fs::metadata(&path).unwrap().len();
    json!({
        "version": 1,
        "files": [{"path": "model.bin", "size": size, "sha256": sha, "role": role}]
    })
    .to_string()
}

#[test]
fn known_sha256_vector() {
    // FIPS 180-4 test vector for "abc".
    let dir = temp_dir("sha");
    let path = dir.join("abc");
    fs::write(&path, b"abc").unwrap();
    assert_eq!(
        sha256_file(&path).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn verifies_size_hash_and_roles() {
    let dir = temp_dir("verify");
    fs::write(dir.join("model.bin"), b"hello").unwrap();
    let manifest = ModelManifest::from_json(&manifest_for(&dir, "model")).unwrap();

    let report = verify(&dir, &manifest, true).unwrap();
    assert_eq!(report.files, 1);
    assert_eq!(report.bytes, 5);
    assert_eq!(
        manifest.role_path(&dir, "model").unwrap(),
        dir.join("model.bin").canonicalize().unwrap()
    );

    // Same size, different bytes: only the hash catches it.
    fs::write(dir.join("model.bin"), b"hellx").unwrap();
    let err = verify(&dir, &manifest, true).unwrap_err().to_string();
    assert!(err.contains("sha256 mismatch"), "{err}");
    // Hash check is optional; size always counts.
    assert!(verify(&dir, &manifest, false).is_ok());
    fs::write(dir.join("model.bin"), b"hell").unwrap();
    assert!(verify(&dir, &manifest, false).is_err());

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_escape_and_missing_files() {
    let dir = temp_dir("escape");
    fs::write(dir.join("model.bin"), b"hello").unwrap();
    let outside = dir.parent().unwrap().join("opi-asr-outside.bin");
    fs::write(&outside, b"hello").unwrap();

    let mut value: serde_json::Value = serde_json::from_str(&manifest_for(&dir, "model")).unwrap();
    value["files"][0]["path"] = json!("../opi-asr-outside.bin");
    let manifest = ModelManifest::from_json(&value.to_string()).unwrap();
    let err = verify(&dir, &manifest, true).unwrap_err().to_string();
    assert!(err.contains("unsafe manifest path"), "{err}");

    value["files"][0]["path"] = json!("missing.bin");
    let manifest = ModelManifest::from_json(&value.to_string()).unwrap();
    assert!(verify(&dir, &manifest, true).is_err());

    fs::remove_file(outside).unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn manifest_requires_unique_roles() {
    let dir = temp_dir("roles");
    fs::write(dir.join("model.bin"), b"hello").unwrap();
    let sha = sha256_file(&dir.join("model.bin")).unwrap();
    let json = json!({
        "version": 1,
        "files": [
            {"path": "model.bin", "size": 5, "sha256": sha, "role": "model"},
            {"path": "model.bin", "size": 5, "sha256": sha, "role": "tokens"}
        ]
    })
    .to_string();
    assert!(ModelManifest::from_json(&json).is_err(), "duplicate path");
    fs::remove_dir_all(dir).unwrap();
}
