use opi_asr::*;
use serde_json::{Value, json};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("opi-asr-ffi-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn read_event(handle: *mut opi_asr::Asr, timeout: Duration) -> Option<Value> {
    let deadline = Instant::now() + timeout;
    let mut buf = vec![0 as c_char; MAX_EVENT_BYTES];
    loop {
        let n = unsafe { opi_asr_poll_event(handle, buf.as_mut_ptr(), buf.len() as i32) };
        if n > 0 {
            let text = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap();
            return Some(serde_json::from_str(text).unwrap());
        }
        assert!(n >= 0, "poll_event failed: {n}");
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn create_reports_config_error() {
    let bad = c("{not json");
    let handle = unsafe { opi_asr_create(bad.as_ptr()) };
    assert!(handle.is_null());

    let mut buf = vec![0 as c_char; 512];
    let n = unsafe { opi_asr_last_error(ptr::null_mut(), buf.as_mut_ptr(), buf.len() as i32) };
    assert!(n > 0);
    let message = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap();
    assert!(message.contains("invalid config"), "{message}");
}

#[test]
fn start_without_model_emits_event_and_keeps_it_on_small_buffer() {
    let config = c(r#"{"language":"auto"}"#);
    let handle = unsafe { opi_asr_create(config.as_ptr()) };
    assert!(!handle.is_null());

    assert_eq!(unsafe { opi_asr_start(handle) }, 0);

    // Wait until the worker has produced the error event.
    let mut small = [0 as c_char; 2];
    let mut rc = 0;
    for _ in 0..400 {
        rc = unsafe { opi_asr_poll_event(handle, small.as_mut_ptr(), 2) };
        if rc == ERR_EVENT_TOO_SMALL {
            break;
        }
        assert_eq!(rc, 0);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(rc, ERR_EVENT_TOO_SMALL, "event must not be consumed");

    let event = read_event(handle, Duration::from_secs(2)).expect("error event");
    assert_eq!(event["type"], "error");
    assert_eq!(event["code"], "not_loaded");

    let n = unsafe { opi_asr_last_error(handle, small.as_mut_ptr(), 2) };
    assert_eq!(n, ERR_EVENT_TOO_SMALL);

    assert_eq!(unsafe { opi_asr_finish(handle) }, 0);
    unsafe { opi_asr_destroy(handle) };
}

#[test]
fn feed_validates_arguments_and_null_handle_is_safe() {
    let config = c(r#"{"language":"zh"}"#);
    let handle = unsafe { opi_asr_create(config.as_ptr()) };
    assert!(!handle.is_null());

    let chunk = vec![0u8; MAX_FEED_BYTES + 1];
    assert_eq!(
        unsafe { opi_asr_feed_pcm(handle, chunk.as_ptr(), chunk.len() as i32) },
        ERR_ARGUMENT
    );
    assert_eq!(
        unsafe { opi_asr_feed_pcm(handle, ptr::null(), 2) },
        ERR_ARGUMENT
    );
    assert_eq!(
        unsafe { opi_asr_feed_pcm(handle, ptr::null(), -1) },
        ERR_ARGUMENT
    );
    assert_eq!(unsafe { opi_asr_start(ptr::null_mut()) }, ERR_ARGUMENT);
    assert_eq!(
        unsafe { opi_asr_poll_event(ptr::null_mut(), ptr::null_mut(), 0) },
        ERR_ARGUMENT
    );

    unsafe { opi_asr_destroy(handle) };
    unsafe { opi_asr_destroy(ptr::null_mut()) };
}

#[test]
fn verify_model_roundtrip() {
    let dir = temp_dir("verify");
    fs::write(dir.join("model.bin"), b"hello").unwrap();
    let sha = opi_asr::sha256_file(&dir.join("model.bin")).unwrap();
    let manifest = json!({
        "version": 1,
        "files": [{"path": "model.bin", "size": 5, "sha256": sha, "role": "model"}]
    })
    .to_string();

    let dir_c = c(dir.to_str().unwrap());
    let manifest_c = c(&manifest);
    let ptr = unsafe { opi_asr_verify_model(dir_c.as_ptr(), manifest_c.as_ptr()) };
    assert!(!ptr.is_null());
    let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
    unsafe { opi_asr_string_free(ptr) };
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["files"], 1);
    assert_eq!(value["bytes"], 5);

    fs::write(dir.join("model.bin"), b"hellx").unwrap();
    let ptr = unsafe { opi_asr_verify_model(dir_c.as_ptr(), manifest_c.as_ptr()) };
    assert!(!ptr.is_null());
    let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
    unsafe { opi_asr_string_free(ptr) };
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["ok"], false);
    assert!(
        value["error"].as_str().unwrap().contains("sha256 mismatch"),
        "{value}"
    );

    assert!(unsafe { opi_asr_verify_model(ptr::null(), manifest_c.as_ptr()) }.is_null());
    assert_eq!(opi_asr_max_event_bytes(), MAX_EVENT_BYTES as i32);
    fs::remove_dir_all(dir).unwrap();
}
