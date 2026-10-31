// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 搜索叠盘（Android ImeScreen 搜索态在面板下方叠的 QwertyKeyboard，96vp = 4 行）。
// 鸿蒙 IME 面板里的 TextInput 唤不起第二个输入法，搜索文本改由本盘直接编辑；
// 键位只留搜索用得到的语义：字母 / 空格 / ⌫ / ↵（收起叠盘，查询与结果保留）。

import { Palette, paletteFor, scenePanelBg } from './Palette';

const ROWS: string[][] = [
  ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
  ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
  ['z', 'x', 'c', 'v', 'b', 'n', 'm'],
];
/** 4 行等分 96vp（Android 同款；Material 最小触控目标以下，空间所限的折中）。
 *  行高由 layoutWeight 分，别再写死 24：4×24 + 上下 padding 会顶出 4vp。 */

@Component
export struct SearchPad {
  onKey: (ch: string) => void = () => {
  };
  onBackspace: () => void = () => {
  };
  onSpace: () => void = () => {
  };
  onEnter: () => void = () => {
  };
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Column() {
      ForEach(ROWS, (row: string[], rowIndex: number) => {
        Row() {
          ForEach(row, (ch: string) => {
            this.Key(ch, () => {
              this.onKey(ch);
            }, 1)
          }, (ch: string) => `s-${rowIndex}-${ch}`)
          if (rowIndex === 2) {
            this.Key('⌫', this.onBackspace, 1.5)
          }
        }.width('100%').layoutWeight(1)
      }, (row: string[], rowIndex: number) => `s-row-${rowIndex}`)
      Row() {
        this.Key('空格', this.onSpace, 4)
        this.Key('↵', this.onEnter, 1)
      }.width('100%').layoutWeight(1)
    }
    .width('100%').height('100%').backgroundColor(scenePanelBg(this.pal().barBg))
    .padding({ left: 2, right: 2, top: 2, bottom: 2 })
  }

  @Builder
  Key(label: string, action: () => void, weight: number) {
    Button(label)
      .fontSize(14).fontColor(this.pal().keyFg)
      .backgroundColor(this.pal().keyBg).borderRadius(5)
      .layoutWeight(weight).height('100%')
      .margin({ left: 1, right: 1, top: 1, bottom: 1 })
      .onClick(action)
  }
}
