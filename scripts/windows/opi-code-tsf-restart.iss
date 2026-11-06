; SPDX-FileCopyrightText: 2026 erik.xyz
; SPDX-License-Identifier: MIT
;
; 安装完成后自动重启仍加载旧 tsf_opi.dll 的 GUI 程序（2026-11-06 用户要求：
; 不要等用户点语音后再弹框让他注销）。Windows 不能热替换进程内 DLL，唯一办法
; 是让宿主进程退出再启动；这里只对有主窗口、能正常 CloseMainWindow 的程序做
; 优雅重启，不强杀、不处理没有窗口的系统进程。保存对话框里点取消的程序不会被
; 再次拉起，留给语音页的兜底提示。
;
; 由 opi.iss 的 [Run] 以原用户令牌执行（runasoriginaluser），静默安装跳过。

procedure PrepareTsfRestartScript();
var
  Script: String;
begin
  Script :=
    '$ErrorActionPreference = "SilentlyContinue"' + #13#10 +
    '$skip = @("explorer","opi","ctfmon","TextInputHost","System","Idle","Registry","Memory Compression","svchost","dwm","winlogon","csrss","smss","lsass","services","fontdrvhost","WUDFHost")' + #13#10 +
    '$targets = @()' + #13#10 +
    'Get-Process | ForEach-Object {' + #13#10 +
    '  $proc = $_' + #13#10 +
    '  if ($skip -contains $proc.ProcessName) { return }' + #13#10 +
    '  if ($proc.MainWindowHandle -eq 0) { return }' + #13#10 +
    '  $uses = $false' + #13#10 +
    '  try { $uses = [bool]($proc.Modules | Where-Object { $_.ModuleName -eq "tsf_opi.dll" }) } catch { $uses = $false }' + #13#10 +
    '  if (-not $uses) { return }' + #13#10 +
    '  $path = $proc.Path' + #13#10 +
    '  if (-not $path) { return }' + #13#10 +
    '  $targets += [pscustomobject]@{ Path = $path; Name = $proc.ProcessName }' + #13#10 +
    '  try { $proc.CloseMainWindow() | Out-Null } catch { }' + #13#10 +
    '}' + #13#10 +
    'Start-Sleep -Seconds 2' + #13#10 +
    'foreach ($t in $targets) {' + #13#10 +
    '  if (-not (Get-Process -Name $t.Name -ErrorAction SilentlyContinue)) {' + #13#10 +
    '    Start-Process -FilePath $t.Path' + #13#10 +
    '  }' + #13#10 +
    '}' + #13#10;
  SaveStringToFile(ExpandConstant('{tmp}\opi-restart-tsf.ps1'), Script, False);
end;
