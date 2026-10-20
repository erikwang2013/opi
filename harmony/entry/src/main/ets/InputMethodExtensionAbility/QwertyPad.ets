// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 26 键族（拼音/英文/双拼/五笔）与数字面板的键面
// （Android QwertyKeyboard.kt + NumberPad.kt 的镜像）。
//
// · 中文 26 键：3 行字母 + 底行 123/中繁/空格/⌫/↵（无 ⇧ —— Android 中文侧不显示）。
// · 英文 26 键：0-9 数字行 + 3 行字母（第三行带 ⇧ 与 ↵）+ 底行 !?#/pro/空格/中/⌫。
// · 数字面板：1-9 三行 + 「, 0 .」行 + ABC/?123/空格/⌫/↵（Gboard 排法）。
//
// ⇧ 长按 = LOCK（`STATE_LONG_PRESSED`），补上了此前「枚举有、用户按不出来」的缺口。

import { KeyboardController } from './KeyboardController';
import { OpiMode, OpiShift } from './OpiEngine';
import { cycleModeLabel } from './InputModes';
import { Palette, paletteFor } from './Palette';

const ROW_1: string[] = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'];
const ROW_2: string[] = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'];
const ROW_3: string[] = ['⇧', 'z', 'x', 'c', 'v', 'b', 'n', 'm'];
const ENGLISH_ROW_3: string[] = ['⇧', 'z', 'x', 'c', 'v', 'b', 'n', 'm', '↵'];
const NUMBER_ROW: string[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
const NUMBER_PAD: string[][] = [
  ['1', '2', '3'], ['4', '5', '6'], ['7', '8', '9'], [',', '0', '.'],
];

@Component
export struct QwertyPad {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiShift') shift: number = OpiShift.Off as number;
  @StorageLink('opiPro') pro: number = 0;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private keySize(): number {
    return this.elder === 1 ? 21 : 18;
  }

  private shiftActive(): boolean {
    return this.shift !== (OpiShift.Off as number);
  }

  private shiftText(): string {
    return this.shift === (OpiShift.Lock as number) ? '⇪' : '⇧';
  }

  private isEnglish(): boolean {
    return this.mode === (OpiMode.English as number);
  }

  private letterRows(): string[][] {
    if (this.isEnglish()) {
      return [NUMBER_ROW, ROW_1, ROW_2, ENGLISH_ROW_3];
    }
    // 中文 ⇧ 不显示（Android QwertyKeyboard 的 onShift 默认 null）。
    return [ROW_1, ROW_2, ROW_3.slice(1)];
  }

  build() {
    if (this.mode === (OpiMode.Number as number)) {
      this.NumberPad()
    } else {
      this.Qwerty()
    }
  }

  // ------------------------------------------------------------ 字母盘

  @Builder
  Qwerty() {
    Column() {
      ForEach(this.letterRows(), (row: string[], rowIndex: number) => {
        Row() {
          ForEach(row, (ch: string) => {
            if (ch === '⇧') {
              this.ShiftKey()
            } else if (ch === '↵') {
              this.Key('↵', () => {
                this.ctrl.tapEnter();
              }, 1)
            } else {
              this.Key(this.isEnglish() && this.shiftActive() && rowIndex > 0
                ? ch.toUpperCase() : ch, () => {
                this.ctrl.tapChar(ch);
              }, 1)
            }
          }, (ch: string) => `${rowIndex}-${ch}`)
        }
        .width('100%').layoutWeight(1)
      }, (row: string[], rowIndex: number) => `q-${rowIndex}`)

      this.FunctionRow()
    }
    .width('100%').height('100%')
  }

  @Builder
  FunctionRow() {
    Row() {
      if (this.isEnglish()) {
        // 英文底行：`!?#` → 符号页；pro → 普通/专业；空格；中英；⌫。
        this.Key('!?#', () => {
          this.ctrl.openSymbolPage();
        }, 1)
        this.Key(this.pro === 1 ? 'pro ✓' : 'pro', () => {
          this.ctrl.toggleEnglishPro();
        }, 1, this.pro === 1)
        this.Key('空格', () => {
          this.ctrl.tapSpace();
        }, 4)
        this.Key(cycleModeLabel(this.mode), () => {
          this.ctrl.cycleMode();
        }, 1)
        this.Key('⌫', () => {
          this.ctrl.tapBackspace();
        }, 1)
      } else {
        // 中文底行：123 / 中繁 / 空格 / ⌫ / ↵。
        this.Key('123', () => {
          this.ctrl.switchMode(OpiMode.Number);
        }, 1)
        this.Key(cycleModeLabel(this.mode), () => {
          this.ctrl.cycleMode();
        }, 1)
        this.Key('空格', () => {
          this.ctrl.tapSpace();
        }, 4)
        this.Key('⌫', () => {
          this.ctrl.tapBackspace();
        }, 1)
        this.Key('↵', () => {
          this.ctrl.tapEnter();
        }, 1)
      }
    }
    .width('100%').layoutWeight(1).margin({ top: 2, bottom: 2 })
  }

  // ------------------------------------------------------------ 数字面板

  @Builder
  NumberPad() {
    Column() {
      ForEach(NUMBER_PAD, (row: string[], rowIndex: number) => {
        Row() {
          ForEach(row, (ch: string) => {
            this.Key(ch, () => {
              this.ctrl.tapChar(ch);
            }, 1)
          }, (ch: string) => `np-${rowIndex}-${ch}`)
        }
        .width('100%').layoutWeight(1)
      }, (row: string[], rowIndex: number) => `np-row-${rowIndex}`)

      Row() {
        this.Key('ABC', () => {
          this.ctrl.cycleToChinese();
        }, 1)
        this.Key('?123', () => {
          this.ctrl.openSymbolPage();
        }, 1)
        this.Key('空格', () => {
          this.ctrl.tapSpace();
        }, 5)
        this.Key('⌫', () => {
          this.ctrl.tapBackspace();
        }, 1)
        this.Key('↵', () => {
          this.ctrl.tapEnter();
        }, 1)
      }
      .width('100%').layoutWeight(1).margin({ top: 2, bottom: 2 })
    }
    .width('100%').height('100%')
  }

  // ------------------------------------------------------------ 零件

  @Builder
  Key(label: string, action: () => void, weight: number = 1, active: boolean = false) {
    Button(label)
      .fontSize(this.keySize())
      .fontColor(active ? this.pal().highlightFg : this.pal().keyFg)
      .backgroundColor(active ? this.pal().highlightBg : this.pal().keyBg)
      .borderRadius(6)
      .layoutWeight(weight)
      .height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(action)
  }

  @Builder
  ShiftKey() {
    Button(this.shiftText())
      .fontSize(this.keySize())
      .fontColor(this.shiftActive() ? this.pal().highlightFg : this.pal().keyFg)
      .backgroundColor(this.shiftActive() ? this.pal().highlightBg : this.pal().keyBg)
      .borderRadius(6)
      .layoutWeight(1)
      .height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(() => {
        this.ctrl.tapShift();
      })
      .gesture(
        LongPressGesture({ repeat: false, duration: 500 })
          .onAction((event: GestureEvent) => {
            this.ctrl.tapShiftLongPress();
          })
      )
  }
}
