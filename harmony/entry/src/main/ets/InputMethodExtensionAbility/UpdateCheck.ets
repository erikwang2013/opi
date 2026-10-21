// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 更新检测（对齐 Android settings/VersionCheck.kt + AboutSection 的「更新」一节）：
// 只 GET https://opi.erik.xyz/version.json，不携带任何用户数据，连接/读取各 4s 超时。
// version.json 形如 {"version":"1.21.2","download":"https://opi.erik.xyz/download"}。
//
// 自动检测（进入关于/设置页）失败**静默**；只有手动点「检测更新」才显示失败原因。
//
// 打开下载页：输入法扩展只有 InputMethodExtensionContext.startAbility，没有
// UIAbilityContext.openLink。系统可能以 16000069（扩展不能拉起第三方应用）拒绝，
// 失败时把下载地址复制到剪贴板并提示。

import { Want } from '@kit.AbilityKit';
import { BusinessError, pasteboard } from '@kit.BasicServicesKit';
import { InputMethodExtensionContext } from '@kit.IMEKit';
import { http } from '@kit.NetworkKit';
import { promptAction } from '@kit.ArkUI';
import { Palette, paletteFor } from './Palette';

const VERSION_URL: string = 'https://opi.erik.xyz/version.json';
const DOWNLOAD_URL: string = 'https://opi.erik.xyz/download';
const TIMEOUT_MS: number = 4000;

export enum UpdateState {
  Idle = 0,
  Checking = 1,
  Newer = 2,
  Latest = 3,
  Failed = 4,
}

export class UpdateResult {
  state: UpdateState = UpdateState.Idle;
  latest: string = '';
  download: string = '';
  reason: string = '';

  constructor(state: UpdateState, latest: string, download: string, reason: string) {
    this.state = state;
    this.latest = latest;
    this.download = download;
    this.reason = reason;
  }
}

interface VersionPoint {
  version: string;
  download: string;
}

/** 只认数字与点（`1.21.2`，调用前已去掉可选的 `v`）。 */
function isVersion(v: string): boolean {
  if (v.length === 0) {
    return false;
  }
  let digits: number = 0;
  for (let i = 0; i < v.length; i++) {
    const c: number = v.charCodeAt(i);
    if (c === 46) { // '.'
      if (digits === 0) {
        return false;
      }
      digits = 0;
    } else if (c >= 48 && c <= 57) {
      digits++;
    } else {
      return false;
    }
  }
  return digits > 0;
}

/** 语义化比较：只比数字段，缺段按 0（`1.20` == `1.20.0`）。 */
export function compareVersions(a: string, b: string): number {
  const pa: string[] = (a.startsWith('v') ? a.substring(1) : a).split('.');
  const pb: string[] = (b.startsWith('v') ? b.substring(1) : b).split('.');
  const n: number = Math.max(pa.length, pb.length);
  for (let i = 0; i < n; i++) {
    const x: number = i < pa.length ? parseInt(pa[i], 10) : 0;
    const y: number = i < pb.length ? parseInt(pb[i], 10) : 0;
    if (x !== y) {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

/** 坏 JSON / 缺 version / 非数字点号 → null（不把坏端点当「已是最新」）。 */
function parseVersionPoint(text: string): VersionPoint | null {
  let raw: VersionPoint;
  try {
    const parsed: Object | null = JSON.parse(text) as Object | null;
    if (parsed === null) {
      return null;
    }
    raw = parsed as VersionPoint;
  } catch (e) {
    return null;
  }
  if (typeof raw.version !== 'string') {
    return null;
  }
  const trimmed: string = raw.version.trim();
  const version: string = trimmed.startsWith('v') ? trimmed.substring(1) : trimmed;
  if (!isVersion(version)) {
    return null;
  }
  let download: string = DOWNLOAD_URL;
  if (typeof raw.download === 'string' && raw.download.startsWith('https://')) {
    download = raw.download;
  }
  const point: VersionPoint = { version: version, download: download };
  return point;
}

/** 拉一次版本端点；结果（含失败原因）经 callback 回到 UI 线程。 */
export function checkForUpdate(current: string, done: (result: UpdateResult) => void): void {
  let req: http.HttpRequest;
  try {
    req = http.createHttp();
  } catch (e) {
    console.error(`[opi] 创建 HTTP 请求失败：${e}`);
    done(new UpdateResult(UpdateState.Failed, '', '', '网络不可用'));
    return;
  }
  try {
    req.request(VERSION_URL, {
      method: http.RequestMethod.GET,
      connectTimeout: TIMEOUT_MS,
      readTimeout: TIMEOUT_MS,
      expectDataType: http.HttpDataType.STRING,
      usingCache: false
    }, (err: BusinessError, data: http.HttpResponse) => {
      if (err) {
        req.destroy();
        done(new UpdateResult(UpdateState.Failed, '', '', '网络不可用'));
        return;
      }
      if (data.responseCode !== http.ResponseCode.OK) {
        req.destroy();
        done(new UpdateResult(UpdateState.Failed, '', '', `服务器返回 ${data.responseCode}`));
        return;
      }
      const point: VersionPoint | null = parseVersionPoint(data.result as string);
      req.destroy();
      if (point === null) {
        done(new UpdateResult(UpdateState.Failed, '', '', '站点返回的版本信息无法识别'));
        return;
      }
      if (compareVersions(point.version, current) > 0) {
        done(new UpdateResult(UpdateState.Newer, point.version, point.download, ''));
      } else {
        done(new UpdateResult(UpdateState.Latest, point.version, point.download, ''));
      }
    });
  } catch (e) {
    req.destroy();
    console.error(`[opi] 更新检测请求失败：${e}`);
    done(new UpdateResult(UpdateState.Failed, '', '', '网络不可用'));
  }
}

function toast(message: string): void {
  promptAction.showToast({ message: message, duration: 2000 });
}

/**
 * 「更新」一节：状态一行 + 前往下载 / 检测更新。进页面即自动检测一次。
 * 调用方负责外层左右边距（关于面板已有 14，设置页自己包一层）。
 */
@Component
export struct UpdateSection {
  @Prop currentVersion: string = '';
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;
  @State state: number = UpdateState.Idle as number;
  @State latest: string = '';
  @State download: string = DOWNLOAD_URL;
  @State reason: string = '';

  aboutToAppear(): void {
    this.check(false);
  }

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  private check(manual: boolean): void {
    this.state = UpdateState.Checking as number;
    this.reason = '';
    checkForUpdate(this.currentVersion, (result: UpdateResult) => {
      if (result.state === UpdateState.Failed && !manual) {
        this.state = UpdateState.Idle as number; // 自动检测失败不打扰
        return;
      }
      this.state = result.state as number;
      this.latest = result.latest;
      this.download = result.download.length > 0 ? result.download : DOWNLOAD_URL;
      this.reason = result.reason;
    });
  }

  private statusText(): string {
    if (this.state === (UpdateState.Checking as number)) {
      return '正在检测更新…';
    }
    if (this.state === (UpdateState.Newer as number)) {
      return `发现新版本 v${this.latest}（当前 v${this.currentVersion}）`;
    }
    if (this.state === (UpdateState.Latest as number)) {
      return `已是最新版本 v${this.currentVersion}`;
    }
    if (this.state === (UpdateState.Failed as number)) {
      return `检测更新失败：${this.reason}`;
    }
    return `当前 v${this.currentVersion}`;
  }

  private openDownload(): void {
    const url: string = this.download.length > 0 ? this.download : DOWNLOAD_URL;
    const host = this.getUIContext().getHostContext();
    if (host === undefined) {
      this.copyUrl(url);
      return;
    }
    const want: Want = { action: 'ohos.want.action.viewData', uri: url };
    try {
      (host as InputMethodExtensionContext).startAbility(want).catch((e: BusinessError) => {
        console.error(`[opi] 打开下载页失败：${e.message}`);
        this.copyUrl(url);
      });
    } catch (e) {
      console.error(`[opi] 打开下载页失败：${e}`);
      this.copyUrl(url);
    }
  }

  private copyUrl(url: string): void {
    try {
      const data: pasteboard.PasteData = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, url);
      pasteboard.getSystemPasteboard().setData(data).then(() => {
        toast(`无法直接打开浏览器，下载链接已复制：${url}`);
      }).catch(() => {
        toast(`请手动访问：${url}`);
      });
    } catch (e) {
      toast(`请手动访问：${url}`);
    }
  }

  build() {
    Column() {
      Text(this.statusText())
        .fontSize(this.elder === 1 ? 14 : 12)
        .fontColor(this.state === (UpdateState.Newer as number)
          ? this.pal().accent : this.pal().panelFg)
        .width('100%').margin({ top: 3 })
      if (this.state === (UpdateState.Newer as number)) {
        Text('前往下载')
          .fontSize(this.elder === 1 ? 15 : 13).fontWeight(FontWeight.Bold)
          .fontColor(this.pal().accent).width('100%').margin({ top: 8 })
          .onClick(() => {
            this.openDownload();
          })
      }
      if (this.state !== (UpdateState.Checking as number)) {
        Text('检测更新')
          .fontSize(this.elder === 1 ? 15 : 13)
          .fontColor(this.pal().accent).width('100%').margin({ top: 8 })
          .onClick(() => {
            this.check(true);
          })
      }
    }
    .width('100%').alignItems(HorizontalAlign.Start)
  }
}
