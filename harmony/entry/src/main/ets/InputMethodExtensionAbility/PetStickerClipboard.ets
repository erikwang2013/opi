// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证（2026-10-18 新增；本机是 Linux，没有鸿蒙 SDK）
//
// 「自有」小欧贴纸的落点：**复制图片到系统剪贴板**。
//
// 为什么不是直接插入：IME Kit 的 `insertText` 只收字符串，没有往宿主输入框插图片的
// 接口，跨应用图片通道只剩剪贴板。用户回宿主 App 自己粘贴 —— 面板上的 toast 必须说
// 清楚这一点，别让用户以为已经上屏了。
//
// 2026-10-20：优先写 PNG → PixelMap（宿主普遍认），PNG 缺失时才试 `image/gif`。
// GIF 分支的 `createData('image/gif', ArrayBuffer)` 是否被当前 API 版本接受
// **未验证**；真机静默失败时先看这里的 `.catch` 日志。
//
// ⚠️ **全部未经编译/上机验证**。按 API 12–15 的 d.ts 记忆写的；最可能在
// `createData` 的入参类型（PixelMap/ArrayBuffer 是否直接收）与 `decodeSync` 的
// 返回类型（Uint8Array vs ArrayBuffer）上对不上。
//
// 单独一个文件而不是塞进 KeyboardController：那边已顶到 500 行门禁，而且剪贴板
// 与输入会话无关（与 iOS `PetStickerClipboard.swift` 同一分层）。

import { promptAction } from '@kit.ArkUI';
import { pasteboard, BusinessError } from '@kit.BasicServicesKit';
import { image } from '@kit.ImageKit';
import { util } from '@kit.ArkTS';

function toast(message: string): void {
  promptAction.showToast({ message: message, duration: 2000 });
}

/** PNG/PixelMap 写剪贴板（宿主最认的格式）；PNG 缺失时才试 GIF。 */
export function copyStickerToClipboard(
  gifBase64: string, pngBase64: string, label: string
): void {
  if (pngBase64.length > 0) {
    copyStickerPng(pngBase64, label);
    return;
  }
  if (gifBase64.length === 0) {
    toast(`贴纸缺失：${label}`);
    return;
  }
  try {
    const bytes: Uint8Array = new util.Base64Helper().decodeSync(gifBase64);
    const data: pasteboard.PasteData = pasteboard.createData('image/gif', bytes.buffer);
    pasteboard.getSystemPasteboard().setData(data).then(() => {
      toast(`已复制${label}贴纸，去粘贴`);
    }).catch((err: BusinessError) => {
      console.error(`[opi] GIF 剪贴板失败：${err.code}`);
      toast('复制失败：剪贴板不可用');
    });
  } catch (e) {
    console.error(`[opi] GIF 数据无效：${e}`);
    toast('贴纸数据损坏');
  }
}

/** PNG → PixelMap → 剪贴板（GIF 不支持时的回退）。 */
function copyStickerPng(pngBase64: string, label: string): void {
  if (pngBase64.length === 0) {
    toast(`贴纸缺失：${label}`);
    return;
  }
  try {
    const bytes: Uint8Array = new util.Base64Helper().decodeSync(pngBase64);
    image.createImageSource(bytes.buffer).createPixelMap().then((pixel: image.PixelMap) => {
      const data: pasteboard.PasteData =
        pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pixel);
      pasteboard.getSystemPasteboard().setData(data).then(() => {
        toast(`已复制${label}贴纸，去粘贴`);
      }).catch((err: BusinessError) => {
        console.error(`[opi] 贴纸写入剪贴板失败：${err.code}`);
        toast('复制失败：剪贴板不可用');
      });
    }).catch((err: BusinessError) => {
      // 解码失败 = 内嵌 base64 与 PNG 不符（生成脚本出错），不是用户操作问题。
      console.error(`[opi] 贴纸解码失败：${err.code}`);
      toast('贴纸数据损坏');
    });
  } catch (e) {
    console.error(`[opi] PNG 数据无效：${e}`);
    toast('贴纸数据损坏');
  }
}
