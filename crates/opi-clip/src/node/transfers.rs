// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 文件传输的公开句柄方法（从 node.rs 拆出守 500 行）。

use super::ClipNode;
use crate::ClipError;
use crate::transfer;
use crate::types::{ClipEvent, InstanceId, TransferFile, TransferId, TransferRoute};
use std::path::PathBuf;

impl ClipNode {
    /// 向已配对且已连接的对端发送文件/文件夹。返回本次传输 ID；进度与结果
    /// 通过 `TransferOffer/Progress/Completed/Failed/Canceled` 事件上报。
    /// 接收端必须在 `TransferOffer` 事件后调用 [`Self::respond_transfer`]。
    pub fn send_files(
        &self,
        peer: InstanceId,
        files: Vec<TransferFile>,
    ) -> Result<TransferId, ClipError> {
        if self.inner.is_stopped() {
            return Err(ClipError::Stopped);
        }
        transfer::validate_send(&files)?;
        let id = transfer::random_id()?;
        let sender = {
            let mut core = self.inner.lock();
            if !core.trust.contains(peer) {
                return Err(ClipError::NotTrusted);
            }
            let Some(session) = core.sessions.get(&peer) else {
                return Err(ClipError::Busy("peer is not connected".into()));
            };
            let sender = session.tx.clone();
            if core.has_outgoing_for(peer) {
                return Err(ClipError::Busy(
                    "another transfer to this peer is active".into(),
                ));
            }
            core.register_transfer(
                id,
                TransferRoute {
                    peer,
                    outgoing: true,
                },
            )?;
            sender
        };
        if sender
            .try_send(crate::types::Outbound::SendTransfer { id, files })
            .is_err()
        {
            self.inner.lock().unregister_transfer(id);
            return Err(ClipError::Busy("peer session is busy or closed".into()));
        }
        Ok(id)
    }

    /// 接受/拒绝一次入站传输。接受时必须给出落盘的 `save_dir`（不存在会创建）。
    pub fn respond_transfer(
        &self,
        id: TransferId,
        accept: bool,
        save_dir: Option<PathBuf>,
    ) -> Result<(), ClipError> {
        if self.inner.is_stopped() {
            return Err(ClipError::Stopped);
        }
        if accept
            && save_dir
                .as_ref()
                .is_none_or(|dir| dir.as_os_str().is_empty())
        {
            return Err(ClipError::invalid("save_dir is required when accepting"));
        }
        let (sender, peer, outgoing) = {
            let core = self.inner.lock();
            let route = core.transfer_route(id).ok_or(ClipError::TransferNotFound)?;
            if route.outgoing {
                return Err(ClipError::Busy(
                    "transfer was not offered by the peer".into(),
                ));
            }
            let Some(session) = core.sessions.get(&route.peer) else {
                return Err(ClipError::Busy("peer is not connected".into()));
            };
            (session.tx.clone(), route.peer, route.outgoing)
        };
        if sender
            .try_send(crate::types::Outbound::RespondTransfer {
                id,
                accept,
                save_dir,
            })
            .is_err()
        {
            self.transfer_session_lost(id, peer, outgoing);
            return Err(ClipError::Busy("peer session is busy or closed".into()));
        }
        Ok(())
    }

    /// 取消一次进行中的传输（收发两侧都可调用）。未知 ID → `TransferNotFound`。
    pub fn cancel_transfer(&self, id: TransferId) -> Result<(), ClipError> {
        if self.inner.is_stopped() {
            return Err(ClipError::Stopped);
        }
        let (sender, peer, outgoing) = {
            let core = self.inner.lock();
            let route = core.transfer_route(id).ok_or(ClipError::TransferNotFound)?;
            let Some(session) = core.sessions.get(&route.peer) else {
                return Err(ClipError::Busy("peer is not connected".into()));
            };
            (session.tx.clone(), route.peer, route.outgoing)
        };
        if sender
            .try_send(crate::types::Outbound::CancelTransfer { id })
            .is_err()
        {
            self.transfer_session_lost(id, peer, outgoing);
            return Err(ClipError::Busy("peer session is busy or closed".into()));
        }
        Ok(())
    }

    fn transfer_session_lost(&self, id: TransferId, peer: InstanceId, outgoing: bool) {
        if self.inner.lock().unregister_transfer(id).is_none() {
            return;
        }
        let event = if outgoing {
            ClipEvent::TransferFailed {
                instance_id: peer,
                transfer_id: id,
                reason: "peer session is closed".into(),
                outgoing,
            }
        } else {
            ClipEvent::TransferCanceled {
                instance_id: peer,
                transfer_id: id,
                outgoing,
            }
        };
        self.inner.emit(event);
    }
}
