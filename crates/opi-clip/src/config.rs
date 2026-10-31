// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 配置与所有持久化文件的原子写。剪贴板内容不落盘，只有身份、配置、信任表。

use crate::ClipError;
use crate::types::{DEFAULT_DISCOVERY_PORT, now_ms};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipConfig {
    /// 身份/信任/配置文件的目录（由各端传入应用私有目录）。
    pub data_dir: PathBuf,
    /// 0 表示由系统分配临时端口；beacon 里通告实际端口。
    #[serde(default)]
    pub tcp_port: u16,
    #[serde(default = "default_discovery_port")]
    pub discovery_port: u16,
    /// 仅用于配对界面显示与信任表记录，不参与发现广播。
    #[serde(default)]
    pub device_name: String,
}

fn default_discovery_port() -> u16 {
    DEFAULT_DISCOVERY_PORT
}

impl ClipConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            tcp_port: 0,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            device_name: String::from("Opi"),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ClipError> {
        if self.data_dir.as_os_str().is_empty() {
            return Err(ClipError::invalid("data_dir is empty"));
        }
        if self.discovery_port == 0 {
            return Err(ClipError::invalid("discovery_port must be non-zero"));
        }
        if self.device_name.len() > 64 {
            return Err(ClipError::invalid("device_name must be at most 64 bytes"));
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, ClipError> {
        let bytes = fs::read(path)?;
        let config: Self = serde_json::from_slice(&bytes)
            .map_err(|e| ClipError::invalid(format!("config file: {e}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), ClipError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|e| ClipError::invalid(format!("config encode: {e}")))?;
        atomic_write(path, &bytes, 0o600)?;
        Ok(())
    }
}

/// 同目录临时文件 → fsync → rename，避免掉电留下半截身份/信任表。
/// 目录 fsync 失败不报错：rename 已完成，文件内容至少是完整的。
pub(crate) fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let nonce = format!("{}.{}", std::process::id(), now_ms());
    let tmp = parent.join(format!(".{}.tmp.{nonce}", file_name.to_string_lossy()));

    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        #[cfg(not(unix))]
        let _ = mode;

        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
        }
        fs::rename(&tmp, path)?;
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
pub(crate) fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("opi-clip-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip_and_mode() {
        let dir = temp_dir("config");
        let path = dir.join("config.json");
        let config = ClipConfig {
            data_dir: dir.clone(),
            tcp_port: 0,
            discovery_port: 45999,
            device_name: "书房".into(),
        };
        config.save(&path).unwrap();
        assert_eq!(ClipConfig::load(&path).unwrap(), config);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn config_rejects_zero_discovery_port() {
        let mut config = ClipConfig::new("/tmp/x");
        config.discovery_port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn atomic_write_replaces_content() {
        let dir = temp_dir("atomic");
        let path = dir.join("f");
        atomic_write(&path, b"one", 0o600).unwrap();
        atomic_write(&path, b"two", 0o600).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        let _ = fs::remove_dir_all(dir);
    }
}
