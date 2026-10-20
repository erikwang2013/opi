// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 词库随包：把 HAP 的 rawfile 词库拷进扩展沙箱（`ctx.filesDir`），返回**绝对路径**。
//
// 为什么必须拷：引擎（`opi_load` / `opi_load_trad` / `opi_load_stroke` / `opi_load_wubi`）
// 要的是文件系统路径并 mmap 它；rawfile 是资源，不是文件路径，直接传 resourceManager
// 路径必然返回 false。
//
// rawfile 里的四张表由 `scripts/prepare_harmony_dicts.sh` 在构建期从
// `android/app/src/main/assets/` 复制（复制品 gitignore，不入库）。
// 不拷 wubi98：与 86 二选一。trad.opid 是可选增强：缺了照常可中文，繁体回退简库。
//
// 拷贝是**每进程一次**（调用方确保）：目标文件已存在且大小与 rawfile 一致就跳过，
// 所以进程重启后只做四次 stat。任何一步失败都返回空串，由调用方回退内置词库并
// 记录降级 —— **不静默假装完整词库已装上**。

import { resourceManager } from '@kit.LocalizationKit';
import { fileIo } from '@kit.CoreFileKit';

/** 随包词库在 rawfile 里的文件名（与准备脚本逐字一致）。 */
const LUNA_FILE: string = 'luna.opid';
const TRAD_FILE: string = 'trad.opid';
const STROKE_FILE: string = 'stroke.opid';
const WUBI86_FILE: string = 'wubi86.opid';

/** 四张表的沙箱绝对路径；空串 = 该表没随包或拷贝失败。 */
export class DictionaryFiles {
  luna: string = '';
  trad: string = '';
  stroke: string = '';
  wubi: string = '';
}

/** Uint8Array 的字节区间 → writeSync 要的 ArrayBuffer（整块时零拷贝）。 */
function toArrayBuffer(data: Uint8Array): ArrayBuffer {
  if (data.byteOffset === 0 && data.byteLength === data.buffer.byteLength) {
    return data.buffer;
  }
  return data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength);
}

/** 拷一张表；返回沙箱绝对路径，失败返回空串（并打日志）。 */
function syncOne(rm: resourceManager.ResourceManager, dir: string, name: string): string {
  const dest: string = dir + '/' + name;
  try {
    const src: Uint8Array = rm.getRawFileContentSync(name);
    if (src.byteLength === 0) {
      console.error(`[opi] rawfile/${name} 为空，词库不可用`);
      return '';
    }
    try {
      if (fileIo.statSync(dest).size === src.byteLength) {
        return dest; // 上次已完整拷贝（大小即校验；词库随 HAP 更新时大小会变）
      }
    } catch (e) {
      // 目标不存在 ⇒ 往下写
    }
    const file: fileIo.File = fileIo.openSync(dest,
      fileIo.OpenMode.CREATE | fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.TRUNC);
    try {
      const written: number = fileIo.writeSync(file.fd, toArrayBuffer(src));
      if (written !== src.byteLength) {
        console.error(`[opi] rawfile/${name} 写入不完整：${written}/${src.byteLength}`);
        return '';
      }
    } finally {
      fileIo.closeSync(file);
    }
    if (fileIo.statSync(dest).size !== src.byteLength) {
      console.error(`[opi] rawfile/${name} 落盘后大小不符，词库不可用`);
      return '';
    }
    return dest;
  } catch (e) {
    // 资源缺失 / 磁盘满 / 权限：都按「该表不可用」处理，绝不上报成功。
    console.error(`[opi] 拷贝 rawfile/${name} 失败：${e}`);
    return '';
  }
}

/** 把四张随包词库同步到沙箱（幂等；已是最新则只 stat）。 */
export function syncDictionaries(
  rm: resourceManager.ResourceManager, filesDir: string
): DictionaryFiles {
  const out: DictionaryFiles = new DictionaryFiles();
  out.luna = syncOne(rm, filesDir, LUNA_FILE);
  out.trad = syncOne(rm, filesDir, TRAD_FILE);
  out.stroke = syncOne(rm, filesDir, STROKE_FILE);
  out.wubi = syncOne(rm, filesDir, WUBI86_FILE);
  return out;
}
