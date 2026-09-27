// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 用户词文档（自造词 + 频次）的落盘与读取 —— 原子写 + 三条失败语义。
//!
//! **为什么在 engine-data 而不在 engine-core**：`engine-core` 无 IO、无平台依赖是本仓库
//! 的不变量（README 与架构图明文，Cargo.toml 也是零 IO 依赖）—— 引擎要在任意目标
//! （Apple / 鸿蒙 / 宿主单测）上可链。文件 IO 归本 crate。
//!
//! **本模块只收发字符串，不认识 `Learner`**：解析、版本判定、合并全在
//! `engine_core::learner`，与 Android 把 IO 放 `UserWordStore.kt`（引擎外）、引擎只
//! 收发 JSON 的先例同构。调用方的编排是「`read_document` → `import_json`」与
//! 「`export_json` → `atomic_write`」。
//!
//! 语义对照 `android/.../UserWordStore.kt`（权威参考）的**落盘部分**：原子写 =
//! 同目录 tmp → fsync → rename。刻意**不**在本层做的两件事：**防抖**（调用方的事，
//! 对照 `SAVE_DEBOUNCE_MS`）、**创建父目录**（调用方的事，fcitx5 侧见
//! `data_dir::ensure_dirs`，缺失即 Err）。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 用户词文件名（镜像 Android `UserWordStore.FILE_NAME`）。
///
/// 放词库同目录（fcitx5 `data_dir::xdg_data_dir()`、TSF `dict_path::candidates` 的
/// `%LOCALAPPDATA%\opi`）—— 两端**共用这一个常量**，别各发明一个名字。
pub const FILE_NAME: &str = "opi_user_words.json";

/// tmp 后缀。测试据此扫残骸 —— 与实现共用同一常量，避免改名后测试**静默失效**。
pub(crate) const TMP_SUFFIX: &str = ".tmp";

/// tmp 名前缀：`<目标文件名>.`。同前缀 + 同后缀 = 「可能存在的 tmp」。
pub(crate) fn tmp_prefix_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".");
    PathBuf::from(s)
}

/// 本次落盘的 tmp 路径，**每次唯一**（pid + 进程内序号）。同目录 —— rename 必须
/// 同文件系统才原子，故 tmp 不放系统临时目录。
///
/// 唯一而非固定名：固定名在**并发落盘**时会交错写同一路径，两个写入者各写一半合成
/// 半截 tmp，再被 rename 成目标 → 用户词表被写坏；而且一个写入者的 rename 会**吃掉**
/// 另一个人的 tmp，让对方的 rename 以 ENOENT 失败（整次落盘白丢）。唯一名让两个写入者
/// 各写各的，rename 原子替换，目标只会是其中某一份**完整**快照（last-writer-wins，
/// 不产生损坏）。对照 `UserWordStore.scheduleSave` 同处的 `createTempFile` 裁决
/// （那边有两个实例各带单线程执行器，固定名同样会交叉）。
///
/// 代价：失败路径必须自己收尸（见 [`atomic_write`]），否则每失败一次留一个残骸。
fn tmp_of(path: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let mut s = tmp_prefix_of(path).into_os_string();
    s.push(format!(
        "{}.{}{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed),
        TMP_SUFFIX
    ));
    PathBuf::from(s)
}

/// 原子写一份 UTF-8 文档：写同目录 tmp → **fsync** → rename（POSIX 同目录 rename 原子替换）。
///
/// 不变式：**任何失败都不破坏既有文件** —— 写 tmp 失败时目标文件原样保留（只丢这一次
/// 写入），rename 成功即整体替换；崩在写 tmp 时残留的 tmp 不会被 [`read_document`]
/// 读成有效数据（后者只读 `path` 本身，不扫同目录）。故本函数不 panic、也不静默：
/// 失败一律返回 Err 交调用方上报（静默的落盘失败等于用户的学习结果凭空消失）。
///
/// 失败路径**自己收尸**（删掉本次 tmp）：tmp 名唯一之后「下次保存覆盖 tmp」不再成立，
/// 不清就是每失败一次多一个残骸。收尸不掩盖原始错误。
///
/// 并发：同时调用的两个写入者各写各的 tmp，目标最终是其中某一份**完整**快照
/// （last-writer-wins，见 [`tmp_of`]），不会写坏。
///
/// ponytail: 只 fsync 文件不 fsync 目录，rename 本身在掉电后可能未落盘 —— 代价是回退到
/// 上一个合法文档（不产生损坏），故不加，除非要求「重启即见最新」。
pub fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = tmp_of(path);
    // fsync 不是可选项：只 write 的话数据停在页缓存，而 rename 是元数据操作，可能先于
    // 数据落盘 —— 掉电后目标文件存在却是 0 字节（对照 UserWordStore 同处注释）。
    // `sync_all()` 即 fsync(2)。
    let written = fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(contents.as_bytes())?;
        f.sync_all()
    });
    if let Err(e) = written {
        // 清残留 tmp，但不掩盖原始错误（对照 fcitx5 `data_dir::ensure_dict`）。
        let _ = fs::remove_file(&tmp);
        return Err(format!("文档落盘失败（{}）: {e}", path.display()));
    }
    // rename 失败（目录只读/目标被换成目录）只丢这一次写入，旧文件仍在。
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "文档 rename 失败（{} → {}）: {e}",
            tmp.display(),
            path.display()
        ));
    }
    Ok(())
}

/// 读一份 UTF-8 文档。三条失败语义（**调用方据此区分，不可互换**）：
///
/// - **不存在 → `Ok(None)`**：首次运行的**正常路径**，不是错误。调用方静默即可。
/// - **存在但为空 → `Err`**：⚠️ 0 字节**不是**空文档。本模块写出的永远是合法 JSON
///   （空表也是 `{"version":1,"words":[]}`），故 0 字节只可能是「没 fsync 的 rename
///   掉电后目标存在却为 0 字节」的残留形态。若把它并进 `Ok(None)`，就恰好放过掉电残留
///   —— 用户几千条一次全没且无人知晓。
/// - **存在但读不出（权限 / 是目录 / 非 UTF-8）→ `Err`**：同样绝不静默。
///
/// ⚠️ **内容是否损坏（坏 JSON / 版本不符）本层判不了**（本模块不认识 schema），
/// 那一跳在调用方的 `Learner::import_json` —— 它同样是「全有或全无」并且**返回 Err**。
/// 调用方**必须**把它的 Err 原样上报，**不得**降级成空表：同族坑见 fcitx5 词库静默失效
/// （文件在但坏了 → 引擎完全不出字、无回退无报错），是本仓库栽过的那一类。
///
/// 坏文件**不删除也不改名**（对照 `UserWordStore.load` 的同一裁决：损坏即无可恢复信息，
/// 改名只多一次写盘、多一个失败点；留着可人工查看，下一次成功落盘会原子覆盖它）。
pub fn read_document(path: &Path) -> Result<Option<String>, String> {
    let contents = match fs::read_to_string(path) {
        Ok(s) => s,
        // 缺失 = 首次运行；其余（权限、目录、非 UTF-8）都是真错误。
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("文档读取失败（{}）: {e}", path.display())),
    };
    if contents.is_empty() {
        // 见上文：0 字节是关键判据，不能与「不存在」同判。
        return Err(format!(
            "文档为空（{}）—— 0 字节是掉电/中断残留，本模块写出的文档永不为空",
            path.display()
        ));
    }
    Ok(Some(contents))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::learner::Learner;

    /// 唯一临时目录（并行测试互不冲突），测试结束清理。
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "opi-uw-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    /// 造一个**当前形态**的 tmp（复用实现的 `tmp_of`，命名改了测试不会静默失准）。
    fn a_tmp_of(target: &Path) -> PathBuf {
        tmp_of(target)
    }

    /// 目录里所有「可能存在的 tmp」（前缀/后缀都取自实现，避免改名后断言变空跑）。
    fn tmps_in(target: &Path) -> Vec<PathBuf> {
        let pre = tmp_prefix_of(target)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        fs::read_dir(target.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let n = p.file_name().unwrap().to_string_lossy().into_owned();
                n.starts_with(&pre) && n.ends_with(TMP_SUFFIX)
            })
            .collect()
    }

    #[test]
    fn write_then_read_roundtrips() {
        let dir = temp_dir("rt");
        let path = dir.join(FILE_NAME);
        let doc = r#"{"version":1,"words":[{"text":"好","freq":2}]}"#;

        atomic_write(&path, doc).expect("落盘成功");
        assert_eq!(read_document(&path).unwrap().as_deref(), Some(doc));
        assert!(tmps_in(&path).is_empty(), "成功后不得留 tmp");

        let _ = fs::remove_dir_all(&dir);
    }

    /// 语义 1：不存在 = 首次运行的正常路径。
    #[test]
    fn missing_file_is_none_not_error() {
        let dir = temp_dir("missing");
        let path = dir.join("never-written.json");
        assert_eq!(
            read_document(&path).unwrap(),
            None,
            "缺失 → Ok(None)，不是 Err"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// 语义 2 的边界：0 字节是**损坏残留**，不与「不存在」同判。
    /// 若这里返回 Ok(None)，掉电残留会被当成「首次运行」静默吞掉。
    #[test]
    fn zero_byte_file_is_err_not_none() {
        let dir = temp_dir("zerobyte");
        let path = dir.join(FILE_NAME);
        fs::write(&path, b"").unwrap();

        let r = read_document(&path);
        assert!(r.is_err(), "0 字节必须是 Err（掉电残留），实得 {r:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    /// 原子性：写在途中的 tmp 绝不能被当成文档。目标缺失时只能得到 Ok(None)。
    #[test]
    fn half_written_tmp_is_not_read_as_document() {
        let dir = temp_dir("atomic");
        let path = dir.join(FILE_NAME);

        // 场景 A：目标尚未生成，只有半截 tmp（首次落盘写到一半就崩）
        fs::write(a_tmp_of(&path), r#"{"version":1,"words":[{"text":"好","#).unwrap();
        assert_eq!(read_document(&path).unwrap(), None, "tmp 不得被当成文档");

        // 场景 B：目标有效 + tmp 半截 → 读到的必须是旧的有效内容
        atomic_write(&path, "OLD").unwrap();
        fs::write(a_tmp_of(&path), "HALF-WRITTEN").unwrap();
        assert_eq!(read_document(&path).unwrap().as_deref(), Some("OLD"));

        let _ = fs::remove_dir_all(&dir);
    }

    /// 并发写入不得写坏文档 —— tmp 名唯一的**唯一理由**，也是它的门禁。
    /// 固定名下两个写入者会吃掉对方的 tmp（rename ENOENT）+ 交错写同一路径。
    #[test]
    fn concurrent_writes_never_corrupt_the_document() {
        const WRITERS: usize = 4;
        const ROUNDS: usize = 40;

        let dir = temp_dir("race");
        let path = dir.join(FILE_NAME);
        let docs: Vec<String> = (0..WRITERS).map(|w| format!("DOC-{w}-complete")).collect();

        std::thread::scope(|s| {
            for doc in &docs {
                let path = path.clone();
                let doc = doc.clone();
                s.spawn(move || {
                    for _ in 0..ROUNDS {
                        atomic_write(&path, &doc).expect("并发落盘不得失败");
                    }
                });
            }
        });

        let got = read_document(&path)
            .unwrap_or_else(|e| panic!("并发写坏了文档: {e}"))
            .expect("文档必须存在");
        assert!(
            docs.contains(&got),
            "必须恰好是某一个写入者的**完整**文档，实得 {got:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// 语义 3：写入失败不 panic、不静默，且**旧文档原样保留**。
    #[test]
    fn write_failure_preserves_previous_document() {
        let dir = temp_dir("rofs");
        let path = dir.join(FILE_NAME);
        atomic_write(&path, "OLD").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

            let r = atomic_write(&path, "NEW");
            assert!(r.is_err(), "只读目录必须 Err（不 panic、不静默）：{r:?}");

            fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(read_document(&path).unwrap().as_deref(), Some("OLD"));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    /// 语义 3 的另两种形态：父目录不存在 / 目标是目录 —— 一律 Err 且不破坏现场。
    #[test]
    fn write_to_bad_target_errs_without_panic() {
        let dir = temp_dir("badtarget");

        assert!(
            atomic_write(&dir.join("nodir/out.json"), "X").is_err(),
            "父目录缺失"
        );

        // 目标是目录：tmp 建得出来、rename 必然失败 —— 唯一真正走到**收尸**的失败路径
        let as_dir = dir.join("iamadir");
        fs::create_dir(&as_dir).unwrap();
        assert!(atomic_write(&as_dir, "X").is_err());
        assert!(as_dir.is_dir(), "失败不得破坏既有目标");
        assert!(
            tmps_in(&as_dir).is_empty(),
            "rename 失败后必须自己收尸，实得 {:?}",
            tmps_in(&as_dir)
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_directory_path_errs() {
        let dir = temp_dir("isdir");
        assert!(read_document(&dir).is_err(), "目录不是文档");
        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------ 与引擎的端到端契约

    /// 端到端：`export_json` → `atomic_write` → `read_document` → `import_json`，逐条相同。
    #[test]
    fn learner_roundtrips_through_file() {
        let dir = temp_dir("e2e");
        let path = dir.join(FILE_NAME);

        let mut l = Learner::new(true);
        l.record_selection("好");
        l.record_selection("好");
        l.record_selection("号");
        l.import_json(r#"{"version":1,"words":[{"text":"词","freq":4294967295}]}"#)
            .unwrap();
        atomic_write(&path, &l.export_json()).unwrap();

        let mut fresh = Learner::new(true);
        let doc = read_document(&path).unwrap().expect("文档存在");
        assert_eq!(fresh.import_json(&doc).unwrap(), 3);
        assert_eq!(fresh.freq_of("好"), 2);
        assert_eq!(fresh.freq_of("词"), u32::MAX);
        assert_eq!(
            fresh.export_json(),
            l.export_json(),
            "导出串相等 = 逐条相同"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// **语义 2 的核心锁定**：文件存在但内容损坏 —— 绝不允许静默变空表。
    ///
    /// 本层（IO）如实把内容交出来，`import_json` 判它损坏并**返回 Err**；调用方若把
    /// 这个 Err 降级成空表，就是用户几千条一次全没且无人知晓（fcitx5 词库静默失效同族）。
    /// 这里把「损坏 → Err → 内存不变」钉成一条链。
    #[test]
    fn corrupt_document_never_silently_empties() {
        let dir = temp_dir("corrupt");
        for (tag, body) in [
            ("truncated", r#"{"version":1,"words":[{"text":"好","freq":"#),
            ("notjson", "not json"),
            ("nofields", "{}"),
            ("badver", r#"{"version":2,"words":[]}"#),
        ] {
            let path = dir.join(format!("{tag}.json"));
            fs::write(&path, body).unwrap();

            // IO 层不认识 schema，只如实交出内容（这就是它该做的）
            let doc = read_document(&path)
                .unwrap_or_else(|e| panic!("{tag}: 非空文件应可读: {e}"))
                .expect("文件存在");
            // 解析层必须拒绝，且不动内存
            let mut l = Learner::new(true);
            l.record_selection("旧");
            let r = l.import_json(&doc);
            assert!(
                r.is_err(),
                "{tag}: 损坏内容必须 Err（静默空表是最坏的答案）"
            );
            assert_eq!(l.freq_of("旧"), 1, "{tag}: 失败不得清掉已有内存");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    /// 0 字节这一路**在 IO 层就断了**，根本到不了解析层 —— 双保险。
    #[test]
    fn zero_byte_document_never_reaches_the_parser() {
        let dir = temp_dir("zerobyte-e2e");
        let path = dir.join(FILE_NAME);
        fs::write(&path, b"").unwrap();

        let mut l = Learner::new(true);
        l.record_selection("旧");
        // 调用方的正确编排：read_document 先 Err，就不该继续 import
        assert!(read_document(&path).is_err());
        assert_eq!(l.freq_of("旧"), 1);

        let _ = fs::remove_dir_all(&dir);
    }
}
