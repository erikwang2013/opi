// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Minimal text service object: the bare `ITfTextInputProcessor` contract.
//!
//! Phase 1 only proves the object can be created and activated; key sinks,
//! composition and candidate UI are Phase 2. `Activate` still touches the
//! engine crates so a broken path dependency fails here rather than at first
//! keystroke: it reads one mode flag from engine-core and loads the 35-word
//! fallback dictionary from engine-data.

use engine_core::Dictionary;
use engine_core::composer::Mode;
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::UI::TextServices::{
    ITfTextInputProcessor, ITfTextInputProcessor_Impl, ITfThreadMgr,
};
use windows::core::{PCWSTR, Ref, Result, implement};

use crate::dll::DllLock;

#[implement(ITfTextInputProcessor)]
pub struct TsfTextService {
    /// Keeps the DLL loaded while COM holds a service object (see dll.rs).
    _lock: DllLock,
    mode: Mode,
    fallback_len: usize,
}

impl TsfTextService {
    pub fn new() -> Self {
        // Engine-data is compiled in; this also validates the embedded fallback
        // dictionary at activation rather than hiding a corrupt path until a
        // later release.
        let fallback_len = engine_data::fallback_dict().len();
        Self {
            _lock: DllLock::new(),
            mode: Mode::Pinyin,
            fallback_len,
        }
    }

    fn activate(&self) {
        let msg = format!(
            "opi-win7: activate mode={:?} fullwidth={} fallback_words={}",
            self.mode,
            self.mode.default_fullwidth(),
            self.fallback_len
        );
        let wide: Vec<u16> = msg.encode_utf16().chain([0]).collect();
        // SAFETY: wide is NUL-terminated and alive for the call.
        unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
    }
}

impl ITfTextInputProcessor_Impl for TsfTextService_Impl {
    fn Activate(&self, _ptim: Ref<'_, ITfThreadMgr>, _tid: u32) -> Result<()> {
        self.activate();
        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        Ok(())
    }
}
