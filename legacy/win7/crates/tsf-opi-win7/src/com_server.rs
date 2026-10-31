// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! COM server surface for the Win7 fork: class factory, four `Dll*` exports,
//! and the registration log.
//!
//! Forked from `crates/tsf-opi/src/com_server.rs` with the engine/candidate
//! wiring removed. `CreateInstance` builds the bare `TsfTextService`; Phase 2
//! adds key sinks and the pipe client back here.

use core::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER, HMODULE, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, Ref, Result, implement};

use crate::dll::{CLSID_TEXT_SERVICE, DllLock, dll_can_unload};
use crate::registration::{register, unregister};
use crate::tsf::TsfTextService;

/// Every COM exit catches panics: escaping an `extern "system"` boundary aborts
/// the host process (Word/browser), and the DLL is in-proc.
fn guard(f: impl FnOnce() -> Result<()>) -> Result<()> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(E_FAIL.into()))
}

fn com_exit(f: impl FnOnce() -> Result<()>) -> HRESULT {
    guard(f).map_or_else(|e| e.code(), |()| S_OK)
}

pub(crate) fn parse_guid(s: &str) -> Result<GUID> {
    GUID::try_from(s).map_err(|_| E_FAIL.into())
}

#[implement(IClassFactory)]
struct ClassFactory {
    _self_lock: DllLock,
    server_lock: Mutex<Option<DllLock>>,
}

impl ClassFactory {
    fn new() -> Self {
        Self {
            _self_lock: DllLock::new(),
            server_lock: Mutex::new(None),
        }
    }
}

impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        guard(|| {
            if !punkouter.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            if riid.is_null() || ppvobject.is_null() {
                return Err(E_POINTER.into());
            }
            // SAFETY: ppvobject was checked for null.
            unsafe { *ppvobject = core::ptr::null_mut() };

            let service = TsfTextService::new();
            let unknown: IUnknown = service.into();
            // SAFETY: riid points at a caller-owned GUID, ppvobject is writable.
            unsafe { unknown.query(riid, ppvobject) }.ok()
        })
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        guard(|| {
            let mut slot = match self.server_lock.lock() {
                Ok(g) => g,
                Err(_) => return Err(E_FAIL.into()),
            };
            *slot = if flock.as_bool() {
                Some(DllLock::new())
            } else {
                None
            };
            Ok(())
        })
    }
}

/// # Safety
///
/// COM-only export: `rclsid`/`riid` must point at valid GUIDs and `ppv` at a
/// writable `*mut c_void`. Null inputs return `E_POINTER`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    com_exit(|| {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: rclsid is non-null and COM guarantees a valid GUID.
        if unsafe { *rclsid } != parse_guid(CLSID_TEXT_SERVICE)? {
            return Err(CLASS_E_CLASSNOTAVAILABLE.into());
        }
        let factory: IClassFactory = ClassFactory::new().into();
        // SAFETY: ppv is non-null and points at a writable slot.
        unsafe { *ppv = core::ptr::null_mut() };
        // SAFETY: factory is alive for this call.
        unsafe { factory.query(riid, ppv) }.ok()
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if dll_can_unload() { S_OK } else { S_FALSE }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    reset_register_log();
    let result = guard(register);
    if let Err(e) = &result {
        log_register_failure("DllRegisterServer", e);
    }
    result.map_or_else(|e| e.code(), |()| S_OK)
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    com_exit(unregister)
}

/// Full path of this DLL (the `InprocServer32` default value).
///
/// Must use `FROM_ADDRESS`: `GetModuleFileNameW(None, ..)` returns the host EXE
/// (regsvr32.exe), which makes registration look successful while COM loads the
/// wrong module forever.
pub(crate) fn own_module_path() -> Result<String> {
    let mut hmod = HMODULE::default();
    // SAFETY: DllRegisterServer lives in this module; hmod is a valid output.
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(DllRegisterServer as extern "system" fn() -> HRESULT as *const u16),
            &mut hmod,
        )?;
    }
    let mut buf = vec![0u16; 32768];
    // SAFETY: buf is writable for its full length.
    let n = unsafe { GetModuleFileNameW(Some(hmod), &mut buf) };
    if n == 0 || n as usize >= buf.len() {
        return Err(E_FAIL.into());
    }
    Ok(String::from_utf16_lossy(&buf[..n as usize]))
}

fn register_log_path() -> std::path::PathBuf {
    std::env::var_os("ProgramData")
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from)
        .join("opi")
        .join("tsf-register.log")
}

/// Best-effort registration failure log; never changes the registration result.
pub(crate) fn log_register_failure(stage: &str, err: &windows::core::Error) {
    let path = register_log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let line = format!("{stage}: HRESULT 0x{:08X}\r\n", err.code().0 as u32);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

fn reset_register_log() {
    let _ = std::fs::remove_file(register_log_path());
}
