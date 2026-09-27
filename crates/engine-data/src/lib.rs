// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! engine-data：.opid 二进制词库的格式、校验与 mmap 加载（M2）。

pub mod checksum;
pub mod dictionary;
pub mod format;
pub mod loader;

pub use checksum::fnv1a64;
pub use dictionary::{fallback_dict, load_or_fallback};
pub use format::{FormatError, OpDict, RawEntry, Validated, parse, serialize, validate};
pub use loader::{LoadError, MmapDictionary, load_bytes, load_mmap};
// Dictionary trait 一并转发：opi-tools 的 verify 子命令（bin，不可用 dev-dep）需要
pub use engine_core::dictionary::Dictionary;
