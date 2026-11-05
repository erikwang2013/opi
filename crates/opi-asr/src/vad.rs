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
    // max_segment_ms 在 sherpa VAD 里只是「超过后更容易在 100ms 停顿处切」，
    // 用户要求每 3 秒必出一段：这里再按输入样本数强制 flush 一次，保证连续说话
    // 也能增量上屏。代价是切点可能落在词中间（边界词识别会差一点）。
    let force_samples = cfg.max_segment_ms as u64 * crate::audio::SAMPLE_RATE as u64 / 1000;
    Ok(Box::new(SileroSegmenter {
        vad,
        force_samples,
        since_flush: 0,
    }))
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
    /// 强制切段的样本数（`max_segment_ms`）。
    force_samples: u64,
    /// 距上一次出段的输入样本数；出段后清零。
    since_flush: u64,
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
        // 只数「正在说话」的样本：静音等待不能把下一次开口算成满 3 秒，
        // 否则开口一两声就会被强制切出一个碎片。
        let speaking = self.vad.is_speech();
        if speaking {
            self.since_flush += window.len() as u64;
        } else {
            self.since_flush = 0;
        }
        // 先取 VAD 按停顿切出的段；有段就重新计时。
        let mut out = self.drain();
        if out.is_empty() && speaking && self.since_flush >= self.force_samples {
            // 连续说话、VAD 一直不切：到点强制收一段，保证每 3 秒增量上屏。
            self.vad.flush();
            self.since_flush = 0;
            out = self.drain();
        }
        if !out.is_empty() {
            self.since_flush = 0;
        }
        out
    }

    fn flush(&mut self) -> Vec<VadSegment> {
        self.vad.flush();
        self.since_flush = 0;
        self.drain()
    }

    fn reset(&mut self) {
        self.vad.clear();
        self.since_flush = 0;
    }
}
