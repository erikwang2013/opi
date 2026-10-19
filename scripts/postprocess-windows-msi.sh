#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 给 windows runner 出的 .msi 做 jpackage 做不了的事（2026-10-17 用户裁决
# 「增加安装的时候中断正在运行的应用，安装后启动」；2026-10-18 用户裁决
# 「覆盖安装新版本后还要重启才生效」+「DLL 不要单独发布，用户不会用」；
# 2026-10-19 用户真机报障「装完有进程、键盘不显示」）：
#
# ⚠️ **2026-10-19 的根因**：发布出去的是 **jpackage 原始产物** —— Property 表没有
# WixSilentExec64CmdLine、CustomAction/InstallExecuteSequence 里没有注册动作，
# 安装时**从未执行 regsvr32**，TSF 的 COM/类别/语言 profile 全未注册 ⇒ 键盘不出现；
# 桌面应用文件照装、进程照起。自此本脚本是 CI 的**必需**步骤
# （.github/workflows/windows.yml 的 msi-postprocess job），不是可选手工动作。
#
#   0. **安装目录**（2026-10-19 用户裁决「Windows 安装目录以 opi 为基础目录」）：
#      Directory 表 `INSTALLDIR` 行的 DefaultDir 由 jpackage 的 `...|opi-candidates`
#      改为 `opi` ⇒ 默认装到 `C:\Program Files\opi`。注册动作里的 `[INSTALLDIR]`
#      随之解析到新路径。packageName 不改（TSF 的 settings_exe 依赖
#      `opi-candidates.exe`）。
#   1. **装前杀进程**：执行序列 797（在 RemoveExistingProducts@798 之前）跑
#      `taskkill /f /t /im <应用名>.exe`。旧实例会让 RemoveFiles 走「改名 + 重启后
#      删除」，装完旧进程还活着，与「装后自启」叠成两个实例。
#   2. **装后注册 TSF DLL**：DLL 已由 Gradle 挂进 jpackage `--input`、随 MSI 落
#      在 `INSTALLDIR\app\tsf_opi.dll`（Windows CI 把 tsf-dll artifact 传给
#      `OPI_TSF_DLL`）。执行序列 5900（InstallFiles 之后、InstallFinalize 之前）经
#      WixCA 的 `WixSilentExec64` 跑 `"[System64Folder]regsvr32.exe" /s "[INSTALLDIR]app\tsf_opi.dll"`；
#      条件 `NOT REMOVE`（安装/修复跑，卸载不跑）。旧 artifact 不含 DLL 时跳过本步
#      并打印提示。
#      · **必须用 64 位变体**（2026-10-19 复审 blocker）：WixCA 本身是 PE32 i386，
#        裸 `WixSilentExec` 在 WOW64 里解析到 SysWOW64 的 32 位 regsvr32，加载不了
#        x86-64 的 tsf_opi.dll。`WixSilentExec64` 配 `WixSilentExec64CmdLine`，
#        命令与 regsvr32 都是 64 位。
#      · **CA 类型是 1（DLL，Return=check），不是 65**（2026-10-19 reviewer 裁决）：
#        65 = 1 + 0x40（Continue）—— msiexec 会把失败当成功，`regsvr32` 失败时仍是
#        「装成功、键盘不出现」。类型 1 让注册失败直接**回滚安装**并留日志，宁可响亮
#        失败也不留半注册状态。`OpKillRunning` 保持 65（没有旧进程不该让安装失败）。
#   3. **完成页自启 + 重启 Explorer**：ExitDialog 的 OptionalCheckBox 默认勾选，
#      点 Finish 时 WixShellExec 拉起包装内的 `launch-and-restart.cmd`（Gradle 随
#      DLL 一起打进 app 目录）。脚本以**登录用户**身份启动候选窗，然后只
#      `taskkill /f /im explorer.exe` —— 从提权 msiexec 里 `start explorer.exe` 会
#      得到提权 shell（UAC 旁路），故意不写；Explorer 由 Winlogon 的
#      AutoRestartShell（默认 1）以用户完整性级别拉回。AutoRestartShell=0 时 shell
#      不会自己回来（任务管理器手动启动；安装本身不受影响）。
#      · 脚本只在 Finish 触发：静默/基本 UI 不杀 shell；取消勾选则既不启动也不重启。
#      · 没有脚本的旧 artifact 回退为直接启动 `<应用>.exe`（不重启 Explorer）。
#      · 两条 WixCA 命令用不同属性（797 的 WixQuietExecCmdLine / 注册的
#        WixSilentExec64CmdLine）—— 同一属性两条命令会互相踩。
#      · `OpLaunchApp` 用 32 位 `WixShellExec`：WixCA 没有 `WixShellExec64` 导出
#        （导出表实证）；32 位 ShellExecute 同样能拉起 `.cmd`。
#
# 为什么走 msitools 后处理而不是 jpackage 选项：CMP 1.11.1 的 AbstractJPackageTask
# **没有** --resource-dir 的口子（javap 实证：只有自用的 jpackageResources Provider，
# 不是 DSL 旋钮），换 main.wxs 模板的路走不通；jpackage 自身也没有关停/自启选项。
# 于是照本仓对 .deb 的先例（打包后注入）办 —— 只不过这次的「注入」用 msitools
# 在 **Linux 容器**里做：可本地复核（表级读回），比在 Windows runner 上写一段
# 本机无法执行的 PowerShell（WindowsInstaller COM）可信得多。
#
# ⚠️ **本脚本能证明什么、不能证明什么**（照实写）：
#   · 能证明：表被正确改到目标形状（读回断言 + 全表 diff 只差这几张）；
#     Binary.WixCA 是 PE32 i386，真导出 WixShellExec/WixQuietExec/WixSilentExec/
#     WixSilentExec64（objdump 读的导出表）；
#     cab **抽样可解包**（m1：msiinfo tables 不列 `_Streams`，全表 diff 覆盖不到 cab，
#     故只验「msiextract 能解出启动器/DLL 且非空」，不是 cab 逐字节比较）。
#   · 不能证明：**在真 Windows 上装一遍的行为**。杀进程/regsvr32/自启/脚本杀 Explorer
#     都需要在 Windows 主机上装一次来验。2026-10-19 真机装过的三份包（本地
#     processed-static / processed、CI opi-desktop-msi）都含后处理动作，仍无输入法；
#     但它们包内的 desktop jar 与 DLL 都是**修复前构建**，所以「修复后的包」仍未装过。
#     「AutoRestartShell 默认 1」出自 Microsoft 文档，不是本机实测。注册路径
#     `INSTALLDIR\app\tsf_opi.dll` 按 jpackage app-image 布局（Linux 同名布局为
#     `lib/app/`）推定，Windows 侧待真机核。别把表级证据当行为级证据用。
#
# 依赖：msitools（msiinfo / msibuild / msiextract 在 PATH 上）。
# 本机没有 msitools，用 debian:12 容器跑（同 scripts/build-packages.sh 的用法）：
#   docker run --rm -v "$PWD:/w" -w /w debian:12 \
#     sh -c 'apt-get update -qq && apt-get install -y -qq msitools >/dev/null \
#            && scripts/postprocess-windows-msi.sh in.msi out.msi'
#
# 用法：scripts/postprocess-windows-msi.sh <输入.msi> [输出.msi]
#   省略输出 = 原地改（先写临时文件再原子替换）。
# 幂等（m3）：产物带显式 Property `OPI_POSTPROCESSED=<形状版本>`；重跑见到同一
#   标记就原样输出并退出 0（含**不含 tsf_opi.dll** 的旧/测试产物 —— 它们跳过注册
#   动作，但同样已被处理）。无标记的旧产物一律退出 1：更早的（2026-10-17/18）
#   只有杀进程/自启/Explorer；较早的 2026-10-19 版有 OpRegisterTsf 但没有标记
#   （CA 可能仍是 Type 65）—— 在**未处理**的 CI artifact 上重跑，别在半成品上追加。

set -euo pipefail

IN="${1:?用法: scripts/postprocess-windows-msi.sh <输入.msi> [输出.msi]}"
OUT="${2:-$IN}"
# 转绝对路径：下面会 cd 进临时目录跑 msiinfo —— `msiinfo export Binary` 会把
# Binary/、Icon/ 目录解到**当前目录**（2026-10-19 实测，在仓库根留下 root 所有的
# 提取物）。cd 到 WORK 后，这些临时目录跟着 trap 一起清掉。
IN="$(cd "$(dirname "$IN")" && pwd)/$(basename "$IN")"
OUT_DIR="$(cd "$(dirname "$OUT")" && pwd)"
OUT="$OUT_DIR/$(basename "$OUT")"

for tool in msiinfo msibuild msiextract; do
    command -v "$tool" >/dev/null || {
        echo "!! 缺 $tool —— 本脚本要在有 msitools 的环境里跑（debian:12 容器，见文件头）" >&2
        exit 1
    }
done
[ -f "$IN" ] || { echo "!! 输入不存在：$IN" >&2; exit 1; }

# 后处理标记（2026-10-19 reviewer m3）：用显式 Property 判「本脚本处理过」，
# 而不是靠 OpRegisterTsf 等动作的蛛丝马迹 —— 不含 TSF DLL 的旧/测试产物会跳过
# 注册，重跑时旧判据既不正确定诊、也不幂等。形状/行为变了就换 MARKER_VALUE。
MARKER_KEY="OPI_POSTPROCESSED"
MARKER_VALUE="2026-10-20-r1"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/opi-msi-post.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

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
DIR_T="$(export_clean_t Directory)"

grep -q "^ProductName	" <<<"$PROPS" || { echo "!! Property 表里没有 ProductName，不是 MSI？" >&2; exit 1; }
grep -q "^WIXUI_INSTALLDIR	" <<<"$PROPS" || { echo "!! 不是 WixUI_InstallDir 的 MSI（前提变了，先读脚本头再改）" >&2; exit 1; }

# 安装目录基线（2026-10-19）：INSTALLDIR 行必须存在且是 jpackage 的原始形态
# （`短名|opi-candidates`）或已被本脚本改成 `opi`（幂等重跑）。别的形态 ⇒ 前提变了，
# 当场红而不是静默装到别处。
DIR_ROW="$(awk -F'\t' '$1=="INSTALLDIR"{print}' <<<"$DIR_T")"
[ -n "$DIR_ROW" ] || { echo "!! Directory 表里没有 INSTALLDIR 行（jpackage 模板变了）" >&2; exit 1; }
[ "$(wc -l <<<"$DIR_ROW")" -eq 1 ] || { echo "!! Directory 表里 INSTALLDIR 有多行，不敢改" >&2; exit 1; }
DIR_DEFAULT="$(cut -f3 <<<"$DIR_ROW")"
case "$DIR_DEFAULT" in
    opi) ;;
    *'|opi-candidates') echo "安装目录：INSTALLDIR 当前默认名是 opi-candidates，将改为 opi" ;;
    *) echo "!! INSTALLDIR 的 DefaultDir 是「$DIR_DEFAULT」，不是预期的「…|opi-candidates」或「opi」" >&2
       echo "   jpackage 模板变了 —— 先读脚本头再改。" >&2; exit 1 ;;
esac

# 幂等（m3）：先看显式标记。标记在 ⇒ 本脚本的当前形状版本，直接退出 0。
# 这条不受「包里有没有 tsf_opi.dll」影响：无 DLL 的旧/测试产物跳过注册，
# 但也已被本脚本处理过，重跑必须幂等输出成功。
MARKER="$(awk -F'\t' -v k="$MARKER_KEY" '$1==k{print $2}' <<<"$PROPS")"
if [ -n "$MARKER" ]; then
    if [ "$MARKER" != "$MARKER_VALUE" ]; then
        echo "!! $MARKER_KEY=$MARKER，不是本脚本（$MARKER_VALUE）的产物 —— 无法判断表形状版本" >&2
        echo "   请从未处理的 CI artifact 重跑。" >&2
        exit 1
    fi
    if [ "$DIR_DEFAULT" != "opi" ]; then
        echo "!! $MARKER_KEY=$MARKER 但 INSTALLDIR 不是 opi（当前「$DIR_DEFAULT」）—— 表被改过？" >&2
        echo "   请从未处理的 CI artifact 重跑。" >&2
        exit 1
    fi
    echo "已处理过（$MARKER_KEY=$MARKER）—— 什么都不做，退出 0"
    [ "$IN" = "$OUT" ] || cp "$WORK/out.msi" "$OUT"
    exit 0
fi
# 无标记但注册动作在：2026-10-19 早期脚本的产物（当时还没有标记，CA 可能仍是
# Type 65 = 注册失败被吞）。不能自动升级，明确拒绝并指向原始 artifact。
if grep -q "^OpRegisterTsf	" <<<"$CA_T"; then
    echo "!! 这个 MSI 已被较早的 2026-10-19 脚本处理过（OpRegisterTsf 在，但没有 $MARKER_KEY 标记）" >&2
    echo "   该产物可能是 Type 65；请从未处理的 CI artifact 重跑本脚本。" >&2
    exit 1
fi
# 迁移护栏：更早的脚本（2026-10-17/18）产物只剩杀进程/自启，没有标记。
# **别在它上面半途追加** —— 797 槽已被占，前置断言会红；即使绕过也是半新半旧的表。
if grep -q "^WixQuietExecCmdLine	" <<<"$PROPS" || grep -q "^OpRestartExplorer	" <<<"$CA_T"; then
    echo "!! 这个 MSI 已被处理过，但没有 $MARKER_KEY 标记（旧版脚本或早期产物）" >&2
    echo "   无从判断表形状版本（无 DLL 时连 OpRegisterTsf 都没有）；请在**未处理**的 CI artifact 上重跑。" >&2
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

# TSF DLL / 完成页脚本（2026-10-18 用户裁决「DLL 不要单独发布」）：
# jpackage `--input` 里的普通文件落在 `INSTALLDIR\app\`（Link 同 app 镜像布局）。
# 两者都按 File 表发现，键每次构建都变，不能写死。
file_key_by_name() { # $1 = 目标长文件名
    awk -F'\t' -v want="$1" '
        NR>3 { n=split($3, a, "|"); if (a[n]==want || a[1]==want) print $1 }' <<<"$FILE_T"
}
DLL_KEY="$(file_key_by_name 'tsf_opi.dll')"
SCRIPT_KEY="$(file_key_by_name 'launch-and-restart.cmd')"
if [ -n "$DLL_KEY" ] && [ "$(wc -l <<<"$DLL_KEY")" -ne 1 ]; then
    echo "!! tsf_opi.dll 在 File 表里有多行：$DLL_KEY" >&2
    exit 1
fi
if [ -n "$SCRIPT_KEY" ] && [ "$(wc -l <<<"$SCRIPT_KEY")" -ne 1 ]; then
    echo "!! launch-and-restart.cmd 在 File 表里有多行：$SCRIPT_KEY" >&2
    exit 1
fi
echo "产品=$PRODUCT 启动器=$EXE File键=$FILEKEY"
if [ -n "$DLL_KEY" ]; then
    echo "TSF DLL 在包内（File 键 $DLL_KEY）—— 将注入 regsvr32 注册动作"
else
    echo "⚠️  File 表里没有 tsf_opi.dll：本 MSI 不含 TSF DLL；跳过注册动作（旧产物/本地测试）"
fi
if [ -n "$SCRIPT_KEY" ]; then
    echo "完成页脚本在包内（File 键 $SCRIPT_KEY）—— 复选框将启动候选窗并重启 Explorer"
else
    echo "⚠️  没有 launch-and-restart.cmd：完成页复选框只启动候选窗（旧产物行为）"
fi

# 基线断言：WixCA 二进制在位（下面的动作都挂在它上面）。
grep -q "^WixCA	" <<<"$BIN_T" || { echo "!! 没有 WixCA 二进制（WixUtilExtension 没进来？）" >&2; exit 1; }

# 基线断言：我们要落的坑位与名字都空着。这几条防的是「换了个 MSI 还照改」——
# 比如 jpackage 改了标准动作编号，797 被占，静默叠号是灾难性的（两条同号行为未定义）。
grep -qE "^[^\t]+\t[^\t]*\t797$" <<<"$IES" \
    && { echo "!! 序列 797 已被占用，槽位假设失效（读脚本头注释再改号）" >&2; exit 1; }
grep -qE "^(OpKillRunning|OpLaunchApp|OpRegisterTsf)\t" <<<"$CA_T" \
    && { echo "!! 动作名已存在（改过一半的产物？）—— 拒改" >&2; exit 1; }
grep -q "^WixSilentExec64CmdLine	" <<<"$PROPS" \
    && { echo "!! WixSilentExec64CmdLine 已存在（改过一半的产物？）—— 拒改" >&2; exit 1; }
if [ -n "$DLL_KEY" ]; then
    grep -qE "^[^\t]+\t[^\t]*\t5900$" <<<"$IES" \
        && { echo "!! 序列 5900 已被占用，槽位假设失效（读脚本头注释再改号）" >&2; exit 1; }
fi
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

# 0) 安装目录 → C:\Program Files\opi（2026-10-19 用户裁决）。
#    DefaultDir 的 `短名|长名` 格式里，`opi` 本身是合法短名（≤8 字符、无空格），
#    故直接写 `opi`。只改 INSTALLDIR 这一行，其余行原样整表导回；统一 CRLF。
edit_install_dir() {
    local before="$WORK/Directory.before" clean="$WORK/Directory.clean" after="$WORK/Directory.after"
    export_t Directory >"$before"
    tr -d '\r' <"$before" >"$clean"
    awk -F'\t' -v OFS='\t' '$1=="INSTALLDIR"{$3="opi"} {print}' "$clean" \
        | sed 's/$/\r/' >"$after"
    msibuild "$M" -i "$after" >/dev/null || { echo "!! 写 Directory 失败" >&2; exit 1; }
    echo "  修改表：Directory（INSTALLDIR 的 DefaultDir → opi）"
}
edit_install_dir

# 后处理标记（m3）：所有改动都只在 WORK 副本上做，最后成功才落盘，所以这里先写
# 标记也不会在失败时留下「已处理」的 OUT；标记让重跑有显式判据。
edit_table Property "${MARKER_KEY}	${MARKER_VALUE}"

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
# 完成页脚本负责「启动候选窗 + 重启 Explorer」；没有脚本的旧产物仍直接启动 exe。
# 只杀不启：从提权的 msiexec 里 `start explorer.exe` 会得到提权 shell（UAC 旁路），
# 脚本只 taskkill，由 Winlogon 的 AutoRestartShell 以用户令牌拉回。
LAUNCH_KEY="${SCRIPT_KEY:-$FILEKEY}"
edit_table Property "WixShellExecTarget	[#${LAUNCH_KEY}]"
edit_table CustomAction "OpLaunchApp	1	WixCA	WixShellExec"
# Ordering=1 < EndDialog 的 999 ⇒ DoAction 先于关对话框发布（与 WiX 官方
# LaunchApplication 配方同数）。条件用官配，别自创。
edit_table ControlEvent "ExitDialog	Finish	DoAction	OpLaunchApp	WIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 AND NOT Installed	1"

# 3) 注册 TSF DLL（2026-10-18「DLL 不要单独发布」）。
#    装在 `INSTALLDIR\app\tsf_opi.dll`（jpackage `--input` 普通文件的落点，见脚本头）；
#    `regsvr32 /s` 以提权 msiexec 身份注册 per-machine COM。`WixSilentExec64` 与 797 的
#    WixQuietExecCmdLine 分开两条属性 —— 同一属性两条命令会互相踩。
#    64 位变体不可换成 32 位：WixCA 是 PE32 i386，裸 WixSilentExec 会调 SysWOW64 的
#    regsvr32，加载不了 x64 DLL。
#    条件 NOT REMOVE：安装/修复执行，卸载不执行（卸载产物会缺文件）。
if [ -n "$DLL_KEY" ]; then
    # 2026-10-20：用 [System64Folder] 全路径。CI Server Core 容器里裸 "regsvr32"
    # 让 QuietExec64 返回 0x80070003（ERROR_PATH_NOT_FOUND），安装回滚、目录为空。
    edit_table Property "WixSilentExec64CmdLine	\"[System64Folder]regsvr32.exe\" /s \"[INSTALLDIR]app\\tsf_opi.dll\""
    # Type 1（不是 65）：regsvr32 失败要让安装失败/回滚，不能被 Continue 位吞掉。
    edit_table CustomAction "OpRegisterTsf	1	WixCA	WixSilentExec64"
    edit_table InstallExecuteSequence "OpRegisterTsf	NOT REMOVE	5900"
    edit_table Property "WIXUI_EXITDIALOGOPTIONALTEXT	Checking the box restarts Explorer so Windows reloads the input method. Other open apps keep the old version until restarted."
fi

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
check "后处理标记"                "^${MARKER_KEY}\t${MARKER_VALUE}$"                        "$(export_clean_t Property)"
check "安装目录（INSTALLDIR=opi）" '^INSTALLDIR\tProgramFiles64Folder\topi$'                 "$(export_clean_t Directory)"
check "杀进程动作（797）"        '^OpKillRunning\t\t797$'                                  "$(export_clean_t InstallExecuteSequence)"
# 尾列空 ⇒ 导出行的行尾是 tab（+未知 CRLF），锚 $ 会失配；只锚行首段。
check "杀进程 CA（WixQuietExec）" '^OpKillRunning\t65\tWixCA\tWixQuietExec' "$(export_clean_t CustomAction)"
check "杀进程命令行"             "^WixQuietExecCmdLine\t\"taskkill\" /f /t /im ${EXE}$"     "$(export_clean_t Property)"
check "自启 CA（WixShellExec）"  '^OpLaunchApp\t1\tWixCA\tWixShellExec' "$(export_clean_t CustomAction)"
check "自启目标"                 "^WixShellExecTarget\t\[#${LAUNCH_KEY}\]$"                 "$(export_clean_t Property)"
check "复选框默认勾选"           '^WIXUI_EXITDIALOGOPTIONALCHECKBOX\t1$'                    "$(export_clean_t Property)"
check "复选框文案"               "^WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT\tLaunch ${PRODUCT} and restart Explorer$" "$(export_clean_t Property)"
check "Finish 发布自启"          '^ExitDialog\tFinish\tDoAction\tOpLaunchApp\t'             "$(export_clean_t ControlEvent)"
if [ -n "$DLL_KEY" ]; then
    # 尾列空 ⇒ 行尾是 tab：用 \t*$ 锚定，既排除 WixSilentExec 前缀误匹配又容忍空列。
    check "注册 TSF CA（WixSilentExec64 / Return=check）" '^OpRegisterTsf\t1\tWixCA\tWixSilentExec64\t*$' "$(export_clean_t CustomAction)"
    check "注册命令行（64 位全路径）" '^WixSilentExec64CmdLine\t"\[System64Folder\]regsvr32.exe" /s "\[INSTALLDIR\]app\\tsf_opi.dll"$' "$(export_clean_t Property)"
    check "注册时机（5900 / NOT REMOVE）" '^OpRegisterTsf\tNOT REMOVE\t5900$'                "$(export_clean_t InstallExecuteSequence)"
    check "完成页提示文案"           '^WIXUI_EXITDIALOGOPTIONALTEXT\tChecking the box restarts Explorer' "$(export_clean_t Property)"
fi
if [ -n "$SCRIPT_KEY" ]; then
    check "完成页脚本在包内"         "^${SCRIPT_KEY}	"                                      "$(export_clean_t File)"
fi
[ "$fail" -eq 0 ] || { echo "!! 读回断言失败，产物不落" >&2; exit 1; }

# ---------- 完整性：除这五张表外全库不许有变化 ----------
# 逐表逐字节比，任何意外表变更/丢失都会在这里红。⚠️ msiinfo tables **不列**
# `_Streams`（cab 在它里面），所以这一段只覆盖表结构；cab 另做抽样解包（见下）。
echo "完整性（全表 diff）："
for t in $(msiinfo tables "$IN" 2>/dev/null); do
    case "$t" in Property|CustomAction|InstallExecuteSequence|ControlEvent|Directory) continue ;; esac
    if ! diff -q <(msiinfo export "$IN" "$t" 2>/dev/null) <(export_t "$t") >/dev/null; then
        echo "!! 表 $t 出现了非预期变化" >&2
        fail=1
    fi
done
[ "$fail" -eq 0 ] || { echo "!! 完整性失败，产物不落" >&2; exit 1; }
echo "  ✓ 其余表逐字节未变"

# cab 抽样（m1）：msiextract 能解出 = cab 至少可读；不能证明每个流逐字节未变。
echo "cab 抽样解包："
EXTRACT="$WORK/extract"
mkdir -p "$EXTRACT"
msiextract -C "$EXTRACT" "$M" >/dev/null 2>&1 \
    || { echo "!! msiextract 解包失败（cab 可能损坏）" >&2; exit 1; }
EXE_PATH="$(find "$EXTRACT" -type f -name "$EXE" -print -quit)"
[ -n "$EXE_PATH" ] && [ -s "$EXE_PATH" ] \
    || { echo "!! 抽样解包里没有非空的 $EXE" >&2; exit 1; }
if [ -n "$DLL_KEY" ]; then
    DLL_PATH="$(find "$EXTRACT" -type f -name 'tsf_opi.dll' -print -quit)"
    [ -n "$DLL_PATH" ] && [ -s "$DLL_PATH" ] \
        || { echo "!! 抽样解包里没有非空的 tsf_opi.dll" >&2; exit 1; }
fi
EXTRACTED="$(find "$EXTRACT" -type f | wc -l)"
echo "  ✓ 解出 $EXTRACTED 个文件（含 $EXE${DLL_KEY:+ / tsf_opi.dll}）"

# ---------- 落盘（原地改走原子替换）----------
if [ "$IN" = "$OUT" ]; then
    cp "$WORK/out.msi" "$OUT.tmp" && mv "$OUT.tmp" "$OUT"
else
    cp "$WORK/out.msi" "$OUT"
fi
echo "✅ 完成：$OUT（$(wc -c <"$OUT") 字节）"
