// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 文件/文件夹传输的公开值构造、路径校验与本地枚举。
//! 真正的分块收发状态机在 `session/transfer.rs`；这里只保证进入网络的元数据合法。

use crate::ClipError;
use crate::types::{
    MAX_TRANSFER_FILE_BYTES, MAX_TRANSFER_FILES, MAX_TRANSFER_PATH_BYTES, MAX_TRANSFER_TOTAL_BYTES,
    TRANSFER_CHUNK_BYTES, TransferFile, TransferFileInfo, TransferId,
};
use std::fs;
use std::path::{Path, PathBuf};

impl TransferFile {
    /// 从本地文件/目录构造；相对名取文件名，MIME 按扩展名猜。
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, ClipError> {
        let path = path.into();
        let name = file_name(&path)?;
        Self::from_path_named(path, name, None)
    }

    /// 从本地路径 + 自定义相对名 + 可选 MIME 构造。大小与目录标志取自文件系统，
    /// 不信任宿主传值（发送前还会再核对一次）。
    pub fn from_path_named(
        path: impl Into<PathBuf>,
        relative_path: impl Into<String>,
        mime: Option<&str>,
    ) -> Result<Self, ClipError> {
        let path = path.into();
        let relative_path = relative_path.into();
        validate_relative_path(&relative_path)?;
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(ClipError::invalid("symlinks are not transferred"));
        }
        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };
        if size > MAX_TRANSFER_FILE_BYTES {
            return Err(ClipError::TooLarge(size as usize));
        }
        let mime = mime.map_or_else(|| guess_mime(&path, is_dir), str::to_owned);
        if mime.len() > 128 {
            return Err(ClipError::invalid("mime is longer than 128 bytes"));
        }
        Ok(Self {
            source: path,
            info: TransferFileInfo {
                relative_path,
                size,
                mime,
                is_dir,
            },
        })
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn relative_path(&self) -> &str {
        &self.info.relative_path
    }

    pub fn size(&self) -> u64 {
        self.info.size
    }

    pub fn mime(&self) -> &str {
        &self.info.mime
    }

    pub fn is_dir(&self) -> bool {
        self.info.is_dir
    }

    pub fn info(&self) -> &TransferFileInfo {
        &self.info
    }
}

/// 枚举一组本地路径：文件按原名发送；目录递归展开为目录项 + 文件（相对路径以
/// 目录名开头）。符号链接跳过（目录内）或拒绝（显式传入）。
pub fn collect_files(paths: &[PathBuf]) -> Result<Vec<TransferFile>, ClipError> {
    let mut out = Vec::new();
    for path in paths {
        collect_one(path, None, None, &mut out)?;
    }
    check_collection(&out)?;
    Ok(out)
}

pub(crate) fn collect_one(
    path: &Path,
    relative: Option<&str>,
    mime: Option<&str>,
    out: &mut Vec<TransferFile>,
) -> Result<(), ClipError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(ClipError::invalid("symlinks are not transferred"));
    }
    if !metadata.is_dir() {
        let name = match relative {
            Some(name) => name.to_owned(),
            None => file_name(path)?,
        };
        out.push(TransferFile::from_path_named(path, name, mime)?);
        return Ok(());
    }

    let root = match relative {
        Some(name) => name.to_owned(),
        None => file_name(path)?,
    };
    validate_relative_path(&root)?;
    let root_meta = TransferFile::from_path_named(path, root.clone(), mime)?;
    out.push(root_meta);
    // 广度优先 + 目录内按名字排序：输出顺序稳定，宿主提示可预测。
    let mut queue = std::collections::VecDeque::from([(path.to_path_buf(), root)]);
    while let Some((dir, prefix)) = queue.pop_front() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let meta = fs::symlink_metadata(entry.path())?;
            if meta.file_type().is_symlink() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(ClipError::invalid("file name is not valid UTF-8"));
            };
            let relative = format!("{prefix}/{name}");
            if meta.is_dir() {
                out.push(TransferFile::from_path_named(
                    entry.path(),
                    &relative,
                    None,
                )?);
                queue.push_back((entry.path(), relative));
            } else {
                out.push(TransferFile::from_path_named(
                    entry.path(),
                    &relative,
                    None,
                )?);
            }
            if out.len() > MAX_TRANSFER_FILES {
                return Err(ClipError::invalid(format!(
                    "too many entries; limit is {MAX_TRANSFER_FILES}"
                )));
            }
            check_collection(out)?;
        }
    }
    Ok(())
}

fn check_collection(files: &[TransferFile]) -> Result<(), ClipError> {
    let infos: Vec<TransferFileInfo> = files.iter().map(|file| file.info.clone()).collect();
    validate_offer(&infos).map(|_| ())
}

fn file_name(path: &Path) -> Result<String, ClipError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| ClipError::invalid("path has no valid UTF-8 file name"))
}

/// 相对路径是唯一的安全边界：拒绝绝对路径、盘符、`..`/`.`/空组件与反斜杠。
pub(crate) fn validate_relative_path(path: &str) -> Result<(), ClipError> {
    if path.is_empty() {
        return Err(ClipError::invalid("empty relative path"));
    }
    if path.len() > MAX_TRANSFER_PATH_BYTES {
        return Err(ClipError::invalid(format!(
            "relative path is longer than {MAX_TRANSFER_PATH_BYTES} bytes"
        )));
    }
    if path.contains('\0') || path.contains('\\') || path.contains(':') {
        return Err(ClipError::invalid("path contains a forbidden character"));
    }
    if path.starts_with('/') {
        return Err(ClipError::invalid("absolute path is not allowed"));
    }
    if path
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ClipError::invalid("path traversal is not allowed"));
    }
    Ok(())
}

/// 接收端校验 offer 并返回总字节数；重复路径、目录带大小、超限都拒绝。
pub(crate) fn validate_offer(files: &[TransferFileInfo]) -> Result<u64, ClipError> {
    if files.is_empty() {
        return Err(ClipError::invalid("empty transfer offer"));
    }
    if files.len() > MAX_TRANSFER_FILES {
        return Err(ClipError::invalid(format!(
            "too many files; limit is {MAX_TRANSFER_FILES}"
        )));
    }
    let mut seen = std::collections::HashSet::with_capacity(files.len());
    let mut total = 0u64;
    for file in files {
        validate_relative_path(&file.relative_path)?;
        if !seen.insert(file.relative_path.as_str()) {
            return Err(ClipError::invalid("duplicate path in transfer offer"));
        }
        if file.mime.len() > 128 {
            return Err(ClipError::invalid("mime is longer than 128 bytes"));
        }
        if file.is_dir {
            if file.size != 0 {
                return Err(ClipError::invalid("directory entry with non-zero size"));
            }
        } else if file.size > MAX_TRANSFER_FILE_BYTES {
            return Err(ClipError::TooLarge(file.size as usize));
        }
        total = total
            .checked_add(file.size)
            .filter(|total| *total <= MAX_TRANSFER_TOTAL_BYTES)
            .ok_or_else(|| {
                ClipError::invalid(format!(
                    "transfer exceeds {MAX_TRANSFER_TOTAL_BYTES} bytes total"
                ))
            })?;
    }
    Ok(total)
}

pub(crate) fn random_id() -> Result<TransferId, ClipError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|e| ClipError::invalid(format!("random transfer id: {e}")))?;
    Ok(TransferId::from_bytes(bytes))
}

/// 发送端校验：所有条目仍指向可用文件，大小未变，总数/总量在限内。
pub(crate) fn validate_send(files: &[TransferFile]) -> Result<u64, ClipError> {
    let infos: Vec<TransferFileInfo> = files.iter().map(|file| file.info.clone()).collect();
    let total = validate_offer(&infos)?;
    for file in files {
        let metadata = fs::metadata(&file.source)?;
        if metadata.is_dir() != file.info.is_dir
            || (!metadata.is_dir() && metadata.len() != file.info.size)
        {
            return Err(ClipError::invalid(format!(
                "local file changed since it was selected: {}",
                file.info.relative_path
            )));
        }
    }
    Ok(total)
}

pub(crate) fn validate_chunk_len(len: usize) -> Result<(), ClipError> {
    if len == 0 || len > TRANSFER_CHUNK_BYTES {
        return Err(ClipError::invalid("transfer chunk size out of range"));
    }
    Ok(())
}

fn guess_mime(path: &Path, is_dir: bool) -> String {
    if is_dir {
        return String::new();
    }
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/aac",
        "wav" => "audio/wav",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "zip" => "application/zip",
        "apk" => "application/vnd.android.package-archive",
        _ => "application/octet-stream",
    };
    mime.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::temp_dir;

    #[test]
    fn relative_paths_reject_traversal_and_absolute() {
        for bad in [
            "",
            "..",
            "../etc/passwd",
            "a/../../b",
            "/etc/passwd",
            "\\windows\\system32",
            "C:/windows",
            "C:windows",
            "a//b",
            "./a",
            "a/./b",
            "a\\b",
            "a\0b",
        ] {
            assert!(
                validate_relative_path(bad).is_err(),
                "{bad:?} must be rejected"
            );
        }
        for good in ["a", "a/b", "照片/2026.png", ".hidden", "a/..b"] {
            assert!(validate_relative_path(good).is_ok(), "{good:?} must pass");
        }
        let long = "x".repeat(MAX_TRANSFER_PATH_BYTES + 1);
        assert!(validate_relative_path(&long).is_err());
    }

    #[test]
    fn offer_validation_enforces_limits_and_duplicates() {
        let file = |path: &str, size: u64| TransferFileInfo {
            relative_path: path.into(),
            size,
            mime: "text/plain".into(),
            is_dir: false,
        };
        assert!(validate_offer(&[]).is_err());
        assert!(validate_offer(&[file("a", 1), file("a", 2)]).is_err());
        assert!(validate_offer(&[file("../a", 1)]).is_err());
        assert!(
            validate_offer(&[TransferFileInfo {
                relative_path: "d".into(),
                size: 1,
                mime: String::new(),
                is_dir: true,
            }])
            .is_err()
        );
        assert_eq!(validate_offer(&[file("a", 3), file("b", 4)]).unwrap(), 7);
        let many: Vec<_> = (0..MAX_TRANSFER_FILES + 1)
            .map(|i| file(&format!("f{i}"), 0))
            .collect();
        assert!(validate_offer(&many).is_err());
    }

    #[test]
    fn collect_files_walks_directories_and_keeps_relative_paths() {
        let dir = temp_dir("collect");
        let root = dir.join("相册");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), b"alpha").unwrap();
        fs::write(root.join("sub/b.png"), b"beta").unwrap();
        let files = collect_files(std::slice::from_ref(&root)).unwrap();
        let paths: Vec<_> = files.iter().map(|f| f.relative_path()).collect();
        assert_eq!(
            paths,
            vec!["相册", "相册/a.txt", "相册/sub", "相册/sub/b.png"]
        );
        assert!(files[0].is_dir());
        assert_eq!(files[1].size(), 5);
        assert_eq!(files[2].mime(), "");
        assert_eq!(files[3].mime(), "image/png");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn single_file_gets_name_and_guessed_mime() {
        let dir = temp_dir("single");
        let path = dir.join("note.txt");
        fs::write(&path, b"hello").unwrap();
        let file = TransferFile::from_path(&path).unwrap();
        assert_eq!(file.relative_path(), "note.txt");
        assert_eq!(file.size(), 5);
        assert_eq!(file.mime(), "text/plain");
        assert!(TransferFile::from_path_named(&path, "custom", Some("text/x-note")).is_ok());
        assert!(TransferFile::from_path_named(&path, "../escape", None).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
