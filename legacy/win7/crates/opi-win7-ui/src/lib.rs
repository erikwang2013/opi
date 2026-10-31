// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Native candidate window skeleton for the Windows 7/8 legacy client.
//!
//! Phase 1 is deliberately small: a Win32 top-level window with the two-line
//! layout (input buffer / candidates) and a named-pipe NDJSON client. No key
//! handling, no rendering of a candidate list, no click selection.

pub mod protocol;

#[cfg(target_os = "windows")]
pub mod pipe;
#[cfg(target_os = "windows")]
pub mod window;
