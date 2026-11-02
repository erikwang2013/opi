// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 文件传输的 C 出口（从 `cabi_clip.rs` 拆出守 500 行）。
//!
//! 文件清单 JSON：`[{"path":"/abs/local","name":"可选/相对名","mime":"可选"}]`。
//! 目录条目只能给 `path`（递归展开，相对名取目录名）；`name`/`mime` 只对文件有效。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use opi_clip::{MAX_TRANSFER_FILES, TransferFile, collect_files};

use super::OpiClip;
use crate::cabi::OpiString;
use crate::cabi::utf16::read_utf16;

impl OpiClip {
    /// 返回本次传输 ID 的十六进制串。
    pub(crate) fn send_files(&self, peer_hex: &str, files_json: &str) -> Result<String, String> {
        self.check_owner()?;
        let peer = super::parse_peer(peer_hex)?;
        let files = parse_files(files_json)?;
        let id = self
            .node()?
            .send_files(peer, files)
            .map_err(|e| e.to_string())?;
        Ok(id.to_hex())
    }

    /// `save_dir` 为空 = 拒绝（accept=true 时内核也会拒绝缺目录）。
    pub(crate) fn respond_transfer(
        &self,
        id_hex: &str,
        accept: bool,
        save_dir: &str,
    ) -> Result<bool, String> {
        self.check_owner()?;
        let id = opi_clip::TransferId::from_hex(id_hex.trim()).map_err(|e| e.to_string())?;
        let save_dir = (!save_dir.is_empty()).then(|| PathBuf::from(save_dir));
        self.node()?
            .respond_transfer(id, accept, save_dir)
            .map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub(crate) fn cancel_transfer(&self, id_hex: &str) -> Result<bool, String> {
        self.check_owner()?;
        let id = opi_clip::TransferId::from_hex(id_hex.trim()).map_err(|e| e.to_string())?;
        self.node()?
            .cancel_transfer(id)
            .map_err(|e| e.to_string())?;
        Ok(true)
    }
}

pub(super) fn parse_files(json: &str) -> Result<Vec<TransferFile>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("files JSON: {e}"))?;
    let specs = value
        .as_array()
        .ok_or_else(|| "files JSON must be an array".to_string())?;
    if specs.is_empty() {
        return Err("files JSON array is empty".into());
    }
    if specs.len() > MAX_TRANSFER_FILES {
        return Err(format!("too many entries; limit is {MAX_TRANSFER_FILES}"));
    }
    let mut files = Vec::new();
    for spec in specs {
        let path = spec
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|path| !path.is_empty())
            .ok_or_else(|| "each entry needs a non-empty string `path`".to_string())?;
        let name = optional_string(spec, "name")?;
        let mime = optional_string(spec, "mime")?;
        let path = PathBuf::from(path);
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if metadata.is_dir() {
            if name.is_some() || mime.is_some() {
                return Err("`name`/`mime` are only valid for file entries".into());
            }
            files.extend(collect_files(std::slice::from_ref(&path)).map_err(|e| e.to_string())?);
        } else {
            let file = match (name, mime) {
                (None, None) => TransferFile::from_path(&path),
                (name, mime) => {
                    let name = match name {
                        Some(name) => name.to_owned(),
                        None => path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .ok_or_else(|| "path has no UTF-8 file name".to_string())?
                            .to_owned(),
                    };
                    TransferFile::from_path_named(&path, name, mime)
                }
            }
            .map_err(|e| e.to_string())?;
            files.push(file);
        }
        if files.len() > MAX_TRANSFER_FILES {
            return Err(format!("too many entries; limit is {MAX_TRANSFER_FILES}"));
        }
    }
    Ok(files)
}

fn optional_string<'a>(spec: &'a serde_json::Value, key: &str) -> Result<Option<&'a str>, String> {
    match spec.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(format!("`{key}` must be a string")),
    }
}

// ---------- C 导出 ----------

/// sendFiles(clip, peer_hex, files_json) -> 传输 ID 十六进制；失败 → 空串。
/// # Safety
/// `peer_id` / `files_json` 必须各指向至少对应 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_send_files(
    clip: *mut OpiClip,
    peer_id: *const u16,
    peer_len: usize,
    files_json: *const u16,
    files_len: usize,
) -> OpiString {
    let id = super::settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer =
                unsafe { read_utf16(peer_id, peer_len) }.ok_or("peer id is not valid UTF-16")?;
            let files = unsafe { read_utf16(files_json, files_len) }
                .ok_or("files JSON is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.send_files(&peer, &files)
        })),
        String::new(),
    );
    OpiString::from_utf16(&id)
}

/// respondTransfer(clip, transfer_hex, accept, save_dir) -> bool；接受时 save_dir 必填。
/// # Safety
/// `transfer_id` / `save_dir` 必须各指向至少对应 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_respond_transfer(
    clip: *mut OpiClip,
    transfer_id: *const u16,
    id_len: usize,
    accept: bool,
    save_dir: *const u16,
    save_dir_len: usize,
) -> bool {
    super::settle(
        catch_unwind(AssertUnwindSafe(|| {
            let id = unsafe { read_utf16(transfer_id, id_len) }
                .ok_or("transfer id is not valid UTF-16")?;
            let dir = unsafe { read_utf16(save_dir, save_dir_len) }
                .ok_or("save_dir is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.respond_transfer(&id, accept, &dir)
        })),
        false,
    )
}

/// cancelTransfer(clip, transfer_hex) -> bool。
/// # Safety
/// `transfer_id` 必须指向至少 `id_len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clip_cancel_transfer(
    clip: *mut OpiClip,
    transfer_id: *const u16,
    id_len: usize,
) -> bool {
    super::settle(
        catch_unwind(AssertUnwindSafe(|| {
            let id = unsafe { read_utf16(transfer_id, id_len) }
                .ok_or("transfer id is not valid UTF-16")?;
            unsafe { OpiClip::borrow(clip) }?.cancel_transfer(&id)
        })),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opi-ffi-files-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_specs_build_metadata_and_reject_escape_names() {
        let dir = dir("parse");
        let path = dir.join("note.txt");
        fs::write(&path, b"hello").unwrap();
        let path = path.to_str().unwrap();
        let ok = serde_json::to_string(&json!([{"path": path}])).unwrap();
        let files = parse_files(&ok).unwrap();
        assert_eq!(files[0].relative_path(), "note.txt");
        assert_eq!(files[0].size(), 5);
        assert_eq!(files[0].mime(), "text/plain");

        for bad in [
            json!([{"path": path, "name": "../escape"}]),
            json!([{"path": path, "name": "/abs"}]),
            json!([{"path": path, "name": ""}]),
            json!([{"path": ""}]),
            json!([{"path": path, "mime": 1}]),
        ] {
            let json = serde_json::to_string(&bad).unwrap();
            assert!(parse_files(&json).is_err(), "{json} 必须被拒");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn directory_specs_expand_with_relative_paths() {
        let dir = dir("walk");
        let root = dir.join("相册");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), b"alpha").unwrap();
        fs::write(root.join("sub/b.png"), b"beta").unwrap();
        let spec = serde_json::to_string(&json!([{"path": root}])).unwrap();
        let paths: Vec<_> = parse_files(&spec)
            .unwrap()
            .iter()
            .map(|file| file.relative_path().to_owned())
            .collect();
        assert_eq!(paths, ["相册", "相册/a.txt", "相册/sub", "相册/sub/b.png"]);
        assert!(
            parse_files(&serde_json::to_string(&json!([{"path": root, "name": "x"}])).unwrap())
                .is_err(),
            "目录条目不接受 name"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
