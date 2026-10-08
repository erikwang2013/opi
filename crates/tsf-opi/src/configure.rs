// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」的 Windows 入口：把设置窗拉起来（`ITfFnConfigure::Show` 的执行体）。
//!
//! 【契约来自文档，本机无法验证】Text Services（控制面板里那一页）取本接口的办法是
//! `CoCreateInstance(CLSID_TEXT_SERVICE, IID_ITfFnConfigure)` ⇒ **它必须实现在我们的
//! 服务对象上** —— `tsf.rs` 的 `#[implement]` 列表里因此加了 `ITfFunction` +
//! `ITfFnConfigure`（前者是后者的基接口）。**不涉及** `ITfFunctionProvider` /
//! `ITFSourceSingle::AdviseSingleSink` 那一路：那条是给「暴露自定义函数对象」用的，
//! 配置对话框这条按 MSDN 的 `ITfFnConfigure` 页面走 `CoCreateInstance`。
//! 【验证边界】与 `langbar.rs` 同档：本机只能过
//! `cargo check --target x86_64-pc-windows-msvc`（证接口名/签名与 windows crate 相符），
//! **不证明**控制面板真的会调它、更不证明窗口拉得起来。
//!
//! 候选表的判定在 `settings_exe.rs`（平台中立、有主机单测）；本文件只剩 Win32 那一步。

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
use windows::core::{PCWSTR, PWSTR};

use crate::settings_exe::{SETTINGS_EXE, candidates};

/// 拉起设置窗。`Err` = 拉不起来（调用方转成 E_FAIL；**不弹任何 UI** —— 宁可这一次
/// 不响，也不弹一个空窗或错误框）。
pub fn launch_settings() -> Result<(), String> {
    let dll = crate::com_server::own_module_path().ok().map(PathBuf::from);
    let dll_dir = dll.as_deref().and_then(Path::parent);
    let local = std::env::var_os("LOCALAPPDATA");
    let all = candidates(dll_dir, local.as_deref());
    let exe = all
        .iter()
        .find(|p| p.is_file())
        .ok_or_else(|| {
            let tried = all
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" / ");
            format!("找不到 {SETTINGS_EXE}（找过：{tried}）")
        })?
        .clone();

    // ⚠️ 命令行必须带 `--settings`：Windows 上**无参数 = 候选窗**（Linux 上无参数才进
    // 设置窗，见 Main.kt）。CreateProcessW 要的是**可写**缓冲区，故 vector 末尾加 NUL。
    let mut cmd: Vec<u16> = format!("\"{}\" --settings", exe.display())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();
    // SAFETY: 指针都指向本次调用期内存活的对象；lpApplicationName 传 null = 从
    // lpCommandLine 取路径（全局已加引号，路径含空格也安全）。
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            // ⚠️ 命令行那一格是 `Option<PWSTR>`（**可写**）：CreateProcessW 会就地把
            // 它拆成 argv。传 PCWSTR 是编译错（实测 E0308）。
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            false,
            Default::default(),
            None,
            PCWSTR::null(),
            &si,
            &mut pi,
        )
    }
    .map_err(|e| format!("CreateProcessW 失败：{e}"))?;
    // 只要拉起来，不跟踪它 —— 句柄不关就是泄漏（每点一次「属性」漏两个）。
    // SAFETY: 两个句柄来自刚刚成功的 CreateProcessW，尚未关闭。
    unsafe {
        let _ = CloseHandle(pi.hProcess);
        let _ = CloseHandle(pi.hThread);
    }
    Ok(())
}
