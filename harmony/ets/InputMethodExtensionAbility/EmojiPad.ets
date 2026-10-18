// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。ArkUI 的 @Component / ForEach /
// @State 在普通 tsc 下就断。
//
// 表情页（Symbol 模式），2026-10-18 重写为与 Android `keyboard/EmojiPanel.kt` 同布局：
//
//   [‹ 返回]   [自有 | Emoji | 颜文字]   [🔍]
//   ┌────────────── 网格 ──────────────┐
//   └──── 可横滑的分类条（底部一条）────┘
//
// · **自有** = 独立顶级分类（用户裁决）：4 列网格画 `PetStickerImages.ets` 内嵌的
//   12 张小欧 PNG；点击**复制图片到系统剪贴板**（`PetStickerClipboard.ets`）
//   并弹 toast，**不提交任何文本**。自有页没有分类条、也没有 🔍。
// · **Emoji** 分类 = **只用**引擎 `emojiGroups()`（9 组，不再加 `自有` 偏移）。
// · **颜文字** 分类 = `KaomojiData.ets` 的 7 类（从 data/raw/kaomoji.tsv 机械生成；
//   解析/分类口径同 Android `KaomojiCategories`）。
// · 分类点击 = **切片**到该组首项（本端不做 `Scroller` 滚动：网格只画当前组，
//   与旧版分组条同一取舍；见 README 缺口 3）。因此高亮恒为「当前组」。
// · 搜索态保持既有两套工具条：`[‹ 返回][搜索框]`；返回 = 清查询回浏览态。
//   搜索只覆盖 Emoji / 颜文字（自有是图片，没有可搜文本）。
//   ⚠️ 搜索框的输入通道仍未解决（README 缺口 3 ②）：面板自己就是输入法，
//   `TextInput` 聚焦时没有第二个输入法可唤起 —— 真机上大概率「能显示、不能打字」。
//
// ⚠️ 网格仍是 `Scroll + Column + ForEach`（全量铺节点，不虚拟化），理由与代价见
// README 缺口 3 ④。**未做画不出的字形过滤**（缺口 3 ③）。

import { KeyboardController } from './KeyboardController';
import { EmojiGroup, OpiMode } from './OpiEngine';
import { OpiPet, PetMood } from './OpiPet';
import { nextMode } from './InputModes';
import { KAOMOJI_CATEGORIES } from './KaomojiData';
import {
  PET_STICKER_FORMS, PetStickerForm, stickerBase64, stickerDataUri,
} from './PetStickerImages';
import { copyStickerToClipboard } from './PetStickerClipboard';

/** 每行几个表情。与 Android `EmojiPanel` 的 8 列同值（两端要一致改）。 */
const EMOJI_PER_ROW: number = 8;
/** 自有贴纸独立 4 列网格（12 形态 = 3 行）。 */
const STICKER_PER_ROW: number = 4;
/** 贴纸显示尺寸（vp）。 */
const STICKER_SIZE: number = 56;
/** 颜文字比单个 emoji 宽：3 列 + 小字号，少截断（Android 同款）。 */
const KAOMOJI_PER_ROW: number = 3;

@Component
export struct EmojiPad {
  // 本地状态：只存「看哪个子页/哪一组/搜什么」，**不存数据** —— 数据每次渲染现读
  // 控制器（缓存与「空结果不缓存」的纪律都在那边），两处存两份迟早漂移。
  @State emojiQuery: string = '';
  /** 是否处于「点 🔍 才出现」的搜索工具条（同 Android 的 `searchWanted`）。 */
  @State searching: boolean = false;
  /** 子页：0 = 自有，1 = Emoji，2 = 颜文字（默认 Emoji，保持改写前的行为）。 */
  @State subTab: number = 1;
  /** Emoji 分类下标（引擎组下标，0 起；0 不再是「自有」）。 */
  @State emojiGroup: number = 0;
  /** 颜文字分类下标（KAOMOJI_CATEGORIES 下标）。 */
  @State kaomojiGroup: number = 0;
  /**
   * 搜索结果（已切片成二维行）。在 onChange 里算一次 —— 别在 build 里算：
   * `searchEmoji` 每次都会打引擎（控制器不缓存中间结果），build 里多处读会重复解码。
   */
  @State searchRows: string[][] = [];

  private ctrl: KeyboardController = KeyboardController.getInstance();

  build() {
    Column() {
      this.TopBar()
      if (this.searching && this.emojiQuery.length > 0) {
        this.SearchGrid()
      } else if (this.subTab === 0) {
        this.StickerBrowse()
      } else if (this.subTab === 1) {
        this.EmojiBrowse()
      } else {
        this.KaomojiBrowse()
      }
      // 自有是图片网格：没有分类条（它只有一页 12 格）。
      if (!this.searching && this.subTab !== 0) {
        this.CategoryBar()
      }
    }
    .width('100%').backgroundColor('#FFFFFF')
  }

  // ------------------------------------------------------------ 顶部工具条

  /**
   * 浏览态 `[‹ 返回][自有 | Emoji | 颜文字][🔍]`；搜索态 `[‹ 返回][搜索框]`。
   * 自有是图片页：没有分类条，也不画 🔍（见 `build`）。
   *
   * ⚠️ 搜索框**不常驻**：它是「点 🔍 才出现」的那一态（与 Android 2026-10-12
   * 裁决的两套工具条同构）。
   */
  @Builder
  TopBar() {
    Row() {
      Text('‹ 返回')
        .fontSize(15).fontColor('#1E6B63').padding({ left: 12, right: 10 })
        .onClick(() => { this.back(); })

      if (this.searching) {
        TextInput({ placeholder: '搜索（拼音/英文）', text: this.emojiQuery })
          .layoutWeight(1).height(34).fontSize(14).margin({ right: 8 })
          .onChange((v: string) => { this.updateSearch(v); })
      } else {
        // 子页切换：选中态同时用底色 + 字色表达（同旧版分组条）。
        Row() {
          Text('自有')
            .fontSize(15).fontColor(this.subTab === 0 ? '#FFFFFF' : '#1E6B63')
            .backgroundColor(this.subTab === 0 ? '#2E8B80' : '#FFFFFF')
            .borderRadius(6).padding({ left: 14, right: 14, top: 5, bottom: 5 })
            .margin({ left: 3, right: 3 })
            .onClick(() => { this.selectSub(0); })
          Text('Emoji')
            .fontSize(15).fontColor(this.subTab === 1 ? '#FFFFFF' : '#1E6B63')
            .backgroundColor(this.subTab === 1 ? '#2E8B80' : '#FFFFFF')
            .borderRadius(6).padding({ left: 14, right: 14, top: 5, bottom: 5 })
            .margin({ left: 3, right: 3 })
            .onClick(() => { this.selectSub(1); })
          Text('颜文字')
            .fontSize(15).fontColor(this.subTab === 2 ? '#FFFFFF' : '#1E6B63')
            .backgroundColor(this.subTab === 2 ? '#2E8B80' : '#FFFFFF')
            .borderRadius(6).padding({ left: 14, right: 14, top: 5, bottom: 5 })
            .margin({ left: 3, right: 3 })
            .onClick(() => { this.selectSub(2); })
        }
        .layoutWeight(1).justifyContent(FlexAlign.Center)

        // 自有是图片，没有可搜文本：这一页不画 🔍（别留点了没反应的按钮）。
        if (this.subTab !== 0) {
          Text('🔍')
            .fontSize(18).padding({ left: 10, right: 14 })
            .onClick(() => { this.searching = true; })
        }
      }
    }
    .width('100%').height(44).backgroundColor('#FFFFFF')
  }

  // ------------------------------------------------------------ 网格

  /** 自有子页：4 列网格，画真实的 12 张小欧 PNG；点击复制图片（不上屏）。 */
  @Builder
  StickerBrowse() {
    Scroll() {
      Column() {
        ForEach(this.stickerRows(), (row: PetStickerForm[], rowIndex: number) => {
          Row() {
            ForEach(row, (f: PetStickerForm, col: number) => {
              Column() {
                Image(stickerDataUri(f.sticker))
                  .width(STICKER_SIZE).height(STICKER_SIZE)
                  .objectFit(ImageFit.Contain)
                Text(f.label).fontSize(10).fontColor('#1E6B63').margin({ top: 2 })
              }
              .layoutWeight(1).height(78).justifyContent(FlexAlign.Center)
              .onClick(() => {
                // 剪贴板回退：IME Kit 没有往宿主输入框插图片的接口（见 README 缺口）。
                copyStickerToClipboard(stickerBase64(f.sticker), f.label);
              })
            }, (f: PetStickerForm, col: number) => `${col}-${f.sticker}`)
            this.Blanks(STICKER_PER_ROW - row.length)
          }
          .width('100%').margin({ top: rowIndex === 0 ? 6 : 2 })
        }, (row: PetStickerForm[], rowIndex: number) => `sticker-${rowIndex}`)
      }
      .width('100%')
    }
    .layoutWeight(1).width('100%')
  }

  /** Emoji 子页：引擎表情表当前组；分组拿不到时整表兜底。 */
  @Builder
  EmojiBrowse() {
    Scroll() {
      Column() {
        if (this.ctrl.emojiSymbols().length === 0) {
          // 表情表**不经引擎单例**，正常冷启动第一帧就有；走到这里是模块/数据真没起来。
          Text('表情库未就绪').fontSize(14).fontColor('#9AA7A4').margin({ top: 12 })
        } else {
          this.EmojiRows()
        }
      }
      .width('100%')
    }
    .layoutWeight(1).width('100%')
  }

  /** 颜文字子页：数据缺失时显示空态（不崩 IME，理由同符号页的「没有类目数据」）。 */
  @Builder
  KaomojiBrowse() {
    if (KAOMOJI_CATEGORIES.length === 0) {
      Text('没有颜文字数据').fontSize(14).fontColor('#9AA7A4').margin({ top: 12 })
    } else {
      Scroll() {
        Column() {
          this.KaomojiRows()
        }
        .width('100%')
      }
      .layoutWeight(1).width('100%')
    }
  }

  /** 搜索态网格（两子页共用；列宽/字号按子页定，颜文字要窄列）。 */
  @Builder
  SearchGrid() {
    if (this.searchRows.length === 0) {
      Row() {
        OpiPet({ mood: PetMood.Puzzled, size: 30, dark: false, forceLegend: false })
        Text('无匹配').fontSize(14).fontColor('#9AA7A4').margin({ left: 8 })
      }
      .width('100%').layoutWeight(1).justifyContent(FlexAlign.Center)
    } else {
      Scroll() {
        Column() {
          this.SearchRows()
        }
        .width('100%')
      }
      .layoutWeight(1).width('100%')
    }
  }

  // ------------------------------------------------------------ 行渲染

  /** 引擎表情表当前组的行。 */
  @Builder
  EmojiRows() {
    ForEach(this.emojiRows(), (row: string[], rowIndex: number) => {
      Row() {
        ForEach(row, (e: string, col: number) => {
          Text(e)
            .fontSize(24).layoutWeight(1).textAlign(TextAlign.Center)
            .onClick(() => { this.ctrl.tapEmoji(e); })
        }, (e: string, col: number) => `${col}-${e}`)
        this.Blanks(EMOJI_PER_ROW - row.length)
      }
      .width('100%').margin({ top: rowIndex === 0 ? 4 : 2 })
    }, (row: string[], rowIndex: number) => `emoji-${rowIndex}`)
  }

  /** 颜文字当前分类的行（3 列 + 14 号字）。 */
  @Builder
  KaomojiRows() {
    ForEach(this.kaomojiRows(), (row: string[], rowIndex: number) => {
      Row() {
        ForEach(row, (k: string, col: number) => {
          Text(k)
            .fontSize(14).layoutWeight(1).textAlign(TextAlign.Center)
            .onClick(() => { this.ctrl.tapEmoji(k); })
        }, (k: string, col: number) => `${col}-${k}`)
        this.Blanks(KAOMOJI_PER_ROW - row.length)
      }
      .width('100%').margin({ top: rowIndex === 0 ? 4 : 2 })
    }, (row: string[], rowIndex: number) => `kaomoji-${rowIndex}`)
  }

  /** 搜索结果的行：Emoji 8 列 / 20 号字，颜文字 3 列 / 14 号字（Android 同款）。 */
  @Builder
  SearchRows() {
    ForEach(this.searchRows, (row: string[], rowIndex: number) => {
      Row() {
        ForEach(row, (s: string, col: number) => {
          Text(s)
            .fontSize(this.subTab === 1 ? 20 : 14).layoutWeight(1).textAlign(TextAlign.Center)
            .onClick(() => { this.ctrl.tapEmoji(s); })
        }, (s: string, col: number) => `${col}-${s}`)
        this.Blanks((this.subTab === 1 ? EMOJI_PER_ROW : KAOMOJI_PER_ROW) - row.length)
      }
      .width('100%').margin({ top: rowIndex === 0 ? 4 : 2 })
    }, (row: string[], rowIndex: number) => `search-${rowIndex}`)
  }

  /** 补齐末行空格，避免最后一行的格子被 `layoutWeight` 拉伸成整行（同 Android 网格）。 */
  @Builder
  Blanks(n: number) {
    ForEach(this.range(n), (b: number) => {
      Text('').layoutWeight(1)
    }, (b: number) => `blank-${b}`)
  }

  // ------------------------------------------------------------ 底部分类条

  /**
   * 可横滑的分类条（底部一条；选中 = 强调色 + 药丸底）。
   *
   * 与 Android 的差别**只在动作**：那边 `scrollToItem(组.start)` 滚动，这边切成
   * 该组的切片（同旧版 EmojiPad）。省一条本机查不到的 `Scroller` API，也不必重算
   * 过滤后的下标 ⇒ 高亮 = 当前组，天然与「可见项」一致。
   */
  @Builder
  CategoryBar() {
    Scroll() {
      Row() {
        ForEach(this.categoryNames(), (name: string, i: number) => {
          Text(name)
            .fontSize(13)
            .fontColor(i === this.selectedCategory() ? '#FFFFFF' : '#1E6B63')
            .backgroundColor(i === this.selectedCategory() ? '#2E8B80' : '#FFFFFF')
            .borderRadius(8).padding({ left: 10, right: 10, top: 6, bottom: 6 })
            .margin({ left: 3, right: 3 })
            .onClick(() => { this.selectCategory(i); })
        }, (name: string, i: number) => `${i}-${name}`)
      }
    }
    .scrollable(ScrollDirection.Horizontal)
    .width('100%').height(40).backgroundColor('#FFFFFF')
  }

  // ------------------------------------------------------------ 状态与数据

  /**
   * 返回：搜索态 → 清查询回浏览态；浏览态 → 退出符号模式。
   *
   * 本端没有 Android 的 `onClose`（表情页是 Symbol 模式整片换的），退出就是切模式；
   * 用 `nextMode` 与功能行模式键**同一张表**（符号的下一档 = 拼音）。
   */
  private back(): void {
    if (this.searching) {
      this.searching = false;
      this.emojiQuery = '';
      this.searchRows = [];
      return;
    }
    this.ctrl.switchMode(nextMode(OpiMode.Symbol as number));
  }

  /** 搜索词变化：更新查询并按子页重算结果（只算一次，见 [searchRows]）。 */
  private updateSearch(q: string): void {
    this.emojiQuery = q;
    if (q.length === 0) {
      this.searchRows = [];
      return;
    }
    if (this.subTab === 1) {
      // `searchEmoji` 已与表情集求交（同 Android `filter(catalog::isEmoji)`）。
      this.searchRows = this.chunk(this.dedup(this.ctrl.searchEmoji(q)), EMOJI_PER_ROW);
    } else {
      this.searchRows = this.chunk(this.searchKaomoji(q), KAOMOJI_PER_ROW);
    }
  }

  /** 当前 Emoji 分类下标（夹紧：引擎重载后组数变少时不越界；0 起，无「自有」偏移）。 */
  private selectedEmojiGroup(): number {
    const n: number = this.ctrl.emojiGroups().length;
    if (n === 0) {
      return 0;
    }
    return Math.max(0, Math.min(this.emojiGroup, n - 1));
  }

  /** 当前颜文字分类下标（夹紧同上）。 */
  private selectedKaomojiGroup(): number {
    return Math.min(this.kaomojiGroup, KAOMOJI_CATEGORIES.length - 1);
  }

  /** 分类条的名称：Emoji = 引擎的 9 组（不含「自有」）；颜文字 = 内嵌数据的类名。 */
  private categoryNames(): string[] {
    const out: string[] = [];
    if (this.subTab === 1) {
      const groups: EmojiGroup[] = this.ctrl.emojiGroups();
      for (let i = 0; i < groups.length; i++) {
        out.push(groups[i].name);
      }
    } else {
      for (let i = 0; i < KAOMOJI_CATEGORIES.length; i++) {
        out.push(KAOMOJI_CATEGORIES[i].label);
      }
    }
    return out;
  }

  /** 分类条当前高亮（与 [categoryNames] 的下标同一坐标系）。 */
  private selectedCategory(): number {
    if (this.subTab === 1) {
      return this.selectedEmojiGroup();
    }
    return this.selectedKaomojiGroup();
  }

  private selectCategory(i: number): void {
    if (this.subTab === 1) {
      this.emojiGroup = i;
    } else {
      this.kaomojiGroup = i;
    }
  }

  /**
   * 当前该画的引擎表情：选中组 → `[start, start+count)` 切片；
   * 分组拿不到 → **整表兜底**（别让页面全空）。末尾保序去重是渲染前最后一道
   * （Android 同款裁决；上游已有零重复门禁，这里防将来哪层漏）。
   */
  private emojiRows(): string[][] {
    const groups: EmojiGroup[] = this.ctrl.emojiGroups();
    const all: string[] = this.ctrl.emojiSymbols();
    let list: string[];
    if (groups.length === 0) {
      list = all;
    } else {
      // 子页已删「自有」，下标就是引擎组下标（0 起）。
      const g: EmojiGroup = groups[this.selectedEmojiGroup()];
      list = all.slice(g.start, g.start + g.count);
    }
    return this.chunk(this.dedup(list), EMOJI_PER_ROW);
  }

  /** 当前颜文字分类的条目（拼平口径见 KaomojiData.ets 头注）。 */
  private kaomojiRows(): string[][] {
    if (KAOMOJI_CATEGORIES.length === 0) {
      return [];
    }
    return this.chunk(KAOMOJI_CATEGORIES[this.selectedKaomojiGroup()].items, KAOMOJI_PER_ROW);
  }

  /** 颜文字搜索：拼平后按子串过滤、保序去重（Android `contains(ignoreCase)` + distinct）。 */
  private searchKaomoji(q: string): string[] {
    const needle: string = q.toLowerCase();
    const seen: Set<string> = new Set<string>();
    const out: string[] = [];
    for (let c = 0; c < KAOMOJI_CATEGORIES.length; c++) {
      const items: string[] = KAOMOJI_CATEGORIES[c].items;
      for (let i = 0; i < items.length; i++) {
        const item: string = items[i];
        if (item.toLowerCase().indexOf(needle) >= 0 && !seen.has(item)) {
          seen.add(item);
          out.push(item);
        }
      }
    }
    return out;
  }

  /** 切子页：离开搜索态（搜索只属于 Emoji/颜文字）；切换后网格按新子页重画。 */
  private selectSub(i: number): void {
    if (i === this.subTab) {
      return;
    }
    this.subTab = i;
    this.searching = false;
    this.emojiQuery = '';
    this.searchRows = [];
  }

  /** 12 个自有贴纸切片成二维（ForEach 只铺一维；4 列 = 3 行）。 */
  private stickerRows(): PetStickerForm[][] {
    const out: PetStickerForm[][] = [];
    for (let i = 0; i < PET_STICKER_FORMS.length; i += STICKER_PER_ROW) {
      out.push(PET_STICKER_FORMS.slice(i, i + STICKER_PER_ROW));
    }
    return out;
  }

  private chunk(list: string[], perRow: number): string[][] {
    const out: string[][] = [];
    for (let i = 0; i < list.length; i += perRow) {
      out.push(list.slice(i, i + perRow));
    }
    return out;
  }

  /** 保序去重（不用 Array.from：U23 的泛型构造未验证，手写循环更稳）。 */
  private dedup(list: string[]): string[] {
    const seen: Set<string> = new Set<string>();
    const out: string[] = [];
    for (let i = 0; i < list.length; i++) {
      if (!seen.has(list[i])) {
        seen.add(list[i]);
        out.push(list[i]);
      }
    }
    return out;
  }

  /** `[0, n)` 的下标数组，给末行补位用。 */
  private range(n: number): number[] {
    const out: number[] = [];
    for (let i = 0; i < n; i++) {
      out.push(i);
    }
    return out;
  }
}
