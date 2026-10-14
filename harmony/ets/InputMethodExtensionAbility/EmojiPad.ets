// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**。ArkUI 的 @Component / ForEach /
// @State 在普通 tsc 下就断。
//
// 表情页（Symbol 模式）—— **2026-10-14 从 pages/Index.ets 原样搬出来**：那一页要加
// 「输入模式」的键面分支，而 Index.ets 已经贴着 500 行门禁（`opi-ffi/tests/line_limit.rs`），
// 所以把这一整块（搜索框 + 分组条 + 网格，约 120 行）搬成**独立组件**，一个字符都没改
// 语义 —— 唯一的变化是「本页」变成「本组件」、`private ctrl` 字段取代对 Index 成员的引用。
// 搬迁是**机械的**：若与 Index.ets 的历史版本对不上，先查这次搬移。

import { KeyboardController } from './KeyboardController';
import { EmojiGroup } from './OpiEngine';

/**
 * 每行几个表情。与 Android `SymbolPanel` 的 `perRow = 8` 同值（两端要一致改）。
 * ⚠️ 原先的 `SYM_ROW_1..3`（29 个 ASCII 符号）已随换页删除 —— 见 README 缺口第 3 条。
 */
const EMOJI_PER_ROW: number = 8;

/**
 * 表情页：搜索框 + 分组条 + 网格（对标 Android `SymbolPanel.kt` 的表情 Tab，2026-10-13 补齐）。
 *
 * ⚠️ 两条**如实记下**的代价（README「已知缺口」第 3 条）：
 *   ① 原 `SYM_ROW_1..3` 的 29 个 ASCII 符号不再一键可达（Android 有「常用/全部」Tab，本端没做）；
 *   ② **搜索框的输入通道未定**：面板自己就是输入法，`TextInput` 聚焦时没有第二个输入法可唤起
 *      ⇒ 真机上大概率「能显示、不能打字」（Android 的做法见 README 缺口第 3 条）——**没做**。
 */
@Component
export struct EmojiPad {
  // 本地状态：只存「看哪一组/搜什么」，**不存数据** —— 数据每次渲染现读控制器（缓存与
  // 「空结果不缓存」的纪律都在那边），两处存两份迟早漂移。
  @State emojiQuery: string = '';
  @State emojiGroup: number = 0;

  private ctrl: KeyboardController = KeyboardController.getInstance();

  build() {
    Column() {
      // ⚠️ 未编译，DevEco 上核实 `TextInput` 的 placeholder/text/onChange 三参与面板内能否聚焦；
      // 备选 `Search({ value, placeholder })`，再退 = 面板字母键直接喂 emojiQuery（见上 ②）。
      TextInput({ placeholder: '搜索（拼音/英文）', text: this.emojiQuery })
        .width('100%')
        .height(38)
        .fontSize(14)
        .onChange((v: string) => {
          this.emojiQuery = v;
        })

      if (this.emojiQuery.length === 0) {
        // 分组条：点一项 = 网格换成那一组（搜索态不显示，同 Android 的两套工具条）。
        // ⚠️ 与 Android **判据同、动作不同**：那边 `scrollToItem(组.start)` 滚动，这边**切片
        // 过滤**（见 emojiShown）—— 少一条本机查不到的 `Scroller` API，也不必重算过滤后下标。
        // ⚠️ 未编译：`Scroll` + `ScrollDirection.Horizontal`；备选
        // `List(){…}.listDirection(Axis.Horizontal)`，再退裸 Row（9 组会横向溢出）。
        Scroll() {
          Row() {
            ForEach(this.ctrl.emojiGroups(), (g: EmojiGroup, i: number) => {
              Text(g.name)
                .fontSize(14)
                .fontColor(i === this.emojiGroup ? '#FFFFFF' : '#1E6B63')
                .backgroundColor(i === this.emojiGroup ? '#2E8B80' : '#FFFFFF')
                .borderRadius(6)
                .padding({ left: 10, right: 10, top: 5, bottom: 5 })
                .margin({ left: 3, right: 3 })
                .onClick(() => {
                  this.emojiGroup = i;
                })
            }, (g: EmojiGroup, i: number) => `${i}-${g.name}`)
          }
        }
        .scrollable(ScrollDirection.Horizontal)
        .width('100%')
        .height(36)
      }

      if (this.ctrl.emojiSymbols().length === 0) {
        // 空态给一句白话、别留白（iOS 同款）。表情表**不经引擎单例**，正常冷启动第一帧就有；
        // 走到这里是模块/数据真没起来 —— 切走再切回即重试。
        Text('表情库未就绪').fontSize(14).fontColor('#9AA7A4').margin({ top: 12 })
      } else {
        // 网格（每行 EMOJI_PER_ROW 个）。⚠️ 未编译，DevEco 上核实纵向 `Scroll`；备选
        // `Grid{GridItem}.columnsTemplate('1fr '.repeat(8))`。Column/Row/ForEach 与 KeyRows 同构。
        //
        // ⚠️ 天花板：ForEach **不虚拟化** —— 选中一组就整组铺出来（「人物」2430 项 ≈ 300 行节点，
        // 真机点它可能卡）。换 `List{LazyForEach}` 要一个 IDataSource，且 LazyForEach **只能**做
        // List/Grid/Swiper 的直接子节点（塞不进 Scroll/Column）—— 骨架与判据见 README 缺口。
        //
        // ⚠️ **未做「画不出的字形」过滤**：Android 用 `Paint.hasGlyph`、iOS 用
        // `CTFontGetGlyphsForCharacters`，鸿蒙侧的对应 API 本机查不到 —— **不编一个**。
        // 现状是个别新码位（如 U+1FAEB）画成空心框；日后补上时**只判单码位**，且分组
        // start/count 要跟着重算（Android 的教训：过滤会摘掉条目，原下标整体偏前）。
        Scroll() {
          Column() {
            ForEach(this.emojiRows(), (row: string[], rowIndex: number) => {
              Row() {
                ForEach(row, (e: string, col: number) => {
                  Text(e)
                    .fontSize(24)
                    .layoutWeight(1)
                    .textAlign(TextAlign.Center)
                    .onClick(() => {
                      this.ctrl.tapEmoji(e);
                    })
                }, (e: string, col: number) => `${col}-${e}`)
              }
              .width('100%')
              .margin({ top: rowIndex === 0 ? 6 : 4 })
            }, (row: string[], rowIndex: number) => `emoji-${rowIndex}`)
          }
          .width('100%')
        }
        .layoutWeight(1).width('100%')
      }
    }
    .width('100%').backgroundColor('#FFFFFF')
  }

  /**
   * 当前该画的表情：有搜索词 → 引擎搜索 ∩ 表情集（判据在控制器/引擎，本组件不另判）；
   * 否则 → 选中分组的切片 `[start, start+count)`（「分组连续覆盖全表」由引擎门禁保证）；
   * 分组拿不到 → **整表兜底**（别让页面全空）。末尾**保序去重**是渲染前最后一道
   * （Android 同款裁决；上游已有零重复门禁，这里防将来哪层漏）。
   */
  private emojiShown(): string[] {
    let list: string[];
    if (this.emojiQuery.length > 0) {
      list = this.ctrl.searchEmoji(this.emojiQuery);
    } else {
      const groups: EmojiGroup[] = this.ctrl.emojiGroups();
      if (groups.length === 0) {
        list = this.ctrl.emojiSymbols();
      } else {
        // 组数变了（引擎重载）而 emojiGroup 还停在大下标时夹回最后一组，别越界成空。
        const g: EmojiGroup = groups[Math.min(this.emojiGroup, groups.length - 1)];
        list = this.ctrl.emojiSymbols().slice(g.start, g.start + g.count);
      }
    }
    // ⚠️ 未编译：`new Set(数组)` + `Array.from` 的 ArkTS 可用性；备选 = 手写 has/add 循环。
    return Array.from(new Set<string>(list));
  }

  /** 每行 EMOJI_PER_ROW 个切成二维（ForEach 只铺一维）。 */
  private emojiRows(): string[][] {
    const list: string[] = this.emojiShown();
    const out: string[][] = [];
    for (let i = 0; i < list.length; i += EMOJI_PER_ROW) {
      out.push(list.slice(i, i + EMOJI_PER_ROW));
    }
    return out;
  }
}
