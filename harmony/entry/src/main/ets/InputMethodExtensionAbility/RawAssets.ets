// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 只读的 rawfile 文本资产（symbol_categories.tsv / rare_index.tsv）。
// 词库走 DictionaryAssets（要先落盘给引擎 mmap）；这两张表是**宿主自己解析**的，
// 直接 `getRawFileContentSync` 读进内存即可，不用拷贝。
//
// ResourceManager 由 KeyboardController.init 绑定（面板页拿不到扩展上下文）。

import { resourceManager } from '@kit.LocalizationKit';
import { util } from '@kit.ArkTS';

let rm: resourceManager.ResourceManager | null = null;

export function bindResourceManager(manager: resourceManager.ResourceManager): void {
  rm = manager;
}

/** 读 rawfile 文本；缺失/解码失败返回空串（面板据此显示「没有数据」，不崩）。 */
export function readRawText(name: string): string {
  if (rm === null) {
    console.error(`[opi] rawfile/${name} 读取失败：ResourceManager 未绑定`);
    return '';
  }
  try {
    const bytes: Uint8Array = rm.getRawFileContentSync(name);
    if (bytes.byteLength === 0) {
      return '';
    }
    return util.TextDecoder.create('utf-8').decodeToString(bytes);
  } catch (e) {
    console.error(`[opi] rawfile/${name} 读取失败：${e}`);
    return '';
  }
}
