// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 入口面（B3）：data_dir 词库初始化出口 + 符号浏览一族（2026-10-10）。
//! 自 lib.rs 迁出，避免 lib.rs 超 500 行；字符串约定与 lib.rs 其余入口一致
//! （UTF-8 + 长度，非 NUL 结尾）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use crate::data_dir;
use crate::{ensure_panic_hook, install, read_utf8, with_state};

/// 初始化词库（B3）：源路径 + XDG 数据目录 → 建目录 + size 校验拷贝 + 装载引擎；任一步失败 → 内置回退词库并返回 false（对照 EngineLoader：load(null)）。
/// # Safety
///
/// `source_ptr`/`data_dir_ptr` 必须分别指向至少 `source_len`/`data_dir_len`
/// 字节的有效内存（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_init_dict(
    source_ptr: *const u8,
    source_len: usize,
    data_dir_ptr: *const u8,
    data_dir_len: usize,
) -> bool {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        let source = unsafe { read_utf8(source_ptr, source_len) };
        let dir = unsafe { read_utf8(data_dir_ptr, data_dir_len) };
        let (source, dir) = match (source, dir) {
            (Some(s), Some(d)) if !s.is_empty() && !d.is_empty() => (s, d),
            _ => {
                let _ = install(None); // 参数缺失/空串 → 内置回退词库；若内置词库亦装载失败，引擎将留空（无法输入，直至下次装载成功）
                return false;
            }
        };
        match data_dir::init_dict(Path::new(&source), Path::new(&dir)) {
            Ok(()) => true,
            Err(_) => {
                let _ = install(None);
                false
            }
        }
    }))
    .unwrap_or(false)
}

/// 符号浏览开关（「符号大全」入口；2026-10-10 与 fcitx5-glue 对齐的硬契约 ——
/// 声明在 `cpp/opi_rust_ffi.h`，那边是弱符号接入，名字不许单方面改）：
/// `action` 0 = 退出 / 1 = 进入 / 2 = 切换；返回**执行后**的浏览态（1 = 在浏览，0 = 不在）。
///
/// - 0 **同步**：返回时 `opi_fcitx5_buffer()` 已是真缓冲 —— 切走/切英文（W1）的顺序是
///   「先 action 0 退浏览 → 再上缓冲原文」（2026-10-10 深夜用户裁决订正）；glue 的提交路径
///   每次切走都**无条件**先调一次 0，故 0 必须幂等（非浏览态 = 空操作，**不丢类目数据**）。
/// - 1 在浏览态 = no-op（保持层级）；无数据（没装过/装失败）→ 返回 0 ——
///   「入口禁用」的唯一表达。装没装另见 [`opi_fcitx5_load_symbols`] 的返回值。
/// - 2 = 引擎按自身状态决定进/出（C++ 侧看不到浏览态，热键要来回切）。
/// - 未知 action = 空操作，返回当前态（防御，不崩）。
///
/// 契约全文（buffer 劫持 / 未处理键先退出 / 类目不可上屏）在
/// `engine-core/src/engine_symbol_browse.rs` 头。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_symbol_browse(action: i32) -> i32 {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| match action {
            0 => {
                s.exit_symbol_browse();
                false
            }
            1 => s.enter_symbol_browse(),
            2 => {
                if s.is_symbol_browsing() {
                    s.exit_symbol_browse();
                    false
                } else {
                    s.enter_symbol_browse()
                }
            }
            _ => s.is_symbol_browsing(),
        })
        .unwrap_or(false) as i32
    }))
    .unwrap_or(0)
}

/// 装载符号类目数据（「符号大全」）：路径字节 + 长度 —— 语义与 `opi_fcitx5_load` 一致，
/// **收路径不收内容**（读文件在 Rust 侧 data_dir.rs；engine-core 禁 IO）。
///
/// 返回：**≥0 = 跳过的坏行数**（0 = 全好；此时类目已可用，入口随之可用）；
///       -1 = 读失败（路径为空/文件缺失/读不出/引擎未装载）；
///       -2 = 读到但**零合法类目**（空文件/全坏行）。
/// 三种降级都不静默：各打一行 stderr；-1/-2 之后入口保持禁用，插件其余部分不受影响。
/// 可重入：替换数据集并复位浏览态；**失败不清旧数据**（退化成旧数据集继续可用）。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 字节的有效内存（或为 null，视为空路径 → -1）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_load_symbols(ptr: *const u8, len: usize) -> i32 {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        let Some(path) = (unsafe { read_utf8(ptr, len) }).filter(|p| !p.is_empty()) else {
            eprintln!("fcitx5-opi: 符号数据装载被跳过（路径为空）—— 入口保持禁用");
            return -1;
        };
        match data_dir::load_symbols(Path::new(&path)) {
            Err(e) => {
                eprintln!("fcitx5-opi: 符号数据装载失败（入口保持禁用）: {e}");
                -1
            }
            Ok(skipped) => {
                if !with_state(|s| s.symbol_browse_available()).unwrap_or(false) {
                    eprintln!("fcitx5-opi: 符号数据无合法类目（入口保持禁用）: {path}");
                    return -2;
                }
                eprintln!("fcitx5-opi: 符号数据装载完成（跳过 {skipped} 行坏行）: {path}");
                skipped as i32
            }
        }
    }))
    .unwrap_or(-1)
}
