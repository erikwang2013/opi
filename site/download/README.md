# 站点下载目录

静态站“下载”页直接链接本目录下的安装包，**不跳转 GitHub**。文件名与仓库
`dist/` 保持一致，发版后从 `dist/` 拷入：

```bash
# 在仓库根目录执行（版本号按实际发布替换）
cp dist/opi-1.29.0-linux-amd64.deb \
   dist/opi-1.29.0-linux-x86_64.rpm \
   dist/opi-1.29.0-linux-amd64-hot.zip \
   dist/opi-1.29.0-windows-x64-setup.exe \
   dist/opi-1.29.0-android-universal.apk \
   dist/opi-1.29.0-macos-universal.pkg \
   dist/opi-1.29.0-ios-unsigned.ipa \
   dist/opi-1.29.0-harmonyos-unsigned.hap \
   dist/opi-1.29.0-custom-pinyin-200k.tsv \
   dist/opi-1.29.0-rime-wanxiang-200k.tsv \
   site/download/
```

二进制不进入 git（见同目录 `.gitignore`）；部署时把该目录与 `site/` 一起同步到
静态服务器即可。

## 应用内下载清单（2026-10-31）

`site/version.json` 除 `version` / `download` 外，可带 `packages`：应用现在直接在
「关于」里下载并显示进度，不再跳浏览器。每个平台可给 `full`（完整安装包）和
`hot`（热更新包），字段都是 `url`（**必须 https**）、`size`（字节）、`sha256`
（64 位十六进制；缺省 = 客户端显示「未校验」）：

```json
"packages": {
  "windows-x64": { "full": { "url": "...exe", "size": 66094276, "sha256": "..." },
                    "hot":  { "url": "...zip", "size": 0, "sha256": "..." } },
  "linux-amd64": { "full": { "url": "...deb", ... }, "hot": { "url": "...zip", ... } },
  "linux-x86_64": { "full": { "url": "...rpm", ... }, "hot": { "url": "...zip", ... } },
  "android": { "full": { "url": "...apk", ... } }
}
```

平台键：Windows = `windows-x64`；Linux deb 系 = `linux-amd64`/`linux-arm64`，
rpm 系 = `linux-x86_64`/`linux-aarch64`；Android = `android`。客户端按本机包管理器
选键；没有对应 full/hot 时按钮不出现，退回下载页。

热更新包（桌面）= jpackage 应用映像里的 **app 目录**（不是整个安装包，省掉 JRE）：

```bash
# Linux（安装后目标是 /opt/opi/lib/app）
(cd desktop/build/compose/binaries/main/app/opi/lib/app && zip -r -9 /tmp/opi-$V-linux-amd64-hot.zip .)
# Windows（安装后目标是 {app}\app）
#   在 Windows 上把 <app-image>\app 整个目录压成 opi-$V-windows-x64-hot.zip
```

客户端只下载 + 校验，不自动提权覆盖；落地后由用户在「关于」里打开文件/文件夹，
关闭小欧后手工覆盖对应 app 目录（Linux 需 sudo）。Android 的 APK 由系统安装器二次
确认，无法静默替换。
