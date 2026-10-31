// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 系统剪贴板 ⇄ opi-clip 内核。只处理纯文本；图片/文件/URI 一律忽略。
//
// 写入（远端 → 本机）不需要权限。读取（本机复制 → 远端）在 API 12+ 受
// ohos.permission.READ_PASTEBOARD 限制（system_basic + user_grant，普通应用不能
// 只声明就用）。所以本文件走两条路：
//   · update 监听里尝试 getDataSync()：有权限就同步；没有就静默置 blocked，
//     由设置页提示「用粘贴按钮手动同步一次」；
//   · 设置页的 PasteButton（系统安全控件）被用户点击后，宿主调 readText()，
//     这一次读取拿到系统的临时授权。
// 内容**不写日志**（只记失败事实），由调用方决定写到哪里。

import { pasteboard } from '@kit.BasicServicesKit';

/** 内核按 UTF-8 字节限 256 KiB；这里按 UTF-16 字符数近似挡。 */
const MAX_TEXT_CHARS: number = 256 * 1024;

export class ClipboardAdapter {
  private readonly onLocalText: (text: string) => void;
  private readonly onReadLimited: () => void;
  // off('update') 需要同一个函数引用；与 KeyboardController 的订阅回调同一条先例。
  private readonly listener: () => void = (): void => {
    this.handleUpdate();
  };
  /** 最近一次远端写入的内容：抑制系统回放的同一条 update。 */
  private echoText: string = '';
  private started: boolean = false;
  private blocked: boolean = false;

  constructor(onLocalText: (text: string) => void, onReadLimited: () => void) {
    this.onLocalText = onLocalText;
    this.onReadLimited = onReadLimited;
  }

  start(): void {
    if (this.started) {
      return;
    }
    this.started = true;
    try {
      pasteboard.getSystemPasteboard().on('update', this.listener);
    } catch (e) {
      console.error('[opi] 剪贴板监听注册失败');
      this.started = false;
    }
  }

  stop(): void {
    if (!this.started) {
      return;
    }
    this.started = false;
    this.echoText = '';
    this.blocked = false;
    try {
      pasteboard.getSystemPasteboard().off('update', this.listener);
    } catch (e) {
      console.error('[opi] 剪贴板监听取消失败');
    }
  }

  /** 远端文本 → 系统剪贴板。**只写剪贴板，绝不自动粘贴**。false = 写入失败。 */
  applyRemote(text: string): boolean {
    this.echoText = text;
    try {
      const data: pasteboard.PasteData =
        pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text);
      pasteboard.getSystemPasteboard().setDataSync(data);
      return true;
    } catch (e) {
      console.error('[opi] 写入系统剪贴板失败');
      return false;
    }
  }

  /**
   * 读当前剪贴板纯文本。必须在 PasteButton 点击成功回调里调用才可能拿到临时授权；
   * 无权限 / 非文本 / 超限 → 空串。`readBlocked()` 区分「读不到」与「真的空」。
   */
  readText(): string {
    this.blocked = false;
    try {
      const data: pasteboard.PasteData = pasteboard.getSystemPasteboard().getDataSync();
      if (!data.hasType(pasteboard.MIMETYPE_TEXT_PLAIN)) {
        return '';
      }
      const text: string = data.getPrimaryText();
      // shortcut: 按字符数近似 256 KiB 字节上限；需要精确字节时再转 UTF-8 量。
      return text.length > MAX_TEXT_CHARS ? '' : text;
    } catch (e) {
      // 201 = 没有 READ_PASTEBOARD / 临时授权；后台监听里是常态，不是错误。
      this.blocked = true;
      return '';
    }
  }

  readBlocked(): boolean {
    return this.blocked;
  }

  private handleUpdate(): void {
    if (!this.started) {
      return;
    }
    const text: string = this.readText();
    if (this.blocked) {
      this.onReadLimited();
      return;
    }
    if (text.length === 0) {
      return;
    }
    if (this.echoText.length > 0) {
      if (this.echoText === text) {
        return;
      }
      this.echoText = '';
    }
    this.onLocalText(text);
  }
}
