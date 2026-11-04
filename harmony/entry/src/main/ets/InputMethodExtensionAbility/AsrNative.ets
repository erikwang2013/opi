// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// libopiime.so 的语音出口（cpp/napi_asr.c）→ 有类型 ArkTS API。
// C ABI 真源 crates/opi-asr/src/ffi.rs；本文件不解析事件，只搬运字节。

import opiime from 'libopiime.so';
import { util } from '@kit.ArkTS';

/** 与 crates/opi-asr/src/error.rs 同名（调用方只对 -5 BUSY 做退避）。 */
export const ASR_OK: number = 0;
export const ASR_ERR_ARGUMENT: number = -1;
export const ASR_ERR_MODEL: number = -3;
export const ASR_ERR_STATE: number = -4;
export const ASR_ERR_BUSY: number = -5;
export const ASR_ERR_EVENT_TOO_SMALL: number = -8;

/** AsrConfig 的 JSON 形态（字段名与 crates/opi-asr/src/config.rs 一致）。 */
interface AsrConfigJson {
  models_dir: string;
  language: string;
  verify_models: boolean;
  num_threads: number;
  max_segment_ms: number;
}

const decoder: util.TextDecoder = util.TextDecoder.create('utf-8');

export function create(modelsDir: string): number {
  const cfg: AsrConfigJson = {
    models_dir: modelsDir,
    language: 'auto',
    verify_models: false, // 下载/内置拷贝时已校验 SHA-256；加载时只查大小，不重算 239MB
    num_threads: 2,
    max_segment_ms: 3000 // 连续听写：每 3 秒收一段
  };
  return opiime.asrCreate(JSON.stringify(cfg));
}

/** NAPI async work：加载期间 UI 线程不冻。返回 false/回调 -1 都按失败处理。 */
export function loadModel(handle: number): Promise<number> {
  return new Promise<number>((resolve: (code: number) => void) => {
    const queued: boolean = opiime.asrLoadModel(handle, (code: number) => {
      resolve(code);
    });
    if (!queued) {
      resolve(-1);
    }
  });
}

export function start(handle: number): number {
  return opiime.asrStart(handle);
}

export function feedPcm(handle: number, pcm: Uint8Array): number {
  return opiime.asrFeedPcm(handle, pcm);
}

export function finish(handle: number): number {
  return opiime.asrFinish(handle);
}

export function cancel(handle: number): number {
  return opiime.asrCancel(handle);
}

/** 事件 JSON；空串 = 队列暂时没有事件。 */
export function pollEvent(handle: number, buf: Uint8Array): string {
  const n: number = opiime.asrPollEvent(handle, buf);
  return n > 0 ? decoder.decodeToString(buf.subarray(0, n)) : '';
}

/** handle 传 0 可读 create 失败原因。 */
export function lastError(handle: number, buf: Uint8Array): string {
  const n: number = opiime.asrLastError(handle, buf);
  return n > 0 ? decoder.decodeToString(buf.subarray(0, n)) : '';
}

export function destroy(handle: number): void {
  opiime.asrDestroy(handle);
}
