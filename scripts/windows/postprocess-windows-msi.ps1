#Requires -Version 5.1
<#
.SYNOPSIS
  Windows-native post-processing for the jpackage MSI (WindowsInstaller COM).

.DESCRIPTION
  Mirrors scripts/postprocess-windows-msi.sh for people who build the MSI on a
  Windows machine and do not have msitools/WSL. It edits the same five tables:

    Directory              INSTALLDIR -> opi
    Property               OPI_POSTPROCESSED, WixQuietExecCmdLine,
                           WixSilentExec64CmdLine, WIXUI_EXITDIALOGOPTIONAL*
    CustomAction           OpKillRunning (65), OpLaunchApp (1), OpRegisterTsf (1)
    InstallExecuteSequence OpKillRunning @797, OpRegisterTsf @5900
    ControlEvent           ExitDialog/Finish -> OpLaunchApp

  The CI path still uses the bash/msitools script on Linux; this script is for
  local Windows builds and is intentionally dependency-free (WindowsInstaller
  COM ships with Windows). It has NOT been exercised on a real Windows host in
  this repository's Linux-only verification environment. Treat the table
  read-backs as structural evidence, not as proof that msiexec installs it.

  Run from an elevated 64-bit PowerShell. The script refuses to touch an MSI
  without the jpackage/WixUI baseline, an already-postprocessed MSI, or an MSI
  produced by an older marker-less postprocess version.

.PARAMETER InputMsi
  jpackage MSI to read.

.PARAMETER OutputMsi
  Destination MSI. Omit to edit InputMsi in place (temporary copy + replace).
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$InputMsi,
    [Parameter(Position = 1)]
    [string]$OutputMsi
)

$ErrorActionPreference = 'Stop'
$MarkerKey = 'OPI_POSTPROCESSED'
$MarkerValue = '2026-10-19-r3'

function Sql-Quote([string]$Value) {
    return "'" + $Value.Replace("'", "''") + "'"
}

function Open-MsiDatabase([string]$Path, [int]$Mode) {
    $installer = New-Object -ComObject WindowsInstaller.Installer
    return $installer.GetType().InvokeMember(
        'OpenDatabase', [Reflection.BindingFlags]::InvokeMethod,
        $null, $installer, @($Path, $Mode))
}

function Get-MsiRows($Db, [string]$Sql, [string[]]$Columns) {
    $view = $Db.OpenView($Sql)
    try {
        $view.Execute()
        while ($null -ne ($record = $view.Fetch())) {
            $values = [ordered]@{}
            for ($i = 0; $i -lt $Columns.Count; $i++) {
                try {
                    $values[$Columns[$i]] = $record.StringData($i + 1)
                } catch {
                    $values[$Columns[$i]] = $record.IntegerData($i + 1)
                }
            }
            [pscustomobject]$values
        }
    } finally {
        $view.Close()
    }
}

function Get-MsiScalar($Db, [string]$Sql) {
    $rows = @(Get-MsiRows $Db $Sql @('Value'))
    if ($rows.Count -eq 0) { return $null }
    return $rows[0].Value
}

function Invoke-MsiSql($Db, [string]$Sql) {
    $view = $Db.OpenView($Sql)
    try {
        $view.Execute()
    } finally {
        $view.Close()
    }
}

function Get-FileKey($FileRows, [string]$Name) {
    $hits = @()
    foreach ($row in $FileRows) {
        $parts = $row.FileName -split '\|'
        if ($parts -contains $Name -or $parts[-1] -eq $Name) {
            $hits += $row.File
        }
    }
    return $hits
}

function Assert-Value($Db, [string]$Sql, [string]$Expected, [string]$Label) {
    $actual = Get-MsiScalar $Db $Sql
    if ("$actual" -ne "$Expected") {
        throw "$Label : expected [$Expected], got [$actual]"
    }
    Write-Host "  OK  $Label"
}

function Assert-RowCount($Db, [string]$Sql, [int]$Expected, [string]$Label) {
    $rows = @(Get-MsiRows $Db $Sql @('Value'))
    if ($rows.Count -ne $Expected) {
        throw "$Label : expected $Expected row(s), got $($rows.Count)"
    }
    Write-Host "  OK  $Label"
}

$inFull = (Resolve-Path -LiteralPath $InputMsi).Path
if (-not $OutputMsi) {
    $outFull = $inFull
} elseif ([IO.Path]::IsPathRooted($OutputMsi)) {
    $outFull = [IO.Path]::GetFullPath($OutputMsi)
} else {
    $outFull = [IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputMsi))
}
$outDir = [IO.Path]::GetDirectoryName($outFull)
if (-not (Test-Path -LiteralPath $outDir -PathType Container)) {
    throw "output directory does not exist: $outDir"
}

$tmp = Join-Path $outDir ('.opi-msi-' + [guid]::NewGuid().ToString('N') + '.msi')
Copy-Item -LiteralPath $inFull -Destination $tmp -Force
$db = $null
try {
    $db = Open-MsiDatabase $tmp 1  # msiOpenDatabaseModeTransact

    # ---------- baseline: this must be the jpackage/WixUI MSI ----------
    $propRows = @(Get-MsiRows $db 'SELECT `Property`,`Value` FROM `Property`' @('Property', 'Value'))
    $props = @{}
    foreach ($row in $propRows) { $props[$row.Property] = $row.Value }
    if (-not $props.ContainsKey('ProductName')) { throw 'Property table has no ProductName' }
    if (-not $props.ContainsKey('WIXUI_INSTALLDIR')) { throw 'not a WixUI_InstallDir MSI' }

    $product = $props['ProductName']
    $exe = $product + '.exe'
    $dirDefault = Get-MsiScalar $db ('SELECT `DefaultDir` FROM `Directory` WHERE `Directory`=' + (Sql-Quote 'INSTALLDIR'))
    if (-not $dirDefault) { throw 'Directory table has no INSTALLDIR row' }
    if ($dirDefault -notmatch '(\||^)opi-candidates$' -and $dirDefault -ne 'opi') {
        throw "unexpected INSTALLDIR DefaultDir: $dirDefault"
    }

    $marker = $props[$MarkerKey]
    if ($marker) {
        if ($marker -ne $MarkerValue) {
            throw "$MarkerKey=$marker is not this script's shape ($MarkerValue)"
        }
        if ($dirDefault -ne 'opi') {
            throw "$MarkerKey is present but INSTALLDIR is not opi ($dirDefault)"
        }
        if ($inFull -ne $outFull) { Copy-Item -LiteralPath $tmp -Destination $outFull -Force }
        Write-Host "already postprocessed ($MarkerKey=$marker); nothing to do"
        exit 0
    }
    $caExists = Get-MsiScalar $db ('SELECT `Action` FROM `CustomAction` WHERE `Action`=' + (Sql-Quote 'OpRegisterTsf'))
    if ($caExists) {
        throw 'MSI has OpRegisterTsf but no marker: produced by an older postprocess; rerun on the raw CI artifact'
    }
    foreach ($name in @('WixQuietExecCmdLine', 'WixSilentExec64CmdLine')) {
        if ($props.ContainsKey($name)) { throw "MSI already has $name but no marker; refusing to append" }
    }
    foreach ($name in @('OpKillRunning', 'OpLaunchApp', 'OpRestartExplorer')) {
        if (Get-MsiScalar $db ('SELECT `Action` FROM `CustomAction` WHERE `Action`=' + (Sql-Quote $name))) {
            throw "MSI already has CustomAction $name but no marker; refusing to append"
        }
    }

    $fileRows = @(Get-MsiRows $db 'SELECT `File`,`FileName` FROM `File`' @('File', 'FileName'))
    $exeKeys = @(Get-FileKey $fileRows $exe)
    if ($exeKeys.Count -ne 1) { throw "expected one File row for $exe, got $($exeKeys.Count)" }
    $fileKey = $exeKeys[0]
    $dllKeys = @(Get-FileKey $fileRows 'tsf_opi.dll')
    $scriptKeys = @(Get-FileKey $fileRows 'launch-and-restart.cmd')
    $fontKeys = @(Get-FileKey $fileRows 'opi-emoji.ttf')
    $fontScriptKeys = @(Get-FileKey $fileRows 'install-emoji-font.ps1')
    if ($dllKeys.Count -gt 1) { throw 'tsf_opi.dll has multiple File rows' }
    if ($scriptKeys.Count -gt 1) { throw 'launch-and-restart.cmd has multiple File rows' }

    Write-Host "product=$product exe=$exe File=$fileKey"
    if ($dllKeys.Count -eq 1) { Write-Host 'TSF DLL is present: regsvr32 action will be added' }
    else { Write-Host 'WARNING: tsf_opi.dll is absent; install will not register TSF' }
    if ($scriptKeys.Count -eq 1) { Write-Host 'Finish launcher script is present' }
    else { Write-Host 'WARNING: launch-and-restart.cmd is absent; Finish only starts the app' }
    if ($fontKeys.Count -ne 1) { Write-Host 'WARNING: opi-emoji.ttf is absent; per-user font install will be skipped' }
    if ($fontScriptKeys.Count -ne 1) { Write-Host 'WARNING: install-emoji-font.ps1 is absent; font install will fail harmlessly' }

    # ---------- baseline slots / WixCA ----------
    if (-not (Get-MsiScalar $db ('SELECT `Name` FROM `Binary` WHERE `Name`=' + (Sql-Quote 'WixCA')))) {
        throw 'WixCA binary is missing'
    }
    if (Get-MsiScalar $db 'SELECT `Action` FROM `InstallExecuteSequence` WHERE `Sequence`=797') {
        throw 'InstallExecuteSequence slot 797 is already occupied'
    }
    if ($dllKeys.Count -eq 1 -and
        (Get-MsiScalar $db 'SELECT `Action` FROM `InstallExecuteSequence` WHERE `Sequence`=5900')) {
        throw 'InstallExecuteSequence slot 5900 is already occupied'
    }
    Assert-Value $db `
        ('SELECT `Sequence` FROM `InstallExecuteSequence` WHERE `Action`=' + (Sql-Quote 'RemoveExistingProducts')) `
        '798' 'RemoveExistingProducts is at 798'
    Assert-Value $db `
        ('SELECT `Event` FROM `ControlEvent` WHERE `Dialog_`=' + (Sql-Quote 'ExitDialog') +
            ' AND `Control_`=' + (Sql-Quote 'Finish') + ' AND `Event`=' + (Sql-Quote 'EndDialog')) `
        'EndDialog' 'ExitDialog/Finish EndDialog baseline'

    # ---------- edits ----------
    Invoke-MsiSql $db ('UPDATE `Directory` SET `DefaultDir`=' + (Sql-Quote 'opi') +
        ' WHERE `Directory`=' + (Sql-Quote 'INSTALLDIR'))
    Invoke-MsiSql $db ('INSERT INTO `Property` (`Property`,`Value`) VALUES (' +
        (Sql-Quote $MarkerKey) + ',' + (Sql-Quote $MarkerValue) + ')')

    Invoke-MsiSql $db ('INSERT INTO `Property` (`Property`,`Value`) VALUES (' +
        (Sql-Quote 'WixQuietExecCmdLine') + ',' +
        (Sql-Quote ('"taskkill" /f /t /im ' + $exe)) + ')')
    Invoke-MsiSql $db ('INSERT INTO `CustomAction` (`Action`,`Type`,`Source`,`Target`) VALUES (' +
        (Sql-Quote 'OpKillRunning') + ',65,' + (Sql-Quote 'WixCA') + ',' +
        (Sql-Quote 'WixQuietExec') + ')')
    Invoke-MsiSql $db ('INSERT INTO `InstallExecuteSequence` (`Action`,`Condition`,`Sequence`) VALUES (' +
        (Sql-Quote 'OpKillRunning') + ',' + (Sql-Quote '') + ',797)')

    Invoke-MsiSql $db ('INSERT INTO `Property` (`Property`,`Value`) VALUES (' +
        (Sql-Quote 'WIXUI_EXITDIALOGOPTIONALCHECKBOX') + ',' + (Sql-Quote '1') + ')')
    Invoke-MsiSql $db ('INSERT INTO `Property` (`Property`,`Value`) VALUES (' +
        (Sql-Quote 'WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT') + ',' +
        (Sql-Quote ('Launch ' + $product + ' and restart Explorer')) + ')')
    $launchKey = if ($scriptKeys.Count -eq 1) { $scriptKeys[0] } else { $fileKey }
    Invoke-MsiSql $db ('INSERT INTO `Property` (`Property`,`Value`) VALUES (' +
        (Sql-Quote 'WixShellExecTarget') + ',' + (Sql-Quote ('[#' + $launchKey + ']')) + ')')
    Invoke-MsiSql $db ('INSERT INTO `CustomAction` (`Action`,`Type`,`Source`,`Target`) VALUES (' +
        (Sql-Quote 'OpLaunchApp') + ',1,' + (Sql-Quote 'WixCA') + ',' +
        (Sql-Quote 'WixShellExec') + ')')
    Invoke-MsiSql $db ('INSERT INTO `ControlEvent` (`Dialog_`,`Control_`,`Event`,`Argument`,`Condition`,`Ordering`) VALUES (' +
        (Sql-Quote 'ExitDialog') + ',' + (Sql-Quote 'Finish') + ',' + (Sql-Quote 'DoAction') + ',' +
        (Sql-Quote 'OpLaunchApp') + ',' +
        (Sql-Quote 'WIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 AND NOT Installed') + ',1)')

    if ($dllKeys.Count -eq 1) {
        # 必须用 64 位变体：WixCA 是 PE32 i386，裸 WixSilentExec 会调 SysWOW64 的
        # 32 位 regsvr32，加载不了 x64 DLL。
        Invoke-MsiSql $db ('INSERT INTO `Property` (`Property`,`Value`) VALUES (' +
            (Sql-Quote 'WixSilentExec64CmdLine') + ',' +
            (Sql-Quote '"regsvr32" /s "[INSTALLDIR]app\tsf_opi.dll"') + ')')
        Invoke-MsiSql $db ('INSERT INTO `CustomAction` (`Action`,`Type`,`Source`,`Target`) VALUES (' +
            (Sql-Quote 'OpRegisterTsf') + ',1,' + (Sql-Quote 'WixCA') + ',' +
            (Sql-Quote 'WixSilentExec64') + ')')
        Invoke-MsiSql $db ('INSERT INTO `InstallExecuteSequence` (`Action`,`Condition`,`Sequence`) VALUES (' +
            (Sql-Quote 'OpRegisterTsf') + ',' + (Sql-Quote 'NOT REMOVE') + ',5900)')
        Invoke-MsiSql $db ('INSERT INTO `Property` (`Property`,`Value`) VALUES (' +
            (Sql-Quote 'WIXUI_EXITDIALOGOPTIONALTEXT') + ',' +
            (Sql-Quote 'Checking the box restarts Explorer so Windows reloads the input method. Other open apps keep the old version until restarted.') + ')')
    }

    # ---------- read-back (same shape as the bash script) ----------
    Write-Host 'read-back:'
    Assert-Value $db ('SELECT `Value` FROM `Property` WHERE `Property`=' + (Sql-Quote $MarkerKey)) `
        $MarkerValue 'postprocess marker'
    Assert-Value $db 'SELECT `DefaultDir` FROM `Directory` WHERE `Directory`=''INSTALLDIR''' `
        'opi' 'INSTALLDIR=opi'
    Assert-Value $db 'SELECT `Sequence` FROM `InstallExecuteSequence` WHERE `Action`=''OpKillRunning''' `
        '797' 'kill action @797'
    Assert-Value $db 'SELECT `Type` FROM `CustomAction` WHERE `Action`=''OpKillRunning''' `
        '65' 'kill CA type=65'
    Assert-Value $db ('SELECT `Value` FROM `Property` WHERE `Property`=' + (Sql-Quote 'WixQuietExecCmdLine')) `
        ('"taskkill" /f /t /im ' + $exe) 'kill command line'
    Assert-Value $db 'SELECT `Type` FROM `CustomAction` WHERE `Action`=''OpLaunchApp''' `
        '1' 'launch CA type=1'
    Assert-Value $db ('SELECT `Value` FROM `Property` WHERE `Property`=' + (Sql-Quote 'WixShellExecTarget')) `
        ('[#' + $launchKey + ']') 'launch target'
    Assert-Value $db 'SELECT `Value` FROM `Property` WHERE `Property`=''WIXUI_EXITDIALOGOPTIONALCHECKBOX''' `
        '1' 'Finish checkbox default'
    Assert-RowCount $db 'SELECT `Event` FROM `ControlEvent` WHERE `Dialog_`=''ExitDialog'' AND `Control_`=''Finish'' AND `Argument`=''OpLaunchApp''' `
        1 'Finish launches OpLaunchApp'
    if ($dllKeys.Count -eq 1) {
        Assert-Value $db 'SELECT `Type` FROM `CustomAction` WHERE `Action`=''OpRegisterTsf''' `
            '1' 'register CA type=1'
        Assert-Value $db 'SELECT `Target` FROM `CustomAction` WHERE `Action`=''OpRegisterTsf''' `
            'WixSilentExec64' 'register CA target=WixSilentExec64'
        Assert-Value $db ('SELECT `Value` FROM `Property` WHERE `Property`=' + (Sql-Quote 'WixSilentExec64CmdLine')) `
            '"regsvr32" /s "[INSTALLDIR]app\tsf_opi.dll"' 'register command line (64-bit)'
        Assert-Value $db 'SELECT `Condition` FROM `InstallExecuteSequence` WHERE `Action`=''OpRegisterTsf''' `
            'NOT REMOVE' 'register condition'
        Assert-Value $db 'SELECT `Sequence` FROM `InstallExecuteSequence` WHERE `Action`=''OpRegisterTsf''' `
            '5900' 'register action @5900'
    }

    $db.Commit()
    # Release the database handle before moving the file over the destination
    # (Windows may otherwise refuse the rename while the MSI is still open).
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($db)
    $db = $null
    Move-Item -LiteralPath $tmp -Destination $outFull -Force
    Write-Host "OK: $outFull ($((Get-Item -LiteralPath $outFull).Length) bytes)"
} finally {
    if ($db) { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($db) }
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue }
}
