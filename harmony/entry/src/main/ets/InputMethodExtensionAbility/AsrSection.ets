// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置页「语音输入」区块：开关 + 模型状态 + normal 档下载（进度/重试）。
// 模型在 full 档随包、首次预热时自动装；normal 档只从这里/预热外的下载入口触发。

import * as AsrModels from './AsrModels';
import * as AsrVoice from './AsrVoice';
import * as SettingsStore from './SettingsStore';
import { t } from './I18n';
import { Palette, paletteFor } from './Palette';

@Component
export struct AsrSection {
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @StorageLink('opiLang') lang: number = 0;
  @State on: boolean = false;
  @State phase: number = AsrModels.ModelPhase.Missing;
  @State done: number = 0;
  @State total: number = AsrModels.TOTAL_BYTES;
  @State reason: string = '';
  @State bundled: boolean = false;
  private timer: number = -1;

  aboutToAppear(): void {
    this.on = AsrVoice.isEnabled();
    this.refresh();
    this.timer = setInterval(() => {
      this.refresh();
    }, 500);
  }

  aboutToDisappear(): void {
    if (this.timer !== -1) {
      clearInterval(this.timer);
      this.timer = -1;
    }
  }

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private refresh(): void {
    this.bundled = AsrVoice.modelBundled();
    const st: AsrModels.ModelStatus | null = AsrVoice.modelStatus();
    if (st === null) {
      this.phase = AsrModels.ModelPhase.Missing;
      this.reason = '';
      return;
    }
    this.phase = st.phase as number;
    this.done = st.doneBytes;
    this.total = st.totalBytes;
    this.reason = st.message;
  }

  private toggle(): void {
    this.on = !this.on;
    SettingsStore.saveBool(SettingsStore.BOOL_VOICE, this.on);
    AsrVoice.setEnabled(this.on);
    this.refresh();
  }

  private percent(): number {
    if (this.total <= 0) {
      return 0;
    }
    return Math.min(100, Math.floor(this.done * 100 / this.total));
  }

  private statusText(): string {
    switch (this.phase as AsrModels.ModelPhase) {
      case AsrModels.ModelPhase.Ready:
        return t('已就绪');
      case AsrModels.ModelPhase.Bundled:
        return t('内置模型（首次使用时自动准备）');
      case AsrModels.ModelPhase.Installing:
        return t('正在准备内置模型…');
      case AsrModels.ModelPhase.Downloading:
        return `${t('正在下载模型')} ${this.percent()}%`;
      case AsrModels.ModelPhase.Failed:
        return this.reason.length > 0 ? t(this.reason) : t('语音模型不可用');
      default:
        return `${t('未下载')}（${Math.round(this.total / 1048576)} MB）`;
    }
  }

  private ready(): boolean {
    return this.phase === (AsrModels.ModelPhase.Ready as number);
  }

  build() {
    Column() {
      Row() {
        Column() {
          Text(t('启用语音输入'))
            .fontSize(this.elder === 1 ? 16 : 14).fontColor(this.pal().panelFg)
          Text(t('本地识别，录音不上传；开启后键盘工具条出现麦克风'))
            .fontSize(this.elder === 1 ? 12 : 11).fontColor(this.pal().dimFg)
            .margin({ top: 2 }).maxLines(2)
        }
        .layoutWeight(1).alignItems(HorizontalAlign.Start)
        Toggle({ type: ToggleType.Switch, isOn: this.on })
          .selectedColor(this.pal().accent)
          .onChange((isOn: boolean) => {
            if (isOn !== this.on) {
              this.toggle();
            }
          })
      }
      .width('100%').padding({ left: 14, right: 14, top: 8, bottom: 8 })

      if (this.on) {
        Row() {
          Text(t('模型'))
            .fontSize(this.elder === 1 ? 16 : 14).fontColor(this.pal().panelFg)
          Text(this.statusText())
            .fontSize(this.elder === 1 ? 14 : 12)
            .fontColor(this.phase === (AsrModels.ModelPhase.Failed as number)
              ? this.pal().accent : this.pal().dimFg)
            .layoutWeight(1).textAlign(TextAlign.End).margin({ left: 8 })
            .maxLines(2).textOverflow({ overflow: TextOverflow.Ellipsis })
        }
        .width('100%').padding({ left: 14, right: 14, top: 6, bottom: 6 })

        if (this.phase === (AsrModels.ModelPhase.Downloading as number)) {
          Progress({ value: this.done, total: this.total, type: ProgressType.Linear })
            .width('100%').padding({ left: 14, right: 14 })
            .color(this.pal().accent)
        } else if (!this.ready() && !this.bundled) {
          Button(this.phase === (AsrModels.ModelPhase.Failed as number)
            ? t('重试下载') : t('下载语音模型'))
            .fontSize(13).height(32).margin({ left: 14, right: 14, top: 4, bottom: 8 })
            .backgroundColor(this.pal().accent).fontColor(this.pal().highlightFg)
            .onClick(() => {
              AsrVoice.downloadModel();
              this.refresh();
            })
        } else if (this.phase === (AsrModels.ModelPhase.Failed as number)) {
          Button(t('重试准备'))
            .fontSize(13).height(32).margin({ left: 14, right: 14, top: 4, bottom: 8 })
            .backgroundColor(this.pal().accent).fontColor(this.pal().highlightFg)
            .onClick(() => {
              AsrVoice.retryPrepare();
              this.refresh();
            })
        }
      }
    }
    .width('100%')
  }
}
