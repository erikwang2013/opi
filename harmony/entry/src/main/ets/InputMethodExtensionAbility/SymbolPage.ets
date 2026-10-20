// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 符号页 + 生僻字面板（Android `keyboard/SymbolPage.kt` / `RarePanel.kt` 的镜像）。
//
// 两张表都**不经过引擎**：数据来自 rawfile TSV（SymbolData.ets 解析），点格子直接
// `insertText`。Android 同款（RARE/SYMBOL_PAGE 视图不碰引擎 scheme/mode）。
//
// 与 Android 的已知差异：符号页翻页用页头 ‹ › 而不是左右滑动 —— ArkUI 的
// Swiper/手势在本机无 SDK 可验证，选可编译的按钮形态（见 docs/install/harmony.md）。

import { KeyboardController } from './KeyboardController';
import { PanelHeader } from './Panels';
import { Palette, paletteFor } from './Palette';
import {
  SymbolCategory, radicalGlyph, rareChars, rareIndex, rareStrokeCounts, symbolCategories,
} from './SymbolData';

const SYMBOL_PER_ROW: number = 8;
const SYMBOL_ROWS: number = 4;
const SYMBOL_PAGE_SIZE: number = SYMBOL_PER_ROW * SYMBOL_ROWS;
const INDEX_COLS: number = 8;

function blanks(n: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    out.push(i);
  }
  return out;
}

function rowsOfRange(from: number, to: number, cols: number): number[][] {
  const out: number[][] = [];
  let row: number[] = [];
  for (let i = from; i <= to; i++) {
    row.push(i);
    if (row.length === cols) {
      out.push(row);
      row = [];
    }
  }
  if (row.length > 0) {
    out.push(row);
  }
  return out;
}

@Component
export struct SymbolPage {
  @State selected: number = 0;
  @State page: number = 0;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private categories(): SymbolCategory[] {
    return symbolCategories();
  }

  private current(): SymbolCategory | null {
    const cats: SymbolCategory[] = this.categories();
    if (cats.length === 0) {
      return null;
    }
    return cats[Math.max(0, Math.min(this.selected, cats.length - 1))];
  }

  private pageCount(): number {
    const cat: SymbolCategory | null = this.current();
    if (cat === null) {
      return 0;
    }
    return Math.max(1, Math.ceil(cat.symbols.length / SYMBOL_PAGE_SIZE));
  }

  /** 第 row 行、第 col 列在当前页的符号（空串 = 补位格）。 */
  private symbolAt(row: number, col: number): string {
    const cat: SymbolCategory | null = this.current();
    if (cat === null) {
      return '';
    }
    const i: number = this.page * SYMBOL_PAGE_SIZE + row * SYMBOL_PER_ROW + col;
    return i < cat.symbols.length ? cat.symbols[i] : '';
  }

  private selectCategory(i: number): void {
    this.selected = i;
    this.page = 0;
  }

  private hint(): string {
    const cat: SymbolCategory | null = this.current();
    if (cat === null) {
      return '没有类目数据';
    }
    return `${cat.index}.${cat.label}  ${this.page + 1}/${this.pageCount()}`;
  }

  build() {
    Column() {
      PanelHeader({
        title: '符号',
        hint: this.hint(),
        onClose: () => {
          this.ctrl.closePanel();
        }
      })
      // 翻页条：Android 是左右滑动；本端用可编译的按钮形态（见文件头差异说明）。
      Row() {
        Text('‹')
          .fontSize(22).fontColor(this.page > 0 ? this.pal().accent : this.pal().dimFg)
          .padding({ left: 12, right: 12, top: 2, bottom: 2 })
          .onClick(() => {
            if (this.page > 0) {
              this.page--;
            }
          })
        Text(`${this.page + 1}/${Math.max(1, this.pageCount())}`)
          .fontSize(12).fontColor(this.pal().dimFg).layoutWeight(1).textAlign(TextAlign.Center)
        Text('›')
          .fontSize(22).fontColor(this.page + 1 < this.pageCount() ? this.pal().accent : this.pal().dimFg)
          .padding({ left: 12, right: 12, top: 2, bottom: 2 })
          .onClick(() => {
            if (this.page + 1 < this.pageCount()) {
              this.page++;
            }
          })
      }.width('100%').height(28).backgroundColor(this.pal().barBg)

      if (this.current() === null) {
        Text('没有类目数据（symbol_categories.tsv 未随包）')
          .fontSize(14).fontColor(this.pal().dimFg).layoutWeight(1)
      } else {
        Column() {
          ForEach(rowsOfRange(0, SYMBOL_ROWS - 1, 1), (r: number[]) => {
            Row() {
              ForEach(blanks(SYMBOL_PER_ROW), (col: number) => {
                Stack({ alignContent: Alignment.Center }) {
                  Text(this.symbolAt(r[0], col))
                    .fontSize(this.symbolAt(r[0], col).length > 1 ? 14 : (this.elder === 1 ? 26 : 22))
                    .fontColor(this.pal().keyFg).maxLines(1)
                    .textOverflow({ overflow: TextOverflow.Ellipsis })
                }
                .layoutWeight(1).height('100%').margin(2).borderRadius(6)
                .backgroundColor(this.symbolAt(r[0], col).length === 0
                  ? this.pal().panelBg : this.pal().keyBg)
                .onClick(() => {
                  const s: string = this.symbolAt(r[0], col);
                  if (s.length > 0) {
                    this.ctrl.commitText(s);
                  }
                })
              }, (col: number) => `sym-${this.page}-${r[0]}-${col}`)
            }.width('100%').layoutWeight(1)
          }, (r: number[]) => `sym-row-${r[0]}-${this.page}`)
        }.layoutWeight(1).width('100%').padding({ left: 4, right: 4, top: 4, bottom: 4 })
      }

      // 底部分类条（22 项，可横滑；序号.名称 与 Android 一致）。
      Scroll() {
        Row() {
          ForEach(this.categories(), (c: SymbolCategory, i: number) => {
            Text(`${c.index}.${c.label}`)
              .fontSize(13)
              .fontColor(i === this.selected ? this.pal().accent : this.pal().panelFg)
              .fontWeight(i === this.selected ? FontWeight.Bold : FontWeight.Normal)
              .padding({ left: 10, right: 10, top: 12, bottom: 12 })
              .onClick(() => {
                this.selectCategory(i);
              })
          }, (c: SymbolCategory, i: number) => `cat-${i}-${c.label}`)
        }
      }
      .scrollable(ScrollDirection.Horizontal)
      .width('100%').height(46).backgroundColor(this.pal().barBg)
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}

// ---------------------------------------------------------------- 生僻字

@Component
export struct RarePanel {
  @State radical: number = 0;
  @State strokes: number = 0;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private hasData(): boolean {
    return rareIndex().size > 0;
  }

  private title(): string {
    if (this.radical === 0) {
      return '生僻字 · 选部首';
    }
    if (this.strokes === 0) {
      return `生僻字 · ${radicalGlyph(this.radical)}（部首 ${this.radical}）· 选笔画数`;
    }
    return `生僻字 · 部首 ${this.radical} · ${this.strokes} 画`;
  }

  private back(): void {
    if (this.strokes !== 0) {
      this.strokes = 0;
    } else if (this.radical !== 0) {
      this.radical = 0;
    } else {
      this.ctrl.closePanel();
    }
  }

  private strokeCounts(): number[] {
    if (this.radical === 0) {
      return [];
    }
    return rareStrokeCounts(this.radical);
  }

  private chars(): string[] {
    if (this.radical === 0 || this.strokes === 0) {
      return [];
    }
    return rareChars(this.radical, this.strokes);
  }

  build() {
    Column() {
      // 生僻字自带页头：返回 = 逐级回退（部首 ← 笔画 ← 字），收起 = 直接退出。
      Row() {
        Text('‹ 返回')
          .fontSize(14).fontColor(this.pal().accent).padding({ left: 12, right: 8 })
          .onClick(() => {
            this.back();
          })
        Text(this.title())
          .fontSize(14).fontColor(this.pal().panelFg).layoutWeight(1).maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Text('收起')
          .fontSize(13).fontColor(this.pal().accent).padding({ left: 8, right: 12 })
          .onClick(() => {
            this.ctrl.closePanel();
          })
      }.width('100%').height(42).backgroundColor(this.pal().toolbarBg)

      if (!this.hasData()) {
        Text('生僻字索引未随包（rare_index.tsv）⇒ 暂无可选内容')
          .fontSize(14).fontColor(this.pal().dimFg).layoutWeight(1)
      } else if (this.radical === 0) {
        Scroll() {
          Column() {
            ForEach(rowsOfRange(1, 214, INDEX_COLS), (row: number[]) => {
              Row() {
                ForEach(row, (r: number) => {
                  Stack({ alignContent: Alignment.Center }) {
                    Text(radicalGlyph(r)).fontSize(20).fontColor(this.pal().keyFg)
                  }
                  .layoutWeight(1).height(44).margin(2).borderRadius(6)
                  .backgroundColor(this.pal().keyBg)
                  .onClick(() => {
                    this.radical = r;
                    this.strokes = 0;
                  })
                }, (r: number) => `rad-${r}`)
                ForEach(blanks(INDEX_COLS - row.length), (b: number) => {
                  Text('').layoutWeight(1).height(44)
                }, (b: number) => `rad-blank-${b}`)
              }.width('100%')
            }, (row: number[]) => `rad-row-${row[0]}`)
          }.width('100%')
        }.layoutWeight(1).width('100%')
      } else if (this.strokes === 0) {
        if (this.strokeCounts().length === 0) {
          Text('该部首没有索引数据').fontSize(14).fontColor(this.pal().dimFg).layoutWeight(1)
        } else {
          Scroll() {
            Row() {
              ForEach(this.strokeCounts(), (s: number) => {
                Text(`${s} 画`)
                  .fontSize(16).fontColor(this.pal().keyFg)
                  .backgroundColor(this.pal().keyBg).borderRadius(8)
                  .padding({ left: 16, right: 16, top: 10, bottom: 10 })
                  .margin(5)
                  .onClick(() => {
                    this.strokes = s;
                  })
              }, (s: number) => `stroke-${s}`)
            }.padding({ left: 6, right: 6 })
          }
          .scrollable(ScrollDirection.Horizontal)
          .layoutWeight(1).width('100%')
        }
      } else {
        Scroll() {
          Column() {
            ForEach(rowsOfRange(0, this.chars().length - 1, INDEX_COLS), (row: number[]) => {
              Row() {
                ForEach(row, (i: number) => {
                  Stack({ alignContent: Alignment.Center }) {
                    Text(this.chars()[i]).fontSize(22).fontColor(this.pal().keyFg)
                  }
                  .layoutWeight(1).height(44).margin(2).borderRadius(6)
                  .backgroundColor(this.pal().keyBg)
                  .onClick(() => {
                    this.ctrl.commitText(this.chars()[i]);
                  })
                }, (i: number) => `rare-${i}-${this.chars()[i]}`)
                ForEach(blanks(INDEX_COLS - row.length), (b: number) => {
                  Text('').layoutWeight(1).height(44)
                }, (b: number) => `rare-blank-${b}`)
              }.width('100%')
            }, (row: number[]) => `rare-row-${row[0]}`)
          }.width('100%')
        }.layoutWeight(1).width('100%')
      }
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}
