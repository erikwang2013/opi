// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**（`import ... from 'libopiime.so'`
// 在普通 tsc 下就断）。它做三件事：
//   1. 把 libopiime.so 的数字/字符串出口包成有类型的 ArkTS API；
//   2. 定死**键码常量**（唯一定义处，别再在 UI 里写魔法数字）；
//   3. 把数字 → 枚举的映射收敛到一处，未知值一律降级成安全侧。
//
// **这里没有一行输入法逻辑** —— 全部在 crates/engine-core，与 Android / iOS /
// 桌面 / fcitx5 / TSF 共用同一份引擎。本文件只是搬运。

import opiime from 'libopiime.so';

// ---------------------------------------------------------------- 枚举

/** 引擎模式。**五个取值必须齐** —— 漏一个会让 UI 显示与引擎行为不一致。 */
export enum OpiMode {
  Pinyin = 0,
  English = 1,
  Number = 2,
  Symbol = 3,
  Traditional = 4,
}

/** keyEvent 的结果类型。对应 C ABI 的 OpiKeyEventResult.action。 */
export enum OpiAction {
  /** 0 = 未处理：**调用方自己把键输出去**（见 KeyboardController.pressKey） */
  PassThrough = 0,
  /** 1 = 已消费：引擎吃掉了，调用方什么都不要做 */
  Consumed = 1,
  /** 2 = 提交 text：把 text 上屏 */
  Commit = 2,
}

/** **前端** ⇧ 三态。⇧ 的高亮（尤其 Lock）只能读这个，引擎侧 shift 位看不出三态。 */
export enum OpiShift {
  Off = 0,
  Single = 1,
  Lock = 2,
}

// ---------------------------------------------------------------- 键码常量
//
// 真源：crates/engine-core/src/keys.rs。
// **特殊键 = 0x1_0000 | 低 16 位码**；可打印字符 = **Unicode 码点**。
// ⚠️ 空格是唯一不套 SPECIAL_BASE 的键（0x20 属于可打印段）。
// 用错编码的后果不是「多认一个键」而是键**静默失效**（详见 cabi.rs 的对照表）。

const SPECIAL_BASE: number = 0x10000;

/** 空格：**可打印段**，不是 SPECIAL_BASE|0x20。 */
export const KEY_SPACE: number = 0x20;
export const KEY_BACK_SPACE: number = SPECIAL_BASE | 0x08;
export const KEY_TAB: number = SPECIAL_BASE | 0x09;
export const KEY_RETURN: number = SPECIAL_BASE | 0x0D;
export const KEY_ESCAPE: number = SPECIAL_BASE | 0x1B;
export const KEY_PAGE_UP: number = SPECIAL_BASE | 0x80;
export const KEY_PAGE_DOWN: number = SPECIAL_BASE | 0x81;
export const KEY_DELETE: number = SPECIAL_BASE | 0x82;
export const KEY_SHIFT: number = SPECIAL_BASE | 0x83;
// ⚠️ `keys.rs` 还有 KEY_UP/DOWN/LEFT/RIGHT（`| 0x84`..`| 0x87`），**这里故意不抄**：
// 软键盘上没有方向键，而路由对这四个键一律 PassThrough（`router.rs` 的
// `KEY_TAB | KEY_ESCAPE | KEY_UP | ... => KeyAction::PassThrough`），
// 客户端抄了也没有分支可写。接物理键盘时再补，别现在抄。
// `cpp/opi_ffi.h` 同样只抄桥要用的那几条，是同一处取舍。

/** 修饰位（states 参数）。 */
export const STATE_SHIFT: number = 1 << 0;
export const STATE_CAPS_LOCK: number = 1 << 1;
export const STATE_CTRL: number = 1 << 2;
export const STATE_ALT: number = 1 << 3;
export const STATE_META: number = 1 << 4;
/**
 * 抬手事件。
 *
 * ⚠️ 这里原先写的是「只送按下会让 ⇧ 三态卡住」—— **因果是错的**（本轮订正）。
 * `router.rs` 的 `handle_shift`（`if !released && !repeat`）**明确忽略** released
 * 与 repeat：⇧ 只认按下，所以软键盘只送按下**不会**卡住三态。
 * 抬手真正影响的是别的键 —— `router.rs` 对退格/回车/空格/可打印键**都判 released**，
 * 目的是「按下放行、抬起也放行」保持对称：只放行按下会让宿主收到 keydown 收不到
 * keyup（依赖键状态的控件卡键）；可打印键更早还漏判过一次，同一个字符被送进引擎
 * 两次（"ni" → "nnii"）。
 *
 * 结论：**软键盘一次点击只送按下是对的**（见 `KeyboardController.pressKey`）。
 * 接物理键盘时才需要补抬手 —— 那时也**不要**顺手给 ⇧ 补抬手去「修三态」，
 * 那不是病根，只会多一条被忽略的事件。
 */
export const STATE_RELEASED: number = 1 << 26;

/**
 * 按键重复事件（长按自动连发）。
 *
 * ⚠️ 与 ⇧ **无关**：`router.rs` 的 `handle_shift` 把 released 与 repeat **一起忽略**
 * （`if !released && !repeat`）—— 长按连发不会把 ⇧ 推成 Lock。见下方 STATE_LONG_PRESSED。
 * 保留本常量是为了让键码表与 `keys.rs` 对齐、且**软键盘之外**的输入源（未来的
 * 物理键盘）能表达「这是连发而非新按下」。
 */
export const STATE_REPEAT: number = 1 << 27;

/**
 * 长按事件 —— **⇧ 进 Lock 的唯一途径**。
 *
 * ⚠️ 本常量与 STATE_REPEAT **原先一起漏抄**（只抄到 STATE_RELEASED），后果是 `OpiShift.Lock`
 * 成了**永远到不了的枚举值**：`router.rs` 的 `shift_long_press()` 是唯一把前端
 * 三态置成 Lock 的地方，而它只在 states 带本位时被调用；`shift_tap()` 只在
 * Off↔Single 之间来回。表现是 UI 里有 `⇪` 的分支、枚举里有 `Lock`，
 * 但用户**怎么按都出不来**（属于「静默失效」：不报错、不崩溃、功能就是没有）。
 * 键是 `KEY_SHIFT`，见 `KeyboardController.tapShiftLongPress`。
 */
export const STATE_LONG_PRESSED: number = 1 << 28;

// ---------------------------------------------------------------- 返回值

/** keyEvent 的类型化返回。 */
export class KeyResult {
  action: OpiAction = OpiAction.PassThrough;
  text: string = '';

  constructor(action: OpiAction, text: string) {
    this.action = action;
    this.text = text;
  }
}

/**
 * action 数字 → 枚举。
 * **未知值一律降级成 PassThrough（= 把键交还调用方），绝不吞键** ——
 * 这是 C ABI 自己的立场（引擎未装载/panic 时也是 PassThrough）。
 * 不抛异常：输入法路径上抛一个异常，整个键盘就废了。
 */
function toAction(v: number): OpiAction {
  if (v === 2) {
    return OpiAction.Commit;
  }
  if (v === 1) {
    return OpiAction.Consumed;
  }
  return OpiAction.PassThrough;
}

/** mode 数字 → 枚举。未知值 → Pinyin（兜底），但见 README 的告警：取值会漂。 */
function toMode(v: number): OpiMode {
  switch (v) {
    case 1:
      return OpiMode.English;
    case 2:
      return OpiMode.Number;
    case 3:
      return OpiMode.Symbol;
    case 4:
      return OpiMode.Traditional;
    default:
      return OpiMode.Pinyin;
  }
}

// ---------------------------------------------------------------- 出口

/**
 * 装载词库。
 * 传空串 → 内置回退词库（**开箱即用**，鸿蒙端第一步就靠它）；
 * 坏路径 → false，且**引擎保持为空** —— 此时所有出口都退化成空操作，
 * 表现是「键盘看着正常但一个键都不出字」。**返回值必须接。**
 */
export function loadDictionary(path: string): boolean {
  return opiime.load(path);
}

/** 当前拼音缓冲（= preedit 的来源）。 */
export function buffer(): string {
  return opiime.buffer();
}

/**
 * 当前页候选。
 * 出口是 JSON 文本数组（C ABI 用 JSON 表达多返回值），这里解码成 string[]。
 * 解码失败 → 空数组：宁可少显示候选，也不能让键盘崩。
 */
export function candidatesPage(): string[] {
  const raw: string = opiime.candidatesPage();
  if (raw.length === 0) {
    return [];
  }
  try {
    return JSON.parse(raw) as string[];
  } catch (e) {
    return [];
  }
}

/** 提交**当前页**第 k 个候选（页内索引，0 起）。越界/无候选 → 空串。 */
export function selectPage(index: number): string {
  return opiime.selectPage(index);
}

/**
 * 当前候选页 / 总页数。
 * **必须读这两个出口、不要自己数页** —— PageDown 到末页时路由会把页码钳到
 * 最后一页，本地计数超过末页就会与引擎漂移（高亮的页 ≠ 实际选词所在的页）。
 */
export function page(): number {
  return opiime.page();
}

export function pageCount(): number {
  return opiime.pageCount();
}

export function clear(): void {
  opiime.clear();
}

export function mode(): OpiMode {
  return toMode(opiime.mode());
}

/** 切模式。⚠️ 越界值是**静默不动作**，所以这里先夹紧到 0..4。 */
export function switchMode(m: OpiMode): void {
  const v: number = m as number;
  if (v < 0 || v > 4) {
    return;
  }
  opiime.switchMode(v);
}

/** **前端** ⇧ 三态。 */
export function shiftState(): OpiShift {
  const v: number = opiime.shiftState();
  if (v === 2) {
    return OpiShift.Lock;
  }
  if (v === 1) {
    return OpiShift.Single;
  }
  return OpiShift.Off;
}

/** 打**引擎侧** shift 位（与三态不是一回事：三态是前端状态）。 */
export function setShift(on: boolean): void {
  opiime.setShift(on);
}

/** 学习开关。关掉 = 不记词，小欧睡着（见 OpiPet.ets 的 petMood）。 */
export function learnerEnabled(): boolean {
  return opiime.learnerEnabled();
}

export function setLearner(on: boolean): void {
  opiime.setLearner(on);
}

// ---------------------------------------------------------------- 全角 / 符号开关
//
// ⚠️ 这三条在引擎里一直有，但**直到 2026-09-27 才进 C ABI** —— 在此之前任何客户端
// 都调不到（`grep -rn 'fullwidth\|toggle_symbol' crates/opi-ffi/src/` 当时为空）。
// 所以本端若有人自己抄过一份「按模式给全角默认值」的表，那是**对着一份到不了的语义**
// 抄的：删掉，改读 `fullwidthState()`。

/**
 * 切换全角，返回**切换后的新状态**（直接拿去刷高亮，不用再读一次）。
 *
 * ⚠️ 全角**随模式默认、跨模式不粘**（`Mode::default_fullwidth`：Pinyin|Traditional → 全角，
 * 其余 → 半角），而引擎的 `switch_mode` 是**无条件**按默认值重置
 * ⇒ 用户手动开的全角会被**任何一次模式切换**抹掉。
 * 这是**设计**（英文/数字必须半角直通），**别在 ArkTS 侧加一个 sticky 标志去「修」它** ——
 * 那会让英文模式出全角标点。
 */
export function toggleFullwidth(): boolean {
  return opiime.toggleFullwidth();
}

/**
 * 全角开关的**读侧** —— 只给状态栏用。
 *
 * ⚠️ **调完 `switchMode()` 或 `toggleSymbol()` 之后必须重读本出口。**
 * 后者最容易漏：它内部走了一次 `switch_mode`，而 Symbol 的默认是**半角**
 * ⇒ 按符号键时全角指示**悄悄灭掉**（不报错、不崩溃，指示就是错的）。
 *
 * ⚠️ **不许拿它去预测按键结果**：全角映射**不是 `(mode, fullwidth)` 的纯函数** ——
 * `'` 在缓冲非空时是**音节分隔符**（`xi'an`），同一个键在同一个模式下结果不同。
 * 键一律整颗交给 `keyEvent()`。
 */
export function fullwidthState(): boolean {
  return opiime.fullwidthState();
}

/**
 * 拼音 ⇄ 符号模式切换。
 *
 * ⚠️ 返回值**不是**「刚切出来的那个符号」，而是**切模式前那截缓冲的待提交文本**
 * （有候选 → 首候选；乱码缓冲如 `zzz` → 清掉且**不上屏** —— 把拼音原文塞进文档更糟）。
 * 空串 = 无提交；**非空串必须上屏**，否则用户丢字。
 *
 * ⚠️ 副作用不止一个：切模式 + 清缓冲 + 换候选 + 重置全角
 * ⇒ 调用后必须重读 **mode / buffer / candidates / fullwidth 四样**。
 *
 * 本端已有 `switchMode(OpiMode.Symbol)` 那条入口（模式键轮转），两者的差别**正是**上面这条
 * 「先把待提交的缓冲提交掉」—— `switchMode` 的路径是直接清掉缓冲
 * （见 `KeyboardController.switchMode` 先调 `clear()`，引擎侧 `Composer::switch_mode` 也清）。
 */
export function toggleSymbol(): string {
  return opiime.toggleSymbol();
}

/**
 * 键事件路由 —— **所有按键的唯一入口，可打印字符也走这里**。
 *
 * ⚠️ **不要**绕开它直接调 `opi_input_key`：模式分派（英文/数字/符号直传）、
 * ⇧ 三态的大小写、数字键选词、翻页钳制，全在引擎的路由表
 * （crates/engine-core/src/router.rs）里。绕过去的表现是
 * 「英文模式下打字母仍然进拼音缓冲」。
 *
 * @param keyval Unicode 码点（空格 0x20）或 KEY_* 常量
 * @param states 修饰位（STATE_* 常量），按下的字符大小写靠它
 */
export function keyEvent(keyval: number, states: number): KeyResult {
  const r = opiime.keyEvent(keyval, states);
  return new KeyResult(toAction(r.action), r.text);
}

/** 可打印字符 → keyEvent 的 keyval（就是码点本身）。 */
export function charKeyval(ch: string): number {
  return ch.charCodeAt(0);
}

/** 数字/符号模式下由调用方自己上屏用：码点 → 单字符。 */
export function charFromKeyval(keyval: number): string {
  return String.fromCharCode(keyval);
}
