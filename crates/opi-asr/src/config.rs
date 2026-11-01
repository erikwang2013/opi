use crate::error::AsrError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Spoken language hint. `Auto` = SenseVoice with its own language ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Zh,
    En,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    SenseVoice,
    Whisper,
}

impl ModelKind {
    pub fn engine(self) -> &'static str {
        match self {
            Self::SenseVoice => "sensevoice",
            Self::Whisper => "whisper",
        }
    }
}

impl Language {
    /// zh / auto -> SenseVoice, en -> Whisper. This is the whole routing policy.
    pub fn model_kind(self) -> ModelKind {
        match self {
            Self::En => ModelKind::Whisper,
            Self::Zh | Self::Auto => ModelKind::SenseVoice,
        }
    }

    /// SenseVoice language hint. Never used for Whisper (which is pinned to "en").
    pub fn sense_voice_code(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
            Self::Auto => "auto",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AsrConfig {
    /// Root for relative model paths. Empty means "paths are absolute".
    pub models_dir: PathBuf,
    pub sense_voice_dir: PathBuf,
    pub whisper_dir: PathBuf,
    /// Silero VAD ONNX. Relative paths resolve against `models_dir`.
    pub vad_model: PathBuf,
    pub language: Language,
    pub num_threads: i32,
    /// Full SHA-256 verification on `load_model`. Size is always checked.
    pub verify_models: bool,
    pub max_segment_ms: u32,
    pub vad_min_silence_ms: u32,
    pub vad_threshold: f32,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            models_dir: PathBuf::from("."),
            sense_voice_dir: PathBuf::from("sensevoice"),
            whisper_dir: PathBuf::from("whisper-small.en"),
            vad_model: PathBuf::from("vad/silero_vad.onnx"),
            language: Language::Auto,
            num_threads: 2,
            verify_models: true,
            max_segment_ms: 30_000,
            vad_min_silence_ms: 500,
            vad_threshold: 0.5,
        }
    }
}

impl AsrConfig {
    pub fn from_json(json: &str) -> Result<Self, AsrError> {
        let cfg: Self = serde_json::from_str(json).map_err(|e| AsrError::Config(e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), AsrError> {
        if !(1..=16).contains(&self.num_threads) {
            return Err(AsrError::Config(format!(
                "num_threads must be 1..=16, got {}",
                self.num_threads
            )));
        }
        if !(1_000..=120_000).contains(&self.max_segment_ms) {
            return Err(AsrError::Config(format!(
                "max_segment_ms must be 1000..=120000, got {}",
                self.max_segment_ms
            )));
        }
        if !(100..=5_000).contains(&self.vad_min_silence_ms) {
            return Err(AsrError::Config(format!(
                "vad_min_silence_ms must be 100..=5000, got {}",
                self.vad_min_silence_ms
            )));
        }
        if !(0.0..=1.0).contains(&self.vad_threshold) {
            return Err(AsrError::Config(format!(
                "vad_threshold must be 0.0..=1.0, got {}",
                self.vad_threshold
            )));
        }
        if self.sense_voice_dir.as_os_str().is_empty()
            || self.whisper_dir.as_os_str().is_empty()
            || self.vad_model.as_os_str().is_empty()
        {
            return Err(AsrError::Config(
                "model directory paths must not be empty".into(),
            ));
        }
        Ok(())
    }

    pub fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() || self.models_dir.as_os_str().is_empty() {
            path.to_path_buf()
        } else {
            self.models_dir.join(path)
        }
    }

    pub fn model_dir(&self, kind: ModelKind) -> PathBuf {
        match kind {
            ModelKind::SenseVoice => self.resolve(&self.sense_voice_dir),
            ModelKind::Whisper => self.resolve(&self.whisper_dir),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_and_resolution() {
        assert_eq!(Language::Zh.model_kind(), ModelKind::SenseVoice);
        assert_eq!(Language::Auto.model_kind(), ModelKind::SenseVoice);
        assert_eq!(Language::En.model_kind(), ModelKind::Whisper);
        assert_eq!(Language::Auto.sense_voice_code(), "auto");

        let cfg = AsrConfig {
            models_dir: PathBuf::from("/models"),
            ..Default::default()
        };
        assert_eq!(
            cfg.resolve(Path::new("sensevoice")),
            PathBuf::from("/models/sensevoice")
        );
        assert_eq!(
            cfg.resolve(Path::new("/abs/whisper")),
            PathBuf::from("/abs/whisper")
        );
    }

    #[test]
    fn json_defaults_and_rejects_bad_values() {
        let cfg = AsrConfig::from_json(r#"{"language":"en"}"#).unwrap();
        assert_eq!(cfg.language, Language::En);
        assert!(cfg.verify_models);
        assert!(AsrConfig::from_json(r#"{"language":"fr"}"#).is_err());
        assert!(AsrConfig::from_json(r#"{"num_threads":0}"#).is_err());
        assert!(AsrConfig::from_json(r#"{"vad_threshold":1.5}"#).is_err());
    }
}
