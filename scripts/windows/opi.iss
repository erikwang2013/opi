; SPDX-FileCopyrightText: 2026 erik.xyz
; SPDX-License-Identifier: MIT
;
; User-facing Windows installer for opi (2026-10-20).
;
; Input is the jpackage app image produced by `./gradlew.bat createDistributable`
; with OPI_TSF_DLL set. Layout on Windows:
;   <app-image>\opi.exe              launcher (candidate window / --settings)
;   <app-image>\app\tsf_opi.dll      TSF COM server (registered below)
;   <app-image>\app\luna.opid        dictionary, loaded from the DLL's directory
;   <app-image>\app\*.jar, opi.exe's jars, runtime\ (jlinked JRE)
;
; Why Inno Setup instead of jpackage's own MSI: jpackage has no knob to run
; regsvr32, and post-processing the MSI made TSF registration fatal (a failed
; regsvr32 rolled the whole install back on Windows Server Core). Here [Code]
; runs regsvr32 after files land and only logs the exit code: registration is
; best-effort and can never roll back the install.
;
; Build (CI or local, on Windows; ISCC is the Inno Setup 6 compiler):
;   ISCC.exe /DAppVersion=1.20.2 ^
;            /DAppImageDir=C:\path\to\build\compose\binaries\main\app\opi ^
;            /DOutputDir=C:\path\to\dist scripts\windows\opi.iss
;
; Silent install/uninstall (used by the Windows container smoke test):
;   opi-<version>-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART ^
;       /DIR=C:\opi-test /LOG=C:\work\install-exe.log
;   C:\opi-test\unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
;
; Uninstall (2026-10-21): there is NO Windows service in this tree. Candidate,
; settings and statusbar are all opi.exe, so usUninstall kills that image before
; file removal; [UninstallDelete] removes {app}; a tsf_opi.dll still mapped in
; Explorer is renamed out of {app} to a UNIQUE same-volume trash directory
; ({commonappdata}\opi\trash, or <drive>:\opi-trash when the app is on another
; drive) and Windows is asked to delete that copy at the next boot. The old
; fixed %TEMP% name failed on retry: the previous still-mapped copy blocked
; RenameFile. If even the rename fails, deletion is scheduled for the next
; restart and the log says so. No reboot is requested. Explorer is never killed
; (that was the white-screen root cause).
;
; Upgrade (2026-10-21): the same rename runs at ssInstall, BEFORE [InstallDelete]
; and [Files]. Without it, a DLL mapped in Explorer makes Inno's restartreplace
; leave the OLD file at {app}\app\tsf_opi.dll and queue the new one for the next
; restart -- so installing v1.20.1 over v1.20.0 kept running the old TSF code.
; restartreplace stays as the fallback for when the rename itself fails.

#ifndef AppVersion
  #error AppVersion is required: /DAppVersion=<x.y.z>
#endif
#ifndef AppImageDir
  #error AppImageDir is required: /DAppImageDir=<jpackage app-image directory>
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

#define AppName "opi"
#define AppPublisher "opi"
#define AppURL "https://opi.erik.xyz"

[Setup]
; Stable AppId: must never change, or upgrades install side by side.
AppId={{7E4C2B1A-6D3F-4A9C-8B57-1E2F3A4C5D60}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
DefaultDirName={autopf}\opi
DefaultGroupName=opi
DisableProgramGroupPage=yes
DisableWelcomePage=yes
PrivilegesRequired=admin
; x64 only (the TSF DLL and jlinked runtime are x86_64).
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=opi-{#AppVersion}-windows-x64-setup
; Installer/uninstaller icon = project pet (same art as opi.exe). A relative
; directive path resolves against this script's directory, hence two levels up.
SetupIconFile=..\..\desktop\icons\opi-pet.ico
; 安装向导侧栏/左上角 = 项目宠物（2026-10-21 用户裁决「安装过程中界面显示项目宠物图」）。
; Inno Setup 6 支持 PNG；240x459 是 164:314 的 1.46 倍，高分屏不糊。
WizardImageFile=..\..\desktop\icons\opi-pet-wizard.png
WizardSmallImageFile=..\..\desktop\icons\opi-pet-wizard-small.png
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\opi.exe
; 2026-10-20 white-screen fix: never let Setup close applications. tsf_opi.dll is
; loaded inside explorer.exe while a TSF client is active, so Restart Manager
; (CloseApplications=yes, the default) can close the shell during an upgrade.
; With RestartApplications=no the shell stayed closed -> blank desktop. Here Setup
; never closes anything; a locked tsf_opi.dll is renamed out at ssInstall (see
; MoveLockedTsfDll) and [Files] restartreplace remains only as the fallback.
CloseApplications=no
; Inert while CloseApplications=no; kept at yes as a second belt so re-enabling
; Restart Manager later cannot recreate the white screen.
RestartApplications=yes
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; Drop the previous app image before laying down the new one; stale jars or
; runtime files from an older version otherwise survive an upgrade.
Type: filesandordirs; Name: "{app}\app"
Type: filesandordirs; Name: "{app}\runtime"

[Files]
; restartreplace is the FALLBACK, not the normal upgrade path: at ssInstall
; MoveLockedTsfDll(True) renames a mapped old DLL to a same-volume trash
; directory, so this entry simply writes the new file. restartreplace only
; matters when that rename failed; Inno then queues the new file for the next
; restart and logs "Will replace on restart".
; NO uninsrestartdelete (2026-10-21): that flag schedules a reboot when Explorer
; still maps tsf_opi.dll, and the user saw an uninstall-triggered reboot. On
; uninstall we instead move a locked DLL out of {app} (see MoveLockedTsfDllOut),
; so {app} can be deleted immediately and no restart is requested.
Source: "{#AppImageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace

[UninstallDelete]
; Remove everything under {app}, including files the uninstall log did not record
; (logs, stale jars). A locked tsf_opi.dll is moved out of {app} before this runs
; (MoveLockedTsfDllOut); the directory is deleted now, without a reboot. If even
; the rename fails, the DLL stays and deletion is scheduled for next boot (the
; log says so). Never kill Explorer to unlock the DLL.
Type: filesandordirs; Name: "{app}"

[Icons]
Name: "{autoprograms}\opi settings"; Filename: "{app}\opi.exe"; Parameters: "--settings"; WorkingDir: "{app}"
Name: "{autodesktop}\opi settings"; Filename: "{app}\opi.exe"; Parameters: "--settings"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; The TSF text service only inserts text; the candidate window is a separate
; process. Without a login entry, a reboot leaves the input method with no
; candidate UI. HKLM64 Run starts it (hidden until a show message arrives) for
; every user; uninstall removes the value. ArchitecturesInstallIn64BitMode=x64
; already makes plain HKLM use the native view; keep HKLM64 explicit so this
; entry cannot silently fall into Wow6432Node if the install mode changes.
Root: HKLM64; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "opi"; ValueData: """{app}\opi.exe"""; Flags: uninsdeletevalue
; Statusbar is a second process of the same opi.exe; the candidate process hosts the
; control-plane broker it needs. Both Run values are required.
Root: HKLM64; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "opi-statusbar"; ValueData: """{app}\opi.exe"" --statusbar"; Flags: uninsdeletevalue

[Run]
; Finish page: start the candidate window only. It must NOT touch Explorer --
; the shell restart was the 2026-10-20 white-screen root cause. TSF picks up the
; freshly registered DLL on the next sign-out/reboot, and the user must then add
; 小欧输入法 under the Chinese (Simplified) keyboard list -- the description says
; both, because "installed but still English" was the 2026-10-21 field report.
; runasoriginaluser, NOT runascurrentuser: Setup is elevated, so the current-user
; token is the admin token; a shell restarted with it is not the user's shell.
; (postinstall entries default to the original user, but state it explicitly --
; the earlier runascurrentuser here was one half of the white screen.)
; The .cmd keeps its historical name because the MSI postprocess looks it up.
Filename: "{app}\app\launch-and-restart.cmd"; Description: "Launch opi (then sign out or restart Windows, and add/switch to 小欧输入法 in the Chinese (Simplified) keyboard list)"; Flags: postinstall nowait skipifsilent runasoriginaluser

[Code]
const
  // MoveFileExW flag: perform the operation at next boot (session manager).
  MOVEFILE_DELAY_UNTIL_REBOOT = 4;

// Used instead of RenameFile so a failure can be logged with its Win32 error.
// Same-volume only: MoveFile cannot cross volumes, and a cross-volume move
// would have to delete the mapped source, which Windows refuses.
function MoveFileExW(ExistingName, NewName: String; Flags: Cardinal): Boolean;
  external 'MoveFileExW@kernel32.dll stdcall';

procedure KillRunningApp();
var
  ResultCode: Integer;
begin
  // Upgrade: the old candidate window keeps opi.exe files locked. Best-effort —
  // a machine with no opi.exe running returns non-zero and that is fine.
  if Exec(ExpandConstant('{sys}\taskkill.exe'), '/f /t /im opi.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('opi: taskkill opi.exe exit code ' + IntToStr(ResultCode))
  else
    Log('opi: taskkill could not be started; install continues');
end;

procedure RegisterTsf();
var
  DllPath, ProfileKey: String;
  ResultCode: Integer;
  Size64: Int64;
begin
  DllPath := ExpandConstant('{app}\app\tsf_opi.dll');
  if not FileExists(DllPath) then
  begin
    Log('opi: TSF DLL not found at ' + DllPath + '; skipping regsvr32');
    Exit;
  end;
  // Evidence for the upgrade path: compare with the "on disk" size logged at
  // ssInstall. Equal sizes do not prove the old file was replaced (the CI hash
  // check does); a difference proves the new one landed before any reboot.
  if FileSize64(DllPath, Size64) then
    Log('opi: tsf_opi.dll installed: ' + IntToStr(Size64) + ' bytes');
  // {sys} is the 64-bit System32 because ArchitecturesInstallIn64BitMode=x64.
  // A failure here is logged and never aborts the install: Server Core and
  // locked-down machines may refuse TSF registration, but the files must land.
  if not Exec(ExpandConstant('{sys}\regsvr32.exe'), '/s "' + DllPath + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    Log('opi: regsvr32 could not be started; install continues');
    Exit;
  end;
  Log('opi: regsvr32 /s tsf_opi.dll exit code ' + IntToStr(ResultCode));

  // Exit code 0 is not the end-to-end criterion: the CTF language-profile key is
  // what makes the IME show up in the keyboard list. Both failures must be
  // visible to an interactive user -- the 2026-10-21 report was "installed, still
  // English only, no status bar", with nothing on screen saying why.
  ProfileKey := 'SOFTWARE\Microsoft\CTF\TIP\{98C51858-590B-404D-8915-21EB44250062}\LanguageProfile\0x00000804\{A31A137C-1404-42F2-B82C-4E63F15DEFD6}';
  if (ResultCode = 0) and RegKeyExists(HKLM64, ProfileKey) then
    Log('opi: TSF language profile present under HKLM\...\LanguageProfile\0x00000804')
  else
  begin
    if ResultCode = 0 then
      Log('opi: regsvr32 succeeded but the CTF profile key is missing: ' + ProfileKey)
    else
      Log('opi: TSF registration FAILED; details in %ProgramData%\opi\tsf-register.log');
    if not WizardSilent then
      MsgBox('小欧输入法注册失败（详见 %ProgramData%\opi\tsf-register.log）。' + #13#10 +
        '安装文件已就位，但输入法不会出现在键盘列表里。' + #13#10 + #13#10 +
        '请重启 Windows 后再运行一次安装程序；若仍失败，请把该日志发给开发者。',
        mbError, MB_OK);
  end;
end;

procedure UnregisterTsf();
var
  DllPath: String;
  ResultCode: Integer;
begin
  DllPath := ExpandConstant('{app}\app\tsf_opi.dll');
  if not FileExists(DllPath) then
  begin
    Log('opi: TSF DLL not found at ' + DllPath + '; skipping regsvr32 /u');
    Exit;
  end;
  if Exec(ExpandConstant('{sys}\regsvr32.exe'), '/s /u "' + DllPath + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('opi: regsvr32 /u tsf_opi.dll exit code ' + IntToStr(ResultCode))
  else
    Log('opi: regsvr32 /u could not be started; uninstall continues');
end;

procedure CleanupStatusBarState();
var
  RunKey, LockBase, LockPath: String;
begin
  // This tree has NO Windows service: candidate, settings and statusbar are all
  // opi.exe, killed by KillRunningApp above. Delete Run values defensively, so a
  // future opi-statusbar autostart cannot survive an uninstall either. Plain
  // HKEY_LOCAL_MACHINE is the native view in 64-bit install mode.
  RunKey := 'Software\Microsoft\Windows\CurrentVersion\Run';
  RegDeleteValue(HKEY_LOCAL_MACHINE, RunKey, 'opi');
  RegDeleteValue(HKEY_LOCAL_MACHINE, RunKey, 'opi-statusbar');
  RegDeleteValue(HKEY_CURRENT_USER, RunKey, 'opi');
  RegDeleteValue(HKEY_CURRENT_USER, RunKey, 'opi-statusbar');

  // StatusBarLock.kt puts its single-instance lock in XDG_RUNTIME_DIR or %TEMP%;
  // killing the process releases the OS lock but leaves the file behind.
  LockBase := GetEnv('XDG_RUNTIME_DIR');
  if LockBase = '' then
    LockBase := GetEnv('TEMP');
  LockPath := LockBase + '\opi-statusbar-' + GetUserNameString() + '.lock';
  if FileExists(LockPath) and (not DeleteFile(LockPath)) then
    Log('opi: could not remove statusbar lock ' + LockPath);
end;

// 2026-10-21 field report: v1.20.1 uninstall left {app} behind and Windows said
// tsf_opi.dll was in use. The old code renamed to a FIXED %TEMP% name; Inno's
// RenameFile fails if the destination exists (previous attempt's still-mapped
// copy is exactly that), and MoveFile cannot cross volumes. Both leave the DLL
// in {app}; the uninstaller then reports "Some elements could not be removed".
// The rename must also run before an upgrade copies files (ssInstall): with a
// mapped DLL, [Files] restartreplace leaves the OLD file in place until the
// next reboot. A mapped image cannot be deleted, but the loader shares delete
// access, so renaming it within its volume works while Explorer holds it.
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

// One DLL per upgrade/uninstall stays here while its holder is alive. Deleting
// it on the next run keeps the trash bounded; Windows also deletes it at boot.
procedure CleanupTSFTrash(const Dir: String);
var
  FindRec: TFindRec;
begin
  if (Dir = '') or (not DirExists(Dir)) then
    Exit;
  if not FindFirst(Dir + '\opi-tsf-*.dll', FindRec) then
    Exit;
  try
    repeat
      if not DeleteFile(Dir + '\' + FindRec.Name) then
        Log('opi: old trash DLL is still mapped and stays until the next restart: ' + Dir + '\' + FindRec.Name);
    until not FindNext(FindRec);
  finally
    FindClose(FindRec);
  end;
end;

function TryMoveDllTo(const DllPath, Dir: String; var MovedTo: String): Boolean;
var
  ErrorCode: LongInt;
begin
  Result := False;
  if (Dir = '') or (not DirExists(Dir)) then
    Exit;
  if not SameText(ExtractFileDrive(Dir), ExtractFileDrive(DllPath)) then
  begin
    Log('opi: not using ' + Dir + ' for the TSF DLL: different volume');
    Exit;
  end;
  // Milliseconds in the name: MoveFileExW without REPLACE_EXISTING fails when
  // the target exists, so two moves within one second must not collide.
  MovedTo := RemoveBackslash(Dir) + '\opi-tsf-' +
    GetDateTimeString('yyyymmddhhnnsszzz', '-', ':') + '.dll';
  Result := MoveFileExW(DllPath, MovedTo, 0);
  if not Result then
  begin
    ErrorCode := DLLGetLastError;
    Log('opi: MoveFileExW(' + MovedTo + ') failed, error ' + IntToStr(ErrorCode));
    MovedTo := '';
  end;
end;

procedure MoveLockedTsfDll(const ForInstall: Boolean);
var
  DllPath, TrashDir, MovedTo: String;
  Moved: Boolean;
  Size64: Int64;
begin
  DllPath := ExpandConstant('{app}\app\tsf_opi.dll');
  if not FileExists(DllPath) then
    Exit;
  // Upgrade evidence: compare this size with the "installed" line from
  // RegisterTsf after the file copy. Equal does not prove replacement (the CI
  // hash check does); different proves the new DLL landed before any reboot.
  if FileSize64(DllPath, Size64) then
    Log('opi: tsf_opi.dll on disk: ' + IntToStr(Size64) + ' bytes');
  if DeleteFile(DllPath) then
    Exit; // not locked: normal path

  // Trash first (same volume by construction, outside {app}), then the old
  // fallbacks. %TEMP% may be on another drive (RAM disk), which the same-volume
  // check rejects. Setup and the uninstaller are elevated.
  TrashDir := TSFTrashDir(DllPath);
  CleanupTSFTrash(TrashDir);
  MovedTo := '';
  Moved := TryMoveDllTo(DllPath, TrashDir, MovedTo);
  if not Moved then
    Moved := TryMoveDllTo(DllPath, GetEnv('TEMP'), MovedTo);
  if not Moved then
    Moved := TryMoveDllTo(DllPath, ExpandConstant('{win}\Temp'), MovedTo);

  if not FileExists(DllPath) then
  begin
    Log('opi: moved locked TSF DLL to ' + MovedTo);
    // Still mapped under the new name. Ask Windows to delete it at next boot
    // so the trash does not accumulate one DLL per upgrade/uninstall.
    if MoveFileExW(MovedTo, '', MOVEFILE_DELAY_UNTIL_REBOOT) then
      Log('opi: trash DLL deletion scheduled for next boot: ' + MovedTo)
    else
      Log('opi: could not schedule trash DLL deletion, error ' + IntToStr(DLLGetLastError) + '; it stays until the next run');
  end
  else if ForInstall then
    // Do NOT also schedule the old file for deletion: [Files] restartreplace
    // owns this path in the fallback case (two pending renames would race).
    Log('opi: TSF DLL is locked and could not be moved; [Files] restartreplace puts the new DLL in place at the next restart')
  else
  begin
    // Last resort: the file stays in {app} for this session. Never claim the
    // directory was cleaned; Inno will report the remaining element.
    if MoveFileExW(DllPath, '', MOVEFILE_DELAY_UNTIL_REBOOT) then
      Log('opi: TSF DLL is locked and could not be moved; deletion scheduled for the next restart, {app} remains until then: ' + DllPath)
    else
      Log('opi: TSF DLL is locked and could not be moved or scheduled, error ' + IntToStr(DLLGetLastError));
  end;
end;

// Kept as a named call: .github/workflows/ci.yml asserts it appears inside
// CurUninstallStepChanged (the uninstaller must not kill Explorer instead).
procedure MoveLockedTsfDllOut();
begin
  MoveLockedTsfDll(False);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
  begin
    // ssInstall runs before [InstallDelete] and [Files]. Moving a mapped old DLL
    // out now makes room for the new file immediately, instead of letting
    // restartreplace defer the swap to the next reboot (the 2026-10-21 field
    // report: installing v1.20.1 over v1.20.0 still ran the old TSF code).
    KillRunningApp();
    // 先摘旧 profile/COM 键，再动文件；新 DLL 落下后由 ssPostInstall 重建。
    UnregisterTsf();
    MoveLockedTsfDll(True);
  end;
  if CurStep = ssPostInstall then
  begin
    // RegisterProfile 不会更新已存在 profile 的图标；ssInstall 已摘掉旧 profile，
    // 这里再 /u 一次再注册，保证 profile 由新 DLL 以新图标（opi.exe 宠物）重建。
    UnregisterTsf();
    RegisterTsf();
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    // Before Inno removes any file. /im covers every mode (candidate, settings,
    // statusbar); there is no Windows service to stop.
    KillRunningApp();
    UnregisterTsf();
    MoveLockedTsfDllOut();
    CleanupStatusBarState();
  end;
end;
