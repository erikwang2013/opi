// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键面弹层与共用键（Android KeyPopup.kt / KeyButton.kt 的镜像）：
//   · PopupKey：长按或上滑弹出备选列表（26/14/9 键字母键），抬手提交选中项；
//   · FuncButton / BackspaceButton / ShiftButton：三张键盘共用的功能键。
// 弹层只负责画：手指的按下/拖动/抬起仍由键自己的 onTouch 收，所以 bindPopup
// 必须 focusable=false（抢焦点会中断原手势）。

import { KeyboardController } from './KeyboardController';
import { OpiShift } from './OpiEngine';
import { Palette, paletteFor } from './Palette';

/** 弹层里的一行：label 是键面字，value 是发送值（两者只对 14 键大写项不同）。 */
export class PopupChoice {
  label: string;
  value: string;
  /** 0 = 沿用发起键字号（26 键）；>0 = 本行字号（9/14 键逐行递减）。 */
  size: number;

  constructor(label: string, value: string, size: number = 0) {
    this.label = label;
    this.value = value;
    this.size = size;
  }
}

/** 弹层每格宽/高：手势按同一常数把**横向**拖动换算成「移了几格」（Android 44dp 同档）。 */
const POPUP_ITEM_WIDTH: number = 44;
const POPUP_ITEM_HEIGHT: number = 44;
/** 上滑超过这个距离就开弹层（不再等长按超时）。 */
const SWIPE_UP: number = 16;
/** 长按开弹层（Android viewConfiguration.longPressTimeoutMillis 同档）。 */
const POPUP_LONG_PRESS_MS: number = 320;

@Component
export struct PopupKey {
  @Prop label: string = '';
  /** 键面上方的小字（26 键符号提示 / 14 键数字与符号提示）。 */
  @Prop hint: string = '';
  /** 右上角小字（9 键数字徽标 / 拼音 26 键 Q..P 的 1-0）。 */
  @Prop corner: string = '';
  @Prop mainSize: number = 21;
  @Prop hintSize: number = 10;
  @Prop placementTop: boolean = true;
  choices: PopupChoice[] = [];
  defaultChoice: number = 0;
  onTap: () => void = () => {
  };
  onChoice: (choice: PopupChoice) => void = () => {
  };
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @State open: boolean = false;
  @State selected: number = 0;
  private timer: number = -1;
  private startY: number = 0;
  private lastY: number = 0;
  private startX: number = 0;
  private lastX: number = 0;
  private baseX: number = 0;
  private suppressClick: boolean = false;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  aboutToDisappear(): void {
    if (this.timer !== -1) {
      clearTimeout(this.timer);
    }
  }

  private startChoice(): void {
    this.baseX = this.lastX;
    this.selected = Math.max(0, Math.min(this.defaultChoice, this.choices.length - 1));
    this.open = true;
    // 弹层接管了这一轮按下：抬手时别再让 onClick 补发一次普通字符。
    this.suppressClick = true;
  }

  private handleTouch(event: TouchEvent): void {
    if (this.choices.length === 0) {
      return;
    }
    if (event.type === TouchType.Down) {
      if (event.touches.length === 0) {
        return;
      }
      this.startY = event.touches[0].windowY;
      this.lastY = this.startY;
      this.startX = event.touches[0].windowX;
      this.lastX = this.startX;
      this.suppressClick = false;
      if (this.timer !== -1) {
        clearTimeout(this.timer);
      }
      this.timer = setTimeout(() => {
        this.timer = -1;
        this.startChoice();
      }, POPUP_LONG_PRESS_MS);
    } else if (event.type === TouchType.Move) {
      if (event.touches.length === 0) {
        return;
      }
      this.lastY = event.touches[0].windowY;
      this.lastX = event.touches[0].windowX;
      if (!this.open) {
        if (this.startY - this.lastY > SWIPE_UP) {
          if (this.timer !== -1) {
            clearTimeout(this.timer);
            this.timer = -1;
          }
          this.startChoice();
        }
        return;
      }
      // 首项在最左：向右拖 = 选更靠后的格；在开弹层那点的基础上算位移。
      const steps: number = Math.round((this.lastX - this.baseX) / POPUP_ITEM_WIDTH);
      this.selected = Math.max(0, Math.min(this.defaultChoice + steps, this.choices.length - 1));
    } else if (event.type === TouchType.Up) {
      if (this.timer !== -1) {
        clearTimeout(this.timer);
        this.timer = -1;
        return; // 阈值内抬手 = 普通点击，交给 onClick
      }
      if (this.open) {
        this.open = false;
        this.onChoice(this.choices[Math.max(0, Math.min(this.selected, this.choices.length - 1))]);
      }
    } else {
      // 本版 SDK 的 TouchType 只有 Down/Up/Move；余下按取消处理（别把弹层留在屏上）。
      if (this.timer !== -1) {
        clearTimeout(this.timer);
        this.timer = -1;
      }
      this.open = false;
      this.suppressClick = false;
    }
  }

  private choiceSize(choice: PopupChoice): number {
    return choice.size > 0 ? choice.size : this.mainSize;
  }

  @Builder
  PopupList() {
    // 横向一行（2026-10-25 用户订正：竖向太难操作）。
    Row() {
      ForEach(this.choices, (choice: PopupChoice, index: number) => {
        Text(choice.label)
          .fontSize(this.choiceSize(choice))
          .fontColor(index === this.selected ? this.pal().highlightFg : this.pal().keyFg)
          .backgroundColor(index === this.selected ? this.pal().highlightBg : this.pal().keyBg)
          .width(POPUP_ITEM_WIDTH)
          .height(POPUP_ITEM_HEIGHT)
          .textAlign(TextAlign.Center)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }, (choice: PopupChoice, index: number) => `${index}-${choice.label}`)
    }
    .padding(4)
    .backgroundColor(this.pal().keyBg)
    .borderRadius(8)
  }

  build() {
    Column() {
      if (this.corner.length > 0) {
        Row() {
          Text(this.corner).fontSize(this.hintSize).fontColor(this.pal().dimFg)
        }
        .width('100%')
        .justifyContent(FlexAlign.End)
        .padding({ right: 4, top: 1 })
      } else if (this.hint.length > 0) {
        Text(this.hint)
          .fontSize(this.hintSize)
          .fontColor(this.pal().dimFg)
          .height(this.hintSize + 2)
      }
      Text(this.label)
        .fontSize(this.mainSize)
        .fontColor(this.pal().keyFg)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .justifyContent(FlexAlign.Center)
    .height('100%')
    .margin({ left: 2, right: 2 })
    .backgroundColor(this.pal().keyBg)
    .borderRadius(6)
    .onClick(() => {
      if (this.suppressClick) {
        this.suppressClick = false;
        return;
      }
      this.onTap();
    })
    .onTouch((event: TouchEvent) => {
      this.handleTouch(event);
    })
    .bindPopup(this.open, {
      builder: this.PopupList,
      placement: this.placementTop ? Placement.Top : Placement.Bottom,
      popupColor: this.pal().keyBg,
      enableArrow: false,
      autoCancel: false,
      focusable: false,
      radius: 8
    })
  }
}

/** 普通功能键（底行 / 数字面板）：无长按，避免 500ms 长按吞掉点击。 */
@Component
export struct FuncButton {
  @Prop label: string = '';
  @Prop fontSize: number = 15;
  @Prop weight: number = 1;
  @Prop active: boolean = false;
  onTap: () => void = () => {
  };
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Button(this.label)
      .fontSize(this.fontSize)
      .fontColor(this.active ? this.pal().highlightFg : this.pal().keyFg)
      .backgroundColor(this.active ? this.pal().highlightBg : this.pal().keyBg)
      .borderRadius(6)
      .layoutWeight(this.weight)
      .height('100%')
      .margin({ left: 2, right: 2 })
      .onClick(this.onTap)
  }
}

/** ⌫：单击删一格；长按按住连续退格，松手停止。 */
@Component
export struct BackspaceButton {
  @Prop fontSize: number = 21;
  @Prop weight: number = 1;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  build() {
    Button('⌫')
      .fontSize(this.fontSize)
      .fontColor(this.pal().keyFg)
      .backgroundColor(this.pal().keyBg)
      .borderRadius(6)
      .layoutWeight(this.weight)
      .height('100%')
      .margin({ left: 2, right: 2 })
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
}

/** ⇧：单击 OFF↔SINGLE、长按 LOCK；中文 26 键传 usable=false 当占位。
 *  ⚠️ 属性名不能叫 `enabled`：与 CustomComponent 的 `enabled(value)` 冲突（ArkTS 编译错误）。 */
@Component
export struct ShiftButton {
  @Prop fontSize: number = 21;
  @Prop weight: number = 1;
  @Prop usable: boolean = true;
  @StorageLink('opiShift') shift: number = OpiShift.Off as number;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private active(): boolean {
    return this.shift !== (OpiShift.Off as number);
  }

  build() {
    Button(this.shift === (OpiShift.Lock as number) ? '⇪' : '⇧')
      .fontSize(this.fontSize)
      .fontColor(this.active() ? this.pal().highlightFg : this.pal().keyFg)
      .backgroundColor(this.active() ? this.pal().highlightBg : this.pal().keyBg)
      .borderRadius(6)
      .layoutWeight(this.weight)
      .height('100%')
      .margin({ left: 2, right: 2 })
      .enabled(this.usable)
      .opacity(this.usable ? 1 : 0.45)
      .onClick(() => {
        this.ctrl.tapShift();
      })
      .gesture(
        LongPressGesture({ repeat: false, duration: 500 })
          .onAction(() => {
            this.ctrl.tapShiftLongPress();
          })
      )
  }
}
