#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 跨端版本一致性门禁：唯一真源是 `Cargo.toml` 的 `[workspace.package] version`。
# 校验各端写死的版本位 —— 鸿蒙 `versionName`/`versionCode`、Android `versionName`、
# desktop `opiAppVersion`、iOS/macOS `Info.plist`、Windows 安装器的 ISCC 用法示例、
# 站点 `version.json`。
#
# Android 的 `versionCode` **不在这里**：它是独立递增计数器（97→98→…，docs/install/
# android.md §3.2），不是 maj.min.pat 编码；「versionName 变了必须严格变大」那条判据
# 需要对比最新 release tag，放在 ci.yml 的 versions job（那里才取 tag）。
#
# **stdout 只打印版本号**（CI 的 artifact 名从这里取），诊断一律走 stderr ——
# 否则 `ver="$(scripts/check-versions.sh)"` 会把诊断文本一起吞进变量。
#
# 用法：
#   scripts/check-versions.sh      # 全一致 → stdout 打印 1.26.4；否则非零退出
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# 真源的取法与形态校验只有一份实现（scripts/apple/version.sh），不在这里抄第二份。
v="$(bash scripts/apple/version.sh)"
IFS=. read -r major minor patch <<<"$v"
# 鸿蒙 versionCode 的构造规则（1.26.4 → 12604）。10# 前缀是必须的：
# bash 算术把带前导零的数当八进制，1.08.0 会直接报 "value too great for base"。
want_code=$((10#$major * 10000 + 10#$minor * 100 + 10#$patch))

fail=0
check() { # 文件 字段 实际值 期望值
  if [ -z "$3" ]; then
    echo "!! $1: 取不到 $2" >&2
    fail=1
  elif [ "$3" != "$4" ]; then
    echo "!! $1: $2=$3 ≠ $4" >&2
    fail=1
  fi
}

plist_str() { # 文件 key → 值；key 与 <string> 之间允许夹注释行（iOS 那份就夹了）
  awk -v key="$2" '
    !found && index($0, "<key>" key "</key>") { found = 1; next }
    found && /<string>/ {
      sub(/^.*<string>/, ""); sub(/<\/string>.*$/, ""); print; exit
    }
  ' "$1"
}

check harmony/AppScope/app.json5 versionName \
  "$(sed -n 's/^[[:space:]]*"versionName"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' harmony/AppScope/app.json5)" \
  "$v"
check harmony/AppScope/app.json5 versionCode \
  "$(sed -n 's/^[[:space:]]*"versionCode"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' harmony/AppScope/app.json5)" \
  "$want_code"
check harmony/entry/oh-package.json5 version \
  "$(sed -n 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' harmony/entry/oh-package.json5)" \
  "$v"
check android/app/build.gradle.kts versionName \
  "$(sed -n 's/^[[:space:]]*versionName = "\(.*\)"/\1/p' android/app/build.gradle.kts)" \
  "$v"
check desktop/build.gradle.kts opiAppVersion \
  "$(sed -n 's/^val opiAppVersion = "\(.*\)"/\1/p' desktop/build.gradle.kts)" \
  "$v"
check ios/Info.plist CFBundleShortVersionString \
  "$(plist_str ios/Info.plist CFBundleShortVersionString)" "$v"
check macos/Info.plist CFBundleShortVersionString \
  "$(plist_str macos/Info.plist CFBundleShortVersionString)" "$v"
# 真正的 AppVersion 由 CI 从 Cargo.toml 传进来；这里守的是文件头那句用法示例
# （发版清单里的一格，历史上有过漂移）。
check scripts/windows/opi.iss 'ISCC /DAppVersion 示例' \
  "$(sed -n 's/^; *ISCC\.exe \/DAppVersion=\([0-9][0-9.]*\).*/\1/p' scripts/windows/opi.iss)" \
  "$v"
check site/version.json version \
  "$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' site/version.json)" \
  "$v"

# 站点下载卡片：所有 download/opi-* 链接必须指向当前版本。历史 changelog 没有下载 URL，
# 所以可以整页匹配；防的是「version.json 改了、页面下载链接还挂旧版」这种半更新。
for page in site/index.html site/en/index.html; do
  bad="$(grep -oE '(\.\./)?download/opi-[0-9][^"[:space:]]*' "$page" \
    | grep -v "download/opi-$v-" || true)"
  if [ -n "$bad" ]; then
    echo "!! $page: 下载链接版本不是 $v：" >&2
    printf '   %s\n' "$bad" >&2
    fail=1
  fi
  if ! grep -q "download/opi-$v-" "$page"; then
    echo "!! $page: 没有 $v 的下载链接" >&2
    fail=1
  fi
done

[ "$fail" = 0 ] || {
  echo "版本不一致（真源 Cargo.toml=$v）—— 上面带 !! 的每一处都要一起动" >&2
  exit 1
}
echo "版本 $v：各端版本位一致（鸿蒙 versionCode=$want_code）" >&2
printf '%s\n' "$v"
