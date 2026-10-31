use crate::config::{AsrConfig, ModelKind};
use crate::error::AsrError;
use crate::manifest::ModelManifest;
use std::path::{Path, PathBuf};

/// Files chosen by manifest role, not by fixed on-disk names.
#[derive(Debug)]
#[cfg_attr(not(feature = "sherpa"), allow(dead_code))]
pub(crate) struct ModelFiles {
    pub model: Option<PathBuf>,
    pub encoder: Option<PathBuf>,
    pub decoder: Option<PathBuf>,
    pub tokens: PathBuf,
}

impl ModelFiles {
    pub fn from_manifest(
        dir: &Path,
        manifest: &ModelManifest,
        kind: ModelKind,
    ) -> Result<Self, AsrError> {
        let tokens = manifest.role_path(dir, "tokens")?;
        match kind {
            ModelKind::SenseVoice => Ok(Self {
                model: Some(manifest.role_path(dir, "model")?),
                encoder: None,
                decoder: None,
                tokens,
            }),
            ModelKind::Whisper => Ok(Self {
                model: None,
                encoder: Some(manifest.role_path(dir, "encoder")?),
                decoder: Some(manifest.role_path(dir, "decoder")?),
                tokens,
            }),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Recognition {
    pub text: String,
    pub lang: String,
}

/// One loaded offline recognizer. `sherpa-rs` owns all decoding/feature logic.
pub(crate) trait Recognizer: Send {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Recognition, AsrError>;
}

#[cfg(feature = "sherpa")]
pub(crate) fn load(
    kind: ModelKind,
    files: &ModelFiles,
    cfg: &AsrConfig,
) -> Result<Box<dyn Recognizer>, AsrError> {
    use sherpa_rs::sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer};
    use sherpa_rs::whisper::{WhisperConfig, WhisperRecognizer};

    let path = |p: &Option<PathBuf>| -> Result<String, AsrError> {
        p.as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .ok_or_else(|| AsrError::Model("missing model path".into()))
    };
    let tokens = files.tokens.to_string_lossy().into_owned();

    match kind {
        ModelKind::SenseVoice => {
            let recognizer = SenseVoiceRecognizer::new(SenseVoiceConfig {
                model: path(&files.model)?,
                language: cfg.language.sense_voice_code().to_string(),
                use_itn: true,
                provider: None,
                num_threads: Some(cfg.num_threads),
                debug: false,
                tokens,
            })
            .map_err(|e| AsrError::Model(format!("load SenseVoice: {e}")))?;
            Ok(Box::new(SherpaRecognizer::SenseVoice(recognizer)))
        }
        ModelKind::Whisper => {
            let recognizer = WhisperRecognizer::new(WhisperConfig {
                encoder: path(&files.encoder)?,
                decoder: path(&files.decoder)?,
                tokens,
                language: "en".into(),
                bpe_vocab: None,
                tail_paddings: None,
                provider: None,
                num_threads: Some(cfg.num_threads),
                debug: false,
            })
            .map_err(|e| AsrError::Model(format!("load Whisper: {e}")))?;
            Ok(Box::new(SherpaRecognizer::Whisper(recognizer)))
        }
    }
}

#[cfg(not(feature = "sherpa"))]
pub(crate) fn load(
    _kind: ModelKind,
    _files: &ModelFiles,
    _cfg: &AsrConfig,
) -> Result<Box<dyn Recognizer>, AsrError> {
    Err(AsrError::Unavailable(
        "opi-asr built without the `sherpa` feature".into(),
    ))
}

#[cfg(feature = "sherpa")]
enum SherpaRecognizer {
    SenseVoice(sherpa_rs::sense_voice::SenseVoiceRecognizer),
    Whisper(sherpa_rs::whisper::WhisperRecognizer),
}

#[cfg(feature = "sherpa")]
impl Recognizer for SherpaRecognizer {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Recognition, AsrError> {
        let result = match self {
            Self::SenseVoice(r) => r.transcribe(crate::audio::SAMPLE_RATE, samples),
            Self::Whisper(r) => r.transcribe(crate::audio::SAMPLE_RATE, samples),
        };
        Ok(Recognition {
            text: result.text,
            lang: result.lang,
        })
    }
}
