// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
// 已过 API 19 的 hvigor CompileArkTS（2026-10-20）· 真机未验证
// 输入法会话的唯一持有者：面板窗口 / InputClient / 引擎路由 / AppStorage 推送。
// UI（pages/Index.ets）不认识引擎，只调本类的方法 + 读 AppStorage。

import { inputMethodEngine, InputMethodExtensionContext } from '@kit.IMEKit';
import { BusinessError } from '@kit.BasicServicesKit';
import * as OpiEngine from './OpiEngine';
import { KeyResult, OpiAction, OpiMode, OpiShift } from './OpiEngine';
import * as DictionaryAssets from './DictionaryAssets';
// 输入模式：只做转发 + publish —— 状态机/九项表/数字真源都在 InputModes.ets，本文件不重编一份。
import * as InputModes from './InputModes';
import * as KeyboardModes from './KeyboardModes';
import { PanelView } from './KeyboardModes';
import * as RawAssets from './RawAssets';
import * as ScreenLayout from './ScreenLayout';
import * as SettingsStore from './SettingsStore';
import { publishState } from './StatePublish';
import * as BackspaceRepeat from './BackspaceRepeat';

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
  /** 面板窗口的基准尺寸（px，随旋转/折叠刷新）；悬浮/游戏切换时用同一份重算。 */
  private metrics: ScreenLayout.ScreenMetrics = new ScreenLayout.ScreenMetrics();
  /** 偏好里的学习开关，等词库装载后再推给引擎（引擎未装载时写侧会被丢）。 */
  private pendingLearner: boolean = true;

  // off() 需要**同一个函数引用**才能真的退订（'inputStop' 的 SDK 签名甚至不给省略 callback）。
  private onInputStartCallback: (kbController: inputMethodEngine.KeyboardController,
    inputClient: inputMethodEngine.InputClient) => void =
    (kbController: inputMethodEngine.KeyboardController,
      inputClient: inputMethodEngine.InputClient): void => {
      this.kbController = kbController;
      this.client = inputClient;
      // 无会话期间可能转过屏/折叠过：新会话先重读尺寸再 resize。
      this.metrics = ScreenLayout.currentMetrics();
      this.ensureDictionary();
      this.applyPanelWindow();
      // 新会话：上一轮可能死在半路，先清引擎缓冲，否则会带着上次的拼音进新输入框。
      OpiEngine.clear();
      this.publish();
    };

  /** 旋转 / 折叠 / 分屏：重读短边尺寸，按同一公式重算窗口。 */
  private onDisplayChangeCallback: (displayId: number) => void = (displayId: number): void => {
    this.metrics = ScreenLayout.currentMetrics();
    this.applyPanelWindow();
  };

  private onInputStopCallback: () => void = (): void => {
    // InputClient 随会话失效，必须丢引用；引擎侧缓冲要清，长按连删计时器也要停。
    BackspaceRepeat.stop();
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

    // 偏好先于一切：模块级键盘模式/输入模式在 init 时装回来（词库装载后再推给引擎）。
    SettingsStore.init(ctx);
    const saved: SettingsStore.ImeSettings = SettingsStore.load();
    KeyboardModes.restore(saved);
    InputModes.restore(saved.inputModeIndex, saved.lastChineseIndex);
    this.pendingLearner = saved.learn;

    const ability = inputMethodEngine.getInputMethodAbility();

    this.metrics = ScreenLayout.currentMetrics();
    ScreenLayout.watchDisplay(this.onDisplayChangeCallback);
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
      // 路径相对 src/main/ets、不带 .ets，且须在 main_pages.json 注册；错一处 = 白屏。
      panel.setUiContent('pages/Index');
    });

    ability.on('inputStart', this.onInputStartCallback);
    ability.on('inputStop', this.onInputStopCallback);
  }

  release(): void {
    const ability = inputMethodEngine.getInputMethodAbility();
    ability.off('inputStart', this.onInputStartCallback);
    ability.off('inputStop', this.onInputStopCallback);
    ScreenLayout.unwatchDisplay(this.onDisplayChangeCallback);
    if (this.panel !== null) {
      // destroyPanel 挂在 InputMethodAbility 上（不是 Panel 上），且是异步的。
      ability.destroyPanel(this.panel, (err: BusinessError) => {
        if (err) {
          console.error(`[opi] destroyPanel 失败: ${err.message}`);
        }
      });
      this.panel = null; // 面板是系统资源，必须显式 destroy，不能只等 GC。
    }
    BackspaceRepeat.stop();
    this.client = null;
    this.kbController = null;
    this.ctx = null;
    this.started = false;
  }

  // -------------------------------------------------------------- 词库

  /** true = 正用内置回退库（拷贝失败或完整库装载失败）；UI 据此画 DEGRADED。 */
  private dictDegraded: boolean = true;

  dictionaryDegraded(): boolean { return this.dictDegraded; }

  /** 装载词库（每进程一次）：文件拷进 `ctx.filesDir` 后把绝对路径交给引擎；任一步失败回退内置库。 */
  private ensureDictionary(): void {
    if (this.dictLoaded) { return; }
    this.dictLoaded = true;
    if (this.ctx === null) {
      console.error('[opi] 无扩展上下文，词库回退内置库');
      OpiEngine.loadDictionary('');
    } else {
      const files: DictionaryAssets.DictionaryFiles =
        DictionaryAssets.syncDictionaries(this.ctx.resourceManager, this.ctx.filesDir);
      let ok: boolean = files.luna.length > 0 && OpiEngine.loadDictionary(files.luna);
      if (!ok) {
        // 引擎契约：坏路径 → false 且引擎保持为空；不回退就一个字都不出。
        console.error('[opi] 完整词库不可用，回退内置词库');
        ok = OpiEngine.loadDictionary('');
      }
      this.dictDegraded = !(ok && files.luna.length > 0);
      // 繁体是**可选增强**（Android EngineLoader 同一条）：失败不碰已装的 luna，
      // 繁体模式回退查简库 —— 所以不参与 degraded 判定。
      if (files.trad.length > 0 && !OpiEngine.loadTradDictionary(files.trad)) {
        console.warn('[opi] trad.opid 未装载：繁体模式回退简体词库');
      }
      InputModes.loadOptionalTables(files.stroke, files.wubi);
    }
    // 引擎就绪后才推：学习开关 + 持久化的输入档位（scheme/mode）。
    OpiEngine.setLearner(this.pendingLearner);
    InputModes.applyCurrentMode();
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

  /** 9 键「分词」：有拼音插 `'`，没拼音打「1」（2026-10-23 用户订正）。 */
  tapSegment(): void {
    if (OpiEngine.buffer().length === 0) { this.commitText('1'); return; }
    this.tapChar('\'');
  }

  /** 9 键左列点读音 = 钉住/解除（空串解除）；只改引擎过滤，不上屏。 */
  pinReading(text: string): void {
    OpiEngine.pinReading(text);
    this.publish();
  }

  /** 缓冲变化时静默解除旧钉（外层 publish 已按新缓冲算过候选，不要再嵌套 publish）。 */
  clearPinnedReading(): void {
    OpiEngine.pinReading('');
  }

  /** 空格 —— 是**可打印段**的 0x20，走路由（缓冲非空时提交候选）。 */
  tapSpace(): void {
    this.pressKey(OpiEngine.KEY_SPACE, this.shiftStates());
  }

  tapBackspace(): void { this.pressKey(OpiEngine.KEY_BACK_SPACE, 0); }
  /** 长按 ⌫：true = 开始连续退格（先删一格，之后每 60ms 一格）；false = 松手/取消停止。 */
  setBackspaceRepeat(on: boolean): void {
    if (on) {
      BackspaceRepeat.start(() => {
        this.tapBackspace();
      });
    } else {
      BackspaceRepeat.stop();
    }
  }

  /** 回车：缓冲非空提交首候选；空缓冲是已知缺口（缺 IME Kit 送键接口，别退化成上屏 '\n'）。 */
  tapEnter(): void {
    this.pressKey(OpiEngine.KEY_RETURN, 0);
  }

  /** ⇧ 单击：引擎三态只在 OFF ↔ SINGLE 间走；LOCK 只能长按（见 tapShiftLongPress）。 */
  tapShift(): void { this.pressKey(OpiEngine.KEY_SHIFT, 0); }

  /** ⇧ 长按 → LOCK（持续大写）。states 带 LONG_PRESSED，别带 REPEAT（会被忽略）。 */
  tapShiftLongPress(): void { this.pressKey(OpiEngine.KEY_SHIFT, OpiEngine.STATE_LONG_PRESSED); }

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

  /** 平铺候选：**全局**下标（`opiAllCandidates` 数组下标），走 opi_select。 */
  tapTiledCandidate(index: number): void {
    const text: string = OpiEngine.selectGlobal(index);
    this.insertText(text);
    this.publish();
  }

  /** 长按候选 = 从用户词表删词（Android CandidateBar onLongPress 同款；不存在则无操作）。 */
  removeCandidateWord(text: string): void {
    OpiEngine.removeUserWord(text);
    this.publish();
  }

  /** 清除全部用户词（设置页确认后调用）：只清引擎学习表（本端无用户词落盘出口），随后刷新候选。 */
  clearUserWords(): void {
    OpiEngine.clearUserWords();
    this.publish();
  }

  /** 候选栏 ⌄：展开平铺视图（只在有候选时由 UI 提供入口）。 */
  expandTiled(): void {
    KeyboardModes.expandTiled();
    this.publish();
  }

  /** 平铺「返回」：回键盘，**不动引擎、不清缓冲**。 */
  collapseTiled(): void {
    KeyboardModes.collapseTiled();
    this.publish();
  }

  /** 切模式：先清缓冲。⇧ 与引擎 shift 的同步归 `KeyRouter::switch_mode`（单一来源：
   *  离开 English 清前端三态，进入 English 时重推；single 用掉即复位）。
   *  ⚠️ 别在这里再 `setShift(false)`：会清掉刚重推的引擎位（首个字母变小写）。 */
  switchMode(m: OpiMode): void {
    if (m !== OpiMode.English) {
      KeyboardModes.leaveEnglishPro();
    }
    OpiEngine.clear();
    OpiEngine.switchMode(m);
    this.publish();
  }

  /** 点小欧：学习开关取反（落盘，Android ImePreferences.learn）；关掉 = 它睡着。 */
  toggleLearner(): void {
    const on: boolean = !OpiEngine.learnerEnabled();
    OpiEngine.setLearner(on);
    SettingsStore.saveBool(SettingsStore.BOOL_LEARN, on);
    this.publish();
  }

  // -------------------------------------------------------------- 输入模式（2026-10-14）

  /** 选输入模式（MODE_*）：换布局+方案。⚠️ 转发后都 `publish()`（`setScheme` 清缓冲）。 */
  selectInputMode(index: number): void {
    if (index === InputModes.MODE_RARE) {
      // 生僻字是面板视图（不进引擎、不改键位）。
      KeyboardModes.openNestedPanel(PanelView.Rare);
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

  /** 面板窗口：高度按 Android 公式（ScreenLayout）；悬浮/游戏收窄居中，游戏再压高度。 */
  private applyPanelWindow(): void {
    if (this.panel === null) {
      return;
    }
    const r: ScreenLayout.PanelRect = ScreenLayout.panelRect(this.metrics,
      KeyboardModes.gameEnabled(), KeyboardModes.floatingEnabled());
    if (r.w <= 0 || r.h <= 0) {
      return;
    }
    this.panel.resize(r.w, r.h);
    this.panel.moveTo(r.x, r.y);
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

  /** 表情页「自有」分类开关（Android ImePreferences.showOwn）。 */
  toggleShowOwn(): void {
    KeyboardModes.toggleShowOwn();
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

  /** 把引擎状态推给 UI（StatePublish：AppStorage 契约 + 平铺自动收拢）。UI 不直接调引擎。 */
  publish(): void {
    publishState();
  }
}
