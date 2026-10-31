; SPDX-FileCopyrightText: 2026 erik.xyz
; SPDX-License-Identifier: MIT
;
; Inno Setup skeleton for the Windows 7/8 legacy client (Phase 1).
;
; NOT SIGNED. This script has no SignTool section on purpose: the repository has
; no code-signing certificate, so SmartScreen will warn. Do not add a signing
; step here until the signing process is decided.
;
; Architecture model: one installer, native payload per OS. On 64-bit Windows
; the x64 DLL/EXE is installed in 64-bit mode (Program Files); on 32-bit
; Windows the x86 payload is installed. There is no WoW64 cross-install.
;
; Manual register/unregister (elevated cmd):
;   regsvr32 /s "%ProgramFiles%\opi-win7\tsf_opi.dll"
;   regsvr32 /s /u "%ProgramFiles%\opi-win7\tsf_opi.dll"
;
; Build inputs (override with ISCC /DBuildRoot=... in CI):
#ifndef BuildRoot
  #define BuildRoot "..\..\..\target"
#endif

[Setup]
AppId={{6E2F4C1A-7B3D-4A55-9C8E-1D0B2A6F7C11}
AppName=opi (Windows 7/8)
AppVersion=0.1.0
AppPublisher=erik.xyz
DefaultDirName={autopf}\opi-win7
DefaultGroupName=opi-win7
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=opi-win7-phase1-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Windows 7 SP1 is the floor (TSF profile APIs + the Rust std Win7 target).
MinVersion=6.1sp1
ArchitecturesAllowed=x86compatible
ArchitecturesInstallIn64BitMode=x64compatible
; TSF profile registration writes HKLM.
PrivilegesRequired=admin
; Do not terminate a running candidate window during an upgrade.
CloseApplications=no
UninstallDisplayName=opi (Windows 7/8)
; SignTool intentionally omitted (unsigned Phase 1 build).

[Files]
Source: "{#BuildRoot}\x86_64-win7-windows-msvc\release\tsf_opi.dll"; DestDir: "{app}"; Flags: ignoreversion; Check: IsWin64
Source: "{#BuildRoot}\x86_64-win7-windows-msvc\release\opi-win7-ui.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: IsWin64
Source: "{#BuildRoot}\i686-win7-windows-msvc\release\tsf_opi.dll"; DestDir: "{app}"; Flags: ignoreversion; Check: not IsWin64
Source: "{#BuildRoot}\i686-win7-windows-msvc\release\opi-win7-ui.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: not IsWin64
; Phase 2 TODO: ship data/generated/*.opid next to the DLL. The Phase 1 skeleton
; only uses the 35-word fallback dictionary compiled into the binary.

[Run]
; {sys} is the OS-native System32 in 64-bit install mode and SysWOW64 otherwise,
; matching the payload bitness. A registration failure shows a warning but does
; not roll back the installed files (same policy as the Windows 10 installer).
Filename: "{sys}\regsvr32.exe"; Parameters: "/s ""{app}\tsf_opi.dll"""; StatusMsg: "Registering opi TSF text service..."; Flags: runhidden waituntilterminated

[UninstallRun]
Filename: "{sys}\regsvr32.exe"; Parameters: "/s /u ""{app}\tsf_opi.dll"""; Flags: runhidden waituntilterminated

[UninstallDelete]
Type: files; Name: "{commonappdata}\opi\tsf-register.log"
