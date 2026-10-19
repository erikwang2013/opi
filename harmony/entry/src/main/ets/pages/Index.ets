// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。ArkUI 的 @Entry/@Component
// 装饰器、@StorageLink、@Builder 在普通 tsc 下就断，所以这里一个字符都没编译过。
//
// 键盘面板的最小 UI 骨架：候选栏 + preedit + 键面 + 功能行。
// **本页不认识引擎** —— 只读 AppStorage、只调 KeyboardController。
// 引擎状态的唯一来源是 KeyboardController.publish()。
//
// 键面按「输入模式」分支（2026-10-14）：26 键族（拼音/英文/双拼/五笔）用本页的
// KeyRows；9 键/大 9 键/14 键/笔画/生僻字在 KeyPanels.ets（各自带底行，见 usesSharedRow）；
// 表情页在 EmojiPad.ets（Symbol 模式，**不是**输入模式的一项）。
//
// ⚠️ 本页必须在 resources/base/profile/main_pages.json 里注册为 "pages/Index"，
//    且与 KeyboardController 的 panel.setUiContent('pages/Index') 字符串一致。
//    任一处对不上 → 面板起来了但是一片白。

import { KeyboardController } from '../InputMethodExtensionAbility/KeyboardController';
import { OpiMode, OpiShift } from '../InputMethodExtensionAbility/OpiEngine';
import { OpiPet, PetMood, petMood } from '../InputMethodExtensionAbility/OpiPet';
import {
  MODE_BIG9, MODE_PINYIN9, MODE_PINYIN14, MODE_RARE, MODE_SHUANGPIN, MODE_STROKE, MODE_WUBI,
  modeLabel, nextMode
} from '../InputMethodExtensionAbility/InputModes';
import { EmojiPad } from '../InputMethodExtensionAbility/EmojiPad';
import { FourteenKeyPanel, InputModePanel, NineKeyPanel, RarePanel, StrokeKeyPanel } from '../InputMethodExtensionAbility/KeyPanels';

/** QWERTY 三行。数字模式用下面的 NUM_ROW_* 顶上；Symbol 模式**不是键行**，是表情页。 */
const ROW_1: string[] = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'];
const ROW_2: string[] = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'];
const ROW_3: string[] = ['z', 'x', 'c', 'v', 'b', 'n', 'm'];

/** 数字行（Number 模式）。 */
const NUM_ROW_1: string[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
const NUM_ROW_2: string[] = ['-', '/', ':', ';', '(', ')', '$', '&', '@', '"'];
const NUM_ROW_3: string[] = ['.', ',', '?', '!', '\'', '+', '=', '%'];

/**
 * 候选栏里的小欧尺寸。**小于 64vp 所以自动不刻 OPI**（刻字会糊成脏点）。
 * 与 Android 三处落点同值（`ImeScreen.kt` / `CandidateBar.kt` / `SymbolPanel.kt`
 * 的 `size = 30.dp`）—— 两端要一致改。
 */
const PET_SIZE_IN_BAR: number = 30;

@Entry
@Component
struct Index {
  // ---- 引擎状态：全部从 AppStorage 读，本页不自己缓存、不自己算 ----
  //
  // ⚠️ 这里的键名**只能内联字符串字面量**，不能写 `@StorageLink(KEY_BUFFER)`：
  // ArkTS 的装饰器入参是**编译期静态处理**的，只收字面量，传常量编译不过。
  // 这和 KeyboardController.ets 顶部那条「键名集中在这里，避免两边写不一致的字符串」
  // 是**直接冲突**的 —— 那边的做法在写入侧成立（`AppStorage.setOrCreate(KEY_X, …)`
  // 是运行时调用，不受限制），在**装饰器入参这个位置不成立**。
  // 所以键名在**用到它的每个文件里各存一份**（2026-10-14 起有三个：本文件、
  // KeyboardController.ets 的写入侧常量、KeyPanels.ets 的 `opiMode`），
  // **改一边必须改另一边**。当前值与本文件逐字对应：
  //   buffer=opiBuffer  candidates=opiCandidates  page=opiPage  pageCount=opiPageCount
  //   mode=opiMode  shift=opiShift  learner=opiLearner
  //   layout=opiLayout（输入模式下标）  panel=opiPanel（输入模式面板开没开）
  // 核对命令（各处的字面量必须一一对应）：
  //   grep -rn "opiBuffer\|opiCandidates\|opiPage\|opiPageCount\|opiMode\|opiShift\|opiLearner\|opiLayout\|opiPanel" harmony/
  // 漏改的表现是「面板起来了、按键也有反应，但候选栏/preedit 永远不刷新」——
  // **不报错、不崩溃**，属最难查的一类。
  // ⚠️ 这条 ArkTS 限制出自华为规格与一则同症状的求助帖，**二手资料，待 DevEco 确认**
  //   （见 harmony/README.md「最不确定的 API」→「待 DevEco 确认：@StorageLink 的
  //   入参必须是字符串字面量」）。字面量在「规则严」与「规则松」下都成立，
  //   所以按最稳妥的写法写。
  @StorageLink('opiBuffer') buffer: string = '';
  @StorageLink('opiCandidates') candidates: string[] = [];
  @StorageLink('opiPage') page: number = 0;
  @StorageLink('opiPageCount') pageCount: number = 0;
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiShift') shift: number = OpiShift.Off as number;
  @StorageLink('opiLearner') learner: number = 1;
  /** 输入模式下标（InputModes.ets 的 INPUT_MODES；由控制器的 selectInputMode 写）。 */
  @StorageLink('opiLayout') layout: number = 0;
  /** 输入模式面板是否展开（1/0）。 */
  @StorageLink('opiPanel') panel: number = 0;

  private ctrl: KeyboardController = KeyboardController.getInstance();

  /**
   * 小欧的情绪：**由输入状态推导，本页不自己维护**（与 Android 端同一套规则，
   * 纯函数 petMood 在 OpiPet.ets，规格见 OpiPetMoodTest.kt）。
   *
   * ⚠️ **只传 3 参**，`dictionaryDegraded` 用默认 `false` —— 不是漏了，是**鸿蒙端拿不到
   * 那个信号**：`KeyboardController.ensureDictionary()` 调的是 `OpiEngine.loadDictionary('')`，
   * 空路径按设计就直达内置词库（鸿蒙的完整词库装载尚未实现），所以它的返回值恒为 true，
   * 区分不出「装上了」与「回退了」。
   * ⇒ **鸿蒙端永远画不出 DEGRADED**。等哪天完整词库装载落地，这里要跟着接上。
   */
  private mood(): PetMood {
    return petMood(this.buffer, this.candidates.length, this.learner === 1);
  }

  private isDark(): boolean {
    // 候选栏底色是浅色，所以用默认调色板。深色主题要读系统主题再返回 dark() —— 未做。
    return false;
  }

  private letterRows(): string[][] {
    if (this.mode === (OpiMode.Number as number)) {
      return [NUM_ROW_1, NUM_ROW_2, NUM_ROW_3];
    }
    // ⚠️ **Symbol 不在这里**：那个模式整片换成表情页（见 build() 的分支），不是换键行。
    return [ROW_1, ROW_2, ROW_3];
  }

  /**
   * 是否画**共用功能行**：26 键族（拼音/英文/双拼/五笔）与表情页要；
   * 9 键/大 9 键/14 键/笔画/生僻字**各自带底行**（对齐 Android：每个键面自带底行，
   * 那几行里有 符/中英/123/英文 这些快捷入口，共用行里没有）。
   */
  private usesSharedRow(): boolean {
    if (this.mode === (OpiMode.Symbol as number)) {
      // 表情页没有自己的底行 —— 模式键是它**唯一**的出口，这一行必须留着。
      return true;
    }
    return this.mode === (OpiMode.Number as number)
      || this.layout === 0 || this.layout === 1
      || this.layout === MODE_SHUANGPIN || this.layout === MODE_WUBI;
  }

  build() {
    Column() {
      this.CandidateBar()
      this.PreeditBar()
      if (this.panel === 1) {
        // 输入模式面板：整片盖住键面与功能行（它自带「关闭」，见 KeyPanels.ets）。
        InputModePanel()
      } else {
        // Symbol 模式整片换成表情页（照 `letterRows()` 的同一处判据，**无新路由/新面板**）。
        // 功能行留着：模式键是**退出表情页的唯一入口**（中→EN→123→#+=→中）。
        if (this.mode === (OpiMode.Symbol as number)) {
          EmojiPad()
        } else if (this.mode === (OpiMode.Number as number)) {
          // 数字键面由**模式**决定，排在布局分支前：9 键/14 键/笔画/生僻字布局下按 123
          // 也走这里（判据与 `letterRows()` 同一处）。Android 的 `openNumber()` 同款 ——
          // 只 `switchMode(NUMBER)` 换 view，**不改输入模式** ⇒ 退出后键面照旧。
          this.KeyRows()
        } else if (this.layout === MODE_RARE) {
          // 生僻字：面板式 —— **不进引擎**（见 InputModes.selectInputMode）。
          RarePanel()
        } else if (this.layout === MODE_PINYIN9) {
          NineKeyPanel({ big: false })
        } else if (this.layout === MODE_BIG9) {
          NineKeyPanel({ big: true })
        } else if (this.layout === MODE_PINYIN14) {
          FourteenKeyPanel()
        } else if (this.layout === MODE_STROKE) {
          StrokeKeyPanel()
        } else {
          // 26 键族：拼音26键 / 英文26键 / 拼音双拼 / 五笔（方案不同、键面同为一套 QWERTY）。
          this.KeyRows()
        }
        if (this.usesSharedRow()) {
          this.FunctionRow()
        }
      }
    }
    .width('100%')
    .backgroundColor('#F4F7F6')
  }

  // ------------------------------------------------------------ 候选栏

  @Builder
  CandidateBar() {
    Row() {
      // 小欧站在候选栏最左边。没有候选时它就在这儿待命/发愣/睡觉。
      OpiPet({ mood: this.mood(), petSize: PET_SIZE_IN_BAR, dark: this.isDark(), forceLegend: false })
        .margin({ left: 6, right: 6 })
        // 点一下小欧 = 学习开关取反（关掉它就去睡觉，这也是一条设置入口）
        .onClick(() => {
          this.ctrl.toggleLearner();
        })

      // 候选：横向铺开。空态给一句提示而不是留白 —— 别让用户以为键盘坏了。
      if (this.candidates.length === 0) {
        Text(this.emptyHint())
          .fontSize(15)
          .fontColor('#9AA7A4')
          .layoutWeight(1)
      } else {
        Row() {
          ForEach(this.candidates, (item: string, index: number) => {
            Row() {
              // 可见序号 = **页内**下标 + 1（翻页后从 1 重数）：与 Android 候选栏、
              // Windows 候选窗同款，也与数字选词的语义（页内索引）一致。
              // ⚠️ 端上字母盘没有数字行 ⇒ 实际是**排名**展示：数字选词引擎支持、鸿蒙端按不到。
              Text(`${index + 1}.`)
                .fontSize(13)
                .fontColor('#9AA7A4')
                .margin({ right: 3 })
              Text(item)
                .fontSize(20)
                .fontColor('#1E6B63')
            }
            .padding({ left: 10, right: 10 })
            .onClick(() => {
              // **页内索引** —— 换算交给引擎，别自己算 page * 8 + index。
              this.ctrl.tapCandidate(index);
            })
          }, (item: string, index: number) => `${index}-${item}`)
        }
        .layoutWeight(1)
      }

      // 翻页。页码显示「当前/总数」，两个数都读引擎，不自己数。
      if (this.pageCount > 1) {
        Row() {
          Text('‹')
            .fontSize(22)
            .fontColor(this.page > 0 ? '#1E6B63' : '#C7D2CF')
            .padding({ left: 8, right: 8 })
            .onClick(() => {
              this.ctrl.tapPageUp();
            })
          Text(`${this.page + 1}/${this.pageCount}`)
            .fontSize(13)
            .fontColor('#9AA7A4')
          Text('›')
            .fontSize(22)
            .fontColor(this.page + 1 < this.pageCount ? '#1E6B63' : '#C7D2CF')
            .padding({ left: 8, right: 8 })
            .onClick(() => {
              this.ctrl.tapPageDown();
            })
        }
        .margin({ right: 6 })
      }

      // 输入模式入口（2026-10-14）：点开九项面板（InputModePanel，见 KeyPanels.ets）。
      // ⚠️ **与功能行的模式键是两根轴**，别合：功能行的键走**四态循环**（中/英/数/符，只换
      //    语言模式），这个键换的是**键面布局 + 引擎方案**（拼音9键/双拼/五笔…）。
      //    合掉任一个都会让另一件事没有入口。
      // ⚠️ 面板只盖住**键面**（见 build()：面板排在候选栏下面），所以本键在面板开着时**还在**
      //    ⇒ 再点一次即关（toggle）。关不掉的话面板里也另有一个「关闭」（KeyPanels.ets）。
      Text('⌨')
        .fontSize(20)
        .fontColor('#1E6B63')
        .padding({ left: 10, right: 10 })
        .onClick(() => {
          this.ctrl.toggleModePanel();
        })
    }
    .height(46)
    .width('100%')
    .backgroundColor('#FFFFFF')
  }

  // ------------------------------------------------------------ preedit

  @Builder
  PreeditBar() {
    Row() {
      // 拼音缓冲（preedit）。空的时候占位不留白，避免键盘高度跳动。
      Text(this.buffer.length === 0 ? ' ' : this.buffer)
        .fontSize(17)
        .fontColor('#2E8B80')
        .layoutWeight(1)
        .padding({ left: 12 })

      // 学习关闭时常驻提示 —— 小欧睡着了得有个说法。
      if (this.learner === 0) {
        Text('不记词')
          .fontSize(12)
          .fontColor('#D9553F')
          .margin({ right: 12 })
      }
    }
    .height(30)
    .width('100%')
    .backgroundColor('#E9F3F1')
  }

  // ------------------------------------------------------------ 键盘

  @Builder
  KeyRows() {
    Column() {
      ForEach(this.letterRows(), (row: string[], rowIndex: number) => {
        Row() {
          ForEach(row, (ch: string) => {
            this.KeyCap(this.shiftLabel(ch), () => {
              this.ctrl.tapChar(ch);
            })
          }, (ch: string) => ch)
        }
        .width('100%')
        .justifyContent(FlexAlign.Center)
        .margin({ top: rowIndex === 0 ? 8 : 6 })
      }, (row: string[], rowIndex: number) => `row-${rowIndex}`)
    }
    .width('100%')
  }

  @Builder
  FunctionRow() {
    Row() {
      // 模式键：中/英/数/符 轮转。⇧ 只在非数字/符号模式有意义。
      // 键面字/下一档都是 InputModes.ets 的**具名导出**（2026-10-14 从本页搬走：
      // 9 键族底行的「中/EN」键也用同一张表，两处编两套迟早漂移）。
      this.KeyCap(modeLabel(this.mode), () => {
        this.ctrl.switchMode(nextMode(this.mode));
      }, 1.3)

      if (this.mode === (OpiMode.Pinyin as number) || this.mode === (OpiMode.English as number)) {
        // ⚠️ **⇪ 这个分支目前到不了，是已知缺口**（不是「已实现」）：
        //    引擎进 LOCK 的唯一入口是 `tapShiftLongPress()`（states 带
        //    `KEY_STATE_LONG_PRESSED`），而本文件**还没有给这个键绑长按手势**。
        //    单击的 `tapShift()` 按 `router.rs` 的 `shift_tap` 只在 OFF↔SINGLE 之间
        //    来回 —— 所以 LOCK 是枚举有、UI 有、用户按不出来的状态。
        //    没在这里补 `.gesture(LongPressGesture())` 的原因：那是本机查不到、
        //    也编不了的 ArkUI API，而 `KeyCap` 是字母键与整个功能行**共用的** @Builder
        //    （5 个调用点，见下面的 KeyRows / FunctionRow；多参 @Builder 的坑见
        //    README 的 U18）—— 往这条路径上塞凭记忆写的代码，
        //    正是本目录存在的意义所要避免的那类改动。
        //    DevEco 上要做的是：在候选栏的 ⇧ 键上绑长按 → `tapShiftLongPress()`。
        this.KeyCap(this.shift === (OpiShift.Lock as number) ? '⇪' : '⇧', () => {
          this.ctrl.tapShift();
        }, 1.3, this.shift !== (OpiShift.Off as number))
      }

      this.KeyCap('⌫', () => {
        this.ctrl.tapBackspace();
      }, 1.3)

      // 空格占满剩余。缓冲非空时按它是**提交候选**，不是打空格 —— 逻辑在引擎。
      Button('空格')
        .fontSize(15)
        .fontColor('#1E6B63')
        .backgroundColor('#FFFFFF')
        .layoutWeight(4)
        .height(42)
        .margin({ left: 3, right: 3 })
        .onClick(() => {
          this.ctrl.tapSpace();
        })

      this.KeyCap('↵', () => {
        this.ctrl.tapEnter();
      }, 1.3)
    }
    .width('100%')
    .margin({ top: 8, bottom: 8 })
  }

  /** 一个键帽。weight 默认 1（用 layoutWeight 撑开，不用固定宽度）。 */
  @Builder
  KeyCap(label: string, action: () => void, weight: number = 1, active: boolean = false) {
    Button(label)
      .fontSize(17)
      .fontColor(active ? '#FFFFFF' : '#1E6B63')
      .backgroundColor(active ? '#2E8B80' : '#FFFFFF')
      .layoutWeight(weight)
      .height(42)
      .margin({ left: 3, right: 3 })
      .onClick(action)
  }

  // ------------------------------------------------------------ 小工具
  //
  // （原「表情页」整段 + `emojiShown/emojiRows` 两个私有方法已于 2026-10-14 **原样**
  //   搬到 EmojiPad.ets；`modeLabel/nextMode` 搬到 InputModes.ets 成为模块级函数。
  //   本页现在只剩键面骨架与候选栏/preedit。）

  /**
   * ⇧ 点亮时字母键显示大写（**只是显示**，真正的大小写由引擎按 states 决定）。
   *
   * 判据是「字母模式」= Pinyin **或** English。原先只判 Pinyin，方向是反的：
   * 引擎真正按 ⇧ 改大小写的地方在**英文**的直传路径
   * （`router.rs` 的 `Mode::English` 空缓冲分支 → `to_ascii_uppercase()`），
   * 拼音模式下字母进缓冲、大小写不参与检索 —— 也就是说旧写法**该显大写的不显、
   * 不影响的显了**（⇧ 亮着打英文，键帽却还是小写）。
   * 注：英文**缓冲非空**时引擎也不转大写（走 `input_key`），
   * 那是与 fcitx5 / TSF 两轨逐行同构的既有语义，客户端不在这层纠正。
   */
  private shiftLabel(ch: string): string {
    const letterMode: boolean = this.mode === (OpiMode.Pinyin as number)
      || this.mode === (OpiMode.English as number);
    if (this.shift === (OpiShift.Off as number) || !letterMode) {
      return ch;
    }
    return ch.toUpperCase();
  }

  /**
   * 候选栏空态的那句话。
   *
   * **两种空态不是一回事**：缓冲为空 = 安静待命，提示怎么开始；有缓冲无候选 =
   * 「我也没辙」（小欧此刻正是 PUZZLED），这时说「打拼音试试」是答非所问 ——
   * 用户刚打的**就是**拼音。后一种照 Android 候选栏 / 符号面板 / 桌面候选窗
   * 同一句话写「无匹配」（那三处都是小欧摊手 + 这句白话）。
   */
  private emptyHint(): string {
    if (this.buffer.length > 0) {
      return '无匹配';
    }
    return this.mode === (OpiMode.Pinyin as number) ? '打拼音试试' : '';
  }

}
