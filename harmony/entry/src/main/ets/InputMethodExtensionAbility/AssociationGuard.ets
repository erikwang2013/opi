// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 智能联想的鸿蒙接线（2026-10-27，Spec §5）：收起、选区守卫、自我插入时间窗。
// 联想态本身不在这里存 —— `publishState()` 把它算进 KEY_ASSOCIATION，本模块只读
// 那格缓存决定「要不要收」，所以每键不必重取候选。
// ⚠️ 本机未经构建/真机验证（鸿蒙构建与真机验收见 harmony.yml）。

import { inputMethodEngine } from '@kit.IMEKit';
import { KEY_ASSOCIATION } from './KeyboardAppStorage';
import * as OpiEngine from './OpiEngine';
import { publishState } from './StatePublish';

/** 自我插入窗口（Spec §5.2）：自己的提交也会触发 selectionChange，窗口内不收联想。 */
const SELF_INSERT_WINDOW_MS: number = 200;

let lastSelfInsertAt: number = 0;

/** 上屏前调用：接下来的 selectionChange 是我们自己引起的，不能把刚建的联想收掉。 */
export function noteSelfInsert(): void {
  lastSelfInsertAt = Date.now();
}

function inSelfInsertWindow(): boolean {
  return Date.now() - lastSelfInsertAt <= SELF_INSERT_WINDOW_MS;
}

/** 收起联想（引擎 clear 同时清缓冲与联想）。 */
export function dismiss(): void {
  OpiEngine.clear();
  publishState();
}

/** 各入口的第一件事；`tapTiledCandidate` 例外 —— 候选点击要交给引擎链式建下一条。 */
export function dismissIfNeeded(): void {
  if (AppStorage.get<number>(KEY_ASSOCIATION) === 1) {
    dismiss();
  }
}

const onSelectionChange = (oldBegin: number, oldEnd: number, newBegin: number,
  newEnd: number): void => {
  if (!inSelfInsertWindow()) {
    dismissIfNeeded();
  }
};

/** 新会话订阅；`off()` 需要同一函数引用（与控制器里 inputStart/inputStop 同一纪律）。 */
export function watchSelection(): void {
  inputMethodEngine.getKeyboardDelegate().on('selectionChange', onSelectionChange);
}

export function unwatchSelection(): void {
  inputMethodEngine.getKeyboardDelegate().off('selectionChange', onSelectionChange);
}
