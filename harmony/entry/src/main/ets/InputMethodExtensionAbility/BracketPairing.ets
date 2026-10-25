// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 括号自动配对（鸿蒙宿主）。表与行为契约的唯一规范：`shared/brackets/README.md`
// —— Android `ime/BracketPairing.kt`、iOS `BracketPairing.swift` 逐项镜像，改表三端一起改。
//
// ⚠️ IME Kit 的「forward/backward」相对光标，但方向与 Android 命名相反（已核对 SDK
// `@ohos.inputMethodEngine.d.ts` 的 API 说明，并与 OpenHarmony 社区实测一致）：
//   · getForward = 光标**前**的文本；getBackward = 光标**后**的文本；
//   · deleteForward = 删光标**前**（= 退格）；deleteBackward = 删光标**后**；
//   · moveCursor(Direction.CURSOR_LEFT/RIGHT) 左右移一格。
// 单位按宿主原生字符；本表每个符号都只占一个单元，左右移/删一格不会劈开字符。

import { inputMethodEngine } from '@kit.IMEKit';

export interface BracketPair {
  open: string;
  close: string;
}

/** 顺序与 `shared/brackets/README.md` 一致（改表三端一起改）。 */
export const BRACKET_PAIRS: BracketPair[] = [
  { open: '(', close: ')' },
  { open: '[', close: ']' },
  { open: '{', close: '}' },
  { open: '（', close: '）' },
  { open: '【', close: '】' },
  { open: '《', close: '》' },
  { open: '「', close: '」' },
  { open: '『', close: '』' },
  { open: '“', close: '”' },
  { open: '‘', close: '’' },
];

function openingPair(text: string): BracketPair | null {
  for (let i = 0; i < BRACKET_PAIRS.length; i++) {
    if (BRACKET_PAIRS[i].open === text) {
      return BRACKET_PAIRS[i];
    }
  }
  return null;
}

function closingPair(text: string): BracketPair | null {
  for (let i = 0; i < BRACKET_PAIRS.length; i++) {
    if (BRACKET_PAIRS[i].close === text) {
      return BRACKET_PAIRS[i];
    }
  }
  return null;
}

interface PendingPair {
  pair: BracketPair;
  before: string;
  after: string;
}

/** 纯状态机（对齐 Android/iOS）：只跟踪刚插入的那一对 + 两侧各 8 个字符快照。 */
export class BracketPairing {
  private context: number = 8;
  private pending: PendingPair | null = null;
  /** 密码判定按会话缓存（InputClient 换实例 = 新会话），避免每次按键一次 binder 往返。 */
  private cachedClient: inputMethodEngine.InputClient | null = null;
  private passwordEditor: boolean = false;

  /** 返回 true = 已处理（调用方不要再 insertText）。 */
  insertTyped(client: inputMethodEngine.InputClient, text: string, english: boolean): boolean {
    if (english || this.isPassword(client)) {
      this.pending = null;
      return false;
    }
    const open: BracketPair | null = openingPair(text);
    if (open !== null) {
      this.pending = null;
      try {
        client.insertTextSync(open.open + open.close);
      } catch (e) {
        return false;
      }
      try {
        client.moveCursorSync(inputMethodEngine.Direction.CURSOR_LEFT);
      } catch (e) {
        // 配对已进文档、光标却停在闭符号后：仍算已处理，只是后续不跟踪（不猜位置）。
        return true;
      }
      this.pending = this.snapshot(client, open);
      return true;
    }
    const close: BracketPair | null = closingPair(text);
    const p: PendingPair | null = this.pending;
    if (close !== null && p !== null && p.pair.open === close.open && this.stillBetween(client, p)) {
      this.pending = null;
      try {
        client.moveCursorSync(inputMethodEngine.Direction.CURSOR_RIGHT);
      } catch (e) {
        // 移不动就不动：绝不重复插入闭符号。
      }
      return true;
    }
    this.pending = null;
    return false;
  }

  /** 返回 true = 已删掉一对（调用方不要再退格）。 */
  backspace(client: inputMethodEngine.InputClient | null): boolean {
    if (client === null || client !== this.cachedClient) {
      this.pending = null;
      return false;
    }
    const p: PendingPair | null = this.pending;
    this.pending = null;
    if (p === null || !this.stillBetween(client, p)) {
      return false;
    }
    try {
      // 先删闭符号再删开符号：第二次失败时调用方还会补一次退格（删光标前），
      // 此时至多删掉剩下的开符号，不会碰到配对之外的正文。
      client.deleteBackwardSync(1); // 删光标后 = 闭符号
      client.deleteForwardSync(1); // 删光标前 = 开符号
      return true;
    } catch (e) {
      return false;
    }
  }

  /** 新会话（inputStart/inputStop）调用：配对与密码缓存都作废，下一键重读编辑框属性。 */
  reset(): void {
    this.pending = null;
    this.cachedClient = null;
    this.passwordEditor = false;
  }

  /** 插入后的上下文快照；读不到 / 不是「open|close」就返回 null（不跟踪，绝不猜）。 */
  /**
   * 光标在 open|close 中间的快照。SDK 的 getForward/getBackward 只写了「forward/backward」，
   * 方向命名在不同版本/文档里不够硬；这里**按内容判定**：其中一侧以 open 结尾、另一侧以
   * close 开头，谁在前谁就是 before。这样方向约定反过来也不会让整档静默失效。
   */
  private snapshot(client: inputMethodEngine.InputClient, pair: BracketPair): PendingPair | null {
    try {
      const f: string = client.getForwardSync(pair.open.length + this.context);
      const b: string = client.getBackwardSync(pair.close.length + this.context);
      if (f.endsWith(pair.open) && b.startsWith(pair.close)) {
        return { pair: pair, before: f, after: b };
      }
      if (b.endsWith(pair.open) && f.startsWith(pair.close)) {
        return { pair: pair, before: b, after: f };
      }
      return null;
    } catch (e) {
      return null;
    }
  }

  private stillBetween(client: inputMethodEngine.InputClient, p: PendingPair): boolean {
    const now: PendingPair | null = this.snapshot(client, p.pair);
    return now !== null && now.before === p.before && now.after === p.after;
  }

  private isPassword(client: inputMethodEngine.InputClient): boolean {
    if (this.cachedClient !== client) {
      this.cachedClient = client;
      // 新会话：上一处的配对不再属于当前文档。
      this.pending = null;
      this.passwordEditor = false;
      try {
        const pattern: number = client.getEditorAttributeSync().inputPattern;
        this.passwordEditor = pattern === inputMethodEngine.PATTERN_PASSWORD ||
          pattern === inputMethodEngine.PATTERN_PASSWORD_NUMBER ||
          pattern === inputMethodEngine.PATTERN_PASSWORD_SCREEN_LOCK;
      } catch (e) {
        // 读不到属性 = 按普通输入框处理（不阻断打字）。
      }
    }
    return this.passwordEditor;
  }
}

/** 单例：KeyboardController 只持有一个输入会话。 */
export const bracketPairing: BracketPairing = new BracketPairing();
