use crate::audio::{self, PcmFramer, WINDOW};
use crate::config::{AsrConfig, ModelKind};
use crate::engine::{self, ModelFiles, Recognizer};
use crate::error::AsrError;
use crate::manifest::{self, MANIFEST_FILE, ModelManifest};
use crate::vad::{self, Segmenter, VadSegment};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

/// Upper bound for one `poll_event` JSON. Text fields are capped separately.
pub const MAX_EVENT_BYTES: usize = 64 * 1024;
/// One `feed_pcm` call may carry at most 8 s of 16 kHz PCM16.
pub const MAX_FEED_BYTES: usize = 256 * 1024;
const FEED_QUEUE: usize = 32;
const MAX_QUEUED_EVENTS: usize = 1024;
const LEVEL_SAMPLES: u64 = 1_600; // 100 ms

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AsrEvent {
    /// Roughly every 100 ms while a session is active.
    Level {
        rms_db: f32,
    },
    /// One VAD-delimited utterance. `lang` comes from the model when available.
    Segment {
        index: u32,
        start_ms: u64,
        end_ms: u64,
        text: String,
        lang: String,
        engine: String,
        truncated: bool,
    },
    /// Emitted once after `finish`. `text` is the whole session joined.
    Final {
        text: String,
        duration_ms: u64,
        segment_count: u32,
        truncated: bool,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub engine: String,
    pub model_dir: String,
    pub bytes: u64,
    pub files: usize,
}

type Events = Arc<Mutex<VecDeque<AsrEvent>>>;
type LastError = Arc<Mutex<Option<String>>>;

enum Command {
    Load(SyncSender<Result<ModelInfo, AsrError>>),
    Start,
    Feed(Vec<u8>),
    Finish,
    Cancel,
    Shutdown,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Public handle. The worker thread owns all decoder state; this type only
/// queues commands and reads events, so it is `Send + Sync`. `destroy`/`Drop`
/// must be the last call for a handle.
pub struct Asr {
    tx: SyncSender<Command>,
    events: Events,
    last_error: LastError,
    cancel: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl Asr {
    pub fn new(cfg: AsrConfig) -> Result<Self, AsrError> {
        cfg.validate()?;
        let (tx, rx) = sync_channel(FEED_QUEUE);
        let events: Events = Arc::new(Mutex::new(VecDeque::new()));
        let last_error: LastError = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = Worker::new(
            cfg,
            Arc::clone(&events),
            Arc::clone(&last_error),
            Arc::clone(&cancel),
        );
        let join = std::thread::Builder::new()
            .name("opi-asr".into())
            .spawn(move || worker.run(rx))
            .map_err(|e| AsrError::Io(format!("spawn worker: {e}")))?;
        Ok(Self {
            tx,
            events,
            last_error,
            cancel,
            join: Mutex::new(Some(join)),
        })
    }

    /// Blocking: reads/verifies the selected model and creates the recognizer.
    /// The selected model is `language.model_kind()`.
    pub fn load_model(&self) -> Result<ModelInfo, AsrError> {
        let (tx, rx) = sync_channel(1);
        self.send(Command::Load(tx))?;
        let result = rx
            .recv()
            .map_err(|_| AsrError::State("worker stopped during load".into()))?;
        if let Err(e) = &result {
            self.record_error(e);
        }
        result
    }

    /// Queued; state errors arrive as `error` events.
    pub fn start(&self) -> Result<(), AsrError> {
        self.send(Command::Start)
    }

    /// Non-blocking. `Busy` means the 32-chunk queue is full: pace the caller.
    pub fn feed_pcm(&self, bytes: &[u8]) -> Result<(), AsrError> {
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > MAX_FEED_BYTES {
            return Err(AsrError::Argument(format!(
                "feed chunk {} bytes > {MAX_FEED_BYTES}",
                bytes.len()
            )));
        }
        match self.tx.try_send(Command::Feed(bytes.to_vec())) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(AsrError::Busy("audio queue full".into())),
            Err(TrySendError::Disconnected(_)) => Err(AsrError::State("worker stopped".into())),
        }
    }

    /// Queued; success is reported by the `final` event.
    pub fn finish(&self) -> Result<(), AsrError> {
        self.send(Command::Finish)
    }

    /// Drops queued audio/events immediately. A decode already in progress in
    /// the worker cannot be interrupted; its result is discarded on the next
    /// cancel check.
    pub fn cancel(&self) -> Result<(), AsrError> {
        self.cancel.store(true, Ordering::SeqCst);
        lock(&self.events).clear();
        self.send(Command::Cancel)
    }

    pub fn poll_event(&self) -> Option<AsrEvent> {
        lock(&self.events).pop_front()
    }

    /// Used by the C ABI when the caller's buffer was too small: the event is
    /// put back instead of being lost.
    pub(crate) fn return_event(&self, event: AsrEvent) {
        lock(&self.events).push_front(event);
    }

    pub fn last_error(&self) -> Option<String> {
        lock(&self.last_error).clone()
    }

    fn send(&self, cmd: Command) -> Result<(), AsrError> {
        self.tx
            .send(cmd)
            .map_err(|_| AsrError::State("worker stopped".into()))
    }

    fn record_error(&self, e: &AsrError) {
        *lock(&self.last_error) = Some(e.to_string());
    }
}

impl Drop for Asr {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(handle) = lock(&self.join).take() {
            let _ = handle.join();
        }
    }
}

struct Worker {
    cfg: AsrConfig,
    kind: ModelKind,
    recognizer: Option<Box<dyn Recognizer>>,
    segmenter: Option<Box<dyn Segmenter>>,
    framer: PcmFramer,
    window: Vec<f32>,
    total_samples: u64,
    level_sq: f64,
    level_n: u64,
    active: bool,
    warned_not_started: bool,
    segment_index: u32,
    texts: Vec<String>,
    events: Events,
    last_error: LastError,
    cancel: Arc<AtomicBool>,
}

impl Worker {
    fn new(cfg: AsrConfig, events: Events, last_error: LastError, cancel: Arc<AtomicBool>) -> Self {
        let kind = cfg.language.model_kind();
        Self {
            cfg,
            kind,
            recognizer: None,
            segmenter: None,
            framer: PcmFramer::default(),
            window: Vec::with_capacity(WINDOW),
            total_samples: 0,
            level_sq: 0.0,
            level_n: 0,
            active: false,
            warned_not_started: false,
            segment_index: 0,
            texts: Vec::new(),
            events,
            last_error,
            cancel,
        }
    }

    fn run(mut self, rx: Receiver<Command>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Command::Load(reply) => {
                    let result = self.load();
                    if let Err(e) = &result {
                        self.error_and_store("model_load", &e.to_string());
                    }
                    let _ = reply.send(result);
                }
                Command::Start => self.start(),
                Command::Feed(bytes) => self.feed(&bytes),
                Command::Finish => self.finish(),
                Command::Cancel => self.cancel(),
                Command::Shutdown => break,
            }
        }
    }

    fn load(&mut self) -> Result<ModelInfo, AsrError> {
        let kind = self.kind;
        let dir = self.cfg.model_dir(kind);
        let manifest_path = dir.join(MANIFEST_FILE);
        let json = std::fs::read_to_string(&manifest_path)
            .map_err(|e| AsrError::Model(format!("read {}: {e}", manifest_path.display())))?;
        let manifest = ModelManifest::from_json(&json)?;
        let report = manifest::verify(&dir, &manifest, self.cfg.verify_models)?;
        let files = ModelFiles::from_manifest(&dir, &manifest, kind)?;
        let recognizer = engine::load(kind, &files, &self.cfg)?;
        let segmenter = vad::load(&self.cfg)?;

        self.recognizer = Some(recognizer);
        self.segmenter = Some(segmenter);
        self.active = false;
        self.reset_audio();
        Ok(ModelInfo {
            engine: kind.engine().to_string(),
            model_dir: dir.to_string_lossy().into_owned(),
            bytes: report.bytes,
            files: report.files,
        })
    }

    fn start(&mut self) {
        if self.recognizer.is_none() || self.segmenter.is_none() {
            self.error_and_store("not_loaded", "call load_model first");
            return;
        }
        lock(&self.events).clear();
        self.reset_audio();
        self.texts.clear();
        self.segment_index = 0;
        self.active = true;
        self.warned_not_started = false;
        self.cancel.store(false, Ordering::SeqCst);
    }

    fn feed(&mut self, bytes: &[u8]) {
        if !self.active {
            if !self.warned_not_started {
                self.warned_not_started = true;
                self.error_and_store("not_started", "feed_pcm before start");
            }
            return;
        }
        if self.cancel.load(Ordering::SeqCst) {
            return;
        }
        let mut samples = Vec::new();
        self.framer.push(bytes, &mut samples);
        for sample in samples {
            if self.cancel.load(Ordering::SeqCst) {
                return;
            }
            self.total_samples += 1;
            self.level_sq += (sample as f64) * (sample as f64);
            self.level_n += 1;
            self.window.push(sample);
            if self.window.len() == WINDOW {
                let window = std::mem::replace(&mut self.window, Vec::with_capacity(WINDOW));
                self.on_window(&window);
            }
            if self.level_n >= LEVEL_SAMPLES {
                let db = audio::rms_db(self.level_sq / self.level_n as f64);
                self.emit(AsrEvent::Level { rms_db: db });
                self.level_sq = 0.0;
                self.level_n = 0;
            }
        }
    }

    fn on_window(&mut self, window: &[f32]) {
        let segments = match self.segmenter.as_mut() {
            Some(segmenter) => segmenter.push(window),
            None => return,
        };
        self.process_segments(segments);
    }

    fn finish(&mut self) {
        if !self.active {
            self.error_and_store("not_started", "finish before start");
            return;
        }
        if !self.window.is_empty() {
            let mut window = std::mem::replace(&mut self.window, Vec::with_capacity(WINDOW));
            window.resize(WINDOW, 0.0);
            self.on_window(&window);
        }
        let segments = match self.segmenter.as_mut() {
            Some(segmenter) => segmenter.flush(),
            None => Vec::new(),
        };
        self.process_segments(segments);

        let mut text = audio::join_texts(&self.texts);
        let truncated = audio::truncate_text(&mut text);
        let duration_ms = audio::samples_to_ms(self.total_samples);
        let segment_count = self.segment_index;
        self.active = false;
        self.reset_audio();
        self.emit(AsrEvent::Final {
            text,
            duration_ms,
            segment_count,
            truncated,
        });
    }

    fn cancel(&mut self) {
        self.active = false;
        lock(&self.events).clear();
        self.reset_audio();
        self.texts.clear();
        self.segment_index = 0;
        self.warned_not_started = false;
        self.cancel.store(false, Ordering::SeqCst);
    }

    fn process_segments(&mut self, segments: Vec<VadSegment>) {
        for segment in segments {
            if self.cancel.load(Ordering::SeqCst) {
                return;
            }
            let result = match self.recognizer.as_mut() {
                Some(recognizer) => recognizer.transcribe(&segment.samples),
                None => continue,
            };
            // cancel() may have arrived while the decoder was running.
            if self.cancel.load(Ordering::SeqCst) {
                return;
            }
            match result {
                Ok(recognition) => {
                    let mut text = recognition.text.trim().to_string();
                    if text.is_empty() {
                        continue;
                    }
                    let truncated = audio::truncate_text(&mut text);
                    let lang = audio::normalize_lang(&recognition.lang);
                    let lang = if lang.is_empty() {
                        self.fallback_lang().to_string()
                    } else {
                        lang
                    };
                    let start_ms = audio::samples_to_ms(segment.start_sample);
                    let end_ms = audio::samples_to_ms(
                        segment
                            .start_sample
                            .saturating_add(segment.samples.len() as u64),
                    );
                    let index = self.segment_index;
                    self.segment_index += 1;
                    self.texts.push(text.clone());
                    self.emit(AsrEvent::Segment {
                        index,
                        start_ms,
                        end_ms,
                        text,
                        lang,
                        engine: self.kind.engine().to_string(),
                        truncated,
                    });
                }
                Err(e) => self.error_and_store("recognition", &e.to_string()),
            }
        }
    }

    fn fallback_lang(&self) -> &'static str {
        match self.cfg.language {
            crate::config::Language::Zh => "zh",
            crate::config::Language::En => "en",
            crate::config::Language::Auto => "auto",
        }
    }

    fn reset_audio(&mut self) {
        self.framer.reset();
        self.window.clear();
        self.total_samples = 0;
        self.level_sq = 0.0;
        self.level_n = 0;
        if let Some(segmenter) = self.segmenter.as_mut() {
            segmenter.reset();
        }
    }

    fn emit(&self, event: AsrEvent) {
        let mut queue = lock(&self.events);
        if queue.len() >= MAX_QUEUED_EVENTS {
            queue.pop_front();
        }
        queue.push_back(event);
    }

    fn error_and_store(&mut self, code: &str, message: &str) {
        *lock(&self.last_error) = Some(format!("{code}: {message}"));
        self.emit(AsrEvent::Error {
            code: code.to_string(),
            message: message.to_string(),
        });
    }
}

#[cfg(test)]
mod tests;
