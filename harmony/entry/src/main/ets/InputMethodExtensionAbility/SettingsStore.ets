// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 输入法偏好的落盘层（对齐 Android `ImePreferences.kt`）。
//
// 存的是**跨进程**该记住的 UI 状态：输入模式 / 皮肤 / 键盘模式（悬浮·游戏·单手·
// 暗光·长辈）/ 英文 pro / 表情「自有」页开关 / 学习开关。暂不存：候选平铺（Android
// 也是临时视图状态）、繁体（引擎模式由输入模式的 Pinyin/Traditional 决定）。
//
// ⚠️ 只做存储，不 import KeyboardModes / InputModes —— 那两个模块的 setter 反过来
// 调本文件，互相 import 会绕成环。恢复值由 KeyboardController 在 init 时读出来、
// 再调各模块的 restore。
//
// 读写全部 try/catch：扩展进程里 preferences 不可用（沙箱/配额）时**退回缺省值**，
// 绝不因为记不住偏好而让键盘起不来。flush 的 Promise 失败只打日志。

import { preferences } from '@kit.ArkData';
import { common } from '@kit.AbilityKit';

const FILE_NAME: string = 'opi_ime_prefs';

const KEY_INPUT_MODE: string = 'input_mode';
const KEY_LAST_CHINESE: string = 'last_chinese_mode';
const KEY_SKIN: string = 'kb_skin';
const KEY_SCENE: string = 'kb_scene';
const KEY_DARK: string = 'kb_dark';
const KEY_ELDER: string = 'kb_elder';
const KEY_ONE_HAND: string = 'kb_one_hand';
const KEY_FLOATING: string = 'kb_floating';
const KEY_GAME: string = 'kb_game';
const KEY_ENGLISH_PRO: string = 'english_pro';
const KEY_SHOW_OWN: string = 'pet_show_own';
const KEY_LEARN: string = 'engine_learn';
/** 智能联想（2026-10-28 用户裁决默认关闭；与桌面/fcitx5/TSF 同语义）。 */
const KEY_ASSOCIATION: string = 'engine_association';
/** 9 键左列末格的自定义标点（与 Android `nine_custom_punct` 同键名）。 */
const KEY_CUSTOM_PUNCT: string = 'nine_custom_punct';

let store: preferences.Preferences | null = null;

/** 一次性读出的偏好。`-1` = 没存过（由调用方决定缺省）。 */
export class ImeSettings {
  inputModeIndex: number = -1;
  lastChineseIndex: number = -1;
  skin: number = 5;
  /** 动态插画主题 id；空 = 纯色皮肤。 */
  scene: string = '';
  dark: boolean = false;
  elder: boolean = false;
  oneHand: number = 0;
  floating: boolean = false;
  game: boolean = false;
  englishPro: boolean = false;
  showOwn: boolean = false;
  learn: boolean = true;
  association: boolean = false;
}

/** 绑定扩展上下文（每进程一次；失败时本模块退化成内存默认值）。 */
export function init(ctx: common.Context): void {
  try {
    const opt: preferences.Options = { name: FILE_NAME };
    store = preferences.getPreferencesSync(ctx, opt);
  } catch (e) {
    console.error(`[opi] 打开偏好存储失败（本轮不落盘）：${e}`);
    store = null;
  }
}

function readNumber(key: string, def: number): number {
  if (store === null) {
    return def;
  }
  try {
    const v: preferences.ValueType = store.getSync(key, def);
    return typeof v === 'number' ? v as number : def;
  } catch (e) {
    return def;
  }
}

function readBool(key: string, def: boolean): boolean {
  if (store === null) {
    return def;
  }
  try {
    const v: preferences.ValueType = store.getSync(key, def);
    return typeof v === 'boolean' ? v as boolean : def;
  } catch (e) {
    return def;
  }
}

function readString(key: string, def: string): string {
  if (store === null) {
    return def;
  }
  try {
    const v: preferences.ValueType = store.getSync(key, def);
    return typeof v === 'string' ? v as string : def;
  } catch (e) {
    return def;
  }
}

function flush(): void {
  if (store === null) {
    return;
  }
  store.flush().catch((e: Error) => {
    console.error(`[opi] 偏好落盘失败：${e.message}`);
  });
}

/** 读全部偏好（缺键给缺省；调用方可用 -1 判断「没存过」）。 */
export function load(): ImeSettings {
  const s: ImeSettings = new ImeSettings();
  s.inputModeIndex = readNumber(KEY_INPUT_MODE, -1);
  s.lastChineseIndex = readNumber(KEY_LAST_CHINESE, -1);
  s.skin = readNumber(KEY_SKIN, 5);
  s.scene = readString(KEY_SCENE, '');
  s.dark = readBool(KEY_DARK, false);
  s.elder = readBool(KEY_ELDER, false);
  s.oneHand = readNumber(KEY_ONE_HAND, 0);
  s.floating = readBool(KEY_FLOATING, false);
  s.game = readBool(KEY_GAME, false);
  s.englishPro = readBool(KEY_ENGLISH_PRO, false);
  s.showOwn = readBool(KEY_SHOW_OWN, false);
  s.learn = readBool(KEY_LEARN, true);
  s.association = readBool(KEY_ASSOCIATION, false);
  return s;
}

export function saveInputMode(index: number): void {
  if (store === null) {
    return;
  }
  try {
    store.putSync(KEY_INPUT_MODE, index);
    flush();
  } catch (e) {
    console.error(`[opi] 保存输入模式失败：${e}`);
  }
}

export function saveLastChinese(index: number): void {
  if (store === null) {
    return;
  }
  try {
    store.putSync(KEY_LAST_CHINESE, index);
    flush();
  } catch (e) {
    console.error(`[opi] 保存中文键位失败：${e}`);
  }
}

export function saveSkin(index: number): void {
  if (store === null) {
    return;
  }
  try {
    store.putSync(KEY_SKIN, index);
    flush();
  } catch (e) {
    console.error(`[opi] 保存皮肤失败：${e}`);
  }
}

export function saveScene(id: string): void {
  if (store === null) {
    return;
  }
  try {
    store.putSync(KEY_SCENE, id);
    flush();
  } catch (e) {
    console.error(`[opi] 保存动态皮肤失败：${e}`);
  }
}

/** 9 键自定义标点：空串 = 还没选过（键面显示「自定义」）。 */
export function customPunc(): string {
  return readString(KEY_CUSTOM_PUNCT, '');
}

export function saveCustomPunc(value: string): void {
  if (store === null) {
    return;
  }
  try {
    store.putSync(KEY_CUSTOM_PUNCT, value);
    flush();
  } catch (e) {
    console.error(`[opi] 保存自定义标点失败：${e}`);
  }
}

export function saveBool(key: string, value: boolean): void {
  if (store === null) {
    return;
  }
  try {
    store.putSync(key, value);
    flush();
  } catch (e) {
    console.error(`[opi] 保存开关失败（${key}）：${e}`);
  }
}

export function saveOneHand(value: number): void {
  if (store === null) {
    return;
  }
  try {
    store.putSync(KEY_ONE_HAND, value);
    flush();
  } catch (e) {
    console.error(`[opi] 保存单手模式失败：${e}`);
  }
}

/** 具名布尔键（模块 setter 不各自拼字符串；拼错就是静默不落盘）。 */
export const BOOL_DARK: string = KEY_DARK;
export const BOOL_ELDER: string = KEY_ELDER;
export const BOOL_FLOATING: string = KEY_FLOATING;
export const BOOL_GAME: string = KEY_GAME;
export const BOOL_ENGLISH_PRO: string = KEY_ENGLISH_PRO;
export const BOOL_SHOW_OWN: string = KEY_SHOW_OWN;
export const BOOL_LEARN: string = KEY_LEARN;
export const BOOL_ASSOCIATION: string = KEY_ASSOCIATION;
