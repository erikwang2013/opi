// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键盘面板路由：顶部两行（TopBar）+ 面板/键面。
//
// 判据顺序（对齐 Android ImeScreen）：面板视图（opiView）优先 → 表情（引擎 Symbol 模式）
// → 数字面板 → 输入模式键面 → 26 键族。生僻字/符号页是**面板视图**，不换引擎键位。
// 单手模式只收窄键区（76%），TopBar 保持全宽（Android 同款，纯布局层）。

import { OpiMode } from '../InputMethodExtensionAbility/OpiEngine';
import { MODE_BIG9, MODE_PINYIN9, MODE_PINYIN14, MODE_STROKE } from '../InputMethodExtensionAbility/InputModes';
import { PanelView } from '../InputMethodExtensionAbility/KeyboardModes';
import { Palette, paletteFor } from '../InputMethodExtensionAbility/Palette';
import { sceneImage } from '../InputMethodExtensionAbility/SceneTheme';
import { TopBar } from '../InputMethodExtensionAbility/TopBar';
import { CandidateTiled } from '../InputMethodExtensionAbility/CandidateTiled';
import { QwertyPad } from '../InputMethodExtensionAbility/QwertyPad';
import { EmojiPad } from '../InputMethodExtensionAbility/EmojiPad';
import { FourteenKeyPanel, NineKeyPanel } from '../InputMethodExtensionAbility/KeyPanels';
import { StrokeKeyPanel } from '../InputMethodExtensionAbility/StrokePanel';
import {
  AboutPanel, InputModePanel, KeyboardModePanel, SkinPanel
} from '../InputMethodExtensionAbility/Panels';
import { SettingsPanel } from '../InputMethodExtensionAbility/SettingsPanel';
import { RarePanel, SymbolPage } from '../InputMethodExtensionAbility/SymbolPage';

@Entry
@Component
struct Index {
  // ⚠️ @StorageLink 的键名必须是字符串字面量（写侧常量在 KeyboardAppStorage.ets）。
  @StorageLink('opiView') view: number = 0;
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiLayout') layout: number = 0;
  @StorageLink('opiOneHand') oneHand: number = 0;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiScene') scene: string = '';
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @StorageLink('opiTiled') tiled: number = 0;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Stack() {
      if (this.scene.length > 0) {
        // 动态皮肤：插画铺在键面后，键缝/边缘透出（Android/iOS/桌面同一条）。
        Image(sceneImage(this.scene))
          .width('100%').height('100%').objectFit(ImageFit.Cover)
        Column()
          .width('100%').height('100%')
          .backgroundColor(this.pal().barBg).opacity(0.18)
      }
      Column() {
      TopBar()
      if (this.tiled === 1 && this.view === 0) {
        // 平铺候选：占满拼音行以下的整块（含单手模式的留白区 —— Android 平铺也在
        // 单手 Row 之外，是整窗宽度）。
        CandidateTiled()
      } else {
        Row() {
          if (this.oneHand === 2) {
            this.SideBlank()
          }
          Column() {
            this.Content()
          }
          .layoutWeight(this.oneHand === 0 ? 1 : 0.76)
          .height('100%')
          if (this.oneHand === 1) {
            this.SideBlank()
          }
        }
        .width('100%').layoutWeight(1)
        .backgroundColor(this.scene.length > 0 ? '#00000000' : this.pal().barBg)
      }
      }
      .width('100%').height('100%')
      .backgroundColor(this.scene.length > 0 ? '#00000000' : this.pal().panelBg)
    }
    .width('100%').height('100%')
  }

  @Builder
  SideBlank() {
    Column().layoutWeight(0.24).height('100%')
  }

  @Builder
  Content() {
    if (this.view === (PanelView.InputMode as number)) {
      InputModePanel()
    } else if (this.view === (PanelView.Skin as number)) {
      SkinPanel()
    } else if (this.view === (PanelView.KeyboardMode as number)) {
      KeyboardModePanel()
    } else if (this.view === (PanelView.About as number)) {
      AboutPanel()
    } else if (this.view === (PanelView.Settings as number)) {
      SettingsPanel()
    } else if (this.view === (PanelView.SymbolPage as number)) {
      SymbolPage()
    } else if (this.view === (PanelView.Rare as number)) {
      RarePanel()
    } else if (this.mode === (OpiMode.Symbol as number)) {
      EmojiPad()
    } else {
      this.Keyboard()
    }
  }

  /** 键面：数字面板由模式决定；其余按输入模式下标挑（26 键族是兜底）。 */
  @Builder
  Keyboard() {
    if (this.mode === (OpiMode.Number as number)) {
      QwertyPad()
    } else if (this.layout === MODE_PINYIN9) {
      NineKeyPanel({ big: false })
    } else if (this.layout === MODE_BIG9) {
      NineKeyPanel({ big: true })
    } else if (this.layout === MODE_PINYIN14) {
      FourteenKeyPanel()
    } else if (this.layout === MODE_STROKE) {
      StrokeKeyPanel()
    } else {
      // 拼音26键 / 英文26键 / 双拼 / 五笔 / 生僻字残留档：同一套 QWERTY。
      QwertyPad()
    }
  }
}
