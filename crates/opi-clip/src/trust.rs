// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 已配对公钥表（TOFU + SAS 人工确认）。只存公钥，不存任何共享密钥。

use crate::ClipError;
use crate::config::atomic_write;
use crate::crypto::fingerprint;
use crate::types::InstanceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub(crate) struct TrustedPeer {
    pub instance_id: InstanceId,
    pub name: String,
    pub public_key: Vec<u8>,
    pub fingerprint: crate::types::Fingerprint,
    pub paired_at: u64,
}

#[derive(Default)]
pub(crate) struct TrustStore {
    peers: HashMap<InstanceId, TrustedPeer>,
}

#[derive(Serialize, Deserialize)]
struct TrustFile {
    version: u8,
    peers: Vec<TrustedPeerFile>,
}

#[derive(Serialize, Deserialize)]
struct TrustedPeerFile {
    instance_id: String,
    public_key: String,
    name: String,
    paired_at: u64,
}

impl TrustStore {
    pub(crate) fn load(path: &Path) -> Result<Self, ClipError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path)?;
        let file: TrustFile =
            serde_json::from_slice(&bytes).map_err(|e| ClipError::Trust(format!("parse: {e}")))?;
        if file.version != 1 {
            return Err(ClipError::Trust("unsupported trust store version".into()));
        }
        let mut peers = HashMap::with_capacity(file.peers.len());
        for entry in file.peers {
            let instance_id = InstanceId::from_hex(&entry.instance_id)
                .map_err(|e| ClipError::Trust(format!("instance id: {e}")))?;
            let public_key = hex::decode(&entry.public_key)
                .map_err(|e| ClipError::Trust(format!("public key hex: {e}")))?;
            if public_key.len() != 32 {
                return Err(ClipError::Trust("public key must be 32 bytes".into()));
            }
            if peers.contains_key(&instance_id) {
                return Err(ClipError::Trust("duplicate trusted peer".into()));
            }
            let fingerprint = fingerprint(&public_key);
            peers.insert(
                instance_id,
                TrustedPeer {
                    instance_id,
                    name: entry.name,
                    public_key,
                    fingerprint,
                    paired_at: entry.paired_at,
                },
            );
        }
        Ok(Self { peers })
    }

    pub(crate) fn save(&self, path: &Path) -> Result<(), ClipError> {
        let mut ids: Vec<_> = self.peers.keys().copied().collect();
        ids.sort();
        let peers = ids
            .into_iter()
            .filter_map(|id| self.peers.get(&id))
            .map(|peer| TrustedPeerFile {
                instance_id: peer.instance_id.to_hex(),
                public_key: hex::encode(&peer.public_key),
                name: peer.name.clone(),
                paired_at: peer.paired_at,
            })
            .collect();
        let file = TrustFile { version: 1, peers };
        let bytes =
            serde_json::to_vec(&file).map_err(|e| ClipError::Trust(format!("encode: {e}")))?;
        atomic_write(path, &bytes, 0o600).map_err(|e| ClipError::Trust(e.to_string()))?;
        Ok(())
    }

    pub(crate) fn get(&self, id: InstanceId) -> Option<&TrustedPeer> {
        self.peers.get(&id)
    }

    pub(crate) fn contains(&self, id: InstanceId) -> bool {
        self.peers.contains_key(&id)
    }

    pub(crate) fn insert(&mut self, peer: TrustedPeer) {
        self.peers.insert(peer.instance_id, peer);
    }

    pub(crate) fn remove(&mut self, id: InstanceId) -> bool {
        self.peers.remove(&id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::temp_dir;

    #[test]
    fn trust_roundtrip() {
        let dir = temp_dir("trust");
        let path = dir.join("trust.json");
        let mut store = TrustStore::load(&path).unwrap();
        let id = InstanceId::from_bytes([9; 16]);
        let key = vec![3u8; 32];
        store.insert(TrustedPeer {
            instance_id: id,
            name: "phone".into(),
            fingerprint: fingerprint(&key),
            public_key: key,
            paired_at: 123,
        });
        store.save(&path).unwrap();
        let loaded = TrustStore::load(&path).unwrap();
        let peer = loaded.get(id).unwrap();
        assert_eq!(peer.name, "phone");
        assert_eq!(peer.public_key, vec![3u8; 32]);
        assert_eq!(peer.fingerprint, fingerprint(&peer.public_key));
        assert!(loaded.contains(id));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_trust_file_is_empty() {
        let dir = temp_dir("trust-missing");
        assert!(
            TrustStore::load(&dir.join("none.json"))
                .unwrap()
                .peers
                .is_empty()
        );
        let _ = fs::remove_dir_all(dir);
    }
}
