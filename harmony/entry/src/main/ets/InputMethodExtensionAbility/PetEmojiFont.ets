// SPDX-FileCopyrightText: 2026 erik.xyz
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

import { font } from '@kit.ArkUI';

export interface PetFontGlyph {
  mood: string;
  codepoint: number;
  file: string;
  label: string;
  shortcode: string;
}

const MAP_TSV: string = `mood	codepoint	file	label_zh	shortcode
IDLE	U+E000	idle.png	小欧·笑	:opi_smile:
WAITING	U+E001	waiting.png	小欧·欢乐	:opi_joy:
PUZZLED	U+E002	puzzled.png	小欧·沮丧	:opi_sad:
CRYING	U+E003	crying.png	小欧·哭	:opi_cry:
TERRIFIED	U+E004	terrified.png	小欧·惊恐	:opi_scream:
ANGRY	U+E005	angry.png	小欧·愤怒	:opi_angry:
COQUETTISH	U+E006	coquettish.png	小欧·撒娇	:opi_plead:
CUTE	U+E007	cute.png	小欧·卖萌	:opi_wink:
PLEASING	U+E008	pleasing.png	小欧·讨好	:opi_pray:
KNOCKING	U+E009	knocking.png	小欧·敲打	:opi_hammer:
FACE_PALM	U+E00A	face-palm.png	小欧·捂脸	:opi_facepalm:
TONGUE_OUT	U+E00B	tongue-out.png	小欧·吐舌头	:opi_tongue:`;
const SHORTCODES_TSV: string = `shortcode	file	label_zh
:opi_smile:	idle.png	小欧·笑
:opi_joy:	waiting.png	小欧·欢乐
:opi_sad:	puzzled.png	小欧·沮丧
:opi_cry:	crying.png	小欧·哭
:opi_scream:	terrified.png	小欧·惊恐
:opi_angry:	angry.png	小欧·愤怒
:opi_plead:	coquettish.png	小欧·撒娇
:opi_wink:	cute.png	小欧·卖萌
:opi_pray:	pleasing.png	小欧·讨好
:opi_hammer:	knocking.png	小欧·敲打
:opi_facepalm:	face-palm.png	小欧·捂脸
:opi_tongue:	tongue-out.png	小欧·吐舌头`;

/** map.tsv → 12 条（表头/列数不对返回空数组）。 */
export function petFontGlyphs(): PetFontGlyph[] {
  const rows: string[][] = MAP_TSV.split('\n')
    .map((line: string) => line.split('\t'));
  if (rows.length === 0 ||
      rows[0].join('\t') !== 'mood\tcodepoint\tfile\tlabel_zh\tshortcode') {
    return [];
  }
  const out: PetFontGlyph[] = [];
  for (let i = 1; i < rows.length; i++) {
    const cells = rows[i];
    if (cells.length !== 5 || !cells[1].startsWith('U+')) continue;
    out.push({
      mood: cells[0],
      codepoint: parseInt(cells[1].substring(2), 16),
      file: cells[2],
      label: cells[3],
      shortcode: cells[4],
    });
  }
  return out;
}

/** shortcodes.tsv → 短码表（短码 → 文件名）。 */
export function petFontShortcodes(): Record<string, string> {
  const rows: string[][] = SHORTCODES_TSV.split('\n')
    .map((line: string) => line.split('\t'));
  const table: Record<string, string> = {};
  for (let i = 1; i < rows.length; i++) {
    if (rows[i].length === 3) table[rows[i][0]] = rows[i][1];
  }
  return table;
}

/** 注册 "Opi Emoji"（幂等：重复注册由框架忽略/返回失败，不影响调用方）。 */
export function registerPetEmojiFont(): boolean {
  try {
    font.registerFont({ familyName: 'Opi Emoji', familySrc: $rawfile('pet-font/opi-emoji.ttf') });
    return true;
  } catch (e) {
    console.error(`[opi] 字体注册失败：${e}`);
    return false;
  }
}
