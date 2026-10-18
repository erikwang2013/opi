#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""iOS / 鸿蒙镜像产物：`gen_pet_emoji_font.py` 的平台输出半。

从主生成器拆出（500 行源码门禁）：这里只做 base64/模板，字体构建与校验仍在
`gen_pet_emoji_font.py`。单独运行没有意义。
"""

import base64
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IOS_MIRROR = ROOT / "ios" / "PetEmojiFont.swift"
HARMONY_MIRROR = ROOT / "harmony" / "ets" / "InputMethodExtensionAbility" / "PetEmojiFont.ets"
HARMONY_RAWFILE = ROOT / "harmony" / "resources" / "base" / "rawfile" / "pet-font"

FONT_FILE = "opi-emoji.ttf"  # 与主生成器同名（只用于拼镜像路径）


def platform_outputs(outputs: dict[str, bytes]) -> dict[Path, bytes]:
    """iOS / 鸿蒙的镜像交付物。

    iOS 没有 Xcode 工程可挂资源：字体与两份 TSV 以常量内嵌，运行时用
    `CTFontManagerRegisterGraphicsFont` 注册（CoreText 对 SVG/CBDT 的支持存疑，
    见 README）。鸿蒙按标准位置放 rawfile，并用生成的 ArkTS helper 注册/解析；
    harmony/ 目前只是一个片段工程，宿主仍需把 resources 放进 entry 模块。
    """
    map_text = outputs["map.tsv"].decode("utf-8").rstrip("\n")
    short_text = outputs["shortcodes.tsv"].decode("utf-8").rstrip("\n")
    ttf_b64 = base64.b64encode(outputs[FONT_FILE]).decode("ascii")

    swift = f'''// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// **生成文件 —— 别手改。** 源：shared/pet/emoji-font/
// 重新生成：python3 scripts/gen_pet_emoji_font.py
//
// 字体与两份 TSV 以常量内嵌（本仓没有 Xcode 工程，资源没有落点）。运行时用
// `CTFontManagerRegisterGraphicsFont` 注册 family "Opi Emoji"。
//
// ⚠️ 当前 TTF 只有 SVG + CBDT/CBLC 两张色表；CoreText 常见支持的是 sbix /
// COLR，这两个表**很可能不渲染**（注册可能成功但字形空白）。真机上先跑
// `PetEmojiFont.register()` 再看 PUA 字形；面板贴纸不依赖它（走 GIF/PNG）。

import CoreText
import Foundation

enum PetEmojiFont {{

    static let familyName = "Opi Emoji"
    static let fontFileName = "opi-emoji.ttf"

    /// map.tsv 的全部内容（表头 + 12 行，制表符分隔）。
    static let mapTSV = """
{map_text}
"""

    /// shortcodes.tsv 的全部内容（表头 + 12 行）。
    static let shortcodesTSV = """
{short_text}
"""

    /// opi-emoji.ttf 的 base64。
    static let ttfBase64 = "{ttf_b64}"

    struct Glyph: Equatable {{
        let mood: String
        let codepoint: UInt32
        let file: String
        let label: String
        let shortcode: String
    }}

    /// map.tsv → 12 条；表头/列数不对返回空数组（内嵌数据坏了要能被发现）。
    static func glyphs() -> [Glyph] {{
        let rows = mapTSV.split(separator: "\\n").map {{
            $0.split(separator: "\\t", omittingEmptySubsequences: false).map(String.init)
        }}
        guard rows.first == ["mood", "codepoint", "file", "label_zh", "shortcode"] else {{ return [] }}
        return rows.dropFirst().compactMap {{ cells in
            guard cells.count == 5, cells[1].hasPrefix("U+"),
                  let code = UInt32(cells[1].dropFirst(2), radix: 16) else {{ return nil }}
            return Glyph(
                mood: cells[0], codepoint: code, file: cells[2],
                label: cells[3], shortcode: cells[4]
            )
        }}
    }}

    /// shortcodes.tsv → 短码表（短码 → 文件名）。
    static func shortcodes() -> [String: String] {{
        let rows = shortcodesTSV.split(separator: "\\n").map {{
            $0.split(separator: "\\t", omittingEmptySubsequences: false).map(String.init)
        }}
        guard rows.first == ["shortcode", "file", "label_zh"] else {{ return [:] }}
        var table: [String: String] = [:]
        for cells in rows.dropFirst() where cells.count == 3 {{
            table[cells[0]] = cells[1]
        }}
        return table
    }}

    /// 把内嵌 TTF 注册进进程字体表；重复调用由 CoreText 返回 false（不算错误）。
    @discardableResult
    static func register() -> Bool {{
        guard let data = Data(base64Encoded: ttfBase64),
              let provider = CGDataProvider(data: data as CFData),
              let font = CGFont(provider) else {{ return false }}
        var error: Unmanaged<CFError>?
        let ok = CTFontManagerRegisterGraphicsFont(font, &error)
        error?.release()
        return ok
    }}
}}
'''

    harmony = f'''// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// **生成文件 —— 别手改。** 源：shared/pet/emoji-font/
// 重新生成：python3 scripts/gen_pet_emoji_font.py
//
// 字体按标准位置放 resources/base/rawfile/pet-font/（opi-emoji.ttf + 两份 TSV），
// 本文件内嵌同一份 TSV 供纯解析；`registerPetEmojiFont()` 负责注册 family。
//
// ⚠️ 草案 · 从未编译（本机没有可构建的鸿蒙工程）。`$rawfile` 在普通函数里能否
// 使用、`font.registerFont` 是否接受该描述符，均未上机验证。字体只有 SVG +
// CBDT/CBLC 色表，ArkUI 能否渲染未验证；面板贴纸不依赖它（走 GIF/PNG）。

import {{ font }} from '@kit.ArkUI';

export interface PetFontGlyph {{
  mood: string;
  codepoint: number;
  file: string;
  label: string;
  shortcode: string;
}}

const MAP_TSV: string = `{map_text}`;
const SHORTCODES_TSV: string = `{short_text}`;

/** map.tsv → 12 条（表头/列数不对返回空数组）。 */
export function petFontGlyphs(): PetFontGlyph[] {{
  const rows: string[][] = MAP_TSV.split('\\n')
    .map((line: string) => line.split('\\t'));
  if (rows.length === 0 ||
      rows[0].join('\\t') !== 'mood\\tcodepoint\\tfile\\tlabel_zh\\tshortcode') {{
    return [];
  }}
  const out: PetFontGlyph[] = [];
  for (let i = 1; i < rows.length; i++) {{
    const cells = rows[i];
    if (cells.length !== 5 || !cells[1].startsWith('U+')) continue;
    out.push({{
      mood: cells[0],
      codepoint: parseInt(cells[1].substring(2), 16),
      file: cells[2],
      label: cells[3],
      shortcode: cells[4],
    }});
  }}
  return out;
}}

/** shortcodes.tsv → 短码表（短码 → 文件名）。 */
export function petFontShortcodes(): Record<string, string> {{
  const rows: string[][] = SHORTCODES_TSV.split('\\n')
    .map((line: string) => line.split('\\t'));
  const table: Record<string, string> = {{}};
  for (let i = 1; i < rows.length; i++) {{
    if (rows[i].length === 3) table[rows[i][0]] = rows[i][1];
  }}
  return table;
}}

/** 注册 "Opi Emoji"（幂等：重复注册由框架忽略/返回失败，不影响调用方）。 */
export function registerPetEmojiFont(): boolean {{
  try {{
    font.registerFont({{ familyName: 'Opi Emoji', familySrc: $rawfile('pet-font/opi-emoji.ttf') }});
    return true;
  }} catch (e) {{
    console.error(`[opi] 字体注册失败：${{e}}`);
    return false;
  }}
}}
'''

    return {
        IOS_MIRROR: swift.encode("utf-8"),
        HARMONY_MIRROR: harmony.encode("utf-8"),
        HARMONY_RAWFILE / FONT_FILE: outputs[FONT_FILE],
        HARMONY_RAWFILE / "map.tsv": outputs["map.tsv"],
        HARMONY_RAWFILE / "shortcodes.tsv": outputs["shortcodes.tsv"],
    }
