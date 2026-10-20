// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 表情数据缓存与搜索（从 KeyboardController 搬出：那边撞 500 行门禁）。
// 纪律与 Android SymbolCatalog 相同：**空结果不写缓存**（空 = 没拿到，不是真的没有）。

import { EmojiGroup } from './OpiEngine';
import * as OpiEngine from './OpiEngine';

let emojiCache: string[] | null = null;
let emojiSetCache: Set<string> | null = null;
let emojiGroupsCache: EmojiGroup[] | null = null;

/** 表情全量（官方分组序），懒取一次。 */
export function emojiSymbols(): string[] {
  if (emojiCache === null) {
    const list: string[] = OpiEngine.emojiSymbols();
    if (list.length > 0) {
      emojiCache = list;
    }
    return list;
  }
  return emojiCache;
}

/** 表情分组（分类条）。 */
export function emojiGroups(): EmojiGroup[] {
  if (emojiGroupsCache === null) {
    const groups: EmojiGroup[] = OpiEngine.emojiGroups();
    if (groups.length > 0) {
      emojiGroupsCache = groups;
    }
    return groups;
  }
  return emojiGroupsCache;
}

/** 表情页搜索：引擎搜索结果 ∩ 表情集（同 Android `filter(catalog::isEmoji)`）。 */
export function searchEmoji(q: string): string[] {
  const all: string[] = OpiEngine.searchSymbols(q);
  if (all.length === 0) {
    return [];
  }
  if (emojiSetCache === null) {
    const set: Set<string> = new Set<string>();
    const emojis: string[] = emojiSymbols();
    for (let i = 0; i < emojis.length; i++) {
      set.add(emojis[i]);
    }
    if (set.size > 0) {
      emojiSetCache = set;
    }
  }
  if (emojiSetCache === null) {
    return [];
  }
  const out: string[] = [];
  for (let i = 0; i < all.length; i++) {
    if (emojiSetCache.has(all[i])) {
      out.push(all[i]);
    }
  }
  return out;
}
