// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi.iss 的 [Code] 片段：卸载时清理 TSF 官方 API 不管的每用户注册表残留。
// 由 opi.iss 的 `#include "opi-code-uninstall.iss"` 文本插入（Inno Setup 6 自带
// ISPP）—— 拆文件只为守住仓库的 500 行源码门禁。
//
// ---- Uninstall residue cleanup (2026-10-21 real-machine test) ----
// DllUnregisterServer removes what the official TSF APIs own, but Windows
// keeps per-user UI state that those APIs do not remove:
//   · HKCU\Control Panel\International\User Profile\<lang>\0804:{CLSID}{PROFILE}
//     (the keyboard-list entry; without this a dead "小欧输入法" stays selectable)
//   · HKCU\Software\Microsoft\CTF\SortOrder\AssemblyItem\<lang>\<assembly>\<n>
//     (the user's input-method ordering; the CLSID value identifies ours)
// It also leaves the HKCU CTF TIP enable subtree behind. Clean them here, NOT in
// DllUnregisterServer: install/upgrade calls regsvr32 /u before copying files,
// and dropping the user's keyboard-list entry during an upgrade would be
// destructive (RegisterProfile does not re-add it to the user's list).
procedure RemoveTsfRegistryResidue();
var
  I: Integer;
  Key: String;
  Roots: array[0..3] of Integer;
  Keys: array[0..3] of String;
begin
  // 64-bit per-machine registration (the installer's regsvr32 is 64-bit).
  Roots[0] := HKEY_LOCAL_MACHINE; Keys[0] := 'SOFTWARE\Microsoft\CTF\TIP\' + CLSID_TEXT_SERVICE;
  Roots[1] := HKEY_LOCAL_MACHINE; Keys[1] := 'SOFTWARE\Classes\CLSID\' + CLSID_TEXT_SERVICE;
  // Per-user fallbacks: a non-elevated regsvr32 redirects HKCR here, and the
  // TSF enable subtree is per-user. Also covers a missing DLL: UnregisterTsf
  // exits early when the file is gone, so then this is the only cleanup.
  Roots[2] := HKEY_CURRENT_USER; Keys[2] := 'Software\Microsoft\CTF\TIP\' + CLSID_TEXT_SERVICE;
  Roots[3] := HKEY_CURRENT_USER; Keys[3] := 'Software\Classes\CLSID\' + CLSID_TEXT_SERVICE;
  for I := 0 to 3 do
  begin
    if RegKeyExists(Roots[I], Keys[I]) then
    begin
      if RegDeleteKeyIncludingSubkeys(Roots[I], Keys[I]) then
        Log('opi: removed registry residue ' + Keys[I])
      else
        Log('opi: could not remove registry residue ' + Keys[I]);
    end;
  end;
  // Only relevant if an older 32-bit build registered in the WOW64 view.
  Key := 'SOFTWARE\Classes\WOW6432Node\CLSID\' + CLSID_TEXT_SERVICE;
  if RegKeyExists(HKEY_LOCAL_MACHINE, Key) then
  begin
    if RegDeleteKeyIncludingSubkeys(HKEY_LOCAL_MACHINE, Key) then
      Log('opi: removed registry residue ' + Key)
    else
      Log('opi: could not remove registry residue ' + Key);
  end;
end;

procedure RemoveUserInputMethodTip();
var
  BaseKey: String;
  Names: TArrayOfString;
  I: Integer;
begin
  BaseKey := 'Control Panel\International\User Profile';
  if not RegGetSubkeyNames(HKEY_CURRENT_USER, BaseKey, Names) then
  begin
    Log('opi: no per-user input method list to clean');
    Exit;
  end;
  for I := 0 to GetArrayLength(Names) - 1 do
  begin
    if RegValueExists(HKEY_CURRENT_USER, BaseKey + '\' + Names[I], TIP_VALUE_NAME) then
    begin
      if RegDeleteValue(HKEY_CURRENT_USER, BaseKey + '\' + Names[I], TIP_VALUE_NAME) then
        Log('opi: removed per-user input method entry ' + Names[I])
      else
        Log('opi: could not remove per-user input method entry ' + Names[I]);
    end;
  end;
end;

procedure RemoveSortOrderEntries();
var
  Base, LangPath, AssemblyPath, ItemPath, V: String;
  Langs, Assemblies, Items: TArrayOfString;
  I, J, K: Integer;
begin
  Base := 'Software\Microsoft\CTF\SortOrder\AssemblyItem';
  if not RegGetSubkeyNames(HKEY_CURRENT_USER, Base, Langs) then
    Exit;
  for I := 0 to GetArrayLength(Langs) - 1 do
  begin
    LangPath := Base + '\' + Langs[I];
    if not RegGetSubkeyNames(HKEY_CURRENT_USER, LangPath, Assemblies) then
      Continue;
    for J := 0 to GetArrayLength(Assemblies) - 1 do
    begin
      AssemblyPath := LangPath + '\' + Assemblies[J];
      if not RegGetSubkeyNames(HKEY_CURRENT_USER, AssemblyPath, Items) then
        Continue;
      for K := 0 to GetArrayLength(Items) - 1 do
      begin
        ItemPath := AssemblyPath + '\' + Items[K];
        if RegQueryStringValue(HKEY_CURRENT_USER, ItemPath, 'CLSID', V) and
           SameText(V, CLSID_TEXT_SERVICE) then
        begin
          if RegDeleteKeyIncludingSubkeys(HKEY_CURRENT_USER, ItemPath) then
            Log('opi: removed input-method sort-order entry ' + ItemPath)
          else
            Log('opi: could not remove input-method sort-order entry ' + ItemPath);
        end;
      end;
    end;
  end;
end;

procedure PurgeUserData();
var
  Dir: String;
begin
  Dir := ExpandConstant('{localappdata}\opi');
  if not DirExists(Dir) then
    Exit;
  if DelTree(Dir, True, True, True) then
    Log('opi: removed user data ' + Dir)
  else
    Log('opi: could not remove user data ' + Dir);
end;

procedure RemoveRegistrationLog();
var
  Path: String;
begin
  // Diagnostic-only file written by DllRegisterServer on failure/degraded
  // registration. It is not user data; drop it with the install.
  Path := ExpandConstant('{commonappdata}\opi\tsf-register.log');
  if FileExists(Path) and (not DeleteFile(Path)) then
    Log('opi: could not remove registration log ' + Path);
end;
