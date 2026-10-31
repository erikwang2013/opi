// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置页「跨设备剪贴板」区块。只在键盘设置页可见；数据由 ClipSync 每 500ms 投影一次。
// 安全：SAS 必须人工核对后点「允许」；移除信任两步确认；内容不上屏、不写日志。

import { BusinessError } from '@kit.BasicServicesKit';
import * as ClipSync from './ClipSync';
import { ClipPairingPrompt, ClipPeer, ClipStatus } from './ClipSync';
import { Palette, paletteFor } from './Palette';

@Component
export struct ClipSection {
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @State clipOn: boolean = false;
  @State running: boolean = false;
  @State peers: ClipPeer[] = [];
  @State promptId: string = '';
  @State promptSas: string = '';
  @State promptIncoming: boolean = false;
  @State fingerprint: string = '';
  @State lastSource: string = '';
  @State lastPairing: string = '';
  @State lastError: string = '';
  @State readLimited: boolean = false;
  @State confirmForget: string = '';
  private timer: number = -1;

  aboutToAppear(): void {
    this.clipOn = ClipSync.sharingEnabled();
    this.refresh();
    this.timer = setInterval((): void => {
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

  private fs(normal: number): number {
    return this.elder === 1 ? normal + 2 : normal;
  }

  private refresh(): void {
    this.running = ClipSync.isRunning();
    this.peers = ClipSync.peerList();
    const st: ClipStatus | null = ClipSync.currentStatus();
    this.fingerprint = st !== null ? st.fingerprint : '';
    const pr: ClipPairingPrompt | null = ClipSync.prompt();
    this.promptId = pr !== null ? pr.instanceId : '';
    this.promptSas = pr !== null ? pr.sas : '';
    this.promptIncoming = pr !== null ? pr.incoming : false;
    this.lastSource = ClipSync.sourceDevice();
    this.lastPairing = ClipSync.pairingResult();
    this.lastError = ClipSync.errorText();
    this.readLimited = ClipSync.isReadLimited();
  }

  private sasText(): string {
    return this.promptSas.length === 6
      ? `${this.promptSas.substring(0, 3)} ${this.promptSas.substring(3)}`
      : this.promptSas;
  }

  private toggle(on: boolean): void {
    this.clipOn = on;
    ClipSync.setEnabled(on);
    this.refresh();
  }

  @Builder
  Info(label: string, value: string, dim: boolean) {
    Row() {
      Text(label).fontSize(this.fs(13)).fontColor(this.pal().panelFg)
      Text(value)
        .fontSize(this.fs(11)).fontColor(dim ? this.pal().dimFg : this.pal().panelFg)
        .layoutWeight(1).textAlign(TextAlign.End).margin({ left: 8 })
        .maxLines(2).textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%').padding({ left: 14, right: 14, top: 6, bottom: 6 })
  }

  @Builder
  Hint(text: string) {
    Text(text)
      .fontSize(this.fs(10)).fontColor(this.pal().dimFg)
      .width('100%').padding({ left: 14, right: 14, top: 2, bottom: 6 })
  }

  @Builder
  MiniButton(label: string, primary: boolean, action: () => void) {
    Button(label)
      .fontSize(this.fs(12)).height(30)
      .backgroundColor(primary ? this.pal().accent : this.pal().keyBg)
      .fontColor(primary ? this.pal().highlightFg : this.pal().keyFg)
      .margin({ left: 6 })
      .onClick(action)
  }

  build() {
    Column() {
      Row() {
        Column() {
          Text('跨设备剪贴板')
            .fontSize(this.fs(14)).fontColor(this.pal().panelFg)
          Text('仅在键盘输入时运行；默认关闭')
            .fontSize(this.fs(11)).fontColor(this.pal().dimFg).margin({ top: 2 })
        }
        .layoutWeight(1).alignItems(HorizontalAlign.Start)
        Toggle({ type: ToggleType.Switch, isOn: this.clipOn })
          .selectedColor(this.pal().accent)
          .onChange((isOn: boolean) => {
            if (isOn !== this.clipOn) {
              this.toggle(isOn);
            }
          })
      }
      .width('100%').padding({ left: 14, right: 14, top: 8, bottom: 8 })

      if (!this.running) {
        this.Info('状态', this.clipOn ? '等待键盘输入会话' : '已关闭', true)
      } else {
        this.Info('状态', `运行中 · 发现 ${this.peers.length} 台设备`, false)
        if (this.fingerprint.length > 0) {
          this.Info('本机指纹', this.fingerprint, true)
        }
      }
      if (this.lastSource.length > 0) {
        this.Info('最近来源', this.lastSource, true)
      }
      if (this.lastPairing.length > 0) {
        this.Info('上次配对', this.lastPairing, true)
      }
      if (this.lastError.length > 0) {
        Text(this.lastError)
          .fontSize(this.fs(11)).fontColor('#C62828')
          .width('100%').padding({ left: 14, right: 14, top: 2, bottom: 4 })
      }

      // ---- 配对确认：SAS 只能人工核对，绝不自动接受 ----
      if (this.promptId.length > 0) {
        Column() {
          Text(this.promptIncoming ? '对方请求配对' : '确认配对')
            .fontSize(this.fs(12)).fontColor(this.pal().dimFg)
          Text(this.sasText())
            .fontSize(this.fs(26)).fontWeight(FontWeight.Bold)
            .fontColor(this.pal().panelFg).letterSpacing(4).margin({ top: 4, bottom: 4 })
          Text('核对两台设备显示相同的 6 位数字；不一致请拒绝。')
            .fontSize(this.fs(10)).fontColor(this.pal().dimFg).maxLines(2)
          Row() {
            this.MiniButton('拒绝', false, () => {
              ClipSync.pairConfirm(this.promptId, false);
              this.refresh();
            })
            this.MiniButton('允许', true, () => {
              ClipSync.pairConfirm(this.promptId, true);
              this.refresh();
            })
          }.margin({ top: 8 })
        }
        .width('100%').alignItems(HorizontalAlign.Start)
        .padding({ left: 14, right: 14, top: 8, bottom: 8 })
        .backgroundColor(this.pal().keyBg)
      }

      // ---- 系统剪贴板 → 远端：PasteButton 的点击给一次临时读取授权 ----
      if (this.running) {
        Row() {
          PasteButton({ icon: PasteIconStyle.LINES, text: PasteDescription.PASTE })
            .fontSize(this.fs(12)).height(32)
            .onClick((event: ClickEvent, result: PasteButtonOnClickResult, error?: BusinessError<void>) => {
              if (result === PasteButtonOnClickResult.SUCCESS) {
                ClipSync.publishCurrentClipboard();
              } else {
                ClipSync.markReadDenied();
              }
              this.refresh();
            })
          Text('把当前剪贴板发到已配对设备')
            .fontSize(this.fs(10)).fontColor(this.pal().dimFg)
            .layoutWeight(1).margin({ left: 8 }).maxLines(2)
        }
        .width('100%').padding({ left: 14, right: 14, top: 4, bottom: 8 })
      }
      if (this.readLimited) {
        this.Hint('系统限制后台读取剪贴板：点「粘贴」手动同步一次，或从系统复制后立即同步。')
      }
      this.Hint('收到的文本只写入系统剪贴板，不会自动粘贴；请回到应用里手动粘贴。')

      // ---- 设备列表 ----
      if (this.running) {
        Text('设备')
          .fontSize(this.fs(11)).fontWeight(FontWeight.Bold).fontColor(this.pal().dimFg)
          .width('100%').padding({ left: 14, right: 14, top: 8, bottom: 2 })
        if (this.peers.length === 0) {
          this.Hint('未发现设备。确保两台设备在同一局域网、都打开开关，且本机未被基础体验模式限制。')
        }
        ForEach(this.peers, (peer: ClipPeer) => {
          Row() {
            Column() {
              Text(peer.name.length > 0 ? peer.name : peer.instanceId.substring(0, 8))
                .fontSize(this.fs(14)).fontColor(this.pal().panelFg)
              Text(`${peer.connected ? '已连接' : '未连接'}${peer.trusted ? ' · 已配对' : ''}`)
                .fontSize(this.fs(10))
                .fontColor(peer.connected ? this.pal().accent : this.pal().dimFg)
                .margin({ top: 2 })
            }
            .layoutWeight(1).alignItems(HorizontalAlign.Start)
            if (!peer.trusted && peer.connected) {
              this.MiniButton('配对', true, () => {
                ClipSync.pairStart(peer.instanceId);
                this.refresh();
              })
            }
            if (peer.trusted) {
              if (this.confirmForget === peer.instanceId) {
                this.MiniButton('确认移除', true, () => {
                  ClipSync.forgetPeer(peer.instanceId);
                  this.confirmForget = '';
                  this.refresh();
                })
              } else {
                this.MiniButton('移除', false, () => {
                  this.confirmForget = peer.instanceId;
                })
              }
            }
          }
          .width('100%').padding({ left: 14, right: 14, top: 6, bottom: 6 })
        }, (peer: ClipPeer): string => peer.instanceId)
      }

      if (this.clipOn) {
        Button('立即停止共享')
          .fontSize(this.fs(12)).height(32).width('100%')
          .backgroundColor(this.pal().keyBg).fontColor(this.pal().keyFg)
          .margin({ top: 8 })
          .onClick(() => {
            this.toggle(false);
          })
      }
      this.Hint('基础体验模式会限制后台网络与剪贴板能力；若始终发现不到设备，请在系统设置中退出基础体验模式后重试。')
    }
    .width('100%')
  }
}
