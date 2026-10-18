#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""Generate the Opi pet color emoji font and its shortcode tables.

Outputs (committed, under shared/pet/emoji-font/):
  opi-emoji.ttf    color OpenType font; 12 PUA codepoints U+E000..U+E00B
  map.tsv          mood, codepoint, sticker file, Chinese label, shortcode
  shortcodes.tsv   shortcode, sticker file, Chinese label

The sticker artwork and the generated font are **proprietary project assets**
(the generator itself is MIT, like the rest of the source).  See
shared/pet/emoji-font/README.md and ASSETS-LICENSE.md.

Font format: TrueType with two color representations for the same glyphs:
  - SVG  : one embedded 280x280 PNG per glyph (data URI) -- OT-SVG renderers
  - CBDT/CBLC: one 120 ppem PNG strike -- FreeType, Chrome, Android
No font outlines are generated (empty glyf); renderers without a color table
show a blank advance, which is normal for color-only emoji fonts.

Dependencies (not installed automatically):
  fontTools 4.63.0  (font construction + verification)
  Pillow    12.2.0  (downscale stickers to the 120 ppem bitmap strike)

Run:
  python3 scripts/gen_pet_emoji_font.py
Check that committed outputs match the sources (CI / local):
  python3 scripts/gen_pet_emoji_font.py --check

Output is byte-stable: fixed head timestamps, deterministic PNG settings,
deterministic table order.  Sources of truth: PetEmojiMap.kt (labels and
mood set), PetMood in OpiPet.kt (ordering), shared/pet/stickers/*.png.
"""

from __future__ import annotations

import base64
import io
import re
import struct
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.DefaultTable import DefaultTable
from fontTools.ttLib.tables.S_V_G_ import SVGDocument
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
STICKERS = ROOT / "shared" / "pet" / "stickers"
PET_KT = ROOT / "shared" / "pet" / "OpiPet.kt"
MAP_KT = ROOT / "shared" / "pet" / "PetEmojiMap.kt"
OUT_DIR = ROOT / "shared" / "pet" / "emoji-font"

FONT_FILE = "opi-emoji.ttf"
FAMILY = "Opi Emoji"
PS_NAME = "OpiEmoji-Regular"
UNITS_PER_EM = 1000
STRIKE = 120  # CBDT ppem; bearingY must fit int8, so <= 127
PUA_BASE = 0xE000
# 2026-10-18 00:00:00 UTC in the 1904 epoch used by head.created/modified.
FIXED_TIMESTAMP = 3875126400

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

    @property
    def glyph(self) -> str:
        return "opi_" + self.file.removesuffix(".png").replace("-", "_")


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


def strike_png(path: Path) -> bytes:
    """Deterministic 120x120 RGBA PNG for the CBDT strike."""
    image = Image.open(path).convert("RGBA")
    image.load()
    image = image.resize((STRIKE, STRIKE), Image.Resampling.LANCZOS)
    image.info.clear()
    buffer = io.BytesIO()
    image.save(buffer, format="PNG", optimize=False, compress_level=9)
    return buffer.getvalue()


def svg_document(png: bytes) -> str:
    # OT-SVG uses y-down coordinates with the baseline at y=0, so the image
    # occupies y=-1000..0.  width/height are in unitsPerEm per the spec.
    payload = base64.b64encode(png).decode("ascii")
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" '
        'xmlns:xlink="http://www.w3.org/1999/xlink" '
        'width="1000" height="1000" viewBox="0 -1000 1000 1000">'
        f'<image x="0" y="-1000" width="1000" height="1000" '
        f'xlink:href="data:image/png;base64,{payload}"/>'
        "</svg>"
    )


class RawTable(DefaultTable):
    """A table whose compile() returns already-packed bytes."""

    def __init__(self, tag: str, data: bytes):
        super().__init__(tag)
        self.data = data

    def compile(self, ttFont) -> bytes:
        return self.data

    def decompile(self, data: bytes, ttFont) -> None:
        self.data = data


def bitmap_tables(pngs: list[bytes]) -> tuple[bytes, bytes]:
    """Build CBLC (locations) and CBDT (version 3, image format 17) tables."""
    count = len(pngs)
    images = [
        struct.pack(">BBbbB", STRIKE, STRIKE, 0, STRIKE, STRIKE)
        + struct.pack(">L", len(png))
        + png
        for png in pngs
    ]
    offsets = [0]
    for image in images:
        offsets.append(offsets[-1] + len(image))

    def line_metrics() -> bytes:
        # ascender, descender, widthMax then 9 zero bytes (SbitLineMetrics).
        return struct.pack(">bbb" + "B" * 9, STRIKE, 0, STRIKE, *([0] * 9))

    # One index subtable covering all glyphs; offsets are relative to the
    # start of CBDT and point at each format-17 image record.
    subtable = (
        struct.pack(">HHI", 1, 17, 4)
        + b"".join(struct.pack(">L", offset) for offset in offsets)
    )
    array_offset = 8 + 48  # CBLC header + one BitmapSize record
    bitmap_size = (
        struct.pack(">IIII", array_offset, 8 + len(subtable), 1, 0)
        + line_metrics()
        + line_metrics()
        + struct.pack(">HHBBBb", 1, count, STRIKE, STRIKE, 32, 1)
    )
    cblc = (
        struct.pack(">II", 0x00030000, 1)
        + bitmap_size
        + struct.pack(">HHI", 1, count, 8)  # subtable follows the 8-byte record
        + subtable
    )
    cbdt = struct.pack(">I", 0x00030000) + b"".join(images)
    return cblc, cbdt


def build_font(forms: list[Form], svg_pngs: list[bytes], pngs: list[bytes]) -> bytes:
    glyph_order = [".notdef"] + [form.glyph for form in forms]
    builder = FontBuilder(UNITS_PER_EM, isTTF=True)
    builder.setupGlyphOrder(glyph_order)
    builder.setupCharacterMap({form.codepoint: form.glyph for form in forms})
    builder.setupGlyf({name: TTGlyphPen(None).glyph() for name in glyph_order})
    builder.setupHorizontalMetrics({name: (UNITS_PER_EM, 0) for name in glyph_order})
    builder.setupHorizontalHeader(
        ascent=UNITS_PER_EM, descent=0, lineGap=0
    )
    builder.setupNameTable(
        {
            "familyName": FAMILY,
            "styleName": "Regular",
            "uniqueFontIdentifier": f"{FAMILY} Regular 1.0; 2026 erik.xyz",
            "fullName": f"{FAMILY} Regular",
            "psName": PS_NAME,
            "version": "Version 1.0",
            "copyright": "Copyright 2026 erik.xyz. Proprietary project asset.",
            "description": "Opi pet stickers as a color emoji font.",
            "licenseDescription": (
                "Proprietary asset of the Opi project. See "
                "shared/pet/emoji-font/README.md and ASSETS-LICENSE.md."
            ),
        }
    )
    builder.setupOS2(
        sTypoAscender=UNITS_PER_EM,
        sTypoDescender=0,
        sTypoLineGap=0,
        usWinAscent=UNITS_PER_EM,
        usWinDescent=0,
        sxHeight=500,
        sCapHeight=700,
        achVendID="OPIX",
        fsType=0,
        fsSelection=0x40,
        usFirstCharIndex=forms[0].codepoint,
        usLastCharIndex=forms[-1].codepoint,
    )
    builder.setupPost(keepGlyphNames=False)

    svg = newTable("SVG ")
    svg.docList = [
        SVGDocument(svg_document(png), index + 1, index + 1)
        for index, png in enumerate(svg_pngs)
    ]
    builder.font["SVG "] = svg
    cblc, cbdt = bitmap_tables(pngs)
    builder.font["CBLC"] = RawTable("CBLC", cblc)
    builder.font["CBDT"] = RawTable("CBDT", cbdt)

    # Byte stability: never let fontTools stamp the current time.
    builder.font["head"].created = FIXED_TIMESTAMP
    builder.font["head"].modified = FIXED_TIMESTAMP
    builder.font["head"].fontRevision = 1.0

    buffer = io.BytesIO()
    builder.font.save(buffer)
    return buffer.getvalue()


def verify(font_bytes: bytes, forms: list[Form], svg_pngs: list[bytes], pngs: list[bytes]) -> None:
    font = TTFont(io.BytesIO(font_bytes), lazy=False)
    cmap = font.getBestCmap()
    # post 3.0 drops glyph names, so the reloaded font synthesizes uniXXXX
    # names; verify codepoints resolve to the expected glyph IDs instead.
    if sorted(cmap) != [form.codepoint for form in forms]:
        fail(f"cmap mismatch: {sorted(cmap)!r}")
    if len(font.getGlyphOrder()) != len(forms) + 1:
        fail(f"unexpected glyph count: {len(font.getGlyphOrder())}")
    for index, form in enumerate(forms):
        if font.getGlyphID(cmap[form.codepoint]) != index + 1:
            fail(f"cmap entry for {form.mood} maps to the wrong glyph ID")
    if font["head"].unitsPerEm != UNITS_PER_EM:
        fail("unexpected unitsPerEm")
    if (font["head"].created, font["head"].modified) != (FIXED_TIMESTAMP, FIXED_TIMESTAMP):
        fail("head timestamps are not fixed")

    # SVG table: one non-empty, well-formed document per glyph, with the
    # exact source PNG embedded.
    if "SVG " not in font:
        fail("SVG table missing")
    docs = {doc.startGlyphID: doc for doc in font["SVG "].docList}
    if len(docs) != len(forms):
        fail(f"SVG has {len(docs)} documents, expected {len(forms)}")
    for index, (form, source) in enumerate(zip(forms, svg_pngs)):
        gid = index + 1
        doc = docs.get(gid)
        if doc is None or doc.startGlyphID != doc.endGlyphID:
            fail(f"SVG document missing for {form.mood}")
        root = ET.fromstring(doc.data)
        image = root.find("{http://www.w3.org/2000/svg}image")
        if image is None:
            fail(f"SVG document for {form.mood} has no image")
        href = image.get("{http://www.w3.org/1999/xlink}href") or image.get("href")
        prefix = "data:image/png;base64,"
        if not href or not href.startswith(prefix):
            fail(f"SVG document for {form.mood} is not a PNG data URI")
        if base64.b64decode(href[len(prefix) :]) != source:
            fail(f"SVG PNG payload differs from source for {form.mood}")

    # CBDT/CBLC: one 120 ppem strike, image data equals the scaled PNG.
    if "CBLC" not in font or "CBDT" not in font:
        fail("CBDT/CBLC tables missing")
    if len(font["CBLC"].strikes) != 1:
        fail("expected exactly one bitmap strike")
    strike = font["CBLC"].strikes[0].bitmapSizeTable
    if (strike.ppemX, strike.ppemY, strike.bitDepth, strike.flags) != (STRIKE, STRIKE, 32, 1):
        fail("unexpected bitmap strike parameters")
    bitmaps = font["CBDT"].strikeData[0]
    for form, source in zip(forms, pngs):
        bitmap = bitmaps[cmap[form.codepoint]]
        if bitmap.imageData != source:
            fail(f"CBDT image differs from scaled source for {form.mood}")
        metrics = bitmap.metrics
        if (
            metrics.height,
            metrics.width,
            metrics.BearingX,
            metrics.BearingY,
            metrics.Advance,
        ) != (STRIKE, STRIKE, 0, STRIKE, STRIKE):
            fail(f"CBDT metrics wrong for {form.mood}")


def build_outputs() -> dict[str, bytes]:
    forms = load_forms()
    svg_pngs = [(STICKERS / form.file).read_bytes() for form in forms]
    pngs = [strike_png(STICKERS / form.file) for form in forms]
    font_bytes = build_font(forms, svg_pngs, pngs)
    verify(font_bytes, forms, svg_pngs, pngs)

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
        FONT_FILE: font_bytes,
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
                stale.append(name)
        if stale:
            fail(
                "committed output is missing or stale: "
                + ", ".join(stale)
                + f" (run python3 {Path(__file__).relative_to(ROOT)})"
            )
        print(f"OK: {len(outputs[FONT_FILE])} byte font and tables match sources")
        return

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        (OUT_DIR / name).write_bytes(data)
        print(f"wrote {OUT_DIR.relative_to(ROOT) / name} ({len(data)} bytes)")


if __name__ == "__main__":
    main()
