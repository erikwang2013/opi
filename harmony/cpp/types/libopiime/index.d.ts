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
// **逐条对齐**（名字一一对应，**两边都不写条数** —— 它随出口增删而漂：
// 核对命令见 napi_bridge.c「模块导出」那段）。

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

/**
 * 切换全角，返回**切换后的新状态**（UI 直接拿去刷高亮）。
 * ⚠️ 全角**随模式默认、跨模式不粘**：用户手动开的全角会被任何一次模式切换抹掉
 * （Pinyin|Traditional 全角，English|Number|Symbol 半角）—— 这是设计，别加 sticky 标志去「修」。
 */
export const toggleFullwidth: () => boolean;

/**
 * 全角开关的**读侧**，只喂状态栏。
 * ⚠️ 调完 `toggleSymbol` / `switchMode` 之后**必须重读** —— 两者都会重置全角，
 * 而 `toggleSymbol` 内部走 `switch_mode`（Symbol 默认半角）⇒ 按符号键时指示会悄悄灭掉。
 * ⚠️ **不要拿它预测按键结果**：映射不是 `(mode, fullwidth)` 的纯函数。
 */
export const fullwidthState: () => boolean;

/**
 * 拼音 ⇄ 符号模式切换，返回**切模式前那截缓冲的待提交文本**（空串 = 无提交）。
 * ⚠️ **不是**「刚切出来的那个符号」；乱码缓冲（`zzz`）会被清掉且不上屏。
 * ⚠️ 副作用不止一个 ⇒ 调用后要重读 **mode / buffer / candidates / fullwidth 四样**。
 * 拿不到「插入文本」通道的端不要调它。
 */
export const toggleSymbol: () => string;
