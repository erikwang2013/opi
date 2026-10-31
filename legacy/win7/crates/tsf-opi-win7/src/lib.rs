// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Minimal Windows 7/8 fork of `crates/tsf-opi`.
//!
//! Phase 1 keeps only what is needed for a registerable, compilable TSF DLL:
//! COM class factory + `Dll*` exports, profile registration, and a bare
//! `ITfTextInputProcessor` that proves the engine crates link. Key handling,
//! composition, candidates and the pipe client are Phase 2 and live in the
//! Windows 10 crate — they are intentionally not copied here.
//!
//! Win7 deltas versus the Windows 10 crate:
//! - `windows` 0.61.3 instead of 0.62 (no unconditional windows-future, no
//!   combase import).
//! - DPI is looked up dynamically (`dpi.rs`); `GetDpiForWindow` is not linked.
//! - Static CRT + panic=abort via `.cargo/config.toml` (no VCRUNTIME import).
//! - The main workspace's engine crates are used as path dependencies; no
//!   engine copy and no shim is needed because the pinned nightly accepts
//!   edition 2024 let-chains (MSRV 1.88).

pub mod dll;

mod scale;

#[cfg(target_os = "windows")]
mod com_server;
#[cfg(target_os = "windows")]
mod dpi;
#[cfg(target_os = "windows")]
mod registration;
#[cfg(target_os = "windows")]
mod tsf;

#[cfg(target_os = "windows")]
pub use com_server::{DllCanUnloadNow, DllGetClassObject, DllRegisterServer, DllUnregisterServer};
