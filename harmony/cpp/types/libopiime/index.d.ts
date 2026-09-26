// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。
// 这是 libopiime.so（cpp/napi_bridge.c）的 ArkTS 侧类型声明。
// ArkTS 靠它知道 `import opiime from 'libopiime.so'` 上有哪些方法；
// **声明少一个方法**的表现是运行期 `xxx is not a function`，
// 而**声明多一个**不存在的，要在真机上打到那条路径才炸。
// 所以这里的每一条都必须与 cpp/napi_bridge.c 的 napi_property_descriptor 表
// **逐条对齐**（现在两边各 15 条，名字一一对应）。

/// keyEvent 的返回。action 的取值同 C ABI 的 OpiKeyEventResult.action。
export interface OpiKeyEventResult {
  /**
   * 0 = 未处理：**调用方自己把键输出去**（例如缓冲为空时的退格 → deleteForward；
   *     数字/符号模式的可打印键 → 自己上屏该字符）
   * 1 = 已消费：引擎吃掉了这次按键，调用方**什么都不要做**
   * 2 = 提交 text：把 text 上屏
   */
  action: number;
  /** action === 2 时才有内容；其余情况为空串。 */
  text: string;
}

/**
 * 装载词库。缺参/空串 → 内置回退词库；坏路径 → false。
 * **返回值必须接**：false 时后面所有出口都退化成空操作（键盘看着正常但不出字）。
 */
export const load: (path?: string) => boolean;

/** 单字符入引擎，返回需要上屏的文本（多数时候是空串）。 */
export const inputKey: (ch: string) => string;

/**
 * 键事件路由 —— **所有按键（含可打印字符）都走这里**。
 * 可打印字符传 Unicode 码点（空格 = 0x20）；特殊键见 OpiEngine.ets 的 KEY_* 常量。
 * states 是修饰位（SHIFT/RELEASED 等）。
 */
export const keyEvent: (keyval: number, states: number) => OpiKeyEventResult;

/** 提交**当前页**第 k 个候选（页内索引，0 起）。越界/无候选 → 空串。 */
export const selectPage: (index: number) => string;

/** JSON 文本数组：**当前页**候选。未装载 → `[]`。 */
export const candidatesPage: () => string;

/** 当前拼音缓冲（preedit 来源）。 */
export const buffer: () => string;

/** 当前候选页（0 起）。**不要自己数页** —— 末页由路由钳制。 */
export const page: () => number;

/** 候选总页数。无候选 → 0。 */
export const pageCount: () => number;

/** 清空缓冲。 */
export const clear: () => void;

/** 0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional。 */
export const mode: () => number;

/** ⚠️ 越界值是**静默不动作**（无返回值），传之前自己夹紧到 0..4。 */
export const switchMode: (mode: number) => void;

/** **前端** ⇧ 三态：0=OFF 1=SINGLE 2=LOCK。⇧ 高亮只能读这里。 */
export const shiftState: () => number;

/** 打**引擎侧** shift 位（与三态不是一回事）。 */
export const setShift: (on: boolean) => void;

/** 学习开关。关掉 = 不记词，小欧睡着。 */
export const learnerEnabled: () => boolean;
export const setLearner: (on: boolean) => void;
