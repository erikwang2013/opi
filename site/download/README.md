# 站点下载目录

静态站“下载”页直接链接本目录下的安装包，**不跳转 GitHub**。文件名与仓库
`dist/` 保持一致，发版后从 `dist/` 拷入：

```bash
# 在仓库根目录执行（版本号按实际发布替换）
cp dist/opi-1.22.0-linux-amd64.deb \
   dist/opi-1.22.0-linux-x86_64.rpm \
   dist/opi-1.22.0-windows-x64-setup.exe \
   dist/opi-1.22.0-android-universal.apk \
   dist/opi-1.22.0-macos-universal.pkg \
   dist/opi-1.22.0-ios-unsigned.ipa \
   dist/opi-1.22.0-harmonyos-unsigned.hap \
   site/download/
```

二进制不进入 git（见同目录 `.gitignore`）；部署时把该目录与 `site/` 一起同步到
静态服务器即可。
