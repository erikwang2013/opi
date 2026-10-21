// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// PendingFileRenameOperations 清理（从 opi.iss 的 [Code] 拆出守 500 行）
//
// 2026-10-21 real-machine upgrade: the previous setup left PendingFileRenameOperations
// entries for luna.opid (and its Inno temp file). Inno's PrepareToInstall then refuses
// to install until Windows is restarted — the exact "must reboot" behavior this
// installer is not allowed to have. Drop only entries that are ours (we will
// move/overwrite those files ourselves); other programs' pending renames are untouched.
//
// 2026-10-22 custom-dir fix: matching only `SetupTargetDir()` (which knows /DIR or the
// default Program Files\opi) misses interactive installs that pick a custom directory
// (e.g. D:\swoft\opi). Our pending pairs are recognized by the app-image layout
// `<install>\app\{tsf_opi.dll,luna.opid,is-*.tmp}` as well, so the cleanup no longer
// depends on guessing the install directory.

function PendingPath(const Raw: String): String;
begin
  Result := Raw;
  if Copy(Result, 1, 3) = '*1!' then
    Delete(Result, 1, 3)
  else if Copy(Result, 1, 2) = '*1' then
    Delete(Result, 1, 2);
  if Copy(Result, 1, 4) = '\??\' then
    Delete(Result, 1, 4);
end;

// app image 的布局是 <安装目录>\app\{tsf_opi.dll,luna.opid,is-*.tmp}，三个文件名
// 足够专有；别的程序的 pending 项不碰。
function IsOurAppFile(const P: String): Boolean;
var
  L: String;
begin
  L := Lowercase(P);
  Result := (Pos('\app\tsf_opi.dll', L) > 0) or (Pos('\app\luna.opid', L) > 0);
end;

function IsOurAppTemp(const P: String): Boolean;
var
  L: String;
begin
  L := Lowercase(P);
  Result := Pos('\app\is-', L) > 0;
end;

// AppPrefixRaw = SetupTargetDir() 的结果（可能为空/猜错，文件形态匹配兜底）。
procedure DropStalePendingRenames(const AppPrefixRaw: String);
var
  Key, AppPrefix, Data, Kept, Src, Dst: String;
  Parts: TArrayOfString;
  I, N: Integer;
  Dropped: Boolean;
begin
  Key := 'SYSTEM\CurrentControlSet\Control\Session Manager';
  if not RegQueryMultiStringValue(HKEY_LOCAL_MACHINE, Key,
    'PendingFileRenameOperations', Data) then
    Exit;
  AppPrefix := Lowercase(AppPrefixRaw);
  Log('opi: InitializeSetup target=' + AppPrefix);
  if AppPrefix = '' then
    Exit; // cannot tell our own directory: do not touch another program's entries
  AppPrefix := AppPrefix + '\';

  // RegQueryMultiStringValue returns one String with #0 between entries.
  N := 0;
  SetArrayLength(Parts, 0);
  while Data <> '' do
  begin
    I := Pos(#0, Data);
    SetArrayLength(Parts, N + 1);
    if I = 0 then
    begin
      Parts[N] := Data;
      Data := '';
    end
    else
    begin
      Parts[N] := Copy(Data, 1, I - 1);
      Delete(Data, 1, I);
    end;
    N := N + 1;
  end;

  Kept := '';
  Dropped := False;
  I := 0;
  while I + 1 < N do
  begin
    Src := PendingPath(Parts[I]);
    Dst := PendingPath(Parts[I + 1]);
    if (Pos(AppPrefix, Lowercase(Src)) = 1) or (Pos(AppPrefix, Lowercase(Dst)) = 1)
       or IsOurAppFile(Src) or IsOurAppFile(Dst)
       or (IsOurAppTemp(Src) and ((Dst = '') or IsOurAppFile(Dst)))
       or (IsOurAppTemp(Dst) and ((Src = '') or IsOurAppFile(Src))) then
    begin
      Log('opi: dropped stale pending rename ' + Parts[I] + ' -> ' + Parts[I + 1]);
      Dropped := True;
    end
    else
    begin
      if Kept <> '' then
        Kept := Kept + #0;
      Kept := Kept + Parts[I] + #0 + Parts[I + 1];
    end;
    I := I + 2;
  end;
  if Dropped then
  begin
    if Kept = '' then
      RegDeleteValue(HKEY_LOCAL_MACHINE, Key, 'PendingFileRenameOperations')
    else
      RegWriteMultiStringValue(HKEY_LOCAL_MACHINE, Key,
        'PendingFileRenameOperations', Kept);
  end;
end;
