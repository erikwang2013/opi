// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 工具条面板：皮肤 / 键盘模式 / 关于 / 输入模式（Android ImeToolbar.kt 的四个面板）。
// 符号页与生僻字面板在 SymbolPage.ets（数据驱动、体量更大）。
//
// 形态与 Android 同一套：左上「‹ 返回」+ 标题 + 提示，整片盖住键面；皮肤**不自动关**
// （要连换着比），键盘模式可连开多个，输入模式选中即换（控制器决定收起或转生僻字）。

import { bundleManager } from '@kit.AbilityKit';
import { promptAction } from '@kit.ArkUI';
import { pasteboard } from '@kit.BasicServicesKit';
import { KeyboardController } from './KeyboardController';
import { OpiMode } from './OpiEngine';
import { INPUT_MODES, InputMode, currentModeIndex } from './InputModes';
import { ONE_HAND_LEFT, ONE_HAND_OFF, ONE_HAND_RIGHT, darkEnabled, elderEnabled } from './KeyboardModes';
import { Palette, SKINS, Skin, paletteFor } from './Palette';
import { SCENES, SceneSkin } from './SceneTheme';
import { UpdateSection } from './UpdateCheck';

const ABOUT_INTRO: string =
  '小欧输入法（opi）是一款回归本质、人人可用的多端输入法。' +
  'Open People Input 开放、纯粹、跨端，' +
  '致力于为每一位用户提供不被干扰、不被窥探、不被绑架的输入体验。';
const ABOUT_HOME_URL: string = 'https://opi.erik.xyz';
const ABOUT_PROJECT_URL: string = 'https://github.com/erikwang2013/opi';
const ABOUT_BLOG_URL: string = 'https://erik.xyz';
const ABOUT_PRIVACY_URL: string = 'https://opi.erik.xyz/privacy/index.html';

// ---------------------------------------------------------------- 零件

/** 面板头（四个面板 + 符号页共用）：返回 / 标题 / 提示 / 收起。 */
@Component
export struct PanelHeader {
  @Prop title: string = '';
  @Prop hint: string = '';
  onClose: () => void = () => {
  };
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Column() {
      // 2026-10-21 用户裁决：返回箭头单独一行，标题/提示下一行（对齐 Android）。
      Row() {
        SymbolGlyph($r('sys.symbol.arrow_left'))
          .fontSize(20).fontColor([this.pal().toolbarFg]).accessibilityText('返回')
          .padding({ left: 14, right: 10, top: 4, bottom: 4 })
          .onClick(this.onClose)
      }.width('100%').height(28)
      Row() {
        Text(this.title)
          .fontSize(15).fontColor(this.pal().panelFg).fontWeight(FontWeight.Bold)
        Text(this.hint)
          .fontSize(11).fontColor(this.pal().dimFg).layoutWeight(1)
          .padding({ left: 8 }).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      }.width('100%').height(24).padding({ left: 10 })
    }
    .width('100%').height(54).backgroundColor(this.pal().panelBg)
  }
}

/** 面板选项 chip：选中 = 强调色描边 + 粗体 + 行尾 ✓。 */
@Component
export struct ModeChip {
  @Prop label: string = '';
  @Prop selected: boolean = false;
  onTap: () => void = () => {
  };
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;

  build() {
    Text(this.selected ? `${this.label} ✓` : this.label)
      .fontSize(this.elder === 1 ? 17 : 14)
      .fontColor(this.selected ? paletteFor(this.skin, this.dark === 1, this.elder === 1).accent
        : paletteFor(this.skin, this.dark === 1, this.elder === 1).panelFg)
      .fontWeight(this.selected ? FontWeight.Bold : FontWeight.Normal)
      .backgroundColor(paletteFor(this.skin, this.dark === 1, this.elder === 1).keyBg)
      .border({
        width: this.selected ? 2 : 1,
        color: this.selected ? paletteFor(this.skin, this.dark === 1, this.elder === 1).accent
          : paletteFor(this.skin, this.dark === 1, this.elder === 1).dimFg
      })
      .borderRadius(8)
      .padding({ left: 14, right: 14, top: 8, bottom: 8 })
      .margin({ left: 4, right: 4, top: 4, bottom: 4 })
      .onClick(this.onTap)
  }
}

/** 一张皮肤卡：小键盘样张 + 名字。 */
@Component
struct SkinCard {
  @Prop index: number = 0;
  @Prop selected: boolean = false;
  onTap: () => void = () => {
  };

  build() {
    Column() {
      Column() {
        ForEach([0, 1, 2], (row: number) => {
          Row() {
            ForEach([0, 1, 2, 3], (col: number) => {
              Text('')
                .layoutWeight(1).height(5).borderRadius(1)
                .backgroundColor(row === 1 && col === 2
                  ? SKINS[this.index].palette.highlightBg : SKINS[this.index].palette.keyBg)
                .margin({ left: 1, right: 1 })
            }, (col: number) => `c-${col}`)
          }.width('100%').margin({ top: row === 0 ? 0 : 2 })
        }, (row: number) => `r-${row}`)
      }
      .width('86%').height(46).padding(6).borderRadius(6)
      .backgroundColor(SKINS[this.index].palette.barBg)
      .border({
        width: this.selected ? 2 : 1,
        color: this.selected ? SKINS[this.index].palette.accent : SKINS[this.index].palette.dimFg
      })
      Text(SKINS[this.index].label)
        .fontSize(11).margin({ top: 3 }).maxLines(1)
        .fontColor(this.selected ? SKINS[this.index].palette.accent : SKINS[this.index].palette.panelFg)
        .fontWeight(this.selected ? FontWeight.Bold : FontWeight.Normal)
    }
    .width(60).height(84).justifyContent(FlexAlign.Center)
    .margin({ left: 5, right: 5 })
    .onClick(this.onTap)
  }
}

/** 一张动态皮肤卡：直接显示该主题的高清插画缩略图。 */
@Component
struct SceneCard {
  @Prop index: number = 0;
  @Prop selected: boolean = false;
  onTap: () => void = () => {
  };

  build() {
    Column() {
      Image(SCENES[this.index].image)
        .width('90%').height(46).objectFit(ImageFit.Cover).borderRadius(6)
        .border({
          width: this.selected ? 2 : 1,
          color: this.selected ? SKINS[SCENES[this.index].skin].palette.accent : '#33000000'
        })
      Text(SCENES[this.index].label)
        .fontSize(11).margin({ top: 3 }).maxLines(1)
        .fontColor(this.selected ? SKINS[SCENES[this.index].skin].palette.accent
          : SKINS[SCENES[this.index].skin].palette.panelFg)
        .fontWeight(this.selected ? FontWeight.Bold : FontWeight.Normal)
    }
    .width(78).height(84).justifyContent(FlexAlign.Center)
    .margin({ left: 4, right: 4 })
    .onClick(this.onTap)
  }
}

// ---------------------------------------------------------------- 皮肤

@Component
export struct SkinPanel {
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiScene') scene: string = '';
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Column() {
      PanelHeader({
        title: '皮肤', hint: '11 款纯色 + 19 套动态；选中即换',
        onClose: () => {
          this.ctrl.closePanel();
        }
      })
      Scroll() {
        Column() {
          Text('纯色')
            .fontSize(12).fontWeight(FontWeight.Bold).fontColor(this.pal().dimFg)
            .width('100%').padding({ left: 8, top: 8, bottom: 2 })
          // 11 款纯色（SKINS[0] 是旧「不改动」，不再列出）。
          Flex({ wrap: FlexWrap.Wrap, justifyContent: FlexAlign.Start }) {
            ForEach(SKINS.slice(1), (s: Skin, i: number) => {
              SkinCard({
                index: i + 1,
                selected: this.scene.length === 0 && (i + 1) === this.skin,
                onTap: () => {
                  this.ctrl.selectSkin(i + 1);
                }
              })
            }, (s: Skin, i: number) => `skin-${i}`)
          }.width('100%').padding({ left: 8, right: 8, top: 2 })
          Text('动态')
            .fontSize(12).fontWeight(FontWeight.Bold).fontColor(this.pal().dimFg)
            .width('100%').padding({ left: 8, top: 10, bottom: 2 })
          Flex({ wrap: FlexWrap.Wrap, justifyContent: FlexAlign.Start }) {
            ForEach(SCENES, (s: SceneSkin, i: number) => {
              SceneCard({
                index: i,
                selected: this.scene === s.id,
                onTap: () => {
                  this.ctrl.selectScene(s.skin, s.id);
                }
              })
            }, (s: SceneSkin) => `scene-${s.id}`)
          }.width('100%').padding({ left: 8, right: 8, top: 2, bottom: 8 })
        }.width('100%')
      }.layoutWeight(1).width('100%')
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}

// ---------------------------------------------------------------- 键盘模式

@Component
export struct KeyboardModePanel {
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @StorageLink('opiOneHand') oneHand: number = 0;
  @StorageLink('opiFloat') floating: number = 0;
  @StorageLink('opiGame') game: number = 0;
  @StorageLink('opiShowOwn') showOwn: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Column() {
      PanelHeader({
        title: '键盘模式', hint: '可连开多个；再点一下关',
        onClose: () => {
          this.ctrl.closePanel();
        }
      })
      Scroll() {
        // Flex 折行（Android FlowRow + 可滚动）：窄屏/长辈字号不裁项。
        Flex({ wrap: FlexWrap.Wrap, justifyContent: FlexAlign.Start }) {
          ModeChip({
            label: '繁体模式', selected: this.mode === (OpiMode.Traditional as number),
            onTap: () => {
              this.ctrl.toggleTraditional();
            }
          })
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
          ModeChip({
            label: '悬浮模式', selected: this.floating === 1,
            onTap: () => {
              this.ctrl.toggleFloating();
            }
          })
          ModeChip({
            label: '游戏模式', selected: this.game === 1,
            onTap: () => {
              this.ctrl.toggleGame();
            }
          })
          ModeChip({
            label: '暗光模式', selected: darkEnabled(),
            onTap: () => {
              this.ctrl.toggleDark();
            }
          })
          ModeChip({
            label: '长辈模式', selected: elderEnabled(),
            onTap: () => {
              this.ctrl.toggleElder();
            }
          })
          ModeChip({
            label: '表情页显示自有', selected: this.showOwn === 1,
            onTap: () => {
              this.ctrl.toggleShowOwn();
            }
          })
        }.width('100%').padding({ left: 8, right: 8, top: 4, bottom: 8 })
      }.layoutWeight(1).width('100%')
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}

// ---------------------------------------------------------------- 关于

let cachedVersion: string = '';

export function appVersion(): string {
  if (cachedVersion.length > 0) {
    return cachedVersion;
  }
  try {
    cachedVersion = bundleManager.getBundleInfoForSelfSync(0).versionName;
  } catch (e) {
    console.error(`[opi] 读取版本失败：${e}`);
    cachedVersion = '未知';
  }
  return cachedVersion;
}

function toast(message: string): void {
  promptAction.showToast({ message: message, duration: 2000 });
}

/** 点链接 = 复制；IME 扩展不能 startAbility 拉起第三方应用（SDK 错误 16000069）。 */
function copyLink(url: string): void {
  try {
    const data: pasteboard.PasteData = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, url);
    pasteboard.getSystemPasteboard().setData(data).then(() => {
      toast(`链接已复制：${url}`);
    }).catch(() => {
      toast(`复制失败，请手动记录：${url}`);
    });
  } catch (e) {
    console.error(`[opi] 复制链接失败：${e}`);
    toast(`复制失败，请手动记录：${url}`);
  }
}

@Component
export struct AboutPanel {
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  @Builder
  Para(text: string, accent: boolean = false) {
    Text(text)
      .fontSize(this.elder === 1 ? 14 : 12)
      .fontColor(accent ? this.pal().accent : this.pal().panelFg)
      .width('100%').margin({ top: 3 })
      .onClick(() => {
        if (accent) {
          copyLink(text);
        }
      })
  }

  @Builder
  Head(text: string) {
    Text(text)
      .fontSize(this.elder === 1 ? 14 : 12).fontWeight(FontWeight.Bold)
      .fontColor(this.pal().panelFg).width('100%').margin({ top: 8 })
  }

  build() {
    Column() {
      PanelHeader({
        title: '关于',
        onClose: () => {
          this.ctrl.closePanel();
        }
      })
      Scroll() {
        Column() {
          this.Para(ABOUT_INTRO)
          this.Head('版本')
          this.Para(`v${appVersion()}`)
          this.Head('更新')
          UpdateSection({ currentVersion: appVersion() })
          this.Head('版权')
          this.Para('© 2026 erik.xyz · 代码按 MIT 许可（全文见仓库 LICENSE）；词库数据与代码分开授权')
          this.Head('表情数据')
          this.Para('Emoji 取自 OpenMoji（CC BY-SA 4.0）；颜文字取自 rime_kaomoji_dict（MIT）；小欧贴纸/图标为项目专有资产，商业使用需授权')
          this.Head('主页地址（点按复制）')
          this.Para(ABOUT_HOME_URL, true)
          this.Head('项目地址（点按复制）')
          this.Para(ABOUT_PROJECT_URL, true)
          this.Head('博客地址（点按复制）')
          this.Para(ABOUT_BLOG_URL, true)
          this.Head('隐私政策（点按复制）')
          this.Para(ABOUT_PRIVACY_URL, true)
          this.Para('鸿蒙输入法扩展不能直接拉起浏览器；复制后到浏览器打开。')
        }
        .width('100%').padding({ left: 14, right: 14, bottom: 10 })
      }.layoutWeight(1).width('100%')
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}

// ---------------------------------------------------------------- 输入模式

@Component
export struct InputModePanel {
  @StorageLink('opiLayout') layout: number = 0;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Column() {
      PanelHeader({
        title: '输入模式', hint: '选中即换',
        onClose: () => {
          this.ctrl.closePanel();
        }
      })
      Scroll() {
        // Flex 折行 + 可滚动（Android FlowRow）：320vp 窄屏与长辈字号下都不裁项。
        Flex({ wrap: FlexWrap.Wrap, justifyContent: FlexAlign.Start }) {
          ForEach(INPUT_MODES, (m: InputMode, i: number) => {
            Text(m.label)
              .fontSize(this.elder === 1 ? 17 : 14)
              .fontColor(i === currentModeIndex() ? this.pal().highlightFg : this.pal().panelFg)
              .backgroundColor(i === currentModeIndex() ? this.pal().highlightBg : this.pal().keyBg)
              .borderRadius(6).textAlign(TextAlign.Center)
              .padding({ left: 16, right: 16, top: 10, bottom: 10 })
              .margin({ left: 4, right: 4, top: 5 })
              .onClick(() => {
                this.ctrl.selectInputMode(i);
              })
          }, (m: InputMode, i: number) => `im-${i}`)
        }.width('100%').padding({ left: 8, right: 8, bottom: 8 })
      }.layoutWeight(1).width('100%')
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}
