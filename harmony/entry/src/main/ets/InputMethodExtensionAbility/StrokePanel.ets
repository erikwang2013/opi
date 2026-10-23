// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 笔画键面（Android StrokeKeyboard 的镜像）。从 KeyPanels.ets 拆出，
// 让两个面板文件都留在 500 行以内。键表是协议：h/s/p/n/z = 横竖撇捺折。

import { KeyboardController } from './KeyboardController';
import { MODE_ENGLISH26, traditionalModeLabel } from './InputModes';
import { OpiMode } from './OpiEngine';
import { Palette, paletteFor } from './Palette';
import { BackspaceButton, FuncButton } from './KeyPopup';

/** 键位容器（笔画共用外观）。 */
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

@Component
export struct StrokeKeyPanel {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private keyFont(): number {
    return this.elder === 1 ? 17 : 15;
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
        FuncButton({
          label: '符', fontSize: this.keyFont(),
          onTap: () => {
            this.ctrl.openSymbolPage();
          }
        })
        FuncButton({
          label: traditionalModeLabel(this.mode), fontSize: this.keyFont(),
          onTap: () => {
            this.ctrl.toggleTraditional();
          }
        })
        FuncButton({
          label: '英文', fontSize: this.keyFont(),
          onTap: () => {
            this.ctrl.selectInputMode(MODE_ENGLISH26);
          }
        })
        FuncButton({
          label: '生僻字', fontSize: this.keyFont(), weight: 1.4,
          onTap: () => {
            this.ctrl.openRare();
          }
        })
        FuncButton({
          label: '空格', fontSize: this.keyFont(), weight: 2,
          onTap: () => {
            this.ctrl.tapSpace();
          }
        })
        BackspaceButton({ fontSize: this.keyFont(), weight: 1 })
        FuncButton({
          label: '↵', fontSize: this.keyFont(),
          onTap: () => {
            this.ctrl.tapEnter();
          }
        })
      }.layoutWeight(1).margin({ top: 6, bottom: 8 })
    }.width('100%')
  }
}
