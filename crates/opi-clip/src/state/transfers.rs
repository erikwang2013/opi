// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 传输路由表的边界（从 state.rs 拆出守 500 行）。

use super::*;
use crate::types::MAX_CONCURRENT_TRANSFERS;

impl Core {
    pub(crate) fn register_transfer(
        &mut self,
        id: TransferId,
        route: TransferRoute,
    ) -> Result<(), ClipError> {
        if self.transfers.len() >= MAX_CONCURRENT_TRANSFERS {
            return Err(ClipError::Busy(format!(
                "too many concurrent transfers (limit {MAX_CONCURRENT_TRANSFERS})"
            )));
        }
        if self.transfers.insert(id, route).is_some() {
            return Err(ClipError::invalid("duplicate transfer id"));
        }
        Ok(())
    }

    pub(crate) fn unregister_transfer(&mut self, id: TransferId) -> Option<TransferRoute> {
        self.transfers.remove(&id)
    }

    pub(crate) fn transfer_route(&self, id: TransferId) -> Option<&TransferRoute> {
        self.transfers.get(&id)
    }

    pub(crate) fn has_outgoing_for(&self, peer: InstanceId) -> bool {
        self.transfers
            .values()
            .any(|route| route.outgoing && route.peer == peer)
    }

    /// 接收端在发出 offer 事件前登记路由；并发已满时返回 `Busy`。
    pub(crate) fn offer_transfer(
        &mut self,
        id: TransferId,
        peer: InstanceId,
    ) -> Result<(), ClipError> {
        self.register_transfer(
            id,
            TransferRoute {
                peer,
                outgoing: false,
            },
        )
    }
}
