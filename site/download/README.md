# 站点下载目录

静态站“下载”页直接链接本目录下的安装包，**不跳转 GitHub**。文件名与仓库
`dist/` 保持一致，发版后从 `dist/` 拷入：

```bash
# 在仓库根目录执行（版本号按实际发布替换）
cp dist/opi_1.17.1_amd64.deb \
   dist/opi-1.17.1-1.x86_64.rpm \
   dist/opi-1.17.1-setup.exe \
   dist/opi_1.17.1_android-release.apk \
   dist/opi-inputmethod-1.17.1-unsigned.pkg \
   dist/opi-ios-keyboard-1.17.1-unsigned.ipa \
   dist/opi-harmony-1.17.1-unsigned.hap \
   site/download/
```

二进制不进入 git（见同目录 `.gitignore`）；部署时把该目录与 `site/` 一起同步到
静态服务器即可。
