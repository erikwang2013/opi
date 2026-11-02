// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 语音模型目录与固定校验值（对齐 Android AsrModels.kt 的 SenseVoice + VAD 子集）。
//
// full 档：模型在 HAP 的 rawfile/asr/，首次预热时拷进 filesDir/asr/（sherpa 要文件路径）。
// normal 档：从站点逐文件流式下载，写入时算 SHA-256，校验通过才改名成正式文件。
// 两种路径都写 manifest.json（schema 真源 crates/opi-asr/src/manifest.rs），
// 加载时 verify_models=false —— 大小仍查，239MB 哈希不重算。
//
// 站点地址是用户指定的唯一 normal 源；失败只报可读原因，不换第三方源。

import { common } from '@kit.AbilityKit';
import { fileIo } from '@kit.CoreFileKit';
import { http } from '@kit.NetworkKit';
import { cryptoFramework } from '@kit.CryptoArchitectureKit';
import { resourceManager } from '@kit.LocalizationKit';

export const ASR_BASE_URL: string = 'https://opi.erik.xyz/download/components/asr/';
const SENSE_VOICE_DIR: string = 'sensevoice';
const MODEL_FILE: string = 'model.int8.onnx';
const TOKENS_FILE: string = 'tokens.txt';
const VAD_FILE: string = 'vad/silero_vad.onnx';
const MANIFEST_FILE: string = 'manifest.json';

const MODEL_BYTES: number = 239233841;
const MODEL_SHA: string = 'c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51';
const TOKENS_BYTES: number = 315894;
const TOKENS_SHA: string = 'f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc';
const VAD_BYTES: number = 643854;
const VAD_SHA: string = '9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6';
/** 三个文件合计；进度条的分母。 */
export const TOTAL_BYTES: number = MODEL_BYTES + TOKENS_BYTES + VAD_BYTES;

class ModelFile {
  path: string;
  url: string;
  bytes: number;
  sha256: string;

  constructor(path: string, url: string, bytes: number, sha256: string) {
    this.path = path;
    this.url = url;
    this.bytes = bytes;
    this.sha256 = sha256;
  }
}

const FILES: ModelFile[] = [
  new ModelFile(SENSE_VOICE_DIR + '/' + MODEL_FILE, ASR_BASE_URL + 'sensevoice/' + MODEL_FILE,
    MODEL_BYTES, MODEL_SHA),
  new ModelFile(SENSE_VOICE_DIR + '/' + TOKENS_FILE, ASR_BASE_URL + 'sensevoice/' + TOKENS_FILE,
    TOKENS_BYTES, TOKENS_SHA),
  new ModelFile(VAD_FILE, ASR_BASE_URL + 'vad/silero_vad.onnx', VAD_BYTES, VAD_SHA),
];

/** 与 scripts/prepare_harmony_asr.sh 写进 rawfile 的内容逐字一致。 */
const MANIFEST_JSON: string =
  '{"version":1,"files":[' +
  '{"path":"sensevoice/model.int8.onnx","size":239233841,' +
  '"sha256":"c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51","role":"model"},' +
  '{"path":"sensevoice/tokens.txt","size":315894,' +
  '"sha256":"f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc","role":"tokens"},' +
  '{"path":"vad/silero_vad.onnx","size":643854,' +
  '"sha256":"9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"}' +
  ']}';

export enum ModelPhase {
  /** normal 档且还没下载。 */
  Missing = 0,
  /** full 档自带模型，等待首启拷贝。 */
  Bundled = 1,
  Installing = 2,
  Downloading = 3,
  Ready = 4,
  Failed = 5,
}

export class ModelStatus {
  phase: ModelPhase = ModelPhase.Missing;
  doneBytes: number = 0;
  totalBytes: number = TOTAL_BYTES;
  message: string = '';
}

let phase: ModelPhase = ModelPhase.Missing;
let doneBytes: number = 0;
let lastError: string = '';
let running: boolean = false;
/** 下载只在本模块内串行跑；保存一次根目录，避免每个回调都从 ctx 取。 */
let currentRoot: string = '';

function root(ctx: common.Context): string {
  return ctx.filesDir + '/asr';
}

function exists(path: string): boolean {
  try {
    return fileIo.accessSync(path);
  } catch (e) {
    return false;
  }
}

function sizeOf(path: string): number {
  try {
    return fileIo.statSync(path).size;
  } catch (e) {
    return -1;
  }
}

/** 大小 + 清单存在即可（哈希在下载/内置拷贝时校验过）。 */
export function isReady(ctx: common.Context): boolean {
  const dir: string = root(ctx);
  if (sizeOf(dir + '/' + MANIFEST_FILE) <= 0) {
    return false;
  }
  for (let i = 0; i < FILES.length; i++) {
    if (sizeOf(dir + '/' + FILES[i].path) !== FILES[i].bytes) {
      return false;
    }
  }
  return true;
}

/** full 档检测：rawfile 里有构建期写入的清单。 */
export function hasBundled(ctx: common.Context): boolean {
  try {
    const fd = ctx.resourceManager.getRawFdSync('asr/' + MANIFEST_FILE);
    const present: boolean = fd.length > 0;
    ctx.resourceManager.closeRawFdSync('asr/' + MANIFEST_FILE);
    return present;
  } catch (e) {
    return false;
  }
}

export function status(ctx: common.Context): ModelStatus {
  const s: ModelStatus = new ModelStatus();
  if (isReady(ctx)) {
    phase = ModelPhase.Ready;
  } else if (phase === ModelPhase.Missing && hasBundled(ctx)) {
    phase = ModelPhase.Bundled;
  }
  s.phase = phase;
  s.doneBytes = doneBytes;
  s.totalBytes = TOTAL_BYTES;
  s.message = lastError;
  return s;
}

function setPhase(p: ModelPhase, message: string): void {
  phase = p;
  lastError = message;
}

function sleep(ms: number): Promise<void> {
  return new Promise<void>((resolve: () => void) => {
    setTimeout(resolve, ms);
  });
}

function hex(data: Uint8Array): string {
  let out: string = '';
  for (let i = 0; i < data.byteLength; i++) {
    out += data[i].toString(16).padStart(2, '0');
  }
  return out;
}

function writeText(path: string, text: string): void {
  const file: fileIo.File = fileIo.openSync(path,
    fileIo.OpenMode.CREATE | fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.TRUNC);
  try {
    const written: number = fileIo.writeSync(file.fd, text);
    if (written !== text.length) {
      throw new Error('清单写入不完整');
    }
  } finally {
    fileIo.closeSync(file);
  }
}

async function copyRaw(rm: resourceManager.ResourceManager, name: string, dest: string,
  expectedBytes: number, expectedSha: string): Promise<void> {
  const raw = rm.getRawFdSync(name);
  const md = cryptoFramework.createMd('SHA256');
  let file: fileIo.File | null = null;
  try {
    file = fileIo.openSync(dest,
      fileIo.OpenMode.CREATE | fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.TRUNC);
    const chunk: ArrayBuffer = new ArrayBuffer(1024 * 1024);
    let pos: number = 0;
    while (pos < raw.length) {
      const want: number = Math.min(chunk.byteLength, raw.length - pos);
      // raw.offset 是资产在 HAP 里的起点；fs.read 的 offset 相对同一个 fd。
      const n: number = await fileIo.read(raw.fd, chunk, { offset: raw.offset + pos, length: want });
      if (n <= 0) {
        throw new Error('读取内置模型失败');
      }
      const view: Uint8Array = new Uint8Array(chunk, 0, n);
      md.updateSync({ data: view });
      const written: number = fileIo.writeSync(file.fd, n === chunk.byteLength ? chunk : chunk.slice(0, n));
      if (written !== n) {
        throw new Error('写入内置模型失败');
      }
      pos += n;
    }
    fileIo.fsyncSync(file.fd);
  } finally {
    if (file !== null) {
      fileIo.closeSync(file);
    }
    rm.closeRawFdSync(name);
  }
  // 内容校验同时兜住「rawfile fd offset 语义不符」的设备差异。
  if (sizeOf(dest) !== expectedBytes || hex(md.digestSync().data) !== expectedSha) {
    throw new Error('内置模型校验失败');
  }
}

/** 整块读取兜底：个别设备不支持按 offset 读 rawfile fd 时才走到这里。 */
function copyRawWhole(rm: resourceManager.ResourceManager, name: string, dest: string,
  expectedBytes: number, expectedSha: string): void {
  const bytes: Uint8Array = rm.getRawFileContentSync(name);
  if (bytes.byteLength !== expectedBytes) {
    throw new Error('内置模型大小不符');
  }
  const md = cryptoFramework.createMd('SHA256');
  md.updateSync({ data: bytes });
  if (hex(md.digestSync().data) !== expectedSha) {
    throw new Error('内置模型校验失败');
  }
  const file: fileIo.File = fileIo.openSync(dest,
    fileIo.OpenMode.CREATE | fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.TRUNC);
  try {
    const buffer: ArrayBuffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    if (fileIo.writeSync(file.fd, buffer) !== bytes.byteLength) {
      throw new Error('写入内置模型失败');
    }
  } finally {
    fileIo.closeSync(file);
  }
}

/** full 档：把随包模型拷进沙箱（幂等；失败返回 false，调用方显示可读状态）。 */
export async function installBundled(ctx: common.Context): Promise<boolean> {
  if (isReady(ctx)) {
    setPhase(ModelPhase.Ready, '');
    return true;
  }
  const rm = ctx.resourceManager;
  if (!hasBundled(ctx)) {
    setPhase(ModelPhase.Missing, '');
    return false;
  }
  setPhase(ModelPhase.Installing, '正在准备内置语音模型…');
  doneBytes = 0;
  try {
    const dir: string = root(ctx);
    fileIo.mkdirSync(dir + '/' + SENSE_VOICE_DIR, true);
    fileIo.mkdirSync(dir + '/vad', true);
    for (let i = 0; i < FILES.length; i++) {
      const f: ModelFile = FILES[i];
      const dest: string = dir + '/' + f.path;
      try {
        await copyRaw(rm, 'asr/' + f.path, dest, f.bytes, f.sha256);
      } catch (e) {
        try {
          fileIo.unlinkSync(dest);
        } catch (ignored) {
          // 分块拷贝没写成功时目标可能不存在
        }
        copyRawWhole(rm, 'asr/' + f.path, dest, f.bytes, f.sha256);
      }
      doneBytes += f.bytes;
    }
    writeText(dir + '/' + MANIFEST_FILE, MANIFEST_JSON);
    if (!isReady(ctx)) {
      throw new Error('内置模型安装后校验失败');
    }
    setPhase(ModelPhase.Ready, '');
    return true;
  } catch (e) {
    try {
      fileIo.unlinkSync(root(ctx) + '/' + MANIFEST_FILE);
    } catch (ignored) {
      // 清单可能尚未写入
    }
    setPhase(ModelPhase.Failed, `内置语音模型准备失败：${e}`);
    return false;
  }
}

class DownloadFailure extends Error {
}

/** 单文件流式下载：边写边算 SHA-256，成功后才 rename 成正式文件。 */
async function downloadFile(f: ModelFile): Promise<void> {
  const dest: string = currentRoot + '/' + f.path;
  const part: string = dest + '.part';
  fileIo.mkdirSync(part.substring(0, part.lastIndexOf('/')), true);
  let file: fileIo.File | null = null;
  let req: http.HttpRequest | null = null;
  let writeError: string = '';
  let received: number = 0;
  const md = cryptoFramework.createMd('SHA256');
  try {
    fileIo.unlinkSync(part);
  } catch (e) {
    // 没有残留
  }
  try {
    file = fileIo.openSync(part,
      fileIo.OpenMode.CREATE | fileIo.OpenMode.READ_WRITE | fileIo.OpenMode.TRUNC);
    const fd: number = file.fd;
    req = http.createHttp();
    req.on('dataReceive', (chunk: ArrayBuffer) => {
      if (writeError.length > 0) {
        return;
      }
      const data: Uint8Array = new Uint8Array(chunk);
      const written: number = fileIo.writeSync(fd, chunk);
      if (written !== data.byteLength) {
        writeError = '模型写入失败（存储空间不足？）';
        return;
      }
      md.updateSync({ data: data });
      received += written;
      doneBytes += written;
    });
    const code: number = await req.requestInStream(f.url, {
      method: http.RequestMethod.GET,
      connectTimeout: 30000,
      readTimeout: 60000,
      usingCache: false
    });
    if (code !== http.ResponseCode.OK) {
      throw new DownloadFailure(`服务器返回 ${code}`);
    }
    if (writeError.length > 0) {
      throw new DownloadFailure(writeError);
    }
    if (received !== f.bytes) {
      throw new DownloadFailure(`下载不完整（${received}/${f.bytes} 字节）`);
    }
    if (hex(md.digestSync().data) !== f.sha256) {
      throw new DownloadFailure('模型校验失败（SHA-256 不符）');
    }
  } finally {
    if (file !== null) {
      fileIo.closeSync(file);
    }
    if (req !== null) {
      req.destroy();
    }
  }
  try {
    fileIo.unlinkSync(dest);
  } catch (e) {
    // 首次下载
  }
  fileIo.renameSync(part, dest);
}

/** 下载入口：同一时间只跑一个；结果写进 status()。调用方不 await。 */
export async function download(ctx: common.Context): Promise<void> {
  if (running || isReady(ctx)) {
    return;
  }
  running = true;
  currentRoot = root(ctx);
  doneBytes = 0;
  setPhase(ModelPhase.Downloading, '');
  try {
    fileIo.mkdirSync(currentRoot + '/' + SENSE_VOICE_DIR, true);
    fileIo.mkdirSync(currentRoot + '/vad', true);
    for (let i = 0; i < FILES.length; i++) {
      const f: ModelFile = FILES[i];
      let last: string = '';
      let ok: boolean = false;
      for (let attempt = 0; attempt < 3 && !ok; attempt++) {
        try {
          await downloadFile(f);
          ok = true;
        } catch (e) {
          last = `${e}`;
          // doneBytes 含本次失败的字节；回退到已完成的文件大小，进度不回跳太多。
          doneBytes = completedBytes(i);
          if (attempt < 2) {
            await sleep(1000);
          }
        }
      }
      if (!ok) {
        throw new DownloadFailure(last.length > 0 ? last : '下载失败');
      }
    }
    writeText(currentRoot + '/' + MANIFEST_FILE, MANIFEST_JSON);
    if (!isReady(ctx)) {
      throw new DownloadFailure('下载后校验失败');
    }
    setPhase(ModelPhase.Ready, '');
  } catch (e) {
    setPhase(ModelPhase.Failed, `语音模型下载失败：${e}`);
  } finally {
    running = false;
  }
}

function completedBytes(index: number): number {
  let total: number = 0;
  for (let i = 0; i < index && i < FILES.length; i++) {
    total += FILES[i].bytes;
  }
  return total;
}
