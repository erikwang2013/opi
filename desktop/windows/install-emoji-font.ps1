# Per-user install of the bundled Opi Emoji PUA font.
#
# Runs from launch-and-restart.cmd at the MSI Finish page, as the logged-in user
# (WixShellExec), not as SYSTEM. Per-user fonts live in
# %LOCALAPPDATA%\Microsoft\Windows\Fonts and are registered under HKCU; no admin
# rights are needed and no machine-wide state is touched.
#
# The font is optional: the desktop UI already loads pet-font/opi-emoji.ttf from
# the application classpath. This copy only makes the family name "Opi Emoji"
# available to other Windows text hosts. Every failure is swallowed on purpose:
# a missing font must never stop the candidate window from launching.
$ErrorActionPreference = 'Stop'

try {
    $source = Join-Path $PSScriptRoot 'opi-emoji.ttf'
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { exit 0 }

    $fontDir = Join-Path $env:LOCALAPPDATA 'Microsoft\Windows\Fonts'
    New-Item -ItemType Directory -Force -Path $fontDir | Out-Null
    $target = Join-Path $fontDir 'opi-emoji.ttf'
    Copy-Item -LiteralPath $source -Destination $target -Force

    # Full path is the documented per-user font form (Windows 10 1809+).
    $fontsKey = 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts'
    New-ItemProperty -Path $fontsKey -Name 'Opi Emoji (TrueType)' `
        -Value $target -PropertyType String -Force | Out-Null

    # Tell already-running apps that the font list changed. 0xFFFF = HWND_BROADCAST,
    # WM_FONTCHANGE = 0x001D, SMTO_ABORTIFHUNG = 0x0002.
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class OpiFontBroadcast {
    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    public static extern IntPtr SendMessageTimeout(
        IntPtr hWnd, uint msg, UIntPtr wParam, IntPtr lParam,
        uint flags, uint timeout, out UIntPtr result);
}
'@
    $result = [UIntPtr]::Zero
    [void][OpiFontBroadcast]::SendMessageTimeout(
        [IntPtr]0xFFFF, 0x001D, [UIntPtr]::Zero, [IntPtr]::Zero,
        2, 1000, [ref]$result)
} catch {
    # Intentionally silent; caller redirects output.
}
exit 0
