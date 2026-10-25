// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 用户词恢复/防抖落盘/清空的门禁（`user_words.rs`）。全部用临时目录，不碰真实
//! `$XDG_DATA_HOME/opi`，无网络。

use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "opi-fcitx5-uw-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

fn state() -> CandidateState {
    CandidateState::load(None).expect("内置回退词库")
}

/// 打一个字母并回车选首候选，返回上屏文本。回退词库对 "n" 有候选。
fn pick_first(s: &mut CandidateState) -> String {
    s.input_key('n');
    let picked = s.select(0);
    assert!(!picked.is_empty(), "回退词库对 n 必须有候选");
    picked
}

fn due_later() -> Instant {
    Instant::now() + SAVE_DEBOUNCE + Duration::from_millis(1)
}

#[test]
fn missing_file_is_cold_start_and_not_created() {
    let dir = temp_dir("missing");
    let mut s = state();
    assert_eq!(attach(&mut s, &dir), Restore::Missing);
    assert!(!file_path(&dir).exists(), "冷启动不得创建文件");
    assert!(s.persist.path.is_some(), "缺失也要接线，之后才能落盘");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn valid_file_is_imported() {
    let dir = temp_dir("valid");
    let path = file_path(&dir);
    fs::write(&path, r#"{"version":1,"words":[{"text":"词甲","freq":7}]}"#).unwrap();
    let mut s = state();
    assert_eq!(attach(&mut s, &dir), Restore::Imported);
    assert!(s.engine.export_user_words().contains("词甲"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn zero_byte_file_is_reported_and_preserved() {
    let dir = temp_dir("empty");
    let path = file_path(&dir);
    fs::write(&path, "").unwrap();
    let mut s = state();
    assert_eq!(attach(&mut s, &dir), Restore::Failed);
    assert!(path.exists(), "0 字节是掉电残留证据，不得删除");
    assert_eq!(fs::metadata(&path).unwrap().len(), 0, "也不得改写");
    // 坏文件没有标脏 ⇒ 定时器不会用空表把它盖掉。
    assert!(!s.save_user_words_if_due_at(due_later()));
    assert_eq!(fs::metadata(&path).unwrap().len(), 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_file_is_reported_and_preserved() {
    let dir = temp_dir("corrupt");
    let path = file_path(&dir);
    let garbage = b"{ this is not json";
    fs::write(&path, garbage).unwrap();
    let mut s = state();
    assert_eq!(attach(&mut s, &dir), Restore::Failed);
    assert_eq!(fs::read(&path).unwrap(), garbage, "坏文件必须原样保留");
    assert!(!s.engine.export_user_words().contains("词"));
    assert!(
        !s.save_user_words_if_due_at(due_later()),
        "坏文件不得被空表覆盖"
    );
    assert_eq!(fs::read(&path).unwrap(), garbage);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unsupported_version_is_corrupt() {
    let dir = temp_dir("version");
    let path = file_path(&dir);
    let doc = r#"{"version":2,"words":[{"text":"词乙","freq":1}]}"#;
    fs::write(&path, doc).unwrap();
    let mut s = state();
    assert_eq!(attach(&mut s, &dir), Restore::Failed);
    assert_eq!(fs::read_to_string(&path).unwrap(), doc);
    assert!(!s.engine.export_user_words().contains("词乙"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn typing_without_selection_does_not_save() {
    let dir = temp_dir("typing");
    let mut s = state();
    assert_eq!(attach(&mut s, &dir), Restore::Missing);
    for ch in "nihao".chars() {
        let out = s.input_key(ch);
        assert!(out.is_empty(), "只打字不应上屏（{ch}）");
    }
    assert!(!s.persist.dirty, "未选词不得标脏（不能每键写文件）");
    assert!(!s.save_user_words_if_due_at(due_later()));
    assert!(!file_path(&dir).exists(), "整个打字过程不得创建文件");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn selection_saves_only_after_debounce_and_roundtrips() {
    let dir = temp_dir("roundtrip");
    let mut s = state();
    assert_eq!(attach(&mut s, &dir), Restore::Missing);
    let picked = pick_first(&mut s);
    assert!(s.persist.dirty, "成功选词必须标脏");
    assert!(
        !s.save_user_words_if_due_at(Instant::now()),
        "防抖未到不得写盘（不能每键写文件）"
    );
    assert!(!file_path(&dir).exists());

    assert!(s.save_user_words_if_due_at(due_later()), "到点必须尝试落盘");
    assert!(!s.persist.dirty);
    let path = file_path(&dir);
    assert!(path.exists());
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        1,
        "成功后目录里只应有目标文件（无 tmp 残骸）"
    );

    // 重启：新状态从同一文件恢复，刚学的词必须在。
    let mut restarted = state();
    assert_eq!(attach(&mut restarted, &dir), Restore::Imported);
    assert!(restarted.engine.export_user_words().contains(&picked));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn clear_deletes_file_memory_and_pending_save() {
    let dir = temp_dir("clear");
    let mut s = state();
    attach(&mut s, &dir);
    let picked = pick_first(&mut s);
    assert!(s.save_user_words_if_due_at(due_later()));
    assert!(file_path(&dir).exists());
    assert!(s.engine.export_user_words().contains(&picked));

    s.clear_user_words().expect("文件存在时清空应成功");
    assert!(!file_path(&dir).exists(), "清空必须删文件，否则重启复活");
    assert!(!s.engine.export_user_words().contains(&picked));
    assert_eq!(s.page, 0);
    assert!(
        !s.save_user_words_if_due_at(due_later()),
        "清空后没有待写内容"
    );
    assert!(!file_path(&dir).exists(), "定时器不得把空表写回来");
    // 已删除后再清一次是幂等成功。
    s.clear_user_words().expect("文件已不存在也应成功");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn clear_before_first_save_leaves_no_file() {
    let dir = temp_dir("clear-dirty");
    let mut s = state();
    attach(&mut s, &dir);
    pick_first(&mut s);
    assert!(s.persist.dirty);
    s.clear_user_words().expect("无文件可删也算成功");
    assert!(!s.persist.dirty, "清空必须取消待落盘");
    assert!(!s.save_user_words_if_due_at(due_later()));
    assert!(!file_path(&dir).exists(), "待落盘不得在清空后复活文件");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn save_creates_missing_parent_directory() {
    let dir = temp_dir("nested");
    let nested = dir.join("not-yet");
    let mut s = state();
    attach(&mut s, &nested);
    pick_first(&mut s);
    assert!(s.save_user_words_if_due_at(due_later()));
    assert!(nested.join(FILE_NAME).exists(), "父目录由落盘侧创建");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn write_failure_keeps_dirty_for_retry() {
    let dir = temp_dir("write-fail");
    let mut s = state();
    attach(&mut s, &dir);
    pick_first(&mut s);
    // 让父目录不可创建：同路径放一个普通文件。
    fs::remove_dir_all(&dir).unwrap();
    fs::write(&dir, b"x").unwrap();
    assert!(
        !s.save_user_words_if_due_at(due_later()),
        "写不进去应返回 false"
    );
    assert!(s.persist.dirty, "失败必须保留 dirty，等下次重试");
    let _ = fs::remove_file(&dir);
}

#[test]
fn unconnected_state_never_touches_files() {
    let mut s = state();
    let picked = pick_first(&mut s);
    assert!(!s.persist.dirty, "未接线不标脏");
    assert!(!s.save_user_words_if_due_at(due_later()));
    s.clear_user_words().expect("未接线时清空内存也应成功");
    assert!(!s.engine.export_user_words().contains(&picked));
}
