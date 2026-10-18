// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证（2026-10-18 新增；本机是 Linux，没有鸿蒙 SDK）
//
// 「自有」小欧贴纸的落点：**复制 PNG 到系统剪贴板**。
//
// 为什么不是直接插入：IME Kit 的 `insertText` 只收字符串，没有往宿主输入框插图片的
// 接口，跨应用图片通道只剩剪贴板。用户回宿主 App 自己粘贴 —— 面板上的 toast 必须说
// 清楚这一点，别让用户以为已经上屏了。
//
// 链路：base64 → `util.Base64Helper().decodeSync` → `image.createImageSource` →
// `createPixelMap` → `pasteboard.createData(MIMETYPE_PIXELMAP)` → 系统剪贴板。
//
// ⚠️ **全部未经编译/上机验证**。按 API 12–15 的 d.ts 记忆写的；最可能在
// `createData` 的入参类型（PixelMap 是否直接收）与 `decodeSync` 的返回类型
// （Uint8Array vs ArrayBuffer）上对不上。真机若**静默失败**，先看 `setData` 的
// `.catch`（剪贴板在扩展里可能受前台/权限限制），再看 `Image(dataUri)` 能不能画
// （见 PetStickerImages.ets 头注）。
//
// 单独一个文件而不是塞进 KeyboardController：那边已顶到 500 行门禁，而且剪贴板
// 与输入会话无关（与 iOS `PetStickerClipboard.swift` 同一分层）。

import { promptAction } from '@kit.ArkUI';
import { pasteboard, BusinessError } from '@kit.BasicServicesKit';
import { image } from '@kit.ImageKit';
import { util } from '@kit.ArkTS';

/**
 * 复制一张贴纸并弹 toast。**不提交任何文本**。
 *
 * @param base64 `PetStickerImages.stickerBase64()` 的结果（纯 base64，无 data URI 前缀）
 * @param label 形态中文标签，只用于提示文案
 */
export function copyStickerToClipboard(base64: string, label: string): void {
  if (base64.length === 0) {
    promptAction.showToast({ message: `贴纸缺失：${label}`, duration: 2000 });
    return;
  }
  const bytes: Uint8Array = new util.Base64Helper().decodeSync(base64);
  image.createImageSource(bytes.buffer).createPixelMap().then((pixel: image.PixelMap) => {
    const data: pasteboard.PasteData =
      pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pixel);
    pasteboard.getSystemPasteboard().setData(data).then(() => {
      promptAction.showToast({ message: `已复制小欧${label}贴纸，去粘贴`, duration: 2000 });
    }).catch((err: BusinessError) => {
      console.error(`[opi] 贴纸写入剪贴板失败：${err.code}`);
      promptAction.showToast({ message: '复制失败：剪贴板不可用', duration: 2000 });
    });
  }).catch((err: BusinessError) => {
    // 解码失败 = 内嵌 base64 与 PNG 不符（生成脚本出错），不是用户操作问题。
    console.error(`[opi] 贴纸解码失败：${err.code}`);
    promptAction.showToast({ message: '贴纸数据损坏', duration: 2000 });
  });
}
