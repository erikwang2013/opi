// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 顶部两行（对齐 Android ImeScreen 的「拼音行 + 候选/工具条共用槽」）：
//   Row1  = 拼音行；空闲时是随机多形态宠物条（Android PetStrip）。
//   Row2  = 有缓冲/候选 → 候选栏；否则 → 工具条（图标：表情/皮肤/输入/键盘/关于/收起）。
// 面板视图（view != 0）不画 Row1，只留工具条 —— 与 Android 的面板视图一致。
// 平铺候选（opiTiled）时只画 Row1，候选/工具条槽与键区让给 CandidateTiled。
//
// 候选不显示序号（Android 2026-10-17 裁决：手机字母盘没有数字行，序号没有对应动作）。
// 候选栏不画 ‹ ›（Android 平铺视图上线后撤掉）：右侧 ⌄ 展开平铺、上下滑翻页；
// 长按候选 = 从用户词表删词（Android CandidateBar onLongPress）。

import { OpiMode } from './OpiEngine';
import { KeyboardController } from './KeyboardController';
import { lastChineseModeIndex } from './InputModes';
import { PanelView } from './KeyboardModes';
import { OpiPet, PetMood, petMood, petStripSelection } from './OpiPet';
import { Palette, paletteFor } from './Palette';
import { stickerDataUri } from './PetStickerImages';

const PET_SIZE_IN_BAR: number = 30;
const PET_FORM_HOLD_MS: number = 2800;
/** 工具条图标按钮的点击区（vp）；五个图标 + 宠物 + 收起在 320vp 窄屏也放得下。 */
const TOOL_ICON_SIZE: number = 36;
const TOOL_ICON_GLYPH: number = 21;

function petStickerName(mood: PetMood): string {
  switch (mood) {
    case PetMood.Waiting: return 'waiting';
    case PetMood.Puzzled: return 'puzzled';
    case PetMood.Crying: return 'crying';
    case PetMood.Terrified: return 'terrified';
    case PetMood.Angry: return 'angry';
    case PetMood.Coquettish: return 'coquettish';
    case PetMood.Cute: return 'cute';
    case PetMood.Pleasing: return 'pleasing';
    case PetMood.Knocking: return 'knocking';
    case PetMood.FacePalm: return 'face-palm';
    case PetMood.TongueOut: return 'tongue-out';
    default: return 'idle';
  }
}

@Component
export struct TopBar {
  @StorageLink('opiBuffer') buffer: string = '';
  @StorageLink('opiCandidates') @Watch('onCandidatesChanged') candidates: string[] = [];
  @StorageLink('opiMode') mode: number = OpiMode.Pinyin as number;
  @StorageLink('opiLearner') learner: number = 1;
  @StorageLink('opiView') view: number = 0;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @StorageLink('opiTiled') tiled: number = 0;

  @State petForms: PetMood[] = petStripSelection(Math.random);
  private petTimer: number = -1;
  private candidatesScroller: Scroller = new Scroller();
  private ctrl: KeyboardController = KeyboardController.getInstance();

  /** 候选整批换新 → 滚动回起点（Android CandidateBar 同款，否则头几个候选留在视口外）。 */
  onCandidatesChanged(): void {
    this.candidatesScroller.scrollEdge(Edge.Start);
  }

  aboutToAppear(): void {
    this.petTimer = setInterval(() => {
      this.petForms = petStripSelection(Math.random);
    }, PET_FORM_HOLD_MS);
  }

  aboutToDisappear(): void {
    if (this.petTimer !== -1) {
      clearInterval(this.petTimer);
      this.petTimer = -1;
    }
  }

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private mood(): PetMood {
    return petMood(this.buffer, this.candidates.length, this.learner === 1,
      this.ctrl.dictionaryDegraded());
  }

  private idlePetStrip(): boolean {
    return this.view === 0 && this.buffer.length === 0 && this.candidates.length === 0;
  }

  private showCandidates(): boolean {
    // 英文**空闲**（无缓冲、无候选）显示工具条，不是 EN 条 —— Android `englishIdle` 同款。
    return this.view === 0 && (this.buffer.length > 0 || this.candidates.length > 0);
  }

  /** 平铺视图占满拼音行以下的整块（Android ImeScreen 的 CANDIDATES_TILED 分支）。 */
  private inTiled(): boolean {
    return this.view === 0 && this.tiled === 1;
  }

  private stripPetSize(): number {
    const n: number = this.petForms.length;
    if (n >= 10) {
      return 18;
    }
    if (n >= 7) {
      return 24;
    }
    return PET_SIZE_IN_BAR;
  }

  build() {
    Column() {
      if (this.view === 0 && this.mode !== (OpiMode.Symbol as number)) {
        this.ComposingRow()
      }
      if (!this.inTiled()) {
        if (this.showCandidates()) {
          this.CandidateRow()
        } else {
          this.Toolbar()
        }
      }
    }
    .width('100%')
  }

  @Builder
  ComposingRow() {
    Row() {
      if (this.idlePetStrip()) {
        Row() {
          ForEach(this.petForms, (m: PetMood, index: number) => {
            Image(stickerDataUri(petStickerName(m)))
              .width(this.stripPetSize()).height(this.stripPetSize())
              .objectFit(ImageFit.Contain).margin({ left: 2, right: 2 })
          }, (m: PetMood, index: number) => `${index}-${m}`)
        }
        .layoutWeight(1).justifyContent(FlexAlign.SpaceEvenly)
        .onClick(() => {
          this.ctrl.toggleLearner();
        })
      } else {
        Text(this.buffer.length === 0 ? ' ' : this.buffer)
          .fontSize(this.elder === 1 ? 20 : 17)
          .fontColor(this.pal().bufferFg)
          .layoutWeight(1).padding({ left: 12 }).maxLines(1)
      }
      if (this.learner === 0) {
        Text('不记词').fontSize(12).fontColor(this.pal().accent).margin({ right: 12 })
          .onClick(() => {
            this.ctrl.toggleLearner();
          })
      }
    }
    .height(32).width('100%').backgroundColor(this.pal().toolbarBg)
  }

  @Builder
  CandidateRow() {
    Row() {
      // 英文无候选 = 模式条（Android CandidateBar 的同一分支）。
      if (this.mode === (OpiMode.English as number) && this.candidates.length === 0) {
        Text('EN').fontSize(16).fontWeight(FontWeight.Bold).fontColor(this.pal().accent)
          .padding({ left: 12, right: 8 })
        Text('字母直接上屏').fontSize(13).fontColor(this.pal().dimFg).layoutWeight(1)
      } else {
        Scroll(this.candidatesScroller) {
          Row() {
            ForEach(this.candidates, (item: string, index: number) => {
              Text(item)
                .fontSize(this.elder === 1 ? 24 : 22).fontColor(this.pal().keyFg)
                .padding({ left: 12, right: 12, top: 8, bottom: 8 })
                .onClick(() => {
                  this.ctrl.tapCandidate(index);
                })
                .gesture(LongPressGesture({ repeat: false, duration: 500 }).onAction(() => {
                  this.ctrl.removeCandidateWord(item);
                }))
            }, (item: string, index: number) => `${index}-${item}`)
            if (this.candidates.length === 0 && this.buffer.length > 0) {
              Row() {
                OpiPet({
                  mood: this.mood(), petSize: PET_SIZE_IN_BAR,
                  dark: this.dark === 1, forceLegend: false
                })
                Text('无匹配').fontSize(13).fontColor(this.pal().dimFg).margin({ left: 8 })
              }.padding({ left: 8 })
            }
          }
        }
        .scrollable(ScrollDirection.Horizontal)
        .layoutWeight(1)
      }

      // ⌄ 展开平铺候选（Android CandidateBar：只在有候选时出现）。
      if (this.candidates.length > 0) {
        this.IconButton($r('sys.symbol.chevron_down'), '展开候选', () => {
          this.ctrl.expandTiled();
        })
      }
      if (this.mode === (OpiMode.English as number)) {
        this.HiddenChinese()
      }
    }
    .height(44).width('100%').backgroundColor(this.pal().barBg)
  }

  @Builder
  Toolbar() {
    Row() {
      OpiPet({
        mood: this.mood(), petSize: 24, dark: this.dark === 1, forceLegend: false
      }).margin({ left: 6, right: 2 }).onClick(() => {
        this.ctrl.toggleLearner();
      })
      this.IconButton($r('sys.symbol.face_smiling'), '表情', () => {
        this.ctrl.openEmoji();
      })
      this.IconButton($r('sys.symbol.paintpalette'), '皮肤', () => {
        this.ctrl.togglePanel(PanelView.Skin);
      })
      this.IconButton($r('sys.symbol.character_square'), '输入模式', () => {
        this.ctrl.togglePanel(PanelView.InputMode);
      })
      this.IconButton($r('sys.symbol.keyboard'), '键盘模式', () => {
        this.ctrl.togglePanel(PanelView.KeyboardMode);
      })
      this.IconButton($r('sys.symbol.gearshape'), '设置', () => {
        this.ctrl.togglePanel(PanelView.Settings);
      })
      this.IconButton($r('sys.symbol.info_circle'), '关于', () => {
        this.ctrl.togglePanel(PanelView.About);
      })
      Text('').layoutWeight(1)
      if (this.mode === (OpiMode.English as number)) {
        this.HiddenChinese()
      }
      // 收起键盘压最右（Android 2026-10-10 裁决）；图标 = 向下折角，不是 ✕。
      this.IconButton($r('sys.symbol.line_below_chevron_down'), '收起键盘', () => {
        this.ctrl.hideKeyboard();
      })
    }
    .height(44).width('100%').backgroundColor(this.pal().toolbarBg)
  }

  @Builder
  IconButton(icon: Resource, label: string, action: () => void) {
    Column() {
      SymbolGlyph(icon)
        .fontSize(TOOL_ICON_GLYPH)
        .fontColor([this.pal().toolbarFg])
        .accessibilityText(label)
    }
    .width(TOOL_ICON_SIZE).height(TOOL_ICON_SIZE)
    .justifyContent(FlexAlign.Center)
    .borderRadius(6)
    .onClick(action)
  }

  @Builder
  HiddenChinese() {
    Text('中')
      .fontSize(13).fontColor(this.pal().dimFg).padding({ left: 7, right: 7 })
      .onClick(() => {
        this.ctrl.selectInputMode(lastChineseModeIndex());
      })
  }
}
