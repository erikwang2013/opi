// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 跨设备剪贴板运行时不 (opi-clip 的 ArkTS 侧)。只在输入会话可见时运行，默认关闭。
// 句柄按创建线程校验：create/poll/stop/destroy 全部在 ArkTS 主线程，轮询用
// setInterval + pollEvent(handle, 0)，绝不在 UI 线程传 timeout > 0。
// 安全：不记录内容、不自动粘贴、不自动打开链接；身份/信任文件在 ctx.filesDir/clip。

import { common } from '@kit.AbilityKit';
import { deviceInfo } from '@kit.BasicServicesKit';
import opiime from 'libopiime.so';
import { ClipboardAdapter } from './ClipboardAdapter';
import * as SettingsStore from './SettingsStore';

const DISCOVERY_PORT: number = 45888;
const POLL_MS: number = 200;
/** 空闲多少轮刷一次 peers/status（发现/离线事件偶尔会丢）。 */
const REFRESH_EVERY_POLLS: number = 8;
/** 内核按 UTF-8 字节限 256 KiB；这里按 UTF-16 字符数近似挡。 */
const MAX_TEXT_CHARS: number = 256 * 1024;
const MAX_DEVICE_NAME: number = 48;

export interface ClipPeer {
  instanceId: string;
  name: string;
  address: string;
  fingerprint: string;
  trusted: boolean;
  connected: boolean;
}

export interface ClipStatus {
  running: boolean;
  instanceId: string;
  fingerprint: string;
  deviceName: string;
  peers: number;
}

export interface ClipPairingPrompt {
  instanceId: string;
  sas: string;
  /** true = 这次配对由对端发起。 */
  incoming: boolean;
}

// ---- JSON 形状：真源 crates/opi-ffi/src/cabi_clip/encode.rs，字段名 snake_case ----
interface RawPeer {
  instance_id?: string;
  name?: string;
  address?: string;
  fingerprint?: string;
  trusted?: boolean;
  connected?: boolean;
}

interface RawStatus {
  running?: boolean;
  instance_id?: string | null;
  fingerprint?: string | null;
  device_name?: string;
  peers?: number;
}

interface RawEvent {
  type?: string;
  instance_id?: string;
  sas?: string;
  incoming?: boolean;
  trusted?: boolean;
  reason?: string | null;
  text?: string;
  message?: string;
}

interface ClipConfig {
  data_dir: string;
  tcp_port: number;
  discovery_port: number;
  device_name: string;
}

let started: boolean = false;
let handle: number = 0;
let pollTimer: number = -1;
let stopTimer: number = -1;
let idlePolls: number = 0;
let adapter: ClipboardAdapter | null = null;
let activeCtx: common.Context | null = null;
let prefLoaded: boolean = false;
let prefEnabled: boolean = false;

let peers: ClipPeer[] = [];
let status: ClipStatus | null = null;
let pairingPrompt: ClipPairingPrompt | null = null;
let lastSource: string = '';
let lastPairing: string = '';
let lastError: string = '';
let readLimited: boolean = false;

// ---------------------------------------------------------------- 只读投影

export function isRunning(): boolean { return started && handle !== 0; }
export function peerList(): ClipPeer[] { return peers; }
export function currentStatus(): ClipStatus | null { return status; }
export function prompt(): ClipPairingPrompt | null { return pairingPrompt; }
/** 上次远端内容的来源设备名（只记来源，不记内容）。 */
export function sourceDevice(): string { return lastSource; }
export function pairingResult(): string { return lastPairing; }
export function errorText(): string { return lastError; }
/** 后台读剪贴板被系统限制（需要用户点 PasteButton 手动同步）。 */
export function isReadLimited(): boolean { return readLimited; }

// ---------------------------------------------------------------- 生命周期

/** 开关持久化在 SettingsStore；缺省关闭。 */
export function sharingEnabled(): boolean {
  if (!prefLoaded) {
    prefEnabled = SettingsStore.load().clipEnabled;
    prefLoaded = true;
  }
  return prefEnabled;
}

/** 设置页开关：立即落盘；会话进行中则立即起停。 */
export function setEnabled(on: boolean): void {
  prefEnabled = on;
  prefLoaded = true;
  SettingsStore.saveBool(SettingsStore.BOOL_CLIP, on);
  if (!on) {
    stopNow();
  } else if (activeCtx !== null) {
    start(activeCtx);
  }
}

/** InputMethodService inputStart：记住上下文，开关打开才启动。 */
export function onInputStart(ctx: common.Context | null): void {
  activeCtx = ctx;
  if (ctx !== null && sharingEnabled()) {
    start(ctx);
  }
}

/** inputStop：延迟停止，不阻塞键盘收起。 */
export function onInputStop(): void { stop(); }

/** 输入会话开始且开关打开时调用；重复调用幂等。 */
export function start(ctx: common.Context): void {
  if (stopTimer !== -1) { // 刚停到一半：取消延迟停止，保持原句柄
    clearTimeout(stopTimer);
    stopTimer = -1;
  }
  if (started || handle !== 0) {
    return;
  }
  const config: ClipConfig = {
    data_dir: `${ctx.filesDir}/clip`,
    tcp_port: 0,
    discovery_port: DISCOVERY_PORT,
    device_name: deviceName(),
  };
  const created: number = opiime.clipCreate(JSON.stringify(config));
  if (created === 0) {
    lastError = lastErrorText();
    return;
  }
  handle = created;
  started = true;
  if (!opiime.clipStart(handle)) {
    lastError = lastErrorText();
    stopNow();
    return;
  }
  lastError = '';
  readLimited = false;
  idlePolls = 0;
  const createdAdapter: ClipboardAdapter = new ClipboardAdapter(
    (text: string): void => {
      publishText(text);
    },
    (): void => {
      readLimited = true;
    },
  );
  adapter = createdAdapter;
  createdAdapter.start();
  pollTimer = setInterval((): void => {
    pollOnce();
  }, POLL_MS);
  refreshStatus();
  refreshPeers();
}

/** 输入会话结束：延迟到当前回调之后停止（Rust stop 会 join 后台线程，最多约 3 秒）。 */
export function stop(): void {
  if ((!started && handle === 0) || stopTimer !== -1) {
    return;
  }
  stopTimer = setTimeout((): void => {
    stopTimer = -1;
    stopNow();
  }, 0);
}

/** 扩展销毁：同步停止并释放句柄。 */
export function shutdown(): void {
  activeCtx = null;
  stopNow();
}

// ---------------------------------------------------------------- 配对 / 信任

export function pairStart(instanceId: string): void {
  lastError = '';
  if (handle === 0) {
    return;
  }
  if (!opiime.clipPairStart(handle, instanceId)) {
    lastError = lastErrorText();
  }
  refreshPeers();
}

/** 用户在 SAS 上点「允许」：accept=true；任何自动接受路径都不存在。 */
export function pairConfirm(instanceId: string, accept: boolean): void {
  pairingPrompt = null;
  if (handle === 0) {
    return;
  }
  if (!opiime.clipPairConfirm(handle, instanceId, accept)) {
    lastError = lastErrorText();
  }
  refreshPeers();
}

export function pairCancel(instanceId: string): void {
  pairingPrompt = null;
  if (handle === 0) {
    return;
  }
  if (!opiime.clipPairCancel(handle, instanceId)) {
    lastError = lastErrorText();
  }
  refreshPeers();
}

export function forgetPeer(instanceId: string): void {
  if (handle === 0) {
    return;
  }
  if (!opiime.clipForgetPeer(handle, instanceId)) {
    lastError = lastErrorText();
  }
  refreshPeers();
}

// ---------------------------------------------------------------- 剪贴板桥

/** PasteButton 点击成功后调用：读当前剪贴板并发布。临时授权只对这次点击有效。 */
export function publishCurrentClipboard(): void {
  lastError = '';
  if (adapter === null || handle === 0) {
    lastError = '跨设备剪贴板未运行';
    return;
  }
  const text: string = adapter.readText();
  if (adapter.readBlocked()) {
    readLimited = true;
    lastError = '系统未授予剪贴板读取权限';
    return;
  }
  if (text.length === 0) {
    lastError = '系统剪贴板里没有纯文本';
    return;
  }
  if (text.length > MAX_TEXT_CHARS) {
    lastError = '剪贴板内容超过 256 KiB，未同步';
    return;
  }
  readLimited = false;
  publishText(text);
}

/** PasteButton 返回授权失败：不重试、不弹窗，只在设置页给提示。 */
export function markReadDenied(): void {
  readLimited = true;
  lastError = '未获得剪贴板读取授权';
}

function publishText(text: string): void {
  if (handle === 0 || text.length === 0 || text.length > MAX_TEXT_CHARS) {
    return;
  }
  const code: number = opiime.clipLocalText(handle, text);
  // 0=已发布 1=防回环抑制（正常，不上屏也不报错）-1=失败。
  if (code < 0) {
    lastError = lastErrorText();
  }
}

// ---------------------------------------------------------------- 内核事件

function pollOnce(): void {
  if (handle === 0) {
    return;
  }
  const json: string = opiime.clipPollEvent(handle, 0);
  if (json.length === 0) {
    idlePolls++;
    if (idlePolls >= REFRESH_EVERY_POLLS) {
      idlePolls = 0;
      refreshPeers();
      refreshStatus();
    }
    return;
  }
  idlePolls = 0;
  handleEvent(json);
  refreshPeers();
}

function handleEvent(json: string): void {
  let event: RawEvent;
  try {
    event = JSON.parse(json) as RawEvent;
  } catch (e) {
    return;
  }
  const type: string = typeof event.type === 'string' ? event.type : '';
  if (type === 'peer_discovered' || type === 'peer_offline') {
    return;
  }
  if (type === 'pairing_prompt') {
    if (typeof event.instance_id === 'string' && typeof event.sas === 'string') {
      const parsed: ClipPairingPrompt = {
        instanceId: event.instance_id,
        sas: event.sas,
        incoming: event.incoming === true,
      };
      pairingPrompt = parsed;
    }
    return;
  }
  if (type === 'pairing_finished') {
    const id: string = typeof event.instance_id === 'string' ? event.instance_id : '';
    if (pairingPrompt !== null && pairingPrompt.instanceId === id) {
      pairingPrompt = null;
    }
    const reason: string = typeof event.reason === 'string' ? event.reason : '';
    const name: string = peerName(id);
    lastPairing = event.trusted === true
      ? `已与 ${name} 配对`
      : `与 ${name} 的配对未完成${reason.length > 0 ? `：${reason}` : ''}`;
    return;
  }
  if (type === 'clipboard_received') {
    const id: string = typeof event.instance_id === 'string' ? event.instance_id : '';
    const text: string = typeof event.text === 'string' ? event.text : '';
    lastSource = peerName(id);
    if (text.length > 0 && text.length <= MAX_TEXT_CHARS && adapter !== null) {
      if (!adapter.applyRemote(text)) {
        lastError = '写入系统剪贴板失败';
      }
    }
    return;
  }
  if (type === 'send_failed') {
    lastError = '发送到一台已配对设备失败';
    return;
  }
  if (type === 'error') {
    lastError = typeof event.message === 'string' && event.message.length > 0
      ? event.message : '剪贴板内核错误';
  }
}

// ---------------------------------------------------------------- 状态刷新

function refreshPeers(): void {
  if (handle === 0) {
    peers = [];
    return;
  }
  const json: string = opiime.clipPeers(handle);
  if (json.length === 0) {
    return;
  }
  let raw: RawPeer[];
  try {
    raw = JSON.parse(json) as RawPeer[];
  } catch (e) {
    return;
  }
  const out: ClipPeer[] = [];
  for (let i = 0; i < raw.length; i++) {
    const peer: ClipPeer | null = peerOf(raw[i]);
    if (peer !== null) {
      out.push(peer);
    }
  }
  peers = out;
}

function refreshStatus(): void {
  if (handle === 0) {
    status = null;
    return;
  }
  const json: string = opiime.clipStatus(handle);
  if (json.length === 0) {
    return;
  }
  let raw: RawStatus;
  try {
    raw = JSON.parse(json) as RawStatus;
  } catch (e) {
    return;
  }
  const parsed: ClipStatus = {
    running: raw.running === true,
    instanceId: typeof raw.instance_id === 'string' ? raw.instance_id : '',
    fingerprint: typeof raw.fingerprint === 'string' ? raw.fingerprint : '',
    deviceName: typeof raw.device_name === 'string' ? raw.device_name : '',
    peers: typeof raw.peers === 'number' ? raw.peers : 0,
  };
  status = parsed;
}

function peerOf(raw: RawPeer): ClipPeer | null {
  if (typeof raw.instance_id !== 'string' || raw.instance_id.length === 0) {
    return null;
  }
  const peer: ClipPeer = {
    instanceId: raw.instance_id,
    name: typeof raw.name === 'string' ? raw.name : '',
    address: typeof raw.address === 'string' ? raw.address : '',
    fingerprint: typeof raw.fingerprint === 'string' ? raw.fingerprint : '',
    trusted: raw.trusted === true,
    connected: raw.connected === true,
  };
  return peer;
}

function peerName(instanceId: string): string {
  for (let i = 0; i < peers.length; i++) {
    if (peers[i].instanceId === instanceId) {
      return peers[i].name.length > 0 ? peers[i].name : instanceId.substring(0, 8);
    }
  }
  return instanceId.substring(0, 8);
}

// ---------------------------------------------------------------- 停止 / 工具

function stopNow(): void {
  if (stopTimer !== -1) {
    clearTimeout(stopTimer);
    stopTimer = -1;
  }
  if (pollTimer !== -1) {
    clearInterval(pollTimer);
    pollTimer = -1;
  }
  if (adapter !== null) {
    adapter.stop();
    adapter = null;
  }
  if (handle !== 0) {
    opiime.clipStop(handle);
    opiime.clipDestroy(handle);
    handle = 0;
  }
  started = false;
  idlePolls = 0;
  peers = [];
  status = null;
  pairingPrompt = null;
  lastSource = '';
  readLimited = false;
}

function lastErrorText(): string {
  const message: string = opiime.clipLastError();
  return message.length > 0 ? message : '跨设备剪贴板调用失败';
}

function deviceName(): string {
  let name: string = '';
  try {
    name = deviceInfo.marketName;
  } catch (e) {
    name = '';
  }
  if (name.length === 0) {
    name = 'HarmonyOS';
  }
  return name.length > MAX_DEVICE_NAME ? name.substring(0, MAX_DEVICE_NAME) : name;
}
