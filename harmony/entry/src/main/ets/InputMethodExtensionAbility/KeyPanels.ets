// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 输入模式各自的键面（Android keyboard/ 下同名文件的镜像，2026-10-24 参考图改版）：
//   9 键 / 大 9 键 · 14 键。
// 26 键族与数字面板在 QwertyPad.ets；笔画（含 KeyBox）在 StrokePanel.ets；
// 生僻字/符号页在 SymbolPage.ets；输入模式面板在 Panels.ets。
//
// 键表是协议：9 键发送 `2..9`（类表见 scheme.rs），14 键发送字母对首字母。
// 底行口径（2026-10-15 Android 裁决）：9/笔画的中/繁键**只切简繁**，进英文走
// 独立的 [英] 键；[符] 打开符号页（不是表情页）。14 键底行 2026-10-25 起是「英」。

import { KeyboardController } from './KeyboardController';
import { lastChineseModeIndex, MODE_ENGLISH26, traditionalModeLabel } from './InputModes';
import { OpiMode, OpiShift } from './OpiEngine';
import { Palette, paletteFor } from './Palette';
import { BackspaceButton, FuncButton, PopupChoice, PopupKey, ShiftButton } from './KeyPopup';
import { CustomSymbolPicker } from './SymbolPicker';
import { fourteenChoices, fourteenSymbol, nineChoices } from './KeyChoices';
import * as SettingsStore from './SettingsStore';

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

/** 左列空闲时的标点列（键值都取引擎中文标点表已有的键；输入时整列换成拼音读数）。 */
export const NINE_STRIP: NineKey[] = [
  new NineKey('，', ','), new NineKey('。', '.'), new NineKey('、', '\\'),
  new NineKey('？', '?'), new NineKey('！', '!'), new NineKey('：', ':'),
  new NineKey('；', ';'), new NineKey('（', '('), new NineKey('）', ')'),
  new NineKey('“', '"'), new NineKey('‘', '\''), new NineKey('【', '['),
  new NineKey('】', ']'), new NineKey('《', '<'), new NineKey('》', '>'),
  new NineKey('～', '~'),
];

/**
 * 9 键主区域：第一格 = **分词键**（发送 `'`；空缓冲由 tapSegment 拦住），
 * 其余键发送 `2..9`。label 是数字协议值，hint 是键面字母。
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
const FOURTEEN_DIGITS: string = '1234567890';

// ---------------------------------------------------------------- 9 键 / 大 9 键

@Component
export struct NineKeyPanel {
  @Prop big: boolean = false;
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiBuffer') @Watch('onBufferChanged') buffer: string = '';
  @StorageLink('opiReadings') readings: string[] = [];
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @State pinned: string = '';
  @State custom: string = '';
  @State pickerOpen: boolean = false;
  /** 长按已接管本轮按下：抬手时别再让 onClick 发送旧自定义符号。 */
  private customLong: boolean = false;
  private stripScroller: Scroller = new Scroller();
  private readScroller: Scroller = new Scroller();
  /** 读数 Scroll 当前是否挂上（未挂时调 Scroller 无意义，也别碰空控制器）。 */
  private readBound: boolean = false;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private mainSize(): number {
    return this.big ? 30 : 21;
  }

  private keyFont(): number {
    return this.elder === 1 ? 24 : 21;
  }

  /** 左标点列字号（2026-10-25：比主区小一档，用户反馈标点键太大）。 */
  private stripSize(): number {
    return this.elder === 1 ? 18 : 17;
  }

  /** 左列每格高度：可视约三行，条目多于三行时整列可滚（Android maxHeight/3 同档）。 */
  private stripCell(): number {
    return 58;
  }

  aboutToAppear(): void {
    this.custom = SettingsStore.customPunc();
  }

  /** 缓冲一变读数就换批：钉住的读音作废、整列回顶部（引擎侧同缓冲内有效）。 */
  onBufferChanged(): void {
    if (this.pinned.length > 0) {
      this.pinned = '';
      this.ctrl.clearPinnedReading();
    }
    if (this.readBound && this.readings.length > 0) {
      this.readScroller.scrollEdge(Edge.Top);
    }
  }

  private tapNineKey(k: NineKey): void {
    if (k.key === '\'') {
      this.ctrl.tapSegment();
      return;
    }
    this.ctrl.tapChar(k.key);
  }

  /** 点读音 = 钉住（候选收窄到恰等）；同一行再点解除。 */
  private pickReading(reading: string): void {
    const next: string = this.pinned === reading ? '' : reading;
    this.pinned = next;
    this.ctrl.pinReading(next);
  }

  private openCustom(): void {
    if (this.custom.length === 0) {
      this.pickerOpen = true;
      return;
    }
    this.ctrl.commitText(this.custom);
  }

  @Builder
  ReadingColumn() {
    Scroll(this.readScroller) {
      Column() {
        ForEach(this.readings, (reading: string, index: number) => {
          Text(reading)
            .fontSize(this.elder === 1 ? 17 : 15)
            .fontColor(this.pinned === reading ? this.pal().accent : this.pal().dimFg)
            .fontWeight(this.pinned === reading ? FontWeight.Bold : FontWeight.Normal)
            .width('100%').height(this.stripCell()).textAlign(TextAlign.Center)
            .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
            .onClick(() => {
              this.pickReading(reading);
            })
        }, (reading: string, index: number) => `read-${index}-${reading}`)
      }.width('100%')
    }
    .scrollable(ScrollDirection.Vertical).scrollBar(BarState.Off)
    .layoutWeight(1).width('100%')
    .onAppear(() => {
      this.readBound = true;
    })
    .onDisAppear(() => {
      this.readBound = false;
    })
  }

  @Builder
  StripColumn() {
    Scroll(this.stripScroller) {
      Column() {
        ForEach(NINE_STRIP, (k: NineKey) => {
          Row() {
            Button(k.label)
              .fontSize(this.stripSize()).fontColor(this.pal().keyFg)
              .backgroundColor(this.pal().keyBg).borderRadius(6)
              .layoutWeight(1).height('100%').margin({ left: 2, right: 2 })
              .onClick(() => {
                this.ctrl.tapChar(k.key);
              })
          }.width('100%').height(this.stripCell())
        }, (k: NineKey) => k.key)
        // 末尾自定义格：未选 = 点按开选择器；已选 = 点按输入、长按更换。
        Row() {
          Button(this.custom.length > 0 ? this.custom : '自定义')
            .fontSize(this.custom.length > 0 ? this.stripSize() : 14)
            .fontColor(this.pal().keyFg).backgroundColor(this.pal().keyBg)
            .borderRadius(6).layoutWeight(1).height('100%').margin({ left: 2, right: 2 })
            .onClick(() => {
              if (this.customLong) {
                this.customLong = false;
                return;
              }
              this.openCustom();
            })
            .gesture(
              LongPressGesture({ repeat: false, duration: 500 })
                .onAction(() => {
                  this.customLong = true;
                  this.pickerOpen = true;
                })
            )
        }.width('100%').height(this.stripCell())
      }.width('100%')
    }
    .scrollable(ScrollDirection.Vertical).scrollBar(BarState.Off)
    .layoutWeight(1).width('100%')
  }

  @Builder
  Grid() {
    Column() {
      ForEach(NINE_ROWS, (row: NineKey[], rowIndex: number) => {
        Row() {
          ForEach(row, (k: NineKey, colIndex: number) => {
            PopupKey({
              label: k.hint.length > 0 ? k.hint : k.label,
              // 数字徽标按格序 1..9（分词格也有）；大 9 键不画徽标。
              corner: this.big ? '' : `${rowIndex * 3 + colIndex + 1}`,
              mainSize: this.mainSize(),
              hintSize: this.big ? 15 : 12,
              placementTop: rowIndex > 0,
              choices: k.hint.length > 0
                ? nineChoices(k.hint, k.key, k.label, this.mainSize(), this.big ? 15 : 12) : [],
              defaultChoice: k.hint.length,
              onTap: () => {
                this.tapNineKey(k);
              },
              onChoice: (choice: PopupChoice) => {
                if (choice.direct) {
                  this.ctrl.commitText(choice.value);
                } else {
                  this.ctrl.tapChar(choice.value);
                }
              }
            }).layoutWeight(1)
          }, (k: NineKey) => k.key)
        }.layoutWeight(1)
      }, (row: NineKey[], rowIndex: number) => `nine-${rowIndex}-${this.big}`)
    }.layoutWeight(3)
  }

  build() {
    Stack() {
      Column() {
        Row() {
          Column() {
            if (this.readings.length > 0) {
              this.ReadingColumn()
            } else {
              this.StripColumn()
            }
          }.layoutWeight(0.8)
          this.Grid()
          Column() {
            BackspaceButton({ fontSize: this.keyFont(), weight: 1 })
            FuncButton({
              label: '重输', fontSize: this.keyFont(),
              onTap: () => {
                this.ctrl.retype();
              }
            })
            FuncButton({
              // 2026-10-25：右列空格换成 0 键，空格移到底行中间并加宽。
              label: '0', fontSize: this.keyFont(),
              onTap: () => {
                this.ctrl.tapChar('0');
              }
            })
          }.layoutWeight(1)
        }.layoutWeight(3).margin({ top: 6 })

        // 符 / 中繁 / 123 / 空格(宽) / 英 / ↵。
        Row() {
          FuncButton({
            label: '符', fontSize: this.keyFont(), weight: 0.9,
            onTap: () => {
              this.ctrl.openSymbolPage();
            }
          })
          FuncButton({
            label: traditionalModeLabel(this.mode), fontSize: this.keyFont(), weight: 0.9,
            onTap: () => {
              this.ctrl.toggleTraditional();
            }
          })
          FuncButton({
            label: '123', fontSize: this.keyFont(), weight: 0.9,
            onTap: () => {
              this.ctrl.switchMode(OpiMode.Number);
            }
          })
          FuncButton({
            label: '空格', fontSize: this.keyFont(), weight: 2.4,
            onTap: () => {
              this.ctrl.tapSpace();
            }
          })
          FuncButton({
            label: '英', fontSize: this.keyFont(), weight: 0.9,
            onTap: () => {
              this.ctrl.selectInputMode(MODE_ENGLISH26);
            }
          })
          FuncButton({
            label: '↵', fontSize: this.keyFont(),
            onTap: () => {
              this.ctrl.tapEnter();
            }
          })
        }.layoutWeight(1).margin({ top: 6, bottom: 8 })
      }.width('100%').height('100%')

      if (this.pickerOpen) {
        CustomSymbolPicker({
          onPick: (symbol: string) => {
            this.custom = symbol;
            this.pickerOpen = false;
            this.customLong = false;
            SettingsStore.saveCustomPunc(symbol);
          },
          onClose: () => {
            this.pickerOpen = false;
            // 长按后若没有补发 onClick，旗标会留到下一次点击；随选择器关闭清掉。
            this.customLong = false;
          }
        })
      }
    }.width('100%').height('100%')
  }
}

// ---------------------------------------------------------------- 14 键

@Component
export struct FourteenKeyPanel {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiShift') shift: number = OpiShift.Off as number;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pairSize(): number {
    return this.elder === 1 ? 27 : 24;
  }

  private keyFont(): number {
    return this.elder === 1 ? 24 : 21;
  }

  private bottomFont(): number {
    return this.elder === 1 ? 21 : 18;
  }

  private shiftActive(): boolean {
    return this.shift !== (OpiShift.Off as number);
  }

  /** 单击发协议字母（⇧ 亮时发大写，引擎把拼音大写折成小写）。 */
  private tapPair(pair: string): void {
    const first: string = pair.charAt(0);
    this.ctrl.tapChar(this.shiftActive() ? first.toUpperCase() : first);
  }

  private hintFor(pair: string, rowIndex: number, index: number): string {
    if (rowIndex === 0) {
      return FOURTEEN_DIGITS.charAt(index);
    }
    if (rowIndex === 1) {
      return FOURTEEN_DIGITS.charAt(5 + index);
    }
    return fourteenSymbol(pair);
  }

  /** 中英：中文 → 英文 26 键；英文 → 回上次中文方案（不是固定 26 键）。 */
  private toggleChineseEnglish(): void {
    if (this.mode === (OpiMode.English as number)) {
      this.ctrl.selectInputMode(lastChineseModeIndex());
    } else {
      this.ctrl.selectInputMode(MODE_ENGLISH26);
    }
  }

  build() {
    Column() {
      ForEach(FOURTEEN_ROWS, (row: string[], rowIndex: number) => {
        Row() {
          if (rowIndex === 2) {
            ShiftButton({ fontSize: this.keyFont(), weight: 1.2, usable: true })
          }
          ForEach(row, (pair: string, index: number) => {
            PopupKey({
              label: pair.toUpperCase(),
              hint: this.hintFor(pair, rowIndex, index),
              mainSize: this.pairSize(),
              hintSize: this.elder === 1 ? 13 : 11,
              placementTop: rowIndex > 0,
              choices: fourteenChoices(pair, this.pairSize(), this.hintFor(pair, rowIndex, index)),
              defaultChoice: 1,
              onTap: () => {
                this.tapPair(pair);
              },
              onChoice: (choice: PopupChoice) => {
                if (choice.direct) {
                  this.ctrl.commitText(choice.value);
                } else {
                  this.ctrl.tapChar(choice.value);
                }
              }
            }).layoutWeight(rowIndex === 2 ? (pair === 'm' ? 0.95 : 1.55) : 1)
          }, (pair: string) => pair)
          if (rowIndex === 2) {
            BackspaceButton({ fontSize: this.keyFont(), weight: 1.2 })
          }
        }.layoutWeight(1)
      }, (row: string[], rowIndex: number) => `fp-${rowIndex}`)

      // 符 | 123 | , | 空格（宽）| 。| 英 | 换行。
      Row() {
        FuncButton({
          label: '符', fontSize: this.bottomFont(), weight: 1.3,
          onTap: () => {
            this.ctrl.openSymbolPage();
          }
        })
        FuncButton({
          label: '123', fontSize: this.bottomFont(), weight: 1.3,
          onTap: () => {
            this.ctrl.switchMode(OpiMode.Number);
          }
        })
        FuncButton({
          label: ',', fontSize: this.keyFont(),
          onTap: () => {
            this.ctrl.tapChar(',');
          }
        })
        FuncButton({
          label: '空格', fontSize: this.bottomFont(), weight: 2.7,
          onTap: () => {
            this.ctrl.tapSpace();
          }
        })
        FuncButton({
          label: '。', fontSize: this.keyFont(),
          onTap: () => {
            // 发半角 `.`：中文标点层映射成「。」（Android 14 键同款）。
            this.ctrl.tapChar('.');
          }
        })
        FuncButton({
          label: '英', fontSize: this.bottomFont(), weight: 1.3,
          onTap: () => {
            this.toggleChineseEnglish();
          }
        })
        FuncButton({
          label: '换行', fontSize: this.bottomFont(), weight: 1.3,
          onTap: () => {
            this.ctrl.tapEnter();
          }
        })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%').height('100%')
  }
}
