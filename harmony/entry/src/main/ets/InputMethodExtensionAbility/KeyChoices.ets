// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 9 / 14 键弹层的条目表（2026-10-25 从 `KeyPanels.ets` 拆出：那边加了
// 「全键大写/小写/数字」后越过 500 行门禁）。
//
// 条目次序统一为：**大写在前、小写随后、数字/符号末位**，默认选中末位。
// `direct=true` 的项由宿主直接原样上屏（大写/数字/符号）；小写项进引擎匹配词库。
// 9 键小写项的 `value` 是**选中的字母本身**（2026-10-26 订正）：引擎 Nine 已有
// 单字母类，选 a 只匹配 a；旧实现统一发该格 T9 数字，选哪个字母都匹配同一类。

import { PopupChoice } from './KeyPopup';

/** 14 键第三行四枚字母键上方的符号提示（发送键仍是对首字母）。 */
export function fourteenSymbol(pair: string): string {
  if (pair === 'zx') {
    return '~';
  }
  if (pair === 'cv') {
    return '!';
  }
  if (pair === 'bn') {
    return '@';
  }
  if (pair === 'm') {
    return '?';
  }
  return '';
}

/** 14 键备选：[大写对, 数字或符号, 小写对]；所有键都有，默认中间项。 */
export function fourteenChoices(pair: string, size: number, hint: string): PopupChoice[] {
  return [
    new PopupChoice(pair.toUpperCase(), pair.toUpperCase(), size, true),
    new PopupChoice(hint, hint, Math.max(12, size - 3), true),
    new PopupChoice(pair, pair.charAt(0), Math.max(12, size - 6)),
  ];
}

/** 9 键备选：大写在前、小写随后、数字末位；小写项的值是选中的字母本身。 */
export function nineChoices(
  hint: string, key: string, label: string, letterSize: number, digitSize: number
): PopupChoice[] {
  // 没有字母/数字/符号的格（分词键）不弹层（2026-10-25 用户裁决）。
  if (hint.length === 0) {
    return [];
  }
  const out: PopupChoice[] = [];
  for (let i = 0; i < hint.length; i++) {
    const ch: string = hint.charAt(i);
    out.push(new PopupChoice(ch.toUpperCase(), ch.toUpperCase(),
      Math.max(12, letterSize - i * 3), true));
  }
  for (let i = 0; i < hint.length; i++) {
    const ch: string = hint.charAt(i);
    out.push(new PopupChoice(ch, ch, Math.max(12, letterSize - (hint.length + i) * 3)));
  }
  out.push(new PopupChoice(label, key, digitSize, true));
  return out;
}
