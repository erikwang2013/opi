**[中文](README.md) · English**

# Open People Input (OPI)

## An input method that goes back to the basics — for everyone, on every device

<img src="docs/opi-pet.svg" width="168" align="right" alt="OPI's project pet: Opi the keycap sprite">

### 🐾 Meet Opi

**Opi** is OPI's project pet — a **keycap sprite**.

It looks like a single key on a keyboard, because that is what an input method ought to be: nothing more.

| On its body | What it means |
|---|---|
| **Body = a keycap** | The essence of input. Never steals the spotlight; does one small thing |
| **Mouth = the letter O** | Open — open source, open dictionaries, auditable |
| **Antenna = the pinyin tone mark ˉ** | Pinyin input. Cinnabar red — the only warm colour on the whole figure |
| **Side legend = OPI** | The legend position on a keycap, and the project's name |
| **Ripples below** | The afterglow of a keypress, spreading only on this machine — it never leaves |

See [`docs/opi-pet.svg`](docs/opi-pet.svg) for the full figure (hand-written SVG, no external dependencies, scales to any size). It already lives inside the codebase:

- **Android settings page**: a Compose component, `OpiPet`, whose expression tracks engine state — idle / waiting / puzzled / asleep (turn learning off and it dozes)
- **Empty states**: on both the Android candidate bar and the Windows candidate window, Opi shrugs when pinyin yields no candidates, replacing what used to be a blank gap
- **The app launcher icon**: adaptive icon, pure VectorDrawable, no extra PNG density buckets
- **The `opi-tools` CLI**: prints a character-art Opi on a successful compile or verify (single-width characters only, so it never misaligns in a CJK terminal), and `--version` carries it too

Both Compose frontends (Android and the Windows candidate window) share a single [`shared/pet/OpiPet.kt`](shared/pet/OpiPet.kt). Not sharing UI code across platforms is a principle of this project — but the pet is one drawing, and two copies of the same geometry inevitably drift apart.

<br clear="right">

### 📖 Origin Story

**Born from frustration, built with passion.**

Input methods have become the digital "infrastructure" of daily life — and yet we find ourselves increasingly exasperated by the absurdity:

- You want to type a rare character, and you page through three screens of candidates without finding it
- You turn off every privacy switch, yet the input method still "helpfully" pushes ads for things you just chatted about
- The dictionary grows bigger, but the word you use most is always at the bottom
- Pop-ups, skin shops, AI assistants… so many features it's dizzying, and it still can't do the one thing that matters — typing well

We're not against iteration and evolution. The problem is that, in the race to become "big and comprehensive", many input methods have lost sight of their core mission — **making input itself simple, accurate, and efficient.**

And so **Open People Input** was born.

It's a serious rebellion born of "enough is enough" — and a gift to every user who is disappointed with the status quo.

### 🎯 What It Stands For

**Open People Input** is an **open, pure, cross-platform** input method, committed to giving every user an input experience that is **undisturbed, unobserved, and unconstrained.**

The name says it all:

| | Meaning |
|---|---|
| **Open** | Open engine, open dictionaries, transparent and auditable. No black boxes, no backdoors — your input data belongs to you alone. |
| **People** | Designed for everyone — whatever device you use, whatever language you speak, whatever special needs you have, you deserve equal input rights. |
| **Input** | Back to the essence of input. We don't steal the spotlight — we do one small thing, and we do it to perfection. |

### ✨ Core Features

#### 1. True Cross-Platform Coverage
Build once, deploy everywhere. Covering **Android, iOS, HarmonyOS, Windows, macOS, Linux, and Web (including mini-apps)**, with a consistent input experience across devices, and end-to-end encrypted cloud sync for dictionaries and personalization.

#### 2. An Open and Transparent Ecosystem
- **Open source**: the core engine and primary client code are fully open, audited and contributed to by the community
- **Community dictionaries**: submit, review, and merge new words — keeping the dictionary truly "alive"
- **Custom input schemes**: pinyin, shuangpin, wubi, Cangjie, Bopomofo and more — you can even define your own input rules

#### 3. Privacy First
- **Local by default**: all input data stays on your device by default; nothing is uploaded
- **Works offline**: the core input features run fully offline with no network dependency
- **Optional cloud sync**: if you want multi-device sync, it's end-to-end encrypted — the server can never read your content

#### 4. Accessibility & Inclusivity
- Full screen-reader support (TalkBack, VoiceOver, NVDA, etc.)
- Voice input, scanning input, and other assistive input methods
- Built-in schemes for major minority languages and dialects (Tibetan, Uyghur, Mongolian, Cantonese, Wu, etc.)

#### 5. No Feature Bloat
- **Minimalist core mode**: every "extra" is an optional plugin; by default you get the cleanest input surface
- Install what you need, and nothing is forced on you

### 🛠 Tech Stack

| Layer | Approach |
|---|---|
| **Core engine** | Pure Rust multi-crate workspace (`engine-core` / `engine-data` / `opi-tools`), no IO, no platform dependencies |
| **Export layer** | `opi-ffi` dual ABI (JNI + C) · `fcitx5-opi` (cdylib) · `tsf-opi` (cdylib COM server) — each holds one in-process engine singleton |
| **Client UI** | Native per platform, no cross-platform framework: Jetpack Compose on Android; a C++ AddonInstance calling Rust on Linux; TSF COM plus a Compose Desktop candidate window on Windows (NDJSON over a named pipe) |
| **Platform integration** | Android (InputMethodService), Linux (fcitx5), Windows (TSF), iOS (M7, C ABI ready) |
| **Data sync** | Reserved for V2: end-to-end encryption + self-hosted support — use the official service or run your own sync server |
| **Versioning** | Single source of truth: `[workspace.package] version` in the root `Cargo.toml`, shared by all six crates; Android `versionName` and desktop `packageVersion` align to it, and releases are tagged with the same number |

### 🧭 Architecture

<img src="docs/diagrams/architecture.svg" alt="OPI architecture: client, export, engine, data and build-pipeline layers" width="100%">

**Five layers, dependencies pointing only downwards.** The lower the layer, the more stable; the higher, the closer to the user:

- **Client layer** — native UI per platform. The four clients share no UI code, only input semantics.
- **Export layer** — thin ABI shells doing type conversion, boundary validation and panic isolation. Each process holds one engine singleton: on Android the settings page and the IME share the same Rust singleton, so toggling learning in settings takes effect in the IME immediately.
- **Engine layer** — `engine-core`, **pure logic, no IO, no platform dependencies**. This is the heart of the project and the easiest layer to test: six modules (`Composer` state machine / `Pinyin` segmentation / `Trie` dictionary / `Candidates` ranking and merge / `Learner` / `Symbols`) under a single `Engine` facade that the UI layer talks to.
- **Data layer** — the `.opid` binary dictionary: 11-byte header + 14-byte fixed-size entry table + two blobs + an FNV-1a64 trailer. Loaded via read-only `mmap`, queried with zero copies.
- **Build pipeline** — TSV sources in `data/raw` are compiled by `opi-tools` into `.opid`, then pass checksum, ordering and coverage gates before being committed.

> **Key invariant: a broken dictionary must never crash the input method.** Any failure falls back to the 35-word built-in dictionary — only corruption of the built-in itself is unrecoverable.

### 🧩 Feature Design

<img src="docs/diagrams/features.svg" alt="OPI feature design: five input modes and six feature domains" width="100%">

**Five input modes** are decided by the `Composer` state machine using a mode integer (0..4, identical across all five platforms):

| Mode | Behaviour |
|---|---|
| **Pinyin** (0, default) | Lowercase letters and the `'` separator enter the buffer, capped at 16 chars; queries the main dictionary |
| **Traditional** (4) | Behaves exactly like Pinyin but routes to the `trad` dictionary; falls back to the simplified one if `trad` is missing |
| **English** (1) | `⇧` decides one-shot uppercase; with an empty buffer the key **never reaches the engine** — it commits directly |
| **Number** (2) | Digits only; space / enter commits |
| **Symbol** (3) | The buffer accepts nothing; the symbol panel commits directly. Pending pinyin is committed before the panel opens, leaving no residue |

Of the **six feature domains**, candidate ranking deserves a note. It has one counter-intuitive rule: **`limit` must not be pushed down into the dictionary query.** A learned low-frequency word can overtake a word outside the cut-off, so ranking must collect everything, sort globally, then deduplicate and truncate. The score is

```
score = static_freq + user_freq × boost
```

where `boost` **scales dynamically** with the dictionary's maximum frequency (`max_freq × 2`) rather than being a hard-coded constant. With an early hard-coded value of 100k, selecting 我 once still lost to the static word 倭 at luna's million-scale frequencies. Dynamic scaling is what guarantees the promise "select once and it beats every static word".

### 🔄 Lifecycle

<img src="docs/diagrams/lifecycle.svg" alt="OPI lifecycle: dictionary loading and the keystroke cycle" width="100%">

Two independent lifelines with a single intersection — **being installed into the process singleton**.

The **dictionary lifecycle** runs once at process start: locate (assets→filesDir on Android, the XDG data directory on fcitx5) → compare sizes to decide whether to re-copy (idempotent, guards against stale dictionaries) → `mmap` → verify → install into the singleton. Afterwards `install_trad` can hot-swap the traditional dictionary at any time without affecting simplified mode.

The **keystroke lifecycle** runs one lap per keypress in ten steps: key dispatch → `Composer` updates the buffer → `rank_and_pick` ranks → the candidate bar renders → the user selects → learning is recorded → text is committed → the buffer is cleared. The crucial branch is step two: **with an empty buffer the key never enters the engine** (English and number modes commit directly). That is what keeps the keyboard responsive — the engine is only woken when there is genuinely something to compose.

After loading, every keystroke is just a zero-copy lookup over read-only memory.

### 🏗 Build & Test

```bash
cargo test --workspace                   # unit + integration + property tests (245, incl. fcitx5 61 / TSF 42)
cargo clippy --workspace --all-targets -- -D warnings   # gate: zero warnings
cd android && ./gradlew testDebugUnitTest   # Android unit tests (engine FFI + IME state machine + key routing + pet)
cd android && ./gradlew assembleDebug       # build debug APK (cargokit compiles opi-ffi three-ABI .so)
cargo build --release -p fcitx5_opi         # Linux plugin (the C++ glue also needs fcitx5-dev)
cd desktop && ./gradlew package             # Windows candidate window (Compose Desktop)
./target/debug/opi-tools --version          # version + Opi
```

> **`assembleDebug` needs `dart` on the machine**: cargokit's build tool is written in Dart. Without it the `cargokitCargoBuildOpi_ffiDebug` task fails with `dart: command not found` (exit code 127). The unit tests don't need it — `testDebugUnitTest` runs on pure JVM fakes and never touches the `.so`.

### 📁 Repository Structure

```
crates/                        # Rust workspace (6 crates)
  engine-core/                 # pure logic core: no IO, no platform dependencies
    src/                       #   composer / pinyin / trie / dictionary /
                               #   candidates / learner / symbols / engine
    tests/                     #   engine_integration · proptests · trad_mode
  engine-data/                 # .opid binary dictionary: format, FNV-1a64 checksum, mmap load, fallback
  opi-tools/                   # dictionary compiler CLI: tsv / dict.yaml → .opid, with verify
  opi-ffi/                     # dual ABI exports: JNI (Android) + C (iOS)
  fcitx5-opi/                  # Linux fcitx5 plugin: Rust logic (cdylib) + cpp/ AddonInstance glue
  tsf-opi/                     # Windows TSF plugin: Rust logic + COM server + candidate-window protocol
android/                       # Android IME (Kotlin + Jetpack Compose)
  app/                         #   IME service · keyboard / candidate bar / panels / settings · pet component
    src/main/assets/           #   luna.opid (simplified) · trad.opid (traditional)
    src/main/res/              #   adaptive launcher icon (VectorDrawable, featuring Opi)
  rust_builder/                #   standalone cargokit: compiles crates/opi-ffi → three-ABI .so
  jni_smoke/                   #   JNI connectivity smoke test
desktop/                       # Windows candidate window (Compose Desktop / JVM, NDJSON over named pipe)
shared/                        # Kotlin sources shared across clients
  pet/OpiPet.kt                #   the Compose drawing of Opi (one copy, used by Android and desktop)
data/                          # dictionary data
  raw/                         #   TSV sources + LICENSES.md (per-item source and licence records)
  generated/                   #   build artifacts: fallback.opid · luna.opid · trad.opid
docs/                          # pet, diagrams and design docs
  opi-pet.svg                  #   the project pet, Opi
  diagrams/                    #   architecture · features · lifecycle
  superpowers/                 #   specs (design) + plans (implementation)
scripts/                       # dictionary generation: gen_luna_dict.py · gen_trad_dict.py
```

### 📅 Project Status

> **Current stage: M6 multi-platform native ✅ (2026-08): Flutter deleted, fully native Android**

V1 milestone progress:

- [x] **M1 Engine core**: cargo workspace + Composer key state machine + pinyin syllable table/segmentation + Trie dictionary + candidate ranking & merge + local learning + Unicode symbol engine + Engine facade
- [x] **M2 Data pipeline**: opi-tools compiles dictionaries → `.opid` binary (mmap loading, verification, corruption fallback)
- [x] **M3 FFI**: flutter_rust_bridge bindings + EngineController (superseded by the M6 opi-ffi dual ABI)
- [x] **M4/M5 Android integration & UI**: InputMethodService + keyboard/panels/settings (Flutter version, natively rewritten in M6)
- [x] **M6a Android native rewrite**: opi-ffi dual ABI (JNI + C) replaces frb; Compose native IME + keyboard/candidate bar/panels/settings; flutter/ deleted
- [x] **M6b Simplified/Traditional dual dictionary**: `Mode::Traditional` + dual-dictionary routing + `trad.opid` + a GB2312 single-character coverage gate
- [~] **M6c Linux fcitx5 plugin**: Rust logic and unit tests done; C++ AddonInstance glue written — needs `fcitx5-dev` headers to compile and be accepted
- [~] **M6d Windows TSF plugin**: Rust logic, candidate-window wire protocol and the Compose Desktop window done — the COM server is target-gated, awaiting acceptance on Windows
- [ ] **M7 iOS**: C ABI export ready; the SwiftUI keyboard extension is still to come

### 📄 License

- **Code**: MIT
- **Dictionary data**: declared per upstream license (rime-luna-pinyin is LGPL-3.0), with per-item source and license records in `data/raw`

---

### 💬 A Final Word

> *"I couldn't take it anymore, so I built one myself."*

---

### 🤝 Support Us

> If OPI helps you, scan the QR code below to support us (WeChat / Alipay — any amount is welcome, the thought is what counts).

| WeChat rewards | Alipay rewards |
|---|---|
| <img src="docs/weixinpay.png" width="130" height="130" alt="WeChat rewards QR code"> | <img src="docs/alipay.png" width="130" height="130" alt="Alipay rewards QR code"> |
