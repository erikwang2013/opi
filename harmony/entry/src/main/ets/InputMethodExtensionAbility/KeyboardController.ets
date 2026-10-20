// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 已过 API 19 的 hvigor CompileArkTS（2026-10-20）· 真机未验证
//
// 输入法会话的唯一持有者：面板窗口 / InputClient / 引擎路由 / AppStorage 推送。
// UI（pages/Index.ets）不认识引擎，只调本类的方法 + 读 AppStorage。

import { inputMethodEngine, InputMethodExtensionContext } from '@kit.IMEKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { display } from '@kit.ArkUI';
import * as OpiEngine from './OpiEngine';
import { KeyResult, OpiAction, OpiMode, OpiShift } from './OpiEngine';
import * as DictionaryAssets from './DictionaryAssets';
// 输入模式：只做转发 + publish —— 状态机/九项表/数字真源都在 InputModes.ets，本文件不重编一份。
import * as InputModes from './InputModes';
import * as KeyboardModes from './KeyboardModes';
import { PanelView } from './KeyboardModes';
import * as RawAssets from './RawAssets';
import {
  KEY_BUFFER, KEY_CANDIDATES, KEY_PAGE, KEY_PAGE_COUNT, KEY_MODE, KEY_SHIFT, KEY_LEARNER,
  KEY_MODE_INDEX, KEY_VIEW, KEY_SKIN, KEY_DARK, KEY_ELDER, KEY_ONE_HAND, KEY_FLOAT, KEY_GAME, KEY_PRO
} from './KeyboardAppStorage';

/** 键盘占屏高的比例。凭感觉定的，真机上要按候选栏行数调。 */
const PANEL_HEIGHT_RATIO: number = 0.42;
/** 悬浮/游戏窗口宽度比例（Android OpiImeService 同值）。 */
const FLOATING_WIDTH_RATIO: number = 0.86;
/** 游戏模式再压到常规高度的这个比例（Android 同值）。 */
const GAME_HEIGHT_RATIO: number = 0.72;

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
  // 面板窗口的基准尺寸（px）；悬浮/游戏/单手切换时重算。
  private screenW: number = 0;
  private screenH: number = 0;

  // off() 需要**同一个函数引用**才能真的退订（'inputStop' 的 SDK 签名甚至不给省略 callback）。
  private onInputStartCallback: (kbController: inputMethodEngine.KeyboardController,
    inputClient: inputMethodEngine.InputClient) => void =
    (kbController: inputMethodEngine.KeyboardController,
      inputClient: inputMethodEngine.InputClient): void => {
      this.kbController = kbController;
      this.client = inputClient;
      this.ensureDictionary();
      this.applyPanelWindow();
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

  /** InputMethodService.onCreate 调用；进程可能被反复拉起，靠 started 挡重入。 */
  init(ctx: InputMethodExtensionContext): void {
    if (this.started) {
      return;
    }
    this.started = true;
    this.ctx = ctx;
    RawAssets.bindResourceManager(ctx.resourceManager);

    const ability = inputMethodEngine.getInputMethodAbility();

    // 面板尺寸：屏宽 × 屏高的比例。display 拿到的单位是 px，不是 vp ——
    // 真机上如果键盘高度明显不对，先查这里需不需要 vp2px 换算（见 README）。
    const d = display.getDefaultDisplaySync();
    this.screenW = d.width;
    this.screenH = d.height;
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
      this.panel = panel;
      this.applyPanelWindow();
      // 路径是**相对 src/main/ets 的页面路径，不带 .ets 后缀**，且该页必须在
      // resources/base/profile/main_pages.json 里注册过 —— 两处任一处写错，
      // 表现是「面板起来了但是白的」。
      panel.setUiContent('pages/Index');
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

  /** 所有按键的唯一入口（action：2 上屏 / 1 引擎已吃 / 0 交还应用）。 */
  pressKey(keyval: number, states: number): void {
    const r: KeyResult = OpiEngine.keyEvent(keyval, states);
    if (r.action === OpiAction.Commit) {
      this.insertText(r.text);
    } else if (r.action === OpiAction.PassThrough) {
      this.handlePassThrough(keyval);
    }
    this.publish();
  }

  /** 引擎放行的键 = 由应用承接。空缓冲的退格必须真的删应用里的字。 */
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
    // 空缓冲的 ↵ 也落到这里：KEY_RETURN > 0xFFFF，两个分支都不接 ⇒ 已知死键（见 tapEnter）。
  }

  /** 面板上的字符键。英文 pro 关时直提（不经过引擎），其余走统一路由。 */
  tapChar(ch: string): void {
    if (this.englishDirect()) {
      this.insertText(OpiEngine.shiftState() === OpiShift.Off ? ch : ch.toUpperCase());
      if (OpiEngine.shiftState() === OpiShift.Single) {
        OpiEngine.keyEvent(OpiEngine.KEY_SHIFT, 0); // single 用掉即复位
      }
      this.publish();
      return;
    }
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
   * 回车：缓冲非空时提交首候选；空缓冲是已知缺口（需 IME Kit 的「送键给应用」接口，
   * 本机 SDK 未查证）。别退化成 insertText('\n')——单行框里不等于提交。
   */
  tapEnter(): void {
    this.pressKey(OpiEngine.KEY_RETURN, 0);
  }

  /** ⇧ 单击：引擎三态只在 OFF ↔ SINGLE 间走；LOCK 只能长按（见 tapShiftLongPress）。 */
  tapShift(): void {
    this.pressKey(OpiEngine.KEY_SHIFT, 0);
  }

  /** ⇧ 长按 → LOCK（持续大写）。states 带 LONG_PRESSED，别带 REPEAT（会被忽略）。 */
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
    if (m !== OpiMode.English) {
      KeyboardModes.leaveEnglishPro();
    }
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
    if (index === InputModes.MODE_RARE) {
      // 生僻字是面板视图（不进引擎、不改键位）。
      KeyboardModes.openPanel(PanelView.Rare);
      this.publish();
      return;
    }
    InputModes.selectInputMode(index);
    KeyboardModes.closePanel(); // 选中即回键面（Android selectInputMode 同款）
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

  // -------------------------------------------------------------- 工具条 / 面板（2026-10-20）

  /** 面板窗口：常规全宽贴底；悬浮/游戏收窄居中，游戏再压高度（Android OpiImeService 同值）。 */
  private applyPanelWindow(): void {
    if (this.panel === null || this.screenW === 0) {
      return;
    }
    let w: number = this.screenW;
    let h: number = Math.floor(this.screenH * PANEL_HEIGHT_RATIO);
    if (KeyboardModes.gameEnabled()) {
      h = Math.floor(h * GAME_HEIGHT_RATIO);
    }
    if (KeyboardModes.floatingEnabled() || KeyboardModes.gameEnabled()) {
      w = Math.floor(this.screenW * FLOATING_WIDTH_RATIO);
    }
    this.panel.resize(w, h);
    this.panel.moveTo(Math.floor((this.screenW - w) / 2), this.screenH - h);
  }

  togglePanel(v: PanelView): void {
    KeyboardModes.togglePanel(v);
    this.publish();
  }

  closePanel(): void {
    KeyboardModes.closePanel();
    this.publish();
  }

  selectSkin(i: number): void {
    KeyboardModes.setSkinValue(i);
    this.publish();
  }

  toggleDark(): void { KeyboardModes.toggleDark(); this.publish(); }

  toggleElder(): void { KeyboardModes.toggleElder(); this.publish(); }

  setOneHand(v: number): void { KeyboardModes.setOneHandValue(v); this.publish(); }

  toggleFloating(): void {
    KeyboardModes.toggleFloating();
    this.applyPanelWindow();
    this.publish();
  }

  toggleGame(): void {
    KeyboardModes.toggleGame();
    this.applyPanelWindow();
    this.publish();
  }

  /** 收起键盘（会话还在；下次 inputStart 仍是同一扩展）。 */
  hideKeyboard(): void {
    if (this.kbController === null) {
      return;
    }
    this.kbController.hide().catch((err: BusinessError) => {
      console.error(`[opi] 收起键盘失败: ${err.message}`);
    });
  }

  /** 符号页（符/?123）与生僻字面板：面板视图，不碰引擎。 */
  openSymbolPage(): void {
    KeyboardModes.openPanel(PanelView.SymbolPage);
    this.publish();
  }

  openRare(): void {
    KeyboardModes.openPanel(PanelView.Rare);
    this.publish();
  }

  /** 表情页：引擎 Symbol 模式；记住来路，返回时切回（Android 表情是视图，不换引擎模式）。 */
  openEmoji(): void {
    KeyboardModes.setEmojiReturn(OpiEngine.mode());
    OpiEngine.clear();
    OpiEngine.switchMode(OpiMode.Symbol);
    KeyboardModes.closePanel();
    this.publish();
  }

  exitEmoji(): void {
    OpiEngine.clear();
    OpiEngine.switchMode(KeyboardModes.emojiReturn());
    this.publish();
  }

  /** 符号/生僻字/表情直提：整串送，不拆键（旗帜/家庭是多码位序列）。 */
  commitText(text: string): void {
    this.insertText(text);
  }

  /** 英文 pro：关掉时清掉引擎里可能残留的缓冲（此后字母直接上屏）。 */
  toggleEnglishPro(): void {
    KeyboardModes.toggleEnglishPro();
    if (!KeyboardModes.englishProEnabled()) {
      OpiEngine.clear();
    }
    this.publish();
  }

  /** 9/14/笔画底行：只切简繁（Android 2026-10-15 裁决）。 */
  toggleTraditional(): void {
    this.switchMode(OpiEngine.mode() === OpiMode.Traditional ? OpiMode.Pinyin : OpiMode.Traditional);
  }

  /** 数字面板 ABC：回到上次的中文键位（不是固定的拼音26键）。 */
  cycleToChinese(): void {
    this.selectInputMode(InputModes.lastChineseModeIndex());
  }

  /** 26 键模式键：中→繁→英→上次中文（Android cycleMode 的次序）。 */
  cycleMode(): void {
    const m: OpiMode = OpiEngine.mode();
    if (m === OpiMode.Pinyin) {
      this.switchMode(OpiMode.Traditional);
    } else if (m === OpiMode.Traditional) {
      this.switchMode(OpiMode.English);
    } else {
      this.switchMode(InputModes.INPUT_MODES[InputModes.lastChineseModeIndex()].mode);
    }
  }

  /** 英文 pro 关 = 字母直提（不经过引擎，Android 的直接输出档）。 */
  private englishDirect(): boolean {
    return OpiEngine.mode() === OpiMode.English && !KeyboardModes.englishProEnabled();
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
    AppStorage.setOrCreate<number>(KEY_VIEW, KeyboardModes.panelView());
    AppStorage.setOrCreate<number>(KEY_SKIN, KeyboardModes.skinValue());
    AppStorage.setOrCreate<number>(KEY_DARK, KeyboardModes.darkEnabled() ? 1 : 0);
    AppStorage.setOrCreate<number>(KEY_ELDER, KeyboardModes.elderEnabled() ? 1 : 0);
    AppStorage.setOrCreate<number>(KEY_ONE_HAND, KeyboardModes.oneHandValue());
    AppStorage.setOrCreate<number>(KEY_FLOAT, KeyboardModes.floatingEnabled() ? 1 : 0);
    AppStorage.setOrCreate<number>(KEY_GAME, KeyboardModes.gameEnabled() ? 1 : 0);
    AppStorage.setOrCreate<number>(KEY_PRO, KeyboardModes.englishProEnabled() ? 1 : 0);
    this.syncPreviewText();
  }

  /**
   * 组合串推进宿主输入框（preview text）。`{0,0}` 只按 API 19 d.ts 选过，
   * 无真机证据；见 docs/install/harmony.md §0.6。
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
