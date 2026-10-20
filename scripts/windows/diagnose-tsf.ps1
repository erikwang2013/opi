#Requires -Version 5.1
<#
.SYNOPSIS
  Diagnose why OPI does not appear in the Windows 10/11 input method list.

.DESCRIPTION
  Read-only. Checks the COM in-proc server, the CTF TIP language profile,
  the HKLM machine-wide default-enable bit, per-user enable state, the
  installed language/input-method list, (optionally) the install log —
  the MSI's OpRegisterTsf or the EXE installer's `opi: regsvr32` line — and
  %ProgramData%\opi\tsf-register.log (fixed-path failures written by the DLL).

  Run in an elevated 64-bit PowerShell for complete HKLM visibility:
    powershell -ExecutionPolicy Bypass -File .\diagnose-tsf.ps1 `
      -InstallLog .\install.log

  Exit code 0 = required checks passed; 1 = at least one required check failed.
  A "WARN" line is informational and does not affect the exit code.
#>
[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:ProgramFiles 'opi'),
    [string]$DllPath,
    [string]$InstallLog
)

$ErrorActionPreference = 'Continue'
$script:fail = 0

$Clsid = '{98C51858-590B-404D-8915-21EB44250062}'
$ProfileGuid = '{A31A137C-1404-42F2-B82C-4E63F15DEFD6}'
$LangId = '0x00000804'
# TSF categories (msctf.h / windows crate constants). TIPCAP_* are the Windows 8+
# capabilities; a missing IMMERSIVESUPPORT is the documented reason a TIP is
# offered to desktop apps but not to UWP/Store (immersive) hosts.
$TipCategories = [ordered]@{
    '{34745C63-B2F0-4784-8B67-5E12C8701A31}' = 'GUID_TFCAT_TIP_KEYBOARD'
    '{13A016DF-560B-46CD-947A-4C3AF1E0E35D}' = 'GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT'
    '{49D2F9CF-1F5E-11D7-A6D3-00065B84435C}' = 'GUID_TFCAT_TIPCAP_UIELEMENTENABLED'
    '{25504FB4-7BAB-4BC1-9C69-CF81890F0EF5}' = 'GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT'
}

function Section([string]$Title) {
    Write-Host ''
    Write-Host "=== $Title ==="
}

function Check([string]$Label, [bool]$Ok, [string]$Detail = '') {
    if ($Ok) {
        Write-Host ("  [OK]   {0}" -f $Label)
    } else {
        Write-Host ("  [FAIL] {0}" -f $Label)
        $script:fail++
    }
    if ($Detail) { Write-Host ("         {0}" -f $Detail) }
}

function Warn([string]$Label, [string]$Detail = '') {
    Write-Host ("  [WARN] {0}" -f $Label)
    if ($Detail) { Write-Host ("         {0}" -f $Detail) }
}

function Get-RegDefault([string]$Path) {
    $key = Get-Item -LiteralPath ("Registry::" + $Path) -ErrorAction SilentlyContinue
    if (-not $key) { return $null }
    return $key.GetValue('')
}

function Test-Pe64([string]$Path) {
    try {
        $stream = [IO.File]::OpenRead($Path)
        try {
            $reader = New-Object IO.BinaryReader($stream)
            if ($reader.ReadUInt16() -ne 0x5A4D) { return $false }  # MZ
            $stream.Position = 0x3C
            $peOffset = $reader.ReadInt32()
            if ($peOffset -lt 0 -or $peOffset -gt ($stream.Length - 6)) { return $false }
            $stream.Position = $peOffset
            if ($reader.ReadUInt32() -ne 0x00004550) { return $false }  # PE\0\0
            return ($reader.ReadUInt16() -eq 0x8664)                    # AMD64
        } finally {
            $stream.Dispose()
        }
    } catch {
        return $false
    }
}

Section 'OS / PowerShell'
$os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
if ($os) {
    Write-Host ("OS: {0} build {1} ({2})" -f $os.Caption, $os.BuildNumber, $os.OSArchitecture)
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
$elevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Write-Host ("PowerShell: {0}, 64-bit process: {1}, elevated: {2}" -f `
    $PSVersionTable.PSVersion, [Environment]::Is64BitProcess, $elevated)
Write-Host ("Account: {0} (SID {1}), SYSTEM: {2}" -f `
    $identity.Name, $identity.User.Value, $identity.IsSystem)
if ($identity.IsSystem) {
    Write-Host '         Running as SYSTEM: HKCU below is SYSTEM''s hive, not the target user''s.'
}
if (-not [Environment]::Is64BitProcess) {
    Check '64-bit PowerShell' $false 'Run the 64-bit PowerShell; 32-bit reads a different registry view.'
}

Section 'Installed files'
if (-not $DllPath) { $DllPath = Join-Path $InstallDir 'app\tsf_opi.dll' }
$DllPath = [IO.Path]::GetFullPath($DllPath)
$dllOk = Test-Path -LiteralPath $DllPath -PathType Leaf
Check 'TSF DLL exists' $dllOk $DllPath
if ($dllOk) {
    $file = Get-Item -LiteralPath $DllPath
    Write-Host ("         size={0} bytes, mtime={1}" -f $file.Length, $file.LastWriteTime)
    Write-Host ("         sha256={0}" -f (Get-FileHash -LiteralPath $DllPath -Algorithm SHA256).Hash)
    Check 'TSF DLL is x86-64 (AMD64)' (Test-Pe64 $DllPath)
}

Section 'COM in-proc server (HKCR)'
$inprocPath = "HKEY_CLASSES_ROOT\CLSID\$Clsid\InprocServer32"
$inprocDll = Get-RegDefault $inprocPath
$inprocKey = Get-Item -LiteralPath ("Registry::" + $inprocPath) -ErrorAction SilentlyContinue
$threading = if ($inprocKey) { $inprocKey.GetValue('ThreadingModel') } else { $null }
Check 'HKCR\CLSID\...\InprocServer32 exists' ($null -ne $inprocDll) $inprocPath
if ($inprocDll) {
    $matches = $false
    try {
        $matches = ([IO.Path]::GetFullPath($inprocDll) -eq $DllPath)
    } catch { }
    Check 'InprocServer32 points at the installed DLL' $matches $inprocDll
    Check 'InprocServer32 file exists' (Test-Path -LiteralPath $inprocDll -PathType Leaf)
}
Check 'ThreadingModel = Apartment' ("$threading" -eq 'Apartment') "got [$threading]"

Section 'CTF TIP registration (HKLM)'
$tipPath = "HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\CTF\TIP\$Clsid"
$tipKey = Get-Item -LiteralPath ("Registry::" + $tipPath) -ErrorAction SilentlyContinue
Check 'HKLM CTF\TIP\{CLSID} exists' ($null -ne $tipKey) $tipPath
$profilePath = "$tipPath\LanguageProfile\$LangId\$ProfileGuid"
$profileKey = Get-Item -LiteralPath ("Registry::" + $profilePath) -ErrorAction SilentlyContinue
Check 'Language profile (zh-CN / profile GUID) exists' ($null -ne $profileKey) $profilePath
if ($profileKey) {
    $desc = $profileKey.GetValue('Description')
    $enable = $profileKey.GetValue('Enable')
    Write-Host ("         Description=[{0}] IconFile=[{1}] IconIndex=[{2}] Enable=[{3}]" -f `
        $desc, $profileKey.GetValue('IconFile'), $profileKey.GetValue('IconIndex'), $enable)
    Write-Host '         HKLM Enable is the machine-wide default (EnableLanguageProfileByDefault); HKCU overrides it per user.'
    if ("$enable" -ne '1') {
        Warn 'HKLM default-enable is not 1' 'Non-interactive (SYSTEM/Intune) deployment relies on this value; reinstall or set it machine-wide.'
    }
}

Section 'Per-user enable state (HKCU)'
$userProfilePath = "HKEY_CURRENT_USER\SOFTWARE\Microsoft\CTF\TIP\$Clsid\LanguageProfile\$LangId\$ProfileGuid"
$userProfileKey = Get-Item -LiteralPath ("Registry::" + $userProfilePath) -ErrorAction SilentlyContinue
if ($userProfileKey) {
    $userEnable = $userProfileKey.GetValue('Enable')
    Write-Host ("         path={0}" -f $userProfilePath)
    Write-Host ("         Enable=[{0}]" -f $userEnable)
    Check 'Profile enabled for the current user' ("$userEnable" -eq '1') "Enable=[$userEnable]"
} else {
    Warn 'No per-user CTF profile key' $userProfilePath
    Warn 'Expected if this account never ran EnableLanguageProfile; the MSI custom action runs in the installing account.'
    if ($identity.IsSystem) {
        Warn 'This account is SYSTEM' 'Target users are covered only by the HKLM default-enable value above, not by SYSTEM''s HKCU.'
    }
}

Section 'TIP categories'
foreach ($guid in $TipCategories.Keys) {
    $name = $TipCategories[$guid]
    $catPath = "$tipPath\Category\Category\$guid"
    if (Get-Item -LiteralPath ("Registry::" + $catPath) -ErrorAction SilentlyContinue) {
        Write-Host ("  [OK]   {0}" -f $name)
    } else {
        Warn "$name category not found" $catPath
    }
}
Write-Host '         The registry spelling varies; inspect the CTF dump below before treating a miss as a failure.'
Write-Host '         Missing IMMERSIVESUPPORT = the TIP is not offered to UWP/Store apps.'
Write-Host '         Missing UIELEMENTENABLED = the host may expect the system to draw candidates.'

$ctfDump = @(& reg.exe query "HKLM\SOFTWARE\Microsoft\CTF" /s /f $Clsid 2>$null)
if ($ctfDump.Count -gt 0) {
    Write-Host '  reg query HKLM\SOFTWARE\Microsoft\CTF /s /f {CLSID} (first 30 lines):'
    $ctfDump | Select-Object -First 30 | ForEach-Object { Write-Host ("         {0}" -f $_) }
} else {
    Warn 'reg query found no HKLM CTF entries for this CLSID'
}

Section 'Windows language / input method list'
try {
    $languages = @(Get-WinUserLanguageList)
    foreach ($lang in $languages) {
        Write-Host ("  {0}  [{1}]" -f $lang.LanguageTag, (($lang.InputMethodTips) -join ', '))
    }
    $zh = @($languages | Where-Object { $_.LanguageTag -like 'zh-*' })
    Check 'A Chinese language is installed' ($zh.Count -gt 0) 'Settings > Time & Language > Language'
    if ($zh.Count -gt 0) {
        $tips = @($zh | ForEach-Object { $_.InputMethodTips } | Where-Object { $_ })
        $opiTips = @($tips | Where-Object {
            $_ -match [regex]::Escape($Clsid) -and $_ -match [regex]::Escape($ProfileGuid)
        })
        Check 'OPI profile is in InputMethodTips' ($opiTips.Count -gt 0) ($tips -join ', ')
        if ($opiTips.Count -eq 0) {
            Write-Host '         Expected a tip containing 0804 + the CLSID + the profile GUID.'
        }
    }
} catch {
    Warn 'Get-WinUserLanguageList failed' $_.Exception.Message
}

Section 'Install log (MSI OpRegisterTsf / EXE regsvr32)'
if (-not $InstallLog) {
    foreach ($candidate in @(
        (Join-Path (Get-Location) 'install.log'),
        (Join-Path (Get-Location) 'install-exe.log'),
        (Join-Path $env:TEMP 'install.log')
    )) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { $InstallLog = $candidate; break }
    }
}
if ($InstallLog -and (Test-Path -LiteralPath $InstallLog -PathType Leaf)) {
    Write-Host ("  log: {0}" -f $InstallLog)
    $hits = @(Select-String -LiteralPath $InstallLog `
        -Pattern 'OpRegisterTsf|WixSilentExec64|opi: regsvr32|DllRegisterServer|Return value 3|Installation failed|Error 1721' `
        -ErrorAction SilentlyContinue)
    if ($hits.Count -eq 0) {
        Warn 'No registration lines in the log' 'Re-run with a log: msiexec /i <msi> /l*v install.log, or opi-<version>-setup.exe /LOG=install-exe.log'
    } else {
        $hits | Select-Object -First 40 | ForEach-Object { Write-Host ("         {0}" -f $_.Line.Trim()) }
    }
    $bad = @($hits | Where-Object { $_.Line -match 'Return value 3|Installation failed|Error 1721' })
    Check 'No fatal MSI/registration error in the log' ($bad.Count -eq 0)
} else {
    Warn 'No install log supplied or found' 'Use: msiexec /i <msi> /l*v install.log, or opi-<version>-setup.exe /LOG=install-exe.log, then -InstallLog <file>'
}

Section 'TSF registration log (%ProgramData%\opi\tsf-register.log)'
# 2026-10-21: DllRegisterServer writes fatal + degraded steps here (regsvr32 /s only
# yields an exit code, and Inno's install log lives under %TEMP%). A bare
# "DllRegisterServer:" line means the profile/category registration did not finish.
$tsfLog = Join-Path $env:ProgramData 'opi\tsf-register.log'
if (Test-Path -LiteralPath $tsfLog -PathType Leaf) {
    Write-Host ("  log: {0}" -f $tsfLog)
    $lines = @(Get-Content -LiteralPath $tsfLog -ErrorAction SilentlyContinue)
    $lines | Select-Object -First 20 | ForEach-Object { Write-Host ("         {0}" -f $_) }
    $fatal = @($lines | Where-Object { $_ -match '^DllRegisterServer:|^RegisterCategory\(TIP_KEYBOARD\)' })
    Check 'No fatal TSF registration error recorded' ($fatal.Count -eq 0)
} else {
    Write-Host '  no tsf-register.log (no recorded registration failure)'
}

Section 'Result'
if ($script:fail -eq 0) {
    Write-Host 'RESULT: all required checks passed.'
    exit 0
}
Write-Host ("RESULT: {0} required check(s) failed." -f $script:fail)
exit 1
