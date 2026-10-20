// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 已过 API 19 的 hvigor CompileArkTS（2026-10-20）· 真机未验证
//
// 本文件在 Linux 上编写；编译过 ≠ 跑得起来。`@kit.IMEKit` 的
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
import { BusinessError } from '@kit.BasicServicesKit';
import { display } from '@kit.ArkUI';
import * as OpiEngine from './OpiEngine';
import { EmojiGroup, KeyResult, OpiAction, OpiMode, OpiShift } from './OpiEngine';
import * as DictionaryAssets from './DictionaryAssets';
// 输入模式：只做转发 + publish —— 状态机/九项表/数字真源都在 InputModes.ets，本文件不重编一份。
import * as InputModes from './InputModes';
import {
  KEY_BUFFER, KEY_CANDIDATES, KEY_PAGE, KEY_PAGE_COUNT, KEY_MODE, KEY_SHIFT, KEY_LEARNER,
  KEY_MODE_INDEX, KEY_PANEL
} from './KeyboardAppStorage';

/** 键盘占屏高的比例。凭感觉定的，真机上要按候选栏行数调。 */
const PANEL_HEIGHT_RATIO: number = 0.42;

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

  // off() 需要**同一个函数引用**才能真的退订（'inputStop' 的 SDK 签名甚至不给省略 callback）。
  private onInputStartCallback: (kbController: inputMethodEngine.KeyboardController,
    inputClient: inputMethodEngine.InputClient) => void =
    (kbController: inputMethodEngine.KeyboardController,
      inputClient: inputMethodEngine.InputClient): void => {
      this.kbController = kbController;
      this.client = inputClient;
      this.ensureDictionary();
      // 新会话：上一轮可能死在半路，先把引擎缓冲清干净，
      // 否则会带着上次的拼音进新输入框（「输入法错乱」的常见来源）。
      OpiEngine.clear();
      this.publish();
    };

  private onInputStopCallback: () => void = (): void => {
    // InputClient 随会话失效，必须丢引用；引擎侧缓冲也要清。
    OpiEngine.clear();
    this.client = null;
    this.kbController = null;
    this.publish();
  };

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

    // API 19 的 createPanel 只有异步形态（callback / Promise），没有同步返回值。
    // 用 callback 形态保持 init() 的同步签名（onCreate 不等 Promise）。
    ability.createPanel(ctx, info, (err: BusinessError, panel: inputMethodEngine.Panel) => {
      if (err) {
        console.error(`[opi] createPanel 失败: ${err.message}`);
        return;
      }
      panel.resize(d.width, panelH);
      panel.moveTo(0, d.height - panelH);
      // 路径是**相对 src/main/ets 的页面路径，不带 .ets 后缀**，且该页必须在
      // resources/base/profile/main_pages.json 里注册过 —— 两处任一处写错，
      // 表现是「面板起来了但是白的」。
      panel.setUiContent('pages/Index');
      this.panel = panel;
    });

    ability.on('inputStart', this.onInputStartCallback);
    ability.on('inputStop', this.onInputStopCallback);
  }

  release(): void {
    const ability = inputMethodEngine.getInputMethodAbility();
    ability.off('inputStart', this.onInputStartCallback);
    ability.off('inputStop', this.onInputStopCallback);
    if (this.panel !== null) {
      // destroyPanel 挂在 InputMethodAbility 上（不是 Panel 上），且是异步的。
      ability.destroyPanel(this.panel, (err: BusinessError) => {
        if (err) {
          console.error(`[opi] destroyPanel 失败: ${err.message}`);
        }
      });
      // 注意：只置空引用，不等 GC —— 面板是系统资源，必须显式 destroy。
      this.panel = null;
    }
    this.client = null;
    this.kbController = null;
    this.ctx = null;
    this.started = false;
  }

  // -------------------------------------------------------------- 词库

  /** true = 正用内置回退库（拷贝失败或完整库装载失败）；UI 据此画 DEGRADED。 */
  private dictDegraded: boolean = true;

  dictionaryDegraded(): boolean { return this.dictDegraded; }

  /** 装载词库（每进程一次）：rawfile 先由 DictionaryAssets 拷进 `ctx.filesDir`，再把绝对路径交给引擎。
   * 任一步失败都回退内置库并置 degraded。 */
  private ensureDictionary(): void {
    if (this.dictLoaded) { return; }
    this.dictLoaded = true;
    if (this.ctx === null) {
      console.error('[opi] 无扩展上下文，词库回退内置库');
      OpiEngine.loadDictionary('');
      return;
    }
    const files: DictionaryAssets.DictionaryFiles =
      DictionaryAssets.syncDictionaries(this.ctx.resourceManager, this.ctx.filesDir);
    let ok: boolean = files.luna.length > 0 && OpiEngine.loadDictionary(files.luna);
    if (!ok) {
      // 引擎契约：坏路径 → false 且引擎保持为空；不回退就一个字都不出。
      console.error('[opi] 完整词库不可用，回退内置词库');
      ok = OpiEngine.loadDictionary('');
    }
    this.dictDegraded = !(ok && files.luna.length > 0);
    InputModes.loadOptionalTables(files.stroke, files.wubi);
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
   * ⇧ 与引擎 shift 的同步归 `KeyRouter::switch_mode`（单一来源）：**离开** English
   * 清前端三态；**进入** English 时前端三态是唯一真值 —— SINGLE/LOCK 重推给引擎，
   * 首个字母大写、single 用掉即复位（四端同一条语义）。
   * ⚠️ 别在这里再 `setShift(false)`：那会把切换刚重推的引擎位又清掉（首个字母变小写）。
   */
  switchMode(m: OpiMode): void {
    OpiEngine.clear();
    OpiEngine.switchMode(m);
    this.publish();
  }

  /** 点小欧：学习开关取反。关掉 = 不记词 = 它睡着。 */
  toggleLearner(): void {
    OpiEngine.setLearner(!OpiEngine.learnerEnabled());
    this.publish();
  }

  // -------------------------------------------------------------- 输入模式（2026-10-14）

  /**
   * 选一项输入模式（下标见 InputModes.MODE_*）：换布局 + 方案（+ 语言模式）。
   * ⚠️ 下面四条转发后都 `publish()`：`setScheme` 会清缓冲，不刷新就是「点了没反应」。
   */
  selectInputMode(index: number): void {
    InputModes.selectInputMode(index);
    this.publish();
  }

  /** 候选栏的 ⌨：开/关九项面板。 */
  toggleModePanel(): void {
    InputModes.toggleModePanel();
    this.publish();
  }

  /** 面板里的「关闭」。 */
  closeModePanel(): void {
    InputModes.closeModePanel();
    this.publish();
  }

  /** 重输：清掉当前组合、**不提交**（`clear` 幂等）；已上屏的字不动。两端同款见 KeyPanels.ets。 */
  retype(): void {
    OpiEngine.clear();
    this.publish();
  }

  /** 当前 ⇧ 是否点亮（供按键高亮 + 送进 states）。 */
  private shiftStates(): number {
    return OpiEngine.shiftState() === OpiShift.Off ? 0 : OpiEngine.STATE_SHIFT;
  }

  // -------------------------------------------------------------- 表情页（2026-10-13）

  private emojiCache: string[] | null = null;
  private emojiSetCache: Set<string> | null = null;
  private emojiGroupsCache: EmojiGroup[] | null = null;

  /**
   * 表情浏览全量（官方分组序），懒取一次并缓存。
   *
   * ⚠️ **空结果不写进缓存** —— 与 Android `SymbolCatalog` 同一条纪律：空 = 「没拿到」，
   * 不是「真的没有」；缓存了就会**永远空到下次重启**。（这条出口不经引擎单例，正常
   * 冷启动第一帧就有；这里防的是模块没起来 / JSON 解码失败那一瞬。）
   */
  emojiSymbols(): string[] {
    if (this.emojiCache === null) {
      const list: string[] = OpiEngine.emojiSymbols();
      if (list.length > 0) {
        this.emojiCache = list;
      }
      return list;
    }
    return this.emojiCache;
  }

  /** 表情分组（分类条）。缓存纪律同 [emojiSymbols]。 */
  emojiGroups(): EmojiGroup[] {
    if (this.emojiGroupsCache === null) {
      const groups: EmojiGroup[] = OpiEngine.emojiGroups();
      if (groups.length > 0) {
        this.emojiGroupsCache = groups;
      }
      return groups;
    }
    return this.emojiGroupsCache;
  }

  /**
   * 表情页搜索：**引擎搜索 ∩ 表情集**。
   *
   * 求交的理由：`searchSymbols` 查的是**符号表**（含箭头、数学符号等非表情条目），
   * 而这一页是表情页 —— 与 Android 的 `results.filter(catalog::isEmoji)` 同一件事。
   * ⚠️ **每次输入都打一次引擎**（本端不缓存中间结果）：开销主要在 JSON 解码。真机上若
   * 发现打字卡，照 Android 的做法**按 query 缓存**（那边是 `remember(searchQuery)`）。
   */
  searchEmoji(q: string): string[] {
    const all: string[] = OpiEngine.searchSymbols(q);
    if (all.length === 0) {
      return [];
    }
    if (this.emojiSetCache === null) {
      // ⚠️ 未编译：`Set<string>` 在 ArkTS 里的可用性（与 EmojiPad.ets 的 `Array.from` 同一族，
      // 见 README 的 U23）；备选 = 每次线性扫表情全量（O(n) 换 O(1)，n≈4000，会明显变慢）。
      const set: Set<string> = new Set<string>();
      const emojis: string[] = this.emojiSymbols();
      for (let i = 0; i < emojis.length; i++) {
        set.add(emojis[i]);
      }
      // 表情集为空**不写缓存**（同上纪律：空 = 没拿到），下次重试。
      if (set.size > 0) {
        this.emojiSetCache = set;
      }
    }
    if (this.emojiSetCache === null) {
      // 表情集没拿到 ⇒ 交集为空。**别退化成「把非表情也搜出来」**。
      return [];
    }
    const out: string[] = [];
    for (let i = 0; i < all.length; i++) {
      if (this.emojiSetCache.has(all[i])) {
        out.push(all[i]);
      }
    }
    return out;
  }

  /**
   * 点一个表情：**直提**（`insertText`），**不过引擎** —— 与 Android
   * （`ImeScreen.kt` 的 `onCommit = router::commitText`）同一条纪律：引擎的标点表会把
   * `,` 改写成中文标点，而表情页点的是什么就该出什么。
   * ⚠️ **整串送**（旗帜/家庭是多码位序列）：不要拆成按键走 `pressKey`，那既不是它认识的
   * 样子，也会被当成多个键。
   */
  tapEmoji(text: string): void {
    this.insertText(text);
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
    // 这两条不来自引擎（模块级状态机）：未装载时读侧回 0/false 是哨兵，别拿它当 UI 档位。
    AppStorage.setOrCreate<number>(KEY_MODE_INDEX, InputModes.currentModeIndex());
    AppStorage.setOrCreate<number>(KEY_PANEL, InputModes.modePanelShown());
    this.syncPreviewText();
  }

  /**
   * 组合串推进**宿主输入框** —— 鸿蒙的原语是 **preview text**（预上屏）。
   *
   * 功能与 Android 同一条裁决（2026-10-10「打字期间输入框里能看到拼音」，以及
   * 2026-10-12 晚「待编的键必须看得见」）：本端此前只在**自家面板**的 preedit 里
   * 显示缓冲（`pages/Index.ets` 的 PreeditBar），宿主的输入框里一个字都没有 ——
   * 用户判为「没输入进去」。iOS 端的对应物是 marked text（见 `ios/KeyboardViewController.swift`
   * 的 `syncMarkedText`），鸿蒙这边就是这一对调用。
   *
   * API 19 的 d.ts 实测签名（2026-10-19 真编译后）：
   *   · `setPreviewTextSync(text, range: Range)` —— range 是**要替换的编辑框区间**，
   *     不是光标偏移量；本端用 `{ start: 0, end: 0 }`（光标处插入/更新）
   *   · `finishTextPreviewSync()` —— 缓冲清空时收尾
   * ⚠️ `{0,0}` 的语义只按 d.ts 选过，**没有真机证据**；见 docs/install/harmony.md §0.6。
   */
  private syncPreviewText(): void {
    if (this.client === null) {
      return;
    }
    const buf: string = OpiEngine.buffer();
    if (buf.length === 0) {
      this.client.finishTextPreviewSync();
      return;
    }
    // API 19 的 setPreviewText 收 Range（要替换的编辑框区间），不是光标偏移量。
    // 用 [0,0] 表示「在光标处插入 / 更新预上屏」。
    // shortcut: Range 的确切语义只在 d.ts 上核对过，未在真机验证；见 docs/install/harmony.md。
    const range: inputMethodEngine.Range = { start: 0, end: 0 };
    this.client.setPreviewTextSync(buf, range);
  }
}
