// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 26 键族（拼音/英文/双拼/五笔）与数字面板的键面
// （Android QwertyKeyboard.kt + NumberPad.kt 的镜像，2026-10-24 参考图 aa/cc）。
//
// · 拼音 26 键：**无数字行**；Q..P 右上角 1-0 角标；A/Z 行上方符号提示；
//   字母键面固定大写、发送值仍是原字符（引擎把拼音大写折成小写）；
//   底行 符 / 123 / , / 空格 / 。 / 中英 / 换行。
// · 英文 26 键：顶部 0-9 数字行 + 3 行字母（小写，随 ⇧ 变大写）；
//   所有字母上方符号提示；底行 !?# / 123 / , / 空格 / . / 中英 / 换行。
// · 字母键长按/上滑出备选弹层：大写、符号（拼音 Q 行是角标数字）、小写，
//   默认选中中间项（符号；Q 行是数字）。
// · 数字面板：1-9 三行 + 「, 0 .」行 + ABC/?123/空格/⌫/↵（Gboard 排法）。

import { KeyboardController } from './KeyboardController';
import { OpiMode, OpiShift } from './OpiEngine';
import { MODE_ENGLISH26, MODE_PINYIN26, MODE_WUBI, lastChineseModeIndex } from './InputModes';
import { BackspaceButton, FuncButton, PopupChoice, PopupKey, ShiftButton } from './KeyPopup';

const ROW_1: string[] = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'];
const ROW_2: string[] = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'];
const ROW_3: string[] = ['z', 'x', 'c', 'v', 'b', 'n', 'm'];
const NUMBER_ROW: string[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
const NUMBER_PAD: string[][] = [
  ['1', '2', '3'], ['4', '5', '6'], ['7', '8', '9'], [',', '0', '.'],
];
/** 26 键字母键上方的符号提示（aa.jpg 的映射，逐字符与 [HINT_CHARS] 对齐）。 */
const HINT_CHARS: string = 'qwertyuiopasdfghjklzxcvbnm';
const HINT_SYMBOLS: string = '`=+$…"^[]|~!@#%\'&*?()-:;/';
/** 拼音 26 键 Q..P 右上角的数字 1-0。 */
const DIGITS: string = '1234567890';

@Component
export struct QwertyPad {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiShift') shift: number = OpiShift.Off as number;
  @StorageLink('opiElder') elder: number = 0;
  /** 输入模式下标（InputModes.INPUT_MODES 的下标，不是 OpiMode）。五笔键面两处差异用它判。 */
  @StorageLink('opiLayout') layout: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private isEnglish(): boolean {
    return this.mode === (OpiMode.English as number);
  }

  private isWubi(): boolean {
    return this.layout === MODE_WUBI;
  }

  private shiftActive(): boolean {
    return this.shift !== (OpiShift.Off as number);
  }

  private keySize(): number {
    // Android 26 键 keyFont = 21sp（英文 23sp）；长辈模式加 3。
    return this.elder === 1 ? (this.isEnglish() ? 26 : 24) : (this.isEnglish() ? 23 : 21);
  }

  private hintSize(): number {
    return this.elder === 1 ? 12 : 10;
  }

  /** 键面字：拼音固定大写；英文随 ⇧（发送值不变，大小写由引擎/⇧ 语义决定）。 */
  private labelFor(ch: string): string {
    return this.isEnglish() && !this.shiftActive() ? ch : ch.toUpperCase();
  }

  private symbolHint(ch: string): string {
    const i: number = HINT_CHARS.indexOf(ch);
    return i < 0 ? '' : HINT_SYMBOLS.charAt(i);
  }

  /** 键面上方小字；拼音 Q 行的数字在右上角，不占提示行。 */
  private hintFor(ch: string, rowIndex: number): string {
    if (this.isEnglish()) {
      return this.symbolHint(ch);
    }
    if (rowIndex === 0) {
      return '';
    }
    if (ch === 'h') {
      return '“';
    }
    if (ch === 'j') {
      return '”';
    }
    return this.symbolHint(ch);
  }

  private cornerFor(ch: string, rowIndex: number): string {
    if (this.isEnglish() || rowIndex !== 0) {
      return '';
    }
    const i: number = ROW_1.indexOf(ch);
    return i < 0 ? '' : DIGITS.charAt(i);
  }

  /** 备选：[大写, 符号(拼音 Q 行 = 数字), 小写]；默认中间项。 */
  private choicesFor(ch: string, rowIndex: number): PopupChoice[] {
    const upper: string = ch.toUpperCase();
    let middle: string = this.symbolHint(ch);
    if (!this.isEnglish()) {
      if (rowIndex === 0) {
        middle = this.cornerFor(ch, 0);
      } else if (ch === 'h') {
        middle = '“';
      } else if (ch === 'j') {
        middle = '”';
      }
    }
    return [
      // 大写/符号(数字) 直接原样上屏；小写进引擎匹配词库。
      new PopupChoice(upper, upper, 0, true),
      new PopupChoice(middle, middle, 0, true),
      new PopupChoice(ch, ch),
    ];
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
      if (this.isEnglish()) {
        this.NumberRow()
      }
      this.LetterRow(ROW_1, 0)
      this.LetterRow(ROW_2, 1)
      this.LetterRow(ROW_3, 2)
      this.BottomRow()
    }
    .width('100%').height('100%')
  }

  @Builder
  NumberRow() {
    Row() {
      ForEach(NUMBER_ROW, (ch: string) => {
        FuncButton({
          label: ch, fontSize: this.keySize(),
          onTap: () => {
            this.ctrl.tapChar(ch);
          }
        })
      }, (ch: string) => `num-${ch}`)
    }
    .width('100%').layoutWeight(0.7)
  }

  @Builder
  LetterRow(keys: string[], rowIndex: number) {
    Row() {
      if (rowIndex === 2) {
        if (this.isWubi()) {
          // 五笔键面（ads.jpg）：⇧ 位换成「拼」，走输入模式状态机切拼音 26（非只改 UI）。
          FuncButton({
            label: '拼', fontSize: this.keySize(), weight: 1,
            onTap: () => {
              this.ctrl.selectInputMode(MODE_PINYIN26);
            }
          })
        } else {
          // 中文侧 ⇧ 是死键（Android onShift=null 的同款形态）。
          ShiftButton({ fontSize: this.keySize(), weight: 1, usable: this.isEnglish() })
        }
      }
      ForEach(keys, (ch: string) => {
        PopupKey({
          label: this.labelFor(ch),
          hint: this.hintFor(ch, rowIndex),
          corner: this.cornerFor(ch, rowIndex),
          mainSize: this.keySize(),
          hintSize: this.hintSize(),
          // 拼音首行在键盘最上沿：弹层朝下；其余朝上（26 键三行备选都放得下）。
          placementTop: this.isEnglish() || rowIndex > 0,
          choices: this.choicesFor(ch, rowIndex),
          defaultChoice: 1,
          onTap: () => {
            this.ctrl.tapChar(ch);
          },
          onChoice: (choice: PopupChoice) => {
            if (choice.direct) {
              this.ctrl.commitText(choice.value);
            } else {
              this.ctrl.tapChar(choice.value);
            }
          }
        }).layoutWeight(1)
      }, (ch: string) => `${rowIndex}-${ch}-${this.mode}`)
      if (rowIndex === 2) {
        BackspaceButton({ fontSize: this.keySize(), weight: 1 })
      }
    }
    .width('100%')
    .layoutWeight(this.isEnglish() ? 1.05 : 1)
  }

  @Builder
  BottomRow() {
    Row() {
      // 英文 !?# / 中文 符：都进符号页；空格两侧的标点按参考图区分（. / 。）。
      FuncButton({
        label: this.isEnglish() ? '!?#' : '符', fontSize: this.keySize(),
        onTap: () => {
          this.ctrl.openSymbolPage();
        }
      })
      FuncButton({
        label: '123', fontSize: this.keySize(),
        onTap: () => {
          this.ctrl.switchMode(OpiMode.Number);
        }
      })
      FuncButton({
        label: ',', fontSize: this.keySize(),
        onTap: () => {
          this.ctrl.tapChar(',');
        }
      })
      FuncButton({
        // 五笔键面空格键显示「五笔」（ads.jpg）；动作仍是空格。
        label: this.isWubi() ? '五笔' : '空格', fontSize: this.keySize(), weight: 2.5,
        onTap: () => {
          this.ctrl.tapSpace();
        }
      })
      FuncButton({
        label: this.isEnglish() ? '.' : '。', fontSize: this.keySize(),
        onTap: () => {
          this.ctrl.tapChar(this.isEnglish() ? '.' : '。');
        }
      })
      FuncButton({
        // 2026-10-25：键面 = 目标语言（中文面「英」、英文面「中」），一次点击直达；
        // 不再走 中→繁→英 三态循环（繁体仍可从输入模式面板选）。
        label: this.isEnglish() ? '中' : '英', fontSize: this.keySize(),
        onTap: () => {
          if (this.isEnglish()) {
            this.ctrl.selectInputMode(lastChineseModeIndex());
          } else {
            this.ctrl.selectInputMode(MODE_ENGLISH26);
          }
        }
      })
      FuncButton({
        label: '换行', fontSize: this.keySize(),
        onTap: () => {
          this.ctrl.tapEnter();
        }
      })
    }
    .width('100%').layoutWeight(this.isEnglish() ? 0.95 : 1).margin({ top: 2, bottom: 2 })
  }

  // ------------------------------------------------------------ 数字面板

  @Builder
  NumberPad() {
    Column() {
      ForEach(NUMBER_PAD, (row: string[], rowIndex: number) => {
        Row() {
          ForEach(row, (ch: string) => {
            FuncButton({
              label: ch, fontSize: this.keySize(),
              onTap: () => {
                this.ctrl.tapChar(ch);
              }
            })
          }, (ch: string) => `np-${rowIndex}-${ch}`)
        }
        .width('100%').layoutWeight(1)
      }, (row: string[], rowIndex: number) => `np-row-${rowIndex}`)

      Row() {
        FuncButton({
          label: 'ABC', fontSize: this.keySize(),
          onTap: () => {
            this.ctrl.cycleToChinese();
          }
        })
        FuncButton({
          label: '?123', fontSize: this.keySize(),
          onTap: () => {
            this.ctrl.openSymbolPage();
          }
        })
        FuncButton({
          label: '空格', fontSize: this.keySize(), weight: 5,
          onTap: () => {
            this.ctrl.tapSpace();
          }
        })
        BackspaceButton({ fontSize: this.keySize(), weight: 1 })
        FuncButton({
          label: '↵', fontSize: this.keySize(),
          onTap: () => {
            this.ctrl.tapEnter();
          }
        })
      }
      .width('100%').layoutWeight(1).margin({ top: 2, bottom: 2 })
    }
    .width('100%').height('100%')
  }
}
