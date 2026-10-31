// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 9 键左列「自定义」标点的选择器（Android CustomSymbolPicker.kt 的镜像）：
// 常用符号网格，点一格 = 选中并关闭。画在键区之上（同层覆盖，不是系统 Dialog ——
// IME 窗口里的 Dialog 要另起窗口，真机焦点/尺寸都不可控）。

import { Palette, paletteFor, scenePanelBg } from './Palette';
import { symbolCategories } from './SymbolData';

const PER_ROW: number = 8;

/** 常用符号 = 符号表「常用」类目（缺类目时退回第一类）；去重保序。 */
function commonSymbols(): string[] {
  const categories = symbolCategories();
  if (categories.length === 0) {
    return [];
  }
  let source: string[] = categories[0].symbols;
  for (let i = 0; i < categories.length; i++) {
    if (categories[i].label === '常用') {
      source = categories[i].symbols;
      break;
    }
  }
  const out: string[] = [];
  for (let i = 0; i < source.length; i++) {
    if (out.indexOf(source[i]) < 0) {
      out.push(source[i]);
    }
  }
  return out;
}

@Component
export struct CustomSymbolPicker {
  onPick: (symbol: string) => void = () => {
  };
  onClose: () => void = () => {
  };
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private symbols: string[] = [];

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  aboutToAppear(): void {
    this.symbols = commonSymbols();
  }

  /** 每行固定 8 格，末行用空串补满（空格子不画底色、点了不动作）。 */
  private rows(): string[][] {
    const out: string[][] = [];
    for (let i = 0; i < this.symbols.length; i += PER_ROW) {
      const row: string[] = this.symbols.slice(i, i + PER_ROW);
      while (row.length < PER_ROW) {
        row.push('');
      }
      out.push(row);
    }
    return out;
  }

  build() {
    Column() {
      Row() {
        Button('返回')
          .fontSize(this.elder === 1 ? 19 : 17)
          .fontColor(this.pal().keyFg).backgroundColor(this.pal().keyBg)
          .borderRadius(6).layoutWeight(1).height('100%')
          .margin({ left: 2, right: 2 })
          .onClick(() => {
            this.onClose();
          })
        Text('选择自定义符号')
          .fontSize(this.elder === 1 ? 18 : 15).fontColor(this.pal().panelFg)
          .layoutWeight(4).padding({ left: 8 })
      }
      .width('100%').height(44)

      if (this.symbols.length === 0) {
        Text('暂无可用符号').fontSize(15).fontColor(this.pal().dimFg).layoutWeight(1)
      } else {
        Scroll() {
          Column() {
            ForEach(this.rows(), (row: string[], rowIndex: number) => {
              Row() {
                ForEach(row, (symbol: string, col: number) => {
                  Stack({ alignContent: Alignment.Center }) {
                    Text(symbol).fontSize(this.elder === 1 ? 24 : 20)
                      .fontColor(this.pal().keyFg)
                  }
                  .layoutWeight(1).height(40).margin(2).borderRadius(6)
                  .backgroundColor(symbol.length > 0 ? this.pal().keyBg : this.pal().panelBg)
                  .onClick(() => {
                    if (symbol.length > 0) {
                      this.onPick(symbol);
                    }
                  })
                }, (symbol: string, col: number) => `${rowIndex}-${col}-${symbol}`)
              }.width('100%')
            }, (row: string[], rowIndex: number) => `sym-${rowIndex}-${row[0]}`)
          }.width('100%')
        }
        .scrollable(ScrollDirection.Vertical)
        .scrollBar(BarState.Off)
        .layoutWeight(1).width('100%')
      }
    }
    .width('100%').height('100%').backgroundColor(scenePanelBg(this.pal().panelBg))
    .onClick(() => {
      // 吞掉空白处点击，别漏到下面的键。
    })
  }
}
