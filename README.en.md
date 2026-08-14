**[中文](README.md) · English**

# Open People Input (OPI)

## An input method that goes back to the basics — for everyone, on every device

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
| **Core engine** | Pure Rust (`engine-core` / `engine-data` / `opi-tools` / `opi-ffi` multi-crate workspace), lightweight and efficient |
| **FFI layer** | opi-ffi dual ABI exports: JNI (Android) + C (iOS reserve), replacing flutter_rust_bridge |
| **Client UI** | Native per platform: Jetpack Compose on Android (keyboard, candidate bar, panels, settings) — no cross-platform framework |
| **Platform integration** | Android (InputMethodService), Linux (fcitx5), Windows (TSF), iOS (M7), HarmonyOS (IME Kit) |
| **Data sync** | Reserved for V2: end-to-end encryption + self-hosted support — use the official service or run your own sync server |

### 🏗 Build & Test

```bash
cargo test --workspace                   # unit + integration + property tests
cargo clippy --workspace --all-targets -- -D warnings   # gate: zero warnings
cd android && ./gradlew testDebugUnitTest   # Android unit tests (engine FFI + IME state machine + key routing)
cd android && ./gradlew assembleDebug       # build debug APK (cargokit compiles opi-ffi three-ABI .so)
```

Repository structure:

```
crates/
  engine-core/    # pure logic core, no IO, no platform dependencies
    src/          # composer / pinyin / trie / dictionary / learner / symbols / candidates / engine
    tests/        # engine_integration + proptests
  engine-data/    # .opid binary dictionary: format, FNV-1a checksum, mmap loading, corruption fallback (M2)
  opi-tools/      # dictionary compiler: dict.yaml → .opid + verify (M2)
  opi-ffi/        # dual ABI exports: JNI (Android) + C (iOS reserve) (M6)
android/          # Android IME (Kotlin + Jetpack Compose)
  app/            # IME service + keyboard/candidate bar/panels + settings
  rust_builder/   # standalone cargokit: compiles crates/opi-ffi → three-ABI .so (JNI)
docs/superpowers/ # design specs and implementation plans (multi-platform: specs/2026-08-14-opi-multi-platform-design.md)
data/             # dictionary source data (raw) and build artifacts (generated; fallback.opid checked in)
```

### 📅 Project Status

> **Current stage: M6 native rewrite ✅ complete (2026-08): Flutter deleted, fully native Android**

V1 milestone progress:

- [x] **M1 Engine core**: cargo workspace + Composer key state machine + pinyin syllable table/segmentation + Trie dictionary + candidate ranking & merge + local learning + Unicode symbol engine + Engine facade (62 tests green, clippy zero warnings)
- [x] **M2 Data pipeline**: opi-tools compiles dictionaries → `.opid` binary (mmap loading, verification, corruption fallback)
- [x] **M3 FFI**: flutter_rust_bridge bindings + EngineController (superseded by M6 opi-ffi dual ABI)
- [x] **M4/M5 Android integration & UI**: InputMethodService + keyboard/panels/settings (Flutter version, natively rewritten in M6)
- [x] **M6 Native rewrite**: opi-ffi dual ABI (JNI + C) replaces frb; Compose native IME + keyboard/candidate bar/panels/settings; flutter/ deleted
- [ ] **M7 Multi-platform**: Linux fcitx5 plugin / Windows TSF / iOS (C ABI)

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
