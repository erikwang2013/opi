// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// IME 面板窗口的尺寸/位置（对齐 Android `OpiImeService.keyboardHeight` 与
// `applyKeyboardWindow`）。
//
// 高度公式（2026-10-09 Android 实测调过，2026-10-15 加拼音行）：
//   side      = min(屏宽, 屏高)              —— 横屏时按短边算，否则会超屏
//   keyArea   = side × 0.48 + 76dp × density
//   composing = 38dp × density               —— 拼音行常驻，必须计入窗口
//   height    = min(keyArea + composing, 屏高)
// 游戏模式再 × 0.72；悬浮/游戏宽度 = 屏宽 × 0.86，水平居中、贴底。
//
// 单位：display 的 width/height 是 **px**，`densityPixels` 是 px/vp 比 ——
// 76dp / 38dp 必须先乘密度再进窗口。别把 dp 当 px 直接传 resize（矮 3 倍）。
//
// 旋转/折叠：display 的 'change' 事件触发时由控制器重读尺寸并重新 resize；
// 320–430vp 手机、横屏、折叠屏展开都走同一条公式（短边基数 + 高度钳制）。

import { display } from '@kit.ArkUI';

const KEY_AREA_RATIO: number = 0.48;
const KEY_AREA_EXTRA_DP: number = 76;
const COMPOSING_BAR_DP: number = 38;
const FLOATING_WIDTH_RATIO: number = 0.86;
const GAME_HEIGHT_RATIO: number = 0.72;

export class ScreenMetrics {
  width: number = 0;
  height: number = 0;
  /** px / vp（display.densityPixels；缺省 1 防除零）。 */
  density: number = 1;
}

export class PanelRect {
  x: number = 0;
  y: number = 0;
  w: number = 0;
  h: number = 0;
}

/** 读当前默认显示的尺寸（异常 → 全 0，调用方跳过 resize）。 */
export function currentMetrics(): ScreenMetrics {
  const m: ScreenMetrics = new ScreenMetrics();
  try {
    const d: display.Display = display.getDefaultDisplaySync();
    m.width = d.width;
    m.height = d.height;
    m.density = d.densityPixels > 0 ? d.densityPixels : 1;
  } catch (e) {
    console.error(`[opi] 读取显示尺寸失败：${e}`);
  }
  return m;
}

/** 按当前模式算窗口矩形（px）。 */
export function panelRect(m: ScreenMetrics, game: boolean, floating: boolean): PanelRect {
  const r: PanelRect = new PanelRect();
  if (m.width <= 0 || m.height <= 0) {
    return r;
  }
  const side: number = Math.min(m.width, m.height);
  const keyArea: number = Math.floor(side * KEY_AREA_RATIO)
    + Math.floor(KEY_AREA_EXTRA_DP * m.density);
  const composing: number = Math.floor(COMPOSING_BAR_DP * m.density);
  let h: number = Math.min(keyArea + composing, m.height);
  if (game) {
    h = Math.max(1, Math.floor(h * GAME_HEIGHT_RATIO));
  }
  let w: number = m.width;
  if (floating || game) {
    w = Math.floor(m.width * FLOATING_WIDTH_RATIO);
  }
  r.w = w;
  r.h = h;
  r.x = Math.floor((m.width - w) / 2);
  r.y = m.height - h;
  return r;
}

/** 订阅显示变化（旋转 / 折叠 / 分屏）。cb 必须是稳定引用，退订要用同一个。 */
export function watchDisplay(cb: (displayId: number) => void): void {
  try {
    display.on('change', cb);
  } catch (e) {
    console.error(`[opi] 订阅显示变化失败：${e}`);
  }
}

export function unwatchDisplay(cb: (displayId: number) => void): void {
  try {
    display.off('change', cb);
  } catch (e) {
    console.error(`[opi] 退订显示变化失败：${e}`);
  }
}
