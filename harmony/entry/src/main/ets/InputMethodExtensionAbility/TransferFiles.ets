// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 系统选择器 → 应用缓存 staging → clipSendFiles 的本地路径。
// 内核只收本机绝对路径（std::fs），所以必须先把 picker 的 URI 复制进
// `${ctx.cacheDir}/transfer/<id>`；终态事件后由 TransferSync 删除。
// 目录只给内核一个 path（递归展开），避免宿主自己拼相对名与内核不一致。
// 选中的 URI 由系统授予临时读权限，不需要新声明任何存储权限。

import { common } from '@kit.AbilityKit';
import { fileIo, fileUri, picker } from '@kit.CoreFileKit';
import { photoAccessHelper } from '@kit.MediaLibraryKit';
import {
  MAX_TRANSFER_FILES, TransferEntry, TransferSpec, mimeForName, safeTransferName,
  transferLimitError, uniqueTransferName
} from './TransferProtocol';

export enum TransferKind {
  Media = 0,
  Files = 1,
  Folder = 2,
}

export interface PreparedTransfer {
  specs: TransferSpec[];
  entries: TransferEntry[];
  stageDir: string;
}

function joinUri(base: string, name: string): string {
  const trimmed: string = base.endsWith('/') ? base.substring(0, base.length - 1) : base;
  return `${trimmed}/${name}`;
}

function displayName(uri: string): string {
  try {
    const name: string = new fileUri.FileUri(uri).name;
    if (name.length > 0) {
      return safeTransferName(name);
    }
  } catch (e) {
    console.error('[opi] 读取选中文件名失败');
  }
  return '文件';
}

async function pickUris(ctx: common.Context, kind: TransferKind): Promise<string[]> {
  if (kind === TransferKind.Media) {
    const photoPicker: photoAccessHelper.PhotoViewPicker = new photoAccessHelper.PhotoViewPicker();
    const options: photoAccessHelper.PhotoSelectOptions = {
      MIMEType: photoAccessHelper.PhotoViewMIMETypes.IMAGE_VIDEO_TYPE,
      maxSelectNumber: MAX_TRANSFER_FILES,
    };
    const result: photoAccessHelper.PhotoSelectResult = await photoPicker.select(options);
    return result.photoUris;
  }
  const docPicker: picker.DocumentViewPicker = new picker.DocumentViewPicker(ctx);
  const options: picker.DocumentSelectOptions = new picker.DocumentSelectOptions();
  options.maxSelectNumber = kind === TransferKind.Folder ? 1 : MAX_TRANSFER_FILES;
  options.selectMode = kind === TransferKind.Folder
    ? picker.DocumentSelectMode.FOLDER : picker.DocumentSelectMode.FILE;
  return await docPicker.select(options);
}

async function copyFileUri(src: string, dest: string): Promise<void> {
  const file: fileIo.File = fileIo.openSync(src, fileIo.OpenMode.READ_ONLY);
  try {
    await fileIo.copyFile(file.fd, dest);
  } finally {
    fileIo.closeSync(file);
  }
}

async function copyDirUri(src: string, dest: string): Promise<void> {
  fileIo.mkdirSync(dest, true);
  const names: string[] = fileIo.listFileSync(src);
  for (let i = 0; i < names.length; i++) {
    const child: string = joinUri(src, names[i]);
    const target: string = `${dest}/${names[i]}`;
    if (fileIo.statSync(child).isDirectory()) {
      await copyDirUri(child, target);
    } else {
      await copyFileUri(child, target);
    }
  }
}

function scanDir(uri: string, prefix: string, entries: TransferEntry[]): void {
  const names: string[] = fileIo.listFileSync(uri);
  for (let i = 0; i < names.length; i++) {
    const child: string = joinUri(uri, names[i]);
    const relative: string = prefix.length > 0 ? `${prefix}/${names[i]}` : names[i];
    const stat: fileIo.Stat = fileIo.statSync(child);
    if (stat.isDirectory()) {
      entries.push({ relativePath: relative, size: 0, mime: '', isDir: true });
      scanDir(child, relative, entries);
    } else {
      entries.push({
        relativePath: relative, size: stat.size, mime: mimeForName(names[i]), isDir: false,
      });
    }
    if (entries.length > MAX_TRANSFER_FILES) {
      return;
    }
  }
}

/**
 * 选择 + 限额预检 + 复制到 cache staging。抛出的 Error.message 直接给用户看。
 * 调用方拿到结果后必须把 stageDir 交给 TransferSync，终态时删除。
 */
export async function prepare(ctx: common.Context, kind: TransferKind): Promise<PreparedTransfer> {
  const uris: string[] = await pickUris(ctx, kind);
  if (uris.length === 0) {
    throw new Error('没有选择任何项目');
  }
  const root: string = `${ctx.cacheDir}/transfer`;
  fileIo.mkdirSync(root, true);
  const stageDir: string = `${root}/${Date.now()}-${Math.floor(Math.random() * 1000000)}`;
  fileIo.mkdirSync(stageDir, true);
  try {
    if (kind === TransferKind.Folder) {
      const folderName: string = safeTransferName(displayName(uris[0]));
      const entries: TransferEntry[] = [];
      scanDir(uris[0], folderName, entries);
      const limitError: string = transferLimitError(entries);
      if (limitError.length > 0) {
        throw new Error(limitError);
      }
      const stagedDir: string = `${stageDir}/${folderName}`;
      await copyDirUri(uris[0], stagedDir);
      return {
        specs: [{ path: stagedDir, name: '', mime: '', isDir: true }],
        entries: entries,
        stageDir: stageDir,
      };
    }

    const entries: TransferEntry[] = [];
    const names: string[] = [];
    const used: Set<string> = new Set<string>();
    for (let i = 0; i < uris.length; i++) {
      const name: string = uniqueTransferName(displayName(uris[i]), used);
      names.push(name);
      entries.push({
        relativePath: name,
        size: fileIo.statSync(uris[i]).size,
        mime: mimeForName(name),
        isDir: false,
      });
    }
    const limitError: string = transferLimitError(entries);
    if (limitError.length > 0) {
      throw new Error(limitError);
    }
    const specs: TransferSpec[] = [];
    for (let i = 0; i < uris.length; i++) {
      const stagedPath: string = `${stageDir}/${names[i]}`;
      await copyFileUri(uris[i], stagedPath);
      specs.push({
        path: stagedPath, name: names[i], mime: mimeForName(names[i]), isDir: false,
      });
    }
    return { specs: specs, entries: entries, stageDir: stageDir };
  } catch (e) {
    try {
      removeTree(stageDir);
    } catch (ignored) {
      console.error('[opi] 清理传输缓存失败');
    }
    throw e as Error;
  }
}

/** 递归删除：staging 里可能还有子目录，rmdir 在部分实现里只删空目录。 */
function removeTree(path: string): void {
  let stat: fileIo.Stat;
  try {
    stat = fileIo.statSync(path);
  } catch (e) {
    return;
  }
  if (!stat.isDirectory()) {
    fileIo.unlinkSync(path);
    return;
  }
  const names: string[] = fileIo.listFileSync(path);
  for (let i = 0; i < names.length; i++) {
    removeTree(`${path}/${names[i]}`);
  }
  fileIo.rmdirSync(path);
}

/** 终态/取消后清 staging；目录不存在也安全。 */
export function cleanupStage(dir: string): void {
  if (dir.length === 0) {
    return;
  }
  try {
    removeTree(dir);
  } catch (e) {
    console.error('[opi] 删除传输缓存失败');
  }
}

/** 启动时清掉 24 小时前的残留 staging（进程被杀时来不及删）。 */
export function pruneStaging(ctx: common.Context): void {
  const root: string = `${ctx.cacheDir}/transfer`;
  let names: string[] = [];
  try {
    names = fileIo.listFileSync(root);
  } catch (e) {
    return;
  }
  const cutoff: number = Date.now() - 24 * 60 * 60 * 1000;
  for (let i = 0; i < names.length; i++) {
    const path: string = `${root}/${names[i]}`;
    try {
      if (fileIo.statSync(path).mtime * 1000 < cutoff) {
        removeTree(path);
      }
    } catch (e) {
      console.error('[opi] 清理残留传输缓存失败');
    }
  }
}
