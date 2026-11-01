use crate::error::{AsrError, io};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_FILE: &str = "manifest.json";
pub const MANIFEST_VERSION: u32 = 1;

/// Expected sha256 values are lowercase hex; absolute model directory,
/// so `path` must stay inside it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub version: u32,
    pub files: Vec<ModelFile>,
}

/// `role` is what the engine binds to a path: sensevoice uses `model` + `tokens`,
/// whisper uses `encoder` + `decoder` + `tokens`. Names on disk are free.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub files: usize,
    pub bytes: u64,
}

impl ModelManifest {
    pub fn from_json(json: &str) -> Result<Self, AsrError> {
        let manifest: Self =
            serde_json::from_str(json).map_err(|e| AsrError::Model(format!("manifest: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), AsrError> {
        if self.version != MANIFEST_VERSION {
            return Err(AsrError::Model(format!(
                "unsupported manifest version {}, expected {MANIFEST_VERSION}",
                self.version
            )));
        }
        if self.files.is_empty() {
            return Err(AsrError::Model("manifest has no files".into()));
        }
        let mut paths = HashSet::new();
        let mut roles = HashSet::new();
        for f in &self.files {
            if !paths.insert(f.path.as_str()) {
                return Err(AsrError::Model(format!("duplicate path: {}", f.path)));
            }
            if let Some(role) = &f.role
                && role.is_empty()
            {
                return Err(AsrError::Model(format!("empty role for {}", f.path)));
            }
            if let Some(role) = &f.role
                && !roles.insert(role.as_str())
            {
                return Err(AsrError::Model(format!("duplicate role: {role}")));
            }
            if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(AsrError::Model(format!(
                    "sha256 must be 64 hex chars: {}",
                    f.path
                )));
            }
        }
        Ok(())
    }

    pub fn role_path(&self, dir: &Path, role: &str) -> Result<PathBuf, AsrError> {
        let file = self
            .files
            .iter()
            .find(|f| f.role.as_deref() == Some(role))
            .ok_or_else(|| AsrError::Model(format!("manifest role missing: {role}")))?;
        safe_join(dir, &file.path)
    }
}

/// Check every file is real, inside `dir`, has the declared size, and (unless
/// `check_hash` is false) the declared SHA-256.
pub fn verify(
    dir: &Path,
    manifest: &ModelManifest,
    check_hash: bool,
) -> Result<VerifyReport, AsrError> {
    manifest.validate()?;
    let canon_dir = dir
        .canonicalize()
        .map_err(|e| AsrError::Model(format!("model dir {}: {e}", dir.display())))?;
    let mut bytes = 0u64;
    for f in &manifest.files {
        let path = safe_join(&canon_dir, &f.path)?;
        let meta = std::fs::metadata(&path).map_err(|e| io(&path, e))?;
        if !meta.is_file() {
            return Err(AsrError::Model(format!("not a file: {}", path.display())));
        }
        if meta.len() != f.size {
            return Err(AsrError::Model(format!(
                "size mismatch for {}: manifest {} bytes, disk {} bytes",
                f.path,
                f.size,
                meta.len()
            )));
        }
        if check_hash {
            let got = sha256_file(&path)?;
            if !got.eq_ignore_ascii_case(&f.sha256) {
                return Err(AsrError::Model(format!(
                    "sha256 mismatch for {}: manifest {}, disk {got}",
                    f.path, f.sha256
                )));
            }
        }
        bytes = bytes.saturating_add(f.size);
    }
    Ok(VerifyReport {
        files: manifest.files.len(),
        bytes,
    })
}

pub fn sha256_file(path: &Path) -> Result<String, AsrError> {
    let mut file = File::open(path).map_err(|e| io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).map_err(|e| io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn safe_join(dir: &Path, rel: &str) -> Result<PathBuf, AsrError> {
    let rel_path = Path::new(rel);
    if rel_path.as_os_str().is_empty()
        || rel_path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(AsrError::Model(format!(
            "unsafe manifest path: {rel:?} (relative, no '.' / '..' allowed)"
        )));
    }
    let joined = dir.join(rel_path);
    let canon = joined
        .canonicalize()
        .map_err(|e| AsrError::Model(format!("missing model file {}: {e}", joined.display())))?;
    let canon_dir = dir
        .canonicalize()
        .map_err(|e| AsrError::Model(format!("model dir {}: {e}", dir.display())))?;
    if !canon.starts_with(&canon_dir) {
        return Err(AsrError::Model(format!(
            "manifest path escapes model dir: {rel}"
        )));
    }
    Ok(canon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_paths_and_bad_manifest() {
        let dir = std::env::temp_dir();
        for bad in ["", ".", "..", "../x", "/etc/passwd", "a/../../x"] {
            assert!(safe_join(&dir, bad).is_err(), "{bad:?} accepted");
        }
        assert!(ModelManifest::from_json("{}").is_err());
        assert!(
            ModelManifest::from_json(
                r#"{"version":1,"files":[{"path":"a","size":1,"sha256":"zz"}]}"#
            )
            .is_err()
        );
        assert!(
            ModelManifest::from_json(&format!(
                r#"{{"version":1,"files":[{{"path":"a","size":1,"sha256":"{}","role":"model"}},{{"path":"b","size":1,"sha256":"{}","role":"model"}}]}}"#,
                "0".repeat(64),
                "0".repeat(64)
            ))
            .is_err()
        );
    }
}
