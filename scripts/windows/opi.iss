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
;   ISCC.exe /DAppVersion=1.18.0 ^
;            /DAppImageDir=C:\path\to\build\compose\binaries\main\app\opi ^
;            /DOutputDir=C:\path\to\dist scripts\windows\opi.iss
;
; Silent install/uninstall (used by the Windows container smoke test):
;   opi-<version>-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART ^
;       /DIR=C:\opi-test /LOG=C:\work\install-exe.log
;   C:\opi-test\unins000.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
;
; Uninstall (2026-10-20): there is NO Windows service in this tree. Candidate,
; settings and statusbar are all opi.exe, so usUninstall kills that image before
; file removal; [UninstallDelete] removes {app}; a tsf_opi.dll still mapped in
; Explorer is scheduled for reboot deletion by uninsrestartdelete. Explorer is
; never killed (that was the white-screen root cause).

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
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\opi.exe
; 2026-10-20 white-screen fix: never let Setup close applications. tsf_opi.dll is
; loaded inside explorer.exe while a TSF client is active, so Restart Manager
; (CloseApplications=yes, the default) can close the shell during an upgrade.
; With RestartApplications=no the shell stayed closed -> blank desktop. Here Setup
; never closes anything; locked files are handled by the restartreplace flag in
; [Files] (queued for replacement on the next restart).
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
; restartreplace: on upgrade the old tsf_opi.dll may still be mapped in Explorer
; or another TSF host. Instead of closing those processes (which caused the white
; screen) Setup copies the new file and schedules the swap for the next restart.
; uninsrestartdelete: same rule on uninstall. If the DLL cannot be deleted because
; Explorer has it mapped, the uninstaller uses MoveFileEx(MOVEFILE_DELAY_UNTIL_REBOOT)
; and sets NeedRestart; Inno then shows the normal reboot prompt. A DLL that is not
; loaded is deleted immediately and no prompt appears.
Source: "{#AppImageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs restartreplace uninsrestartdelete

[UninstallDelete]
; Remove everything under {app}, including files the uninstall log did not record
; (logs, stale jars). A locked tsf_opi.dll is handled by uninsrestartdelete above;
; its parent directories are retried and, if still non-empty now, scheduled for
; deletion after reboot. Never kill Explorer to unlock the DLL.
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

[Run]
; Finish page: start the candidate window only. It must NOT touch Explorer --
; the shell restart was the 2026-10-20 white-screen root cause. TSF picks up the
; freshly registered DLL on the next sign-out/reboot (the finish page says so).
; runasoriginaluser, NOT runascurrentuser: Setup is elevated, so the current-user
; token is the admin token; a shell restarted with it is not the user's shell.
; (postinstall entries default to the original user, but state it explicitly --
; the earlier runascurrentuser here was one half of the white screen.)
; The .cmd keeps its historical name because the MSI postprocess looks it up.
Filename: "{app}\app\launch-and-restart.cmd"; Description: "Launch opi (sign out or restart Windows to load the input method)"; Flags: postinstall nowait skipifsilent runasoriginaluser

[Code]
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
  DllPath: String;
  ResultCode: Integer;
begin
  DllPath := ExpandConstant('{app}\app\tsf_opi.dll');
  if not FileExists(DllPath) then
  begin
    Log('opi: TSF DLL not found at ' + DllPath + '; skipping regsvr32');
    Exit;
  end;
  // {sys} is the 64-bit System32 because ArchitecturesInstallIn64BitMode=x64.
  // A failure here is logged and never aborts the install: Server Core and
  // locked-down machines may refuse TSF registration, but the files must land.
  if Exec(ExpandConstant('{sys}\regsvr32.exe'), '/s "' + DllPath + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('opi: regsvr32 /s tsf_opi.dll exit code ' + IntToStr(ResultCode))
  else
    Log('opi: regsvr32 could not be started; install continues');
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

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
    KillRunningApp();
  if CurStep = ssPostInstall then
    RegisterTsf();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    // Before Inno removes any file. /im covers every mode (candidate, settings,
    // statusbar); there is no Windows service to stop.
    KillRunningApp();
    UnregisterTsf();
    CleanupStatusBarState();
  end;
end;
