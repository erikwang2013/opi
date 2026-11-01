# opi-asr

Shared offline ASR core for the opi platform shells. P1 scope: model manager,
16 kHz PCM16 input, Silero VAD segmentation, offline recognition, events, C ABI.
Platform UI is not touched by this crate.

## Runtime

One runtime for both models: **sherpa-onnx v1.12.9** through the
[`sherpa-rs`](https://crates.io/crates/sherpa-rs) 0.6.8 bindings. SenseVoice and
Whisper both use its offline recognizer, so there is no second decoder and no
hand-written inference code.

| Component | Version | Size | License |
|---|---|---|---|
| sherpa-onnx (prebuilt, build-time) | v1.12.9 | ~25 MB archive, ~45 MB extracted | Apache-2.0 |
| ONNX Runtime (bundled in the prebuilt) | 1.22.x | included | MIT |
| sherpa-rs / sherpa-rs-sys | 0.6.8 | Rust glue | MIT |

`cargo` downloads the prebuilt sherpa-onnx archive **at build time** only
(`download-binaries` feature). To use libraries you cross-compiled yourself,
set `SHERPA_LIB_PATH` to a directory containing `lib/` and `include/`; the
build script then skips the download and links those. Platform shells own
mobile packaging; nothing in this crate downloads at runtime.

## Models

Language routing is fixed: `zh`/`auto` -> SenseVoice, `en` -> Whisper.
`auto` also uses SenseVoice's own language ID. Language is chosen in the config
before `load_model`; changing it needs a new handle.

| Role | File (role name) | Bytes | Notes |
|---|---|---|---|
| SenseVoice | `model.int8.onnx` (`model`) | 239,233,841 | `sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17` |
| SenseVoice | `tokens.txt` (`tokens`) | 315,894 | |
| Whisper small.en | `small.en-encoder.int8.onnx` (`encoder`) | 112,442,483 | ONNX export of OpenAI Whisper small.en |
| Whisper small.en | `small.en-decoder.int8.onnx` (`decoder`) | 262,223,042 | int8, greedy |
| Whisper small.en | `small.en-tokens.txt` (`tokens`) | 835,554 | |
| VAD | `silero_vad.onnx` | 643,854 | Silero VAD, not in a model manifest |

Licenses:

* SenseVoice weights: FunASR **MODEL_LICENSE** ("other" on Hugging Face,
  <https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE>). Review before
  redistribution; it is not Apache-2.0 despite the code repo's license.
* Whisper small.en weights: MIT (OpenAI). ONNX conversion/export: Apache-2.0.
* Silero VAD: MIT.
* sherpa-onnx: Apache-2.0. ONNX Runtime: MIT.

The core has **no network code**. The platform shell downloads the archives and
verifies them with `opi_asr_verify_model` before use.

## Manifest

Each model directory has `manifest.json`:

```json
{
  "version": 1,
  "files": [
    {"path": "model.int8.onnx", "size": 239233841, "sha256": "<64 hex>", "role": "model"},
    {"path": "tokens.txt", "size": 315894, "sha256": "<64 hex>", "role": "tokens"}
  ]
}
```

Roles: SenseVoice = `model` + `tokens`; Whisper = `encoder` + `decoder` +
`tokens`. Verification always checks size and path containment (no absolute
paths, no `.`/`..`, canonical target must stay inside the directory); SHA-256 is
checked when `verify_models` is true (default). `sha256_file` and the C ABI
`opi_asr_verify_model(dir, manifest_json)` return/perform the same checks.

## Config

```json
{
  "models_dir": "/data/opi/models",
  "sense_voice_dir": "sensevoice",
  "whisper_dir": "whisper-small.en",
  "vad_model": "vad/silero_vad.onnx",
  "language": "auto",
  "num_threads": 2,
  "verify_models": true,
  "max_segment_ms": 30000,
  "vad_min_silence_ms": 500,
  "vad_threshold": 0.5
}
```

`models_dir` may be empty, in which case model paths must be absolute.

## C ABI

```c
void        *opi_asr_create(const char *config_json);      // NULL on error
void         opi_asr_destroy(void *handle);                // last call
int          opi_asr_load_model(void *handle);             // blocking
int          opi_asr_start(void *handle);
int          opi_asr_feed_pcm(void *handle, const uint8_t *data, int32_t len);
int          opi_asr_finish(void *handle);
int          opi_asr_cancel(void *handle);
int          opi_asr_poll_event(void *handle, char *buf, int32_t cap);
int          opi_asr_last_error(void *handle, char *buf, int32_t cap);
int          opi_asr_max_event_bytes(void);                // 65536
char        *opi_asr_verify_model(const char *dir, const char *manifest_json);
void         opi_asr_string_free(char *s);
```

Return codes: `0` ok; `-1` argument; `-2` config; `-3` model; `-4` state;
`-5` busy (feed queue full); `-6` I/O; `-7` runtime unavailable (built without
`sherpa`); `-8` buffer too small (event/error is kept).

* `poll_event` is non-blocking: `>0` JSON length, `0` empty. Buffer is
  NUL-terminated. Allocate `opi_asr_max_event_bytes()`.
* `feed_pcm` is non-blocking and accepts any even/odd byte split across calls
  (PCM16 LE; a trailing odd byte is buffered). Max 256 KiB per call, 32 queued
  chunks; `-5` means retry/pace the caller. Audio never leaves memory and is
  not written to disk.
* Thread contract: the worker thread owns all decoder/VAD state. Every function
  except `destroy` may be called from any thread. `destroy` must be the last
  call for a handle (no concurrent calls). An ONNX decode already running
  cannot be interrupted; `cancel` discards its result and clears queued events.
* `last_error(NULL, ...)` returns the `opi_asr_create` failure. No transcript
  text is ever written to logs; errors carry paths/codes only.

Events (one JSON object per `poll_event`):

```json
{"type":"level","rms_db":-32.5}
{"type":"segment","index":0,"start_ms":760,"end_ms":5600,"text":"...","lang":"zh","engine":"sensevoice","truncated":false}
{"type":"final","text":"...","duration_ms":5592,"segment_count":1,"truncated":false}
{"type":"error","code":"recognition","message":"..."}
```

A segment text is capped at 32 KiB; `truncated` marks the cut. The event queue
keeps at most 1024 entries (oldest dropped), so poll while recording.

## Build and test

```bash
cargo test -p opi-asr                     # unit + ABI, no model needed
cargo test -p opi-asr --no-default-features   # no ONNX runtime build
```

Real-model tests are ignored by default:

```bash
OPI_ASR_TEST_ROOT=/home/erik/tmp/asr-bench/models \
OPI_ASR_TEST_WAV_ZH=/home/erik/tmp/asr-bench/testset/real_zh_sv.wav \
OPI_ASR_TEST_WAV_EN=/home/erik/tmp/asr-bench/testset/real_en_jfk.wav \
cargo test -p opi-asr --test real_wav -- --ignored --nocapture
```

Expected layout under `OPI_ASR_TEST_ROOT`: `sensevoice/manifest.json`,
`whisper-small.en/manifest.json`, `vad/silero_vad.onnx`. Set
`OPI_ASR_TEST_NO_VERIFY=1` to skip SHA-256 and measure the ONNX side alone.

Observed on the bench machine (debug test build, load average ~37; SHA-256 off):
SenseVoice transcribed `real_zh_sv.wav` as `开饭时间早上9点至下午5点。` (5.59 s
audio, load 4.3 s, RTF 0.097); Whisper small.en transcribed the JFK clip in four
VAD segments (11.0 s audio, load 5.7 s, RTF 0.84). With `verify_models` on, the
debug-build SHA-256 pass added 76 s / 28 s; release builds hash far faster.
Whisper ONNX was viable in the same runtime, so no whisper.cpp runtime was
added. These are single-clip sanity checks, not a CER sweep.

## P1 limits

* Offline decoder: one recognition per VAD segment, no streaming partials.
* Language is fixed per handle; Whisper is pinned to English.
* Model download/resume UI and mobile library packaging belong to the shells.
* `auto` routes to SenseVoice, whose mixed zh/en error rate is higher than the
  per-language special cases (~20% bench CER/WER).
* Every `load_model` re-hashes the selected model by default (release cost is
  low, debug tests are slow); a cached verification token is future work.
