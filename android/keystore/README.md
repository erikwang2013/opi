# 小欧 Android 社区签名密钥库

`opi-community.jks` 是本仓库 **Android 分发包（sideload APK）的固定签名密钥**。
它随仓库公开，目的是让任何机器、任何 CI run 打出的 APK 使用同一张证书，
从而可以互相覆盖安装。

| 项 | 值 |
|---|---|
| 密钥库 | `android/keystore/opi-community.jks`（PKCS12） |
| alias | `opi-community` |
| storepass / keypass | `opi-community` |
| 算法 | RSA 2048，SHA256withRSA，有效期至 2126-09-26 |
| DN | `CN=opi community, O=opi, C=CN` |
| 证书 SHA-256 | `3fe2867ce790f5e472cf16381264eb69b2807c7e58256678a6cef70a3dcf00bf` |

## ⚠️ 这不是安全密钥

- **密码是公开的，私钥也在仓库里。** 任何人都能伪造一个签名相同的 APK。
  它只用于「可复现的 sideload 升级」，**不构成任何信任或防篡改保证**。
- **不要用它上架任何应用商店**（Google Play 等要求自己保管的私有上传密钥）。
- 不要把它挪作他用，也不要更换密码 / 重新生成 —— 换 key 会让所有已装用户
  再次无法覆盖安装（见下）。

## 迁移：从旧 debug 签名切过来，用户必须卸载一次

Android 不允许「同包名、不同证书」覆盖安装（`INSTALL_FAILED_UPDATE_INCOMPATIBLE`）。
1.17.1 及更早的发布 APK 用的是构建机各自的 debug 签名，证书与这张不同。因此：

- **已装旧版的用户**：必须先卸载一次（`adb uninstall xyz.erik.opi`，或系统设置里卸载；
  会连应用数据 / 用户词一起删掉），再安装新 APK。
- **从 1.18.0 起**：所有版本都用这张证书，之后可直接覆盖升级。

## 构建与验证

`android/app/build.gradle.kts` 的 `signingConfigs.release` 已指向本文件，
`assembleRelease`（本地与 CI）自动使用：

```bash
cd android && ./gradlew :app:assembleRelease
apksigner verify --print-certs app/build/outputs/apk/release/app-release.apk
```

输出应包含：

```
Signer #1 certificate DN: CN=opi community, O=opi, C=CN
Signer #1 certificate SHA-256 digest: 3fe2867ce790f5e472cf16381264eb69b2807c7e58256678a6cef70a3dcf00bf
```

CI（`.github/workflows/android-release.yml`）也会断言这个指纹，防止悄悄退回 debug 签名。
`assembleDebug` 仍用本机 debug 密钥库（仅本地调试，勿分发）。
