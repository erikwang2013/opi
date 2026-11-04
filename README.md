**[English](README.en.md) · [中文](README.md)**

# Open People Input (OPI)

中文名**小欧**（也是项目宠物名），英文名 **opi**。一款回归本质、人人可用的多端输入法。

> **官网**：[opi.erik.xyz](https://opi.erik.xyz) · **下载**：[GitHub Releases](https://github.com/erikwang2013/opi/releases) · **安装文档**：[docs/install/](docs/install/README.md)

[![OPI 官网首页](docs/site-preview.png)](https://opi.erik.xyz)

## 下载与安装

| 平台 | 产物 | 安装 |
|---|---|---|
| Android | APK（完整 / 普通 / 基础） | 见 [Android 安装](docs/install/android.md) |
| Windows | EXE（完整 / 普通 / 基础） | 见 [Windows 安装](docs/install/windows.md) |
| Linux | DEB / RPM（完整 / 普通 / 基础） | 见 [Linux 安装](docs/install/linux.md) |
| macOS | PKG（内测，unsigned） | 见 [macOS](docs/install/macos.md) |
| iOS | IPA（内测，unsigned） | 见 [iOS](docs/install/ios.md) |
| HarmonyOS | HAP（内测，unsigned） | 见 [HarmonyOS](docs/install/harmony.md) |

安装包分三档：

- **完整包**：语音引擎、SenseVoice/Whisper 模型与全部词库随包提供。
- **普通包**：全部词库随包；语音引擎与模型在设置页按需下载。
- **基础包**：app 代码 + 拼音主词库；扩展词库、语音引擎与模型按需下载。

Linux 装完必须把 OPI 加进 fcitx5 输入法组，见 [Linux 安装 §4](docs/install/linux.md)。

## 核心

- **一份引擎，六端原生客户端**：Android、Windows、Linux、iOS、macOS、鸿蒙。
- **本地优先**：输入、剪贴板与语音识别默认只在本机；无账号、无广告、无遥测。
- **可核对的隐私**：依赖表与代码开源；关于页提供应用内隐私政策。
- **跨设备传输**：局域网发现与加密会话，文件/文件夹/照片/文本/剪贴板；多设备可指定收发对象。
- **本地语音**：SenseVoice / Whisper 本机识别，音频不落盘、不上传。
- **可验证的性能**：引擎热路径、首帧与候选取回均有测试与预算，见 [性能记录](docs/performance.md)。

## 验证边界

各端验证到哪一层、哪些仍未上机，统一以 [docs/install/README.md](docs/install/README.md) 对应章节为准。iOS / macOS / 鸿蒙目前是可构建的内测产物，签名与真机运行仍需自行验证。

## 开发

```bash
cargo test --locked -p engine-core -p engine-data -p opi-clip
./desktop/gradlew -p desktop test
./android/gradlew -p android testDebugUnitTest
```

构建、打包与各端工具链见 [docs/install/](docs/install/README.md)；站点真源在 [`site/`](site/index.html)。

## 许可证

- **代码**：MIT，见 [`LICENSE`](LICENSE)。
- **词库数据**：与代码分开授权，逐条来源与许可见 [`data/raw/LICENSES.md`](data/raw/LICENSES.md)。
- **视觉资产**：小欧形象、贴纸、图标与文档展示图为项目专有资产，商业使用需书面授权，见 [`ASSETS-LICENSE.md`](ASSETS-LICENSE.md)。

<!-- assets-license:start -->
## 视觉资产授权 / Visual Assets License

小欧形象、自有表情、贴纸映射表、应用图标与文档展示图是**专有资产，不是开源内容**：商业使用需取得书面授权；个人非商业用途可随未修改的官方发行版原样使用。代码仍为 MIT；OpenMoji（CC BY-SA 4.0）与颜文字（MIT）按各自上游许可。详见 [ASSETS-LICENSE.md](ASSETS-LICENSE.md)。
<!-- assets-license:end -->
