// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 引擎/模式状态 → AppStorage 的单向推送（从 KeyboardController.publish 拆出，
// 2026-10-21：控制器已在上限边上，且平铺视图要在这里做自动收拢）。
//
// UI 不直接调引擎 —— 单一数据流向，省得两边各自缓存、各自漂移。
// 拼音只画键盘内 TopBar 行，**不写宿主预编辑**（2026-10-23 对齐 Android/iOS）。

import * as InputModes from './InputModes';
import * as KeyboardModes from './KeyboardModes';
import * as OpiEngine from './OpiEngine';
import { OpiMode } from './OpiEngine';
import {
  KEY_ALL_CANDIDATES, KEY_ASSOCIATION, KEY_ASSOCIATION_ON, KEY_BUFFER, KEY_CANDIDATES, KEY_DARK,
  KEY_DISPLAY_BUFFER,
  KEY_ELDER, KEY_FLOAT, KEY_GAME, KEY_LEARNER, KEY_MODE, KEY_MODE_INDEX, KEY_ONE_HAND,
  KEY_PRO, KEY_READINGS, KEY_SHIFT, KEY_SHOW_OWN, KEY_SKIN, KEY_TILED, KEY_VIEW
} from './KeyboardAppStorage';

/** 平铺视图取全量的条数（同 Android EngineController.fetchLimit）。 */
export const TILED_FETCH_LIMIT: number = 512;
/** 9 键左列读数最多缓存多少行（读数 JSON/节点比候选贵，仍截 64）。 */
const READING_LIMIT: number = 64;

/**
 * 平铺候选是否还能站住（Android `ImeState.candidatesShowable` 的镜像）：
 * 英文（空闲提示条也算）/ 缓冲非空 / 有当前页候选。
 */
function candidatesShowable(buffer: string, candidates: string[], mode: OpiMode): boolean {
  return mode === OpiMode.English || buffer.length > 0 || candidates.length > 0;
}


/** 只在值真的变了时写 AppStorage（2026-10-26 延迟修复：旧实现每键无条件写 17 项，
 *  ArkUI 对每个 @StorageLink 观察者都可能触发一轮重建；不变的 mode/skin/开关不该参与）。 */
function putNumber(key: string, value: number): void {
  if (AppStorage.get<number>(key) !== value) {
    AppStorage.setOrCreate<number>(key, value);
  }
}

function putString(key: string, value: string): void {
  if (AppStorage.get<string>(key) !== value) {
    AppStorage.setOrCreate<string>(key, value);
  }
}

function putStrings(key: string, value: string[]): void {
  const old: string[] | undefined = AppStorage.get<string[]>(key);
  if (old !== undefined && old.length === value.length) {
    let same: boolean = true;
    for (let i = 0; i < value.length; i++) {
      if (old[i] !== value[i]) {
        same = false;
        break;
      }
    }
    if (same) {
      return;
    }
  }
  AppStorage.setOrCreate<string[]>(key, value);
}

export function publishState(): void {
  // 行为判定用 buffer（原值）；拼音行用 displayBuffer（9 键带分词）。
  const buffer: string = OpiEngine.buffer();
  const pageCandidates: string[] = OpiEngine.candidatesPage();
  // 读数与当前候选批一起缓存，**紧跟** candidatesPage 读；displayBuffer 仍最后。
  // 左列最多滚 64 行；9 键一次可能产生上千条读数（JSON/节点都贵），截断到 64。
  const readings: string[] = OpiEngine.candidateReadings().slice(0, READING_LIMIT);
  // ⚠️ displayBuffer **必须最后读**：9 键读数是 candidates() 的副产物（缓存与算它时的
  // 缓冲绑存），先读会拿到上一条缓冲的旧值/空串 —— Android EngineController 同款次序。
  const display: string = OpiEngine.displayBuffer();
  const mode: OpiMode = OpiEngine.mode();
  // 智能联想默认关闭（2026-10-28）：空缓冲 + 空显示串下的候选就是联想项，整批挡在 UI 前。
  const associationOff: boolean = !KeyboardModes.associationEnabled() &&
    buffer.length === 0 && display.length === 0;
  const visibleCandidates: string[] = associationOff ? [] : pageCandidates;
  const visibleReadings: string[] = associationOff ? [] : readings;
  // 平铺里候选用尽（选词 / ⌫ 退到空）→ 自动收拢，否则键盘回不来（Android 同款）。
  if (KeyboardModes.tiledEnabled() && !candidatesShowable(buffer, visibleCandidates, mode)) {
    KeyboardModes.collapseTiled();
  }
  const tiled: boolean = KeyboardModes.tiledEnabled();

  // 联想态（2026-10-27，Spec §5）：空缓冲 + 空显示串 + 有当前页候选 + 拼音/繁体。
  // 前缀候选都要求非空缓冲，所以空缓冲下的候选只可能来自联想；缓存给 UI 与收起判据共用。
  putNumber(KEY_ASSOCIATION, buffer.length === 0 && display.length === 0 &&
    visibleCandidates.length > 0 &&
    (mode === OpiMode.Pinyin || mode === OpiMode.Traditional) ? 1 : 0);
  putNumber(KEY_ASSOCIATION_ON, KeyboardModes.associationEnabled() ? 1 : 0);
  putString(KEY_BUFFER, buffer);
  putString(KEY_DISPLAY_BUFFER, display);
  putStrings(KEY_CANDIDATES, visibleCandidates);
  putStrings(KEY_READINGS, visibleReadings);
  putNumber(KEY_MODE, mode as number);
  putNumber(KEY_SHIFT, OpiEngine.shiftState() as number);
  putNumber(KEY_LEARNER, OpiEngine.learnerEnabled() ? 1 : 0);
  // 这两条不来自引擎（模块级状态机）：未装载时读侧回 0/false 是哨兵，别拿它当 UI 档位。
  putNumber(KEY_MODE_INDEX, InputModes.currentModeIndex());
  putNumber(KEY_VIEW, KeyboardModes.panelView());
  putNumber(KEY_SKIN, KeyboardModes.skinValue());
  putNumber(KEY_DARK, KeyboardModes.darkEnabled() ? 1 : 0);
  putNumber(KEY_ELDER, KeyboardModes.elderEnabled() ? 1 : 0);
  putNumber(KEY_ONE_HAND, KeyboardModes.oneHandValue());
  putNumber(KEY_FLOAT, KeyboardModes.floatingEnabled() ? 1 : 0);
  putNumber(KEY_GAME, KeyboardModes.gameEnabled() ? 1 : 0);
  putNumber(KEY_PRO, KeyboardModes.englishProEnabled() ? 1 : 0);
  putNumber(KEY_SHOW_OWN, KeyboardModes.showOwnEnabled() ? 1 : 0);
  putNumber(KEY_TILED, tiled ? 1 : 0);
  // 平铺没开就不取全量：每键都拉 64 条纯属浪费（普通候选栏只用当前页）。
  putStrings(KEY_ALL_CANDIDATES,
    tiled && !associationOff ? OpiEngine.candidates(TILED_FETCH_LIMIT) : []);
}
