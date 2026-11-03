// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 用户词（学习结果）的恢复与自动落盘 —— fcitx5 宿主侧。
//!
//! **路径**：`$XDG_DATA_HOME/opi/opi_user_words.json`（`data_dir::xdg_data_dir()`，
//! 与皮肤/自定义短语/导入词库同一数据目录；文件名常量取自
//! `engine_data::user_words::FILE_NAME`，与 Android/macOS 共用同一个名字）。
//!
//! **分层**：引擎（engine-core）不做 IO 是不变量。本模块只做
//! 「文件 ↔ JSON 串」的编排：恢复 = `read_document` → `Engine::import_user_words`；
//! 落盘 = `Engine::export_user_words` → `engine_data::atomic_write`。
//!
//! **失败语义**（见 `engine_data::user_words::read_document` 的三条）：
//! - 文件不存在 = 冷启动，静默；
//! - 0 字节 / 读不出 / 坏 JSON / 版本不符 = **报告并保留原文件**，绝不当空表覆盖；
//! - 只有成功导入才接管内存状态。
//!
//! **防抖**：宿主 C++ 每 250ms 调 [`opi_fcitx5_save_user_words_if_due`]；本层在最后一次
//! 学习后静置 [`SAVE_DEBOUNCE`] 才取快照（尾随防抖，连续打字不会每键写文件）。计时用
//! Rust 的 `Instant`，不依赖宿主时钟。析构路径调 [`opi_fcitx5_flush_user_words`] 补最后一次。
//!
//! **落盘不挂在事件循环上**：fsync 在慢盘上可能几百毫秒到数秒，而宿主定时器跑在
//! fcitx5 主循环里。快照排进专用写线程（单槽合并），输入/候选/D-Bus 都不等磁盘；
//! 写失败回标 dirty 退避重试。`opi_fcitx5_flush_user_words` 最多等 2 秒。
//!
//! **清空**必须连文件一起删：只清内存的话下次启动会把旧词重新读回来。

use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use engine_data::user_words::{FILE_NAME, atomic_write, read_document};

use crate::candidate::CandidateState;

/// 尾随防抖窗口（与 Android `UserWordStore.SAVE_DEBOUNCE_MS` 同量级）。
pub(crate) const SAVE_DEBOUNCE: Duration = Duration::from_millis(800);

/// 宿主接线状态。`path = None` = 未接线（单测构造的裸状态与 `install()` 走这里，
/// 不会碰用户真实数据目录）。
#[derive(Debug, Default)]
pub(crate) struct Persistence {
    path: Option<PathBuf>,
    dirty: bool,
    /// 最后一次标脏的时刻；`None` 仅在 `dirty == false` 时出现。
    changed_at: Option<Instant>,
    /// 后台落盘的代际号：清空用户词时 +1，写线程发现自己的代际已过期就撤掉
    /// 这份快照（否则「清空」与一次在途写盘会互相打架，文件会复活）。
    epoch: Arc<AtomicU64>,
}

impl Persistence {
    fn enabled(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            dirty: false,
            changed_at: None,
            epoch: Arc::new(AtomicU64::new(0)),
        }
    }

    fn mark_dirty(&mut self) {
        if self.path.is_some() {
            self.dirty = true;
            self.changed_at = Some(Instant::now());
        }
    }

    fn due_at(&self, now: Instant) -> bool {
        self.path.is_some()
            && self.dirty
            && self
                .changed_at
                .is_none_or(|t| now.duration_since(t) >= SAVE_DEBOUNCE)
    }
}

/// 启动恢复结果（诊断/测试用；失败细节已由 `attach` 打到 stderr）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Restore {
    /// 文件不存在 = 冷启动。
    Missing,
    /// 文件有效并已导入引擎（含空词表）。
    Imported,
    /// 读不出 / 0 字节 / 坏 JSON —— 内存保持现状，文件原样保留。
    Failed,
}

pub(crate) fn file_path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// 把持久化挂到 `state` 上，并从 `dir` 恢复用户词。
///
/// 失败只报告、**不删不改**文件：0 字节与坏 JSON 是掉电/中断残留的证据，
/// 静默覆盖等于用户学习结果凭空消失。
pub(crate) fn attach(state: &mut CandidateState, dir: &Path) -> Restore {
    let path = file_path(dir);
    state.persist = Persistence::enabled(path.clone());
    match read_document(&path) {
        Ok(None) => Restore::Missing,
        Ok(Some(json)) => match state.engine.import_user_words(&json) {
            Ok(_) => Restore::Imported,
            Err(e) => report(&path, &e),
        },
        Err(e) => report(&path, &e),
    }
}

fn report(path: &Path, err: &str) -> Restore {
    eprintln!(
        "fcitx5-opi: 用户词表未载入（保留原文件 {}）: {err}",
        path.display()
    );
    Restore::Failed
}

/// 生产装载入口：装载引擎后从 `user_words_dir` 恢复用户词。
/// `None` = 不接线（`install` / `install_with_trad` 与测试走这条）。
pub(crate) fn install_with_trad_at(
    path: Option<&str>,
    trad: Option<&str>,
    user_words_dir: Option<&Path>,
) -> Result<(), String> {
    let mut state = CandidateState::load_with_trad(path, trad)?;
    if let Some(dir) = user_words_dir {
        attach(&mut state, dir);
    }
    let mut guard = crate::SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    *guard = Some(state);
    Ok(())
}

impl CandidateState {
    /// 提交类出口的统一收尾：翻页快照 + 学习落盘标脏。`out` 非空 = 本次上屏了文本
    /// （选词 / 空格接受 / 标点前冲刷），按「可能记了一次学习」处理。
    pub(crate) fn finish_commit(&mut self, out: String) -> String {
        self.reset_page_if_buffer_changed();
        if !out.is_empty() {
            self.persist.mark_dirty();
        }
        out
    }

    /// 选词出口专用：`out` 为空也可能是学习发生了 —— 逐字回退态把片段攒进
    /// pending（`engine_segment.rs`），此刻返回空串但已记账。
    pub(crate) fn finish_selection(&mut self, out: String) -> String {
        self.reset_page_if_buffer_changed();
        if !out.is_empty() || !self.engine.pending_text().is_empty() {
            self.persist.mark_dirty();
        }
        out
    }

    /// 标点出口：**只有调用前缓冲非空**才可能有「垫在标点前的首候选提交」；
    /// 空缓冲的纯标点映射（`，`→`，`）不算学习，别标脏。
    pub(crate) fn finish_punct(&mut self, out: Option<String>, had_buffer: bool) -> Option<String> {
        self.reset_page_if_buffer_changed();
        if had_buffer && out.as_ref().is_some_and(|t| !t.is_empty()) {
            self.persist.mark_dirty();
        }
        out
    }

    /// 到点同步写一次（**只给测试**：生产路径走 take_due + 后台线程）。
    #[cfg(test)]
    pub(crate) fn save_user_words_if_due_at(&mut self, now: Instant) -> bool {
        let Some(job) = self.take_due_user_words(now) else {
            return false;
        };
        match write_snapshot(&job.path, &job.json) {
            Ok(()) => true,
            Err(e) => {
                // 保持 dirty 以便下次重试；失败退避一个防抖窗口，避免只读磁盘上刷屏。
                self.persist.mark_dirty();
                eprintln!("fcitx5-opi: 用户词表落盘失败: {e}");
                false
            }
        }
    }

    /// 到点才取内存快照（**不碰磁盘**）：宿主把它交给后台写线程。
    fn take_due_user_words(&mut self, now: Instant) -> Option<UserWordsJob> {
        if !self.persist.due_at(now) {
            return None;
        }
        self.take_user_words_snapshot()
    }

    /// 卸载路径：不看防抖，dirty 就取。
    fn take_dirty_user_words(&mut self) -> Option<UserWordsJob> {
        if !self.persist.dirty {
            return None;
        }
        self.take_user_words_snapshot()
    }

    fn take_user_words_snapshot(&mut self) -> Option<UserWordsJob> {
        let path = self.persist.path.clone()?;
        let json = self.engine.export_user_words();
        let epoch = Arc::clone(&self.persist.epoch);
        let stamp = epoch.load(Ordering::Relaxed);
        // 取走即清 dirty：写失败由后台线程回标（mark_user_words_dirty）。
        self.persist.dirty = false;
        self.persist.changed_at = None;
        Some(UserWordsJob {
            path,
            json,
            epoch,
            stamp,
        })
    }

    /// 后台写失败时回标 dirty（退避一个防抖窗口后重试）。
    fn mark_user_words_dirty(&mut self) {
        self.persist.mark_dirty();
    }

    /// 清空用户词（学习记忆）。缓冲没变但候选集会变（用户词参与排名）⇒ 作废
    /// 抓取缓存并回第一页，别停在一个已经不存在的页上。**同时删除文件**：
    /// 否则下次启动会把旧词重新读回来。删除失败返回 Err（内存已清）。
    pub fn clear_user_words(&mut self) -> Result<(), String> {
        self.engine.clear_user_words();
        self.page = 0;
        self.invalidate_fetch_cache();
        self.persist.dirty = false;
        self.persist.changed_at = None;
        // 作废一切在途/待写的旧快照：清空比它们新，文件不能被它们复活。
        self.persist.epoch.fetch_add(1, Ordering::Relaxed);
        let Some(path) = self.persist.path.clone() else {
            return Ok(());
        };
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => {
                let msg = format!("用户词表删除失败（{}）: {e}", path.display());
                eprintln!("fcitx5-opi: {msg}");
                Err(msg)
            }
        }
    }
}

/// 落一份快照：建父目录 + 原子写（含 fsync）。**只允许后台线程与测试调用**，
/// 事件循环绝不能走这里（慢盘 fsync 会冻住输入法）。
fn write_snapshot(path: &Path, json: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("用户词目录创建失败（{}）: {e}", parent.display()))?;
    }
    atomic_write(path, json)
}

// ---------- 后台落盘（不阻塞 fcitx5 事件循环） ----------
//
// `atomic_write` 里的 `sync_all()` 是 fsync：/home 在慢盘上时可能几百毫秒到数秒。
// 原来它在 250ms 定时器的调用栈里执行 —— 那正是 fcitx5 的事件循环，一次慢写就冻住
// 整个输入法（系统语言栏不刷新、按键不进引擎、opi-ctl state 全部超时）。
// 现在事件循环只取内存快照入队（单槽，相邻快照合并），磁盘写在专用线程做。

struct UserWordsJob {
    path: PathBuf,
    json: String,
    epoch: Arc<AtomicU64>,
    stamp: u64,
}

#[derive(Default)]
struct WriterSlot {
    job: Option<UserWordsJob>,
    busy: bool,
}

struct UserWordsWriter {
    slot: Mutex<WriterSlot>,
    wake: Condvar,
    idle: Condvar,
}

static WRITER: OnceLock<Arc<UserWordsWriter>> = OnceLock::new();

fn writer() -> &'static Arc<UserWordsWriter> {
    WRITER.get_or_init(|| {
        let w = Arc::new(UserWordsWriter {
            slot: Mutex::new(WriterSlot::default()),
            wake: Condvar::new(),
            idle: Condvar::new(),
        });
        let background = Arc::clone(&w);
        if let Err(e) = std::thread::Builder::new()
            .name("opi-user-words".into())
            .spawn(move || writer_loop(&background))
        {
            // 线程起不来：快照留在槽里，flush 等超时后放弃；输入路径不被阻塞。
            eprintln!("fcitx5-opi: 用户词后台写线程启动失败: {e}");
        }
        w
    })
}

fn writer_loop(w: &UserWordsWriter) {
    loop {
        let job = {
            let mut slot = w.slot.lock().unwrap_or_else(|p| p.into_inner());
            while slot.job.is_none() {
                slot = w.wake.wait(slot).unwrap_or_else(|p| p.into_inner());
            }
            slot.busy = true;
            slot.job.take()
        };
        let Some(job) = job else { continue };
        let started = Instant::now();
        match write_snapshot(&job.path, &job.json) {
            // 写盘途中被「清空用户词」作废：撤掉这份过期快照，别让文件复活。
            Ok(()) if job.epoch.load(Ordering::Relaxed) != job.stamp => {
                let _ = fs::remove_file(&job.path);
            }
            Ok(()) => {}
            Err(e) => {
                eprintln!("fcitx5-opi: 用户词表落盘失败: {e}");
                if job.epoch.load(Ordering::Relaxed) == job.stamp {
                    crate::with_state(|s| s.mark_user_words_dirty());
                }
            }
        }
        // 慢盘取证：下一次「卡死」时日志能直接回答是不是落盘拖的。
        let elapsed = started.elapsed();
        if elapsed >= Duration::from_millis(500) {
            eprintln!(
                "fcitx5-opi: 用户词表落盘耗时 {} ms（后台线程，未阻塞输入）",
                elapsed.as_millis()
            );
        }
        let mut slot = w.slot.lock().unwrap_or_else(|p| p.into_inner());
        slot.busy = false;
        w.idle.notify_all();
    }
}

/// 排一份快照（非阻塞；相邻快照合并，槽里最多留最新一份）。
fn enqueue_user_words(job: UserWordsJob) {
    let w = writer();
    let mut slot = w.slot.lock().unwrap_or_else(|p| p.into_inner());
    slot.job = Some(job);
    w.wake.notify_one();
}

/// 等后台把队列写空；超时返回 false（关机路径最多等这么久，绝不吊住 fcitx5）。
fn wait_user_words_idle(timeout: Duration) -> bool {
    let w = writer();
    let deadline = Instant::now() + timeout;
    let mut slot = w.slot.lock().unwrap_or_else(|p| p.into_inner());
    while slot.job.is_some() || slot.busy {
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        let (next, _) = w
            .idle
            .wait_timeout(slot, deadline - now)
            .unwrap_or_else(|p| p.into_inner());
        slot = next;
    }
    true
}

/// 清空用户词（学习记忆）。返回 false = 引擎还没装载，或文件删不掉
/// （后者内存已清，错误已打到 stderr；C++ 侧据此报 err:4，不哑成功）。
///
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_clear_user_words() -> bool {
    crate::ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        crate::with_state(|s| s.clear_user_words().is_ok()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// 宿主定时器每 250ms 调一次：到防抖窗口才把内存快照排进后台队列。
/// false = 未到点/未装载。**不碰磁盘**，返回值不代表写盘结果。
///
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_save_user_words_if_due() -> bool {
    crate::ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        let snapshot = crate::with_state(|s| s.take_due_user_words(Instant::now())).flatten();
        match snapshot {
            Some(job) => {
                enqueue_user_words(job);
                true
            }
            None => false,
        }
    }))
    .unwrap_or(false)
}

/// 卸载前排入最后一次快照并等后台写完（最多 2 秒）。true = 队列已写空。
///
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_flush_user_words() -> bool {
    crate::ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        let snapshot = crate::with_state(|s| s.take_dirty_user_words()).flatten();
        if let Some(job) = snapshot {
            enqueue_user_words(job);
        }
        // 关机最多等 2 秒：慢盘上宁可丢最后一次学习，也不能吊住 fcitx5 退出。
        wait_user_words_idle(Duration::from_secs(2))
    }))
    .unwrap_or(false)
}

// 测试独立成文件（`#[path]`）以保持本文件 <500 行。
#[cfg(test)]
#[path = "user_words_tests.rs"]
mod tests;
