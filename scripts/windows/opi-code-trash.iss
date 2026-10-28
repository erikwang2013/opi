// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi.iss 的 [Code] 片段：把被旧 TSF DLL（或候选窗进程）映射住的 tsf_opi.dll /
// luna.opid 改名移出 {app}。由 opi.iss 的 `#include "opi-code-trash.iss"` 文本插入
// （Inno Setup 6 自带 ISPP）—— 拆文件只为守住仓库的 500 行源码门禁。
//
// 2026-10-21 field report: v1.20.1 uninstall left {app} behind and Windows said
// tsf_opi.dll was in use. The old code renamed to a FIXED %TEMP% name; Inno's
// RenameFile fails if the destination exists (previous attempt's still-mapped
// copy is exactly that), and MoveFile cannot cross volumes. Both leave the DLL
// in {app}; the uninstaller then reports "Some elements could not be removed".
// The rename also runs before an upgrade copies files (ssInstall). A mapped file
// cannot be deleted, but the loader shares delete access, so renaming it within
// its volume works while Explorer holds it. There is **no restartreplace
// fallback**: a failed rename fails the install loudly (no reboot is queued).

function TSFTrashDir(const DllPath: String): String;
var
  CommonTrash: String;
begin
  Result := '';
  if ExtractFileDrive(DllPath) = '' then
    Exit;
  // Same volume as the DLL by construction; outside {app} so deletes of the
  // app directory never walk into it.
  CommonTrash := ExpandConstant('{commonappdata}\opi\trash');
  if SameText(ExtractFileDrive(CommonTrash), ExtractFileDrive(DllPath)) then
    Result := CommonTrash
  else
    Result := ExtractFileDrive(DllPath) + '\opi-trash';
  if not ForceDirectories(Result) then
  begin
    Log('opi: could not create TSF trash directory ' + Result);
    Result := '';
  end;
end;

// One file per upgrade/uninstall stays here while its holder is alive. Deleting
// it on the next run keeps the trash bounded; Windows also deletes it at boot.
// `opi-*` covers both the TSF DLL and the dictionary (luna.opid).
procedure CleanupTSFTrash(const Dir: String);
var
  FindRec: TFindRec;
begin
  if (Dir = '') or (not DirExists(Dir)) then
    Exit;
  if not FindFirst(Dir + '\opi-*', FindRec) then
    Exit;
  try
    repeat
      if not DeleteFile(Dir + '\' + FindRec.Name) then
        Log('opi: old trash file is still mapped and stays until the next restart: ' + Dir + '\' + FindRec.Name);
    until not FindNext(FindRec);
  finally
    FindClose(FindRec);
  end;
end;

function TryMoveTo(const SourcePath, Dir, NamePrefix, NameExt: String;
  var MovedTo: String): Boolean;
var
  ErrorCode: LongInt;
begin
  Result := False;
  if (Dir = '') or (not DirExists(Dir)) then
    Exit;
  if not SameText(ExtractFileDrive(Dir), ExtractFileDrive(SourcePath)) then
  begin
    Log('opi: not using ' + Dir + ' for ' + NamePrefix + ': different volume');
    Exit;
  end;
  // Milliseconds in the name: MoveFileExW without REPLACE_EXISTING fails when
  // the target exists, so two moves within one second must not collide.
  MovedTo := RemoveBackslash(Dir) + '\' + NamePrefix +
    GetDateTimeString('yyyymmddhhnnsszzz', '-', ':') + NameExt;
  Result := MoveFileExW(SourcePath, MovedTo, 0);
  if not Result then
  begin
    ErrorCode := DLLGetLastError;
    Log('opi: MoveFileExW(' + MovedTo + ') failed, error ' + IntToStr(ErrorCode));
    MovedTo := '';
  end;
end;

// Moves a file that the previous install's TSF host still maps (tsf_opi.dll or
// luna.opid) out of {app}. Both are opened with FILE_SHARE_DELETE, so a
// same-volume rename works while the image/mapping is live.
//
// **No restartreplace fallback**: Inno queues an un-replaceable file into
// PendingFileRenameOperations ("Will replace on restart"), and the next setup
// then refuses with "restart your computer to complete that installation".
// 2026-10-21 real-machine upgrade hit exactly that for luna.opid. If the move
// fails we log it; [Files] then fails the install (retry after closing the
// holder) instead of scheduling a reboot.
procedure MoveLockedFile(const FilePath, NamePrefix, NameExt, What: String;
  const ForInstall: Boolean);
var
  TrashDir, MovedTo: String;
  Moved: Boolean;
  Size64: Int64;
begin
  if not FileExists(FilePath) then
    Exit;
  // Upgrade evidence: compare with the "installed" line after the file copy.
  if FileSize64(FilePath, Size64) then
    Log('opi: ' + What + ' on disk: ' + IntToStr(Size64) + ' bytes');
  if DeleteFile(FilePath) then
    Exit; // not locked: normal path

  // Trash first (same volume by construction, outside {app}), then the old
  // fallbacks. %TEMP% may be on another drive (RAM disk), which the same-volume
  // check rejects. Setup and the uninstaller are elevated.
  TrashDir := TSFTrashDir(FilePath);
  CleanupTSFTrash(TrashDir);
  MovedTo := '';
  Moved := TryMoveTo(FilePath, TrashDir, NamePrefix, NameExt, MovedTo);
  if not Moved then
    Moved := TryMoveTo(FilePath, GetEnv('TEMP'), NamePrefix, NameExt, MovedTo);
  if not Moved then
    Moved := TryMoveTo(FilePath, ExpandConstant('{win}\Temp'), NamePrefix, NameExt, MovedTo);

  if not FileExists(FilePath) then
  begin
    Log('opi: moved locked ' + What + ' to ' + MovedTo);
    if MoveFileExW(MovedTo, '', MOVEFILE_DELAY_UNTIL_REBOOT) then
      Log('opi: trash ' + What + ' deletion scheduled for next boot: ' + MovedTo)
    else
      Log('opi: could not schedule trash ' + What + ' deletion, error ' + IntToStr(DLLGetLastError) + '; it stays until the next run');
  end
  else if ForInstall then
    Log('opi: ' + What + ' is locked and could not be moved; the install will fail rather than queue a restart')
  else
  begin
    // Last resort: the file stays in {app} for this session. Never claim the
    // directory was cleaned; Inno will report the remaining element.
    if MoveFileExW(FilePath, '', MOVEFILE_DELAY_UNTIL_REBOOT) then
      Log('opi: ' + What + ' is locked and could not be moved; deletion scheduled for the next restart, {app} remains until then: ' + FilePath)
    else
      Log('opi: ' + What + ' is locked and could not be moved or scheduled, error ' + IntToStr(DLLGetLastError));
  end;
end;

procedure MoveLockedTsfDll(const ForInstall: Boolean);
begin
  MoveLockedFile(ExpandConstant('{app}\app\tsf_opi.dll'), 'opi-tsf-', '.dll',
    'TSF DLL', ForInstall);
end;

procedure MoveLockedDictionary(const ForInstall: Boolean);
begin
  MoveLockedFile(ExpandConstant('{app}\app\luna.opid'), 'opi-luna-', '.opid',
    'dictionary', ForInstall);
  // 繁体词库（2026-10-28）：新 TSF DLL 也会 mmap 它，升级时必须先挪开，
  // 否则 [Files] 会因文件被占用排队重启（与 luna 同一个坑）。
  MoveLockedFile(ExpandConstant('{app}\app\trad.opid'), 'opi-trad-', '.opid',
    'traditional dictionary', ForInstall);
end;

// Kept as a named call: .github/workflows/ci.yml asserts it appears inside
// CurUninstallStepChanged (the uninstaller must not kill Explorer instead).
procedure MoveLockedTsfDllOut();
begin
  MoveLockedTsfDll(False);
end;
