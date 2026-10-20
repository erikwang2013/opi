// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。装「输入模式」那九项各自的
// **键面**，对齐 android/.../keyboard/ 下的同名 Compose 文件：
//   9 键 / 大 9 键（NineKeyKeyboard.kt，只差字号）· 14 键（FourteenKeyKeyboard.kt）
//   · 笔画（StrokeKeyboard.kt）· 生僻字（RarePanel.kt，**本轮只做骨架**）
//   · 「输入模式」面板（ImeToolbar.kt 的 InputModePanel：标题 + 九枚选项）
// 26 键族（拼音/英文/双拼/五笔）**不在这里** —— 那是 pages/Index.ets 的 KeyRows。
//
// ⚠️ **键表是协议**（9 键 / 14 键）：发送的键值必须落在 engine-core `scheme.rs` 的
// `Scheme::key_class()` 类表里（9 键的 `2`..`9` 各带三四个字母；14 键的两字母键取
// **首个字母** —— 本文件直接按这条规则从 label 推导，不另存一份）。Android 侧有
// NineKeyLayoutTest / FourteenKeyLayoutTest 拿类表逐格交叉核对；本端**没有**对应测试，
// 改这张表 = 改协议，四端一起改。
//
// ⚠️ 与 Android 的三处**已知差异**（如实记，别当成 bug 修）：
//   1. 9 键的**左列只画标点列**：Android 输入时会换成「候选拼音读数」，而那个读数出口
//      （`opijni_candidate_readings`）**只在 JNI 面上有，C ABI 里没有** ⇒ 鸿蒙端拿不到
//      （见 harmony/README.md 的缺口表）。所以左列 = 五枚标点，不随缓冲变化。
//   2. 各键面的底行**不含「输入」入口**：面板入口统一放在候选栏（跟 Android 的工具栏
//      同一位置）。
//   3. 生僻字面板**只有骨架**：`rare_index.tsv` 不在仓库里（Android 是 1.1MB 资产，
//      首次打开时读 + 缓存）。没有数据就**不画假内容** —— 面板明说「索引未随包」。
//      Android 的两级筛序（选部首 → 选笔画数 → 挑字）+ 直提字形，本端未实现；
//      实现它**不需要新的 C ABI 出口**（Android 也是宿主侧解析 TSV）。
//
// ⚠️ **未编译面（本机查不到、只能按常见写法写）**：KeyBox 的 `layoutWeight` 由**调用点**
// 施加（自定义组件上挂布局属性的写法待 DevEco 核实）；`justifyContent`/`alignItems`/
// `borderRadius`/`ForEach` 的三参形式与 Index.ets 同款。任一处名字对不上 ⇒ 第一次
// DevEco 构建在本文件报错，那正是预期内的（照 SDK 改），别把整段删了。

import { KeyboardController } from './KeyboardController';
import {
  INPUT_MODES,
  MODE_ENGLISH26,
  MODE_RARE,
  currentModeIndex,
  modeLabel,
  nextMode,
  previousPlainIndex,
} from './InputModes';
import { OpiMode } from './OpiEngine';

// ---------------------------------------------------------------- 9 键键表（协议）
//
// 发送键（`key`）必须落在 `scheme.rs` 的 Nine 类表里；`hint` 是键面下的字母提示，
// 且**必须正是该数字在类表里带的那些字母**（`2`→abc … `9`→wxyz）。
// 左上角那枚多标点键（`，。?!`）沿引擎标点表：`,` `\` 等由标点层出中文标点，
// 与 26 键的 `,` 是**同一条路**，不是 UI 直提。

class NineKey {
  label: string;
  key: string;
  hint: string = '';

  constructor(label: string, key: string, hint: string = '') {
    this.label = label;
    this.key = key;
    this.hint = hint;
  }
}

/** 左列：空闲时的标点列（键值都取引擎中文标点表**已有**的键）。 */
const NINE_STRIP: NineKey[] = [
  new NineKey('，', ','),
  new NineKey('。', '.'),
  new NineKey('、', '\\'),
  new NineKey('？', '?'),
  new NineKey('！', '!'),
];

const NINE_ROWS: NineKey[][] = [
  [new NineKey('，。?!', ','), new NineKey('2', '2', 'abc'), new NineKey('3', '3', 'def')],
  [new NineKey('4', '4', 'ghi'), new NineKey('5', '5', 'jkl'), new NineKey('6', '6', 'mno')],
  [new NineKey('7', '7', 'pqrs'), new NineKey('8', '8', 'tuv'), new NineKey('9', '9', 'wxyz')],
];

/** 14 键三行：`qw er ty ui op / as df gh jk l / zx cv bn m`（`l`/`m` 是单字母键）。 */
const FOURTEEN_ROWS: string[][] = [
  ['qw', 'er', 'ty', 'ui', 'op'],
  ['as', 'df', 'gh', 'jk', 'l'],
  ['zx', 'cv', 'bn', 'm'],
];

/** 笔画五键（键值 h/s/p/n/z 是 `scheme.rs` 的 Stroke 类表；名字给读屏与键面）。 */
class StrokeKey {
  label: string;
  key: string;
  name: string;

  constructor(label: string, key: string, name: string) {
    this.label = label;
    this.key = key;
    this.name = name;
  }
}

const STROKE_KEYS: StrokeKey[] = [
  new StrokeKey('一', 'h', '横'),
  new StrokeKey('丨', 's', '竖'),
  new StrokeKey('丿', 'p', '撇'),
  new StrokeKey('丶', 'n', '捺'),
  new StrokeKey('乙', 'z', '折'),
];

/** 「输入模式」面板的 3×3 排布（项下标；与 INPUT_MODES 的顺序对应，别单独改）。 */
const MODE_GRID: number[][] = [
  [0, 1, 2],
  [3, 4, 5],
  [6, 7, 8],
];

// ---------------------------------------------------------------- 零件

/** 一位小欧尺寸（与 Android KeyBox 同观感）：圆角底 + 点击。 */
@Component
export struct KeyBox {
  /** 键面主字。 */
  @Prop label: string = '';
  /** 第二行（9 键的字母提示 / 笔画的笔画名）；空串 = 单行大字。 */
  @Prop sub: string = '';
  /** 主字字号（大 9 键放大这一档）。 */
  @Prop mainSize: number = 21;
  /** 第二行字号。 */
  @Prop subSize: number = 12;
  onTap: () => void = () => {
  };

  build() {
    Column() {
      if (this.sub.length === 0) {
        // 单行：多字形键（如 `，。?!`）收一档字号，否则 1/3 键宽里放不下（Android 同款）。
        Text(this.label)
          .fontSize(this.label.length > 1 ? this.mainSize - 6 : this.mainSize)
          .fontColor('#1E6B63')
      } else {
        // 两行：**字母在上、数字在下**（2026-10-09 用户裁决「人们认键靠字母」）。
        Text(this.sub).fontSize(this.mainSize).fontColor('#1E6B63')
        Text(this.label).fontSize(this.subSize).fontColor('#9AA7A4')
      }
    }
    .justifyContent(FlexAlign.Center)
    .height('100%')
    .margin({ left: 2, right: 2 })
    .backgroundColor('#FFFFFF')
    .borderRadius(6)
    .onClick(this.onTap)
  }
}

// ---------------------------------------------------------------- 9 键 / 大 9 键

/**
 * 拼音 9 键（T9）。`big` = 大 9 键：**同一套引擎配置**，只放大字号（对齐主流输入法）。
 * 数字键在此方案下**是输入键**（引擎 Nine 方案收进缓冲），没有数字选词 UI。
 *
 * 布局：左标点列 + 右侧 3×3 数字格 + 右列（⌫/重输/空格）；底行 符/中/123/英文/↵
 * （Android 9 键的底行，含它的两个快捷键：123 = 数字键面、英文 = 英文 26 键）。
 */
@Component
export struct NineKeyPanel {
  @Prop big: boolean = false;
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private mainSize(): number {
    return this.big ? 30 : 21;
  }

  private subSize(): number {
    return this.big ? 15 : 12;
  }

  build() {
    Column() {
      Row() {
        // 左列（权重 1，与右列同宽）：整列标点。⚠️ Android 输入时这列会换成候选拼音
        // 读数 —— 那个出口只有 JNI 有（见文件头第 1 条），本端拿不到，故恒为标点。
        Column() {
          ForEach(NINE_STRIP, (k: NineKey) => {
            KeyBox({
              label: k.label, mainSize: this.mainSize(),
              onTap: () => {
                this.ctrl.tapChar(k.key);
              },
            }).layoutWeight(1)
          }, (k: NineKey) => k.key)
        }.layoutWeight(1)

        // 3×3 数字格（权重 3）。
        Column() {
          ForEach(NINE_ROWS, (row: NineKey[], rowIndex: number) => {
            Row() {
              ForEach(row, (k: NineKey) => {
                KeyBox({
                  label: k.label, sub: k.hint, mainSize: this.mainSize(), subSize: this.subSize(),
                  onTap: () => {
                    this.ctrl.tapChar(k.key);
                  },
                }).layoutWeight(1)
              }, (k: NineKey) => k.key)
            }.layoutWeight(1)
          }, (row: NineKey[], rowIndex: number) => `nine-${rowIndex}`)
        }.layoutWeight(3)

        // 右列（权重 1）：⌫ / 重输 / 空格 —— 2026-10-10 用户裁决把它们从底行搬上来。
        // 重输 = 清掉当前组合、**不提交**：Android `NineKeyKeyboard` 的
        // `onRetype = controller::clear`（= `api.clear()` + refresh）、iOS `.retype` =
        // `engine.clear()` + refresh，两端都明确不提交 —— 直提缓冲会把 9 键刚按的数字
        // 打进输入框（iOS 源码里那句注释）。`clear` 幂等，空缓冲是空操作。
        Column() {
          KeyBox({
            label: '⌫', mainSize: 17,
            onTap: () => {
              this.ctrl.tapBackspace();
            },
          }).layoutWeight(1)
          KeyBox({
            label: '重输', mainSize: 15,
            onTap: () => {
              this.ctrl.retype();
            },
          }).layoutWeight(1)
          KeyBox({
            label: '空格', mainSize: 15,
            onTap: () => {
              this.ctrl.tapSpace();
            },
          }).layoutWeight(1)
        }.layoutWeight(1)
      }.layoutWeight(3).margin({ top: 6 })

      // 底行（权重 1）：符 / 中英 / 123 / 英文 / ↵。
      Row() {
        Button('符')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.switchMode(OpiMode.Symbol);
          })
        Button(modeLabel(this.mode))
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.switchMode(nextMode(this.mode));
          })
        Button('123')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            // **只切语言模式，不动输入模式**：数字键面由 `opiMode === Number` 画（Index.ets
            // 的 build 里数字分支排在布局分支前面）⇒ 9 键用户按完 123 回来还是 9 键。
            // Android 同款（`ImeState.kt::openNumber`：只 `switchMode(NUMBER)` + 换 view）。
            // ⚠️ 别写成 `selectInputMode(MODE_PINYIN26)` —— 那是真改输入模式 + 清缓冲。
            this.ctrl.switchMode(OpiMode.Number);
          })
        Button('英文')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            // 与输入模式面板的「英文26键」**同一条路**（Android 的 onEnglish 亦然）。
            this.ctrl.selectInputMode(MODE_ENGLISH26);
          })
        Button('↵')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.tapEnter();
          })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%')
  }
}

// ---------------------------------------------------------------- 14 键

/** 拼音 14 键：两行 5 键 + 一行 4 键 + 功能行（与 9 键同一套功能键语义）。 */
@Component
export struct FourteenKeyPanel {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  build() {
    Column() {
      ForEach(FOURTEEN_ROWS, (row: string[], rowIndex: number) => {
        Row() {
          ForEach(row, (pair: string) => {
            // 发送键 = 字母对的**首个字母**（`scheme.rs` 的 Fourteen 类表就是这条规则）。
            KeyBox({
              label: pair, mainSize: 22,
              onTap: () => {
                this.ctrl.tapChar(pair.charAt(0));
              },
            }).layoutWeight(1)
          }, (pair: string) => pair)
        }.layoutWeight(1)
      }, (row: string[], rowIndex: number) => `fp-${rowIndex}`)

      Row() {
        Button('符')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.switchMode(OpiMode.Symbol);
          })
        Button(modeLabel(this.mode))
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.switchMode(nextMode(this.mode));
          })
        Button('空格')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(2).height(42)
          .onClick(() => {
            this.ctrl.tapSpace();
          })
        Button('⌫')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.tapBackspace();
          })
        Button('↵')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.tapEnter();
          })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%').margin({ top: 6 })
  }
}

// ---------------------------------------------------------------- 笔画

/** 笔画键盘：一行五个笔画键 + 功能行（含「生僻字」入口 —— 找不到字时下一步就是它）。 */
@Component
export struct StrokeKeyPanel {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  build() {
    Column() {
      Row() {
        ForEach(STROKE_KEYS, (k: StrokeKey) => {
          KeyBox({
            label: k.label, sub: k.name, mainSize: 26, subSize: 12,
            onTap: () => {
              this.ctrl.tapChar(k.key);
            },
          }).layoutWeight(1)
        }, (k: StrokeKey) => k.key)
      }.layoutWeight(1).margin({ top: 6 })

      Row() {
        Button('符')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.switchMode(OpiMode.Symbol);
          })
        Button(modeLabel(this.mode))
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.switchMode(nextMode(this.mode));
          })
        Button('生僻字')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1.4).height(42)
          .onClick(() => {
            this.ctrl.selectInputMode(MODE_RARE);
          })
        Button('空格')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(2).height(42)
          .onClick(() => {
            this.ctrl.tapSpace();
          })
        Button('⌫')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.tapBackspace();
          })
        Button('↵')
          .fontSize(15).fontColor('#1E6B63').backgroundColor('#FFFFFF').layoutWeight(1).height(42)
          .onClick(() => {
            this.ctrl.tapEnter();
          })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%')
  }
}

// ---------------------------------------------------------------- 生僻字（骨架）

/**
 * 生僻字键盘 —— **本轮只做骨架**（数据面未落地）。
 *
 * Android 的做法（`RarePanel.kt` + `RareIndex.kt`）：Unihan 部首 + 总笔画数索引
 * （`rare_index.tsv`，1.1MB 资产）在**首次打开时**读进内存，两级筛序「选部首 →
 * 选笔画数 → 挑字」，点字**直提字形**（`onCommit`）—— **全程不过引擎**，所以
 * 本面板**不调任何 opi_* 出口**（Android 同款：RARE 模式不碰引擎的 scheme/mode）。
 *
 * ⚠️ `rare_index.tsv` **不在本仓库**（脚本 `scripts/gen_rare_index.py` 的产物，
 * 未入库）⇒ 本端**没有数据可画**。这里如实说明，不画假部首/假字表：
 *   要做完整版，需要 ① 把 TSV 放进工程 rawfile；② 宿主侧解析成
 *   `Map<部首, Map<笔画数, 字列表>>`（同名脚本产物的列定义见 `scripts/gen_rare_index.py`）；
 *   ③ 画部首条 + 笔画数条 + 字网格。**不需要新的 C ABI 出口。**
 */
@Component
export struct RarePanel {
  private ctrl: KeyboardController = KeyboardController.getInstance();

  build() {
    Column() {
      // 面包屑/标题行（Android 顶部始终有一条，部首 → 笔画 可逐级回退）。
      Row() {
        Text('生僻字')
          .fontSize(14).fontColor('#1E6B63').layoutWeight(1).padding({ left: 10 })
        // 返回：回上一个普通模式（Android 的 onClose；那边也记住进面板前的模式）。
        Text('‹ 返回')
          .fontSize(14).fontColor('#2E8B80').padding({ left: 10, right: 10 })
          .onClick(() => {
            this.ctrl.selectInputMode(previousPlainIndex());
          })
      }.height(40).width('100%').backgroundColor('#E9F3F1')

      Text('部首')
        .fontSize(14).fontColor('#9AA7A4').margin({ top: 12, left: 12 })
      Text('笔画数')
        .fontSize(14).fontColor('#9AA7A4').margin({ top: 8, left: 12 })
      Text('生僻字索引未随包（rare_index.tsv 不在仓库）⇒ 暂无可选内容')
        .fontSize(14).fontColor('#9AA7A4').margin({ top: 12, left: 12 })
      Text('要接上：把 TSV 放进 rawfile + 宿主侧解析（详解见本组件注释）')
        .fontSize(12).fontColor('#C7D2CF').margin({ top: 6, left: 12 })
    }.width('100%').height('100%').backgroundColor('#FFFFFF')
  }
}

// ---------------------------------------------------------------- 输入模式面板

/**
 * 「输入模式」面板（对齐 Android `ImeToolbar.kt` 的 InputModePanel）：标题 + 九枚选项。
 * 入口在候选栏（Always 可见那一行，同 Android 的工具栏位置）；点一项 = 选中并收起
 * （Android 的「选中即换」）。
 */
@Component
export struct InputModePanel {
  private ctrl: KeyboardController = KeyboardController.getInstance();

  build() {
    Column() {
      Row() {
        Text('‹ 返回')
          .fontSize(14).fontColor('#2E8B80').padding({ left: 10, right: 10 })
          .onClick(() => {
            this.ctrl.closeModePanel();
          })
        Text('输入模式')
          .fontSize(15).fontColor('#1E6B63').layoutWeight(1).padding({ right: 10 })
      }.height(38).width('100%').backgroundColor('#E9F3F1')

      // 九枚选项，3×3 排（不用 Flex 换行 —— 本机查不到它的行为，而九项正好三行）。
      // ⚠️ 高亮读的是**面板打开那一刻**的模式（`currentModeIndex()` 不是响应式状态）——
      // 点一项后面板即收起，所以不会出现「高亮停在旧项上」的观感。
      ForEach(MODE_GRID, (gridRow: number[], gridIndex: number) => {
        Row() {
          ForEach(gridRow, (i: number) => {
            Text(INPUT_MODES[i].label)
              .fontSize(14)
              .fontColor(i === currentModeIndex() ? '#FFFFFF' : '#1E6B63')
              .backgroundColor(i === currentModeIndex() ? '#2E8B80' : '#FFFFFF')
              .borderRadius(6)
              .padding({ left: 8, right: 8, top: 6, bottom: 6 })
              .margin({ left: 3, right: 3, top: 4 })
              .onClick(() => {
                this.ctrl.selectInputMode(i);
                this.ctrl.closeModePanel();
              })
          }, (i: number) => `${i}`)
        }.width('100%').justifyContent(FlexAlign.Center)
      }, (gridRow: number[], gridIndex: number) => `mg-${gridIndex}`)
    }.width('100%').backgroundColor('#F4F7F6')
  }
}
