// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 用户词（自造词 + 频次）落盘：`ctx.filesDir/opi_user_words.json`。
//
// 契约与 fcitx5 / macOS / Android 同一个文件名、同一份 JSON（`{"version":1,"words":[]}`）：
// 引擎只做「JSON 串 ⇄ Learner」、不做 IO；本文件只做「引擎 JSON ⇄ 文件」。
// 唯一真源是引擎内存 —— 落盘直接导出，不从文件反读，故没有第二份缓存会漂。
//
// 失败语义（对齐 engine-data::user_words 的三条，**不静默、也不崩**）：
//   * 文件不存在 = 首次运行（正常，静默）；
//   * 0 字节 / 读不出 / JSON 非法 ⇒ 日志 + **保留原文件**，下一次成功落盘才原子覆盖；
//   * 导出为空（引擎未装载）⇒ 本次不写；**绝不写 0 字节**（下次启动会按损坏处理）。
// 原子写 = 同目录唯一 tmp → fsync → rename（与 Android UserWordStore / engine-data 同形）。

import { fileIo } from '@kit.CoreFileKit';
import { util } from '@kit.ArkTS';
import * as OpiEngine from './OpiEngine';

/** 文件名与 Android `UserWordStore.FILE_NAME` / engine-data 逐字一致。 */
const FILE_NAME: string = 'opi_user_words.json';
/** 防抖窗口（Android `SAVE_DEBOUNCE_MS` 同值）：连续选词合并成一次写盘。 */
const SAVE_DEBOUNCE_MS: number = 1500;

let dir: string = '';
let loaded: boolean = false;
let timer: number = -1;
let saveSeq: number = 0;

function pathOf(): string {
  return dir + '/' + FILE_NAME;
}

function cancelTimer(): void {
  if (timer >= 0) {
    clearTimeout(timer);
    timer = -1;
  }
}

/** 绑定扩展沙箱目录（`KeyboardController.init` 里与 SettingsStore 一起调）。 */
export function init(filesDir: string): void {
  dir = filesDir.endsWith('/') ? filesDir.slice(0, -1) : filesDir;
}

/**
 * 启动导入（词库装载成功后调一次）。缺失/空/损坏都只降级成「没有用户词」。
 * 必须在引擎装载**之后**调：引擎未装载时导入返回 -1，词表不会进内存。
 */
export function load(): void {
  if (loaded) {
    return;
  }
  loaded = true;
  if (dir.length === 0) {
    return;
  }
  const path: string = pathOf();
  try {
    if (!fileIo.accessSync(path)) {
      return; // 首次运行
    }
  } catch (e) {
    console.error(`[opi] 用户词文件不可访问（${path}）：${e}`);
    return;
  }
  try {
    const json: string = fileIo.readTextSync(path);
    if (json.length === 0) {
      console.error(`[opi] 用户词文件是 0 字节（可能是掉电残留），保留原文件：${path}`);
      return;
    }
    const count: number = OpiEngine.importUserWords(json);
    if (count < 0) {
      console.error(`[opi] 用户词文件损坏或版本不符，保留原文件：${path}`);
      return;
    }
    console.info(`[opi] 已载入 ${count} 条用户词：${path}`);
  } catch (e) {
    console.error(`[opi] 用户词读取失败（保留原文件）：${path}：${e}`);
  }
}

/**
 * 学习状态可能变化后调用（选词/删词）：尾随防抖，窗口结束后导出并原子写盘。
 * 引擎未装载时导出是空串 ⇒ 跳过写盘，不会把文件清成 0 字节。
 */
export function scheduleSave(): void {
  if (dir.length === 0) {
    return;
  }
  cancelTimer();
  timer = setTimeout((): void => {
    timer = -1;
    saveNow();
  }, SAVE_DEBOUNCE_MS);
}

/** 立即落盘防抖窗口里的那次保存（会话结束/扩展销毁时调，等不到 1.5s）。 */
export function flush(): void {
  if (timer < 0) {
    return;
  }
  cancelTimer();
  saveNow();
}

/**
 * 清除落盘的词表（设置页「清空用户词」）：先取消待落盘再删，防清完被写回。
 * 内存由调用方的 `OpiEngine.clearUserWords()` 清；本函数只碰文件。
 */
export function clearPersisted(): void {
  cancelTimer();
  if (dir.length === 0) {
    return;
  }
  const path: string = pathOf();
  try {
    if (fileIo.accessSync(path)) {
      fileIo.unlinkSync(path);
    }
  } catch (e) {
    console.error(`[opi] 用户词文件删除失败（${path}）：${e}`);
  }
}

/** 同目录唯一 tmp → fsync → rename；任何一步失败都不破坏既有文件。 */
function saveNow(): void {
  if (dir.length === 0) {
    return;
  }
  const json: string = OpiEngine.exportUserWords();
  if (json.length === 0) {
    console.error('[opi] 用户词导出为空（引擎未装载？），本次不写文件');
    return;
  }
  const path: string = pathOf();
  const tmp: string = `${path}.${Date.now()}.${saveSeq}.tmp`;
  saveSeq++;
  try {
    const file: fileIo.File = fileIo.openSync(tmp,
      fileIo.OpenMode.CREATE | fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC);
    try {
      const bytes: Uint8Array = new util.TextEncoder().encodeInto(json);
      const written: number = fileIo.writeSync(file.fd, bytes.buffer);
      if (written !== bytes.byteLength) {
        throw new Error(`写入不完整：${written}/${bytes.byteLength}`);
      }
      fileIo.fsyncSync(file.fd); // 只 write 的话 rename 可能先于数据落盘（掉电后 0 字节）
    } finally {
      fileIo.closeSync(file);
    }
    fileIo.renameSync(tmp, path);
  } catch (e) {
    try {
      fileIo.unlinkSync(tmp);
    } catch (ignored) {
      // 收尸失败不掩盖原始错误
    }
    console.error(`[opi] 用户词落盘失败（${path}）：${e}`);
  }
}
