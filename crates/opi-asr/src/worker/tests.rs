use super::*;
use crate::config::Language;

struct FakeRecognizer;

impl Recognizer for FakeRecognizer {
    fn transcribe(&mut self, samples: &[f32]) -> Result<engine::Recognition, AsrError> {
        Ok(engine::Recognition {
            text: format!(" seg{} ", samples.len()),
            lang: String::new(),
        })
    }
}

struct FakeSegmenter {
    every: usize,
    windows: usize,
    start: u64,
    buffer: Vec<f32>,
}

impl FakeSegmenter {
    fn take(&mut self) -> Vec<VadSegment> {
        let samples = std::mem::take(&mut self.buffer);
        let start = self.start;
        self.start += samples.len() as u64;
        self.windows = 0;
        vec![VadSegment {
            start_sample: start,
            samples,
        }]
    }
}

impl Segmenter for FakeSegmenter {
    fn push(&mut self, window: &[f32]) -> Vec<VadSegment> {
        self.buffer.extend_from_slice(window);
        self.windows += 1;
        if self.windows == self.every {
            self.take()
        } else {
            Vec::new()
        }
    }

    fn flush(&mut self) -> Vec<VadSegment> {
        if self.buffer.is_empty() {
            Vec::new()
        } else {
            self.take()
        }
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.windows = 0;
        self.start = 0;
    }
}

fn worker() -> Worker {
    let cfg = AsrConfig {
        language: Language::Zh,
        ..Default::default()
    };
    let mut worker = Worker::new(
        cfg,
        Arc::new(Mutex::new(VecDeque::new())),
        Arc::new(Mutex::new(None)),
        Arc::new(AtomicBool::new(false)),
    );
    worker.recognizer = Some(Box::new(FakeRecognizer));
    worker.segmenter = Some(Box::new(FakeSegmenter {
        every: 4,
        windows: 0,
        start: 0,
        buffer: Vec::new(),
    }));
    worker
}

fn events(worker: &Worker) -> Vec<AsrEvent> {
    lock(&worker.events).iter().cloned().collect()
}

#[test]
fn segments_and_final() {
    let mut worker = worker();
    worker.start();
    // 8 windows = 2 fake segments; use nonzero samples so levels are real.
    let bytes: Vec<u8> = (0..WINDOW * 8)
        .flat_map(|_| 1000i16.to_le_bytes())
        .collect();
    worker.feed(&bytes);
    worker.finish();

    let events = events(&worker);
    let segments: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AsrEvent::Segment {
                index,
                start_ms,
                end_ms,
                text,
                ..
            } => Some((*index, *start_ms, *end_ms, text.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].0, 0);
    assert_eq!(segments[0].1, 0);
    assert_eq!(segments[0].2, 128);
    assert_eq!(segments[1].1, 128);
    assert_eq!(segments[0].3, format!("seg{}", WINDOW * 4));

    let finals: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AsrEvent::Final {
                text,
                duration_ms,
                segment_count,
                ..
            } => Some((text.clone(), *duration_ms, *segment_count)),
            _ => None,
        })
        .collect();
    assert_eq!(finals.len(), 1);
    assert_eq!(finals[0].1, 256);
    assert_eq!(finals[0].2, 2);
    assert_eq!(finals[0].0, format!("seg{} seg{}", WINDOW * 4, WINDOW * 4));

    let levels: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AsrEvent::Level { rms_db } => Some(*rms_db),
            _ => None,
        })
        .collect();
    assert_eq!(levels.len(), 2, "one level per 100 ms");
    assert!(levels.iter().all(|db| *db < 0.0));
}

#[test]
fn feed_before_start_reports_once() {
    let mut worker = worker();
    worker.feed(&[0; 1024]);
    worker.feed(&[0; 1024]);
    let errors = events(&worker)
        .into_iter()
        .filter(|e| matches!(e, AsrEvent::Error { .. }))
        .count();
    assert_eq!(errors, 1);
    assert!(lock(&worker.last_error).is_some());
}

#[test]
fn cancel_discards_pending_output() {
    let mut worker = worker();
    worker.start();
    let bytes: Vec<u8> = vec![0; WINDOW * 8 * 2];
    worker.feed(&bytes);
    assert!(matches!(
        events(&worker).last(),
        Some(AsrEvent::Segment { .. }) | Some(AsrEvent::Level { .. })
    ));
    worker.cancel();
    assert!(events(&worker).is_empty());
    worker.finish();
    let has_final = events(&worker)
        .iter()
        .any(|e| matches!(e, AsrEvent::Final { .. }));
    assert!(!has_final);
    assert!(!worker.active);
}

struct CancelRecognizer(Arc<AtomicBool>);

impl Recognizer for CancelRecognizer {
    fn transcribe(&mut self, _samples: &[f32]) -> Result<engine::Recognition, AsrError> {
        self.0.store(true, Ordering::SeqCst);
        Ok(engine::Recognition {
            text: "late".into(),
            lang: String::new(),
        })
    }
}

#[test]
fn cancel_during_decode_discards_segment() {
    let mut worker = worker();
    worker.recognizer = Some(Box::new(CancelRecognizer(Arc::clone(&worker.cancel))));
    worker.start();
    let bytes = vec![0u8; WINDOW * 4 * 2];
    worker.feed(&bytes);
    let has_segment = events(&worker)
        .iter()
        .any(|e| matches!(e, AsrEvent::Segment { .. }));
    assert!(!has_segment, "decoded text must be dropped after cancel");
}

#[test]
fn event_json_has_type_tag() {
    let value = serde_json::to_value(AsrEvent::Level { rms_db: -20.0 }).unwrap();
    assert_eq!(value["type"], "level");
    let value = serde_json::to_value(AsrEvent::Error {
        code: "x".into(),
        message: "y".into(),
    })
    .unwrap();
    assert_eq!(value["type"], "error");
}

#[test]
fn handle_reports_not_loaded() {
    let asr = Asr::new(AsrConfig::default()).unwrap();
    asr.start().unwrap();
    let mut seen = false;
    for _ in 0..100 {
        if let Some(AsrEvent::Error { code, .. }) = asr.poll_event() {
            assert_eq!(code, "not_loaded");
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(seen);
    assert!(asr.last_error().is_some());
}
