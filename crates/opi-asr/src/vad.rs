use crate::config::AsrConfig;
use crate::error::AsrError;

/// One VAD-confirmed speech segment. `start_sample` is absolute within the
/// current session (16 kHz).
#[derive(Debug, Clone)]
pub(crate) struct VadSegment {
    pub start_sample: u64,
    pub samples: Vec<f32>,
}

/// Implemented by Silero (real) and by the test double. Input is always a full
/// 512-sample window at 16 kHz; returning a segment means "speech ended".
pub(crate) trait Segmenter: Send {
    fn push(&mut self, window: &[f32]) -> Vec<VadSegment>;
    fn flush(&mut self) -> Vec<VadSegment>;
    fn reset(&mut self);
}

#[cfg(feature = "sherpa")]
pub(crate) fn load(cfg: &AsrConfig) -> Result<Box<dyn Segmenter>, AsrError> {
    use sherpa_rs::silero_vad::SileroVad;

    let path = cfg.resolve(&cfg.vad_model);
    if !path.is_file() {
        return Err(AsrError::Model(format!(
            "VAD model not found: {}",
            path.display()
        )));
    }
    let json_path = path.to_string_lossy().into_owned();
    let vad = SileroVad::new(
        sherpa_rs::silero_vad::SileroVadConfig {
            model: json_path,
            min_silence_duration: cfg.vad_min_silence_ms as f32 / 1000.0,
            min_speech_duration: 0.2,
            max_speech_duration: cfg.max_segment_ms as f32 / 1000.0,
            threshold: cfg.vad_threshold,
            sample_rate: crate::audio::SAMPLE_RATE,
            window_size: crate::audio::WINDOW as i32,
            provider: None,
            num_threads: Some(1),
            debug: false,
        },
        60.0,
    )
    .map_err(|e| AsrError::Model(format!("create Silero VAD: {e}")))?;
    Ok(Box::new(SileroSegmenter { vad }))
}

#[cfg(not(feature = "sherpa"))]
pub(crate) fn load(_cfg: &AsrConfig) -> Result<Box<dyn Segmenter>, AsrError> {
    Err(AsrError::Unavailable(
        "opi-asr built without the `sherpa` feature".into(),
    ))
}

#[cfg(feature = "sherpa")]
struct SileroSegmenter {
    vad: sherpa_rs::silero_vad::SileroVad,
}

#[cfg(feature = "sherpa")]
impl SileroSegmenter {
    fn drain(&mut self) -> Vec<VadSegment> {
        let mut out = Vec::new();
        while !self.vad.is_empty() {
            let seg = self.vad.front();
            self.vad.pop();
            out.push(VadSegment {
                start_sample: seg.start.max(0) as u64,
                samples: seg.samples,
            });
        }
        out
    }
}

#[cfg(feature = "sherpa")]
impl Segmenter for SileroSegmenter {
    fn push(&mut self, window: &[f32]) -> Vec<VadSegment> {
        self.vad.accept_waveform(window.to_vec());
        self.drain()
    }

    fn flush(&mut self) -> Vec<VadSegment> {
        self.vad.flush();
        self.drain()
    }

    fn reset(&mut self) {
        self.vad.clear();
    }
}
