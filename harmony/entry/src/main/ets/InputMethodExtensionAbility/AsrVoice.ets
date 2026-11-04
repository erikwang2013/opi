// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 本地语音输入运行时：权限 → 模型 → AudioCapturer(16k mono PCM16) → opi-asr → 上屏。
//
// 单例，所有 native 调用都在 ArkTS 主线程；load_model 走 NAPI 线程池（AsrNative）。
// 键盘会话开始时预热一次（模型已存在才装/加载），以后按键/每帧都不再初始化。
// 录音失败只把 phase 置为 Error 并给一句可读原因，不抛到 UI。
//
// AppStorage: opiVoice（0 隐藏 / 1 空闲 / 2 准备 / 3 录音 / 4 识别 / 5 错误）、
// opiVoiceMsg（状态行文字）。TopBar 直接订阅。

import { abilityAccessCtrl, bundleManager, common, Permissions } from '@kit.AbilityKit';
import { audio } from '@kit.AudioKit';
import { BusinessError } from '@kit.BasicServicesKit';
import * as AsrEngine from './AsrEngine';
import * as AsrNative from './AsrNative';
import * as AsrModels from './AsrModels';
import { t } from './I18n';

export enum VoicePhase {
  Disabled = 0,
  Ready = 1,
  Preparing = 2,
  Listening = 3,
  Finishing = 4,
  Failed = 5,
}

const CHUNK_BYTES: number = 3200; // 100ms PCM16 mono
const FEED_RETRY_MS: number = 45000;
const FINISH_TIMEOUT_MS: number = 120000;
const POLL_MS: number = 30;
const FALLBACK_EVENT_BYTES: number = 64 * 1024;

const VOICE_KEY: string = 'opiVoice';
const MESSAGE_KEY: string = 'opiVoiceMsg';

enum Request {
  Idle = 0,
  Run = 1,
  Stop = 2,
  Cancel = 3,
}

interface AsrEventJson {
  type: string;
  text?: string;
  message?: string;
  code?: string;
}

let ctx: common.Context | null = null;
let commit: ((text: string) => void) | null = null;
let enabled: boolean = false;
let shuttingDown: boolean = false;

let phase: VoicePhase = VoicePhase.Disabled;
let message: string = '';
let eventBuf: Uint8Array = new Uint8Array(FALLBACK_EVENT_BYTES);
let errorBuf: Uint8Array = new Uint8Array(FALLBACK_EVENT_BYTES);

let request: Request = Request.Idle;
/** 取消/换会话时自增；迟到的 read/poll/回调全部按世代丢弃。 */
let generation: number = 0;
let capturer: audio.AudioCapturer | null = null;
let runningSession: Promise<void> | null = null;

let finalText: string = '';
let gotFinal: boolean = false;
let pendingError: string = '';
let segmentsCommitted: number = 0;

function publish(): void {
  AppStorage.setOrCreate<number>(VOICE_KEY, phase as number);
  AppStorage.setOrCreate<string>(MESSAGE_KEY, message);
}

function setPhase(p: VoicePhase, text: string): void {
  phase = p;
  message = text;
  publish();
}

function sleep(ms: number): Promise<void> {
  return new Promise<void>((resolve: () => void) => {
    setTimeout(resolve, ms);
  });
}

function lastError(h: number, buf: Uint8Array): string {
  return AsrNative.lastError(h, buf);
}

function nativeMessage(h: number, rc: number): string {
  const why: string = lastError(h, errorBuf);
  return why.length > 0 ? why : `${t('语音识别失败')} (${rc})`;
}

function fail(h: number, rc: number): void {
  setPhase(VoicePhase.Failed, nativeMessage(h, rc));
}

function failText(text: string): void {
  setPhase(VoicePhase.Failed, text);
}

// ---------------------------------------------------------------- 权限

async function micGranted(c: common.Context): Promise<boolean> {
  try {
    const at = abilityAccessCtrl.createAtManager();
    const info = bundleManager.getBundleInfoForSelfSync(
      bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT);
    if (at.checkAccessTokenSync(info.appInfo.accessTokenId, 'ohos.permission.MICROPHONE') ===
      abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
      return true;
    }
    const perms: Permissions[] = ['ohos.permission.MICROPHONE'];
    const result = await at.requestPermissionsFromUser(c, perms);
    return result.authResults.length > 0 && result.authResults[0] === 0;
  } catch (e) {
    // IME 扩展不保证能拉起运行时权限弹窗：调用失败按未授权处理，让用户去系统设置。
    console.error(`[opi] 麦克风权限请求失败：${e}`);
    return false;
  }
}

// ---------------------------------------------------------------- 模型

/** 模型存在才装/加载；返回 native 句柄，0 = 失败（user=true 时显示原因）。 */
async function ensureModel(c: common.Context, user: boolean): Promise<number> {
  if (shuttingDown) {
    return 0;
  }
  const ready: AsrEngine.EngineReady = await AsrEngine.ensure(c, user, (text: string) => {
    if (user) {
      setPhase(VoicePhase.Preparing, text);
    }
  });
  if (ready.handle === 0) {
    if (user && ready.error.length > 0) {
      failText(ready.error);
    }
    return 0;
  }
  return ready.handle;
}

/** 输入会话开始/打开语音开关时调用；已加载就什么都不做。失败留给用户点麦克风时显示。 */
export function prewarm(): void {
  const c = ctx;
  if (!enabled || c === null || shuttingDown) {
    return;
  }
  AsrEngine.prewarm(c);
}

// ---------------------------------------------------------------- 录音

async function closeCapture(): Promise<void> {
  const c = capturer;
  capturer = null;
  if (c === null) {
    return;
  }
  try {
    await c.stop();
  } catch (e) {
    // 已停止/未启动
  }
  try {
    await c.release();
  } catch (e) {
    // 已释放
  }
}

async function openCapture(): Promise<audio.AudioCapturer> {
  const options: audio.AudioCapturerOptions = {
    streamInfo: {
      samplingRate: audio.AudioSamplingRate.SAMPLE_RATE_16000,
      channels: audio.AudioChannel.CHANNEL_1,
      sampleFormat: audio.AudioSampleFormat.SAMPLE_FORMAT_S16LE,
      encodingType: audio.AudioEncodingType.ENCODING_TYPE_RAW
    },
    capturerInfo: {
      source: audio.SourceType.SOURCE_TYPE_MIC,
      capturerFlags: 0
    }
  };
  return await audio.createAudioCapturer(options);
}

function drain(h: number, gen: number): void {
  while (gen === generation) {
    const text: string = AsrNative.pollEvent(h, eventBuf);
    if (text.length === 0) {
      return;
    }
    let event: AsrEventJson;
    try {
      event = JSON.parse(text) as AsrEventJson;
    } catch (e) {
      pendingError = t('语音事件解析失败');
      return;
    }
    if (event.type === 'segment') {
      const segment: string = event.text !== undefined ? event.text.trim() : '';
      if (segment.length > 0) {
        segmentsCommitted++;
        if (commit !== null) {
          commit(segment);
        }
      }
    } else if (event.type === 'final') {
      finalText = event.text !== undefined ? event.text : '';
      gotFinal = true;
    } else if (event.type === 'error') {
      const why: string = event.message !== undefined && event.message.length > 0
        ? event.message : (event.code !== undefined ? event.code : '');
      pendingError = why.length > 0 ? why : t('语音识别出错');
    }
  }
}

async function feed(h: number, gen: number, data: Uint8Array): Promise<void> {
  const deadline: number = Date.now() + FEED_RETRY_MS;
  while (gen === generation && request === Request.Run) {
    const rc: number = AsrNative.feedPcm(h, data);
    if (rc === AsrNative.ASR_OK) {
      drain(h, gen);
      if (pendingError.length > 0) {
        throw new Error(pendingError);
      }
      return;
    }
    if (rc !== AsrNative.ASR_ERR_BUSY) {
      throw new Error(nativeMessage(h, rc));
    }
    if (Date.now() > deadline) {
      throw new Error(t('音频处理跟不上，请缩短录音'));
    }
    await sleep(20);
  }
}

async function readLoop(h: number, gen: number): Promise<void> {
  while (gen === generation && request === Request.Run && capturer !== null) {
    const cap: audio.AudioCapturer = capturer;
    let buffer: ArrayBuffer;
    try {
      buffer = await cap.read(CHUNK_BYTES, true);
    } catch (e) {
      // stop()/cancel() 会先停采集再释放：这里的拒绝是正常退出，不是识别失败。
      if (gen === generation && request === Request.Run) {
        throw new Error(`${e}`);
      }
      return;
    }
    if (gen !== generation || request !== Request.Run) {
      return;
    }
    const data: Uint8Array = new Uint8Array(buffer);
    if (data.byteLength > 0) {
      await feed(h, gen, data);
    }
  }
}

async function transcribe(h: number, gen: number): Promise<void> {
  finalText = '';
  gotFinal = false;
  pendingError = '';
  const rc: number = AsrNative.finish(h);
  if (rc !== AsrNative.ASR_OK) {
    throw new Error(nativeMessage(h, rc));
  }
  const deadline: number = Date.now() + FINISH_TIMEOUT_MS;
  while (gen === generation) {
    drain(h, gen);
    if (pendingError.length > 0) {
      throw new Error(pendingError);
    }
    if (gotFinal) {
      const text: string = finalText.trim();
      if (text.length === 0) {
        failText(t('没有识别到语音，请重试'));
        return;
      }
      if (commit !== null && segmentsCommitted === 0) {
        commit(text);
      }
      setPhase(VoicePhase.Ready, '');
      return;
    }
    if (Date.now() > deadline) {
      throw new Error(t('识别超时，请重试'));
    }
    await sleep(POLL_MS);
  }
}

async function runSession(c: common.Context, gen: number): Promise<void> {
  let h: number = 0;
  try {
    if (!(await micGranted(c))) {
      if (gen === generation) {
        failText(t('需要麦克风权限：请在系统设置中允许小欧输入法使用麦克风'));
      }
      return;
    }
    if (gen !== generation || request !== Request.Run) {
      return;
    }
    h = await ensureModel(c, true);
    if (gen !== generation || request !== Request.Run || h === 0) {
      return;
    }
    const rc: number = AsrNative.start(h);
    if (rc !== AsrNative.ASR_OK) {
      fail(h, rc);
      return;
    }
    try {
      const cap: audio.AudioCapturer = await openCapture();
      capturer = cap;
      await cap.start();
    } catch (e) {
      await closeCapture();
      AsrNative.cancel(h);
      failText(`${t('无法打开麦克风：')} ${e}`);
      return;
    }
    if (gen !== generation || request !== Request.Run) {
      await closeCapture();
      AsrNative.cancel(h);
      return;
    }
    setPhase(VoicePhase.Listening, t('正在听… 每 3 秒自动上屏一段，再点麦克风结束'));
    await readLoop(h, gen);
    if (gen !== generation || (request as Request) !== Request.Stop) {
      return;
    }
    request = Request.Idle;
    await closeCapture();
    setPhase(VoicePhase.Finishing, t('正在识别…'));
    await transcribe(h, gen);
  } catch (e) {
    if (gen === generation) {
      failText(`${e}`);
    }
  } finally {
    if (h !== 0) {
      AsrNative.cancel(h);
    }
    await closeCapture();
    if (gen === generation && phase !== VoicePhase.Failed) {
      setPhase(enabled ? VoicePhase.Ready : VoicePhase.Disabled, '');
    }
  }
}

/** 点麦克风：空闲/出错 → 开始；准备中忽略；录音中 → 停止并出字。 */
export function start(): void {
  const c = ctx;
  if (!enabled || c === null || shuttingDown) {
    return;
  }
  if (phase === VoicePhase.Preparing || phase === VoicePhase.Listening ||
    phase === VoicePhase.Finishing) {
    return;
  }
  const gen: number = ++generation;
  segmentsCommitted = 0;
  request = Request.Run;
  setPhase(VoicePhase.Preparing, t('正在准备本地语音模型…'));
  runningSession = runSession(c, gen);
  runningSession.catch((e: Error) => {
    console.error(`[opi] 语音会话异常：${e.message}`);
  });
}

/** 停止收音，等待 final 上屏。只对录音中的会话生效。 */
export function stop(): void {
  if (phase !== VoicePhase.Listening) {
    return;
  }
  request = Request.Stop;
  setPhase(VoicePhase.Finishing, t('正在识别…'));
  closeCapture().catch((e: Error) => {
    console.error(`[opi] 关闭麦克风失败：${e.message}`);
  });
}

/** 取消：换输入框/收起键盘/用户点取消。迟到结果按世代丢弃。 */
export function cancel(): void {
  generation++;
  request = Request.Cancel;
  const h: number = AsrEngine.currentHandle();
  if (h !== 0) {
    AsrNative.cancel(h);
  }
  closeCapture().catch((e: Error) => {
    console.error(`[opi] 关闭麦克风失败：${e.message}`);
  });
  if (phase !== VoicePhase.Disabled) {
    setPhase(enabled ? VoicePhase.Ready : VoicePhase.Disabled, '');
  }
}

export function dismissError(): void {
  if (phase === VoicePhase.Failed) {
    setPhase(enabled ? VoicePhase.Ready : VoicePhase.Disabled, '');
  }
}

export function isEnabled(): boolean {
  return enabled;
}

export function setEnabled(on: boolean): void {
  enabled = on;
  if (!on) {
    cancel();
    setPhase(VoicePhase.Disabled, '');
    return;
  }
  setPhase(VoicePhase.Ready, '');
  prewarm();
}

/** KeyboardController.init 调用一次；commit 是现成的上屏通道。 */
export function init(c: common.Context, onCommit: (text: string) => void, on: boolean): void {
  ctx = c;
  commit = onCommit;
  shuttingDown = false;
  AsrEngine.reset();
  setEnabled(on);
}

/** 输入会话开始：只在语音已启用且模型存在时后台预热。 */
export function onInputStart(): void {
  prewarm();
}

/** 输入会话结束：丢弃进行中的录音，模型句柄保留给下一次会话。 */
export function onInputStop(): void {
  if (phase === VoicePhase.Listening || phase === VoicePhase.Preparing ||
    phase === VoicePhase.Finishing) {
    cancel();
  }
}

// ---------------------------------------------------------------- 设置页只读/下载入口

export function modelStatus(): AsrModels.ModelStatus | null {
  const c = ctx;
  return c === null ? null : AsrModels.status(c);
}

export function modelBundled(): boolean {
  const c = ctx;
  return c !== null && AsrModels.hasBundled(c);
}

export function downloadModel(): void {
  const c = ctx;
  if (c !== null && enabled) {
    AsrModels.download(c).catch((e: Error) => {
      console.error(`[opi] 模型下载异常：${e.message}`);
    });
  }
}

/** full 档内置模型安装失败后的重试（不改变开关）。 */
export function retryPrepare(): void {
  if (!enabled || ctx === null) {
    return;
  }
  setPhase(VoicePhase.Ready, '');
  prewarm();
}

/** 扩展销毁：取消会话，等会话/加载落定后由 AsrEngine 释放句柄。 */
export function shutdown(): void {
  shuttingDown = true;
  cancel();
  enabled = false;
  ctx = null;
  commit = null;
  setPhase(VoicePhase.Disabled, '');
  AsrEngine.shutdown(runningSession);
}
