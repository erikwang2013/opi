// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗可执行文件的**候选表**（纯函数，平台中立 ⇒ 主机可测）。
//!
//! 启动动作在 `configure.rs`（Windows 专属：`CreateProcessW`）；**候选表为什么单独放**
//! 与 `dict_path.rs` 同一个理由：它错了的表现是「点属性没反应」这种**静默**失败，
//! 而判定本身不需要 Windows。
//!
//! 候选（顺序即优先级）：
//!   1. 本 DLL 同目录（把 DLL 与设置窗放一起，最省事）
//!   2. DLL 所在目录的**上一级**（jpackage 安装布局：DLL 在 `INSTALLDIR\app\`，
//!      启动器在 `INSTALLDIR\opi.exe`；MSI 与 EXE 安装器都是这个布局）
//!   3. `%LOCALAPPDATA%\opi\`（与词库的第三候选同一处，见 `dict_path.rs`）
//! 找不到 → 调用方什么都不弹（宁可不响，不弹空窗）。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// 设置窗的可执行文件名（与 `desktop/build.gradle.kts` 在 Windows 上的
/// `packageName` 同源：2026-10-20 起 Windows 包名是 `opi`）。
/// 候选窗与设置窗是**同一个 exe 的两个模式**（`desktop/.../Main.kt`：Windows 上
/// 无参数 = 候选窗、`--settings` = 设置窗）。
pub const SETTINGS_EXE: &str = "opi.exe";

/// 候选路径，按优先级排。两个输入都由调用方给（取环境变量/取 DLL 目录不在本层）。
pub fn candidates(dll_dir: Option<&Path>, local_app_data: Option<&OsStr>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = dll_dir {
        out.push(dir.join(SETTINGS_EXE));
        // jpackage app-image 布局：DLL 在 `INSTALLDIR\app\`，启动器在上一级。
        if let Some(install_dir) = dir.parent() {
            out.push(install_dir.join(SETTINGS_EXE));
        }
    }
    // 空串等同未设置（与 dict_path 的 `filter(|v| !v.is_empty())` 同口径）。
    if let Some(base) = local_app_data.filter(|v| !v.is_empty()) {
        out.push(PathBuf::from(base).join("opi").join(SETTINGS_EXE));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_is_dll_dir_install_dir_then_localappdata() {
        // 用正斜杠：`parent()` 的分隔符语义在 Linux 主机的测试与 Windows 目标上一致
        // （反斜杠在 Linux 上只是普通字符，会把整串当成单段路径）。
        let got = candidates(
            Some(Path::new("C:/Program Files/opi/app")),
            Some(OsStr::new("C:/Users/u/AppData/Local")),
        );
        assert_eq!(
            got,
            vec![
                PathBuf::from("C:/Program Files/opi/app").join(SETTINGS_EXE),
                PathBuf::from("C:/Program Files/opi").join(SETTINGS_EXE),
                PathBuf::from("C:/Users/u/AppData/Local")
                    .join("opi")
                    .join(SETTINGS_EXE),
            ]
        );
    }

    #[test]
    fn missing_inputs_drop_that_candidate() {
        assert_eq!(candidates(None, None), Vec::<PathBuf>::new());
        // 空环境变量等同未设置（与 dict_path 同口径）
        assert_eq!(
            candidates(None, Some(OsStr::new(""))),
            Vec::<PathBuf>::new()
        );
        // 根路径没有父目录：只保留 DLL 同目录那条
        assert_eq!(
            candidates(Some(Path::new("/")), Some(OsStr::new(""))),
            vec![PathBuf::from("/").join(SETTINGS_EXE)]
        );
    }
}
