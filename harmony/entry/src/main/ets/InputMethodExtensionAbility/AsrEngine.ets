// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 语音引擎句柄与模型装载（进程内单例）：
//   模型存在才装/加载；load_model 走 NAPI 线程池（AsrNative）。
//   同一时刻只有一个 load；预热和点麦克风共享同一个 Promise。
//   shutdown 等当前会话结束再 destroy，避免会话在 native 句柄释放后继续调用。
//
// AsrVoice 负责相位/录音/上屏，本文件只回答「有没有可用的引擎」。

import { common } from '@kit.AbilityKit';
import * as AsrNative from './AsrNative';
import * as AsrModels from './AsrModels';
import { t } from './I18n';

export class EngineReady {
  handle: number = 0;
  error: string = '';
}

const ERROR_BYTES: number = 64 * 1024;

let handle: number = 0;
let loaded: boolean = false;
let loading: Promise<EngineReady> | null = null;
let closed: boolean = false;
const errorBuf: Uint8Array = new Uint8Array(ERROR_BYTES);

export function currentHandle(): number {
  return handle;
}

function readError(h: number): string {
  return AsrNative.lastError(h, errorBuf);
}

async function loadEngine(c: common.Context): Promise<EngineReady> {
  const out: EngineReady = new EngineReady();
  const h: number = AsrNative.create(c.filesDir + '/asr');
  if (h === 0) {
    const why: string = readError(0);
    out.error = why.length > 0 ? `${t('语音引擎初始化失败：')} ${why}` : t('语音引擎初始化失败');
    return out;
  }
  const rc: number = await AsrNative.loadModel(h);
  if (closed) {
    AsrNative.destroy(h);
    return out;
  }
  if (rc !== 0) {
    const why: string = readError(h);
    out.error = why.length > 0 ? `${t('语音模型加载失败：')} ${why}`
      : `${t('语音模型加载失败')} (${rc})`;
    AsrNative.destroy(h);
    return out;
  }
  handle = h;
  loaded = true;
  out.handle = h;
  return out;
}

/** user=true 时把「模型未下载/准备中」也写进 error/progress；预热时保持安静。 */
export async function ensure(c: common.Context, user: boolean,
  progress: (text: string) => void): Promise<EngineReady> {
  const out: EngineReady = new EngineReady();
  if (closed) {
    return out;
  }
  if (!AsrModels.isReady(c)) {
    if (!AsrModels.hasBundled(c)) {
      if (user) {
        out.error = t('语音模型尚未下载，请到「设置 → 语音输入」下载');
      }
      return out;
    }
    if (user) {
      progress(t('正在准备内置语音模型…'));
    }
    if (!(await AsrModels.installBundled(c))) {
      if (user) {
        out.error = t('内置语音模型准备失败，请重试或重新安装完整包');
      }
      return out;
    }
    if (closed) {
      return out;
    }
  }
  if (handle !== 0 && loaded) {
    out.handle = handle;
    return out;
  }
  if (loading !== null) {
    return await loading;
  }
  const task: Promise<EngineReady> = loadEngine(c);
  loading = task;
  try {
    const ready: EngineReady = await task;
    return closed ? out : ready;
  } finally {
    if (loading === task) {
      loading = null;
    }
  }
}

/** 输入会话开始/打开语音开关：模型已存在才在后台装/加载一次。 */
export function prewarm(c: common.Context): void {
  if (closed || loaded || loading !== null) {
    return;
  }
  if (!AsrModels.isReady(c) && !AsrModels.hasBundled(c)) {
    return;
  }
  ensure(c, false, (text: string) => {
    console.info(`[opi] ${text}`);
  }).catch((e: Error) => {
    console.error(`[opi] 语音预热失败：${e.message}`);
  });
}

/** 新扩展生命周期开始（init 时调用一次）。 */
export function reset(): void {
  closed = false;
}

/** 扩展销毁：先标记关闭，等 wait（当前会话）落定后再释放句柄。 */
export function shutdown(wait: Promise<void> | null): void {
  closed = true;
  const h: number = handle;
  handle = 0;
  loaded = false;
  const done = (): void => {
    if (h !== 0) {
      AsrNative.destroy(h);
    }
  };
  if (wait !== null) {
    wait.then(done, done);
  } else {
    done();
  }
}
