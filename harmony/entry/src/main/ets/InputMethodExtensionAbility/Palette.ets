// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键盘配色：Android `keyboard/Palette.kt` 的**逐值镜像**（12 款皮肤 + 暗光 + 长辈）。
// 优先级与 Android 同一条：长辈 > 暗光 > 皮肤（「不改动」= 浅色板）。
//
// Swift/Kotlin 那边色板分文件、各自持有字面量；鸿蒙只在**本文件**存一份，UI 全部
// `paletteFor(skin, dark, elder)`。加皮肤 = 在这里加一行 SKINS + 一个 Palette 常量。

export class Palette {
  keyBg: string;
  keyFg: string;
  highlightBg: string;
  highlightFg: string;
  barBg: string;
  bufferFg: string;
  dimFg: string;
  accent: string;
  panelBg: string;
  panelFg: string;
  toolbarBg: string;
  toolbarFg: string;

  constructor(keyBg: string, keyFg: string, highlightBg: string, highlightFg: string,
    barBg: string, bufferFg: string, dimFg: string, accent: string,
    panelBg: string, panelFg: string, toolbarBg: string, toolbarFg: string) {
    this.keyBg = keyBg;
    this.keyFg = keyFg;
    this.highlightBg = highlightBg;
    this.highlightFg = highlightFg;
    this.barBg = barBg;
    this.bufferFg = bufferFg;
    this.dimFg = dimFg;
    this.accent = accent;
    this.panelBg = panelBg;
    this.panelFg = panelFg;
    this.toolbarBg = toolbarBg;
    this.toolbarFg = toolbarFg;
  }
}

export class Skin {
  label: string;
  palette: Palette;

  constructor(label: string, palette: Palette) {
    this.label = label;
    this.palette = palette;
  }
}

const LIGHT: Palette = new Palette(
  '#B8E0E0E0', '#1F1F1F', '#546E7A', '#FFFFFF', '#EEEEEE', '#8A000000', '#757575',
  '#455A64', '#EEEEEE', '#1F1F1F', '#E0E0E0', '#455A64');
const DARK: Palette = new Palette(
  '#B82A2A2A', '#E6E6E6', '#4A6572', '#FFFFFF', '#1F1F1F', '#CCE6E6E6', '#9E9E9E',
  '#9FB4BE', '#1F1F1F', '#E6E6E6', '#2A2A2A', '#CFD8DC');
const ELDER: Palette = new Palette(
  '#FAFAFA', '#000000', '#263238', '#FFFFFF', '#FFFFFF', '#FF000000', '#424242',
  '#263238', '#FFFFFF', '#000000', '#F5F5F5', '#263238');
const PAPER: Palette = new Palette(
  '#B8E7DFCC', '#3D3222', '#8A5A2B', '#FFFBF2', '#F5EFE1', '#CC3D3222', '#8A7A5B',
  '#8A5A2B', '#F5EFE1', '#3D3222', '#E7DFCC', '#8A5A2B');
const CINNABAR: Palette = new Palette(
  '#B89E2228', '#FFF6EC', '#FFD9A0', '#5A1012', '#8A1C21', '#CCFFF6EC', '#E8BCB4',
  '#FFD9A0', '#B89E2228', '#FFF6EC', '#8A1C21', '#FFD9A0');
const FRESH: Palette = new Palette(
  '#B8CFE9DF', '#16332B', '#2E7D6B', '#FFFFFF', '#E6F5EE', '#CC16332B', '#4F7A6C',
  '#2E7D6B', '#E6F5EE', '#16332B', '#CFE9DF', '#2E7D6B');
const AZURE: Palette = new Palette(
  '#B83C6AA3', '#F4F8FD', '#CFE6FF', '#12395C', '#3A679E', '#F0F4F8FD', '#C9DCF2',
  '#FFE3A8', '#3A679E', '#F4F8FD', '#33598A', '#FFE3A8');
const LEMON: Palette = new Palette(
  '#B8F9EA6C', '#3A3000', '#7E6200', '#FFFDF0', '#FBEE7E', '#CC3A3000', '#6E5F1E',
  '#7E6200', '#FBEE7E', '#3A3000', '#F9EA6C', '#5A4A00');
const SUNSET: Palette = new Palette(
  '#B8F4BC8A', '#4A2317', '#A63412', '#FFF3E8', '#F7C79C', '#CC4A2317', '#8E5A40',
  '#A63412', '#F7C79C', '#4A2317', '#F4BC8A', '#B4441F');
const STARLIGHT: Palette = new Palette(
  '#B8262E4A', '#E8EBF7', '#FFD966', '#23283F', '#161B2E', '#CCE8EBF7', '#9AA3C0',
  '#FFD966', '#1B2138', '#E8EBF7', '#262E4A', '#FFD966');
const WISTERIA: Palette = new Palette(
  '#B8E3D9F8', '#3A2A63', '#6B4BC4', '#F7F3FF', '#EDE7FB', '#CC3A2A63', '#6E5E96',
  '#6B4BC4', '#EDE7FB', '#3A2A63', '#E3D9F8', '#5A3AA8');
const PINE: Palette = new Palette(
  '#B826473A', '#DCEEE2', '#7FD3A2', '#17301F', '#1E3A2F', '#CCDCEEE2', '#9DBFAE',
  '#7FD3A2', '#1E3A2F', '#DCEEE2', '#26473A', '#7FD3A2');

/**
 * 12 款，顺序 = Android `SkinOption.entries`（opiSkin 存的就是本表下标）。
 * 键面 keyBg 统一带 72% alpha（#B8）：动态插画要透到键帽上（2026-10-31 用户
 * 要求）；ELDER 保持不透明（高对比优先）。
 */
export const SKINS: Skin[] = [
  new Skin('不改动', LIGHT),
  new Skin('浅色', LIGHT),
  new Skin('深色', DARK),
  new Skin('宣纸', PAPER),
  new Skin('朱砂', CINNABAR),
  new Skin('清爽', FRESH),
  new Skin('湛蓝', AZURE),
  new Skin('柠檬', LEMON),
  new Skin('晚霞', SUNSET),
  new Skin('星光', STARLIGHT),
  new Skin('紫藤', WISTERIA),
  new Skin('松林', PINE),
];

export function skinCount(): number {
  return SKINS.length;
}

/** 当前生效色板。长辈与暗光是「模式」；皮肤只是底（Android 同一条 when）。 */
export function paletteFor(skin: number, dark: boolean, elder: boolean): Palette {
  if (elder) {
    return ELDER;
  }
  if (dark) {
    return DARK;
  }
  const i: number = Math.max(0, Math.min(skin, SKINS.length - 1));
  return SKINS[i].palette;
}

/**
 * 动态皮肤下的面板底：保留原色相，只留 ~62% 不透明度让 Root 的插画透出来。
 * 纯色皮肤原值返回。scene 直接读 AppStorage，面板不必各挂一条 @StorageLink。
 */
export function scenePanelBg(color: string): string {
  const scene: string = AppStorage.get<string>('opiScene') ?? '';
  if (scene.length === 0) {
    return color;
  }
  const rgb: string = color.length === 9 ? color.slice(3) : color.slice(1);
  return '#9E' + rgb;
}
