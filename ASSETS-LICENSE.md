# 视觉资产许可

> 项目**代码**仍按 MIT 许可（见 [`LICENSE`](LICENSE)）；**词库数据**与第三方表情/颜文字数据按各自上游许可（见 [`data/raw/LICENSES.md`](data/raw/LICENSES.md)）。
> 本文件只约束下面列出的**项目自绘视觉资产**：它们是**专有资产，不是开源内容**，版权归项目作者（erik.xyz），**商业使用需书面授权**。

## 覆盖范围

以下资产（含其任意尺寸、格式与修改版）适用本许可：

- **小欧形象与自有表情**：`shared/pet/stickers/*.png`、`shared/pet/stickers-gif/*.gif`，以及 `shared/pet/` 中小欧形象的视觉设计（外观、配色、形态）及其生成的位图/动画产物（含各端构建期拷贝与内嵌副本，如 `ios/PetStickerGifs.swift`、`harmony/.../PetStickerImages.ets`）。⚠️ `shared/pet/*.kt` 的**绘制代码仍为 MIT**；本许可只约束视觉设计与生成产物，不改变代码许可。
- **生成的贴纸映射表**：由上述项目自绘形象派生的标签/短码表 `shared/pet/emoji-font/map.tsv`、`shortcodes.tsv`，以及各端构建期拷贝与内嵌副本。PUA 彩色字体（`opi-emoji.ttf`）及其各端内嵌字体副本已于 2026-10-20 删除
- **应用图标**：Android `android/app/src/main/res/mipmap-*/ic_launcher*`、`mipmap-anydpi-v26/ic_launcher*.xml`、`drawable/ic_launcher_foreground.xml`；Windows `desktop/icons/opi-pet.ico`、`desktop/icons/opi-pet.png`、`crates/tsf-opi/assets/opi-langbar.ico`；macOS `macos/opi.tiff`；HarmonyOS `harmony/resources/base/media/icon.png`；以及 iOS / HarmonyOS / Windows / macOS 安装包中同一形象的应用图标
- **文档与 README 展示图**：`docs/opi-pet.svg`、`docs/opi-pet-forms.svg`、`docs/diagrams/*.svg`、`docs/social-preview*`，以及 `docs/` 下其它项目展示图和 README 中引用的展示图（赞赏码 `docs/alipay.png`、`docs/weixinpay.png` 除外）
- **随项目分发的其它自绘界面素材**：例如 fcitx5 皮肤中的 `crates/fcitx5-opi/data/themes/opi-*/panel.png`、`prev.png`、`next.png`

**不在覆盖范围**（仍按各自原有许可）：

- OpenMoji 表情数据与由它派生的表情表：**CC BY-SA 4.0**（署名 OpenMoji 项目及贡献者）
- rime_kaomoji_dict 颜文字：**MIT**（Copyright (c) 2023 aoguai）
- 词库数据：各自上游许可（见 [`data/raw/LICENSES.md`](data/raw/LICENSES.md)）
- `docs/alipay.png`、`docs/weixinpay.png` 赞赏码：不是本项目的美术作品
- 仓库源码：**MIT**（见 [`LICENSE`](LICENSE)）

## 允许

- **个人、非商业用途**：随**未修改的官方发行版**（官方构建的安装包 / 发布物）原样使用这些资产，无需另行申请。

## 需要书面授权

- 任何**商业使用**，包括但不限于：把资产或其修改版用于商业产品、商业服务、广告、付费内容、周边商品。
- 在商业产品中再分发、提取或修改这些资产。

商业授权请联系：<https://erik.xyz>

## 禁止

- 将资产或其修改版作为独立素材提取、再分发或售卖。
- 修改后将资产用于商业产品，或去除、隐藏版权与来源标识。
- 以任何方式暗示项目作者为你的产品背书。
