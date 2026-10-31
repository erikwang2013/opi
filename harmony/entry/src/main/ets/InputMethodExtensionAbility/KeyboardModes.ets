// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 「键盘模式」与面板视图的模块级状态（对齐 Android `ime/KeyboardModes.kt` + `ImeState.View`）。
//
// 与 InputModes.ets 同一条取舍：放模块级，面板重建（系统改窗口尺寸会重建 UI）不丢。
// 2026-10-21 起**持久化**（SettingsStore，对齐 Android PrefStore）：每个 setter 直接
// 落盘，`restore()` 在扩展 init 时把存过的值装回来。平铺候选是临时视图状态，不存。
//
// 繁体不在本文件：那是**引擎模式**（OpiMode.Traditional），UI 只读 AppStorage 的
// `opiMode`。悬浮/游戏落到 IME 窗口（KeyboardController.applyPanelWindow），
// 单手是纯布局层，暗光/长辈是色板 + 字号。

import { OpiMode } from './OpiEngine';
import * as SettingsStore from './SettingsStore';

/** 面板视图（整片盖住键面）。0 = 键面。 */
export enum PanelView {
  None = 0,
  InputMode = 1,
  Skin = 2,
  KeyboardMode = 3,
  About = 4,
  SymbolPage = 5,
  Rare = 6,
  Settings = 7,
}

export const ONE_HAND_OFF: number = 0;
export const ONE_HAND_LEFT: number = 1;
export const ONE_HAND_RIGHT: number = 2;

let view: number = PanelView.None;
/** 从设置页打开子面板时的返回落点（其余面板返回 = 键面）。 */
let returnView: number = PanelView.None;
let skinIndex: number = 5;
/** 动态插画主题 id；空 = 纯色皮肤。 */
let sceneId: string = '';
let darkOn: boolean = false;
let elderOn: boolean = false;
let oneHand: number = ONE_HAND_OFF;
let floatingOn: boolean = false;
let gameOn: boolean = false;
let englishProOn: boolean = false;
/** 表情页「自有」分类（Android ImePreferences.showOwn；缺省隐藏）。 */
let showOwnOn: boolean = false;
/** 智能联想（Android ImePreferences.association；缺省关）。 */
let associationOn: boolean = false;
/** 平铺候选视图（Android ImeState.View.CANDIDATES_TILED；临时状态，不落盘）。 */
let tiledOn: boolean = false;
/** 进表情页前的引擎模式（表情是 Symbol 模式，返回时切回）。 */
let emojiReturnMode: OpiMode = OpiMode.Pinyin;

/** 从偏好恢复（扩展 init 时一次；没存过的项保持现缺省）。 */
export function restore(s: SettingsStore.ImeSettings): void {
  if (s.skin >= 0 && s.skin < 12) { skinIndex = s.skin; }
  sceneId = s.scene;
  darkOn = s.dark;
  elderOn = s.elder;
  if (s.oneHand >= ONE_HAND_OFF && s.oneHand <= ONE_HAND_RIGHT) { oneHand = s.oneHand; }
  floatingOn = s.floating;
  gameOn = s.game;
  englishProOn = s.englishPro;
  showOwnOn = s.showOwn;
  associationOn = s.association;
}

export function panelView(): number { return view; }

/** 开面板 = 离开平铺视图（Android：打开别的 View 就不会回到 CANDIDATES_TILED）。 */
export function openPanel(v: PanelView): void {
  returnView = PanelView.None;
  view = v as number;
  tiledOn = false;
}

/** 设置页里的「皮肤 / 关于」：返回箭头回设置页，而不是直接回键面。 */
export function openSubPanel(v: PanelView): void {
  openPanel(v);
  returnView = PanelView.Settings;
}

/** 面板里再开面板（输入模式 → 生僻字）：保留原来的返回落点。 */
export function openNestedPanel(v: PanelView): void {
  const back: number = returnView;
  openPanel(v);
  returnView = back;
}

export function closePanel(): void {
  view = returnView as number;
  returnView = PanelView.None;
}

export function togglePanel(v: PanelView): void {
  returnView = PanelView.None;
  view = view === (v as number) ? (PanelView.None as number) : (v as number);
  tiledOn = false;
}

export function skinValue(): number { return skinIndex; }
export function setSkinValue(i: number): void {
  if (i >= 0 && i < 12) {
    skinIndex = i;
    SettingsStore.saveSkin(i);
  }
}

export function sceneValue(): string { return sceneId; }
export function setSceneValue(id: string): void {
  sceneId = id;
  SettingsStore.saveScene(id);
}
export function clearScene(): void {
  if (sceneId.length === 0) { return; }
  sceneId = '';
  SettingsStore.saveScene('');
}

export function darkEnabled(): boolean { return darkOn; }
export function toggleDark(): void {
  darkOn = !darkOn;
  SettingsStore.saveBool(SettingsStore.BOOL_DARK, darkOn);
}

export function elderEnabled(): boolean { return elderOn; }
export function toggleElder(): void {
  elderOn = !elderOn;
  SettingsStore.saveBool(SettingsStore.BOOL_ELDER, elderOn);
}

export function oneHandValue(): number { return oneHand; }
export function setOneHandValue(v: number): void {
  if (v >= ONE_HAND_OFF && v <= ONE_HAND_RIGHT) {
    oneHand = v;
    SettingsStore.saveOneHand(v);
  }
}

export function floatingEnabled(): boolean { return floatingOn; }
export function toggleFloating(): void {
  floatingOn = !floatingOn;
  SettingsStore.saveBool(SettingsStore.BOOL_FLOATING, floatingOn);
}

export function gameEnabled(): boolean { return gameOn; }
export function toggleGame(): void {
  gameOn = !gameOn;
  SettingsStore.saveBool(SettingsStore.BOOL_GAME, gameOn);
}

export function englishProEnabled(): boolean { return englishProOn; }
export function toggleEnglishPro(): void {
  englishProOn = !englishProOn;
  SettingsStore.saveBool(SettingsStore.BOOL_ENGLISH_PRO, englishProOn);
}

/** 离开英文时关掉 pro（Android `leaveEnglishProFor` 的同一裁决；同样落盘）。 */
export function leaveEnglishPro(): void {
  if (!englishProOn) {
    return;
  }
  englishProOn = false;
  SettingsStore.saveBool(SettingsStore.BOOL_ENGLISH_PRO, false);
}

/** 表情页「自有」分类开关（Android ImePreferences.showOwn）。 */
export function showOwnEnabled(): boolean { return showOwnOn; }
export function toggleShowOwn(): void {
  showOwnOn = !showOwnOn;
  SettingsStore.saveBool(SettingsStore.BOOL_SHOW_OWN, showOwnOn);
}

/** 智能联想开关（2026-10-28，默认关）。 */
export function associationEnabled(): boolean { return associationOn; }
export function toggleAssociation(): void {
  associationOn = !associationOn;
  SettingsStore.saveBool(SettingsStore.BOOL_ASSOCIATION, associationOn);
}

/** 平铺候选：展开 / 收起（返回不动引擎、不清缓冲，Android collapseCandidates 同款）。 */
export function tiledEnabled(): boolean { return tiledOn; }
export function expandTiled(): void { tiledOn = true; }
export function collapseTiled(): void { tiledOn = false; }

export function setEmojiReturn(m: OpiMode): void { emojiReturnMode = m; }

export function emojiReturn(): OpiMode { return emojiReturnMode; }

/** 有任一键盘模式开启（Android `anyOn`；工具条读屏用，本端暂只用于提示文案）。 */
export function anyModeOn(): boolean {
  return floatingOn || gameOn || oneHand !== ONE_HAND_OFF || darkOn || elderOn;
}
