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
import { EmojiGroup, OpiMode, OpiShift } from '../OpiEngine';
import { OpiPet, PetMood, petMood } from '../OpiPet';

/** QWERTY 三行。数字模式用下面的 NUM_ROW_* 顶上；Symbol 模式**不是键行**，是表情页。 */
const ROW_1: string[] = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'];
const ROW_2: string[] = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'];
const ROW_3: string[] = ['z', 'x', 'c', 'v', 'b', 'n', 'm'];

/** 数字行（Number 模式）。 */
const NUM_ROW_1: string[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
const NUM_ROW_2: string[] = ['-', '/', ':', ';', '(', ')', '$', '&', '@', '"'];
const NUM_ROW_3: string[] = ['.', ',', '?', '!', '\'', '+', '=', '%'];

/**
 * 每行几个表情。与 Android `SymbolPanel` 的 `perRow = 8` 同值（两端要一致改）。
 * ⚠️ 原先的 `SYM_ROW_1..3`（29 个 ASCII 符号）已随本轮换页删除 —— 见 README 缺口第 3 条。
 */
const EMOJI_PER_ROW: number = 8;

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

  // ---- 表情页（Symbol 模式）的本地状态：只存「看哪一组/搜什么」，**不存数据** ——
  // 数据每次渲染现读控制器（缓存与「空结果不缓存」的纪律都在那边），两处存两份迟早漂移。
  @State emojiQuery: string = '';
  @State emojiGroup: number = 0;

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

  build() {
    Column() {
      this.CandidateBar()
      this.PreeditBar()
      // Symbol 模式整片换成表情页（照 `letterRows()` 的同一处判据，**无新路由/新面板**）。
      // 功能行留着：模式键是**退出表情页的唯一入口**（中→EN→123→#+=→中）。
      if (this.mode === (OpiMode.Symbol as number)) {
        this.EmojiPage()
      } else {
        this.KeyRows()
      }
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

  // ------------------------------------------------------------ 表情页（Symbol 模式）

  /**
   * 表情页：搜索框 + 分组条 + 网格（对标 Android `SymbolPanel.kt` 的表情 Tab，2026-10-13 补齐）。
   *
   * ⚠️ 两条**如实记下**的代价（README「已知缺口」第 3 条）：
   *   ① 原 `SYM_ROW_1..3` 的 29 个 ASCII 符号不再一键可达（Android 有「常用/全部」Tab，本端没做）；
   *   ② **搜索框的输入通道未定**：面板自己就是输入法，`TextInput` 聚焦时没有第二个输入法可唤起
   *      ⇒ 真机上大概率「能显示、不能打字」（Android 的做法见 README 缺口第 3 条）——**没做**。
   */
  @Builder
  EmojiPage() {
    Column() {
      // ⚠️ 未编译，DevEco 上核实 `TextInput` 的 placeholder/text/onChange 三参与面板内能否聚焦；
      // 备选 `Search({ value, placeholder })`，再退 = 面板字母键直接喂 emojiQuery（见上 ②）。
      TextInput({ placeholder: '搜索（拼音/英文）', text: this.emojiQuery })
        .width('100%')
        .height(38)
        .fontSize(14)
        .onChange((v: string) => { this.emojiQuery = v; })

      if (this.emojiQuery.length === 0) {
        // 分组条：点一项 = 网格换成那一组（搜索态不显示，同 Android 的两套工具条）。
        // ⚠️ 与 Android **判据同、动作不同**：那边 `scrollToItem(组.start)` 滚动，这边**切片
        // 过滤**（见 emojiShown）—— 少一条本机查不到的 `Scroller` API，也不必重算过滤后下标。
        // ⚠️ 未编译：`Scroll` + `ScrollDirection.Horizontal`；备选
        // `List(){…}.listDirection(Axis.Horizontal)`，再退裸 Row（9 组会横向溢出）。
        Scroll() {
          Row() {
            ForEach(this.ctrl.emojiGroups(), (g: EmojiGroup, i: number) => {
              Text(g.name)
                .fontSize(14)
                .fontColor(i === this.emojiGroup ? '#FFFFFF' : '#1E6B63')
                .backgroundColor(i === this.emojiGroup ? '#2E8B80' : '#FFFFFF')
                .borderRadius(6)
                .padding({ left: 10, right: 10, top: 5, bottom: 5 })
                .margin({ left: 3, right: 3 })
                .onClick(() => {
                  this.emojiGroup = i;
                })
            }, (g: EmojiGroup, i: number) => `${i}-${g.name}`)
          }
        }
        .scrollable(ScrollDirection.Horizontal)
        .width('100%')
        .height(36)
      }

      if (this.ctrl.emojiSymbols().length === 0) {
        // 空态给一句白话、别留白（iOS 同款）。表情表**不经引擎单例**，正常冷启动第一帧就有；
        // 走到这里是模块/数据真没起来 —— 切走再切回即重试。
        Text('表情库未就绪').fontSize(14).fontColor('#9AA7A4').margin({ top: 12 })
      } else {
        // 网格（每行 EMOJI_PER_ROW 个）。⚠️ 未编译，DevEco 上核实纵向 `Scroll`；备选
        // `Grid{GridItem}.columnsTemplate('1fr '.repeat(8))`。Column/Row/ForEach 与 KeyRows 同构。
        //
        // ⚠️ 天花板：ForEach **不虚拟化** —— 选中一组就整组铺出来（「人物」2430 项 ≈ 300 行节点，
        // 真机点它可能卡）。换 `List{LazyForEach}` 要一个 IDataSource，且 LazyForEach **只能**做
        // List/Grid/Swiper 的直接子节点（塞不进 Scroll/Column）—— 骨架与判据见 README 缺口。
        //
        // ⚠️ **未做「画不出的字形」过滤**：Android 用 `Paint.hasGlyph`、iOS 用
        // `CTFontGetGlyphsForCharacters`，鸿蒙侧的对应 API 本机查不到 —— **不编一个**。
        // 现状是个别新码位（如 U+1FAEB）画成空心框；日后补上时**只判单码位**，且分组
        // start/count 要跟着重算（Android 的教训：过滤会摘掉条目，原下标整体偏前）。
        Scroll() {
          Column() {
            ForEach(this.emojiRows(), (row: string[], rowIndex: number) => {
              Row() {
                ForEach(row, (e: string, col: number) => {
                  Text(e)
                    .fontSize(24)
                    .layoutWeight(1)
                    .textAlign(TextAlign.Center)
                    .onClick(() => {
                      this.ctrl.tapEmoji(e);
                    })
                }, (e: string, col: number) => `${col}-${e}`)
              }
              .width('100%')
              .margin({ top: rowIndex === 0 ? 6 : 4 })
            }, (row: string[], rowIndex: number) => `emoji-${rowIndex}`)
          }
          .width('100%')
        }
        .layoutWeight(1).width('100%')
      }
    }
    .width('100%').backgroundColor('#FFFFFF')
  }

  /**
   * 当前该画的表情：有搜索词 → 引擎搜索 ∩ 表情集（判据在控制器/引擎，本页不另判）；
   * 否则 → 选中分组的切片 `[start, start+count)`（「分组连续覆盖全表」由引擎门禁保证）；
   * 分组拿不到 → **整表兜底**（别让页面全空）。末尾**保序去重**是渲染前最后一道
   * （Android 同款裁决；上游已有零重复门禁，这里防将来哪层漏）。
   */
  private emojiShown(): string[] {
    let list: string[];
    if (this.emojiQuery.length > 0) {
      list = this.ctrl.searchEmoji(this.emojiQuery);
    } else {
      const groups: EmojiGroup[] = this.ctrl.emojiGroups();
      if (groups.length === 0) {
        list = this.ctrl.emojiSymbols();
      } else {
        // 组数变了（引擎重载）而 emojiGroup 还停在大下标时夹回最后一组，别越界成空。
        const g: EmojiGroup = groups[Math.min(this.emojiGroup, groups.length - 1)];
        list = this.ctrl.emojiSymbols().slice(g.start, g.start + g.count);
      }
    }
    // ⚠️ 未编译：`new Set(数组)` + `Array.from` 的 ArkTS 可用性；备选 = 手写 has/add 循环。
    return Array.from(new Set<string>(list));
  }

  /** 每行 EMOJI_PER_ROW 个切成二维（ForEach 只铺一维）。 */
  private emojiRows(): string[][] {
    const list: string[] = this.emojiShown();
    const out: string[][] = [];
    for (let i = 0; i < list.length; i += EMOJI_PER_ROW) {
      out.push(list.slice(i, i + EMOJI_PER_ROW));
    }
    return out;
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

  /**
   * 模式键的键面字 = **点它会切到哪**（2026-10-12 晚与 Android 同步的裁决：用户实测
   * 「切到英文键盘后键面还写着英，点下去却是中文键盘，键面该是『中』才对」）。
   * 逐条对齐本端 [`nextMode`] 的四态循环：中→EN、EN→123、123→#+=、#+=→中；
   * 繁体档本端循环里到不了（`nextMode` 无该分支 ⇒ 落回拼音）⇒ 键面给「中」。
   * ⚠️ **别照抄 Android 的表**：那边是三态（中→繁→英→中），本端是四态。
   */
  private modeLabel(): string {
    if (this.mode === (OpiMode.Pinyin as number)) {
      return 'EN';
    }
    if (this.mode === (OpiMode.English as number)) {
      return '123';
    }
    if (this.mode === (OpiMode.Number as number)) {
      return '#+=';
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
