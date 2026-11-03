// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键盘内「跨设备传输」面板（工具条传输图标进入，设置开关开启才显示）。
// 设备发现/信任/配对沿用 ClipSync（同一个 opi-clip 节点）；本页只做选择、发送、
// 接收答复与进度投影。文件路径只来自系统 picker，入站只落应用沙箱，不需要新权限。
// 布局跟随设置页：PanelHeader + Scroll，颜色全部取当前皮肤。

import { BusinessError } from '@kit.BasicServicesKit';
import * as ClipSync from './ClipSync';
import * as TransferSync from './TransferSync';
import { t } from './I18n';
import { KeyboardController } from './KeyboardController';
import { Palette, paletteFor, sceneKeyBg } from './Palette';
import { PanelHeader } from './Panels';
import { ClipPeer } from './ClipTypes';
import { ClipTransfer, TransferPhase, formatTransferBytes } from './TransferProtocol';
import { TransferKind } from './TransferFiles';

@Component
export struct TransferPanel {
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  /** 订阅语言键：面板打开时切换语言也能当帧重渲染。 */
  @StorageLink('opiLang') lang: number = 0;
  @State peers: ClipPeer[] = [];
  @State transfers: ClipTransfer[] = [];
  @StorageLink('opiClipSelectedPeer') selectedId: string = '';
  @StorageLink('opiClipOnlySelected') onlySelected: boolean = false;
  @State running: boolean = false;
  @State message: string = '';
  @State textOpen: boolean = false;
  @State draft: string = '';
  @State busy: boolean = false;
  @State openNote: string = '';
  private timer: number = -1;
  private ctrl: KeyboardController = KeyboardController.getInstance();

  aboutToAppear(): void {
    this.refresh();
    this.applyRoutes();
    this.timer = setInterval((): void => {
      this.refresh();
    }, 500);
  }

  private applyRoutes(): void {
    TransferSync.setTextRoutes(this.onlySelected ? this.selectedId : '');
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
    const wasRunning: boolean = this.running;
    this.running = ClipSync.isRunning();
    if (this.onlySelected && !wasRunning && this.running) {
      this.applyRoutes();
    }
    const peers: ClipPeer[] = ClipSync.peerList().slice();
    const previousSelection: string = this.selectedId;
    this.peers = peers;
    this.transfers = TransferSync.list().slice();
    this.message = TransferSync.feedback();
    let stillThere: boolean = false;
    for (let i = 0; i < peers.length; i++) {
      const peer: ClipPeer = peers[i];
      if (peer.instanceId === this.selectedId && peer.trusted && peer.connected) {
        stillThere = true;
        break;
      }
    }
    if (!stillThere) {
      this.selectedId = '';
      for (let i = 0; i < peers.length; i++) {
        if (peers[i].trusted && peers[i].connected) {
          this.selectedId = peers[i].instanceId;
          break;
        }
      }
    }
    if (this.onlySelected && this.selectedId !== previousSelection) {
      this.applyRoutes();
    }
  }

  private selectedName(): string {
    for (let i = 0; i < this.peers.length; i++) {
      if (this.peers[i].instanceId === this.selectedId) {
        return this.peers[i].name.length > 0
          ? this.peers[i].name : this.peers[i].instanceId.substring(0, 8);
      }
    }
    return '';
  }

  private canSend(): boolean {
    return this.selectedId.length > 0 && this.running && !this.busy;
  }

  private pick(kind: TransferKind): void {
    if (!this.canSend()) {
      return;
    }
    this.busy = true;
    this.openNote = '';
    TransferSync.sendPicked(kind, this.selectedId)
      .then((): void => {
        this.busy = false;
        this.refresh();
      })
      .catch((): void => {
        this.busy = false;
        this.message = '发送失败';
      });
  }

  private sendText(): void {
    if (!this.canSend()) {
      return;
    }
    TransferSync.sendText(this.draft);
    this.draft = '';
    this.textOpen = false;
    this.refresh();
  }

  private titleFor(item: ClipTransfer): string {
    const first: string = item.files.length > 0 ? item.files[0].relativePath : '';
    if (first.length === 0) {
      return item.outgoing ? t('发送给') + ' ' + item.peerName : t('来自') + ' ' + item.peerName;
    }
    return item.files.length > 1
      ? `${first} ${t('等')} ${item.files.length} ${t('项')}`
      : first;
  }

  private percentOf(item: ClipTransfer): number {
    if (item.totalBytes <= 0) {
      return 0;
    }
    return Math.max(0, Math.min(100,
      Math.floor(item.transferredBytes * 100 / item.totalBytes)));
  }

  @Builder
  Section(title: string) {
    Text(title)
      .fontSize(this.fs(11)).fontWeight(FontWeight.Bold).fontColor(this.pal().dimFg)
      .width('100%').padding({ left: 14, right: 14, top: 10, bottom: 4 })
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
      .backgroundColor(primary ? this.pal().accent : sceneKeyBg(this.pal().keyBg))
      .fontColor(primary ? this.pal().highlightFg : this.pal().keyFg)
      .margin({ left: 6 })
      .onClick(action)
  }

  @Builder
  SendRow(label: string, desc: string, action: () => void) {
    Row() {
      Column() {
        Text(label).fontSize(this.fs(14))
          .fontColor(this.canSend() ? this.pal().panelFg : this.pal().dimFg)
        Text(desc).fontSize(this.fs(10)).fontColor(this.pal().dimFg)
          .margin({ top: 2 }).maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1).alignItems(HorizontalAlign.Start)
      Text(this.canSend() ? t('发送') + ' ›' : t('先选设备'))
        .fontSize(this.fs(12)).fontColor(this.canSend() ? this.pal().accent : this.pal().dimFg)
    }
    .width('100%').padding({ left: 14, right: 14, top: 9, bottom: 9 })
    .onClick((): void => {
      if (this.canSend()) {
        action();
      }
    })
  }

  /** 系统限制后台读剪贴板：借 PasteButton 的一次性临时授权读当前内容。 */
  @Builder
  ClipboardRow() {
    Row() {
      Column() {
        Text(t('剪贴板')).fontSize(this.fs(14)).fontColor(this.pal().panelFg)
        Text(t('读取当前系统剪贴板并发送，内容不显示、不记录'))
          .fontSize(this.fs(10)).fontColor(this.pal().dimFg).margin({ top: 2 })
          .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1).alignItems(HorizontalAlign.Start)
      PasteButton({ icon: PasteIconStyle.LINES, text: PasteDescription.PASTE })
        .fontSize(this.fs(12)).height(32)
        .onClick((event: ClickEvent, result: PasteButtonOnClickResult, error?: BusinessError<void>): void => {
          if (result === PasteButtonOnClickResult.SUCCESS) {
            TransferSync.sendClipboard();
          } else {
            ClipSync.markReadDenied();
            this.message = ClipSync.errorText();
          }
          this.refresh();
        })
    }
    .width('100%').padding({ left: 14, right: 14, top: 8, bottom: 8 })
  }

  @Builder
  PeerRow(peer: ClipPeer) {
    Row() {
      Column() {
        Text(peer.name.length > 0 ? peer.name : peer.instanceId.substring(0, 8))
          .fontSize(this.fs(14)).fontColor(this.pal().panelFg)
        Text(peer.trusted
          ? (peer.connected ? t('已连接 · 已配对') : t('未连接 · 已配对'))
          : t('未配对'))
          .fontSize(this.fs(10))
          .fontColor(peer.trusted && peer.connected ? this.pal().accent : this.pal().dimFg)
          .margin({ top: 2 })
      }
      .layoutWeight(1).alignItems(HorizontalAlign.Start)
      if (peer.trusted && peer.connected) {
        Text(this.selectedId === peer.instanceId ? t('已选择') : t('选择'))
          .fontSize(this.fs(12))
          .fontColor(this.selectedId === peer.instanceId ? this.pal().accent : this.pal().dimFg)
      }
    }
    .width('100%').padding({ left: 14, right: 14, top: 8, bottom: 8 })
    .backgroundColor(this.selectedId === peer.instanceId
      ? sceneKeyBg(this.pal().keyBg) : '#00000000')
    .onClick((): void => {
      if (peer.trusted && peer.connected) {
        this.selectedId = peer.instanceId;
        this.openNote = '';
        this.applyRoutes();
      }
    })
  }

  @Builder
  TransferCard(item: ClipTransfer) {
    Column() {
      Row() {
        Text(this.titleFor(item))
          .fontSize(this.fs(13)).fontColor(this.pal().panelFg)
          .layoutWeight(1).maxLines(2).textOverflow({ overflow: TextOverflow.Ellipsis })
        Text(item.outgoing ? t('发送') : t('接收'))
          .fontSize(this.fs(10)).fontColor(this.pal().dimFg).margin({ left: 8 })
      }
      .width('100%')
      if (item.phase === TransferPhase.Offered) {
        Text(`${item.files.length} ${t('项')} · ${formatTransferBytes(item.totalBytes)}`)
          .fontSize(this.fs(10)).fontColor(this.pal().dimFg).margin({ top: 3 })
        Row() {
          this.MiniButton(t('拒绝'), false, (): void => {
            TransferSync.respond(item.id, false);
            this.refresh();
          })
          this.MiniButton(t('接收'), true, (): void => {
            TransferSync.respond(item.id, true);
            this.refresh();
          })
        }.margin({ top: 6 })
      } else if (item.phase === TransferPhase.Active) {
        Text(TransferSync.progressText(item))
          .fontSize(this.fs(11)).fontColor(this.pal().accent).margin({ top: 3 })
        Row() {
          this.MiniButton(t('取消'), false, (): void => {
            TransferSync.cancel(item.id);
            this.refresh();
          })
        }.margin({ top: 4 })
      } else if (item.phase === TransferPhase.Completed) {
        Text(`${t('已完成')} · ${formatTransferBytes(item.totalBytes)}`)
          .fontSize(this.fs(10)).fontColor(this.pal().accent).margin({ top: 3 })
        if (!item.outgoing && item.savedDir.length > 0) {
          Row() {
            this.MiniButton(t('打开文件夹'), false, (): void => {
              this.openNote = TransferSync.openFolder(item.savedDir);
            })
          }.margin({ top: 4 })
        }
      } else if (item.phase === TransferPhase.Failed) {
        Text(`${t('失败')}${item.reason.length > 0 ? '：' + item.reason : ''}`)
          .fontSize(this.fs(10)).fontColor('#C62828').margin({ top: 3 })
          .maxLines(2).textOverflow({ overflow: TextOverflow.Ellipsis })
      } else {
        Text(t('已取消')).fontSize(this.fs(10)).fontColor(this.pal().dimFg).margin({ top: 3 })
      }
    }
    .width('100%').alignItems(HorizontalAlign.Start)
    .padding({ left: 14, right: 14, top: 7, bottom: 7 })
    .border({ width: { bottom: 1 }, color: sceneKeyBg(this.pal().keyBg) })
  }

  build() {
    Column() {
      PanelHeader({
        title: t('跨设备传输'),
        hint: t('附近设备 · 文件 / 文件夹 / 媒体 / 文本 / 剪贴板'),
        onClose: (): void => {
          this.ctrl.closePanel();
        }
      })
      Scroll() {
        Column() {
          Row() {
            Text(this.running ? t('发现与传输运行中') : t('等待键盘输入会话'))
              .fontSize(this.fs(11))
              .fontColor(this.running ? this.pal().accent : this.pal().dimFg)
              .layoutWeight(1)
            if (!this.running) {
              this.MiniButton(t('重试'), true, (): void => {
                if (!ClipSync.ensureStarted()) {
                  this.message = t('传输内核未启动，请重新打开键盘');
                }
                this.refresh();
              })
            }
          }
          .width('100%').padding({ left: 14, right: 14, top: 6, bottom: 2 })

          this.Section(t('附近设备'))
          if (this.peers.length === 0) {
            this.Hint(t('未发现设备。确保两台设备在同一局域网、都打开传输开关，且本机未被基础体验模式限制。'))
          }
          ForEach(this.peers, (peer: ClipPeer): void => {
            this.PeerRow(peer)
          }, (peer: ClipPeer): string => peer.instanceId)
          if (this.selectedId.length > 0) {
            this.Hint(`${t('当前目标')}：${this.selectedName()}`)
          }
          if (TransferSync.hasTrustedPeer() && this.selectedId.length === 0) {
            this.Hint(t('已配对设备当前不在线；等它上线后再发送。'))
          }
          if (!TransferSync.hasTrustedPeer()) {
            this.Hint(t('还没有已配对设备：先到 设置 → 跨设备剪贴板 与对方核对 6 位数字完成配对。'))
          }

          if (this.peers.length > 0) {
            Row() {
              Column() {
                Text(t('只和选中设备收发文本/剪贴板'))
                  .fontSize(this.fs(13)).fontColor(this.pal().panelFg)
                Text(t('关闭时发给所有已配对设备，也接收所有设备'))
                  .fontSize(this.fs(10)).fontColor(this.pal().dimFg).margin({ top: 2 })
              }.layoutWeight(1).alignItems(HorizontalAlign.Start)
              Toggle({ type: ToggleType.Switch, isOn: this.onlySelected })
                .onChange((on: boolean): void => {
                  this.onlySelected = on;
                  this.applyRoutes();
                })
            }
            .width('100%').padding({ left: 14, right: 14, top: 6, bottom: 6 })
          }

          this.Section(t('发送'))
          this.SendRow(t('图片 / 视频'), t('从系统相册选择，最多 128 个'),
            (): void => {
              this.pick(TransferKind.Media);
            })
          this.SendRow(t('文件'), t('从系统文件选择器选择，最多 128 个'),
            (): void => {
              this.pick(TransferKind.Files);
            })
          this.SendRow(t('文件夹'), t('选择整个目录，内核递归发送'),
            (): void => {
              this.pick(TransferKind.Folder);
            })
          Row() {
            Column() {
              Text(t('文本')).fontSize(this.fs(14))
                .fontColor(this.canSend() ? this.pal().panelFg : this.pal().dimFg)
              Text(t('不超过 256 KiB，只发给已配对设备'))
                .fontSize(this.fs(10)).fontColor(this.pal().dimFg).margin({ top: 2 })
            }
            .layoutWeight(1).alignItems(HorizontalAlign.Start)
            Text(this.textOpen ? t('收起') : t('输入') + ' ›')
              .fontSize(this.fs(12))
              .fontColor(this.canSend() ? this.pal().accent : this.pal().dimFg)
          }
          .width('100%').padding({ left: 14, right: 14, top: 9, bottom: 9 })
          .onClick((): void => {
            if (this.canSend()) {
              this.textOpen = !this.textOpen;
            }
          })
          if (this.textOpen) {
            Column() {
              TextArea({
                text: this.draft,
                placeholder: t('输入要发送的文本'),
              })
                .fontSize(this.fs(14)).height(72).width('100%')
                .backgroundColor(sceneKeyBg(this.pal().keyBg))
                .fontColor(this.pal().keyFg)
                .onChange((value: string): void => {
                  this.draft = value;
                })
              Row() {
                this.MiniButton(t('取消'), false, (): void => {
                  this.textOpen = false;
                  this.draft = '';
                })
                this.MiniButton(t('发送文本'), true, (): void => {
                  this.sendText();
                })
              }.width('100%').justifyContent(FlexAlign.End).margin({ top: 6 })
            }
            .width('100%').padding({ left: 14, right: 14, top: 2, bottom: 8 })
          }
          this.ClipboardRow()

          if (this.message.length > 0) {
            Text(t(this.message))
              .fontSize(this.fs(11)).fontColor(this.pal().accent)
              .width('100%').padding({ left: 14, right: 14, top: 4, bottom: 4 })
              .maxLines(3)
          }

          this.Section(t('传输记录'))
          if (this.transfers.length === 0) {
            this.Hint(t('暂无传输。收到的请求会在这里等待确认。'))
          }
          ForEach(this.transfers, (item: ClipTransfer): void => {
            this.TransferCard(item)
          }, (item: ClipTransfer): string => `${item.id}-${item.phase}-${item.transferredBytes}`)

          if (this.openNote.length > 0) {
            Text(this.openNote)
              .fontSize(this.fs(10)).fontColor(this.pal().dimFg)
              .width('100%').padding({ left: 14, right: 14, top: 4, bottom: 4 })
              .maxLines(3).textOverflow({ overflow: TextOverflow.Ellipsis })
          }
          this.Hint(t('接收文件保存在应用沙箱 Download/opi。HarmonyOS 未向普通输入法扩展开放公共 Downloads 写入权限；卸载应用会删除该目录，重要文件请及时转存。'))
          this.Hint(t('跨设备传输只在键盘输入会话内运行；键盘收起会中断未完成的传输。'))
        }
        .width('100%').padding({ bottom: 10 })
      }.layoutWeight(1).width('100%')
    }
    .width('100%').height('100%').backgroundColor(this.pal().panelBg)
  }
}
