// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 传输设备名规则（从 ClipSync 拆出守 500 行）：只允许中文、英文字母、数字；
// 系统默认名先清洗，绝不把空格/标点带进 beacon/握手。

import { deviceInfo } from '@kit.BasicServicesKit';
import * as SettingsStore from './SettingsStore';

function allowedDeviceChar(code: number): boolean {
  return (code >= 48 && code <= 57) || (code >= 65 && code <= 90) || (code >= 97 && code <= 122)
    || (code >= 0x3400 && code <= 0x4DBF) || (code >= 0x4E00 && code <= 0x9FFF)
    || (code >= 0xF900 && code <= 0xFAFF);
}

function utf8Len(text: string): number {
  let n: number = 0;
  for (let i = 0; i < text.length; i++) {
    const c: number = text.charCodeAt(i);
    n += c < 0x80 ? 1 : (c < 0x800 ? 2 : 3);
  }
  return n;
}

export function validDeviceName(name: string): boolean {
  if (name.length === 0 || utf8Len(name) > 64) {
    return false;
  }
  for (let i = 0; i < name.length; i++) {
    if (!allowedDeviceChar(name.charCodeAt(i))) {
      return false;
    }
  }
  return true;
}

function sanitizeDeviceName(raw: string): string {
  let out: string = '';
  for (let i = 0; i < raw.length; i++) {
    const code: number = raw.charCodeAt(i);
    if (!allowedDeviceChar(code)) {
      continue;
    }
    if (utf8Len(out + raw.charAt(i)) > 64) {
      break;
    }
    out += raw.charAt(i);
  }
  return out;
}

/** 空串 = 可用；非空 = 给用户看的原因。 */
export function deviceNameError(name: string): string {
  const t: string = name.trim();
  if (t.length === 0) {
    return '';
  }
  if (utf8Len(t) > 64) {
    return '最多 64 字节（中文约 21 个字）';
  }
  return validDeviceName(t) ? '' : '只能使用中文、英文字母和数字，不能有空格或符号';
}

export function deviceName(): string {
  const custom: string = SettingsStore.load().clipDeviceName.trim();
  if (validDeviceName(custom)) {
    return custom;
  }
  let name: string = '';
  try {
    name = sanitizeDeviceName(deviceInfo.marketName);
  } catch (e) {
    name = '';
  }
  return name.length === 0 ? 'HarmonyOS' : name;
}
