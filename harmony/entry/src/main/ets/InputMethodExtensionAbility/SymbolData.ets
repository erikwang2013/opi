// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 符号页类目与生僻字索引的解析/缓存（Android `SymbolCategories.kt` + `RareIndex.kt` 的镜像）。
//
// 两张 TSV 都由 scripts/prepare_harmony_dicts.sh 从 Android assets 复制进 rawfile：
//   symbol_categories.tsv  序号<TAB>名称<TAB>符号 符号 …
//   rare_index.tsv         字<TAB>部首号(1..214)<TAB>总笔画数
// 解析是纯函数、坏行跳过不抛（资产可能被替换/截断，页面该降级而不是崩 IME）。
// 首次打开面板时才读：rare_index.tsv 1.1MB，别放服务启动路径。

import { readRawText } from './RawAssets';

export class SymbolCategory {
  index: number;
  label: string;
  symbols: string[];

  constructor(index: number, label: string, symbols: string[]) {
    this.index = index;
    this.label = label;
    this.symbols = symbols;
  }
}

let categories: SymbolCategory[] | null = null;
let categoriesTried: boolean = false;

/** 符号类目（按 TSV 顺序；空 = 资产缺失/全坏）。 */
export function symbolCategories(): SymbolCategory[] {
  if (categories !== null) {
    return categories;
  }
  if (categoriesTried) {
    return [];
  }
  categoriesTried = true;
  const text: string = readRawText('symbol_categories.tsv');
  const out: SymbolCategory[] = [];
  const lines: string[] = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line: string = lines[i].trim();
    if (line.length === 0) {
      continue;
    }
    const parts: string[] = line.split('\t');
    if (parts.length < 3) {
      continue;
    }
    const idx: number = Number(parts[0]);
    if (!Number.isFinite(idx) || Math.floor(idx) !== idx || idx <= 0) {
      continue;
    }
    const label: string = parts[1].trim();
    if (label.length === 0) {
      continue;
    }
    const rawSymbols: string[] = parts[2].split(' ');
    const symbols: string[] = [];
    for (let s = 0; s < rawSymbols.length; s++) {
      if (rawSymbols[s].length > 0) {
        symbols.push(rawSymbols[s]);
      }
    }
    if (symbols.length === 0) {
      continue;
    }
    out.push(new SymbolCategory(idx, label, symbols));
  }
  if (out.length > 0) {
    categories = out;
  }
  return out;
}

/** 部首号 → 康熙部首字形（U+2F00..U+2FD5 与 1..214 一一对应）。 */
export function radicalGlyph(radical: number): string {
  return String.fromCharCode(0x2F00 + Math.max(0, Math.min(radical, 214)) - 1);
}

let rare: Map<number, Map<number, string[]>> | null = null;
let rareTried: boolean = false;

/** 生僻字索引：部首 → 笔画数 → 字表。空 = 资产缺失/全坏。 */
export function rareIndex(): Map<number, Map<number, string[]>> {
  if (rare !== null) {
    return rare;
  }
  if (rareTried) {
    return new Map<number, Map<number, string[]>>();
  }
  rareTried = true;
  const text: string = readRawText('rare_index.tsv');
  const out: Map<number, Map<number, string[]>> = new Map<number, Map<number, string[]>>();
  const lines: string[] = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line: string = lines[i].trim();
    if (line.length === 0) {
      continue;
    }
    const parts: string[] = line.split('\t');
    if (parts.length !== 3) {
      continue;
    }
    // 一个字符 = 1 个 UTF-16 单元（基本区）或 2 个（扩展 B 起的代理对）。
    if (parts[0].length < 1 || parts[0].length > 2) {
      continue;
    }
    const radical: number = Number(parts[1]);
    const strokes: number = Number(parts[2]);
    if (!Number.isInteger(radical) || !Number.isInteger(strokes)) {
      continue;
    }
    if (radical < 1 || radical > 214 || strokes <= 0) {
      continue;
    }
    let byStroke: Map<number, string[]> | undefined = out.get(radical);
    if (byStroke === undefined) {
      byStroke = new Map<number, string[]>();
      out.set(radical, byStroke);
    }
    let chars: string[] | undefined = byStroke.get(strokes);
    if (chars === undefined) {
      chars = [];
      byStroke.set(strokes, chars);
    }
    chars.push(parts[0]);
  }
  if (out.size > 0) {
    rare = out;
  }
  return out;
}

/** 某部首下可选笔画数（升序）。 */
export function rareStrokeCounts(radical: number): number[] {
  const byStroke: Map<number, string[]> | undefined = rareIndex().get(radical);
  const out: number[] = [];
  if (byStroke === undefined) {
    return out;
  }
  // 生僻字总笔画上限按 80 扫（Unicode 最大约 64 画），省掉迭代器 API 的不确定性。
  for (let s = 1; s <= 80; s++) {
    if (byStroke.has(s)) {
      out.push(s);
    }
  }
  return out;
}

/** 某部首 + 笔画数下的字（表序 = 生成序）。 */
export function rareChars(radical: number, strokes: number): string[] {
  const byStroke: Map<number, string[]> | undefined = rareIndex().get(radical);
  if (byStroke === undefined) {
    return [];
  }
  const chars: string[] | undefined = byStroke.get(strokes);
  return chars === undefined ? [] : chars;
}
