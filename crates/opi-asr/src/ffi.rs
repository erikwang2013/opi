//! C ABI. All `opi_asr_*` functions are thread-safe except that
//! `opi_asr_destroy` must be the last call for a handle (no concurrent calls).
//! `feed_pcm` is non-blocking; `poll_event` never blocks; `load_model` blocks
//! until the model is verified and created.
//! Safety contract is stated once here; all pointer arguments are null-checked.

#![allow(clippy::missing_safety_doc)]

use crate::config::AsrConfig;
use crate::error::{AsrError, ERR_ARGUMENT, ERR_EVENT_TOO_SMALL, ERR_IO};
use crate::manifest::{self, ModelManifest};
use crate::worker::{Asr, MAX_EVENT_BYTES};
use serde_json::json;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;
use std::sync::Mutex;

/// Error from `opi_asr_create`, readable via `opi_asr_last_error(NULL, ..)`.
static CREATE_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn set_create_error(e: &AsrError) {
    let mut slot = CREATE_ERROR
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *slot = Some(e.to_string());
}

unsafe fn str_arg<'a>(p: *const c_char) -> Result<&'a str, AsrError> {
    if p.is_null() {
        return Err(AsrError::Argument("null string".into()));
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|_| AsrError::Argument("string is not valid UTF-8".into()))
}

unsafe fn as_ref<'a>(handle: *mut Asr) -> Result<&'a Asr, AsrError> {
    if handle.is_null() {
        return Err(AsrError::Argument("null handle".into()));
    }
    Ok(unsafe { &*handle })
}

/// Writes `s` plus a NUL. Rejects (without consuming) when `cap` is too small.
unsafe fn write_c_string(s: &str, buf: *mut c_char, cap: i32) -> i32 {
    if buf.is_null() || cap <= 0 {
        return ERR_ARGUMENT;
    }
    let bytes = s.as_bytes();
    let cap = cap as usize;
    if bytes.len() >= cap {
        return ERR_EVENT_TOO_SMALL;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf.cast::<u8>(), bytes.len());
        *buf.add(bytes.len()) = 0;
    }
    bytes.len() as i32
}

/// Returns a new handle or NULL. On NULL, details are available through
/// `opi_asr_last_error(NULL, ..)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_create(config_json: *const c_char) -> *mut Asr {
    let result = (|| {
        let json = unsafe { str_arg(config_json) }?;
        let cfg = AsrConfig::from_json(json)?;
        Asr::new(cfg)
    })();
    match result {
        Ok(asr) => {
            *CREATE_ERROR
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = None;
            Box::into_raw(Box::new(asr))
        }
        Err(e) => {
            set_create_error(&e);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_destroy(handle: *mut Asr) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) };
    }
}

/// 0 on success, negative `ERR_*` otherwise. Blocks while hashing/loading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_load_model(handle: *mut Asr) -> i32 {
    let result = unsafe { as_ref(handle) }.and_then(|asr| asr.load_model().map(|_| 0));
    result.unwrap_or_else(|e| e.code())
}

/// Queued; `not_loaded` / `already_started` surface as error events.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_start(handle: *mut Asr) -> i32 {
    let result = unsafe { as_ref(handle) }.and_then(|asr| asr.start().map(|_| 0));
    result.unwrap_or_else(|e| e.code())
}

/// `data` is 16 kHz mono PCM16 LE. At most 256 KiB (8 s) per call.
/// -5 (`ERR_BUSY`) means the queue is full; retry later.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_feed_pcm(handle: *mut Asr, data: *const u8, len: i32) -> i32 {
    if len < 0 || (data.is_null() && len > 0) || len as usize > crate::worker::MAX_FEED_BYTES {
        return ERR_ARGUMENT;
    }
    let bytes: &[u8] = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, len as usize) }
    };
    let result = unsafe { as_ref(handle) }.and_then(|asr| asr.feed_pcm(bytes).map(|_| 0));
    result.unwrap_or_else(|e| e.code())
}

/// Queued; success is reported by the `final` event.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_finish(handle: *mut Asr) -> i32 {
    let result = unsafe { as_ref(handle) }.and_then(|asr| asr.finish().map(|_| 0));
    result.unwrap_or_else(|e| e.code())
}

/// Drops queued audio and events. An ONNX decode already running cannot be
/// interrupted; its output is discarded.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_cancel(handle: *mut Asr) -> i32 {
    let result = unsafe { as_ref(handle) }.and_then(|asr| asr.cancel().map(|_| 0));
    result.unwrap_or_else(|e| e.code())
}

/// Non-blocking. Returns JSON length (>0), 0 when empty, -8 when `cap` is too
/// small (the event is kept, so retry with a bigger buffer),
/// or another negative `ERR_*`. Buffer is NUL-terminated on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_poll_event(handle: *mut Asr, buf: *mut c_char, cap: i32) -> i32 {
    let asr = match unsafe { as_ref(handle) } {
        Ok(asr) => asr,
        Err(e) => return e.code(),
    };
    let Some(event) = asr.poll_event() else {
        return 0;
    };
    let json = match serde_json::to_string(&event) {
        Ok(json) if json.len() <= MAX_EVENT_BYTES => json,
        _ => return ERR_IO,
    };
    let written = unsafe { write_c_string(&json, buf, cap) };
    if written == ERR_EVENT_TOO_SMALL {
        asr.return_event(event);
    }
    written
}

/// Copies the last error (or create-time error for a NULL handle).
/// Returns length, 0 when there is none, -8 if the buffer is too small.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_last_error(handle: *mut Asr, buf: *mut c_char, cap: i32) -> i32 {
    let message = if handle.is_null() {
        CREATE_ERROR
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    } else {
        unsafe { &*handle }.last_error()
    };
    match message {
        Some(message) => unsafe { write_c_string(&message, buf, cap) },
        None => {
            if !buf.is_null() && cap > 0 {
                unsafe { *buf = 0 };
            }
            0
        }
    }
}

/// One event JSON is bounded by this; allocate that much for `poll_event`.
#[unsafe(no_mangle)]
pub extern "C" fn opi_asr_max_event_bytes() -> i32 {
    MAX_EVENT_BYTES as i32
}

/// Verifies a model directory against a manifest JSON (size + SHA-256, path
/// containment). Returns a malloc'd JSON `{"ok":true,"files":..,"bytes":..}`
/// or `{"ok":false,"error":".."}`. Free with `opi_asr_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_verify_model(
    dir: *const c_char,
    manifest_json: *const c_char,
) -> *mut c_char {
    if dir.is_null() || manifest_json.is_null() {
        return ptr::null_mut();
    }
    let result = (|| -> Result<(usize, u64), AsrError> {
        let dir = unsafe { str_arg(dir) }?;
        let json = unsafe { str_arg(manifest_json) }?;
        let manifest = ModelManifest::from_json(json)?;
        let report = manifest::verify(Path::new(dir), &manifest, true)?;
        Ok((report.files, report.bytes))
    })();
    let value = match result {
        Ok((files, bytes)) => json!({"ok": true, "files": files, "bytes": bytes}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    };
    match CString::new(value.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_asr_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}
