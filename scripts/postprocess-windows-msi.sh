#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 给 windows runner 出的桌面应用 .msi 做三件 jpackage 做不了的事（2026-10-17 用户裁决
# 「增加安装的时候中断正在运行的应用，安装后启动」；2026-10-18 用户裁决「覆盖安装新版本后
# 还要重启才生效」）：
#
#   1. **装前杀进程**：执行序列 797（在 RemoveExistingProducts@798 之前）跑
#      `taskkill /f /t /im <应用名>.exe`。为什么必须是 797：jpackage 的 MSI 把
#      旧产品移除排在 CostInitialize 之前，正在运行的旧实例会让 RemoveFiles 走
#      「改名 + 重启后删除」那条路 —— 装完旧进程还活着，与「装完自启」叠成两个实例。
#      排在移除之前杀掉，整套升级就干净。
#   2. **装后自启**：ExitDialog 的 OptionalCheckBox（WixUI 自带、本 MSI 里控件与
#      CheckBox 表行都在）默认勾选，点 Finish 时 DoAction WixShellExec 拉起新装的
#      启动器。WixShellExec 是 impersonate 的 ShellExecute ⇒ 以**登录用户**（非提权）
#      身份启动 —— 这正是 per-machine 安装下想要的。
#   3. **装完重启 Explorer**：ExitDialog 的同一个复选框在 Finish 时再发一条 DoAction
#      （Ordering=2，晚于现有自启），经 WixCA 的 `WixSilentExec`（Type 65 = DLL +
#      continue-on-error）跑 `"taskkill" /f /im explorer.exe`。
#      · **只杀、不 `start explorer.exe`**：本 CA 从提权的 msiexec 里跑，提权上下文
#        起的 explorer 会以管理员令牌成为 shell（UAC 旁路，安全风险）。Explorer 由
#        Winlogon 的 `AutoRestartShell`（默认 1）在**用户自己的完整性级别**上拉回来。
#      · AutoRestartShell=0 的机器上 shell 不会自己回来（手动 Task Manager →
#        Run new task → explorer.exe）；安装本身不受影响（见 docs/install/windows.md）。
#      · 没有 `start` 就不需要 `cmd /c`：命令保持 WixCA 的「引号开头」硬要求，与 797
#        的 taskkill 同一形状。`taskkill /im` 命中所有可访问会话语义上的 explorer.exe
#        （多会话机器的行为本机无法验证，文档里如实写）。
#      · **只挂在 ExitDialog Finish**：静默/基本 UI 安装不走这个对话框 ⇒ 不会静默杀
#        shell；用户取消勾选也就不动作。条件与自启共用一句（NOT Installed ⇒ 首次安装
#        与大版本升级；修复/卸载时该控件本来就被 WixUI 隐藏）。首次安装也会重启一次
#        —— 那时没有旧 DLL 要换，只是同一个复选框带来的副作用，文档里写明。
#      · 用 WixSilentExec 而不是复用 WixQuietExec：后者只认 `WixQuietExecCmdLine`，
#        而那条属性已被 797 的装前 taskkill 占用（同属性两条命令会互相踩）。
#        `WixSilentExec` 与 `WixSilentExecCmdLine` 在这份 MSI 的 Binary.WixCA
#        （3.11.2.4516）导出表/宽字符串里实测存在（容器内 objdump + strings 读的）。
#      · ⚠️ 本 MSI **不含 tsf_opi.dll**（File 表实测；本文件下方会打印提示）。
#        这动作只有在 DLL 由其它步骤部署/替换时才对 TSF 版本有意义；否则只是刷新 shell。
#
# 为什么走 msitools 后处理而不是 jpackage 选项：CMP 1.11.1 的 AbstractJPackageTask
# **没有** --resource-dir 的口子（javap 实证：只有自用的 jpackageResources Provider，
# 不是 DSL 旋钮），换 main.wxs 模板的路走不通；jpackage 自身也没有关停/自启选项。
# 于是照本仓对 .deb 的先例（打包后注入）办 —— 只不过这次的「注入」用 msitools
# 在 **Linux 容器**里做：可本地复核（表级读回），比在 Windows runner 上写一段
# 本机无法执行的 PowerShell（WindowsInstaller COM）可信得多。
#
# ⚠️ **本脚本能证明什么、不能证明什么**（照实写）：
#   · 能证明：表被正确改到目标形状（读回断言 + 全表 diff 只差这四张）；
#     Binary.WixCA 真导出 WixShellExec/WixQuietExec/WixSilentExec（objdump 读的导出表）；
#     cab 未被破坏。
#   · 不能证明：**在真 Windows 上装一遍的行为**。杀进程/自启/复选框/Explorer 真被
#     结束并由 Winlogon 拉回，都需要在 Windows 主机上装一次来验 —— 本仓此前从未在
#     真机装过这个 MSI（docs/install/windows.md 有记）。「AutoRestartShell 默认 1」
#     出自 Microsoft 文档，不是本机实测。别把表级证据当行为级证据用。
#
# 依赖：msitools（msiinfo / msibuild / msiextract 在 PATH 上）。
# 本机没有 msitools，用 debian:12 容器跑（同 scripts/build-packages.sh 的用法）：
#   docker run --rm -v "$PWD:/w" -w /w debian:12 \
#     sh -c 'apt-get update -qq && apt-get install -y -qq msitools >/dev/null \
#            && scripts/postprocess-windows-msi.sh in.msi out.msi'
#
# 用法：scripts/postprocess-windows-msi.sh <输入.msi> [输出.msi]
#   省略输出 = 原地改（先写临时文件再原子替换）。
# 幂等：重复运行会被前置检查挡下（OpRestartExplorer 已在 ⇒ 报「已处理过」退出 0）。
#   **旧版**脚本（2026-10-17，只有前两个动作）的产物会被明确拒绝（退出 1）——
#   在原始 CI artifact 上重跑，别在半成品上追加。

set -euo pipefail

IN="${1:?用法: scripts/postprocess-windows-msi.sh <输入.msi> [输出.msi]}"
OUT="${2:-$IN}"

for tool in msiinfo msibuild; do
    command -v "$tool" >/dev/null || {
        echo "!! 缺 $tool —— 本脚本要在有 msitools 的环境里跑（debian:12 容器，见文件头）" >&2
        exit 1
    }
done
[ -f "$IN" ] || { echo "!! 输入不存在：$IN" >&2; exit 1; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/opi-msi-post.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# 先复制一份来操作，成功后才写 OUT —— 中途失败既不碰原文件，也不在 OUT 留下
# 一个「看起来像处理过」的副本（2026-10-18：旧版会先 cp 到 OUT，拒绝旧产物时
# 也把输入原样留在那里，容易误用）。
cp "$IN" "$WORK/out.msi"
M="$WORK/out.msi"

export_t() { msiinfo export "$M" "$1" 2>/dev/null; }
# ⚠️ msiinfo 导出的 .idt 是 **CRLF**（实测 od 确认）——所有「读/判」的管道都要先去 \r，
# 否则锚在行尾的断言（`...798$`）会全数落空、awk 的末列会拖着 \r。
export_clean_t() { msiinfo export "$M" "$1" 2>/dev/null | tr -d '\r'; }

# ---------- 前置检查：确认拿到的是 jpackage 出的这一款 MSI ----------
# 不是洁癖：这几条同时是「幂等护栏」和「形状假设的显式化」——假设错了要当场红，
# 不能改成另一款 MSI 上的静默错行。
#
# ⚠️ 判法一律「先落变量、再 grep <<<」：`msiinfo | grep -q` 在 pipefail 下会因
# grep 早退（SIGPIPE 141）随机假红 —— 第一次跑就撞上了（RemoveExisting 那条过、
# ControlEvent 那条红，成败全看缓冲时序）。别改回管道。
PROPS="$(export_clean_t Property)"
IES="$(export_clean_t InstallExecuteSequence)"
CA_T="$(export_clean_t CustomAction)"
CE_T="$(export_clean_t ControlEvent)"
BIN_T="$(export_clean_t Binary)"

grep -q "^ProductName	" <<<"$PROPS" || { echo "!! Property 表里没有 ProductName，不是 MSI？" >&2; exit 1; }
grep -q "^WIXUI_INSTALLDIR	" <<<"$PROPS" || { echo "!! 不是 WixUI_InstallDir 的 MSI（前提变了，先读脚本头再改）" >&2; exit 1; }
# 幂等：三个动作都在 ⇒ 直接退出 0（产物原样输出）。
if grep -q "^OpRestartExplorer	" <<<"$CA_T"; then
    echo "已处理过（OpRestartExplorer 已在 CustomAction 表）—— 什么都不做，退出 0"
    [ "$IN" = "$OUT" ] || cp "$WORK/out.msi" "$OUT"
    exit 0
fi
# 迁移护栏：旧版脚本（2026-10-17）的产物只有前两个动作。**别在它上面半途追加** ——
# 797 槽已被占，前置断言会红；即使绕过也会得到半新半旧的表。指回原始 artifact。
if grep -q "^WixQuietExecCmdLine	" <<<"$PROPS"; then
    echo "!! 这个 MSI 是**旧版**脚本处理过的产物（有 WixQuietExecCmdLine，无 OpRestartExplorer）" >&2
    echo "   旧产物缺 Explorer 重启动作；请在**未处理**的 CI artifact 上重跑本脚本。" >&2
    exit 1
fi

PRODUCT="$(awk -F'\t' '$1=="ProductName"{print $2}' <<<"$PROPS")"
EXE="${PRODUCT}.exe"

# 启动器 exe 的 File 键 —— 发现式取值（File 键每次构建都变，写死必漂）。
# 先落变量再 grep（同上：pipefail + SIGPIPE）。
FILE_T="$(export_clean_t File)"
FILEKEY="$(awk -F'\t' -v exe="$EXE" '
    NR>3 { n=split($3, a, "|"); if (a[n]==exe || a[1]==exe) print $1 }' <<<"$FILE_T")"
[ -n "$FILEKEY" ] || { echo "!! File 表里找不到 $EXE —— 启动器名变了？" >&2; exit 1; }
[ "$(wc -l <<<"$FILEKEY")" -eq 1 ] || { echo "!! $EXE 在 File 表里有多行：$FILEKEY" >&2; exit 1; }
echo "产品=$PRODUCT 启动器=$EXE File键=$FILEKEY"

# 如实提示（不阻断）：当前 CI artifact 只含桌面应用，**不含 tsf_opi.dll**
# （docs/install/windows.md §2.5）。没有 DLL 时，重启 Explorer 只刷新 shell，
# 不会让 TSF 换版本 —— 那个 DLL 是另一个 artifact，需另行部署/注册。
if grep -qi 'tsf_opi\.dll' <<<"$FILE_T"; then
    echo "File 表含 tsf_opi.dll —— Explorer 重启会参与 TSF DLL 的重新装载"
else
    echo "⚠️  File 表里没有 tsf_opi.dll：本 MSI 不含 TSF DLL（§2.5）。"
    echo "    仍注入 Explorer 重启（用户裁决），但它不会让 TSF 换版本；DLL 需另行部署。"
fi

# 基线断言：WixCA 二进制在位（下面三个动作都挂在它上面）。
grep -q "^WixCA	" <<<"$BIN_T" || { echo "!! 没有 WixCA 二进制（WixUtilExtension 没进来？）" >&2; exit 1; }

# 基线断言：我们要落的坑位与名字都空着。这几条防的是「换了个 MSI 还照改」——
# 比如 jpackage 改了标准动作编号，797 被占，静默叠号是灾难性的（两条同号行为未定义）。
grep -qE "^[^\t]+\t[^\t]*\t797$" <<<"$IES" \
    && { echo "!! 序列 797 已被占用，槽位假设失效（读脚本头注释再改号）" >&2; exit 1; }
grep -qE "^(OpKillRunning|OpLaunchApp|OpRestartExplorer)\t" <<<"$CA_T" \
    && { echo "!! 动作名已存在（改过一半的产物？）—— 拒改" >&2; exit 1; }
grep -q "^WixSilentExecCmdLine	" <<<"$PROPS" \
    && { echo "!! WixSilentExecCmdLine 已存在（改过一半的产物？）—— 拒改" >&2; exit 1; }
grep -qP "^RemoveExistingProducts\t[^\t]*\t798$" <<<"$IES" \
    || { echo "!! RemoveExistingProducts 不在 798（jpackage 模板变了）—— 读脚本头注释再改" >&2; exit 1; }
grep -qP "^ExitDialog\tFinish\tEndDialog\t" <<<"$CE_T" \
    || { echo "!! ExitDialog/Finish 的 EndDialog 基线不在 —— 不是预期的那套 WixUI" >&2; exit 1; }

# ---------- 改表：导出 → 追加行 → 导回 ----------
# 为什么用 -i 导表而不是 `-q "INSERT ..."`：msibuild 0.101 的 -q 拒 INSERT（实测
# 「failed to open query」），而 -i 的「导出→编辑→导回」是 msitools 文档化的路径，
# 且导回的是**整表**——diff 前/后导出即得完整性证据。
edit_table() { # $1=表名 $2=要追加的行（可多行，字面 TAB 分隔）
    local table="$1" rows="$2" before="$WORK/$1.before" after="$WORK/$1.after"
    export_t "$table" >"$before"
    # 按表自己的列数补齐末尾空列（CustomAction 有第 5 列 ExtendedType/消息 5.0；
    # 少一列 libmsi 不是报错而是**段错误** —— 2026-10-17 实测，别手写列数）。
    local ncols
    ncols="$(awk -F'\t' 'NR==1{print NF}' "$before")"
    printf '%s\n' "$rows" \
        | awk -F'\t' -v n="$ncols" -v OFS='\t' '{ for(i=NF+1;i<=n;i++) $i=""; print }' \
        | sed 's/$/\r/' >>"$before"
    cp "$before" "$after"
    msibuild "$M" -i "$after" >/dev/null || { echo "!! 写 $table 失败" >&2; exit 1; }
    echo "  修改表：$table（追加 $(wc -l <<<"$rows") 行，列数 $ncols）"
}

# 1) 杀进程（执行序列 797，WixQuietExec 立即执行形态读 WixQuietExecCmdLine）。
#    ⚠️ 两个格式要求都读自 wix311rtm 源码（qtexecca.cpp），别按直觉改：
#    · 命令行**必须以带引号的程序名开头**，否则 E_INVALIDARG（源码里的原句：
#      "Command string must begin with quoted application name."）——而 CA 带 +64
#      会把失败吞掉 ⇒ 裸写 `taskkill ...` 是**静默不杀**，最难发现的那种坏。
#    · 程序名**故意不写路径**：msiexec 的进程目录就是 system32（CreateProcess 搜索序
#      第 1 步），`"taskkill"` 无扩展名会自动补 .exe —— 写 [System64Folder] 反而会
#      撞上「目录属性在 CostFinalize 之后才有效」的时机问题（797 < CostInitialize 800）。
edit_table Property "WixQuietExecCmdLine	\"taskkill\" /f /t /im ${EXE}"
edit_table CustomAction "OpKillRunning	65	WixCA	WixQuietExec"
edit_table InstallExecuteSequence "OpKillRunning		797"

# 2) 装后自启（ExitDialog 复选框机制，全部用 WixUI 已有控件）。
#    文案同时点明第 3 条：同一个复选框也会重启 Explorer（否则「杀 shell」就没有
#    完成页上的显式提示）。控件 220x40；文案约 42 字符，**有没有被裁字未在真机看过**
#    （本机无 Windows，读不到实际渲染）。
edit_table Property "WIXUI_EXITDIALOGOPTIONALCHECKBOX	1"
edit_table Property "WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT	Launch ${PRODUCT} and restart Explorer"
edit_table Property "WixShellExecTarget	[#${FILEKEY}]"
edit_table CustomAction "OpLaunchApp	1	WixCA	WixShellExec"
# Ordering=1 < EndDialog 的 999 ⇒ DoAction 先于关对话框发布（与 WiX 官方
# LaunchApplication 配方同数）。条件用官配，别自创。
edit_table ControlEvent "ExitDialog	Finish	DoAction	OpLaunchApp	WIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 AND NOT Installed	1"

# 3) 装完重启 Explorer（同一复选框；Ordering=2 ⇒ 在自启之后、EndDialog(999) 之前）。
#    只杀不启：Winlogon 的 AutoRestartShell（默认 1）会用用户令牌把 shell 拉回来；
#    从提权的 msiexec 里 `start explorer.exe` 会得到提权 shell（UAC 旁路）—— 故意不写。
#    用 WixSilentExec 是为了不跟 797 的 WixQuietExecCmdLine 抢同一条属性。
edit_table Property "WixSilentExecCmdLine	\"taskkill\" /f /im explorer.exe"
edit_table CustomAction "OpRestartExplorer	65	WixCA	WixSilentExec"
# 完成页上的显式说明（WixUI 的 OptionalText 控件；属性为空时该控件隐藏）。
# 只看得到、不能单独勾选 —— 与重启动作共用上面那一个复选框。
edit_table Property "WIXUI_EXITDIALOGOPTIONALTEXT	Checking the box restarts Explorer so Windows reloads the input method. Other open apps keep the old version until restarted."
edit_table ControlEvent "ExitDialog	Finish	DoAction	OpRestartExplorer	WIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 AND NOT Installed	2"

# ---------- 读回断言：表真的到了目标形状 ----------
fail=0
check() { # $1=描述 $2=grep 模式 $3=表内容（不是表名 —— 同上，躲 SIGPIPE）
    if grep -qP "$2" <<<"$3"; then
        echo "  ✓ $1"
    else
        echo "  ✗ $1 —— 表里没找到 /$2/" >&2
        fail=1
    fi
}
echo "读回："
check "杀进程动作（797）"        '^OpKillRunning\t\t797$'                                  "$(export_clean_t InstallExecuteSequence)"
# 尾列空 ⇒ 导出行的行尾是 tab（+未知 CRLF），锚 $ 会失配；只锚行首段。
check "杀进程 CA（WixQuietExec）" '^OpKillRunning\t65\tWixCA\tWixQuietExec' "$(export_clean_t CustomAction)"
check "杀进程命令行"             "^WixQuietExecCmdLine\t\"taskkill\" /f /t /im ${EXE}$"     "$(export_clean_t Property)"
check "自启 CA（WixShellExec）"  '^OpLaunchApp\t1\tWixCA\tWixShellExec' "$(export_clean_t CustomAction)"
check "自启目标"                 "^WixShellExecTarget\t\[#${FILEKEY}\]$"                     "$(export_clean_t Property)"
check "复选框默认勾选"           '^WIXUI_EXITDIALOGOPTIONALCHECKBOX\t1$'                    "$(export_clean_t Property)"
check "复选框文案"               "^WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT\tLaunch ${PRODUCT} and restart Explorer$" "$(export_clean_t Property)"
check "Finish 发布自启"          '^ExitDialog\tFinish\tDoAction\tOpLaunchApp\t'             "$(export_clean_t ControlEvent)"
check "重启 Explorer CA（WixSilentExec）" '^OpRestartExplorer\t65\tWixCA\tWixSilentExec'      "$(export_clean_t CustomAction)"
check "重启 Explorer 命令行"     '^WixSilentExecCmdLine\t"taskkill" /f /im explorer.exe$'   "$(export_clean_t Property)"
check "完成页提示文案"           '^WIXUI_EXITDIALOGOPTIONALTEXT\tChecking the box restarts Explorer' "$(export_clean_t Property)"
check "Finish 发布重启（Ordering=2）" '^ExitDialog\tFinish\tDoAction\tOpRestartExplorer\tWIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 AND NOT Installed\t2' "$(export_clean_t ControlEvent)"
[ "$fail" -eq 0 ] || { echo "!! 读回断言失败，产物不落" >&2; exit 1; }

# ---------- 完整性：除这四张表外全库不许有变化 ----------
# （等价于「msiextract 全解还能出来」的更强版本：逐表逐字节比，
#  任何意外表变更/丢失都会在这里红。）
echo "完整性（全表 diff）："
for t in $(msiinfo tables "$IN" 2>/dev/null); do
    case "$t" in Property|CustomAction|InstallExecuteSequence|ControlEvent) continue ;; esac
    if ! diff -q <(msiinfo export "$IN" "$t" 2>/dev/null) <(export_t "$t") >/dev/null; then
        echo "!! 表 $t 出现了非预期变化" >&2
        fail=1
    fi
done
[ "$fail" -eq 0 ] || { echo "!! 完整性失败，产物不落" >&2; exit 1; }
echo "  ✓ 其余表逐字节未变"

# ---------- 落盘（原地改走原子替换）----------
if [ "$IN" = "$OUT" ]; then
    cp "$WORK/out.msi" "$OUT.tmp" && mv "$OUT.tmp" "$OUT"
else
    cp "$WORK/out.msi" "$OUT"
fi
echo "✅ 完成：$OUT（$(wc -c <"$OUT") 字节）"
