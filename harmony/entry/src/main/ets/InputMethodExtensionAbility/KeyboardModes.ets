// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 「键盘模式」与面板视图的模块级状态（对齐 Android `ime/KeyboardModes.kt` + `ImeState.View`）。
//
// 与 InputModes.ets 同一条取舍：放模块级，面板重建（系统改窗口尺寸会重建 UI）不丢。
// ⚠️ 进程重启即丢 —— 鸿蒙端**没有持久化**（Android 用 PrefStore；鸿蒙要接
// `@ohos.data.preferences` 才能记住，本轮未做，见 docs/install/harmony.md）。
//
// 繁体不在本文件：那是**引擎模式**（OpiMode.Traditional），UI 只读 AppStorage 的
// `opiMode`。悬浮/游戏落到 IME 窗口（KeyboardController.applyPanelWindow），
// 单手是纯布局层，暗光/长辈是色板 + 字号。

import { OpiMode } from './OpiEngine';

/** 面板视图（整片盖住键面）。0 = 键面。 */
export enum PanelView {
  None = 0,
  InputMode = 1,
  Skin = 2,
  KeyboardMode = 3,
  About = 4,
  SymbolPage = 5,
  Rare = 6,
}

export const ONE_HAND_OFF: number = 0;
export const ONE_HAND_LEFT: number = 1;
export const ONE_HAND_RIGHT: number = 2;

let view: number = PanelView.None;
let skinIndex: number = 0;
let darkOn: boolean = false;
let elderOn: boolean = false;
let oneHand: number = ONE_HAND_OFF;
let floatingOn: boolean = false;
let gameOn: boolean = false;
let englishProOn: boolean = false;
/** 进表情页前的引擎模式（表情是 Symbol 模式，返回时切回）。 */
let emojiReturnMode: OpiMode = OpiMode.Pinyin;

export function panelView(): number { return view; }

export function openPanel(v: PanelView): void { view = v as number; }

export function closePanel(): void { view = PanelView.None as number; }

export function togglePanel(v: PanelView): void {
  view = view === (v as number) ? (PanelView.None as number) : (v as number);
}

export function skinValue(): number { return skinIndex; }
export function setSkinValue(i: number): void {
  if (i >= 0 && i < 12) {
    skinIndex = i;
  }
}

export function darkEnabled(): boolean { return darkOn; }
export function toggleDark(): void { darkOn = !darkOn; }

export function elderEnabled(): boolean { return elderOn; }
export function toggleElder(): void { elderOn = !elderOn; }

export function oneHandValue(): number { return oneHand; }
export function setOneHandValue(v: number): void {
  if (v >= ONE_HAND_OFF && v <= ONE_HAND_RIGHT) {
    oneHand = v;
  }
}

export function floatingEnabled(): boolean { return floatingOn; }
export function toggleFloating(): void { floatingOn = !floatingOn; }

export function gameEnabled(): boolean { return gameOn; }
export function toggleGame(): void { gameOn = !gameOn; }

export function englishProEnabled(): boolean { return englishProOn; }
export function toggleEnglishPro(): void { englishProOn = !englishProOn; }

/** 离开英文时关掉 pro（Android `leaveEnglishProFor` 的同一裁决）。 */
export function leaveEnglishPro(): void { englishProOn = false; }

export function setEmojiReturn(m: OpiMode): void { emojiReturnMode = m; }

export function emojiReturn(): OpiMode { return emojiReturnMode; }

/** 有任一键盘模式开启（Android `anyOn`；工具条读屏用，本端暂只用于提示文案）。 */
export function anyModeOn(): boolean {
  return floatingOn || gameOn || oneHand !== ONE_HAND_OFF || darkOn || elderOn;
}
