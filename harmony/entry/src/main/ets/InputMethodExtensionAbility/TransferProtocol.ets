// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 跨设备传输的宿主侧协议面（纯函数，无 UI/无内核句柄）：
//   · transfer_* 事件 JSON（真源 crates/opi-ffi/src/cabi_clip/encode.rs）
//   · clipSendFiles 的文件清单 JSON（真源 cabi_clip/transfer_exports.rs）
//   · 发送前的 128 项 / 2 GiB / 8 GiB 限额（真源 crates/opi-clip/src/types.rs）
// 手写解析而不引第三方：只吃自家 serde 输出，schema 固定；事件上限 1 MiB 由 Rust 保证。

import { util } from '@kit.ArkTS';

export const MAX_TRANSFER_FILES: number = 128;
export const MAX_TRANSFER_FILE_BYTES: number = 2 * 1024 * 1024 * 1024;
export const MAX_TRANSFER_TOTAL_BYTES: number = 8 * 1024 * 1024 * 1024;
export const MAX_TRANSFER_PATH_BYTES: number = 256;
export const MAX_TRANSFER_RECORDS: number = 20;

export enum TransferPhase {
  Offered = 0,
  Active = 1,
  Completed = 2,
  Failed = 3,
  Canceled = 4,
}

export interface TransferFileInfo {
  relativePath: string;
  size: number;
  mime: string;
  isDir: boolean;
}

export interface ClipTransfer {
  id: string;
  instanceId: string;
  peerName: string;
  outgoing: boolean;
  files: TransferFileInfo[];
  totalBytes: number;
  transferredBytes: number;
  fileIndex: number;
  fileCount: number;
  phase: TransferPhase;
  savedDir: string;
  reason: string;
}

/** clipSendFiles 的一项：文件给 path+name+mime；目录只给 path（内核递归展开）。 */
export interface TransferSpec {
  path: string;
  name: string;
  mime: string;
  isDir: boolean;
}

/** 扫描阶段的一项（复制前判限额；目录 size 恒 0）。 */
export interface TransferEntry {
  relativePath: string;
  size: number;
  mime: string;
  isDir: boolean;
}

export interface TransferEvent {
  type: string;
  instanceId: string;
  transferId: string;
  files: TransferFileInfo[];
  totalBytes: number;
  transferredBytes: number;
  fileIndex: number;
  fileCount: number;
  outgoing: boolean;
  savedDir: string;
  reason: string;
}

interface RawFile {
  relative_path?: string;
  size?: number;
  mime?: string;
  is_dir?: boolean;
}

interface RawEvent {
  type?: string;
  instance_id?: string;
  transfer_id?: string;
  files?: RawFile[];
  total_bytes?: number;
  transferred_bytes?: number;
  file_index?: number;
  file_count?: number;
  outgoing?: boolean;
  saved_dir?: string | null;
  reason?: string | null;
}

function str(v: string | null | undefined): string {
  return typeof v === 'string' ? v : '';
}

function num(v: number | undefined): number {
  return typeof v === 'number' ? v : 0;
}

function filesOf(raw: RawFile[] | undefined): TransferFileInfo[] {
  const out: TransferFileInfo[] = [];
  if (!Array.isArray(raw)) {
    return out;
  }
  for (let i = 0; i < raw.length; i++) {
    const item: RawFile = raw[i];
    if (typeof item.relative_path !== 'string') {
      continue;
    }
    out.push({
      relativePath: item.relative_path,
      size: num(item.size),
      mime: str(item.mime),
      isDir: item.is_dir === true,
    });
  }
  return out;
}

/** 只认 transfer_*；其它事件（peer/clipboard/pairing）返回 null，传输层不参与。 */
export function parseTransferEvent(json: string): TransferEvent | null {
  let raw: RawEvent;
  try {
    raw = JSON.parse(json) as RawEvent;
  } catch (e) {
    return null;
  }
  const type: string = str(raw.type);
  if (!type.startsWith('transfer_') || typeof raw.transfer_id !== 'string') {
    return null;
  }
  return {
    type: type,
    instanceId: str(raw.instance_id),
    transferId: raw.transfer_id,
    files: filesOf(raw.files),
    totalBytes: num(raw.total_bytes),
    transferredBytes: num(raw.transferred_bytes),
    fileIndex: num(raw.file_index),
    fileCount: num(raw.file_count),
    outgoing: raw.outgoing === true,
    savedDir: str(raw.saved_dir),
    reason: str(raw.reason),
  };
}

function jsonString(value: string): string {
  let out: string = '"';
  for (let i = 0; i < value.length; i++) {
    const c: string = value.charAt(i);
    if (c === '"') {
      out += '\\"';
    } else if (c === '\\') {
      out += '\\\\';
    } else if (c === '\n') {
      out += '\\n';
    } else if (c === '\r') {
      out += '\\r';
    } else if (c === '\t') {
      out += '\\t';
    } else if (value.charCodeAt(i) < 0x20) {
      out += `\\u${value.charCodeAt(i).toString(16).padStart(4, '0')}`;
    } else {
      out += c;
    }
  }
  return out + '"';
}

export function transferFilesJson(specs: TransferSpec[]): string {
  let out: string = '[';
  for (let i = 0; i < specs.length; i++) {
    const spec: TransferSpec = specs[i];
    if (i > 0) {
      out += ',';
    }
    out += `{"path":${jsonString(spec.path)}`;
    if (!spec.isDir) {
      if (spec.name.length > 0) {
        out += `,"name":${jsonString(spec.name)}`;
      }
      if (spec.mime.length > 0) {
        out += `,"mime":${jsonString(spec.mime)}`;
      }
    }
    out += '}';
  }
  return out + ']';
}

/** 超限原因；空串 = 可发送。判据与内核 validate_offer 同序（数量→路径→单文件→总量）。 */
export function transferLimitError(entries: TransferEntry[]): string {
  if (entries.length === 0) {
    return '没有可发送的文件';
  }
  if (entries.length > MAX_TRANSFER_FILES) {
    return `条目超过 ${MAX_TRANSFER_FILES} 个（当前 ${entries.length} 个），请减少文件`;
  }
  const encoder: util.TextEncoder = new util.TextEncoder();
  const seen: Set<string> = new Set<string>();
  let total: number = 0;
  for (let i = 0; i < entries.length; i++) {
    const entry: TransferEntry = entries[i];
    if (encoder.encodeInto(entry.relativePath).length > MAX_TRANSFER_PATH_BYTES) {
      return `路径过长（超过 ${MAX_TRANSFER_PATH_BYTES} 字节）：${entry.relativePath.substring(0, 48)}…`;
    }
    if (seen.has(entry.relativePath)) {
      return `存在重名路径：${entry.relativePath}`;
    }
    seen.add(entry.relativePath);
    if (!entry.isDir) {
      if (entry.size > MAX_TRANSFER_FILE_BYTES) {
        return `单个文件超过 2 GiB：${entry.relativePath}`;
      }
      total += entry.size;
      if (total > MAX_TRANSFER_TOTAL_BYTES) {
        return '总大小超过 8 GiB，请分批发送';
      }
    }
  }
  return '';
}

/** 显示名 → 安全的相对名（内核拒绝 / \ : 与 . / .. 组件）。 */
export function safeTransferName(raw: string): string {
  let last: string = raw;
  const slash: number = Math.max(last.lastIndexOf('/'), last.lastIndexOf('\\'));
  if (slash >= 0) {
    last = last.substring(slash + 1);
  }
  let cleaned: string = '';
  for (let i = 0; i < last.length; i++) {
    const c: string = last.charAt(i);
    const code: number = last.charCodeAt(i);
    if (c === ':' || c === '\\' || code < 0x20) {
      cleaned += '_';
    } else {
      cleaned += c;
    }
  }
  cleaned = cleaned.trim();
  while (cleaned.startsWith('.')) {
    cleaned = cleaned.substring(1);
  }
  while (cleaned.endsWith('.')) {
    cleaned = cleaned.substring(0, cleaned.length - 1);
  }
  return cleaned.length > 0 ? cleaned : '文件';
}

/** 同一批选择里的重名：保留扩展名，在主干后加 ` (n)`。 */
export function uniqueTransferName(name: string, used: Set<string>): string {
  if (!used.has(name)) {
    used.add(name);
    return name;
  }
  const dot: number = name.lastIndexOf('.');
  const stem: string = dot > 0 ? name.substring(0, dot) : name;
  const ext: string = dot > 0 ? name.substring(dot) : '';
  let index: number = 2;
  while (true) {
    const candidate: string = `${stem} (${index})${ext}`;
    if (!used.has(candidate)) {
      used.add(candidate);
      return candidate;
    }
    index++;
  }
}

export function mimeForName(name: string): string {
  const dot: number = name.lastIndexOf('.');
  const ext: string = dot >= 0 ? name.substring(dot + 1).toLowerCase() : '';
  switch (ext) {
    case 'txt': case 'md': case 'log': return 'text/plain';
    case 'json': return 'application/json';
    case 'html': case 'htm': return 'text/html';
    case 'css': return 'text/css';
    case 'xml': return 'application/xml';
    case 'pdf': return 'application/pdf';
    case 'zip': return 'application/zip';
    case 'png': return 'image/png';
    case 'jpg': case 'jpeg': return 'image/jpeg';
    case 'gif': return 'image/gif';
    case 'webp': return 'image/webp';
    case 'mp4': return 'video/mp4';
    case 'mp3': return 'audio/mpeg';
    case 'wav': return 'audio/wav';
    case 'ogg': return 'audio/ogg';
    default: return 'application/octet-stream';
  }
}

export function formatTransferBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units: string[] = ['KiB', 'MiB', 'GiB', 'TiB'];
  let value: number = bytes;
  let unit: number = -1;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return value >= 100 ? `${Math.floor(value)} ${units[unit]}`
    : `${Math.floor(value * 10) / 10} ${units[unit]}`;
}
