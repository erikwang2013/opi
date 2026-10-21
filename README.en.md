**[中文](README.md) · English**

# Open People Input (OPI)

The Chinese name is **小欧** (also the project pet's name); the English name is **opi**. The input method is **小欧输入法** (opi IME is also acceptable; 小欧输入法 is canonical).

## An input method that goes back to the basics — for everyone, on every device

> **Want OPI on your own machine?** The per-platform tutorials live in
> **[`docs/install/`](docs/install/README.md)** (build / package / install / use).
> **One step is mandatory after installing**: **add OPI to the input method group** — a fresh
> install is not enabled automatically, and without it OPI shows up in the list
> while typing does nothing. Steps: [`docs/install/linux.md`](docs/install/linux.md) §4.

> **Website**: [https://opi.erik.xyz](https://opi.erik.xyz) (source: [`site/index.html`](site/index.html)) — a scroll-snap tour of OPI, a 3D keycap environment and first-visit memory timeline, feature iterations, the full release log, and per-platform downloads (donation QR codes open in place; the footer carries the ICP filing number).
>
> **Verification scope**: hardware and cost are limited, so only **Android (emulator) and Linux (container / local)** have runtime verification; **Windows / iOS / macOS / HarmonyOS** have client code or buildable artifacts but no real-device acceptance, and signing is unverified.
>

![OPI's project pet: Opi the keycap sprite](docs/opi-pet.svg)
<img src="docs/site-preview.png" alt="OPI website: an input method that goes back to the basics" width="760">

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

See [`docs/opi-pet.svg`](docs/opi-pet.svg) for the full figure (hand-written SVG, no external dependencies, scales to any size); the twelve expressions (smile / joy / cry / dejected / terrified / angry / coquettish / cute / pleasing / knocking / face-palm / tongue-out) are in [`docs/opi-pet-forms.svg`](docs/opi-pet-forms.svg); **animated GIF preview**: [`docs/opi-pet-forms.gif`](docs/opi-pet-forms.gif). It already lives inside the codebase:

- **Android**: the settings-page expression tracks engine state (idle / waiting / puzzled / asleep / degraded); three empty states — the candidate bar's idle slot, "no match", and an empty symbol search; the **twelve-form pet strip** in the default page's top slot (smile / joy / cry / dejected / terrified / angry / coquettish / cute / pleasing / knocking / face-palm / tongue-out, animated with mood colours); and the **launcher icon**, an adaptive vector foreground with five PNG fallbacks
- **Windows**: Opi shrugs in the candidate window's empty state; the shipped `.deb` / Windows EXE installer app icon; and a **language-bar button** (TSF; visible only while the language bar is on)
- **HarmonyOS**: the pet in the candidate bar — it is itself the "remember my words" switch; plus the app/subtype icon
- **macOS**: the input-source menu icon (`macos/opi.tiff`) — CI-compiled (unsigned pkg/app), not device-tested; the icon is only visible on a Mac
- **Linux (fcitx5)**: the input method's icon in the input-method list and the tray
- **The `opi-tools` CLI**: prints a character-art Opi on a successful compile or verify (single-width characters only, so it never misaligns in a CJK terminal), and `--version` carries it too

Both Compose frontends (Android and the Windows candidate window) share a single [`shared/pet/OpiPet.kt`](shared/pet/OpiPet.kt). Not sharing UI code across platforms is a principle of this project — but the pet is one drawing, and two copies of the same geometry inevitably drift apart.

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

> This section lists only what **actually runs in the code today**. Everything still on the wish list has been moved down into "🚧 Roadmap", each item labelled with its status. Earlier versions of this README mixed the two together and wrote both as if they were current fact; that is fixed here.

#### 1. One Engine, Native Clients per Platform
**The engine is written once.** `engine-core` is a pure-logic core with no IO and no platform dependencies — **the module list is whatever `crates/engine-core/src/lib.rs` declares as `pub mod`**, and the ones usually named are `Composer` state machine / `Pinyin` segmentation / `Trie` dictionary / `Candidates` ranking and merge / `Learner` / `Symbols` (**that is a reading guide, not a census** — jianpin `jianpin`, fuzzy pinyin `fuzzy`, punctuation `punctuation` and byte order `bytes` live on the same `pub mod` list). Above them sits a single `Engine` facade that the UI layer talks to. There is also a **platform-neutral key-routing layer** (`router.rs`'s `KeyRouter` + `keys.rs`'s keycode table), used by the Apple and HarmonyOS clients through the C ABI — it lives here because it is pure logic; it is a **routing layer, not an engine module**, and its semantics follow the two existing tracks rather than inventing new ones. Above it sit three thin export shells (`opi-ffi` dual ABI · `fcitx5-opi` · `tsf-opi`), each holding one engine singleton per process.

**All six clients have client code**: Android (native Compose IME), Linux (fcitx5 plugin), Windows (TSF plugin + Compose Desktop candidate window — the latter is a **separate UI codebase** talking to TSF over a named pipe), iOS, macOS and HarmonyOS. They share no UI code — only input semantics; only Android (emulator) and Linux (container / local) have runtime verification so far, and Windows / iOS / macOS / HarmonyOS have no real-device evidence yet (see the next paragraph).

**Both Apple platforms and HarmonyOS now have buildable clients, but none is device-verified** — the three directories ([`ios/`](ios/), [`macos/`](macos/), [`harmony/`](harmony/)) have long outgrown their skeletons. **Corrected 2026-10-15**: the Swift sides (`ios/` · `macos/`) **are genuinely compiled by CI** (`swiftc … -typecheck` plus a real link in the `apple.yml` iOS / macOS jobs, green since 2026-10-15, which caught a real error the same day), with iOS producing a host app + unsigned IPA and macOS an unsigned pkg/app; **corrected 2026-10-19**: `harmony/` is now a complete hvigor project, the local HarmonyOS CLI builds it with `hvigorw assembleHap`, and an unsigned HAP has been produced (containing `libopiime.so`; `luna.opid` / `stroke.opid` / `wubi86.opid` are copied from rawfile into the sandbox on first run and loaded through the C ABI by absolute path). ⚠️ **The three are blocked for different reasons — do not bundle them into one sentence**: the two Apple platforms are blocked by **the development machine** (no macOS/Xcode/Apple SDK here; the compilation happens on CI's macOS runner); HarmonyOS is blocked by **zero signing and device evidence** (no signing material and no device, so installation, the IME list and runtime behaviour are all unverified). **Also ready is the platform-neutral C ABI** (`crates/opi-ffi`, see `tests/cabi_test.rs`) — it belongs to no single client, and **has been measured to compile for several Apple and HarmonyOS targets**. See the Roadmap for each one's hard constraint.

#### 2. Privacy First — and You Can Check It Yourself
- **Local by default**: your input data stays on your machine. Neither the engine nor the export layer has a **HTTP client** in its dependency table — no telemetry, no ad SDK
- **Zero permissions**: the Android `AndroidManifest.xml` declares **not a single `uses-permission`**
- **iOS Full Access**: enabled on 2026-10-20 so the keyboard extension can write own-sticker images to the system pasteboard (`RequestsOpenAccess=true`; the user must allow it in Settings, and iOS shows a privacy warning). OPI still makes no network calls, and user words stay in the extension container
- **Works offline**: core input runs fully offline; `.opid` dictionaries are read-only `mmap` maps, so the whole dictionary is never read into the heap (matching entries are materialised on demand at query time — `loader.rs`'s `MmapDictionary::query` calls `.to_string()` per hit and `candidates.rs`'s `rank_and_pick` clones again, so it is **not** zero-copy)

(Multi-device cloud sync has been **decided against** (user decision 2026-09-28) — see the Roadmap. **There is no sync or encryption code in the repository today.**)

#### 3. A Living Dictionary, With Gates Around It
- **Multi-source merge (2026-10-11, user decision "integrate the mainstream open-source dictionaries")**: rime-ice (Wusong) base+ext, rime-essay, chinese-xinhua idioms/xiehouyu and THUOCL technical terms, plus curated brand / new-word / reading-override / surname / slang tables, deduplicated into `luna.opid` (~96k entries → the ~900k range, 33MB; first-letter latency held by a top-N cap on the prefix layer — measurements and trade-offs in `data/raw/LICENSES.md`). ⚠️ The merged artifact is therefore distributed **as a whole under GPL-3.0** (data layer; the source code stays MIT)
- **Simplified + Traditional dictionaries**: `luna.opid` (Simplified) and `trad.opid` (Traditional), distributed per client (Windows ships Simplified only; macOS needs the user to add Trad, see the distribution paths below); a missing or broken traditional dictionary falls back to the simplified one. **Entry points**: Android via the mode-cycle key on the 26-key layout, and a dedicated Simplified/Traditional key on the 9/14-key and stroke layouts (`ImeScreen.kt`) / iOS via the keyboard mode-cycle key with `trad.opid` bundled (`KeyboardViewController.swift`); **fcitx5 (Linux) has `Ctrl+Shift+F` since 2026-10-09** (ships `trad.opid`, see `docs/install/linux.md` §5.3); **macOS has had `Ctrl+Shift+F` and an input-method menu entry since 2026-10-20** (the user must place `trad.opid` in the data directory for it to take effect); Windows / HarmonyOS have an action that switches the Traditional **mode** (status bar / 26-key cycle), but TSF loads only one dictionary and HarmonyOS has not bridged `opi_load_trad` ⇒ they still output Simplified — **neither has a usable Traditional entry yet**. The engine and data sides of Traditional are themselves complete. **2026-10-18**: `trad.opid` gained `trad_modern.tsv` (~42k modern common words, OpenCC phrase-level Simplified→Traditional) plus the 622-entry Kangxi delta, growing from 96,429 to 139,355 entries; `luna.opid` also gained the 1,279-entry Kangxi delta (937,780 entries)
- **Full single-character coverage gate**: every GB2312 character is asserted to produce candidates (`trad_coverage` integration test; the character-count criterion lives inside the test). Break the coverage with a dictionary change and CI goes red
- **A broken dictionary never crashes the input method**: the loading policy is the same on every client — **a bad path always returns `Err` and never falls back silently** (the comment on `engine-data/src/dictionary.rs`'s `load_or_fallback` records that the earlier silent fallback was deliberately removed: the UI would believe a full dictionary had loaded), and the built-in fallback dictionary (`data/raw/fallback.tsv`; take its size from that file's line count) is used only when **no path is configured at all** (an empty string counts as none). Whether to recover from that `Err` is the caller's decision: Android's `EngineLoader` catches it and retries with the built-in dictionary (`EngineLoader.kt`'s `fallback()`), fcitx5 propagates it (the load call site in `fcitx5-opi/src/lib.rs`), and TSF falls back candidate by candidate inside its resolver, worst case to the built-in 35 words (`tsf-opi/src/dict_path.rs`'s `load_dict`)
- **Dictionary distribution paths**: Android goes assets → filesDir (`EngineLoader.kt`), fcitx5 uses the XDG data directory and is installed by CMake (`fcitx5-opi/cpp/CMakeLists.txt`), and Windows tries `OPI_DICT_PATH` → the DLL's own directory → `%LOCALAPPDATA%\opi` → built-in fallback (`tsf-opi/src/dict_path.rs`). ⚠️ **The Windows user-facing artifact became an EXE installer, `opi-<version>-setup.exe`, on 2026-10-20** (Inno Setup; bundles the jpackage app image and `tsf_opi.dll`, installs to `C:\Program Files\opi` by default, and only logs a failed TSF registration instead of rolling back). **Since 2026-10-20 `luna.opid` ships with the package** (placed in the app image's `app\` directory next to `tsf_opi.dll`, together with three licence texts; CI asserts the installed dictionary's SHA-256), so a fresh install starts on the full dictionary; `trad.opid` still does not ship — the Windows side holds only one dictionary (the gap is documented in `dict_path.rs`'s module header)
- **Imported dictionaries and the Kangxi dictionary (2026-10-14 / 2026-10-16)**: one **import contract** across all clients — a `*.tsv` (`word<TAB>pinyin<TAB>freq`) dropped into the dictionary directory is loaded and can be toggled (Android `filesDir/dicts` · desktop app/TSF `%LOCALAPPDATA%\opi\dicts` · fcitx5 `$XDG_DATA_HOME/opi/dicts`). The **Kangxi dictionary** (`shunshi-kangxi-core`, MIT, all readings of 20,794 characters) ships with every package and is **seeded once on first run** (deleted stays deleted); the data pipeline is `scripts/gen_kangxi_dict.py` — provenance and coverage in `data/raw/LICENSES.md`. **Since 2026-10-18** the Kangxi (character, reading) pairs missing from the default dictionaries are also merged into `luna.opid` (1,279 entries) and `trad.opid` (622 entries), so they match even when the imported dictionary is deleted/not imported or on clients without an import path
- **Linux now produces distribution packages**: `scripts/build-packages.sh` plus `scripts/nfpm.yaml` move the `cmake --install` staging tree verbatim into `.deb` / `.rpm` (two `.so` files, two conf files, `luna.opid` and two licence texts — **check the package contents with `dpkg-deb -c` / `rpm -qlp` rather than copying this line**), and `.github/workflows/packages.yml` is **manually triggered** (`gh workflow run packages.yml --ref <tag-or-branch>`, or Actions → Packages → Run workflow) — artifacts land in that run's artifact `opi-packages`, and **they are attached to a release only if the tag input is filled in**. **The runtime floor is fcitx5 5.0**: it is declared in `crates/fcitx5-opi/data/addon/opi_fcitx5.conf`'s `[Addon/Dependencies]` (declare it too high and the addon does not load at all — the user gets an input method that can be selected and types nothing, with no opi-related error in the log), and CI guards it in a debian:12 container with positive / current / negative arms. ⚠️ This packaging path goes **only as far as "the artifact builds and the package contents match the staging tree"** — no gate has ever `dpkg -i` / `rpm -i`'d it; CI verifies the **same staging tree** (`cmake --install` + `cp`), not the package itself
- **Community dictionaries**: the source data is plain text in `data/raw/*.tsv`; submit, review and merge via PR. Process and **licensing requirements** are in [`CONTRIBUTING.md`](CONTRIBUTING.md)

#### 4. No Feature Bloat
- The engine does one thing: a key state machine for five modes (Pinyin / Traditional / English / Number / Symbol) plus jianpin / fuzzy pinyin, candidate ranking, and local learning. No skin shop, no pop-ups, no AI assistant
- **Jianpin** (abbreviation-only input: `nh` → 你好, `zg` → 中国): **on by default, with its own switch** on the engine side (`Engine::set_jianpin`, same origin as fuzzy pinyin; the switch arrived 2026-10-08) — **all three shipped ends can turn it off**: the Android settings page, Linux (the fcitx5 addon config / the desktop settings window writes that same conf) and Windows (the settings window writes `settings.ini`, re-read immediately through the control plane's `reload`; the next Activate applies it if the IME is not running). The trigger condition, the expansion rules and the cap all live in the header comment of `engine-core/src/jianpin.rs`, and the gate is `tests/jianpin_ranking.rs`
- **Exact matches have no cap; the prefix layer truncates by frequency**: the `FETCH_LIMIT` truncation and the one inside `Engine::select` are both gone, and the three copies agree (`engine-core/src/router.rs` · `fcitx5-opi/src/candidate.rs` · `tsf-opi/src/logic.rs`, with equality assertions between them); since 2026-10-11 the **prefix-expansion layer** (half-typed input) keeps a static-frequency top-N (9216 for the 26-key board / 2048 for 9- and 14-key, the two constants in `dictionary.rs` — rationale and measurements in [`docs/performance.md`](docs/performance.md)). ⚠️ **One host still uses the old cap**: Android (`EngineController.kt`'s `fetchLimit`) cannot reach the engine-side paging exports; macOS switched to `candidatesPage()` / `selectPage()` on 2026-10-20, while iOS and HarmonyOS already went through the paging exports and never pass a limit from the front end
- **Emoji and kaomoji (2026-10-18, animated 2026-10-19)**: the emoji panel now uses OpenMoji's common set (689 entries in 9 groups: smileys / people / animals / plants / food / travel / activities / objects / symbols); **Own is a separate top-level tab** (never mixed into Emoji/smileys), showing Opi's 12 forms as **animated GIF** stickers with static PNG fallback — Android inserts via `InputConnection.commitContent` trying **PNG → GIF** (PNG is the format hosts accept most readily; GIF is an animation bonus; the clipboard fallback uses the same order; editors that reject both get a distinguishable text tag prefixed with 小欧·, e.g. `[小欧·笑]`, not a Unicode emoji), while desktop/iOS/Harmony copy **both PNG and GIF** to the clipboard and ask the user to paste because their hosts cannot insert images (the paste target picks; on iOS the only channel that writes the pasteboard requires Full Access, and `Info.plist` has set `RequestsOpenAccess=true` since 2026-10-20, so the user must allow it in Settings). 390 kaomoji entries in 7 groups. Sticker sources live in `shared/pet/stickers/` and `shared/pet/stickers-gif/`; data and licences in [`data/raw/LICENSES.md`](data/raw/LICENSES.md); generators `scripts/gen_emoji_kaomoji.py` / `scripts/gen_pet_stickers.py`
- **Type-to-emoji (2026-10-18, bundled 2026-10-19)**: when a pinyin/English buffer (≥2 letters) matches a common-emoji keyword, the emoji is **appended after the text candidates** (`data/raw/emoji_keywords.tsv`, 295 entries; `weixiao`→😊, `smile`→😄; it never displaces the first text candidate; 9/14-key digit buffers are out of scope this round). The 12 forms also have **shortcodes** such as `:opi_smile:` (`shared/pet/emoji-font/map.tsv` / `shortcodes.tsv`; only Slack/Mattermost-style clients render them); ordinary text fields still get the original GIF/PNG sticker insertion with the 「小欧·」 text-label fallback. The PUA color font and its setting were removed on 2026-10-20; the own-sticker tab is controlled by the "表情中显示自有" switch (hidden by default). Mapping tables and shortcodes are proprietary project assets; see [`ASSETS-LICENSE.md`](ASSETS-LICENSE.md)
- **Character-by-character confirmation (user ruling 2026-10-17)**: when the whole input exactly matches a phrase it ranks first and one selection commits it; otherwise (a longer word that merely starts with the input does not count) the candidates first cover the leading character's full reading set — a confirmed character **stays in the candidate area** (the pinyin row shows "中guo"), and the whole string is committed only after the final segment is confirmed; with an empty buffer, ⌫ undoes the last confirmed character. The semantics live in the engine (`Engine::pending_text` / `commit_buffer_after_select`) and all four ends share them; gate: `tests/segment_commit.rs`
- **Where two ready mechanisms stand on wiring**: first, **Chinese punctuation and fullwidth are two independent switches** (engine-side `chinese_punct` and `fullwidth` are separate, gate `tests/punctuation_switches.rs`; `set_chinese_punct` / `toggle_chinese_punct` and both ABI exports exist) — **wired on the desktop status bar on both platforms** (the `。，` / `🌕` buttons: Linux goes through `opi-ctl`, Windows through the local control plane; the settings window's "Chinese punctuation" and the fcitx5 addon config both land on these two switches, and TSF on Windows reads the same values from `settings.ini` and re-reads them immediately through the control plane's `reload`); **Android's toolbar moon button was removed** (added 2026-10-12, removed 2026-10-13 by the user's ruling "the mobile function row does not need a full-width/half-width toggle" — the capability stays: fullwidth still applies automatically from each mode's default, and the letters/digits passthrough widening (`Engine::fullwidthize`) is untouched), while **Apple / HarmonyOS still have no UI entry point** (their `toggleFullwidth` still has only the JNI smoke table as its caller). **The key binding is not in the engine layer** — the comment in `engine_punct.rs` only *suggests* a tentative `Ctrl+/` for the two desktop tracks; it is not finalised and neither track implements it yet (that comment holds the reasoning and the measurements). Second, **persisting what was learned on the Rust side** (`engine-data/src/user_words.rs`, atomic write) still has **zero callers in the whole repo**: Android persists through its own Kotlin `UserWordStore.kt`, while fcitx5 and TSF still drop it after every lesson
- Ranking has one counter-intuitive rule — **`limit` must not be pushed down into the dictionary query** (a learned low-frequency word can overtake one outside the cut-off). Details like this are where this project is fussy — see "Feature Design" below

### 🚧 Roadmap

Everything below is either part of the vision or a direction that has been **decided against**. Most items have **no corresponding code today**; a few are partially done (see the status column). It is listed to draw the boundary clearly, not as a schedule:

| Direction | Status | How we checked |
|---|---|---|
| **More platforms**: Web (incl. mini-apps) | Not started | This means a Web **input method**; the repo now has a static website under [`site/`](site/index.html), but it is not a browser IME — the browser IME remains unstarted |
| **HarmonyOS** | **Buildable (unsigned)** (`harmony/`) | Complete hvigor project (`build-profile.json5` / `hvigorfile.ts` / module config and build scripts all present); the local HarmonyOS CLI + SDK runs `hvigorw assembleHap`, and an unsigned HAP (containing `libopiime.so`; `luna` / `stroke` / `wubi86` are copied from rawfile into the sandbox on first run) has been produced. The toolbar / skins / keyboard modes / symbol page / rare-character panel now match Android. CI uses the self-hosted `harmony.yml`. **Signing, installation, the real-device IME list and all the new panels remain unverified on hardware**; the Rust C ABI compiles for HarmonyOS targets. |
| **The two Apple platforms**: iOS · macOS | **CI-compiled + unsigned artifacts** (`ios/` · `macos/`) | `apple.yml` compiles the Swift on a macOS runner; iOS produces a host app + unsigned IPA (`Payload/Opi.app` plus the keyboard `.appex`; the keyboard now has the top slot / toolbar / skins / keyboard modes / about, and own stickers go through the pasteboard with Full Access enabled), macOS produces an unsigned pkg/app (symbol panel / input-method menu / settings window / candidate paging added). **Signing, notarization, real-device runs and IME enablement remain unverified**; the C ABI passes `cargo check` for the three Apple targets and its static-library exports have been checked. |
| **Multi-device sync / end-to-end encryption** | **Not doing** (user decision 2026-09-28) | No encryption library, no HTTP client: across `crates/` `android/` `desktop/` (`*.rs` / `*.toml` / `*.kt`), `reqwest` / `ureq` / `hyper` / `openssl` / `rustls` / `chacha` / `argon2` / `aes-gcm` / `https://` each return zero hits. Every `sync` hit is a Rust synchronisation primitive or a TSF flag (`std::sync`, `TF_ES_SYNC`, `trad_assets_in_sync`) — unrelated to cloud sync. **Note**: the exported learning-dictionary JSON does carry a `version` field, but that is version negotiation for the **export format** (`learner.rs`'s `import_json` rejects `version != 1`) and has nothing to do with cloud sync |
| **More input schemes**: shuangpin · wubi · Cangjie · Bopomofo · custom rules | Shuangpin (2026-10-13, six layouts) and wubi (2026-10-14, 86/98) landed; the rest not done | `Mode`'s variants are still Pinyin / Traditional / English / Number / Symbol (`composer.rs`); **key layout** is now a separate axis, `Scheme` (`scheme.rs`: full / pinyin-9key / pinyin-14key / stroke / shuangpin / wubi — matched by key-class expansion). Shuangpin and wubi are wired on Android / desktop / fcitx5 / TSF (iOS and HarmonyOS remain drafts, see their READMEs); Cangjie / Bopomofo / custom rules are still not done |
| **English word association** | **Wired (2026-10-18; normal by default, pro matches since 2026-10-19)** | **Normal mode commits letters directly**; only `pro` prefix-matches `data/raw/en_words.tsv` (10,000 common words, Google Books Ngrams v3, CC-BY 3.0; **not the original Oxford list** — OUP data is not licensed here, and swapping the TSV swaps the source). Candidates are frequency-ordered; for a single-letter buffer the **typed letter itself comes first** (case preserved, deduplicated against dictionary entries); space commits the first candidate, or the raw buffer when nothing matches; all four ends share the engine semantics, gate: `tests/english_words.rs` |
| **Accessibility**: screen readers | Partial (Android only) | The Android keyboard exposes basic screen-reader semantics: candidate changes are announced automatically (`liveRegion`), every key has a spoken name (no more reading out the glyph "⇧" / "⌫"), and the shift lock / one-shot state is readable. **Other platforms (Windows / Linux / iOS) are not covered** |
| **Voice input · scanning input** | Not started | Zero hits |
| **Minority languages and dialects**: Tibetan · Uyghur · Mongolian · Cantonese · Wu | Not started | Zero hits |
| **Plugin system**: every "extra" as an optional plugin | Not started | No plugin registry, no plugin interface, no dynamic loading — and there is nothing that needs a plugin yet |

### 🛠 Tech Stack

| Layer | Approach |
|---|---|
| **Core engine** | Pure Rust multi-crate workspace (`engine-core` / `engine-data` / `opi-tools`); **`engine-core` alone has no IO and no platform dependencies**, `engine-data` does the file mapping and byte parsing, and `opi-tools` is the compile CLI |
| **Export layer** | `opi-ffi` dual ABI (JNI + C) · `fcitx5-opi` (cdylib) · `tsf-opi` (cdylib COM server) — each holds one in-process engine singleton |
| **Client UI** | Native per platform, no cross-platform framework: Jetpack Compose on Android; a C++ AddonInstance calling Rust on Linux; TSF COM plus a Compose Desktop candidate window on Windows (NDJSON over a named pipe) |
| **Platform integration** | Android (InputMethodService), Linux (fcitx5), Windows (TSF), iOS / macOS (CI-compiled + unsigned artifacts), HarmonyOS (complete hvigor project + unsigned HAP); real-device runs and signing for Windows / Apple / HarmonyOS remain unverified |
| **Data sync** | **Not doing** (user decision 2026-09-28): originally planned as end-to-end encryption + self-hosted support; we have decided **not to offer cloud sync and not to have accounts** — your data stays on your machine |
| **Versioning** | Single source of truth: `[workspace.package] version` in the root `Cargo.toml`, shared by every workspace member (the list is that file's `members`); Android `versionName` and desktop `packageVersion` align to it, and releases are tagged with the same number |
| **MSRV** | **1.88**, written as `[workspace.package] rust-version` in the root `Cargo.toml` (where the floor came from and how it was measured is in that file's comment). ⚠️ That is a **declaration**, and on its own it leaks in both directions (code using newer syntax while the declaration lags / the declaration being lowered) — the enforcement point is CI's `msrv` job, which **derives** the toolchain from that key and runs `cargo check --workspace --all-targets --locked` |

### 🧭 Architecture

![OPI architecture: client, export, engine, data and build-pipeline layers](docs/diagrams/architecture.svg)

**Five layers, dependencies pointing only downwards.** The lower the layer, the more stable; the higher, the closer to the user:

- **Client layer** — native UI per platform. The clients share no UI code, only input semantics.
- **Export layer** — thin ABI shells doing type conversion, boundary validation and panic isolation. Each process holds one engine singleton: on Android the settings page and the IME share the same Rust singleton, so toggling learning in settings takes effect in the IME immediately.
- **Engine layer** — `engine-core`, **pure logic, no IO, no platform dependencies**. This is the heart of the project and the easiest layer to test: `Composer` state machine / `Pinyin` segmentation / `Trie` dictionary / `Candidates` ranking and merge / `Learner` / `Symbols`, with **the full module list taken from `src/lib.rs`'s `pub mod`**; above them a single `Engine` facade that the UI layer talks to. The same layer also holds the **platform-neutral key routing** (`router.rs` / `keys.rs`), used by Apple and HarmonyOS through the C ABI — it is a routing layer rather than an engine module, and its semantics follow the two existing tracks.
- **Data layer** — the `.opid` binary dictionary: fixed-size header + fixed-size entry table + two blobs + an FNV-1a64 trailer (take each section's length from the `HEADER_LEN` / `ENTRY_LEN` constants in `engine-data/src/format.rs` — don't copy numbers). Loaded via read-only `mmap` with the **whole dictionary kept out of the heap**; each matching entry is materialised into a `String` at query time (see `loader.rs`'s `MmapDictionary::query`), so it is not zero-copy.
- **Build pipeline** — TSV sources in `data/raw` are compiled by `opi-tools` into `.opid`, then pass checksum and ordering checks in `verify`, with full GB2312 single-character coverage guarded by the `trad_coverage` test, before being committed.

> **Key invariant: a broken dictionary must never crash the input method.** A bad path does **not** fall back silently (deliberate — see `engine-data/src/dictionary.rs`'s `load_or_fallback` comment); the caller catches it instead: Android retries with the built-in fallback dictionary, fcitx5 propagates the `Err`, and TSF falls back candidate by candidate inside its resolver to the built-in dictionary — only corruption of the built-in itself is unrecoverable.

### 🧩 Feature Design

![OPI feature design: five input modes and six feature domains](docs/diagrams/features.svg)

**Five input modes** are decided by the `Composer` state machine using a mode integer (the `Mode` enum's discriminant — **take the actual values from `enum Mode` in `composer.rs`** — and identical across the three language-crossing exports JNI / C ABI / fcitx5):

| Mode | Behaviour |
|---|---|
| **Pinyin** (default) | Lowercase letters and the `'` separator enter the buffer (buffer cap: `MAX_BUFFER` in `composer.rs`); queries the main dictionary |
| **Traditional** | Behaves exactly like Pinyin but routes to the `trad` dictionary; falls back to the simplified one if `trad` is missing |
| **English** | `⇧` taps lock uppercase (tap again to release), long-press for one-shot; the bottom `pro` key toggles normal/pro: **normal (default) commits letters directly with no word matching**, pro feeds the prefix dictionary (a single-letter buffer puts the typed letter first, case preserved); **leaving English for Chinese turns pro off automatically** (user ruling 2026-10-20); only a carried buffer commits directly; the English keyboard has a top 0-9 row (digits commit directly) |
| **Number** | Digits only; space / enter commits |
| **Symbol** | The buffer accepts nothing; the symbol panel commits directly. Pending pinyin is committed before the panel opens, leaving no residue |

Of the **six feature domains**, candidate ranking deserves a note. It has one counter-intuitive rule: **`limit` must not be pushed down into the dictionary query.** A learned low-frequency word can overtake a word outside the cut-off, so ranking must collect everything, sort globally, then deduplicate and truncate. The score is

```
score = static_freq + user_freq × boost
```

where `boost` **scales dynamically** with the dictionary's maximum frequency (`USER_BOOST.max(max_freq × 2)` in `engine.rs`; take the constant and the formula from the code) rather than being a hard-coded constant. When an early version used a hard-coded value only, selecting 我 once still lost to the static word 倭 at luna's million-scale frequencies. Dynamic scaling is what guarantees the promise "select once and it beats every static word".

### 🔄 Lifecycle

![OPI lifecycle: dictionary loading and the keystroke cycle](docs/diagrams/lifecycle.svg)

Two independent lifelines with a single intersection — **being installed into the process singleton**.

The **dictionary lifecycle** runs once at process start: locate (assets→filesDir on Android, the XDG data directory on fcitx5, and on Windows `OPI_DICT_PATH` → the DLL's own directory → `%LOCALAPPDATA%\opi` → the built-in fallback, see `tsf-opi/src/dict_path.rs`) → compare sizes to decide whether to re-copy (idempotent, guards against stale dictionaries) → `mmap` → verify → install into the singleton. Afterwards `install_trad` can hot-swap the traditional dictionary at any time without affecting simplified mode.

The **keystroke lifecycle** runs one lap per keypress in ten steps: ① key event → ② KeyRouter dispatch → ③ `Composer` state machine → ④ buffer update → ⑤ `rank_and_pick` ranking → ⑥ candidate bar renders → ⑦ user selects → ⑧ learning is recorded → ⑨ text is committed → ⑩ buffer is cleared. The crucial branch is ②: **with an empty buffer the key never enters the engine** (English and number modes commit directly). That is what keeps the keyboard responsive — the engine is only woken when there is genuinely something to compose.

After loading, every keystroke walks the read-only mapping and materialises only the entries it hits — the whole dictionary stays out of the heap, but it is **not** zero-copy.

### 🏗 Build & Test

```bash
cargo test --workspace                   # unit + integration + property tests (both the fcitx5 and TSF tracks)
cargo clippy --workspace --all-targets -- -D warnings   # gate: zero warnings
cd android && ./gradlew testDebugUnitTest   # Android unit tests (engine FFI + IME state machine + key routing + pet)
cd android && ./gradlew assembleDebug       # build debug APK (cargokit compiles opi-ffi three-ABI .so)
cargo build --release -p fcitx5_opi         # Linux plugin: the Rust cdylib on its own
cmake -S crates/fcitx5-opi/cpp -B build-fcitx5 -DCMAKE_BUILD_TYPE=Release   # full path (incl. the C++ glue; needs fcitx5-dev)
cmake --build build-fcitx5 && sudo cmake --install build-fcitx5             # the install location is guarded by the opi_locate_check probe and CI
scripts/build-packages.sh                   # build .deb / .rpm (needs nfpm; locations come from the cmake --install above, artifacts land in dist/)
cargo check --workspace --all-targets --locked   # the MSRV pass — run it on the toolchain declared in Cargo.toml (CI's msrv job runs this same line)
cd desktop && ./gradlew package             # Windows candidate window (Compose Desktop)
./target/debug/opi-tools --version          # version + Opi
```

> **`assembleDebug` needs `dart` on the machine**: cargokit's build tool is written in Dart. Without it the `cargokitCargoBuildOpi_ffiDebug` task fails with `dart: command not found` (exit code 127). The unit tests don't need it — `testDebugUnitTest` runs on pure JVM fakes and never touches the `.so`.

> **The fcitx5 plugin requires C++20**: `crates/fcitx5-opi/cpp/CMakeLists.txt` pins `CXX_STANDARD` to 20 —
> newer fcitx5 headers use the C++20 `std::span` / `std::ranges`, and pinning it back to 17 is a
> **hard error** on newer distributions (nothing to do with `-Werror`), while the headers of the two
> older versions compile warning-free and error-free under C++20. Where exactly the boundary sits and
> which versions were measured on each side are in that file's header comment; **do not change that
> value based on what this line says**.

> **The commands above get you to "it is built / packaged", not to "it is installed".** The install
> steps — plus **the one mandatory action after installing** (add OPI to the input method group;
> the fcitx5 reload is automatic) — are in [`docs/install/`](docs/install/README.md).

### 📁 Repository Structure

```
crates/                        # Rust workspace (the crate list is the root Cargo.toml's `members`)
  engine-core/                 # pure logic core: no IO, no platform dependencies
    src/                       #   module list = `pub mod` in src/lib.rs:
                               #   composer · pinyin · trie · dictionary · candidates ·
                               #   learner · symbols · engine, plus jianpin ·
                               #   fuzzy · punctuation · keys · router · bytes
    tests/                     #   three kinds: integration / property / invariant — e.g. engine_integration ·
                               #   proptests · ranking_invariants · router_invariants · trad_mode ·
                               #   symbol_coverage · jianpin_ranking · punctuation_switches ·
                               #   select_index_bounds (**a reading guide, not a census** — see the tests/ dir)
  engine-data/                 # .opid binary dictionary: format, FNV-1a64 checksum, mmap load, fallback
    src/user_words.rs          #   persisting user words (atomic write) — file IO lives in this crate, so engine-core stays IO-free
  opi-tools/                   # dictionary compiler CLI: tsv / dict.yaml → .opid, with verify
  opi-ffi/                     # dual ABI exports: JNI (Android) + C (Apple / HarmonyOS)
  fcitx5-opi/                  # Linux fcitx5 plugin: Rust logic (cdylib) + cpp/ AddonInstance glue
    cpp/CMakeLists.txt         #   the **one and only build + install path**; the location is guarded by the opi_locate_check probe and CI
  tsf-opi/                     # Windows TSF plugin: Rust logic + COM server + candidate-window protocol
    src/dict_path.rs           #   dictionary lookup order: env var → DLL directory → %LOCALAPPDATA% → built-in fallback
    src/settings.rs            #   settings reader (%LOCALAPPDATA%\opi\settings.ini; key names mirror the settings window; control-plane reload re-reads it)
    src/ctl.rs                 #   local control plane: state / action / search / commit / reload (\\.\pipe\opi-control)
android/                       # Android IME (Kotlin + Jetpack Compose)
  app/                         #   IME service · keyboard / candidate bar / panels · settings (incl. user-word import/export) · pet component
    src/main/assets/           #   luna.opid (simplified) · trad.opid (traditional) · stroke.opid (stroke) · rare_index.tsv (radical index)
    src/main/res/              #   adaptive launcher icon (VectorDrawable, featuring Opi)
  rust_builder/                #   standalone cargokit: compiles crates/opi-ffi → three-ABI .so
  jni_smoke/                   #   JNI connectivity smoke test
desktop/                       # Windows candidate window / status bar / settings window (Compose Desktop / JVM, named-pipe NDJSON; About page links the project and blog)
site/                          # static website (scroll-snap / responsive / three.js; see site/README.md)
ios/                           # iOS keyboard extension — CI-compiled; host app + unsigned IPA produced, not device-tested (see README inside)
macos/                         # macOS input method (InputMethodKit) — CI-compiled; symbol panel / input-method menu / settings window / candidate paging; unsigned pkg/app, not device-tested
harmony/                       # HarmonyOS input method (ArkTS + N-API) — complete hvigor project, unsigned HAP produced (contains libopiime.so)
                               #   dictionaries copied from rawfile to the sandbox; signing / installation / real-device IME list remain unverified; built by self-hosted harmony.yml
shared/                        # Kotlin sources shared across clients
  pet/OpiPet.kt                #   the Compose drawing of Opi (one copy, used by Android and desktop)
data/                          # dictionary data
  raw/                         #   TSV sources + LICENSES.md (per-item source, licence and upstream pin):
                               #   symbols.tsv · symbol_blocks.tsv · trad_hanzi.tsv ·
                               #   trad_phrases.tsv · en_words.tsv · fallback.tsv
  generated/                   #   build artifacts: fallback.opid · trad.opid are committed;
                               #   luna.opid is not (rebuilt locally; committed copy lives in android assets)
docs/                          # pet, diagrams and design docs
  install/                     #   per-platform install guides (android / linux / windows / ios / macos / harmony)
  opi-pet.svg                  #   the project pet, Opi
  opi-pet-forms.svg            #   twelve expressions (smile / joy / cry / dejected / terrified / angry / coquettish / cute / pleasing / knocking / face-palm / tongue-out)
  social-preview.svg · .en.svg #   bilingual social-preview image (each has a .png twin for GitHub Settings → Social preview)
  diagrams/                    #   architecture · features · lifecycle
  superpowers/                 #   specs (design) + plans (implementation)
  release-claim-corrections.md #   release-claim corrections (tag annotations cannot be edited, so the measured wording lands here)
  weixinpay.png · alipay.png   #   donation QR codes (referenced in the footer)
scripts/                       # dictionary/emoji generation: gen_luna_dict.py · gen_trad_dict.py · gen_trad_modern.py · gen_kangxi_extra.py · gen_symbols.py · gen_emoji_kaomoji.py · gen_pet_stickers.py · gen_emoji_keywords.py · gen_pet_emoji_table.py
                               #   (+ symbol_keywords.py, the hand-written keyword table) · gen_en_dict.py · hanzi_freq.py · prepare_harmony_dicts.sh (HarmonyOS rawfile dictionaries)
                               # distribution packaging: build-packages.sh (.deb / .rpm) + nfpm.yaml
                               #   (nfpm.yaml only moves the staging tree into a package — it **does not declare locations**)
.github/workflows/ci.yml       # CI: rust (fmt / cargo test / clippy / C consumer + JNI smoke /
                               #   TSF Windows target / Apple targets) · msrv (derives the toolchain from rust-version and really compiles) ·
                               #   android · fcitx5 (C++ build + install location + real load in a debian:12 container with three-arm controls)
                               #   ⚠️ the full list of jobs and steps is that file — don't copy this line
.github/workflows/packages.yml # builds .deb / .rpm — **manually triggered** (`gh workflow run packages.yml
                               #   --ref <tag-or-branch>`, or Actions → Packages → Run workflow). Artifacts land in that
                               #   run's artifact `opi-packages`; **attached to a release only if the tag input is filled in** — separate from ci.yml because the two paths fail differently
LICENSE · CONTRIBUTING.md      # MIT full text · contribution guide (incl. dictionary licensing)
```

### 📅 Project Status

> **Current stage: v1.21.0 (2026-10-22): Windows candidate bar follows the caret with pinyin above the candidates, Shift tap toggles Chinese/English, the candidate window follows the selected skin and font size; Settings gains Keys/Appearance pages plus a new-version checker; installer layout fixed (no extra `opi\opi` nesting); Apple unsigned, device verification pending**

V1 milestone progress:

- [x] **M1 Engine core**: cargo workspace + Composer key state machine + pinyin syllable table/segmentation + Trie dictionary + candidate ranking & merge + local learning + Unicode symbol engine + Engine facade
- [x] **M2 Data pipeline**: opi-tools compiles dictionaries → `.opid` binary (mmap loading, verification, corruption fallback)
- [x] **M3 FFI**: flutter_rust_bridge bindings + EngineController (superseded by the M6 opi-ffi dual ABI)
- [x] **M4/M5 Android integration & UI**: InputMethodService + keyboard/panels/settings (Flutter version, natively rewritten in M6)
- [x] **M6a Android native rewrite**: opi-ffi dual ABI (JNI + C) replaces frb; Compose native IME + keyboard/candidate bar/panels/settings; flutter/ deleted
- [x] **Simplified/Traditional dual dictionary** (no M6 number assigned in spec §8, listed separately): `Mode::Traditional` + dual-dictionary routing + `trad.opid` + a GB2312 single-character coverage gate
- [~] **M6b Linux fcitx5 plugin**: Rust logic and unit tests done; **the C++ glue had never been seen by a compiler**, and now has a CMake build + install path (`crates/fcitx5-opi/cpp/CMakeLists.txt`), a CI `fcitx5` job (compile + install-location assertions + a real load in a debian:12 container with three-arm controls) and `.deb` / `.rpm` packaging (`scripts/build-packages.sh`) — **awaiting acceptance on a real desktop; and no package has ever been `dpkg -i` / `rpm -i`'d**
- [~] **M6c Windows TSF plugin + CMP candidate window**: Rust logic, candidate-window wire protocol and the Compose Desktop candidate window / status bar / settings window done — the COM server is target-gated; **Windows packaging has been wired since 2026-10-18, and since 2026-10-20 the user-facing artifact is the EXE installer `opi-<version>-setup.exe`** (`luna.opid` now ships inside the app image under `app\` — the full Simplified dictionary works out of the box — with three licence texts and a CI SHA-256 assertion; white-screen fix: the launcher no longer restarts Explorer; uninstall kills `opi.exe`, deletes `{app}`, removes the Run entries and the `%TEMP%` lock file; the status bar autostarts at logon and drives TSF through the `\\.\pipe\opi-control` control plane, and settings `reload` immediately); **`trad.opid` still does not ship** (Windows holds a single dictionary), awaiting acceptance on real Windows hardware (Windows 10/11 x64 only; Win7/8/8.1 unsupported)
- [~] **M7 iOS / macOS**: the C ABI is ready; `apple.yml` now compiles the Swift, iOS produces a host app + unsigned IPA (toolbar / skins / keyboard modes / About page aligned with Android; sticker pasteboard writes require Full Access), and macOS produces an unsigned pkg/app (symbol panel / input-method menu / settings window / candidate paging); **signing, real-device runs and IME enablement remain unverified**.

> Milestone numbering follows `docs/superpowers/specs/2026-08-14-opi-multi-platform-design.md` §8 and the M6 plan
> (M6a=Android / M6b=fcitx5 / M6c=TSF+candidate window).

### 📄 License

- **Code**: MIT, full text in [`LICENSE`](LICENSE). Source file headers carry machine-readable **SPDX tags** (`SPDX-FileCopyrightText: 2026 erik.xyz` + `SPDX-License-Identifier: MIT`), and the Rust side additionally declares `[workspace.package] license = "MIT"` in `Cargo.toml`
- **Dictionary data**: **licensed separately from the code** — `data/raw/*.tsv` and the `.opid` files compiled from them are **not** covered by MIT and follow their upstream licenses; **`luna.opid` as a whole is distributed under GPL-3.0** because it now includes rime-ice (GPL-3.0-only, 2026-10-11; which also carries the obligation to make the upstream data available alongside the product), while the other sources are: rime-luna-pinyin / terra / stroke under LGPL-3.0, rime-pinyin-simp under Apache-2.0, chinese-xinhua and THUOCL under MIT, the symbol and Unihan-family data under the Unicode License, and the English word-association source under CC-BY 3.0, **which requires attribution** — the corresponding row in `LICENSES.md` is the attribution notice, and any product shipping that data must keep it; the OpenMoji emoji subset is CC BY-SA 4.0 (attribution: the OpenMoji project and contributors), the kaomoji data is MIT (Copyright (c) 2023 aoguai); the Opi stickers, app icons and documentation images are **project artwork, not open source** — proprietary, copyright the project author; commercial use requires written permission, while personal non-commercial use is allowed as part of an unmodified official release (see [`ASSETS-LICENSE.md`](ASSETS-LICENSE.md)); with per-item source, licence and upstream pin records in [`data/raw/LICENSES.md`](data/raw/LICENSES.md). Read the licensing section of [`CONTRIBUTING.md`](CONTRIBUTING.md) before submitting dictionary changes

---

### 💬 A Final Word

> *"I couldn't take it anymore, so I built one myself."*

---

### 🤝 Support Us

> If OPI helps you, scan the QR code below to support us (WeChat / Alipay — any amount is welcome, the thought is what counts).

| WeChat rewards | Alipay rewards |
|---|---|
| ![WeChat rewards QR code](docs/weixinpay.png) | ![Alipay rewards QR code](docs/alipay.png) |

<!-- assets-license:start -->
## Visual Assets License

The Opi pet, its stickers (`shared/pet/stickers/*.png` and `shared/pet/stickers-gif/*.gif`), the generated sticker tables
(`shared/pet/emoji-font/`), app icons and documentation artwork are **proprietary assets,
not open source**: commercial use requires written authorization; personal non-commercial
use of unmodified official releases is allowed. Code stays MIT; OpenMoji (CC BY-SA 4.0)
and kaomoji (MIT) keep their upstream licenses. See [ASSETS-LICENSE.md](ASSETS-LICENSE.md).
<!-- assets-license:end -->
