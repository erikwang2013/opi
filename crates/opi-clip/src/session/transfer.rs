// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 会话内的文件传输状态机：offer/decision、分块发送、进度、完成/取消/错误。
//! 接收侧在 `transfer/incoming.rs`；内存里任何时刻只有一块（≤48 KiB）。

mod incoming;

use super::Session;
use crate::ClipError;
use crate::transfer;
use crate::types::{
    ClipEvent, TRANSFER_CHUNK_BYTES, TRANSFER_TIMEOUT, TransferFile, TransferFileInfo, TransferId,
};
use crate::wire::Wire;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// 每轮最多连续发 64 块（3 MiB），随后回到读循环处理对端消息/取消。
const MAX_CHUNKS_PER_PASS: usize = 64;
/// 进度事件节流：1 MiB 或 500 ms 先到先发。
const PROGRESS_BYTES: u64 = 1024 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

pub(super) struct PendingOffer {
    pub id: TransferId,
    pub files: Vec<TransferFileInfo>,
    pub total: u64,
}

pub(super) enum OutgoingPhase {
    Decision,
    Streaming,
    Done,
}

pub(super) struct Outgoing {
    id: TransferId,
    files: Vec<TransferFile>,
    infos: Vec<TransferFileInfo>,
    total: u64,
    sent: u64,
    next_file: usize,
    offset: u64,
    file: Option<File>,
    hasher: Sha256,
    phase: OutgoingPhase,
    deadline: Instant,
    last_progress: Instant,
    last_progress_bytes: u64,
    hash: Option<[u8; 32]>,
}

pub(super) struct Incoming {
    id: TransferId,
    files: Vec<TransferFileInfo>,
    save_dir: PathBuf,
    parts: Vec<Option<PathBuf>>,
    total: u64,
    received: u64,
    current: usize,
    offset: u64,
    file: Option<File>,
    hasher: Sha256,
    last_progress: Instant,
    last_progress_bytes: u64,
}

impl Session {
    pub(super) fn start_outgoing(
        &mut self,
        id: TransferId,
        files: Vec<TransferFile>,
    ) -> Result<(), ClipError> {
        if self.outgoing.is_some() {
            self.fail_outgoing(id, "another outgoing transfer is active".into());
            return Ok(());
        }
        let total = match transfer::validate_send(&files) {
            Ok(total) => total,
            Err(error) => {
                self.fail_outgoing(id, error.to_string());
                return Ok(());
            }
        };
        let infos: Vec<_> = files.iter().map(|file| file.info().clone()).collect();
        if let Err(error) = self.send(&Wire::TransferOffer {
            transfer_id: id,
            files: infos.clone(),
        }) {
            self.fail_outgoing(id, format!("send offer: {error}"));
            return Err(error);
        }
        self.inner.emit(ClipEvent::TransferOffer {
            instance_id: self.peer,
            transfer_id: id,
            files: infos.clone(),
            total_bytes: total,
            outgoing: true,
        });
        self.outgoing = Some(Outgoing {
            id,
            files,
            infos,
            total,
            sent: 0,
            next_file: 0,
            offset: 0,
            file: None,
            hasher: Sha256::new(),
            phase: OutgoingPhase::Decision,
            deadline: Instant::now() + TRANSFER_TIMEOUT,
            last_progress: Instant::now(),
            last_progress_bytes: 0,
            hash: None,
        });
        Ok(())
    }

    pub(super) fn local_cancel(&mut self, id: TransferId) -> Result<(), ClipError> {
        if self.outgoing.as_ref().is_some_and(|out| out.id == id) {
            // 取消通知尽力送达；本地状态必须先清，避免连接随后断开时留下路由。
            let _ = self.send(&Wire::TransferCancel { transfer_id: id });
            self.outgoing = None;
            self.emit_canceled(id, true);
            self.unregister(id);
            return Ok(());
        }
        if self.incoming.as_ref().is_some_and(|inc| inc.id == id) {
            let _ = self.send(&Wire::TransferCancel { transfer_id: id });
            if let Some(incoming) = self.incoming.take() {
                cleanup(&incoming);
            }
            self.emit_canceled(id, false);
            self.unregister(id);
            return Ok(());
        }
        if self.offer.as_ref().is_some_and(|offer| offer.id == id) {
            let _ = self.send(&Wire::TransferCancel { transfer_id: id });
            self.offer = None;
            self.emit_canceled(id, false);
            self.unregister(id);
            return Ok(());
        }
        // 已经完成/失败：路由表里若还在就清掉，不再发事件。
        self.unregister(id);
        Ok(())
    }

    /// 每轮循环调用：等待决策时检查超时，传输中读文件并发送一批块。
    pub(super) fn pump_outgoing(&mut self) -> Result<(), ClipError> {
        let Some(mut outgoing) = self.outgoing.take() else {
            return Ok(());
        };
        let result = self.pump_step(&mut outgoing);
        match result {
            Ok(true) => Ok(()),
            Ok(false) => {
                self.outgoing = Some(outgoing);
                Ok(())
            }
            Err(error) => {
                let id = outgoing.id;
                let _ = self.send(&Wire::TransferError {
                    transfer_id: id,
                    message: format!("send failed: {error}"),
                });
                self.fail_outgoing(id, format!("send failed: {error}"));
                Err(error)
            }
        }
    }

    fn pump_step(&mut self, out: &mut Outgoing) -> Result<bool, ClipError> {
        let now = Instant::now();
        match out.phase {
            OutgoingPhase::Decision | OutgoingPhase::Done => {
                if now < out.deadline {
                    return Ok(false);
                }
                let reason = if matches!(out.phase, OutgoingPhase::Decision) {
                    "peer did not answer the transfer offer"
                } else {
                    "peer did not confirm transfer completion"
                };
                let _ = self.send(&Wire::TransferError {
                    transfer_id: out.id,
                    message: reason.into(),
                });
                self.fail_outgoing(out.id, reason.into());
                Ok(true)
            }
            OutgoingPhase::Streaming => {
                for _ in 0..MAX_CHUNKS_PER_PASS {
                    while out.next_file < out.files.len() && out.files[out.next_file].is_dir() {
                        out.next_file += 1;
                        out.offset = 0;
                        out.file = None;
                    }
                    if out.next_file >= out.files.len() {
                        return self.finish_outgoing(out).map(|()| false);
                    }
                    if out.file.is_none() {
                        let source = &out.files[out.next_file];
                        let metadata = fs::metadata(source.source())?;
                        if metadata.is_dir() != source.is_dir()
                            || (!metadata.is_dir() && metadata.len() != source.size())
                        {
                            return Err(ClipError::invalid(format!(
                                "local file changed: {}",
                                source.relative_path()
                            )));
                        }
                        out.file = Some(File::open(source.source())?);
                    }
                    let declared = out.files[out.next_file].size();
                    let remaining = declared.saturating_sub(out.offset);
                    if remaining == 0 {
                        out.file = None;
                        out.next_file += 1;
                        out.offset = 0;
                        continue;
                    }
                    let want = usize::try_from(remaining)
                        .unwrap_or(usize::MAX)
                        .min(TRANSFER_CHUNK_BYTES);
                    let mut buffer = vec![0u8; want];
                    let mut filled = 0;
                    let file = out.file.as_mut().expect("opened above");
                    while filled < buffer.len() {
                        match file.read(&mut buffer[filled..]) {
                            Ok(0) => break,
                            Ok(n) => filled += n,
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                            Err(error) => return Err(error.into()),
                        }
                    }
                    if filled == 0 {
                        return Err(ClipError::invalid(format!(
                            "local file ended early: {}",
                            out.files[out.next_file].relative_path()
                        )));
                    }
                    buffer.truncate(filled);
                    let file_index = u16::try_from(out.next_file)
                        .map_err(|_| ClipError::invalid("file index overflow"))?;
                    self.send(&Wire::TransferChunk {
                        transfer_id: out.id,
                        file_index,
                        offset: out.offset,
                        bytes: buffer.clone(),
                    })?;
                    out.hasher.update(&buffer);
                    out.sent += filled as u64;
                    out.offset += filled as u64;
                    out.deadline = Instant::now() + TRANSFER_TIMEOUT;
                    self.maybe_progress(
                        out.id,
                        out.sent,
                        out.total,
                        file_index,
                        out.files.len() as u16,
                        true,
                        &mut out.last_progress,
                        &mut out.last_progress_bytes,
                    );
                    if out.offset >= declared {
                        out.file = None;
                        out.next_file += 1;
                        out.offset = 0;
                    }
                }
                Ok(false)
            }
        }
    }

    fn finish_outgoing(&mut self, out: &mut Outgoing) -> Result<(), ClipError> {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&out.hasher.clone().finalize());
        out.hash = Some(hash);
        self.send(&Wire::TransferDone {
            transfer_id: out.id,
            sha256: Some(hash),
        })?;
        out.phase = OutgoingPhase::Done;
        out.deadline = Instant::now() + TRANSFER_TIMEOUT;
        self.inner.emit(ClipEvent::TransferProgress {
            instance_id: self.peer,
            transfer_id: out.id,
            transferred_bytes: out.total,
            total_bytes: out.total,
            file_index: out.files.len().saturating_sub(1) as u16,
            file_count: out.files.len() as u16,
            outgoing: true,
        });
        Ok(())
    }

    fn handle_decision(
        &mut self,
        id: TransferId,
        accept: bool,
        reason: Option<String>,
    ) -> Result<(), ClipError> {
        let Some(outgoing) = self.outgoing.take() else {
            return Ok(());
        };
        if outgoing.id != id || !matches!(outgoing.phase, OutgoingPhase::Decision) {
            self.outgoing = Some(outgoing);
            return Ok(());
        }
        if !accept {
            let reason = reason.unwrap_or_else(|| "declined by peer".into());
            self.fail_outgoing(id, reason);
            return Ok(());
        }
        let mut outgoing = outgoing;
        outgoing.phase = OutgoingPhase::Streaming;
        outgoing.deadline = Instant::now() + TRANSFER_TIMEOUT;
        self.outgoing = Some(outgoing);
        Ok(())
    }

    pub(super) fn abort_transfers(&mut self, reason: &str) {
        if let Some(outgoing) = self.outgoing.take() {
            self.fail_outgoing(outgoing.id, reason.to_owned());
        }
        if let Some(incoming) = self.incoming.take() {
            cleanup(&incoming);
            self.fail_incoming(incoming.id, reason.to_owned());
        }
        if let Some(offer) = self.offer.take() {
            self.fail_incoming(offer.id, reason.to_owned());
        }
        // 兜底：错误路径可能已把本地状态取空，但路由表里仍留着本对端的条目。
        self.inner
            .lock()
            .transfers
            .retain(|_, route| route.peer != self.peer);
    }

    fn fail_outgoing(&mut self, id: TransferId, reason: String) {
        self.inner.emit(ClipEvent::TransferFailed {
            instance_id: self.peer,
            transfer_id: id,
            reason,
            outgoing: true,
        });
        self.unregister(id);
    }

    fn fail_incoming(&mut self, id: TransferId, reason: String) {
        self.inner.emit(ClipEvent::TransferFailed {
            instance_id: self.peer,
            transfer_id: id,
            reason,
            outgoing: false,
        });
        self.unregister(id);
    }

    fn emit_canceled(&self, id: TransferId, outgoing: bool) {
        self.inner.emit(ClipEvent::TransferCanceled {
            instance_id: self.peer,
            transfer_id: id,
            outgoing,
        });
    }

    fn unregister(&self, id: TransferId) {
        self.inner.lock().unregister_transfer(id);
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_progress(
        &self,
        id: TransferId,
        sent: u64,
        total: u64,
        file_index: u16,
        file_count: u16,
        outgoing: bool,
        last_at: &mut Instant,
        last_bytes: &mut u64,
    ) {
        let now = Instant::now();
        if sent.saturating_sub(*last_bytes) < PROGRESS_BYTES
            && now.duration_since(*last_at) < PROGRESS_INTERVAL
        {
            return;
        }
        *last_at = now;
        *last_bytes = sent;
        self.inner.emit(ClipEvent::TransferProgress {
            instance_id: self.peer,
            transfer_id: id,
            transferred_bytes: sent,
            total_bytes: total,
            file_index,
            file_count,
            outgoing,
        });
    }
}

fn cleanup(incoming: &Incoming) {
    for part in incoming.parts.iter().flatten() {
        let _ = fs::remove_file(part);
    }
}
