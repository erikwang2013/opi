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
;   <app-image>\app\trad.opid        traditional dictionary (optional, same directory)
;   <app-image>\app\*.jar, opi.exe's jars, runtime\ (jlinked JRE)
;
; Why Inno Setup instead of jpackage's own MSI: jpackage has no knob to run
; regsvr32, and post-processing the MSI made TSF registration fatal (a failed
; regsvr32 rolled the whole install back on Windows Server Core). Here [Code]
; runs regsvr32 after files land and only logs the exit code: registration is
; best-effort and can never roll back the install.
;
; Build (CI or local, on Windows; ISCC is the Inno Setup 6 compiler):
;   ISCC.exe /DAppVersion=1.28.3 ^
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
; restart and the log says so. No reboot is requested. The uninstaller never
; kills Explorer. An interactive UPGRADE restarts only the original user's shell
; when Explorer actually has the old DLL mapped (with a watchdog); 2026-10-23
; white-screen regression: never kill a healthy shell just because it is an
; upgrade. The finish page says "no sign-out; new apps use the update,
; already-open apps need a close and reopen". First installs and silent installs
; never touch the shell. Other TSF hosts are never killed: the installer logs
; (and lists on the finish page) the processes still mapping the old DLL.
;
; Upgrade (2026-10-21): the same rename runs at ssInstall, BEFORE [InstallDelete]
; and [Files], for tsf_opi.dll AND the app\*.opid dictionaries (the old TSF DLL maps the
; dictionary too). Without it, Inno's restartreplace leaves the OLD file at
; {app} and queues the new one for the next restart -- so installing v1.20.1
; over v1.20.0 kept running the old TSF code, and the queued rename made the
; NEXT setup refuse until a reboot. restartreplace is gone: the rename must
; succeed now (the install fails loudly if it cannot) and nothing is ever
; queued for a reboot.

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
; 10.0.14393 = Windows 10 1607：DLL 依赖 GetDpiForWindow，1507/1511 会在加载阶段失败。
MinVersion=10.0.14393
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
; never closes anything; locked files (tsf_opi.dll, luna.opid, trad.opid) are renamed out at
; ssInstall (see MoveLockedTsfDll / MoveLockedDictionary), so [Files] never needs
; restartreplace.
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
; 2026-11-01: app\opi_ffi.dll (LAN clipboard core) ships inside the jpackage app
; image and is installed by the recursesubdirs entry below. First listen triggers
; the Windows Firewall prompt; this installer intentionally adds NO firewall rule.
; 2026-10-21 real-machine upgrade: tsf_opi.dll AND app\*.opid are held by
; the old TSF DLL inside Explorer/other hosts. ssInstall moves both out to a
; same-volume trash directory first, so this entry simply writes the new files.
; **No restartreplace / uninsrestartdelete**: restartreplace queues the locked
; file into PendingFileRenameOperations ("Will replace on restart"), and the
; NEXT setup then refuses with "restart your computer to complete that
; installation" (PrepareToInstall checks that value). That is what the user
; hit. If a file still cannot be moved, the install fails loudly; close the
; holding app and retry — never schedule a reboot. MoveLockedFile(True) handles
; the DLL and the dictionary before this line.
Source: "{#AppImageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

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
; the shell restart lives in RestartExplorerAfterInstall (opi-code-explorer.iss)
; and runs with the original user token plus a watchdog. The user must then add
; 小欧输入法 under the Chinese (Simplified) keyboard list if it is not already
; there -- "installed but still English" was the 2026-10-21 field report.
; runasoriginaluser, NOT runascurrentuser: Setup is elevated, so the current-user
; token is the admin token; a shell restarted with it is not the user's shell.
; (postinstall entries default to the original user, but state it explicitly --
; the earlier runascurrentuser here was one half of the white screen.)
; The .cmd keeps its historical name because the MSI postprocess looks it up.
; 完成页文案（2026-10-23 用户口径）：安装完成不要求注销；新打开的应用生效；
; 已打开的应用关闭重开。静默安装不重启 Explorer。
Filename: "{app}\app\launch-and-restart.cmd"; Description: "打开 opi（无需注销；新打开的应用生效，已打开的应用请关闭重开）"; Flags: postinstall nowait skipifsilent runasoriginaluser

[Code]
const
  // MoveFileExW flag: perform the operation at next boot (session manager).
  MOVEFILE_DELAY_UNTIL_REBOOT = 4;
  // TSF identity. Keep these in sync with crates/tsf-opi/src/registration.rs
  // (CLSID_TEXT_SERVICE / GUID_PROFILE).
  CLSID_TEXT_SERVICE = '{98C51858-590B-404D-8915-21EB44250062}';
  GUID_PROFILE = '{A31A137C-1404-42F2-B82C-4E63F15DEFD6}';
  // Per-user keyboard-list value: "0804:{CLSID}{PROFILE}" (a DWORD under
  // HKCU\Control Panel\International\User Profile\<language tag>).
  TIP_VALUE_NAME = '0804:' + CLSID_TEXT_SERVICE + GUID_PROFILE;

var
  // Set by /DELETEUSERDATA. Uninstall keeps %LOCALAPPDATA%\opi by default
  // (settings + imported dictionaries are user data, not install residue).
  DeleteUserData: Boolean;

function CmdLineParamExists(const Name: String): Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
    if SameText(ParamStr(I), Name) then
    begin
      Result := True;
      Exit;
    end;
end;

function InitializeUninstall(): Boolean;
begin
  DeleteUserData := CmdLineParamExists('/DELETEUSERDATA');
  Result := True;
end;

// PendingFileRenameOperations 清理（含自定义目录识别）在 include 里：同为安装代码，
// 拆出守 500 行；Explorer/进程管理静态门禁也把它列进扫描清单。
#include "opi-code-pending.iss"

// Target directory available before {app} is initialized (InitializeSetup):
// honor /DIR=..., otherwise use the same default as DefaultDirName.
function SetupTargetDir(): String;
var
  I: Integer;
  P, Low: String;
begin
  Result := '';
  for I := 1 to ParamCount do
  begin
    P := ParamStr(I);
    Low := Lowercase(P);
    if Copy(Low, 1, 5) = '/dir=' then
    begin
      Result := Copy(P, 6, Length(P) - 5);
      // ParamStr 可能保留包裹的引号（/DIR="C:\Program Files\opi"）。只剥**成对**的
      // 引号；路径本身含引号是不可能的，落单的引号交给后面的前缀匹配自然失败。
      if (Length(Result) >= 2) and (Result[1] = '"') and
         (Result[Length(Result)] = '"') then
        Result := Copy(Result, 2, Length(Result) - 2);
      Exit;
    end;
  end;
  Result := GetEnv('ProgramW6432');
  if Result = '' then
    Result := GetEnv('ProgramFiles');
  Result := Result + '\opi';
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
  // 交互式安装到自定义目录时，此刻还看不到向导里选的路径（SetupTargetDir 只认
  // /DIR 或默认 Program Files\opi）；include 里的清理同时按应用文件名匹配兜底。
  DropStalePendingRenames(SetupTargetDir());
end;

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

  // StatusBarLock.kt / CandidateLock.kt put their single-instance locks in
  // XDG_RUNTIME_DIR or %TEMP%; killing the process releases the OS lock but
  // leaves the file behind.
  LockBase := GetEnv('XDG_RUNTIME_DIR');
  if LockBase = '' then
    LockBase := GetEnv('TEMP');
  LockPath := LockBase + '\opi-statusbar-' + GetUserNameString() + '.lock';
  if FileExists(LockPath) and (not DeleteFile(LockPath)) then
    Log('opi: could not remove statusbar lock ' + LockPath);
  LockPath := LockBase + '\opi-candidate-' + GetUserNameString() + '.lock';
  if FileExists(LockPath) and (not DeleteFile(LockPath)) then
    Log('opi: could not remove candidate lock ' + LockPath);
end;

#include "opi-code-trash.iss"
#include "opi-code-explorer.iss"
#include "opi-code-uninstall.iss"

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
  begin
    // ssInstall runs before [InstallDelete] and [Files]. Moving a mapped old DLL
    // out now makes room for the new file immediately. (The old restartreplace
    // path deferred the swap to the next reboot; 2026-10-21 field report:
    // installing v1.20.1 over v1.20.0 still ran the old TSF code, and the queued
    // rename made the next setup refuse until a reboot.)
    KillRunningApp();
    // Detect the old in-proc DLL while it is still mapped. Only Explorer that
    // actually loaded the old DLL is restarted (2026-10-23 white-screen regression:
    // killing Explorer on an upgrade that did not map it can leave a blank desktop).
    ExplorerHadTsf := ExplorerMapsTsf();
    // 先摘旧 profile/COM 键，再动文件；新 DLL 落下后由 ssPostInstall 重建。
    UnregisterTsf();
    MoveLockedTsfDll(True);
    // The old TSF DLL maps luna.opid for its lifetime; move it too or [Files]
    // queues a restart.
    MoveLockedDictionary(True);
  end;
  if CurStep = ssPostInstall then
  begin
    // RegisterProfile 不会更新已存在 profile 的图标；ssInstall 已摘掉旧 profile，
    // 这里再 /u 一次再注册，保证 profile 由新 DLL 以新图标（opi.exe 宠物）重建。
    UnregisterTsf();
    RegisterTsf();
    // Interactive upgrade: the shell may still have the old DLL mapped; restart
    // it (original-user token + watchdog) so the new TSF code is live without a
    // sign-out. First installs and silent installs never touch the shell.
    RestartExplorerAfterInstall();
    // Finish page lists the hosts we deliberately did NOT kill (close and reopen).
    RecordTsfHolders();
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  TrashDir: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    // Before Inno removes any file. /im covers every mode (candidate, settings,
    // statusbar); there is no Windows service to stop.
    KillRunningApp();
    UnregisterTsf();
    // Fallback for what the TSF APIs / regsvr32 do not remove (and for the
    // DLL-missing boundary, where UnregisterTsf returns without running).
    RemoveTsfRegistryResidue();
    RemoveUserInputMethodTip();
    RemoveSortOrderEntries();
    RemoveRegistrationLog();
    MoveLockedTsfDllOut();
    MoveLockedDictionary(False);
    CleanupStatusBarState();
    // Off by default: %LOCALAPPDATA%\opi is user data (settings + imported
    // dictionaries), not install residue. /DELETEUSERDATA makes uninstall a
    // full purge for scripted/clean-machine use.
    if DeleteUserData then
      PurgeUserData();
  end;
  if CurUninstallStep = usPostUninstall then
  begin
    // The moved DLL may have become deletable while the uninstaller ran (the
    // holder can exit during uninstall). Try once more, then drop the empty
    // trash directory. A still-mapped copy stays until the next boot.
    TrashDir := TSFTrashDir(ExpandConstant('{app}\app\tsf_opi.dll'));
    CleanupTSFTrash(TrashDir);
    if (TrashDir <> '') and DirExists(TrashDir) then
      if RemoveDir(TrashDir) then
        Log('opi: removed empty TSF trash directory ' + TrashDir)
      else
        Log('opi: TSF trash directory is not empty (locked DLL stays until the next restart): ' + TrashDir);
    // Drop {commonappdata}\opi too when nothing is left in it (log removed,
    // trash empty). Silently does nothing while a locked DLL still lives there.
    if RemoveDir(ExpandConstant('{commonappdata}\opi')) then
      Log('opi: removed empty ProgramData\opi directory');
  end;
end;
