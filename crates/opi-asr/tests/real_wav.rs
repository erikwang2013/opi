//! Opt-in integration tests against real models.
//!
//! ```text
//! OPI_ASR_TEST_ROOT=/path/to/models \
//! OPI_ASR_TEST_WAV_ZH=/path/to/zh.wav \
//! OPI_ASR_TEST_WAV_EN=/path/to/en.wav \
//! cargo test -p opi-asr --test real_wav -- --ignored --nocapture
//! ```
//!
//! Expected layout under ROOT: `sensevoice/`, `whisper-small.en/`,
//! `vad/silero_vad.onnx`; each model dir contains `manifest.json`.

use opi_asr::{Asr, AsrConfig, AsrEvent};
use std::time::{Duration, Instant};

fn feed_all(asr: &Asr, bytes: &[u8]) {
    for chunk in bytes.chunks(640) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match asr.feed_pcm(chunk) {
                Ok(()) => break,
                Err(opi_asr::AsrError::Busy(_)) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => panic!("feed_pcm: {e}"),
            }
        }
    }
}

fn run(language: &str, wav_env: &str) -> Option<String> {
    let Ok(root) = std::env::var("OPI_ASR_TEST_ROOT") else {
        eprintln!("skip: OPI_ASR_TEST_ROOT not set");
        return None;
    };
    let Ok(wav) = std::env::var(wav_env) else {
        eprintln!("skip: {wav_env} not set");
        return None;
    };

    // OPI_ASR_TEST_NO_VERIFY=1 skips SHA-256 to isolate load time.
    let verify_models = std::env::var("OPI_ASR_TEST_NO_VERIFY").is_err();
    let config_json = serde_json::json!({"models_dir": root, "language": language, "verify_models": verify_models});
    let cfg = AsrConfig::from_json(&config_json.to_string()).unwrap();
    let asr = Asr::new(cfg).unwrap();

    let t0 = Instant::now();
    let info = asr.load_model().expect("load_model");
    eprintln!(
        "engine={} files={} bytes={} load_ms={}",
        info.engine,
        info.files,
        info.bytes,
        t0.elapsed().as_millis()
    );

    let mut reader = hound::WavReader::open(&wav).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000, "wav must be 16 kHz");
    assert_eq!(spec.channels, 1, "wav must be mono");
    assert_eq!(spec.bits_per_sample, 16, "wav must be PCM16");
    let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let audio_secs = samples.len() as f64 / 16_000.0;

    asr.start().unwrap();
    let t1 = Instant::now();
    feed_all(&asr, &bytes);
    asr.finish().unwrap();

    let deadline = Instant::now() + Duration::from_secs(300);
    let mut segments = Vec::new();
    let mut final_text = None;
    while Instant::now() < deadline {
        match asr.poll_event() {
            Some(AsrEvent::Segment {
                index,
                start_ms,
                end_ms,
                text,
                lang,
                ..
            }) => {
                eprintln!("segment {index} {start_ms}..{end_ms}ms lang={lang}: {text}");
                segments.push(text);
            }
            Some(AsrEvent::Final {
                text,
                duration_ms,
                segment_count,
                ..
            }) => {
                eprintln!("final segments={segment_count} duration_ms={duration_ms} text={text}");
                final_text = Some(text);
                break;
            }
            Some(AsrEvent::Error { code, message }) => {
                panic!("worker error {code}: {message}")
            }
            Some(AsrEvent::Level { .. }) => {}
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    let final_text = final_text.expect("final event before timeout");
    let elapsed = t1.elapsed().as_secs_f64();
    eprintln!(
        "audio_secs={audio_secs:.2} decode_secs={elapsed:.2} rtf={:.3} segments={} last_error={:?}",
        elapsed / audio_secs,
        segments.len(),
        asr.last_error()
    );
    assert!(!final_text.trim().is_empty(), "empty transcript");
    Some(final_text)
}

#[test]
#[ignore = "needs real models; set OPI_ASR_TEST_ROOT / OPI_ASR_TEST_WAV_ZH"]
fn zh_sensevoice_real_wav() {
    run("zh", "OPI_ASR_TEST_WAV_ZH");
}

#[test]
#[ignore = "needs real models; set OPI_ASR_TEST_ROOT / OPI_ASR_TEST_WAV_EN"]
fn en_whisper_real_wav() {
    run("en", "OPI_ASR_TEST_WAV_EN");
}
