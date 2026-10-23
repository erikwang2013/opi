// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 平铺候选视图（对齐 Android `ime/CandidateTiledView.kt` + `candidate/TilePack.kt`）。
//
// 占满「候选/工具条共用格 + 键区」：拼音行仍由 TopBar 画。中间按「字宽单元」排版：
//   · 候选长度 ≤ 2 → 占 1 格；更长 → 占 2 格（同 Android `tileUnits`）；
//   · 一行 4 格、一页 4 行，**贪心不重排**（保持引擎候选序）；
//   · 候选整批换新 → 回第 1 页；**上下滑动**翻页。
// 右列 = 返回 / ⌫ / 重输 / 空格（与 9 键右列同观感）；返回不动引擎、不清缓冲。
//
// ⚠️ 与 Android 的**明确差异**：左列在 Android 是「候选读音列」（点 = 钉读音）。
// 读音出口 2026-10-24 已桥接，但目前只接在 9 键左列；平铺视图沿用旧的 5 格
// **标点列**（走引擎中文标点表）—— 功能可用、但不是读音钉选。

import { KeyboardController } from './KeyboardController';
import { NINE_STRIP, NineKey } from './KeyPanels';
import { Palette, paletteFor } from './Palette';

/** 一行 4 个「字宽单元」（Android UNITS_PER_ROW）。 */
const UNITS_PER_ROW: number = 4;
/** 一页 4 行（Android ROWS_PER_PAGE）。 */
const ROWS_PER_PAGE: number = 4;
/** 左列格数（与旧 9 键标点列同节奏）。 */
const LEFT_CELLS: number = 5;
/** 9 键标点列 2026-10-24 扩到 16 项；平铺视图保持 5 格，不跟着挤成 16 行。 */
const TILE_STRIP: NineKey[] = NINE_STRIP.slice(0, LEFT_CELLS);

class TileCell {
  index: number = 0;
  units: number = 1;
  text: string = '';

  constructor(index: number, text: string) {
    this.index = index;
    this.text = text;
    this.units = text.length <= 2 ? 1 : 2;
  }
}

class TileRow {
  cells: TileCell[] = [];
  used: number = 0;
}

class TilePage {
  rows: TileRow[] = [];
}

/** 候选数组 → 分页行块（纯函数；贪心、不重排）。 */
function packTiles(candidates: string[]): TilePage[] {
  const pages: TilePage[] = [];
  let page: TilePage = new TilePage();
  let row: TileRow = new TileRow();
  for (let i = 0; i < candidates.length; i++) {
    const cell: TileCell = new TileCell(i, candidates[i]);
    if (row.used + cell.units > UNITS_PER_ROW) {
      page.rows.push(row);
      row = new TileRow();
      if (page.rows.length === ROWS_PER_PAGE) {
        pages.push(page);
        page = new TilePage();
      }
    }
    row.cells.push(cell);
    row.used += cell.units;
  }
  if (row.cells.length > 0) {
    page.rows.push(row);
  }
  if (page.rows.length > 0) {
    pages.push(page);
  }
  return pages;
}

@Component
export struct CandidateTiled {
  @StorageLink('opiAllCandidates') @Watch('rebuild') candidates: string[] = [];
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @State pages: TilePage[] = [];
  @State pageIndex: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  aboutToAppear(): void {
    this.rebuild();
  }

  /** 候选整批换新 → 重排 + 回第一页（Android LaunchedEffect(cands) 同款）。 */
  rebuild(propName: string = ''): void {
    this.pages = packTiles(this.candidates);
    this.pageIndex = 0;
  }

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private cellFont(): number {
    return this.elder === 1 ? 25 : 22;
  }

  /** ForEach 的键必须随内容变（同键不重建 ⇒ 换一批候选会停在旧字上）。 */
  private pageKey(page: TilePage): string {
    let sig: string = '';
    for (let r = 0; r < page.rows.length; r++) {
      const cells: TileCell[] = page.rows[r].cells;
      for (let c = 0; c < cells.length; c++) {
        sig += `${cells[c].index}:${cells[c].text}|`;
      }
    }
    return sig;
  }

  build() {
    Row() {
      // 左列：标点（Android 这里是读音列；本视图仍走标点列，见文件头注）。
      Column() {
        ForEach(TILE_STRIP, (k: NineKey) => {
          Column() {
            Text(k.label)
              .fontSize(this.elder === 1 ? 21 : 18)
              .fontColor(this.pal().keyFg)
          }
          .layoutWeight(1).width('100%').justifyContent(FlexAlign.Center)
          .backgroundColor(this.pal().keyBg).borderRadius(6).margin({ top: 2, bottom: 2 })
          .onClick(() => {
            this.ctrl.tapChar(k.key);
          })
        }, (k: NineKey) => `tile-p-${k.key}`)
      }.layoutWeight(1).height('100%').padding({ left: 4, right: 2 })

      // 中间：4×4 网格，上下滑动翻页。
      Swiper() {
        ForEach(this.pages, (page: TilePage, pageNo: number) => {
          Column() {
            ForEach([0, 1, 2, 3], (r: number) => {
              Row() {
                if (r < page.rows.length) {
                  ForEach(page.rows[r].cells, (cell: TileCell) => {
                    Column() {
                      Text(cell.text)
                        .fontSize(this.cellFont()).fontColor(this.pal().keyFg)
                        .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
                        .textAlign(TextAlign.Center).width('100%')
                    }
                    .layoutWeight(cell.units).height('100%').justifyContent(FlexAlign.Center)
                    .backgroundColor(this.pal().keyBg).borderRadius(6)
                    .margin({ left: 1, right: 1, top: 1, bottom: 1 })
                    .onClick(() => {
                      this.ctrl.tapTiledCandidate(cell.index);
                    })
                  }, (cell: TileCell) => `tile-${cell.index}-${cell.text}`)
                  if (page.rows[r].used < UNITS_PER_ROW) {
                    Text('').layoutWeight(UNITS_PER_ROW - page.rows[r].used)
                  }
                }
              }.width('100%').layoutWeight(1)
            }, (r: number) => `tile-row-${pageNo}-${r}`)
          }.width('100%').height('100%')
        }, (page: TilePage, pageNo: number) => `tile-page-${pageNo}-${this.pageKey(page)}`)
      }
      .layoutWeight(3).height('100%')
      .vertical(true).loop(false).indicator(false)
      .index(this.pageIndex)
      .onChange((index: number) => {
        this.pageIndex = index;
      })

      // 右列：返回 / ⌫ / 重输 / 空格（Android CandidateTiledView 同款）。
      Column() {
        this.SideKey('返回', () => {
          this.ctrl.collapseTiled();
        })
        this.BackspaceSideKey()
        this.SideKey('重输', () => {
          this.ctrl.retype();
        })
        this.SideKey('空格', () => {
          this.ctrl.tapSpace();
        })
      }.layoutWeight(1).height('100%').padding({ left: 2, right: 4 })
    }
    .width('100%').height('100%').backgroundColor(this.pal().barBg)
  }

  /** ⌫：单击删一格；长按按住连续退格，松手停止。 */
  @Builder
  BackspaceSideKey() {
    Column() {
      Text('⌫')
        .fontSize(this.elder === 1 ? 17 : 15).fontColor(this.pal().keyFg)
        .textAlign(TextAlign.Center)
    }
    .layoutWeight(1).width('100%').justifyContent(FlexAlign.Center)
    .backgroundColor(this.pal().keyBg).borderRadius(6)
    .margin({ top: 2, bottom: 2 })
    .onClick(() => {
      this.ctrl.tapBackspace();
    })
    .gesture(
      LongPressGesture({ repeat: false, duration: 500 })
        .onAction(() => {
          this.ctrl.setBackspaceRepeat(true);
        })
        .onActionEnd(() => {
          this.ctrl.setBackspaceRepeat(false);
        })
        .onActionCancel(() => {
          this.ctrl.setBackspaceRepeat(false);
        })
    )
  }

  @Builder
  SideKey(label: string, action: () => void) {
    Column() {
      Text(label)
        .fontSize(this.elder === 1 ? 17 : 15).fontColor(this.pal().keyFg)
        .textAlign(TextAlign.Center)
    }
    .layoutWeight(1).width('100%').justifyContent(FlexAlign.Center)
    .backgroundColor(this.pal().keyBg).borderRadius(6)
    .margin({ top: 2, bottom: 2 })
    .onClick(action)
  }
}
