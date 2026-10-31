// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 长期身份：实例 ID + X25519 静态私钥，首次启动生成，文件权限 0600。

use crate::ClipError;
use crate::config::atomic_write;
use crate::crypto::fingerprint;
use crate::types::{Fingerprint, InstanceId};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const IDENTITY_FILE: &str = "identity.json";

#[derive(Serialize, Deserialize)]
struct IdentityFile {
    version: u8,
    instance_id: String,
    private_key: String,
}

pub(crate) struct Identity {
    pub instance_id: InstanceId,
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
    pub fingerprint: Fingerprint,
}

impl Identity {
    pub(crate) fn load_or_create(data_dir: &Path) -> Result<Self, ClipError> {
        let path = data_dir.join(IDENTITY_FILE);
        if path.exists() {
            let identity = Self::load(&path)?;
            tighten_permissions(&path);
            return Ok(identity);
        }

        let mut instance = [0u8; 16];
        let mut private = [0u8; 32];
        getrandom::fill(&mut instance)
            .map_err(|e| ClipError::invalid(format!("random instance id: {e}")))?;
        getrandom::fill(&mut private)
            .map_err(|e| ClipError::invalid(format!("random private key: {e}")))?;
        let identity = Self::from_private(InstanceId::from_bytes(instance), private);
        let file = IdentityFile {
            version: 1,
            instance_id: identity.instance_id.to_hex(),
            private_key: hex::encode(&identity.private_key),
        };
        let bytes = serde_json::to_vec(&file)
            .map_err(|e| ClipError::invalid(format!("identity encode: {e}")))?;
        atomic_write(&path, &bytes, 0o600)?;
        Ok(identity)
    }

    fn load(path: &Path) -> Result<Self, ClipError> {
        let bytes = fs::read(path)?;
        let file: IdentityFile = serde_json::from_slice(&bytes)
            .map_err(|e| ClipError::invalid(format!("identity file: {e}")))?;
        if file.version != 1 {
            return Err(ClipError::invalid("unsupported identity version"));
        }
        let instance_id = InstanceId::from_hex(&file.instance_id)?;
        let private = hex::decode(file.private_key)
            .map_err(|e| ClipError::invalid(format!("private key hex: {e}")))?;
        let private: [u8; 32] = private
            .try_into()
            .map_err(|_| ClipError::invalid("private key must be 32 bytes"))?;
        Ok(Self::from_private(instance_id, private))
    }

    fn from_private(instance_id: InstanceId, private: [u8; 32]) -> Self {
        let secret = x25519_dalek::StaticSecret::from(private);
        let public = x25519_dalek::PublicKey::from(&secret);
        let public_key = public.as_bytes().to_vec();
        Self {
            instance_id,
            fingerprint: fingerprint(&public_key),
            public_key,
            private_key: private.to_vec(),
        }
    }
}

fn tighten_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::temp_dir;

    #[test]
    fn identity_roundtrip_stable() {
        let dir = temp_dir("identity");
        let a = Identity::load_or_create(&dir).unwrap();
        let b = Identity::load_or_create(&dir).unwrap();
        assert_eq!(a.instance_id, b.instance_id);
        assert_eq!(a.private_key, b.private_key);
        assert_eq!(a.public_key, b.public_key);
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.private_key.len(), 32);
        assert_eq!(a.public_key.len(), 32);
        let other = Identity::load_or_create(&temp_dir("identity-other")).unwrap();
        assert_ne!(a.instance_id, other.instance_id);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn identity_file_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("identity-mode");
        Identity::load_or_create(&dir).unwrap();
        let mode = fs::metadata(dir.join(IDENTITY_FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let _ = fs::remove_dir_all(dir);
    }
}
