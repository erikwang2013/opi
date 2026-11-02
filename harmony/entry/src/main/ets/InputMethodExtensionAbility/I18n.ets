// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置/关于页的中英语言层（2026-11-02）。只翻译这两类页面，键面与候选保持原样。
// 翻译表内嵌自 shared/i18n/zh-en.tsv（鸿蒙端没有运行时读 tsv 的通道）；查表顺序与
// Android/桌面一致：精确匹配 → 最长前缀匹配，查不到保留原文（宁可半句中文，不显示空串）。
//
// 语言态放 AppStorage 'opiLang'（0=中文，1=英文），落盘走 SettingsStore；页面用
// @StorageLink('opiLang') 订阅，切换后当帧重渲染。ensureLanguage() 在面板
// aboutToAppear 首次调用，从偏好灌入 AppStorage（设置页出现前 SettingsStore 已 init）。

import * as SettingsStore from './SettingsStore';
import { Palette, paletteFor } from './Palette';

export const LANG_KEY: string = 'opiLang';

const TABLE: string = `设置\tSettings
关于\tAbout
常用\tCommon
外观\tAppearance
键盘\tKeyboard
输入模式\tInput mode
界面语言\tInterface language
中文\t中文
英文\tEnglish
学习\tLearning
智能联想\tSmart association
词库\tDictionaries
版本\tVersion
更新\tUpdate
版权\tCopyright
状态\tStatus
皮肤\tSkin
未知\tUnknown
常用 / 键盘 / 外观 / 关于\tCommon / Keyboard / Appearance / About
根据选词学习用户词频\tLearn word frequency from your selections
提交后联想常用续接词句（默认关闭）\tSuggest common follow-up words after commit (off by default)
英文专业模式\tEnglish Pro mode
英文候选与联想；关闭时字母直接上屏\tEnglish candidates and suggestions; when off, letters are committed directly
表情页显示自有\tShow own stickers in emoji
在表情页显示小欧贴纸分类\tShow the opi sticker category on the emoji page
内置回退词库（完整词库未装上）\tBuilt-in fallback dictionary (full dictionary not installed)
完整词库\tFull dictionary
清除用户词库\tClear user words
删除学习积累的词频；内置词库不受影响\tDelete learned word frequencies; built-in dictionaries are unaffected
确认清除全部用户词？\tClear all user words?
取消\tCancel
清除\tClear
键盘布局与输入方案\tKeyboard layout and input scheme
繁体模式\tTraditional Chinese
中文输入使用繁体词库\tUse traditional Chinese dictionaries for Chinese input
单手模式：关\tOne-handed: off
单手模式：左手\tOne-handed: left
单手模式：右手\tOne-handed: right
悬浮模式\tFloating mode
键盘收窄居中，可拖动\tNarrow, centred keyboard that can be dragged
游戏模式\tGame mode
更矮的键盘，减少遮挡\tShorter keyboard with less screen coverage
11 款纯色 + 19 套动态；选中即换\t11 solid colours + 19 dynamic themes; tap to apply
暗光模式\tDark mode
深色键盘配色\tDark keyboard colours
长辈模式\tLarge text mode
大字号键盘与面板\tLarger text on keys and panels
关于与链接\tAbout & links
简介 / 版权 / 项目地址\tIntro / copyright / project links
拼音26键\tPinyin 26-key
英文26键\tEnglish 26-key
拼音9键\tPinyin 9-key
拼音14键\tPinyin 14-key
拼音大9键\tPinyin large 9-key
笔画键盘\tStroke keyboard
生僻字键盘\tRare-character keyboard
拼音双拼\tShuangpin
五笔\tWubi
仅在键盘输入时运行；默认关闭\tRuns only during keyboard input; off by default
等待键盘输入会话\tWaiting for a keyboard input session
运行中 · 发现\tRunning · discovered
台设备\tdevice(s)
本机指纹\tLocal fingerprint
最近来源\tLast source
上次配对\tLast pairing
对方请求配对\tPairing requested by the other device
确认配对\tConfirm pairing
核对两台设备显示相同的 6 位数字；不一致请拒绝。\tVerify that both devices display the same 6-digit code; reject if they differ.
允许\tAllow
把当前剪贴板发到已配对设备\tSend the current clipboard to paired devices
系统限制后台读取剪贴板：点「粘贴」手动同步一次，或从系统复制后立即同步。\tThe system limits background clipboard reads: tap Paste to sync once manually, or sync right after copying.
收到的文本只写入系统剪贴板，不会自动粘贴；请回到应用里手动粘贴。\tReceived text is written only to the system clipboard and is never pasted automatically; paste it manually in the app.
设备\tDevices
未发现设备。确保两台设备在同一局域网、都打开开关，且本机未被基础体验模式限制。\tNo devices found. Make sure both devices are on the same LAN, have this feature enabled, and this device is not restricted by Basic Experience mode.
确认移除\tConfirm removal
立即停止共享\tStop sharing now
基础体验模式会限制后台网络与剪贴板能力；若始终发现不到设备，请在系统设置中退出基础体验模式后重试。\tBasic Experience mode restricts background network and clipboard access; if no devices are ever found, exit Basic Experience mode in system settings and try again.
跨设备剪贴板\tCross-device clipboard
已连接\tConnected
未连接\tDisconnected
已配对\tPaired
已关闭\tDisabled
配对\tPair
移除\tRemove
拒绝\tReject
跨设备剪贴板未运行\tCross-device clipboard is not running
系统未授予剪贴板读取权限\tThe system has not granted clipboard read permission
系统剪贴板里没有纯文本\tThe system clipboard contains no plain text
剪贴板内容超过 256 KiB，未同步\tClipboard content exceeds 256 KiB and was not synced
未获得剪贴板读取授权\tClipboard read authorisation not granted
写入系统剪贴板失败\tFailed to write to the system clipboard
发送到一台已配对设备失败\tFailed to send to a paired device
跨设备剪贴板调用失败\tCross-device clipboard call failed
小欧输入法（opi）是一款回归本质、人人可用的多端输入法。Open People Input 开放、纯粹、跨端，致力于为每一位用户提供不被干扰、不被窥探、不被绑架的输入体验。\topi (Open People Input) is a back-to-basics, multi-platform input method for everyone. Open People Input is open, simple and cross-platform, dedicated to an input experience free from interruption, surveillance and lock-in.
© 2026 erik.xyz · 代码按 MIT 许可（全文见仓库 LICENSE）；词库数据与代码分开授权\t© 2026 erik.xyz · Code under the MIT licence (full text in the repository LICENSE); dictionary data is licensed separately.
表情数据\tEmoji data
Emoji 取自 OpenMoji（CC BY-SA 4.0）；颜文字取自 rime_kaomoji_dict（MIT）；小欧贴纸/图标为项目专有资产，商业使用需授权\tEmoji from OpenMoji (CC BY-SA 4.0); kaomoji from rime_kaomoji_dict (MIT); opi stickers/icons are proprietary assets and require authorisation for commercial use.
链接\tLinks
官网主页\tHomepage
GitHub 项目\tGitHub project
博客\tBlog
隐私政策\tPrivacy policy
鸿蒙输入法扩展不能直接拉起浏览器；点链接复制地址后到浏览器打开。\tThe HarmonyOS IME extension cannot open a browser directly; tap a link to copy its address, then open it in a browser.
链接已复制\tLink copied
复制失败\tCopy failed
正在检测更新…\tChecking for updates…
发现新版本\tNew version available
（当前 v\t (current v
）\t)
已是最新版本\tUp to date
检测更新失败：\tUpdate check failed:
当前 v\tCurrent v
站点返回的版本信息无法识别\tThe version information returned by the site could not be read
服务器返回 \tServer returned\u0020
前往下载\tGo to downloads
检测更新\tCheck for updates
无法直接打开浏览器，下载链接已复制：\tCannot open a browser directly; download link copied:
请手动访问：\tPlease visit manually:
网络不可用\tNetwork unavailable
语音输入\tVoice input
启用语音输入\tEnable voice input
本地识别，录音不上传；开启后键盘工具条出现麦克风\tOn-device recognition; recordings are not uploaded. A microphone appears in the keyboard toolbar when enabled.
模型\tModel
已就绪\tReady
内置模型（首次使用时自动准备）\tBuilt-in model (prepared automatically on first use)
正在准备内置模型…\tPreparing built-in model…
正在下载模型\tDownloading model
语音模型不可用\tVoice model unavailable
未下载\tNot downloaded
重试下载\tRetry download
下载语音模型\tDownload voice model
重试准备\tRetry preparation
停止\tStop
关闭\tClose
停止录音\tStop recording
正在听… 再次点麦克风结束\tListening… tap the microphone again to finish
正在识别…\tTranscribing…
正在准备本地语音模型…\tPreparing the on-device voice model…
语音模型尚未下载，请到「设置 → 语音输入」下载\tVoice model is not downloaded yet. Download it in Settings → Voice input.
内置语音模型准备失败，请重试或重新安装完整包\tFailed to prepare the built-in voice model. Retry or reinstall the full package.
需要麦克风权限：请在系统设置中允许小欧输入法使用麦克风\tMicrophone permission required: allow opi to use the microphone in system settings.
没有识别到语音，请重试\tNo speech detected. Please try again.
识别超时，请重试\tRecognition timed out. Please try again.
音频处理跟不上，请缩短录音\tAudio processing cannot keep up. Please record a shorter clip.
语音事件解析失败\tFailed to parse a voice event
语音识别出错\tVoice recognition error
语音识别失败\tVoice recognition failed
语音引擎初始化失败\tVoice engine initialisation failed
语音引擎初始化失败：\tVoice engine initialisation failed:
语音模型加载失败\tVoice model failed to load
语音模型加载失败：\tVoice model failed to load:
语音识别失败（\tVoice recognition failed (
无法打开麦克风：\tCannot open the microphone:
服务器返回 \tServer returned
下载不完整（\tIncomplete download (
模型校验失败（SHA-256 不符）\tModel verification failed (SHA-256 mismatch)
下载后校验失败\tVerification after download failed
下载失败\tDownload failed
模型写入失败（存储空间不足？）\tFailed to write the model (out of storage?)
语音模型下载失败：\tVoice model download failed:
内置语音模型准备失败：\tFailed to prepare the built-in voice model:
读取内置模型失败\tFailed to read the built-in model
写入内置模型失败\tFailed to write the built-in model
内置模型校验失败\tBuilt-in model verification failed
内置模型大小不符\tBuilt-in model size mismatch
清单写入不完整\tManifest write incomplete`;

function parseTable(text: string): Map<string, string> {
  const map: Map<string, string> = new Map<string, string>();
  const lines: string[] = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line: string = lines[i];
    const tab: number = line.indexOf('\t');
    if (tab <= 0) {
      continue;
    }
    map.set(line.substring(0, tab), line.substring(tab + 1));
  }
  return map;
}

const exact: Map<string, string> = parseTable(TABLE);
let prefixes: string[] = [];

/** 译一条静态文案；英文模式下先精确匹配，再按最长中文前缀补英文，动态值原样接在后面。 */
export function t(zh: string): string {
  if (AppStorage.get<number>(LANG_KEY) !== 1 || zh.length === 0) {
    return zh;
  }
  const hit: string | undefined = exact.get(zh);
  if (hit !== undefined) {
    return hit;
  }
  if (prefixes.length === 0) {
    exact.forEach((_v: string, k: string) => {
      if (k.length >= 2) {
        prefixes.push(k);
      }
    });
    prefixes.sort((a: string, b: string): number => b.length - a.length);
  }
  for (let i = 0; i < prefixes.length; i++) {
    const key: string = prefixes[i];
    if (zh.startsWith(key)) {
      const en: string | undefined = exact.get(key);
      if (en !== undefined) {
        return en + zh.substring(key.length);
      }
    }
  }
  return zh;
}

let ensured: boolean = false;

/** 首次进设置/关于页：把落盘语言灌进 AppStorage（之后切换即时生效并落盘）。 */
export function ensureLanguage(): void {
  if (ensured) {
    return;
  }
  ensured = true;
  AppStorage.setOrCreate<number>(LANG_KEY, SettingsStore.load().lang === 1 ? 1 : 0);
}

export function setLanguage(lang: number): void {
  const value: number = lang === 1 ? 1 : 0;
  AppStorage.setOrCreate<number>(LANG_KEY, value);
  SettingsStore.saveLanguage(value);
}

/** 设置页/关于页顶部的中英切换行；选中项加粗并用强调色。 */
@Component
export struct LanguageRow {
  @StorageLink('opiLang') lang: number = 0;
  @StorageLink('opiSkin') skin: number = 0;
  @StorageLink('opiDark') dark: number = 0;
  @StorageLink('opiElder') elder: number = 0;

  private pal(): Palette {
    return paletteFor(this.skin, this.dark === 1, this.elder === 1);
  }

  @Builder
  Choice(label: string, value: number) {
    Text(label)
      .fontSize(this.elder === 1 ? 15 : 13)
      .fontWeight(this.lang === value ? FontWeight.Bold : FontWeight.Normal)
      .fontColor(this.lang === value ? this.pal().accent : this.pal().dimFg)
      .padding({ left: 10, right: 10, top: 6, bottom: 6 })
      .onClick(() => {
        setLanguage(value);
      })
  }

  build() {
    Row() {
      Text(t('界面语言'))
        .fontSize(this.elder === 1 ? 16 : 14).fontColor(this.pal().panelFg)
        .layoutWeight(1)
      this.Choice('中文', 0)
      this.Choice('English', 1)
    }
    .width('100%').padding({ left: 14, right: 14, top: 8, bottom: 8 })
  }
}
