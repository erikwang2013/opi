#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 给 windows runner 出的桌面应用 .msi 做两件 jpackage 做不了的事（2026-10-17 用户裁决
# 「增加安装的时候中断正在运行的应用，安装后启动」）：
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
#     WixCA.dll 真导出 WixShellExec/WixQuietExec；cab 未被破坏。
#   · 不能证明：**在真 Windows 上装一遍的行为**。杀进程/自启/复选框出现的实际
#     效果需要在 Windows 主机上装一次来验 —— 本仓此前从未在真机装过这个 MSI
#     （docs/install/windows.md 有记）。别把表级证据当行为级证据用。
#
# 依赖：msitools（msiinfo / msibuild / msiextract 在 PATH 上）。
# 本机没有 msitools，用 debian:12 容器跑（同 scripts/build-packages.sh 的用法）：
#   docker run --rm -v "$PWD:/w" -w /w debian:12 \
#     sh -c 'apt-get update -qq && apt-get install -y -qq msitools >/dev/null \
#            && scripts/postprocess-windows-msi.sh in.msi out.msi'
#
# 用法：scripts/postprocess-windows-msi.sh <输入.msi> [输出.msi]
#   省略输出 = 原地改（先写临时文件再原子替换）。
# 幂等：重复运行会被前置检查挡下（目标行已在 ⇒ 直接报「已处理过」退出 0）。

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

# 原地改时先复制一份来操作，成功后才替换 —— 中途失败不碰原文件。
if [ "$IN" = "$OUT" ]; then
    cp "$IN" "$WORK/out.msi"
else
    cp "$IN" "$OUT"
    cp "$IN" "$WORK/out.msi"
fi
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
if grep -q "^WixQuietExecCmdLine	" <<<"$PROPS"; then
    echo "已处理过（WixQuietExecCmdLine 已在 Property 表）—— 什么都不做，退出 0"
    [ "$IN" = "$OUT" ] || cp "$WORK/out.msi" "$OUT"
    exit 0
fi

PRODUCT="$(awk -F'\t' '$1=="ProductName"{print $2}' <<<"$PROPS")"
EXE="${PRODUCT}.exe"

# 启动器 exe 的 File 键 —— 发现式取值（File 键每次构建都变，写死必漂）。
FILEKEY="$(export_clean_t File | awk -F'\t' -v exe="$EXE" '
    NR>3 { n=split($3, a, "|"); if (a[n]==exe || a[1]==exe) print $1 }')"
[ -n "$FILEKEY" ] || { echo "!! File 表里找不到 $EXE —— 启动器名变了？" >&2; exit 1; }
[ "$(wc -l <<<"$FILEKEY")" -eq 1 ] || { echo "!! $EXE 在 File 表里有多行：$FILEKEY" >&2; exit 1; }
echo "产品=$PRODUCT 启动器=$EXE File键=$FILEKEY"

# 基线断言：WixCA 二进制在位（下面两个动作都挂在它上面）。
grep -q "^WixCA	" <<<"$BIN_T" || { echo "!! 没有 WixCA 二进制（WixUtilExtension 没进来？）" >&2; exit 1; }

# 基线断言：我们要落的坑位与名字都空着。这几条防的是「换了个 MSI 还照改」——
# 比如 jpackage 改了标准动作编号，797 被占，静默叠号是灾难性的（两条同号行为未定义）。
grep -qE "^[^\t]+\t[^\t]*\t797$" <<<"$IES" \
    && { echo "!! 序列 797 已被占用，槽位假设失效（读脚本头注释再改号）" >&2; exit 1; }
grep -qE "^(OpKillRunning|OpLaunchApp)\t" <<<"$CA_T" \
    && { echo "!! 动作名已存在（改过一半的产物？）—— 拒改" >&2; exit 1; }
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
edit_table Property "WIXUI_EXITDIALOGOPTIONALCHECKBOX	1"
edit_table Property "WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT	Launch ${PRODUCT}"
edit_table Property "WixShellExecTarget	[#${FILEKEY}]"
edit_table CustomAction "OpLaunchApp	1	WixCA	WixShellExec"
# Ordering=1 < EndDialog 的 999 ⇒ DoAction 先于关对话框发布（与 WiX 官方
# LaunchApplication 配方同数）。条件用官配，别自创。
edit_table ControlEvent "ExitDialog	Finish	DoAction	OpLaunchApp	WIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 AND NOT Installed	1"

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
check "复选框文案"               "^WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT\tLaunch ${PRODUCT}$" "$(export_clean_t Property)"
check "Finish 发布自启"          '^ExitDialog\tFinish\tDoAction\tOpLaunchApp\t'             "$(export_clean_t ControlEvent)"
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
