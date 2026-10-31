// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `cabi_clip` 的无网络单测：配置解析、线程归属、事件 JSON 转义/上限。
//! start 要绑端口，真链路留给 `tests/c_abi/consumer_clip.c`（run.sh 执行）。

use super::*;
use opi_clip::{ClipEvent, EventId, Version};
use serde_json::Value;

fn config_json(dir: &std::path::Path) -> String {
    format!(
        r#"{{"data_dir":"{}","tcp_port":0,"discovery_port":46899,"device_name":"ffi-test"}}"#,
        dir.display()
    )
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("opi-ffi-clip-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn create_rejects_bad_config() {
    assert!(OpiClip::create("").is_err());
    assert!(OpiClip::create("{}").is_err(), "缺 data_dir 应报错");
    assert!(OpiClip::create(r#"{"data_dir":""}"#).is_err());
    assert!(OpiClip::create("not json").is_err());
}

#[test]
fn status_before_start_and_release() {
    let dir = temp_dir("lifecycle");
    let raw = OpiClip::create(&config_json(&dir)).unwrap();
    let status = {
        let clip = unsafe { OpiClip::borrow(raw) }.unwrap();
        assert_eq!(clip.peers_json().unwrap(), "[]");
        clip.status_json().unwrap()
    };
    let status: Value = serde_json::from_str(&status).unwrap();
    assert_eq!(status["running"], false);
    assert_eq!(status["device_name"], "ffi-test");
    assert_eq!(status["peers"], 0);
    assert_eq!(status["instance_id"], Value::Null);
    unsafe { OpiClip::free(raw) }.unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn handle_is_rejected_on_another_thread() {
    let dir = temp_dir("thread");
    let raw = OpiClip::create(&config_json(&dir)).unwrap();
    let addr = raw as usize;
    let result = std::thread::spawn(move || {
        let clip = unsafe { OpiClip::borrow(addr as *mut OpiClip) }.unwrap();
        clip.status_json()
    })
    .join()
    .unwrap();
    assert!(
        result.unwrap_err().contains("non-owner thread"),
        "跨线程调用必须被拒绝"
    );
    unsafe { OpiClip::free(raw) }.unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn event_json_escapes_text_and_enforces_1mib_cap() {
    let text = "引号\" 换行\n 控制\u{1} 😄";
    let json = encode_event(ClipEvent::ClipboardReceived {
        instance_id: InstanceId::from_bytes([7; 16]),
        text: text.into(),
        version: Version {
            wall_ms: 1,
            counter: 2,
            instance_id: InstanceId::from_bytes([7; 16]),
            event_id: EventId::from_bytes([9; 16]),
        },
    })
    .unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["type"], "clipboard_received");
    assert_eq!(value["text"], text, "转义后必须能原样解码");

    // 256 KiB 的 U+0001 JSON 转义后 > 1 MiB，必须被拒（防单事件无界膨胀）。
    let huge = "\u{1}".repeat(256 * 1024);
    let err = encode_event(ClipEvent::Error { message: huge }).unwrap_err();
    assert!(err.contains("limit is"), "超过 1 MiB 必须报错：{err}");
}
