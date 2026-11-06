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
    // 连续说话时 VAD 的 is_speech() 会偶发一两拍 false（重采样/噪声）；只在
    // 静音达到 vad_min_silence_ms 后才清零计时，否则这些抖动会把 3 秒计数打断。
    let reset_windows = (cfg.vad_min_silence_ms as u64 * crate::audio::SAMPLE_RATE as u64
        / 1000
        / crate::audio::WINDOW as u64)
        .max(1);
    Ok(Box::new(SileroSegmenter {
        vad,
        cadence: FlushCadence::new(force_samples, reset_windows),
    }))
}

/// 强制切段计时（纯状态，单测不需要真 VAD）。
///
/// 连续说话时 `is_speech()` 会偶发一两拍 false（重采样/噪声）。旧实现每拍 false
/// 都清零，Linux 上偶发抖动会让 3 秒计数永远凑不满 —— 用户看到「只出第一段」。
/// 这里只在静音持续达到 `reset_windows`（vad_min_silence_ms）后才清零。
struct FlushCadence {
    force_samples: u64,
    reset_windows: u64,
    since_flush: u64,
    silent_windows: u64,
}

impl FlushCadence {
    fn new(force_samples: u64, reset_windows: u64) -> Self {
        Self {
            force_samples,
            reset_windows,
            since_flush: 0,
            silent_windows: 0,
        }
    }

    /// 喂一个窗口；返回 true = 现在该强制 flush（只在正在说话时）。
    fn observe(&mut self, speaking: bool, samples: u64) -> bool {
        if speaking {
            self.silent_windows = 0;
            self.since_flush = self.since_flush.saturating_add(samples);
        } else {
            self.silent_windows += 1;
            if self.silent_windows >= self.reset_windows {
                self.since_flush = 0;
            }
        }
        speaking && self.since_flush >= self.force_samples
    }

    fn reset(&mut self) {
        self.since_flush = 0;
        self.silent_windows = 0;
    }
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
    cadence: FlushCadence,
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
        // 只数「说话附近」的样本：真正的静音（达到 vad_min_silence_ms）才清零，
        // 避免 VAD 偶发 false 把连续说话的 3 秒计数打断（Linux 只出第一段的根因之一）。
        let speaking = self.vad.is_speech();
        let due = self.cadence.observe(speaking, window.len() as u64);
        // 先取 VAD 按停顿切出的段；有段就重新计时。
        let mut out = self.drain();
        if out.is_empty() && due {
            // 连续说话、VAD 一直不切：到点强制收一段，保证每 3 秒增量上屏。
            self.vad.flush();
            self.cadence.reset();
            out = self.drain();
        }
        if !out.is_empty() {
            self.cadence.reset();
        }
        out
    }

    fn flush(&mut self) -> Vec<VadSegment> {
        self.vad.flush();
        self.cadence.reset();
        self.drain()
    }

    fn reset(&mut self) {
        self.vad.clear();
        self.cadence.reset();
    }
}

#[cfg(test)]
mod cadence_tests {
    use super::FlushCadence;

    const WINDOW: u64 = 512;
    const FORCE: u64 = 3 * 16_000;

    #[test]
    fn short_false_windows_do_not_starve_the_forced_flush() {
        // 每 7 个窗口报一次 false（32ms，远小于 500ms 静音）：旧实现每次都清零，
        // 3 秒计数永远凑不满；这里应当仍按 ~3 秒出段。
        let mut c = FlushCadence::new(FORCE, 15);
        let mut flushes = 0;
        for i in 0..500 {
            if c.observe(i % 7 != 0, WINDOW) {
                flushes += 1;
                c.reset();
            }
        }
        assert!(flushes >= 3, "16s 连续说话至少 3 段，实际 {flushes}");
    }

    #[test]
    fn real_silence_resets_the_cadence() {
        let mut c = FlushCadence::new(FORCE, 15);
        for _ in 0..90 {
            c.observe(true, WINDOW);
        }
        // 1 秒静音：计数必须清零，之后重新说满 3 秒才会 flush。
        for _ in 0..31 {
            assert!(!c.observe(false, WINDOW));
        }
        let mut due_at = None;
        for i in 0..120 {
            if c.observe(true, WINDOW) {
                due_at = Some(i);
                break;
            }
        }
        let i = due_at.expect("静音后重新说话应再次触发 3 秒切段");
        assert!(i >= 90, "静音后不能立刻切出碎片，实际第 {i} 个窗口");
    }

    #[test]
    fn natural_segment_reset_restarts_the_clock() {
        let mut c = FlushCadence::new(FORCE, 15);
        for _ in 0..80 {
            c.observe(true, WINDOW);
        }
        c.reset(); // 模拟 VAD 自然出段
        for i in 0..100 {
            let due = c.observe(true, WINDOW);
            assert_eq!(due, i >= 93, "自然出段后应重新数满 3 秒，第 {i} 个窗口");
        }
    }
}
