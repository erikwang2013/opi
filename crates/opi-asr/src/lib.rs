//! Offline ASR core shared by the opi platform shells.
//!
//! * Runtime: sherpa-onnx (via `sherpa-rs`) serves both SenseVoice and Whisper.
//! * Input: 16 kHz mono PCM16 LE, arbitrary chunk sizes.
//! * Segmentation: Silero VAD inside the worker thread.
//! * Routing: zh / auto -> SenseVoice, en -> Whisper.
//! * No network: model download belongs to the platform shell; this crate only
//!   verifies a manifest (`manifest::verify` / `opi_asr_verify_model`).
//! * No audio is written to disk and no transcript is logged.
//! * The C ABI lives in `ffi`; every call except `destroy` may run on any
//!   thread, and `destroy` must be the last call for a handle.

mod audio;
mod config;
mod engine;
mod error;
mod ffi;
mod manifest;
mod vad;
mod worker;

pub use audio::SAMPLE_RATE;
pub use config::{AsrConfig, Language, ModelKind};
pub use error::{
    AsrError, ERR_ARGUMENT, ERR_BUSY, ERR_CONFIG, ERR_EVENT_TOO_SMALL, ERR_IO, ERR_MODEL,
    ERR_STATE, ERR_UNAVAILABLE,
};
pub use manifest::{
    MANIFEST_FILE, MANIFEST_VERSION, ModelFile, ModelManifest, VerifyReport, sha256_file, verify,
};
pub use worker::{Asr, AsrEvent, MAX_EVENT_BYTES, MAX_FEED_BYTES, ModelInfo};

// C ABI symbols, also reachable as Rust functions (tests / platform glue).
pub use ffi::*;
