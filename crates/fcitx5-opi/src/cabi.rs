//! C ABI 入口面（B3）：data_dir 词库初始化出口。自 lib.rs 迁出，避免 lib.rs
//! 超 500 行；字符串约定与 lib.rs 其余入口一致（UTF-8 + 长度，非 NUL 结尾）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use crate::data_dir;
use crate::{ensure_panic_hook, install, read_utf8};

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
