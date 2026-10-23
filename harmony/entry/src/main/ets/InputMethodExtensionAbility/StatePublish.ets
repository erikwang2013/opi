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
  KEY_ALL_CANDIDATES, KEY_BUFFER, KEY_CANDIDATES, KEY_DARK, KEY_DISPLAY_BUFFER, KEY_ELDER,
  KEY_FLOAT, KEY_GAME, KEY_LEARNER, KEY_MODE, KEY_MODE_INDEX, KEY_ONE_HAND, KEY_PAGE,
  KEY_PAGE_COUNT, KEY_PRO, KEY_READINGS, KEY_SHIFT, KEY_SHOW_OWN, KEY_SKIN, KEY_TILED, KEY_VIEW
} from './KeyboardAppStorage';

/** 平铺视图取全量的条数（同 Android EngineController.fetchLimit）。 */
export const TILED_FETCH_LIMIT: number = 64;

/**
 * 平铺候选是否还能站住（Android `ImeState.candidatesShowable` 的镜像）：
 * 英文（空闲提示条也算）/ 缓冲非空 / 有当前页候选。
 */
function candidatesShowable(buffer: string, candidates: string[], mode: OpiMode): boolean {
  return mode === OpiMode.English || buffer.length > 0 || candidates.length > 0;
}

export function publishState(): void {
  // 行为判定用 buffer（原值）；拼音行用 displayBuffer（9 键带分词）。
  const buffer: string = OpiEngine.buffer();
  const pageCandidates: string[] = OpiEngine.candidatesPage();
  // 读数与当前候选批一起缓存，**紧跟** candidatesPage 读；displayBuffer 仍最后。
  const readings: string[] = OpiEngine.candidateReadings();
  // ⚠️ displayBuffer **必须最后读**：9 键读数是 candidates() 的副产物（缓存与算它时的
  // 缓冲绑存），先读会拿到上一条缓冲的旧值/空串 —— Android EngineController 同款次序。
  const display: string = OpiEngine.displayBuffer();
  const mode: OpiMode = OpiEngine.mode();
  // 平铺里候选用尽（选词 / ⌫ 退到空）→ 自动收拢，否则键盘回不来（Android 同款）。
  if (KeyboardModes.tiledEnabled() && !candidatesShowable(buffer, pageCandidates, mode)) {
    KeyboardModes.collapseTiled();
  }
  const tiled: boolean = KeyboardModes.tiledEnabled();

  AppStorage.setOrCreate<string>(KEY_BUFFER, buffer);
  AppStorage.setOrCreate<string>(KEY_DISPLAY_BUFFER, display);
  AppStorage.setOrCreate<string[]>(KEY_CANDIDATES, pageCandidates);
  AppStorage.setOrCreate<string[]>(KEY_READINGS, readings);
  AppStorage.setOrCreate<number>(KEY_PAGE, OpiEngine.page());
  AppStorage.setOrCreate<number>(KEY_PAGE_COUNT, OpiEngine.pageCount());
  AppStorage.setOrCreate<number>(KEY_MODE, mode as number);
  AppStorage.setOrCreate<number>(KEY_SHIFT, OpiEngine.shiftState() as number);
  AppStorage.setOrCreate<number>(KEY_LEARNER, OpiEngine.learnerEnabled() ? 1 : 0);
  // 这两条不来自引擎（模块级状态机）：未装载时读侧回 0/false 是哨兵，别拿它当 UI 档位。
  AppStorage.setOrCreate<number>(KEY_MODE_INDEX, InputModes.currentModeIndex());
  AppStorage.setOrCreate<number>(KEY_VIEW, KeyboardModes.panelView());
  AppStorage.setOrCreate<number>(KEY_SKIN, KeyboardModes.skinValue());
  AppStorage.setOrCreate<number>(KEY_DARK, KeyboardModes.darkEnabled() ? 1 : 0);
  AppStorage.setOrCreate<number>(KEY_ELDER, KeyboardModes.elderEnabled() ? 1 : 0);
  AppStorage.setOrCreate<number>(KEY_ONE_HAND, KeyboardModes.oneHandValue());
  AppStorage.setOrCreate<number>(KEY_FLOAT, KeyboardModes.floatingEnabled() ? 1 : 0);
  AppStorage.setOrCreate<number>(KEY_GAME, KeyboardModes.gameEnabled() ? 1 : 0);
  AppStorage.setOrCreate<number>(KEY_PRO, KeyboardModes.englishProEnabled() ? 1 : 0);
  AppStorage.setOrCreate<number>(KEY_SHOW_OWN, KeyboardModes.showOwnEnabled() ? 1 : 0);
  AppStorage.setOrCreate<number>(KEY_TILED, tiled ? 1 : 0);
  // 平铺没开就不取全量：每键都拉 64 条纯属浪费（普通候选栏只用当前页）。
  AppStorage.setOrCreate<string[]>(KEY_ALL_CANDIDATES,
    tiled ? OpiEngine.candidates(TILED_FETCH_LIMIT) : []);
}
