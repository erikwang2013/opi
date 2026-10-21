// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键盘内设置页（2026-10-22，工具条「关于」前的齿轮）：对齐 Android SettingsScreen
// 的**可用子集** —— 只放本端真有出口的开关，没有模糊音/纠错/简拼/词库导入导出这些
// 尚未桥接的项（不做假开关）。
//
// 皮肤 / 关于仍是独立面板：从本页进去时返回箭头回本页（KeyboardModes.openSubPanel），
// 从工具条直接进则返回键面。

import { KeyboardController } from './KeyboardController';
import {
  ONE_HAND_LEFT, ONE_HAND_OFF, ONE_HAND_RIGHT, PanelView, openSubPanel
} from './KeyboardModes';
import { OpiMode } from './OpiEngine';
import { Palette, SKINS, paletteFor } from './Palette';
import { ModeChip, PanelHeader, appVersion } from './Panels';
import { UpdateSection } from './UpdateCheck';

@Component
export struct SettingsPanel {
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiLearner') learner: number = 1;
  @StorageLink('opiPro') pro: number = 0;
  @StorageLink('opiShowOwn') showOwn: number = 0;
  @StorageLink('opiOneHand') oneHand: number = 0;
  @StorageLink('opiFloat') floating: number = 0;
  @StorageLink('opiGame') game: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private openSub(v: PanelView): void {
    openSubPanel(v);
    this.ctrl.publish();
  }

  @Builder
  Section(title: string) {
    Text(title)
      .fontSize(this.elder === 1 ? 13 : 11).fontWeight(FontWeight.Bold)
      .fontColor(this.pal().dimFg).width('100%')
      .padding({ left: 14, right: 14, top: 12, bottom: 4 })
  }

  @Builder
  SwitchRow(label: string, desc: string, checked: boolean, action: () => void) {
    Row() {
      Column() {
        Text(label)
          .fontSize(this.elder === 1 ? 16 : 14).fontColor(this.pal().panelFg)
        if (desc.length > 0) {
          Text(desc)
            .fontSize(this.elder === 1 ? 12 : 11).fontColor(this.pal().dimFg)
            .margin({ top: 2 }).maxLines(2)
        }
      }
      .layoutWeight(1).alignItems(HorizontalAlign.Start)
      Toggle({ type: ToggleType.Switch, isOn: checked })
        .selectedColor(this.pal().accent)
        .onChange((isOn: boolean) => {
          if (isOn !== checked) {
            action();
          }
        })
    }
    .width('100%').padding({ left: 14, right: 14, top: 8, bottom: 8 })
  }

  @Builder
  ActionRow(label: string, desc: string, value: string, action: () => void) {
    Row() {
      Column() {
        Text(label)
          .fontSize(this.elder === 1 ? 16 : 14).fontColor(this.pal().panelFg)
        if (desc.length > 0) {
          Text(desc)
            .fontSize(this.elder === 1 ? 12 : 11).fontColor(this.pal().dimFg)
            .margin({ top: 2 }).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
        }
      }
      .layoutWeight(1).alignItems(HorizontalAlign.Start)
      Text(value)
        .fontSize(this.elder === 1 ? 14 : 12).fontColor(this.pal().dimFg)
        .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
        .margin({ left: 8 })
      Text('›').fontSize(18).fontColor(this.pal().dimFg).margin({ left: 6 })
    }
    .width('100%').padding({ left: 14, right: 14, top: 10, bottom: 10 })
    .onClick(action)
  }

  @Builder
  InfoRow(label: string, value: string) {
    Row() {
      Text(label)
        .fontSize(this.elder === 1 ? 16 : 14).fontColor(this.pal().panelFg)
      Text(value)
        .fontSize(this.elder === 1 ? 14 : 12).fontColor(this.pal().dimFg)
        .layoutWeight(1).textAlign(TextAlign.End).margin({ left: 8 })
        .maxLines(2).textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%').padding({ left: 14, right: 14, top: 10, bottom: 10 })
  }

  build() {
    Column() {
      PanelHeader({
        title: '设置', hint: '常用 / 键盘 / 外观 / 关于',
        onClose: () => {
          this.ctrl.closePanel();
        }
      })
      Scroll() {
        Column() {
          this.Section('常用')
          this.SwitchRow('学习', '根据选词学习用户词频', this.learner === 1, () => {
            this.ctrl.toggleLearner();
          })
          this.SwitchRow('英文专业模式', '英文候选与联想；关闭时字母直接上屏', this.pro === 1, () => {
            this.ctrl.toggleEnglishPro();
          })
          this.SwitchRow('表情页显示自有', '在表情页显示小欧贴纸分类', this.showOwn === 1, () => {
            this.ctrl.toggleShowOwn();
          })
          this.InfoRow('词库', this.ctrl.dictionaryDegraded() ? '内置回退词库（完整词库未装上）' : '完整词库')

          this.Section('键盘')
          this.SwitchRow('繁体模式', '中文输入使用繁体词库', this.mode === (OpiMode.Traditional as number), () => {
            this.ctrl.toggleTraditional();
          })
          Flex({ wrap: FlexWrap.Wrap, justifyContent: FlexAlign.Start }) {
            ModeChip({
              label: '单手模式：关', selected: this.oneHand === ONE_HAND_OFF,
              onTap: () => {
                this.ctrl.setOneHand(ONE_HAND_OFF);
              }
            })
            ModeChip({
              label: '单手模式：左手', selected: this.oneHand === ONE_HAND_LEFT,
              onTap: () => {
                this.ctrl.setOneHand(ONE_HAND_LEFT);
              }
            })
            ModeChip({
              label: '单手模式：右手', selected: this.oneHand === ONE_HAND_RIGHT,
              onTap: () => {
                this.ctrl.setOneHand(ONE_HAND_RIGHT);
              }
            })
          }.width('100%').padding({ left: 10, right: 10, top: 2 })
          this.SwitchRow('悬浮模式', '键盘收窄居中，可拖动', this.floating === 1, () => {
            this.ctrl.toggleFloating();
          })
          this.SwitchRow('游戏模式', '更矮的键盘，减少遮挡', this.game === 1, () => {
            this.ctrl.toggleGame();
          })

          this.Section('外观')
          this.ActionRow('皮肤', '选中即换；「不改动」跟随暗光/长辈', SKINS[this.skin].label, () => {
            this.openSub(PanelView.Skin);
          })
          this.SwitchRow('暗光模式', '深色键盘配色', this.dark === 1, () => {
            this.ctrl.toggleDark();
          })
          this.SwitchRow('长辈模式', '大字号键盘与面板', this.elder === 1, () => {
            this.ctrl.toggleElder();
          })

          this.Section('关于')
          this.InfoRow('版本', `v${appVersion()}`)
          Column() {
            UpdateSection({ currentVersion: appVersion() })
          }.width('100%').padding({ left: 14, right: 14, top: 2, bottom: 4 })
          this.ActionRow('关于与链接', '简介 / 版权 / 项目地址', '', () => {
            this.openSub(PanelView.About);
          })
        }
        .width('100%').padding({ bottom: 10 })
      }.layoutWeight(1).width('100%')
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}
