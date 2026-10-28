// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 日志出口：stderr + Windows 调试器。
//!
//! TSF 跑在 GUI 宿主里，stderr 通常无处可看；`OutputDebugStringW` 是 Windows 上
//! 不引入文件 IO 的可见出口（DebugView / 调试器）。日志格式仍留在各自模块。

/// 一行日志。两个出口都写：主机测试看 stderr，Windows 宿主看调试器。
pub(crate) fn log(msg: &str) {
    eprintln!("{msg}");
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
        use windows::core::HSTRING;
        // SAFETY: OutputDebugStringW 只读这块 UTF-16 内存，调用期间有效。
        unsafe { OutputDebugStringW(&HSTRING::from(msg)) };
    }
}

/// 用法同 `eprintln!`：`tlog!("词库: {line}")`。
macro_rules! tlog {
    ($($arg:tt)*) => {
        $crate::logging::log(&format!($($arg)*))
    };
}
