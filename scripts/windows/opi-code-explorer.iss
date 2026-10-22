// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi.iss 的 [Code] 片段：升级时 Explorer 仍映射着旧 tsf_opi.dll，Windows 不会
// 替换已加载的 in-proc COM 服务器。注销/重启能刷新，但用户要的是“装完即用”。
//
// 2026-10-20 白屏事故的教训：不能由提权安装器直接结束 Explorer，也不能用
// runascurrentuser 拉起 shell（那是管理员令牌，不是用户桌面）。这里只做两件事：
//   1. 先以**原用户令牌**执行 taskkill（只影响该用户的 shell）；
//   2. 以原用户令牌启动一个看门狗：等 Winlogon 自动重启 Explorer，若 3 秒后仍
//      没有 shell 才用 `%windir%\explorer.exe` 补一次，且补启动始终是原用户令牌。
// 即使用户关掉 AutoRestartShell，也不会停在空白桌面。
//
// 只在交互式安装且 **Explorer 确实映射过 tsf_opi.dll** 时调用（首次安装不重启
// 桌面）；静默安装绝不重启（CI/Server Core 没有 shell，且无人可恢复）。

var
  ExplorerHadTsf: Boolean;

// Called before ssInstall moves the locked DLL, when the old image is still mapped.
function ExplorerMapsTsf(): Boolean;
var
  ResultCode: Integer;
begin
  // /m tsf_opi.dll 只列出加载了旧 DLL 的进程；管道给 find 判 explorer.exe 是否在
  // 其中。`find` 的退出码 0 = 命中 —— 不用临时文件，免去 tasklist 输出编码的坑。
  // 不带 /m 的 `tasklist /fi "imagename eq explorer.exe"` 只输出映像名/PID/内存，
  // 永远看不到模块名 —— 那样探测恒为 false，热升级就失效了。
  // {sysnative}: Setup is a 32-bit process; {sys}\tasklist is the WOW64 copy and
  // cannot enumerate modules of 64-bit Explorer. {sysnative} maps to native
  // System32 (x64-only installer, see ArchitecturesAllowed).
  if Exec(ExpandConstant('{sysnative}\cmd.exe'),
    '/c tasklist /nh /m tsf_opi.dll | find /i "explorer.exe" >nul',
    '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Result := ResultCode = 0
  else
  begin
    Log('opi: tasklist could not be started; Explorer check skipped');
    Result := False;
  end;
  if Result then
    Log('opi: Explorer maps old tsf_opi.dll: yes')
  else
    Log('opi: Explorer maps old tsf_opi.dll: no');
end;

// Called after ssPostInstall re-registered the new DLL, interactive installs only.
procedure RestartExplorerAfterInstall();
var
  ResultCode: Integer;
  Watchdog: String;
begin
  if WizardSilent then
  begin
    Log('opi: silent install; Explorer restart skipped');
    Exit;
  end;
  if not ExplorerHadTsf then
  begin
    Log('opi: Explorer did not map the old TSF DLL; no shell restart needed');
    Exit;
  end;
  // Original-user token: the setup process is elevated and may belong to a
  // different admin account. Killing by image name as that admin could hit
  // another user's shell.
  if ExecAsOriginalUser(ExpandConstant('{sysnative}\taskkill.exe'), '/f /im explorer.exe',
    '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('opi: taskkill explorer.exe (original user) exit code ' + IntToStr(ResultCode))
  else
  begin
    Log('opi: could not start taskkill as original user; shell restart skipped');
    Exit;
  end;
  // Watchdog runs as the original user. `ping` is the sleep form available on
  // every supported Windows; `find` decides whether Winlogon already restored
  // the shell, so a healthy auto-restart does not open a second Explorer window.
  Watchdog := '/c ping 127.0.0.1 -n 4 >nul & ' +
    'tasklist /nh /fi "imagename eq explorer.exe" | find /i "explorer.exe" >nul || ' +
    'start "" "%windir%\explorer.exe"';
  if ExecAsOriginalUser(ExpandConstant('{sysnative}\cmd.exe'), Watchdog, '', SW_HIDE, ewNoWait, ResultCode) then
    Log('opi: Explorer restart watchdog started (original user)')
  else
    Log('opi: could not start Explorer restart watchdog; sign out if the desktop stays blank');
end;
