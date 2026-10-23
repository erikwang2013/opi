// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi.iss 的 [Code] 片段：交互式升级时刷新 Explorer（用户免注销）。
//
// TSF TIP 是 in-proc COM：Explorer/Chrome/Office/ctfmon/TextInputHost 各自映射
// tsf_opi.dll，已运行进程继续用旧映像。Windows 不能热替换已加载的 DLL，所以
// 「装完即用」只能做到两层：
//   1. 交互式**升级**：注册新 DLL 后，以原用户令牌结束并重启该用户的 shell
//      （看门狗等 Winlogon 自动拉起，3 秒后仍没有 shell 才补启一次）；
//   2. 其它已打开的宿主：不结束它们（未保存数据风险），完成页/日志列出仍映射旧
//      DLL 的进程，提示关闭重开。
// 首次安装没有旧映像要刷新，不碰 shell；静默安装也不碰（CI/Server Core 没有 shell，
// 且无人可恢复）。
//
// 2026-10-20 白屏事故的教训：不能由提权安装器直接结束 Explorer，也不能用
// runascurrentuser 拉起 shell（那是管理员令牌，不是用户桌面）。这里的结束与补启
// 都走原用户令牌。
//
// 2026-10-23：重启判定从「Explorer 是否映射旧 DLL」改为「是否交互式升级」。
// tasklist /m 在受限机器上可能看不到模块，旧判定会静默跳过重启，用户不注销就
// 一直是旧代码。ExplorerMapsTsf 保留为日志证据（ci.yml 钉着调用点与顺序）。

var
  ExplorerHadTsf: Boolean;
  IsUpgrade: Boolean;
  TsfHolders: String;

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

// 安装后仍映射 tsf_opi.dll 的进程（逗号分隔的映像名；空串 = 没有）。
// tasklist /m 只列加载了该模块的进程；INFO/本地化提示行不含模块名，跳过。
function TasklistTsfHolders(): String;
var
  ResultCode: Integer;
  TmpFile, Line, Name: String;
  Lines: TArrayOfString;
  I: Integer;
begin
  Result := '';
  TmpFile := ExpandConstant('{tmp}\opi-tsf-holders.txt');
  DeleteFile(TmpFile);
  if not Exec(ExpandConstant('{sysnative}\cmd.exe'),
    '/c tasklist /nh /m tsf_opi.dll > "' + TmpFile + '" 2>nul', '',
    SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    Log('opi: tasklist could not be started; holder list unavailable');
    Exit;
  end;
  if not LoadStringsFromFile(TmpFile, Lines) then
  begin
    DeleteFile(TmpFile);
    Exit;
  end;
  for I := 0 to GetArrayLength(Lines) - 1 do
  begin
    Line := Trim(Lines[I]);
    if Pos('tsf_opi.dll', Lowercase(Line)) > 0 then
    begin
      // 输出首列是映像名；同一映像多行时去重。
      Name := Copy(Line, 1, Pos(' ', Line + ' ') - 1);
      if (Name <> '') and
         (Pos(',' + Lowercase(Name) + ',', ',' + Lowercase(Result) + ',') = 0) then
      begin
        if Result <> '' then
          Result := Result + ', ';
        Result := Result + Name;
      end;
    end;
  end;
  DeleteFile(TmpFile);
end;

// Called from ssPostInstall after the shell restart was started: log the list and
// hand it to the finish page (CurPageChanged).
procedure RecordTsfHolders();
begin
  // Same policy as the finish page; the log is the only evidence on silent installs.
  Log('opi: no sign-out required; new apps use the installed version; ' +
    'already-open apps must be closed and reopened');
  TsfHolders := TasklistTsfHolders();
  if TsfHolders = '' then
    Log('opi: no process maps the old tsf_opi.dll after install')
  else
    Log('opi: processes still mapping the old tsf_opi.dll (close and reopen them): ' + TsfHolders);
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
  // Evidence only: ci.yml pins this literal. The restart decision is IsUpgrade --
  // tasklist /m can silently fail to see the module, and that used to skip the
  // restart on exactly the machines that needed it.
  if not ExplorerHadTsf then
    Log('opi: Explorer did not map the old TSF DLL before install')
  else
    Log('opi: Explorer mapped the old TSF DLL before install');
  if not IsUpgrade then
  begin
    Log('opi: first install; no shell to refresh, Explorer restart skipped');
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

// 完成页：用户不用注销，但要重开已打开的应用；仍持旧 DLL 的进程直接列出。
procedure CurPageChanged(CurPageID: Integer);
var
  Note: String;
begin
  if (CurPageID = wpFinished) and (not WizardSilent) then
  begin
    Note := '无需注销：新打开的应用会使用已安装的版本；已打开的应用请关闭后重新打开。' + #13#10 +
      '静默安装不会重启 Explorer。';
    if TsfHolders <> '' then
      Note := Note + #13#10 + '仍加载旧版 TSF DLL 的进程：' + TsfHolders + '（关闭重开即可）。';
    WizardForm.FinishedLabel.Caption := Note;
  end;
end;
