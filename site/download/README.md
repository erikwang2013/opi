# 站点下载目录

静态站“下载”页直接链接本目录下的安装包，**不跳转 GitHub**。三档安装包分目录存放：

- `full/`：语音引擎 + 两套模型 + 全部词库（Android 仅 64 位）
- `normal/`：语音引擎 + 全部词库，模型在设置页下载
- `basic/`：语音引擎 + 仅拼音主词库，扩展词库/模型在设置页下载
- `components/asr/`：语音模型整包（普通/基础包也可在设置内逐文件下载）
- `components/dicts/`：基础包的扩展词库包（设置内下载并自动解压）

二进制不进入 git（见同目录 `.gitignore`）；部署时把该目录与 `site/` 一起同步到
静态服务器即可。文件由 `scripts/build-packages.sh`、Android/Windows/Packages CI
或桌面 jpackage 产出后按版本改名放入对应目录。

```bash
# 版本号按实际发布替换
V=1.31.0
mkdir -p site/download/{full,normal,basic,components/asr,components/dicts}
cp dist/opi-$V-android-universal.apk            site/download/normal/
cp dist/opi-$V-android-universal-full.apk       site/download/full/
cp dist/opi-$V-android-universal-basic.apk      site/download/basic/
cp dist/opi-$V-windows-x64-setup.exe            site/download/normal/
cp dist/opi-$V-windows-x64-full-setup.exe       site/download/full/
cp dist/opi-$V-windows-x64-basic-setup.exe      site/download/basic/
cp dist/opi-$V-linux-amd64.deb                  site/download/normal/
cp dist/opi-$V-linux-amd64-full.deb             site/download/full/
cp dist/opi-$V-linux-amd64-basic.deb            site/download/basic/
cp dist/opi-$V-linux-x86_64.rpm                 site/download/normal/
cp dist/opi-$V-linux-x86_64-full.rpm            site/download/full/
cp dist/opi-$V-linux-x86_64-basic.rpm           site/download/basic/
cp dist/opi-$V-macos-universal.pkg              site/download/normal/
cp dist/opi-$V-ios-unsigned.ipa                 site/download/normal/
cp dist/opi-$V-harmonyos-unsigned.hap           site/download/normal/
cp dist/opi-$V-custom-pinyin-200k.tsv           site/download/normal/
cp dist/opi-$V-rime-wanxiang-200k.tsv           site/download/normal/
cp dist/components/dicts/opi-dicts-$V.zip       site/download/components/dicts/
cp dist/components/asr/opi-asr-models-$V.zip    site/download/components/asr/
```

## 应用内下载清单

`site/version.json` 的 `packages.<平台>` 现在带 `full` / `normal` / `basic` 三档
（每档字段 `url`、`size`、`sha256`，客户端按 `BuildConfig.PACKAGE_TIER` /
`OPI_PACKAGE_TIER` 选自己那一档，缺档回退 `full`）；`hot` 仍是桌面热更新包。
`components` 给站点显示组件体积（`asr`、`dicts`）。字段大小 = 字节，SHA-256 = 64 位
十六进制；缺省 = 客户端显示「未校验」。
