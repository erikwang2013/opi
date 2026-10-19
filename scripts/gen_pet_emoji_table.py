#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""Generate the Opi pet emoji tables (labels, stable IDs, shortcodes).

Outputs (committed, under shared/pet/emoji-font/):
  map.tsv          mood, stable codepoint ID, sticker file, Chinese label, shortcode
  shortcodes.tsv   shortcode, sticker file, Chinese label

The `codepoint` column is a stable internal ID (BMP PUA U+E000..).  Nothing
inserts it into input fields: the apps use the GIF/PNG stickers or the text
label.  Sticker artwork is a **proprietary project asset**; the generator is
MIT.  See shared/pet/emoji-font/README.md and ASSETS-LICENSE.md.

Run:
  python3 scripts/gen_pet_emoji_table.py
Check that committed outputs match the sources (CI / local):
  python3 scripts/gen_pet_emoji_table.py --check

Sources of truth: PetEmojiMap.kt (labels and mood set), PetMood in OpiPet.kt
(ordering), shared/pet/stickers/*.png.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STICKERS = ROOT / "shared" / "pet" / "stickers"
PET_KT = ROOT / "shared" / "pet" / "OpiPet.kt"
MAP_KT = ROOT / "shared" / "pet" / "PetEmojiMap.kt"
IOS_TABLE = ROOT / "ios" / "PetEmojiTable.swift"
OUT_DIR = ROOT / "shared" / "pet" / "emoji-font"

PUA_BASE = 0xE000

# Shortcode names are stable ABI for chat clients: never rename an existing
# one, only append.  Mood order is PetMood order (see load_forms).
SHORTCODES = {
    "IDLE": "opi_smile",
    "WAITING": "opi_joy",
    "PUZZLED": "opi_sad",
    "CRYING": "opi_cry",
    "TERRIFIED": "opi_scream",
    "ANGRY": "opi_angry",
    "COQUETTISH": "opi_plead",
    "CUTE": "opi_wink",
    "PLEASING": "opi_pray",
    "KNOCKING": "opi_hammer",
    "FACE_PALM": "opi_facepalm",
    "TONGUE_OUT": "opi_tongue",
}


@dataclass(frozen=True)
class Form:
    mood: str
    label: str
    codepoint: int
    file: str
    shortcode: str


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        fail(f"missing source {path}")


def load_forms() -> list[Form]:
    enum_match = re.search(r"enum class PetMood\s*\{(.*?)\n\}", read_text(PET_KT), re.S)
    if not enum_match:
        fail(f"cannot find 'enum class PetMood' in {PET_KT}")
    moods = re.findall(r"^\s*([A-Z][A-Z0-9_]*)\s*,", enum_match.group(1), re.M)
    if not moods:
        fail(f"cannot parse PetMood entries in {PET_KT}")

    pairs = re.findall(
        r'PetMood\.([A-Z][A-Z0-9_]*)\s+to\s+\("([^"]+)"\s+to\s+"([^"]+)"\)',
        read_text(MAP_KT),
    )
    labels = {mood: label for mood, label, _emoji in pairs}
    if len(labels) != len(pairs):
        fail(f"duplicate PetMood entries in {MAP_KT}")
    if not set(labels) <= set(moods):
        fail(f"PetEmojiMap references moods missing from PetMood: {set(labels) - set(moods)}")

    forms: list[Form] = []
    for mood in moods:
        if mood not in labels:
            continue  # SLEEPY / DEGRADED are input states, not stickers
        if mood not in SHORTCODES:
            fail(f"no shortcode for new mood {mood}; add it to SHORTCODES")
        label = labels[mood]
        if "\t" in label or "\n" in label:
            fail(f"label for {mood} contains a tab/newline: {label!r}")
        forms.append(
            Form(
                mood=mood,
                label=label,
                codepoint=PUA_BASE + len(forms),
                file=mood.lower().replace("_", "-") + ".png",
                shortcode=f":{SHORTCODES[mood]}:",
            )
        )

    missing = [f.file for f in forms if not (STICKERS / f.file).is_file()]
    if missing:
        fail(f"missing sticker source(s) in {STICKERS}: {', '.join(missing)}")
    if not forms:
        fail("no sticker forms found")
    if forms[-1].codepoint > 0xF8FF:
        fail("too many forms for the BMP private-use area")
    if len({f.shortcode for f in forms}) != len(forms):
        fail("duplicate shortcode")
    return forms


def verify_ios_table(forms: list[Form]) -> None:
    """PetEmojiTable.swift 是手抄的第二份；--check 要比标签与文件名。"""
    entries = re.findall(
        r'Form\(label:\s*"([^"]+)",\s*sticker:\s*"([^"]+)"\)',
        read_text(IOS_TABLE),
    )
    expected = [(form.label, form.file.removesuffix(".png")) for form in forms]
    if entries != expected:
        fail(f"{IOS_TABLE.relative_to(ROOT)} is stale: expected {expected}, found {entries}")


def build_outputs() -> dict[str, bytes]:
    forms = load_forms()

    map_lines = ["mood\tcodepoint\tfile\tlabel_zh\tshortcode"]
    map_lines += [
        f"{form.mood}\tU+{form.codepoint:04X}\t{form.file}\t{form.label}\t{form.shortcode}"
        for form in forms
    ]
    short_lines = ["shortcode\tfile\tlabel_zh"]
    short_lines += [
        f"{form.shortcode}\t{form.file}\t{form.label}" for form in forms
    ]
    return {
        "map.tsv": ("\n".join(map_lines) + "\n").encode("utf-8"),
        "shortcodes.tsv": ("\n".join(short_lines) + "\n").encode("utf-8"),
    }


def main() -> None:
    outputs = build_outputs()
    check_only = "--check" in sys.argv[1:]
    if check_only:
        stale = []
        for name, data in outputs.items():
            path = OUT_DIR / name
            if not path.is_file() or path.read_bytes() != data:
                stale.append(str(path.relative_to(ROOT)))
        if stale:
            fail(
                "committed output is missing or stale: "
                + ", ".join(stale)
                + f" (run python3 {Path(__file__).relative_to(ROOT)})"
            )
        verify_ios_table(load_forms())
        print("OK: pet emoji tables and the iOS hand-copy match the sources")
        return

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        (OUT_DIR / name).write_bytes(data)
        print(f"wrote {OUT_DIR.relative_to(ROOT) / name} ({len(data)} bytes)")


if __name__ == "__main__":
    main()
