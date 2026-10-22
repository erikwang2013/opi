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
/** 显示用缓冲（9 键带分词读数 `ni'hao`）；行为判定仍读 KEY_BUFFER。 */
export const KEY_DISPLAY_BUFFER: string = 'opiDisplayBuffer';
export const KEY_CANDIDATES: string = 'opiCandidates';
export const KEY_PAGE: string = 'opiPage';
export const KEY_PAGE_COUNT: string = 'opiPageCount';
export const KEY_MODE: string = 'opiMode';
export const KEY_SHIFT: string = 'opiShift';
export const KEY_LEARNER: string = 'opiLearner';
/** 当前输入模式下标（InputModes 的 INPUT_MODES 下标，**不是**引擎方案码）。 */
export const KEY_MODE_INDEX: string = 'opiLayout';
/** 面板视图 / 皮肤 / 暗光 / 长辈 / 单手 / 悬浮 / 游戏 / 英文 pro（KeyboardModes 的模块级状态）。 */
export const KEY_VIEW: string = 'opiView';
export const KEY_SKIN: string = 'opiSkin';
export const KEY_DARK: string = 'opiDark';
export const KEY_ELDER: string = 'opiElder';
export const KEY_ONE_HAND: string = 'opiOneHand';
export const KEY_FLOAT: string = 'opiFloat';
export const KEY_GAME: string = 'opiGame';
export const KEY_PRO: string = 'opiPro';
/** 表情页「自有」分类开关（随包偏好，缺省隐藏）。 */
export const KEY_SHOW_OWN: string = 'opiShowOwn';
/** 平铺候选视图（临时）：true 时候选栏收起、键区换成 CandidateTiled。 */
export const KEY_TILED: string = 'opiTiled';
/** 平铺视图用的**全量**候选（前 64 条；普通候选栏仍读 opiCandidates 当前页）。 */
export const KEY_ALL_CANDIDATES: string = 'opiAllCandidates';
