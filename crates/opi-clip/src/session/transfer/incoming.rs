// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 接收侧：offer 登记、宿主确认、分块校验、SHA-256 校验与 `.part` → rename。
//! 发送侧留在 `session/transfer.rs`；两半都跑在同一个会话线程里，无锁竞争。

use super::{Incoming, PendingOffer, Session, cleanup};
use crate::ClipError;
use crate::transfer;
use crate::types::{ClipEvent, TransferFileInfo, TransferId};
use crate::wire::Wire;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

impl Session {
    pub(crate) fn respond_incoming(
        &mut self,
        id: TransferId,
        accept: bool,
        save_dir: Option<PathBuf>,
    ) -> Result<(), ClipError> {
        let Some(offer) = self.offer.take().filter(|offer| offer.id == id) else {
            self.fail_incoming(id, "transfer offer is no longer pending".into());
            return Ok(());
        };
        if !accept {
            let _ = self.send(&Wire::TransferDecision {
                transfer_id: id,
                accept: false,
                reason: Some("declined by receiver".into()),
            });
            self.emit_canceled(id, false);
            self.unregister(id);
            return Ok(());
        }
        if self.incoming.is_some() {
            let _ = self.send(&Wire::TransferDecision {
                transfer_id: id,
                accept: false,
                reason: Some("another incoming transfer is active".into()),
            });
            self.fail_incoming(id, "another incoming transfer is active".into());
            return Ok(());
        }
        let Some(save_dir) = save_dir else {
            let _ = self.send(&Wire::TransferDecision {
                transfer_id: id,
                accept: false,
                reason: Some("save_dir is required".into()),
            });
            self.fail_incoming(id, "save_dir is required".into());
            return Ok(());
        };
        let parts = match prepare(&offer.files, &save_dir) {
            Ok(parts) => parts,
            Err(reason) => {
                let _ = self.send(&Wire::TransferDecision {
                    transfer_id: id,
                    accept: false,
                    reason: Some(reason.clone()),
                });
                self.fail_incoming(id, reason);
                return Ok(());
            }
        };
        // 先安装接收状态再发决策：即使发送失败，会话退出时也能清理 .part。
        self.incoming = Some(Incoming {
            id,
            files: offer.files,
            save_dir,
            parts,
            total: offer.total,
            received: 0,
            current: 0,
            offset: 0,
            file: None,
            hasher: Sha256::new(),
            last_progress: Instant::now(),
            last_progress_bytes: 0,
        });
        self.send(&Wire::TransferDecision {
            transfer_id: id,
            accept: true,
            reason: None,
        })
    }

    pub(crate) fn handle_transfer(&mut self, message: Wire) -> Result<(), ClipError> {
        match message {
            Wire::TransferOffer { transfer_id, files } => self.handle_offer(transfer_id, files),
            Wire::TransferDecision {
                transfer_id,
                accept,
                reason,
            } => self.handle_decision(transfer_id, accept, reason),
            Wire::TransferChunk {
                transfer_id,
                file_index,
                offset,
                bytes,
            } => self.handle_chunk(transfer_id, file_index, offset, bytes),
            Wire::TransferDone {
                transfer_id,
                sha256,
            } => self.handle_done(transfer_id, sha256),
            Wire::TransferCancel { transfer_id } => {
                self.handle_cancel(transfer_id);
                Ok(())
            }
            Wire::TransferError {
                transfer_id,
                message,
            } => {
                self.handle_error(transfer_id, message);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn handle_offer(
        &mut self,
        id: TransferId,
        files: Vec<TransferFileInfo>,
    ) -> Result<(), ClipError> {
        if !self.inner.lock().trust.contains(self.peer) {
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: "peer is not trusted".into(),
            });
            return Ok(());
        }
        if self.offer.is_some() || self.incoming.is_some() {
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: "another incoming transfer is active".into(),
            });
            return Ok(());
        }
        let total = match transfer::validate_offer(&files) {
            Ok(total) => total,
            Err(error) => {
                let _ = self.send(&Wire::TransferError {
                    transfer_id: id,
                    message: error.to_string(),
                });
                return Ok(());
            }
        };
        let registered = self.inner.lock().offer_transfer(id, self.peer);
        if let Err(error) = registered {
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: error.to_string(),
            });
            return Ok(());
        }
        self.inner.emit(ClipEvent::TransferOffer {
            instance_id: self.peer,
            transfer_id: id,
            files: files.clone(),
            total_bytes: total,
            outgoing: false,
        });
        self.offer = Some(PendingOffer { id, files, total });
        Ok(())
    }

    fn handle_chunk(
        &mut self,
        id: TransferId,
        file_index: u16,
        offset: u64,
        bytes: Vec<u8>,
    ) -> Result<(), ClipError> {
        let Some(mut incoming) = self.incoming.take() else {
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: "unknown transfer".into(),
            });
            return Ok(());
        };
        if incoming.id != id {
            self.incoming = Some(incoming);
            return Ok(());
        }
        if let Err(reason) = incoming.apply_chunk(file_index, offset, &bytes) {
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: reason.clone(),
            });
            cleanup(&incoming);
            self.fail_incoming(id, reason);
            return Ok(());
        }
        let index = usize::from(file_index);
        let sent = incoming.received;
        let total = incoming.total;
        let count = incoming.files.len() as u16;
        self.maybe_progress(
            id,
            sent,
            total,
            index as u16,
            count,
            false,
            &mut incoming.last_progress,
            &mut incoming.last_progress_bytes,
        );
        self.incoming = Some(incoming);
        Ok(())
    }

    fn handle_done(&mut self, id: TransferId, sha256: Option<[u8; 32]>) -> Result<(), ClipError> {
        if let Some(outgoing) = self.outgoing.take() {
            if outgoing.id != id || !matches!(outgoing.phase, super::OutgoingPhase::Done) {
                self.outgoing = Some(outgoing);
                return Ok(());
            }
            if let (Some(expected), Some(actual)) = (sha256, outgoing.hash)
                && expected != actual
            {
                self.fail_outgoing(id, "peer reported a different SHA-256".into());
                return Ok(());
            }
            self.inner.emit(ClipEvent::TransferCompleted {
                instance_id: self.peer,
                transfer_id: id,
                files: outgoing.infos.clone(),
                total_bytes: outgoing.total,
                outgoing: true,
                saved_dir: None,
            });
            self.unregister(id);
            return Ok(());
        }

        let Some(mut incoming) = self.incoming.take() else {
            return Ok(());
        };
        if incoming.id != id {
            self.incoming = Some(incoming);
            return Ok(());
        }
        if !incoming.all_files_done() {
            let reason = "transfer ended before all file bytes arrived".to_string();
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: reason.clone(),
            });
            cleanup(&incoming);
            self.fail_incoming(id, reason);
            return Ok(());
        }
        let mut actual = [0u8; 32];
        actual.copy_from_slice(&incoming.hasher.clone().finalize());
        if let Some(expected) = sha256
            && expected != actual
        {
            let reason = "SHA-256 mismatch".to_string();
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: reason.clone(),
            });
            cleanup(&incoming);
            self.fail_incoming(id, reason);
            return Ok(());
        }
        incoming.file = None;
        if let Err(error) = finish_incoming(&incoming) {
            let reason = format!("finalize files: {error}");
            let _ = self.send(&Wire::TransferError {
                transfer_id: id,
                message: reason.clone(),
            });
            cleanup(&incoming);
            self.fail_incoming(id, reason);
            return Ok(());
        }
        self.inner.emit(ClipEvent::TransferCompleted {
            instance_id: self.peer,
            transfer_id: id,
            files: incoming.files.clone(),
            total_bytes: incoming.total,
            outgoing: false,
            saved_dir: Some(incoming.save_dir.clone()),
        });
        self.unregister(id);
        self.send(&Wire::TransferDone {
            transfer_id: id,
            sha256,
        })?;
        Ok(())
    }

    fn handle_cancel(&mut self, id: TransferId) {
        if let Some(outgoing) = self.outgoing.take() {
            if outgoing.id == id {
                self.emit_canceled(id, true);
                self.unregister(id);
                return;
            }
            self.outgoing = Some(outgoing);
        }
        if let Some(incoming) = self.incoming.take() {
            if incoming.id == id {
                cleanup(&incoming);
                self.emit_canceled(id, false);
                self.unregister(id);
                return;
            }
            self.incoming = Some(incoming);
        }
        if self.offer.as_ref().is_some_and(|offer| offer.id == id) {
            self.offer = None;
            self.emit_canceled(id, false);
            self.unregister(id);
        }
    }

    fn handle_error(&mut self, id: TransferId, reason: String) {
        if let Some(outgoing) = self.outgoing.take() {
            if outgoing.id == id {
                self.fail_outgoing(id, reason);
                return;
            }
            self.outgoing = Some(outgoing);
        }
        if let Some(incoming) = self.incoming.take() {
            if incoming.id == id {
                cleanup(&incoming);
                self.fail_incoming(id, reason);
                return;
            }
            self.incoming = Some(incoming);
        }
        if self.offer.as_ref().is_some_and(|offer| offer.id == id) {
            self.offer = None;
            self.fail_incoming(id, reason);
        }
    }
}

impl Incoming {
    fn apply_chunk(&mut self, file_index: u16, offset: u64, bytes: &[u8]) -> Result<(), String> {
        transfer::validate_chunk_len(bytes.len()).map_err(|e| e.to_string())?;
        let index = usize::from(file_index);
        if index >= self.files.len() {
            return Err("file index out of range".into());
        }
        while self.current < index {
            let file = &self.files[self.current];
            if file.is_dir || file.size == 0 {
                self.current += 1;
                self.offset = 0;
            } else {
                return Err("missing chunks for an earlier file".into());
            }
        }
        if self.current != index {
            return Err("chunk file index mismatch".into());
        }
        let file = &self.files[index];
        if file.is_dir || file.size == 0 {
            return Err("chunk for an empty file or directory".into());
        }
        if self.offset != offset {
            return Err("chunk offset mismatch".into());
        }
        let end = offset
            .checked_add(bytes.len() as u64)
            .filter(|end| *end <= file.size)
            .ok_or_else(|| "chunk exceeds declared file size".to_string())?;
        if self.file.is_none() {
            let part = self.parts[index]
                .as_ref()
                .ok_or_else(|| "missing .part path".to_string())?;
            self.file = Some(
                OpenOptions::new()
                    .write(true)
                    .open(part)
                    .map_err(|e| format!("open part: {e}"))?,
            );
        }
        let file_handle = self.file.as_mut().expect("opened above");
        file_handle
            .write_all(bytes)
            .map_err(|e| format!("write part: {e}"))?;
        self.hasher.update(bytes);
        self.received += bytes.len() as u64;
        self.offset = end;
        if end == file.size {
            self.file = None;
            self.current += 1;
            self.offset = 0;
        }
        Ok(())
    }

    fn all_files_done(&self) -> bool {
        self.received == self.total
            && self.files[self.current.min(self.files.len())..]
                .iter()
                .all(|file| file.is_dir || file.size == 0)
    }
}

fn prepare(files: &[TransferFileInfo], save_dir: &Path) -> Result<Vec<Option<PathBuf>>, String> {
    fs::create_dir_all(save_dir).map_err(|e| format!("create save_dir: {e}"))?;
    let mut parts = Vec::with_capacity(files.len());
    for file in files {
        // offer 已在 handle_offer 校验过；这里再验一次，确保 prepare 自身不越界。
        transfer::validate_relative_path(&file.relative_path).map_err(|e| e.to_string())?;
        let target = save_dir.join(&file.relative_path);
        if file.is_dir {
            fs::create_dir_all(&target).map_err(|e| format!("create dir: {e}"))?;
            parts.push(None);
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create parent: {e}"))?;
        }
        let part = part_path(&target);
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&part)
            .map_err(|e| format!("create part: {e}"))?;
        parts.push(Some(part));
    }
    Ok(parts)
}

fn finish_incoming(incoming: &Incoming) -> io::Result<()> {
    for (file, part) in incoming.files.iter().zip(&incoming.parts) {
        let Some(part) = part else {
            continue;
        };
        let target = incoming.save_dir.join(&file.relative_path);
        // Windows 的 rename 不覆盖已存在目标；先删再改名（非原子窗口只在 Windows）。
        if target.exists() {
            fs::remove_file(&target)?;
        }
        fs::rename(part, &target)?;
    }
    Ok(())
}

fn part_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests;
