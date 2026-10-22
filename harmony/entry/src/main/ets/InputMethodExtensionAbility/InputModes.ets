// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**（`import ... from 'libopiime.so'`
// 在普通 tsc 下就断）。装两样东西，都是「输入模式」这件事的数字面：
//   1. 方案 / 双拼布局 / 五笔六键 / 词表出口的类型化包装（2026-10-14 加；C ABI 的同名
//      声明见 cpp/opi_ffi.h，桥见 cpp/napi_bridge.c，.d.ts 见 cpp/types/libopiime/）；
//   2. 九项「输入模式」表 + 当前模式的状态机（对齐 android/.../ime/InputMode.kt 与
//      ImeState.selectInputMode / layoutFor）。
//
// ---- 为什么另起文件、不塞进 OpiEngine.ets ----
// OpiEngine.ets 是门禁锚点（`crates/opi-ffi/tests/keycode_contract.rs` 钉死它里面
// 18 条 KEY_*/STATE_* 常量），且行数余量装不下这十几条。方案/五笔这族数字与键码无关、
// 也不在那道门禁的扫描面上，独立成文件互不牵连。
//
// ---- 数字的真源（UI 里别再编一套）----
//   · 方案整数：crates/engine-core/src/scheme.rs 的 `Scheme::code()`：0=全键盘
//     1=拼音9键 2=拼音14键 3=笔画 4=双拼 5=五笔；按键类表在同文件 `Scheme::key_class()`。
//   · 双拼布局：engine_shuangpin.rs（0=自然码 1=微软 2=小鹤 3=智能ABC 4=紫光 5=拼音加加）。
//   · 五笔六键：engine_wubi.rs（风格/混入方式/回车三个枚举 + 候选数 1..=9 缺省 5 +
//     自动上屏缺省 true）；名字表在 android/.../settings/WubiNames.kt。
//   · 语言模式：本目录 OpiEngine.ets 的 OpiMode（api::mode_from_int 的 0..=4）。
//
// ⚠️ **读侧哨兵**：引擎未装载时所有**读**出口回 0 / false —— 那**不是**「用户选的就是
// 全键盘/自然码/纯五笔」。所以「当前模式」由**本文件自己存**（见下方状态机），
// **别把读侧当 UI 的初始值**。

import opiime from 'libopiime.so';
import * as OpiEngine from './OpiEngine';
import { OpiMode } from './OpiEngine';
import { leaveEnglishPro } from './KeyboardModes';
import * as SettingsStore from './SettingsStore';

/** 输入方案。数值就是 C ABI 的整数码（= `Scheme::code()`），不是本地枚举序号。 */
export enum OpiScheme {
  Full = 0,
  Nine = 1,
  Fourteen = 2,
  Stroke = 3,
  Shuangpin = 4,
  Wubi = 5,
}

// ---------------------------------------------------------------- 引擎出口包装

/**
 * 装载笔画词典（stroke.opid）。**空/坏路径 → false**：方案照切、键照样进引擎，
 * 只是**查不出字**（不假装有候选）。文件不是文件系统路径 —— rawfile 要先读进沙箱，
 * 再把沙箱里的绝对路径传进来（见 KeyboardController.ensureDictionary 的注释）。
 */
export function loadStroke(path: string): boolean {
  return opiime.loadStroke(path);
}

/**
 * 装载五笔码表（wubi86.opid / wubi98.opid，**按方案只装一张**：两表码空间重叠、
 * 权重不可比，同时装就是两套体系打架）。
 * ⚠️ **装载不换方案** —— 要用五笔还得 `setScheme(OpiScheme.Wubi)`。
 */
export function loadWubi(path: string): boolean {
  return opiime.loadWubi(path);
}

/** 方案读侧（未装载 → 0 是哨兵）。⚠️ `setScheme` **会清缓冲** ⇒ 调用后重读 buffer/候选/页码。 */
export function scheme(): number {
  return opiime.scheme();
}

/** 换方案。**越界静默忽略**（引擎侧），这里先夹紧到 0..=5。**清缓冲。** */
export function setScheme(s: OpiScheme): void {
  const v: number = s as number;
  if (v < 0 || v > 5) {
    return;
  }
  opiime.setScheme(v);
}

/** 双拼布局：0=自然码 1=微软 2=小鹤 3=智能ABC 4=紫光 5=拼音加加（越界钳到端点）。**不清缓冲。** */
export function setShuangpinLayout(index: number): void {
  opiime.setShuangpinLayout(index);
}

/** 布局读侧（未装载 → 0 是哨兵）。 */
export function shuangpinLayout(): number {
  return opiime.shuangpinLayout();
}

// ---- 五笔六键（风格/混入方式/候选个数/回车/自动上屏；读侧同值哨兵）----
//
// ⚠️ **本端暂无调用方**：设置页虽已落地，但未接五笔选项（Android 的对应物是设置页的「五笔」节）。
// 这五对先接通，是为了让「设置页落地」变成纯 UI 工作（不必再动 C ABI/桥/.d.ts 三层）。
// **别在 UI 里重编这些整数**：值表真源 `engine_wubi.rs`。

/** 风格：0=纯五笔 1=五笔拼音混输（越界忽略；读侧未装载 → 0 哨兵）。 */
export function setWubiStyle(style: number): void {
  opiime.setWubiStyle(style);
}
export function wubiStyle(): number {
  return opiime.wubiStyle();
}

/** 混输进入方式：0=直接字母 1=z键（越界忽略；读侧未装载 → 0 哨兵）。 */
export function setWubiMixedEntry(entry: number): void {
  opiime.setWubiMixedEntry(entry);
}
export function wubiMixedEntry(): number {
  return opiime.wubiMixedEntry();
}

/** 候选个数：引擎侧 **1..=9 钳区间**（缺省 5；UI 档位见 Android WubiNames 的 3..9）。 */
export function setWubiCandidateLimit(n: number): void {
  opiime.setWubiCandidateLimit(n);
}
export function wubiCandidateLimit(): number {
  return opiime.wubiCandidateLimit();
}

/** 回车行为：0=清空 1=上屏原文（越界忽略；读侧未装载 → 0 哨兵）。 */
export function setWubiReturnStyle(style: number): void {
  opiime.setWubiReturnStyle(style);
}
export function wubiReturnStyle(): number {
  return opiime.wubiReturnStyle();
}

/** 自动上屏（缺省 true；读侧未装载 → **false 是哨兵**，不是「用户关掉了」）。 */
export function setWubiAutoCommit(on: boolean): void {
  opiime.setWubiAutoCommit(on);
}
export function wubiAutoCommit(): boolean {
  return opiime.wubiAutoCommit();
}

// ---------------------------------------------------------------- 九项「输入模式」

/**
 * 一项输入模式 = **布局 + 引擎方案 + 语言模式**（Android `InputMode` 的逐条镜像）。
 *
 * ⚠️ 三根轴分清楚（Android 文件里同一条警告）：
 *   · `scheme`（引擎）：按键 → 字母类（9 键/14 键/笔画/全键盘/双拼/五笔）；
 *   · `mode`（引擎）：语言变体（拼音/繁体/英文…），**与方案正交**；
 *   · 本类（UI）：菜单里给用户看的那一项 —— 每项映射到上面两个引擎轴。
 *
 * `big`（大 9 键）与 `panel`（生僻字）**不进引擎**：前者只差字号，后者是面板视图。
 */
export class InputMode {
  label: string;
  scheme: OpiScheme;
  mode: OpiMode;
  big: boolean = false;
  panel: boolean = false;

  constructor(label: string, scheme: OpiScheme, mode: OpiMode, big: boolean = false,
    panel: boolean = false) {
    this.label = label;
    this.scheme = scheme;
    this.mode = mode;
    this.big = big;
    this.panel = panel;
  }
}

/** 九项。**下标就是 AppStorage `opiLayout` 存的值** ⇒ 只许在末尾增，不许改序。 */
export const INPUT_MODES: InputMode[] = [
  new InputMode('拼音26键', OpiScheme.Full, OpiMode.Pinyin),
  new InputMode('英文26键', OpiScheme.Full, OpiMode.English),
  new InputMode('拼音9键', OpiScheme.Nine, OpiMode.Pinyin),
  new InputMode('拼音14键', OpiScheme.Fourteen, OpiMode.Pinyin),
  new InputMode('拼音大9键', OpiScheme.Nine, OpiMode.Pinyin, true),
  new InputMode('笔画键盘', OpiScheme.Stroke, OpiMode.Pinyin),
  new InputMode('生僻字键盘', OpiScheme.Full, OpiMode.Pinyin, false, true),
  new InputMode('拼音双拼', OpiScheme.Shuangpin, OpiMode.Pinyin),
  new InputMode('五笔', OpiScheme.Wubi, OpiMode.Pinyin),
];

/** 九项的下标常量（Index.ets 按它挑键面；数字与上表逐条对应）。 */
export const MODE_PINYIN26: number = 0;
export const MODE_ENGLISH26: number = 1;
export const MODE_PINYIN9: number = 2;
export const MODE_PINYIN14: number = 3;
export const MODE_BIG9: number = 4;
export const MODE_STROKE: number = 5;
export const MODE_RARE: number = 6;
export const MODE_SHUANGPIN: number = 7;
export const MODE_WUBI: number = 8;

// ---------------------------------------------------------------- 模式状态机
//
// 状态放在**模块级**（不是 Index 的 @State、也不是 KeyboardController 的字段）：
//   · 面板可被系统重建（面板尺寸变化会重建 UI）⇒ @State 会丢；
//   · 五个键面 + 面板都要读写它 ⇒ 放模块里就不用给每个组件传回调。
// 2026-10-21 起持久化（SettingsStore）：选中即落盘，扩展 init 时 restore() 装回来。
// 缺省仍是**拼音9键**（Android 2026-10-11 用户裁决「移动端默认初始 9 键」）。

let currentIndex: number = MODE_PINYIN9;
/** 上次选中过的**中文**键位（对齐 Android `ImeState.lastChineseMode`）：英文键盘
 *  右上角隐藏「中」键切回它；初始 = 拼音9键（Android 同款）。 */
let lastChineseIndex: number = MODE_PINYIN9;

/**
 * 从偏好恢复（扩展 init 时一次，**在词库装载前**）：只写模块状态，不碰引擎。
 * 词库装好后由 `applyCurrentMode()` 把方案/语言推给引擎。
 */
export function restore(inputIndex: number, lastChinese: number): void {
  if (inputIndex >= 0 && inputIndex < INPUT_MODES.length) {
    currentIndex = inputIndex;
  }
  if (lastChinese >= 0 && lastChinese < INPUT_MODES.length
    && INPUT_MODES[lastChinese].mode === (OpiMode.Pinyin as number)) {
    lastChineseIndex = lastChinese;
  }
}

/** 把当前档位推给引擎（词库装载后调一次；不落盘）。 */
export function applyCurrentMode(): void {
  const m: InputMode = INPUT_MODES[currentIndex];
  if (m.panel) {
    return;
  }
  setScheme(m.scheme);
  if ((OpiEngine.mode() as number) !== (m.mode as number)) {
    OpiEngine.switchMode(m.mode);
  }
}

/** 当前模式下标。 */
export function currentModeIndex(): number {
  return currentIndex;
}

/** 面板视图的开关在 KeyboardModes.ets（与皮肤/键盘模式/关于同一套 state）。 */

/**
 * 选一项输入模式：换布局 + 换引擎方案 + 必要时切语言模式。
 *
 * **生僻字（panel）不碰引擎** —— 与 Android `InputMode.RARE` 同一条：它只是面板视图，
 * 引擎方案/模式原地不动（选中它再返回，打字行为不变）。
 *
 * ⚠️ **与 Android 已知的差异（如实记）**：那边选之前会把待提交缓冲**提交掉**
 * （`commitPendingBuffer`），本端**没有**这条路 —— `setScheme` / `switchMode` 在引擎侧
 * 都是清缓冲 ⇒ 选模式时未确认的拼音是**丢掉**的。要补得先有「把文本插入文档」的接口
 * （与 `OpiEngine.toggleSymbol` 的前置条件是同一条）。
 */
export function selectInputMode(i: number): void {
  if (i < 0 || i >= INPUT_MODES.length) {
    return;
  }
  const m: InputMode = INPUT_MODES[i];
  if (m.panel) {
    // 生僻字是**面板视图**，不进引擎、也不改当前键位（Android 同一条）。
    return;
  }
  // 离开英文去别的模式时关掉 pro（Android leaveEnglishProFor 的同一裁决）。
  if ((OpiEngine.mode() as number) === (OpiMode.English as number)
    && (m.mode as number) !== (OpiMode.English as number)) {
    leaveEnglishPro();
  }
  // 英文不在中文侧：只有 Pinyin 档（含 9/14/大9/笔画/双拼/五笔）才更新回切目标。
  if (m.mode === (OpiMode.Pinyin as number)) {
    lastChineseIndex = i;
    SettingsStore.saveLastChinese(i);
  }
  setScheme(m.scheme); // 清缓冲（引擎侧）⇒ 调用方随后要重读 buffer/候选/页码
  if ((OpiEngine.mode() as number) !== (m.mode as number)) {
    // 与 KeyboardController.switchMode 同一份语义：切模式。⇧/引擎 shift 的同步归
    // KeyRouter::switch_mode（进入 English 重推前端三态；别在此 setShift(false)）。
    // （缓冲不必再清一次：引擎的 switch_mode 自己就清。）
    OpiEngine.switchMode(m.mode);
  }
  currentIndex = i;
  SettingsStore.saveInputMode(i);
}

/**
 * 英文键盘右上角隐藏「中」键的目标（2026-10-20）：上次选中的中文键位。
 * 调用方走 `KeyboardController.selectInputMode(...)`（它负责 publish），语义与
 * Android 隐藏键走 `cycleMode()` 的英→中腿同义（切之前清英文缓冲、不提交）。
 */
export function lastChineseModeIndex(): number {
  return lastChineseIndex;
}

/**
 * 26 键模式键的键面字 = **点它会切到哪**（Android `modeKeyLabelOf` 的三态循环）：
 * 中→繁、繁→英、英→中。数字/符号态从 26 键进不去（数字走 123，符号走 ?123），
 * 落到「中」是安全侧。
 */
export function cycleModeLabel(m: number): string {
  if (m === (OpiMode.Pinyin as number)) {
    return '繁';
  }
  if (m === (OpiMode.Traditional as number)) {
    return '英';
  }
  return '中';
}

/** 9/14/笔画底行的模式键：**只切简繁**（Android 2026-10-15 裁决），键面 = 点它会到哪。 */
export function traditionalModeLabel(m: number): string {
  return m === (OpiMode.Traditional as number) ? '中' : '繁';
}

/**
 * 「随包词表」的装载编排（笔画 / 五笔）。路径来自 DictionaryAssets 的沙箱副本；
 * 空串 = 该表没随包或拷贝失败 ⇒ C ABI 语义是「**返回 false 且引擎保持原样**」
 * （不崩、不假装有候选，对应方案下只是查不出字）。
 *
 * 五笔**只装一张**：86/98 码空间重叠、权重不可比。设置页未提供 86/98 选择 ⇒ 固定 86
 * （缺省与 Android 一致）。
 */
export function loadOptionalTables(strokePath: string, wubiPath: string): void {
  const strokeOk: boolean = loadStroke(strokePath);
  const wubiOk: boolean = loadWubi(wubiPath);
  if (!strokeOk) {
    console.warn('[opi] stroke.opid 未装载：笔画方案将查不出字（引擎未装载该表）');
  }
  if (!wubiOk) {
    console.warn('[opi] wubi86.opid 未装载：五笔方案将查不出字（引擎未装载该表）');
  }
}
