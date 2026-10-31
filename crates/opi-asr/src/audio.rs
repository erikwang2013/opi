/// Input contract: 16 kHz, mono, signed 16-bit little-endian PCM.
pub const SAMPLE_RATE: u32 = 16_000;
/// Silero VAD consumes fixed 512-sample windows at 16 kHz.
pub(crate) const WINDOW: usize = 512;
/// Per-event text cap; keeps one poll_event JSON well under 64 KiB.
pub(crate) const MAX_TEXT_BYTES: usize = 32 * 1024;

pub(crate) fn samples_to_ms(samples: u64) -> u64 {
    samples.saturating_mul(1000) / SAMPLE_RATE as u64
}

pub(crate) fn rms_db(mean_square: f64) -> f32 {
    if !mean_square.is_finite() || mean_square <= 0.0 {
        return -120.0;
    }
    ((10.0 * mean_square.log10()) as f32).clamp(-120.0, 0.0)
}

/// Converts arbitrary byte chunks to f32 samples; a trailing odd byte is kept
/// for the next call.
#[derive(Debug, Default)]
pub(crate) struct PcmFramer {
    odd: Option<u8>,
}

impl PcmFramer {
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<f32>) {
        let mut start = 0;
        if let Some(lo) = self.odd.take() {
            let Some(&hi) = bytes.first() else {
                self.odd = Some(lo);
                return;
            };
            out.push(i16::from_le_bytes([lo, hi]) as f32 / 32768.0);
            start = 1;
        }
        let mut i = start;
        while i + 1 < bytes.len() {
            out.push(i16::from_le_bytes([bytes[i], bytes[i + 1]]) as f32 / 32768.0);
            i += 2;
        }
        if i < bytes.len() {
            self.odd = Some(bytes[i]);
        }
    }

    pub fn reset(&mut self) {
        self.odd = None;
    }
}

/// Join finalized segments. A space is added when ASCII text meets ASCII text
/// (including after punctuation), so mixed zh/en output does not gain spaces
/// around Chinese.
pub(crate) fn join_texts(parts: &[String]) -> String {
    let mut out = String::new();
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let needs_space = out
            .chars()
            .next_back()
            .is_some_and(|a| a.is_ascii() && !a.is_whitespace())
            && part.chars().next().is_some_and(|b| b.is_ascii());
        if needs_space {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

/// SenseVoice reports `<|zh|>`; accept both that and plain `zh`.
pub(crate) fn normalize_lang(raw: &str) -> String {
    raw.trim()
        .split("<|")
        .find(|part| !part.trim().is_empty())
        .unwrap_or("")
        .trim_end_matches("|>")
        .trim()
        .to_ascii_lowercase()
}

/// Truncate on a UTF-8 boundary. Returns true when anything was cut.
pub(crate) fn truncate_text(text: &mut String) -> bool {
    if text.len() <= MAX_TEXT_BYTES {
        return false;
    }
    let mut end = MAX_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framer_handles_split_pcm16() {
        let mut f = PcmFramer::default();
        let mut out = Vec::new();
        f.push(&[0x10], &mut out);
        assert!(out.is_empty());
        f.push(&[0x00, 0x00, 0x80], &mut out);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 16.0 / 32768.0).abs() < 1e-9);
        assert_eq!(out[1], -1.0);
        f.push(&[0xff], &mut out);
        assert_eq!(out.len(), 2, "single trailing byte is buffered");
        f.push(&[0xff], &mut out);
        assert_eq!(out.len(), 3);
        assert!((out[2] - (-1.0 / 32768.0)).abs() < 1e-6);
    }

    #[test]
    fn rms_and_join() {
        assert_eq!(rms_db(0.0), -120.0);
        assert!((rms_db(0.5 * 0.5) - (-6.0206)).abs() < 0.01);

        let parts = vec![
            "你好".to_string(),
            "world".to_string(),
            "ok".to_string(),
            "  ".to_string(),
            "世界".to_string(),
        ];
        assert_eq!(join_texts(&parts), "你好world ok世界");
        assert_eq!(
            join_texts(&["Ask not!".to_string(), "What?".to_string()]),
            "Ask not! What?"
        );
        assert_eq!(normalize_lang("<|zh|>"), "zh");
        assert_eq!(normalize_lang("<|zh|><|NEUTRAL|>"), "zh");
        assert_eq!(normalize_lang(" EN "), "en");
        assert_eq!(normalize_lang(""), "");
    }

    #[test]
    fn truncation_is_utf8_safe() {
        let mut s = "汉".repeat(MAX_TEXT_BYTES);
        assert!(truncate_text(&mut s));
        assert!(s.len() <= MAX_TEXT_BYTES);
        assert!(s.chars().all(|c| c == '汉'));
        let mut short = "ok".to_string();
        assert!(!truncate_text(&mut short));
    }
}
