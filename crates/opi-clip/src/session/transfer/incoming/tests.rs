// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use super::*;

fn info(path: &str, size: u64, is_dir: bool) -> TransferFileInfo {
    TransferFileInfo {
        relative_path: path.into(),
        size,
        mime: String::new(),
        is_dir,
    }
}

fn incoming(dir: &Path, files: Vec<TransferFileInfo>) -> Incoming {
    let total = files.iter().map(|file| file.size).sum();
    let parts = prepare(&files, dir).unwrap();
    Incoming {
        id: TransferId::from_bytes([1; 16]),
        files,
        save_dir: dir.to_path_buf(),
        parts,
        total,
        received: 0,
        current: 0,
        offset: 0,
        file: None,
        hasher: Sha256::new(),
        last_progress: Instant::now(),
        last_progress_bytes: 0,
    }
}

#[test]
fn chunks_write_part_files_and_done_renames() {
    let dir = crate::config::temp_dir("incoming");
    let mut inc = incoming(&dir, vec![info("a.txt", 5, false), info("sub", 0, true)]);
    inc.apply_chunk(0, 0, b"hello").unwrap();
    assert!(inc.all_files_done());
    assert_eq!(fs::read(dir.join("a.txt.part")).unwrap(), b"hello");
    assert!(dir.join("sub").is_dir());
    inc.file = None;
    finish_incoming(&inc).unwrap();
    assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"hello");
    assert!(!dir.join("a.txt.part").exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn bad_offsets_sizes_and_empty_files_are_rejected() {
    let dir = crate::config::temp_dir("incoming-bad");
    let mut inc = incoming(
        &dir,
        vec![info("empty.txt", 0, false), info("a.txt", 3, false)],
    );
    // 空文件不允许收块，必须先跳过。
    assert!(inc.apply_chunk(0, 0, b"x").is_err());
    // 正确：直接发给 a.txt（索引 1），apply_chunk 会跳过空文件。
    assert!(inc.apply_chunk(1, 1, b"x").is_err(), "offset 必须从 0 开始");
    inc.apply_chunk(1, 0, b"abc").unwrap();
    assert!(inc.all_files_done());
    cleanup(&inc);
    assert!(!dir.join("a.txt.part").exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn prepare_rejects_traversal_paths() {
    let dir = crate::config::temp_dir("incoming-traversal");
    assert!(prepare(&[info("../escape", 1, false)], &dir).is_err());
    let _ = fs::remove_dir_all(dir);
}
