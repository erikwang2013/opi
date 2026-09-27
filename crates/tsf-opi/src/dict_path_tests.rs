// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `dict_path` 的路径解析 + 回退语义测试。本机（Linux）可真跑：被测代码平台中立，
//! 全部输入由参数给（唯一读环境的 `env_dict_path` 用 `ENV_LOCK` 串行）。

use super::*;
use engine_core::dictionary::Dictionary;
use engine_data::format::{OpDict, RawEntry, serialize};
use std::fs;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// 环境变量互斥：`set_var`/`remove_var` 是全局副作用，串行防并行测试竞争
/// （对照 fcitx5-opi `data_dir.rs` 的同名锁）。
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// 唯一临时目录，`Drop` 时清理（失败路径也不残留）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "opi-tsf-dict-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir); // 清理上次遗留
        fs::create_dir_all(&dir).expect("建临时目录");
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 合法词库字节：`ceshi → 测试`。该词**不在**内置 35 词里（`data/raw/fallback.tsv`），
/// 故 `query("ceshi")` 命中即证明装的是真词库、不是回退。
fn real_dict_bytes() -> Vec<u8> {
    serialize(&OpDict {
        entries: vec![RawEntry {
            pinyin: "ceshi".into(),
            word: "测试".into(),
            freq: 5000,
        }],
        pinyin_total: 5,
    })
}

/// 真词库文件写进 `dir`，返回路径。
fn write_dict(dir: &Path) -> PathBuf {
    let p = dir.join(DICT_FILE_NAME);
    fs::write(&p, real_dict_bytes()).expect("写词库");
    p
}

/// 头对、体坏（校验和不匹配）→ 走 `LoadError::Format`，即"损坏"而非"不存在"。
fn corrupt_dict_bytes() -> Vec<u8> {
    let mut b = real_dict_bytes();
    b[20] ^= 0xFF; // 与 engine-data loader.rs 的 corrupt_file_rejected 同一手法
    b
}

/// 断言装的是真词库（内容可查），兜底走 `Dictionary` 而不是只看长度。
fn assert_real_dict(load: &DictLoad) {
    let hits = load.dict.query("ceshi", 8);
    assert_eq!(
        hits.first().map(|e| e.word.as_str()),
        Some("测试"),
        "装上的应是真词库内容"
    );
}

// ---------- candidates：纯函数 ----------

#[test]
fn candidates_priority_is_env_then_dll_then_local_app_data() {
    let dll = Path::new("/dll/dir");
    let got: Vec<(PathBuf, Origin)> = candidates(
        Some(OsStr::new("/env/luna.opid")),
        Some(dll),
        Some(OsStr::new("/lad")),
    )
    .into_iter()
    .map(|c| (c.path, c.origin))
    .collect();
    assert_eq!(
        got,
        vec![
            (PathBuf::from("/env/luna.opid"), Origin::Env),
            (dll.join(DICT_FILE_NAME), Origin::DllDir),
            (
                PathBuf::from("/lad").join("opi").join(DICT_FILE_NAME),
                Origin::LocalAppData
            ),
        ]
    );
}

#[test]
fn candidates_env_is_taken_verbatim_not_as_a_dir() {
    // 环境变量给的是**文件全路径**：不追加 DICT_FILE_NAME（给它一个别的名字也照用，
    // 排障时才能指向任意构建产物）。
    let c = candidates(Some(OsStr::new("/tmp/whatever.opid")), None, None);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].path, PathBuf::from("/tmp/whatever.opid"));
}

#[test]
fn candidates_empty_env_equals_unset() {
    // 空串等同未设置（XDG 轨惯例；环境变量被 `set VAR=` 清空是常见操作）
    let c = candidates(Some(OsStr::new("")), Some(Path::new("/dll")), None);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].origin, Origin::DllDir);
}

#[test]
fn candidates_drop_absent_inputs_and_empty_all() {
    let c = candidates(None, None, Some(OsStr::new("/lad")));
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].origin, Origin::LocalAppData);
    assert!(candidates(None, None, None).is_empty());
}

// ---------- env_dict_path：唯一读环境的一处 ----------

#[test]
fn env_dict_path_reads_env_empty_is_none() {
    let _g = ENV_LOCK.lock().unwrap();
    // Safety: edition 2024 起 set_var/remove_var 为 unsafe；ENV_LOCK 串行保护
    unsafe { std::env::remove_var(DICT_ENV_VAR) };
    assert_eq!(env_dict_path(), None, "未设置 → None");
    unsafe { std::env::set_var(DICT_ENV_VAR, "") };
    assert_eq!(env_dict_path(), None, "空串等同未设置");
    unsafe { std::env::set_var(DICT_ENV_VAR, "/tmp/x.opid") };
    assert_eq!(env_dict_path(), Some(OsString::from("/tmp/x.opid")));
    unsafe { std::env::remove_var(DICT_ENV_VAR) };
}

// ---------- load_dict：走候选 ----------

#[test]
fn env_candidate_wins_and_loads_real_dict() {
    let env_dir = TempDir::new("envwin");
    let dll_dir = TempDir::new("dllwin");
    let env = write_dict(env_dir.path());
    write_dict(dll_dir.path());
    let r = load_dict(Some(env.as_os_str()), Some(dll_dir.path()), None);
    assert_eq!(r.source.as_ref().map(|c| c.path.clone()), Some(env));
    assert_eq!(r.source.as_ref().map(|c| c.origin), Some(Origin::Env));
    assert!(r.notes.is_empty(), "成功装载不记流水账：{:?}", r.notes);
    assert_real_dict(&r);
}

#[test]
fn local_app_data_candidate_loads_when_others_absent() {
    let lad = TempDir::new("lad");
    // 词库在 `%LOCALAPPDATA%\opi\`（小写 opi 子目录），不是 LOCALAPPDATA 根下。
    let opi_dir = lad.path().join("opi");
    fs::create_dir(&opi_dir).expect("建 opi 子目录");
    let p = write_dict(&opi_dir);
    let r = load_dict(None, None, Some(lad.path().as_os_str()));
    assert_eq!(
        r.source.as_ref().map(|c| c.origin),
        Some(Origin::LocalAppData)
    );
    assert_eq!(r.source.as_ref().map(|c| c.path.clone()), Some(p));
    assert_real_dict(&r);
}

#[test]
fn missing_env_file_falls_through_to_dll_dir() {
    let env_dir = TempDir::new("envmiss");
    let dll_dir = TempDir::new("dllok");
    let missing = env_dir.path().join("not-there.opid");
    let dll = write_dict(dll_dir.path());
    let r = load_dict(Some(missing.as_os_str()), Some(dll_dir.path()), None);
    assert_eq!(r.source.as_ref().map(|c| c.path.clone()), Some(dll));
    assert_eq!(r.notes.len(), 1, "只该记缺失那一条：{:?}", r.notes);
    assert!(
        r.notes[0].contains("不存在"),
        "缺失要记成「不存在」：{:?}",
        r.notes
    );
    assert_real_dict(&r);
}

#[test]
fn corrupt_env_file_is_noted_and_falls_through() {
    // 损坏 ≠ 不存在：破坏的文件必须留下一条原因，且**不能**因此让用户没词库。
    let env_dir = TempDir::new("envbad");
    let dll_dir = TempDir::new("dllok2");
    let bad = env_dir.path().join(DICT_FILE_NAME);
    fs::write(&bad, corrupt_dict_bytes()).expect("写坏词库");
    let dll = write_dict(dll_dir.path());
    let r = load_dict(Some(bad.as_os_str()), Some(dll_dir.path()), None);
    assert_eq!(r.source.as_ref().map(|c| c.path.clone()), Some(dll));
    assert_eq!(r.notes.len(), 1, "坏的候选要留一条：{:?}", r.notes);
    let note = &r.notes[0];
    assert!(note.contains("不可用"), "坏要记成「不可用」：{note}");
    assert!(note.contains(DICT_FILE_NAME), "原因里要带路径：{note}");
    assert!(!note.contains("不存在"), "坏不是「不存在」：{note}");
    assert_real_dict(&r);
}

#[test]
fn existing_directory_as_candidate_is_not_found_but_noted() {
    // 存在但是个目录（或不可读）→ 不是「不存在」那条静默路径，要留原因再往下走。
    let env_dir = TempDir::new("envdir");
    let dll_dir = TempDir::new("dllok3");
    let dir_as_env = env_dir.path().join("a-directory.opid");
    fs::create_dir(&dir_as_env).expect("建目录");
    let dll = write_dict(dll_dir.path());
    let r = load_dict(Some(dir_as_env.as_os_str()), Some(dll_dir.path()), None);
    assert_eq!(r.source.as_ref().map(|c| c.path.clone()), Some(dll));
    assert_eq!(r.notes.len(), 1, "{:?}", r.notes);
    assert!(r.notes[0].contains("不可用"), "{:?}", r.notes);
}

#[test]
fn all_candidates_fail_uses_builtin_and_notes_each() {
    let lad = TempDir::new("ladempty"); // 目录存在但里面没有词库
    let n = load_dict(
        None,
        Some(Path::new("/nonexistent/dll")),
        Some(lad.path().as_os_str()),
    );
    assert!(n.source.is_none(), "全败 → 回退");
    assert_eq!(n.notes.len(), 2, "每个候选各一条：{:?}", n.notes);
    // 回退的是内置 35 词（与 engine-data 的 fallback 同一份），不是空引擎。
    assert_eq!(n.dict.len(), engine_data::fallback_dict().len());
    assert!(!n.dict.is_empty() && n.dict.query("ceshi", 8).is_empty());
}

#[test]
fn corrupt_single_candidate_falls_back_to_builtin() {
    // 最坏情况：唯一候选是坏的 —— 回退（不 panic、不空引擎），且原因留证。
    let dir = TempDir::new("onlybad");
    let bad = dir.path().join(DICT_FILE_NAME);
    fs::write(&bad, corrupt_dict_bytes()).expect("写坏词库");
    let r = load_dict(Some(bad.as_os_str()), None, None);
    assert!(r.source.is_none());
    assert_eq!(r.dict.len(), engine_data::fallback_dict().len());
    assert!(r.notes[0].contains("不可用"), "{:?}", r.notes);
}

// ---------- log_line：唯一一处日志格式（放本机测，Windows 侧只有 eprintln） ----------

#[test]
fn log_line_names_source_and_path() {
    let dir = TempDir::new("logok");
    let p = write_dict(dir.path());
    let r = load_dict(Some(p.as_os_str()), None, None);
    let line = r.log_line();
    assert!(line.contains(&p.display().to_string()), "{line}");
    assert!(line.contains(DICT_ENV_VAR), "来源要写进日志：{line}");
    assert!(line.contains("词库"), "{line}");
}

#[test]
fn log_line_names_fallback_with_reasons() {
    let r = load_dict(None, Some(Path::new("/nonexistent/dll")), None);
    let line = r.log_line();
    assert!(line.contains("回退"), "{line}");
    assert!(line.contains("不存在"), "原因要写进日志：{line}");
    assert!(line.contains(DICT_FILE_NAME), "{line}");
}
