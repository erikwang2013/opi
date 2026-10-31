// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 符号页 + 生僻字面板（Android `keyboard/SymbolPage.kt` / `RarePanel.kt` 的镜像）。
//
// 两张表都**不经过引擎**：数据来自 rawfile TSV（SymbolData.ets 解析），点格子直接
// `insertText`。Android 同款（RARE/SYMBOL_PAGE 视图不碰引擎 scheme/mode）。
//
// 翻页 = 左右滑动（Android HorizontalPager 同款；ArkUI Swiper）。页头只有标题/提示/
// 关闭，没有返回箭头（Android SymbolPage 的紧凑头）。

import { KeyboardController } from './KeyboardController';
import { OpiMode } from './OpiEngine';
import { Palette, paletteFor, scenePanelBg } from './Palette';
import {
  SymbolCategory, radicalGlyph, rareChars, rareIndex, rareStrokeCounts, symbolCategories,
} from './SymbolData';

/** 手机宽度下的列数（Android 同值）；宽屏按 50vp/列加到最多 12 列，手机不变。 */
const SYMBOL_PER_ROW: number = 8;
const SYMBOL_ROWS: number = 4;
/** 每行至少 30vp（22fp 单字符 + 呼吸）；窗口更矮时减少每页行数而不是压扁格子。 */
const SYMBOL_ROW_MIN_VP: number = 30;
const INDEX_COLS: number = 8;
const GRID_CELL_VP: number = 50;
const GRID_MAX_COLS: number = 12;

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
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  /** 2026-10-23：英文键盘的 `!?#` 直达「英文」类目（Android `SymbolPage.initialCategory` 同义）。 */
  aboutToAppear(): void {
    if (this.mode !== (OpiMode.English as number)) {
      return;
    }
    const cats = symbolCategories();
    const i = cats.findIndex((c: SymbolCategory) => c.label === '英文');
    if (i >= 0) {
      this.selected = i;
      this.page = 0;
    }
  }
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  /** 面板实测尺寸（vp）：宽屏加列、矮窗减行；手机竖向取值与常量相同。 */
  @State gridW: number = 360;
  @State gridH: number = 260;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private perRow(): number {
    return Math.min(GRID_MAX_COLS, Math.max(SYMBOL_PER_ROW, Math.floor(this.gridW / GRID_CELL_VP)));
  }

  /** 每页行数：常规窗口 4 行；游戏模式/分屏等矮窗按 30vp/行减到 1 行。 */
  private rowsPerPage(): number {
    return Math.max(1, Math.min(SYMBOL_ROWS, Math.floor(this.gridH / SYMBOL_ROW_MIN_VP)));
  }

  private pageSize(): number {
    return this.perRow() * this.rowsPerPage();
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
    return Math.max(1, Math.ceil(cat.symbols.length / this.pageSize()));
  }

  /** 第 pageNo 页、第 row 行、第 col 列的符号（空串 = 补位格）。 */
  private symbolAt(pageNo: number, row: number, col: number): string {
    const cat: SymbolCategory | null = this.current();
    if (cat === null) {
      return '';
    }
    const i: number = pageNo * this.pageSize() + row * this.perRow() + col;
    return i < cat.symbols.length ? cat.symbols[i] : '';
  }

  /** 页下标数组（Swiper 的每个子页）。 */
  private pages(): number[] {
    return blanks(this.pageCount());
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
      // Android 的紧凑头：标题 + 提示 + 关闭（没有返回箭头那一行）。
      Row() {
        Text('符号')
          .fontSize(15).fontWeight(FontWeight.Bold).fontColor(this.pal().panelFg)
          .padding({ left: 14 })
        Text(this.hint())
          .fontSize(11).fontColor(this.pal().dimFg).layoutWeight(1)
          .padding({ left: 10 }).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
        SymbolGlyph($r('sys.symbol.xmark'))
          .fontSize(20).fontColor([this.pal().toolbarFg]).accessibilityText('关闭符号页')
          .padding({ left: 8, right: 12, top: 4, bottom: 4 })
          .onClick(() => {
            this.ctrl.closePanel();
          })
      }.width('100%').height(42).backgroundColor(this.pal().toolbarBg)

      if (this.current() === null) {
        Text('没有类目数据（symbol_categories.tsv 未随包）')
          .fontSize(14).fontColor(this.pal().dimFg).layoutWeight(1)
      } else {
        // 左右滑动翻页（Android HorizontalPager）；换类目回第 1 页。
        Swiper() {
          ForEach(this.pages(), (pageNo: number) => {
            Column() {
              ForEach(rowsOfRange(0, this.rowsPerPage() - 1, 1), (r: number[]) => {
                Row() {
                  ForEach(blanks(this.perRow()), (col: number) => {
                    Stack({ alignContent: Alignment.Center }) {
                      Text(this.symbolAt(pageNo, r[0], col))
                        .fontSize(this.symbolAt(pageNo, r[0], col).length > 1 ? 14
                          : (this.elder === 1 ? 26 : 22))
                        .fontColor(this.pal().keyFg).maxLines(1)
                        .textOverflow({ overflow: TextOverflow.Ellipsis })
                    }
                    .layoutWeight(1).height('100%').margin(2).borderRadius(6)
                    .backgroundColor(this.symbolAt(pageNo, r[0], col).length === 0
                      ? this.pal().panelBg : this.pal().keyBg)
                    .onClick(() => {
                      const s: string = this.symbolAt(pageNo, r[0], col);
                      if (s.length > 0) {
                        this.ctrl.commitText(s);
                      }
                    })
                  }, (col: number) => `sym-${pageNo}-${r[0]}-${col}`)
                }.width('100%').layoutWeight(1)
              }, (r: number[]) => `sym-row-${pageNo}-${r[0]}`)
            }.width('100%').height('100%').padding({ left: 4, right: 4, top: 4, bottom: 4 })
          }, (pageNo: number) => `sym-page-${pageNo}-${this.selected}-${this.pageSize()}`)
        }
        .layoutWeight(1).width('100%')
        .vertical(false).loop(false).indicator(false)
        .index(this.page)
        .onChange((index: number) => {
          this.page = index;
        })
        .onAreaChange((oldValue: Area, newValue: Area) => {
          const h: number = newValue.height as number;
          if (h > 0 && h !== this.gridH) {
            this.gridH = h;
            // 行数变了页大小也变：旧页号夹回最后一页。
            this.page = Math.min(this.page, Math.max(0, this.pageCount() - 1));
          }
        })
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
    .width('100%').height('100%').backgroundColor(scenePanelBg(this.pal().panelBg))
    .onAreaChange((oldValue: Area, newValue: Area) => {
      const w: number = newValue.width as number;
      if (w > 0 && w !== this.gridW) {
        this.gridW = w;
        // 列数变了页大小也变：旧页号可能越界，夹回最后一页（换类目另由 selectCategory 归零）。
        this.page = Math.min(this.page, Math.max(0, this.pageCount() - 1));
      }
    })
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
  /** 面板实测宽度（vp）：宽屏/平板加列；行仍可纵向滚动，不依赖页大小。 */
  @State gridW: number = 360;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private cols(): number {
    return Math.min(GRID_MAX_COLS, Math.max(INDEX_COLS, Math.floor(this.gridW / GRID_CELL_VP)));
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
        // Android：只在有上一级（部首/笔画）时出现返回；根部只有「收起」。
        if (this.radical !== 0) {
          Text('‹ 返回')
            .fontSize(14).fontColor(this.pal().accent).padding({ left: 12, right: 8 })
            .onClick(() => {
              this.back();
            })
        }
        Text(this.title())
          .fontSize(14).fontColor(this.pal().panelFg).layoutWeight(1).maxLines(1)
          .padding({ left: 12 })
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
            ForEach(rowsOfRange(1, 214, this.cols()), (row: number[]) => {
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
                ForEach(blanks(this.cols() - row.length), (b: number) => {
                  Text('').layoutWeight(1).height(44)
                }, (b: number) => `rad-blank-${b}`)
              }.width('100%')
            }, (row: number[]) => `rad-row-${this.cols()}-${row[0]}`)
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
            ForEach(rowsOfRange(0, this.chars().length - 1, this.cols()), (row: number[]) => {
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
                ForEach(blanks(this.cols() - row.length), (b: number) => {
                  Text('').layoutWeight(1).height(44)
                }, (b: number) => `rare-blank-${b}`)
              }.width('100%')
            }, (row: number[]) => `rare-row-${this.cols()}-${row[0]}`)
          }.width('100%')
        }.layoutWeight(1).width('100%')
      }
    }
    .width('100%').height('100%').backgroundColor(scenePanelBg(this.pal().panelBg))
    .onAreaChange((oldValue: Area, newValue: Area) => {
      const w: number = newValue.width as number;
      if (w > 0 && w !== this.gridW) {
        this.gridW = w;
      }
    })
  }
}
