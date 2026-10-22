// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 输入模式各自的键面（Android keyboard/ 下同名 Compose 文件的镜像）：
//   9 键 / 大 9 键 · 14 键 · 笔画。
// 26 键族与数字面板在 QwertyPad.ets；生僻字/符号页在 SymbolPage.ets；
// 输入模式面板在 Panels.ets。
//
// 键表是协议：9 键发送 `2..9`（类表见 scheme.rs），14 键发送字母对首字母。
// 底行口径（2026-10-15 Android 裁决）：9/14/笔画的中/繁键**只切简繁**，进英文走
// 独立的 [英文] 键；[符] 打开符号页（不是表情页）。

import { KeyboardController } from './KeyboardController';
import { MODE_ENGLISH26, traditionalModeLabel } from './InputModes';
import { OpiMode } from './OpiEngine';
import { Palette, paletteFor } from './Palette';

export class NineKey {
  label: string;
  key: string;
  hint: string = '';

  constructor(label: string, key: string, hint: string = '') {
    this.label = label;
    this.key = key;
    this.hint = hint;
  }
}

/** 左列：空闲时的标点列（键值都取引擎中文标点表已有的键；平铺候选左列复用）。 */
export const NINE_STRIP: NineKey[] = [
  new NineKey('，', ','),
  new NineKey('。', '.'),
  new NineKey('、', '\\'),
  new NineKey('？', '?'),
  new NineKey('！', '!'),
];

/**
 * 9 键主区域第一格 = **分词键**（Android 2026-10-15 裁决）。
 * 发送 `'`（协议值 = `scheme.rs::matches_segmented` 的音节边界分隔符）；
 * **空缓冲由 KeyboardController.tapSegment 拦住** —— 空缓冲的 `'` 是引号，不是分词符。
 */
const NINE_ROWS: NineKey[][] = [
  [new NineKey('分词', '\''), new NineKey('2', '2', 'abc'), new NineKey('3', '3', 'def')],
  [new NineKey('4', '4', 'ghi'), new NineKey('5', '5', 'jkl'), new NineKey('6', '6', 'mno')],
  [new NineKey('7', '7', 'pqrs'), new NineKey('8', '8', 'tuv'), new NineKey('9', '9', 'wxyz')],
];

const FOURTEEN_ROWS: string[][] = [
  ['qw', 'er', 'ty', 'ui', 'op'],
  ['as', 'df', 'gh', 'jk', 'l'],
  ['zx', 'cv', 'bn', 'm'],
];

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

// ---------------------------------------------------------------- 零件

/** 键位容器（9 键 / 14 键 / 笔画共用）。 */
@Component
export struct KeyBox {
  @Prop label: string = '';
  @Prop sub: string = '';
  @Prop mainSize: number = 21;
  @Prop subSize: number = 12;
  onTap: () => void = () => {
  };
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private fontOf(base: number): number {
    return this.elder === 1 ? base + 3 : base;
  }

  build() {
    Column() {
      if (this.sub.length === 0) {
        Text(this.label)
          .fontSize(this.fontOf(this.label.length > 1 ? this.mainSize - 6 : this.mainSize))
          .fontColor(this.pal().keyFg)
      } else {
        Text(this.sub).fontSize(this.fontOf(this.mainSize)).fontColor(this.pal().keyFg)
        Text(this.label).fontSize(this.fontOf(this.subSize)).fontColor(this.pal().dimFg)
      }
    }
    .justifyContent(FlexAlign.Center)
    .height('100%')
    .margin({ left: 2, right: 2 })
    .backgroundColor(this.pal().keyBg)
    .borderRadius(6)
    .onClick(this.onTap)
  }
}

/**
 * 右列 ⌫（9 键 / 平铺右列共用观感）：单击删一格；长按清空**整段**组合（不提交）。
 * 单独成件而不是给 KeyBox 加长按：长按手势会吃掉后续点击，挂在数字/标点键上会让
 * 那些键的长按变成「什么都不发生」。
 */
@Component
export struct BackspaceBox {
  @Prop mainSize: number = 17;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Column() {
      Text('⌫')
        .fontSize(this.elder === 1 ? this.mainSize + 3 : this.mainSize)
        .fontColor(this.pal().keyFg)
    }
    .justifyContent(FlexAlign.Center)
    .height('100%')
    .margin({ left: 2, right: 2 })
    .backgroundColor(this.pal().keyBg)
    .borderRadius(6)
    .onClick(() => {
      this.ctrl.tapBackspace();
    })
    .gesture(
      LongPressGesture({ repeat: false, duration: 500 })
        .onAction((event: GestureEvent) => {
          this.ctrl.tapBackspaceLongPress();
        })
    )
  }
}

// ---------------------------------------------------------------- 9 键 / 大 9 键

@Component
export struct NineKeyPanel {
  @Prop big: boolean = false;
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private mainSize(): number {
    return this.big ? 30 : 21;
  }

  private subSize(): number {
    return this.big ? 15 : 12;
  }

  /** 主区域第一格是分词键（发送 `'`），其余是 9 键数字类键。 */
  private tapNineKey(k: NineKey): void {
    if (k.key === '\'') {
      this.ctrl.tapSegment();
      return;
    }
    this.ctrl.tapChar(k.key);
  }

  build() {
    Column() {
      Row() {
        // 左列：标点（Android 输入时是候选拼音读数，C ABI 没有那个出口）。
        Column() {
          ForEach(NINE_STRIP, (k: NineKey) => {
            KeyBox({
              label: k.label, mainSize: this.mainSize(),
              onTap: () => {
                this.ctrl.tapChar(k.key);
              }
            }).layoutWeight(1)
          }, (k: NineKey) => k.key)
        }.layoutWeight(1)

        Column() {
          ForEach(NINE_ROWS, (row: NineKey[], rowIndex: number) => {
            Row() {
              ForEach(row, (k: NineKey) => {
                KeyBox({
                  label: k.label, sub: k.hint, mainSize: this.mainSize(), subSize: this.subSize(),
                  onTap: () => {
                    this.tapNineKey(k);
                  }
                }).layoutWeight(1)
              }, (k: NineKey) => k.key)
            }.layoutWeight(1)
          }, (row: NineKey[], rowIndex: number) => `nine-${rowIndex}`)
        }.layoutWeight(3)

        Column() {
          BackspaceBox({ mainSize: 17 }).layoutWeight(1)
          KeyBox({
            label: '重输', mainSize: 15,
            onTap: () => {
              this.ctrl.retype();
            }
          }).layoutWeight(1)
          KeyBox({
            label: '空格', mainSize: 15,
            onTap: () => {
              this.ctrl.tapSpace();
            }
          }).layoutWeight(1)
        }.layoutWeight(1)
      }.layoutWeight(3).margin({ top: 6 })

      // 符 / 中繁 / 123 / 英文 / ↵。
      Row() {
        this.Func('符', 1, () => {
          this.ctrl.openSymbolPage();
        })
        this.Func(traditionalModeLabel(this.mode), 1, () => {
          this.ctrl.toggleTraditional();
        })
        this.Func('123', 1, () => {
          this.ctrl.switchMode(OpiMode.Number);
        })
        this.Func('英文', 1, () => {
          this.ctrl.selectInputMode(MODE_ENGLISH26);
        })
        this.Func('↵', 1, () => {
          this.ctrl.tapEnter();
        })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%')
  }

  @Builder
  Func(label: string, weight: number, action: () => void) {
    Button(label)
      .fontSize(this.elder === 1 ? 17 : 15)
      .fontColor(this.pal().keyFg).backgroundColor(this.pal().keyBg)
      .borderRadius(6).layoutWeight(weight).height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(action)
  }
}

// ---------------------------------------------------------------- 14 键

@Component
export struct FourteenKeyPanel {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  @Builder
  Func(label: string, weight: number, action: () => void) {
    Button(label)
      .fontSize(this.elder === 1 ? 17 : 15)
      .fontColor(this.pal().keyFg).backgroundColor(this.pal().keyBg)
      .borderRadius(6).layoutWeight(weight).height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(action)
  }

  /** ⌫：单击删一格；长按清空整段组合（不提交）。 */
  @Builder
  BackspaceFunc() {
    Button('⌫')
      .fontSize(this.elder === 1 ? 17 : 15)
      .fontColor(this.pal().keyFg).backgroundColor(this.pal().keyBg)
      .borderRadius(6).layoutWeight(1).height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(() => {
        this.ctrl.tapBackspace();
      })
      .gesture(
        LongPressGesture({ repeat: false, duration: 500 })
          .onAction((event: GestureEvent) => {
            this.ctrl.tapBackspaceLongPress();
          })
      )
  }

  build() {
    Column() {
      ForEach(FOURTEEN_ROWS, (row: string[], rowIndex: number) => {
        Row() {
          ForEach(row, (pair: string) => {
            KeyBox({
              label: pair, mainSize: 22,
              onTap: () => {
                this.ctrl.tapChar(pair.charAt(0));
              }
            }).layoutWeight(1)
          }, (pair: string) => pair)
        }.layoutWeight(1)
      }, (row: string[], rowIndex: number) => `fp-${rowIndex}`)

      Row() {
        this.Func('符', 1, () => {
          this.ctrl.openSymbolPage();
        })
        this.Func(traditionalModeLabel(this.mode), 1, () => {
          this.ctrl.toggleTraditional();
        })
        this.Func('英文', 1, () => {
          this.ctrl.selectInputMode(MODE_ENGLISH26);
        })
        this.Func('空格', 2, () => {
          this.ctrl.tapSpace();
        })
        this.BackspaceFunc()
        this.Func('↵', 1, () => {
          this.ctrl.tapEnter();
        })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%').margin({ top: 6 })
  }
}

// ---------------------------------------------------------------- 笔画

@Component
export struct StrokeKeyPanel {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  @Builder
  Func(label: string, weight: number, action: () => void) {
    Button(label)
      .fontSize(this.elder === 1 ? 17 : 15)
      .fontColor(this.pal().keyFg).backgroundColor(this.pal().keyBg)
      .borderRadius(6).layoutWeight(weight).height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(action)
  }

  /** ⌫：单击删一格；长按清空整段组合（不提交）。 */
  @Builder
  BackspaceFunc() {
    Button('⌫')
      .fontSize(this.elder === 1 ? 17 : 15)
      .fontColor(this.pal().keyFg).backgroundColor(this.pal().keyBg)
      .borderRadius(6).layoutWeight(1).height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(() => {
        this.ctrl.tapBackspace();
      })
      .gesture(
        LongPressGesture({ repeat: false, duration: 500 })
          .onAction((event: GestureEvent) => {
            this.ctrl.tapBackspaceLongPress();
          })
      )
  }

  build() {
    Column() {
      Row() {
        ForEach(STROKE_KEYS, (k: StrokeKey) => {
          KeyBox({
            label: k.label, sub: k.name, mainSize: 26, subSize: 12,
            onTap: () => {
              this.ctrl.tapChar(k.key);
            }
          }).layoutWeight(1)
        }, (k: StrokeKey) => k.key)
      }.layoutWeight(1).margin({ top: 6 })

      Row() {
        this.Func('符', 1, () => {
          this.ctrl.openSymbolPage();
        })
        this.Func(traditionalModeLabel(this.mode), 1, () => {
          this.ctrl.toggleTraditional();
        })
        this.Func('英文', 1, () => {
          this.ctrl.selectInputMode(MODE_ENGLISH26);
        })
        this.Func('生僻字', 1.4, () => {
          this.ctrl.openRare();
        })
        this.Func('空格', 2, () => {
          this.ctrl.tapSpace();
        })
        this.BackspaceFunc()
        this.Func('↵', 1, () => {
          this.ctrl.tapEnter();
        })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%')
  }
}
