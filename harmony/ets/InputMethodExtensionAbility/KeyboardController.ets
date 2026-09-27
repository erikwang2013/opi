// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。`@kit.IMEKit` 的
// inputMethodEngine / InputMethodExtensionContext 在普通 tsc 下就断，
// 所以这里每一个 IME Kit 调用都是**凭记忆写的**（见 README「最不确定的 API」）。
//
// 职责：**输入法会话的唯一持有者**。做四件事 ——
//   1. 开软键盘面板（createPanel / resize / moveTo / setUiContent）；
//   2. 收 inputStart / inputStop，拿住 InputClient；
//   3. 把按键交给引擎，并按引擎的 action 决定「上屏 / 交还应用 / 什么都不做」；
//   4. 把引擎状态推给 ArkUI（AppStorage），UI 只读状态、不碰引擎。
//
// UI（pages/Index.ets）**不认识引擎**，只调本类的方法 + 读 AppStorage。

import { inputMethodEngine, InputMethodExtensionContext } from '@kit.IMEKit';
import { display } from '@kit.ArkUI';
import * as OpiEngine from './OpiEngine';
import { KeyResult, OpiAction, OpiMode, OpiShift } from './OpiEngine';

/** 键盘占屏高的比例。凭感觉定的，真机上要按候选栏行数调。 */
const PANEL_HEIGHT_RATIO: number = 0.42;

/**
 * AppStorage 键名 —— **写入侧**（下面的 `publish()`）用的唯一真源。
 *
 * ⚠️ 读取侧 `pages/Index.ets` **用不了这些常量**：@StorageLink 的入参必须是
 * 字符串字面量（ArkTS 编译期静态处理），写 `@StorageLink(KEY_BUFFER)` 编译不过。
 * 所以那边内联了字面量，两边**各有一份**，改这里必须同步改那边 —— 核对命令与
 * 症状写在 Index.ets 装饰器旁的注释里。**不要再试图「统一成常量」**：那正是
 * ArkTS 禁止的写法。
 */
export const KEY_BUFFER: string = 'opiBuffer';
export const KEY_CANDIDATES: string = 'opiCandidates';
export const KEY_PAGE: string = 'opiPage';
export const KEY_PAGE_COUNT: string = 'opiPageCount';
export const KEY_MODE: string = 'opiMode';
export const KEY_SHIFT: string = 'opiShift';
export const KEY_LEARNER: string = 'opiLearner';

export class KeyboardController {
  private static instance: KeyboardController = new KeyboardController();

  static getInstance(): KeyboardController {
    return KeyboardController.instance;
  }

  private ctx: InputMethodExtensionContext | null = null;
  private panel: inputMethodEngine.Panel | null = null;
  private client: inputMethodEngine.InputClient | null = null;
  private kbController: inputMethodEngine.KeyboardController | null = null;
  private started: boolean = false;
  private dictLoaded: boolean = false;

  // -------------------------------------------------------------- 生命周期

  /**
   * 由 InputMethodService.onCreate 调用一次。
   *
   * ⚠️ 这个实例会被系统按输入会话**反复创建与销毁**（输入法进程可被杀）。
   * 所以这里只做「重新拉起」成本很低的事；词库装载有 dictLoaded 挡重入，
   * 其余状态每次 inputStart 都重新读引擎。
   */
  init(ctx: InputMethodExtensionContext): void {
    if (this.started) {
      return;
    }
    this.started = true;
    this.ctx = ctx;

    const ability = inputMethodEngine.getInputMethodAbility();

    // 面板尺寸：屏宽 × 屏高的比例。display 拿到的单位是 px，不是 vp ——
    // 真机上如果键盘高度明显不对，先查这里需不需要 vp2px 换算（见 README）。
    const d = display.getDefaultDisplaySync();
    const panelH: number = Math.floor(d.height * PANEL_HEIGHT_RATIO);
    const info: inputMethodEngine.PanelInfo = {
      type: inputMethodEngine.PanelType.SOFT_KEYBOARD,
      flag: inputMethodEngine.PanelFlag.FLG_FIXED,
    };

    const panel = ability.createPanel(ctx, info);
    panel.resize(d.width, panelH);
    panel.moveTo(0, d.height - panelH);
    // 路径是**相对 src/main/ets 的页面路径，不带 .ets 后缀**，且该页必须在
    // resources/base/profile/main_pages.json 里注册过 —— 两处任一处写错，
    // 表现是「面板起来了但是白的」。
    panel.setUiContent('pages/Index');
    this.panel = panel;

    ability.on('inputStart', (kbController: inputMethodEngine.KeyboardController,
      inputClient: inputMethodEngine.InputClient) => {
      this.kbController = kbController;
      this.client = inputClient;
      this.ensureDictionary();
      // 新会话：上一轮可能死在半路，先把引擎缓冲清干净，
      // 否则会带着上次的拼音进新输入框（「输入法错乱」的常见来源）。
      OpiEngine.clear();
      this.publish();
    });

    ability.on('inputStop', () => {
      // InputClient 随会话失效，必须丢引用；引擎侧缓冲也要清。
      OpiEngine.clear();
      this.client = null;
      this.kbController = null;
      this.publish();
    });
  }

  release(): void {
    const ability = inputMethodEngine.getInputMethodAbility();
    ability.off('inputStart');
    ability.off('inputStop');
    if (this.panel !== null) {
      this.panel.destroyPanel();
      // 注意：只置空引用，不等 GC —— 面板是系统资源，必须显式 destroy。
      this.panel = null;
    }
    this.client = null;
    this.kbController = null;
    this.ctx = null;
    this.started = false;
  }

  // -------------------------------------------------------------- 词库

  /**
   * 装载词库。
   *
   * 现在传**空串 = 引擎内置回退词库** —— 这是有意的：让鸿蒙端第一次跑起来就
   * 能出字，不必先搞定资源拷贝。内置库词量小，但「能打字」比「词多」优先。
   *
   * 换完整词库要做的（**尚未实现**，见 README「构建集成」）：
   * rawfile 里的词库**不能直接把资源路径给引擎** —— 引擎要的是文件系统绝对路径。
   * 得先用 resourceManager 读 rawfile、用 fileIo 写进沙箱目录（如
   * `ctx.filesDir + '/opi_dict.bin'`），再把那个绝对路径传进来。
   * 只写了一次（existsSync 挡重入）。
   */
  private ensureDictionary(): void {
    if (this.dictLoaded) {
      return;
    }
    this.dictLoaded = true;
    const ok: boolean = OpiEngine.loadDictionary('');
    if (!ok) {
      // 只记日志。**不要在这里抛** —— 抛出去整个键盘就起不来了，
      // 而引擎未装载时所有出口都是安全的空操作，退化但不崩。
      console.error('[opi] 词库装载失败：引擎将退化为空操作（键盘不出字）');
    }
  }

  // -------------------------------------------------------------- 文本上屏

  private insertText(text: string): void {
    if (this.client === null || text.length === 0) {
      return;
    }
    this.client.insertText(text);
  }

  private deleteForward(n: number): void {
    if (this.client === null) {
      return;
    }
    this.client.deleteForward(n);
  }

  // -------------------------------------------------------------- 按键

  /**
   * **所有按键的唯一入口。**
   *
   * action 的三条语义（这是 C ABI 的契约，别在这里改）：
   *   2 Commit      → 把 text 上屏
   *   1 Consumed    → 引擎吃掉了，**什么都不要做**（字已经进它内部了，还没上屏）
   *   0 PassThrough → **引擎放行了，这一下归应用**（见 handlePassThrough）
   */
  pressKey(keyval: number, states: number): void {
    const r: KeyResult = OpiEngine.keyEvent(keyval, states);
    if (r.action === OpiAction.Commit) {
      this.insertText(r.text);
    } else if (r.action === OpiAction.PassThrough) {
      this.handlePassThrough(keyval);
    }
    this.publish();
  }

  /**
   * 引擎放行的键 —— **这一下由应用承接**。
   *
   * ⚠️ 最容易写错的一条：退格。引擎在**缓冲为空**时故意放行退格，意思是
   * 「我这儿没东西可删了，去删应用里已上屏的字」。如果这里无条件
   * `deleteForward(1)`，就等于吞掉了引擎的裁决 —— 表现是「删不掉已经上屏的字」。
   * 反过来，缓冲非空时引擎会返回 Consumed，根本不会走到这里。
   */
  private handlePassThrough(keyval: number): void {
    if (keyval === OpiEngine.KEY_BACK_SPACE || keyval === OpiEngine.KEY_DELETE) {
      this.deleteForward(1);
      return;
    }
    // 数字/符号模式的可打印键走这里：引擎不管这些模式，UI 自己上屏。
    // 上界 0xFFFF 同时把特殊键（0x1_00xx）挡在外面 —— 它们不是字符。
    if (keyval >= 0x20 && keyval <= 0xFFFF) {
      this.insertText(String.fromCharCode(keyval));
    }
    // Tab / Esc / 方向键 / 翻页越界：软键盘上没有这些键，不处理。
    // ⚠️ **回车也落到这里**（缓冲为空时引擎放行它），而上面两个分支都不接
    //    `KEY_RETURN`（`0x1_000D` > 0xFFFF）—— 所以空缓冲下的 ↵ 是死键。
    //    原因与替代做法写在 `tapEnter` 的注释里；查证到接口后在这里加分支。
  }

  /** 面板上的字符键。 */
  tapChar(ch: string): void {
    this.pressKey(OpiEngine.charKeyval(ch), this.shiftStates());
  }

  /** 空格 —— 是**可打印段**的 0x20，走路由（缓冲非空时提交候选）。 */
  tapSpace(): void {
    this.pressKey(OpiEngine.KEY_SPACE, this.shiftStates());
  }

  tapBackspace(): void {
    this.pressKey(OpiEngine.KEY_BACK_SPACE, 0);
  }

  /**
   * 回车。
   *
   * ✅ 缓冲非空时**是好的**：引擎返回 action=2 + 首候选，走 `insertText` 上屏。
   *
   * ⚠️ **缓冲为空时这个键是死的**（已知缺口，不是 bug —— 但也没人接）：
   * 引擎在缓冲为空时返回 **PassThrough**（`router.rs` 的 `handle_enter`：
   * 「空缓冲 → 直通」，意思是「这个回车归应用」），而 `handlePassThrough`
   * 只认退格/Delete 与可打印段 —— `KEY_RETURN` = `0x1_000D` 大于 `0xFFFF`，
   * 两个分支都不进，于是**什么都不发生**。用户按 ↵ 没反应。
   *
   * 为什么本文件没顺手补上：把回车交还给应用需要 IME Kit 的「送按键给应用」接口
   * （Android 侧对应 `InputConnection.performEditorAction`，见
   * `android/.../ime/KeyRouter.kt` 的 `performEnter`），而**那个接口在鸿蒙叫什么、
   * 本机无法查证**（没有 SDK）。凭记忆写一个 API 名进来，正是本目录要被避免的
   * 「没被编译器看过的代码」。
   *
   * 在 DevEco 上的做法：查到正确的接口后，在 `handlePassThrough` 里给
   * `KEY_RETURN` 加一条分支，**不要**退化成 `insertText('\n')` 了事 ——
   * 单行输入框里插换行符不等于提交（也可能被应用直接忽略），
   * 表现是「按了 ↵ 什么都不发生」换个形式再来一次。
   */
  tapEnter(): void {
    this.pressKey(OpiEngine.KEY_RETURN, 0);
  }

  /**
   * ⇧ 单击。三态由**引擎**的状态机走，UI 不自己数。
   *
   * ⚠️ 单击只在 **OFF ↔ SINGLE** 之间来回（`router.rs` 的 `shift_tap`：
   * `Off → Single`，其余一律回 `Off`）。**进不了 LOCK** —— 见 tapShiftLongPress。
   * 本注释原先写的是「OFF→SINGLE→LOCK 由引擎的状态机走」，那句话是错的：
   * 引擎能走三态，但第三态得靠**长按**那个入口，光按这个方法是到不了的。
   */
  tapShift(): void {
    this.pressKey(OpiEngine.KEY_SHIFT, 0);
  }

  /**
   * ⇧ 长按 → **LOCK**（持续大写）。⇧ 进 Lock 的唯一途径。
   *
   * 引擎侧对应 `router.rs` 的 `shift_long_press()`，判据是 states 里的
   * `KEY_STATE_LONG_PRESSED` 位 —— 缺了这个位，`OpiShift.Lock` 就是永远到不了的
   * 枚举值（UI 里有 `⇪` 分支、用户怎么按都出不来）。`OpiEngine.ets` 原先漏抄了
   * 这个位，本轮补上。
   *
   * 传 states 时**不要**再带 STATE_REPEAT：`handle_shift` 把 released 与 repeat
   * 一起忽略，带了这一下就白按了。
   */
  tapShiftLongPress(): void {
    this.pressKey(OpiEngine.KEY_SHIFT, OpiEngine.STATE_LONG_PRESSED);
  }

  tapPageUp(): void {
    this.pressKey(OpiEngine.KEY_PAGE_UP, 0);
  }

  tapPageDown(): void {
    this.pressKey(OpiEngine.KEY_PAGE_DOWN, 0);
  }

  /** 点候选：**页内索引**，引擎自己换算。不要自己算 page*8+k。 */
  tapCandidate(indexInPage: number): void {
    const text: string = OpiEngine.selectPage(indexInPage);
    this.insertText(text);
    this.publish();
  }

  /**
   * 切模式（中/英/数字/符号）。切之前会清空缓冲，避免带着半截拼音换键盘。
   *
   * ⚠️ `setShift(false)` 打的是**引擎侧** shift 位，**不动前端的 ⇧ 三态**
   * （`cabi.rs` 的 `opi_set_shift` → `router.set_shift` 只写引擎位）。
   * 所以这一行**不是**「切模式清 ⇧」—— 真正清三态的是引擎自己：
   * `KeyRouter::switch_mode` 在**离开 English** 时把三态置 `Off`
   * （判据与 Android `EngineController.switchMode` 的 `if (m != ENGLISH)` 逐字一致）。
   * 留在这一行的作用只是保证「切进 English 时引擎位不与前端三态打架」。
   * **别把它当成三态重置去依赖**：从拼音（⇧ 已点亮）切进英文时，
   * 三态**按设计保留**，于是第一个字母大写 —— 那是 ⇧ Single 的正常行为，不是 bug。
   */
  switchMode(m: OpiMode): void {
    OpiEngine.clear();
    OpiEngine.switchMode(m);
    OpiEngine.setShift(false);
    this.publish();
  }

  /** 点小欧：学习开关取反。关掉 = 不记词 = 它睡着。 */
  toggleLearner(): void {
    OpiEngine.setLearner(!OpiEngine.learnerEnabled());
    this.publish();
  }

  /** 当前 ⇧ 是否点亮（供按键高亮 + 送进 states）。 */
  private shiftStates(): number {
    return OpiEngine.shiftState() === OpiShift.Off ? 0 : OpiEngine.STATE_SHIFT;
  }

  // -------------------------------------------------------------- 状态推送

  /**
   * 把引擎状态推给 UI。
   * UI 不直接调引擎 —— 单一数据流向，省得两边各自缓存、各自漂移。
   */
  publish(): void {
    AppStorage.setOrCreate<string>(KEY_BUFFER, OpiEngine.buffer());
    AppStorage.setOrCreate<string[]>(KEY_CANDIDATES, OpiEngine.candidatesPage());
    AppStorage.setOrCreate<number>(KEY_PAGE, OpiEngine.page());
    AppStorage.setOrCreate<number>(KEY_PAGE_COUNT, OpiEngine.pageCount());
    AppStorage.setOrCreate<number>(KEY_MODE, OpiEngine.mode() as number);
    AppStorage.setOrCreate<number>(KEY_SHIFT, OpiEngine.shiftState() as number);
    AppStorage.setOrCreate<number>(KEY_LEARNER, OpiEngine.learnerEnabled() ? 1 : 0);
  }
}
