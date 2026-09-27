// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。ArkUI 的 @Entry/@Component
// 装饰器、@StorageLink、@Builder 在普通 tsc 下就断，所以这里一个字符都没编译过。
//
// 键盘面板的最小 UI 骨架：候选栏 + preedit + 三行 QWERTY + 功能行。
// **本页不认识引擎** —— 只读 AppStorage、只调 KeyboardController。
// 引擎状态的唯一来源是 KeyboardController.publish()。
//
// ⚠️ 本页必须在 resources/base/profile/main_pages.json 里注册为 "pages/Index"，
//    且与 KeyboardController 的 panel.setUiContent('pages/Index') 字符串一致。
//    任一处对不上 → 面板起来了但是一片白。

import { KeyboardController } from '../KeyboardController';
import { OpiMode, OpiShift } from '../OpiEngine';
import { OpiPet, PetMood, petMood } from '../OpiPet';

/** QWERTY 三行。数字/符号模式用下面的 SYMBOL_ROWS 顶上。 */
const ROW_1: string[] = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'];
const ROW_2: string[] = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'];
const ROW_3: string[] = ['z', 'x', 'c', 'v', 'b', 'n', 'm'];

/** 数字行（Number 模式）。 */
const NUM_ROW_1: string[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
const NUM_ROW_2: string[] = ['-', '/', ':', ';', '(', ')', '$', '&', '@', '"'];
const NUM_ROW_3: string[] = ['.', ',', '?', '!', '\'', '+', '=', '%'];

/** 符号行（Symbol 模式）。够用即可，不是完整符号面板。 */
const SYM_ROW_1: string[] = ['[', ']', '{', '}', '#', '%', '^', '*', '+', '='];
const SYM_ROW_2: string[] = ['_', '\\', '|', '~', '<', '>', '€', '£', '¥', '•'];
const SYM_ROW_3: string[] = ['.', ',', '?', '!', '\'', '"', '-', ':', ';'];

/** 候选栏里的小欧尺寸。**小于 64vp 所以自动不刻 OPI**（刻字会糊成脏点）。 */
const PET_SIZE_IN_BAR: number = 34;

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
  // 所以键名在两个文件里各存一份，**改一边必须改另一边**。当前值与本文件逐字对应：
  //   buffer=opiBuffer  candidates=opiCandidates  page=opiPage  pageCount=opiPageCount
  //   mode=opiMode  shift=opiShift  learner=opiLearner
  // 核对命令（两边的字面量必须一一对应）：
  //   grep -rn "opiBuffer\|opiCandidates\|opiPage\|opiPageCount\|opiMode\|opiShift\|opiLearner" harmony/
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

  private ctrl: KeyboardController = KeyboardController.getInstance();

  /**
   * 小欧的情绪：**由输入状态推导，本页不自己维护**（与 Android 端同一套规则，
   * 纯函数 petMood 在 OpiPet.ets，规格见 OpiPetMoodTest.kt）。
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
    if (this.mode === (OpiMode.Symbol as number)) {
      return [SYM_ROW_1, SYM_ROW_2, SYM_ROW_3];
    }
    return [ROW_1, ROW_2, ROW_3];
  }

  build() {
    Column() {
      this.CandidateBar()
      this.PreeditBar()
      this.KeyRows()
      this.FunctionRow()
    }
    .width('100%')
    .backgroundColor('#F4F7F6')
  }

  // ------------------------------------------------------------ 候选栏

  @Builder
  CandidateBar() {
    Row() {
      // 小欧站在候选栏最左边。没有候选时它就在这儿待命/发愣/睡觉。
      OpiPet({ mood: this.mood(), size: PET_SIZE_IN_BAR, dark: this.isDark(), forceLegend: false })
        .margin({ left: 6, right: 6 })
        // 点一下小欧 = 学习开关取反（关掉它就去睡觉，这也是一条设置入口）
        .onClick(() => {
          this.ctrl.toggleLearner();
        })

      // 候选：横向铺开。空态给一句提示而不是留白 —— 别让用户以为键盘坏了。
      if (this.candidates.length === 0) {
        Text(this.mode === (OpiMode.Pinyin as number) ? '打拼音试试' : '')
          .fontSize(15)
          .fontColor('#9AA7A4')
          .layoutWeight(1)
      } else {
        Row() {
          ForEach(this.candidates, (item: string, index: number) => {
            Text(item)
              .fontSize(20)
              .fontColor('#1E6B63')
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
      this.KeyCap(this.modeLabel(), () => {
        this.ctrl.switchMode(this.nextMode());
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

  private modeLabel(): string {
    if (this.mode === (OpiMode.English as number)) {
      return 'EN';
    }
    if (this.mode === (OpiMode.Number as number)) {
      return '123';
    }
    if (this.mode === (OpiMode.Symbol as number)) {
      return '#+=';
    }
    if (this.mode === (OpiMode.Traditional as number)) {
      return '繁';
    }
    return '中';
  }

  private nextMode(): OpiMode {
    if (this.mode === (OpiMode.Pinyin as number)) {
      return OpiMode.English;
    }
    if (this.mode === (OpiMode.English as number)) {
      return OpiMode.Number;
    }
    if (this.mode === (OpiMode.Number as number)) {
      return OpiMode.Symbol;
    }
    return OpiMode.Pinyin;
  }
}
