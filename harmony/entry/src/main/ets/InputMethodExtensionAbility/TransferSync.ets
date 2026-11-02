// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 跨设备传输运行时的 ArkTS 侧：复用 ClipSync 的唯一 opi-clip 句柄（端口 45888
// 只能绑一个节点），只把 transfer_* 事件投影成面板状态。生命周期沿用输入法扩展：
// 开关与内核都由 ClipSync 管，本模块不认识偏好、不单独起后台任务。
//
// 安全：文件发送只给已配对且已连接的对端（内核再校验一次）；文本/剪贴板走
// publish_text，内核只发给 trusted 会话；入站文件必须用户点「接收」才落盘。
// 不记录文件内容/文本内容，路径只用于展示与打开文件夹。

import { Want } from '@kit.AbilityKit';
import { fileIo, fileUri } from '@kit.CoreFileKit';
import { InputMethodExtensionContext } from '@kit.IMEKit';
import opiime from 'libopiime.so';
import * as ClipSync from './ClipSync';
import { ClipPeer } from './ClipTypes';
import {
  ClipTransfer, MAX_TRANSFER_RECORDS, TransferEvent, TransferPhase, formatTransferBytes,
  parseTransferEvent, transferFilesJson
} from './TransferProtocol';
import { PreparedTransfer, TransferKind, cleanupStage, prepare, pruneStaging } from './TransferFiles';

const MAX_TEXT_CHARS: number = 256 * 1024;
const SAVE_DIR: string = 'Download/opi';

let ctx: InputMethodExtensionContext | null = null;
let transfers: ClipTransfer[] = [];
let message: string = '';
/** transferId → cache staging 目录；终态事件后删除。只在主线程访问。 */
let staged: Map<string, string> = new Map<string, string>();

// ---------------------------------------------------------------- 生命周期

export function init(context: InputMethodExtensionContext): void {
  ctx = context;
  pruneStaging(context);
  ClipSync.setEventSink(onEvent);
}

export function shutdown(): void {
  ClipSync.setEventSink(null);
  clearStages();
  transfers = [];
  message = '';
  ctx = null;
}

export function enabled(): boolean { return ClipSync.transferEnabled(); }

export function setEnabled(on: boolean): void {
  ClipSync.setTransferEnabled(on);
  message = '';
  if (!on) {
    cancelActive();
    clearStages();
    transfers = [];
  }
}

/** 关闭开关但剪贴板仍让内核运行时，主动取消未完成的收发，避免无 UI 状态下继续落盘。 */
function cancelActive(): void {
  const handle: number = ClipSync.handleValue();
  if (handle === 0) {
    return;
  }
  for (let i = 0; i < transfers.length; i++) {
    const phase: TransferPhase = transfers[i].phase;
    if (phase === TransferPhase.Offered || phase === TransferPhase.Active) {
      opiime.clipCancelTransfer(handle, transfers[i].id);
    }
  }
}

// ---------------------------------------------------------------- 只读投影

export function list(): ClipTransfer[] { return transfers; }
export function feedback(): string { return message; }
export function clearFeedback(): void { message = ''; }
export function hasTrustedPeer(): boolean {
  const peers: ClipPeer[] = ClipSync.peerList();
  for (let i = 0; i < peers.length; i++) {
    if (peers[i].trusted) {
      return true;
    }
  }
  return false;
}

/** 入站文件落点：应用沙箱 Download/opi（不申请公共 Downloads 写权限）。 */
export function saveDir(): string {
  return ctx === null ? '' : `${ctx.filesDir}/${SAVE_DIR}`;
}

// ---------------------------------------------------------------- 发送

function peerById(instanceId: string): ClipPeer | null {
  const peers: ClipPeer[] = ClipSync.peerList();
  for (let i = 0; i < peers.length; i++) {
    if (peers[i].instanceId === instanceId) {
      return peers[i];
    }
  }
  return null;
}

function peerName(instanceId: string): string {
  const peer: ClipPeer | null = peerById(instanceId);
  if (peer !== null) {
    return peer.name.length > 0 ? peer.name : instanceId.substring(0, 8);
  }
  return instanceId.substring(0, 8);
}

function clipError(fallback: string): string {
  const text: string = opiime.clipLastError();
  return text.length > 0 ? text : fallback;
}

function errorText(e: Error): string {
  const text: string = typeof e.message === 'string' ? e.message : '';
  return text.length > 0 ? text : '选择或复制文件失败';
}

function sleep(ms: number): Promise<void> {
  return new Promise<void>((resolve: () => void) => {
    setTimeout(resolve, ms);
  });
}

/** picker 期间输入会话可能结束、内核被停；回来后最多等 10 秒重新发现并连上对端。 */
async function waitConnected(peerId: string): Promise<boolean> {
  for (let i = 0; i < 40; i++) {
    const peer: ClipPeer | null = peerById(peerId);
    if (peer !== null && peer.trusted && peer.connected) {
      return true;
    }
    await sleep(250);
  }
  return false;
}

/** 选择并发送文件/文件夹/媒体；peerId 必须来自传输面板当前选中的设备。 */
export async function sendPicked(kind: TransferKind, peerId: string): Promise<void> {
  if (ctx === null) {
    message = '输入法尚未就绪，请重新打开键盘';
    return;
  }
  const peer: ClipPeer | null = peerById(peerId);
  if (peer === null || !peer.trusted || !peer.connected) {
    message = '请先选择一台已配对且已连接的设备';
    return;
  }
  if (ClipSync.handleValue() === 0) {
    message = '传输内核未启动，请重新打开键盘';
    return;
  }
  let prepared: PreparedTransfer;
  try {
    prepared = await prepare(ctx, kind);
  } catch (e) {
    message = errorText(e as Error);
    return;
  }
  // 系统 picker 可能让输入法会话结束：内核停了就用原上下文重启，并等对端重新连上。
  if (!ClipSync.ensureStarted()) {
    cleanupStage(prepared.stageDir);
    message = '传输内核未启动，请重新打开键盘';
    return;
  }
  if (!(await waitConnected(peerId))) {
    cleanupStage(prepared.stageDir);
    message = '设备连接已断开，请重新选择发送';
    return;
  }
  const id: string = opiime.clipSendFiles(
    ClipSync.handleValue(), peer.instanceId, transferFilesJson(prepared.specs));
  if (id.length === 0) {
    cleanupStage(prepared.stageDir);
    message = clipError('发送失败');
    return;
  }
  staged.set(id, prepared.stageDir);
  message = `正在发送 ${prepared.entries.length} 项到 ${peerName(peer.instanceId)}`;
}

/** 文本与剪贴板走 publish_text：内核只发给已配对会话（未配对设备根本不建会话）。 */
export function sendText(text: string): void {
  if (text.length === 0) {
    message = '文本为空';
    return;
  }
  if (text.length > MAX_TEXT_CHARS) {
    message = '文本超过 256 KiB 上限';
    return;
  }
  if (!hasTrustedPeer()) {
    message = '没有已配对设备；请先在设置里完成数字配对';
    return;
  }
  const handle: number = ClipSync.handleValue();
  if (handle === 0) {
    message = '传输内核未启动，请重新打开键盘';
    return;
  }
  const code: number = opiime.clipLocalText(handle, text);
  if (code < 0) {
    message = clipError('文本发送失败');
  } else if (code === 1) {
    message = '与上次发送内容相同，已由防回环规则抑制';
  } else {
    message = `已发送 ${text.length} 字（仅已配对设备，内容不显示、不记录）`;
  }
}

export function sendClipboard(): void {
  if (!hasTrustedPeer()) {
    message = '没有已配对设备；请先在设置里完成数字配对';
    return;
  }
  message = ClipSync.publishClipboardNow();
}

// ---------------------------------------------------------------- 接收 / 取消

export function respond(transferId: string, accept: boolean): void {
  if (ctx === null) {
    message = '输入法尚未就绪';
    return;
  }
  const handle: number = ClipSync.handleValue();
  if (handle === 0) {
    message = '传输内核未启动，请重新打开键盘';
    return;
  }
  let dir: string = '';
  if (accept) {
    dir = saveDir();
    try {
      fileIo.mkdirSync(dir, true);
    } catch (e) {
      try {
        if (!fileIo.statSync(dir).isDirectory()) {
          message = '无法创建接收目录';
          return;
        }
      } catch (ignored) {
        message = '无法创建接收目录';
        return;
      }
    }
  }
  if (!opiime.clipRespondTransfer(handle, transferId, accept, dir)) {
    message = clipError('答复传输失败');
    return;
  }
  message = '';
  updatePhase(transferId, accept ? TransferPhase.Active : TransferPhase.Canceled, dir, '');
}

export function cancel(transferId: string): void {
  const handle: number = ClipSync.handleValue();
  if (handle === 0 || !opiime.clipCancelTransfer(handle, transferId)) {
    message = handle === 0 ? '传输内核未启动' : clipError('取消传输失败');
  }
}

/** 尝试交给系统文件管理器；IME 扩展可能被限制拉起第三方应用，失败就显示路径。 */
export function openFolder(dir: string): string {
  const note: string = `已保存到：${dir}`;
  if (ctx === null || dir.length === 0) {
    return note;
  }
  const want: Want = {
    action: 'ohos.want.action.viewData',
    uri: fileUri.getUriFromPath(dir),
    type: 'application/x-directory',
  };
  try {
    ctx.startAbility(want).catch((e: Error) => {
      console.error(`[opi] 打开接收目录失败：${e.message}`);
    });
  } catch (e) {
    console.error('[opi] 打开接收目录调用失败');
  }
  return note;
}

// ---------------------------------------------------------------- 事件

function clearStages(): void {
  staged.forEach((dir: string) => {
    cleanupStage(dir);
  });
  staged.clear();
}

function updatePhase(transferId: string, phase: TransferPhase, savedDir: string, reason: string): void {
  for (let i = 0; i < transfers.length; i++) {
    if (transfers[i].id === transferId) {
      const old: ClipTransfer = transfers[i];
      transfers[i] = {
        id: old.id, instanceId: old.instanceId, peerName: old.peerName,
        outgoing: old.outgoing, files: old.files, totalBytes: old.totalBytes,
        transferredBytes: old.transferredBytes, fileIndex: old.fileIndex,
        fileCount: old.fileCount, phase: phase,
        savedDir: savedDir.length > 0 ? savedDir : old.savedDir, reason: reason,
      };
      return;
    }
  }
}

function pruneIfNeeded(): void {
  if (transfers.length < MAX_TRANSFER_RECORDS) {
    return;
  }
  for (let i = 0; i < transfers.length; i++) {
    const phase: TransferPhase = transfers[i].phase;
    if (phase === TransferPhase.Completed || phase === TransferPhase.Failed
      || phase === TransferPhase.Canceled) {
      transfers.splice(i, 1);
      return;
    }
  }
  transfers.splice(0, 1);
}

function upsertOffer(ev: TransferEvent): void {
  for (let i = 0; i < transfers.length; i++) {
    if (transfers[i].id === ev.transferId) {
      return;
    }
  }
  pruneIfNeeded();
  transfers.push({
    id: ev.transferId,
    instanceId: ev.instanceId,
    peerName: peerName(ev.instanceId),
    outgoing: ev.outgoing,
    files: ev.files,
    totalBytes: ev.totalBytes,
    transferredBytes: 0,
    fileIndex: 0,
    fileCount: ev.files.length,
    phase: ev.outgoing ? TransferPhase.Active : TransferPhase.Offered,
    savedDir: '',
    reason: '',
  });
  if (!ev.outgoing) {
    message = `收到 ${peerName(ev.instanceId)} 的传输请求`;
  }
}

function onEvent(json: string): void {
  // 开关关闭后内核可能仍为剪贴板运行；此时不得再收传输事件（图标已隐藏，无法答复）。
  if (!ClipSync.transferEnabled()) {
    return;
  }
  if (json.length === 0) {
    interruptActive();
    return;
  }
  const ev: TransferEvent | null = parseTransferEvent(json);
  if (ev === null) {
    return;
  }
  if (ev.type === 'transfer_offer') {
    upsertOffer(ev);
    return;
  }
  if (ev.type === 'transfer_progress') {
    for (let i = 0; i < transfers.length; i++) {
      if (transfers[i].id !== ev.transferId) {
        continue;
      }
      const old: ClipTransfer = transfers[i];
      transfers[i] = {
        id: old.id, instanceId: old.instanceId, peerName: old.peerName,
        outgoing: old.outgoing, files: old.files,
        totalBytes: ev.totalBytes, transferredBytes: ev.transferredBytes,
        fileIndex: ev.fileIndex, fileCount: ev.fileCount,
        phase: TransferPhase.Active, savedDir: old.savedDir, reason: '',
      };
      return;
    }
    return;
  }
  if (ev.type === 'transfer_completed') {
    cleanupByTransfer(ev.transferId);
    updatePhase(ev.transferId, TransferPhase.Completed, ev.savedDir, '');
    message = '';
    return;
  }
  if (ev.type === 'transfer_failed') {
    cleanupByTransfer(ev.transferId);
    updatePhase(ev.transferId, TransferPhase.Failed, '', ev.reason);
    return;
  }
  if (ev.type === 'transfer_canceled') {
    cleanupByTransfer(ev.transferId);
    updatePhase(ev.transferId, TransferPhase.Canceled, '', '');
  }
}

function cleanupByTransfer(transferId: string): void {
  const dir: string | undefined = staged.get(transferId);
  if (dir !== undefined) {
    cleanupStage(dir);
    staged.delete(transferId);
  }
}

/** 键盘收起 = 内核停：未完成的传输不会再有事件，标失败并清缓存（保留已完成记录）。 */
function interruptActive(): void {
  for (let i = 0; i < transfers.length; i++) {
    const t: ClipTransfer = transfers[i];
    if (t.phase === TransferPhase.Offered || t.phase === TransferPhase.Active) {
      transfers[i] = {
        id: t.id, instanceId: t.instanceId, peerName: t.peerName,
        outgoing: t.outgoing, files: t.files, totalBytes: t.totalBytes,
        transferredBytes: t.transferredBytes, fileIndex: t.fileIndex,
        fileCount: t.fileCount, phase: TransferPhase.Failed,
        savedDir: t.savedDir, reason: t.outgoing ? '键盘会话已结束' : '连接已断开',
      };
    }
  }
  clearStages();
}

/** 面板进度文案（不显示文件内容）。 */
export function progressText(t: ClipTransfer): string {
  const total: number = t.totalBytes;
  const percent: number = total > 0
    ? Math.max(0, Math.min(100, Math.floor(t.transferredBytes * 100 / total))) : 0;
  const filePart: string = t.fileCount > 0
    ? ` · 文件 ${Math.max(0, t.fileIndex) + 1}/${t.fileCount}` : '';
  return `${percent}% · ${formatTransferBytes(t.transferredBytes)} / ${formatTransferBytes(total)}${filePart}`;
}
