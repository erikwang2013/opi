// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 表情页「自有」子页：12 张小欧形态的图片网格（Android `PetStickerPanel` 的镜像，
// 从 EmojiPad 拆出以守住 500 行）。点按复制图片到剪贴板（IME 不能直接向宿主插图片）。

import { Palette, paletteFor } from './Palette';
import {
  PET_STICKER_FORMS, PetStickerForm, stickerBase64, stickerGifBase64, stickerGifDataUri,
} from './PetStickerImages';
import { copyStickerToClipboard } from './PetStickerClipboard';

/** 手机宽度下的列数；宽屏按 120vp/列加到最多 6 列（手机 4 列不变）。 */
const PER_ROW: number = 4;
const MAX_PER_ROW: number = 6;
const CELL_VP: number = 120;
const SIZE: number = 56;

@Component
export struct StickerGrid {
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @State gridW: number = 360;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private perRow(): number {
    return Math.min(MAX_PER_ROW, Math.max(PER_ROW, Math.floor(this.gridW / CELL_VP)));
  }

  build() {
    Scroll() {
      Column() {
        ForEach(this.rows(), (row: PetStickerForm[], rowIndex: number) => {
          Row() {
            ForEach(row, (f: PetStickerForm, col: number) => {
              Column() {
                Image(stickerGifDataUri(f.sticker))
                  .width(SIZE).height(SIZE)
                  .objectFit(ImageFit.Contain)
                Text(f.label).fontSize(10).fontColor(this.pal().panelFg).margin({ top: 2 })
              }
              .layoutWeight(1).height(78).justifyContent(FlexAlign.Center)
              .onClick(() => {
                // PNG/PixelMap 优先（宿主最认）；PNG 缺失时 Clipboard helper 才用 GIF。
                copyStickerToClipboard(
                  stickerGifBase64(f.sticker), stickerBase64(f.sticker), f.label
                );
              })
            }, (f: PetStickerForm, col: number) => `${col}-${f.sticker}`)
            this.Blanks(this.perRow() - row.length)
          }
          .width('100%').margin({ top: rowIndex === 0 ? 6 : 2 })
        }, (row: PetStickerForm[], rowIndex: number) => `sticker-${this.perRow()}-${rowIndex}`)
      }
      .width('100%')
    }
    .layoutWeight(1).width('100%')
    .onAreaChange((oldValue: Area, newValue: Area) => {
      const w: number = newValue.width as number;
      if (w > 0 && w !== this.gridW) {
        this.gridW = w;
      }
    })
  }

  @Builder
  Blanks(n: number) {
    ForEach(this.range(n), (b: number) => {
      Text('').layoutWeight(1)
    }, (b: number) => `blank-${b}`)
  }

  private rows(): PetStickerForm[][] {
    const out: PetStickerForm[][] = [];
    const per: number = this.perRow();
    for (let i = 0; i < PET_STICKER_FORMS.length; i += per) {
      out.push(PET_STICKER_FORMS.slice(i, i + per));
    }
    return out;
  }

  private range(n: number): number[] {
    const out: number[] = [];
    for (let i = 0; i < n; i++) {
      out.push(i);
    }
    return out;
  }
}
