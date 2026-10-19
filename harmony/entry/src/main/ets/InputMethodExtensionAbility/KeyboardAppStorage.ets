// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

/**
 * AppStorage 键名 —— 写入侧（`KeyboardController.publish()`）的唯一真源。
 *
 * ⚠️ 读取侧（`pages/Index.ets` 9 条、`KeyPanels.ets` 3 处 `opiMode`）**用不了这些常量**：
 * @StorageLink 的入参必须是字符串字面量（ArkTS 编译期静态处理），写
 * `@StorageLink(KEY_BUFFER)` 编译不过。所以那些地方内联了字面量，**各有一份**，
 * 改这里必须同步改那边 —— 核对命令与症状写在 Index.ets 装饰器旁的注释里。
 * **不要再试图「统一成常量」**：那正是 ArkTS 禁止的写法。
 *
 * 本文件只放这组键名：KeyboardController 已到 500 行门禁边上，按职责把
 * 「AppStorage 契约」拆出来；不改变任何运行时行为。
 */
export const KEY_BUFFER: string = 'opiBuffer';
export const KEY_CANDIDATES: string = 'opiCandidates';
export const KEY_PAGE: string = 'opiPage';
export const KEY_PAGE_COUNT: string = 'opiPageCount';
export const KEY_MODE: string = 'opiMode';
export const KEY_SHIFT: string = 'opiShift';
export const KEY_LEARNER: string = 'opiLearner';
/** 当前输入模式下标（InputModes 的 INPUT_MODES 下标，**不是**引擎方案码）+ 面板开关（1/0）。 */
export const KEY_MODE_INDEX: string = 'opiLayout';
export const KEY_PANEL: string = 'opiPanel';
