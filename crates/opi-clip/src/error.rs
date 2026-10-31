// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::types::MAX_TEXT_BYTES;
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum ClipError {
    #[error("clipboard text is {0} bytes; limit is {MAX_TEXT_BYTES} bytes")]
    TooLarge(usize),

    #[error("invalid: {0}")]
    Invalid(String),

    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("wire: {0}")]
    Wire(String),

    #[error("noise: {0}")]
    Noise(#[from] snow::Error),

    #[error("discovered peer not found")]
    PeerNotFound,

    #[error("no pairing prompt is pending for this peer")]
    NotPairing,

    #[error("trust store: {0}")]
    Trust(String),

    #[error("node is stopped")]
    Stopped,

    #[error("{0}")]
    Busy(String),
}

impl ClipError {
    pub(crate) fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }
}
