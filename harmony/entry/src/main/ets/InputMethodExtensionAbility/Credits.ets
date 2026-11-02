// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 开源致谢（2026-11-02 用户裁决「所有关于页面增加开源致谢」）。条目与 Android/
// 桌面/iOS/macOS 各端同一份；只列项目真的使用的上游，不确定的写「及相关贡献者」。
// 逐条数据许可见 `data/raw/LICENSES.md`。

export interface CreditSection {
  title: string;
  items: string[];
}

export const OPEN_SOURCE_CREDITS: CreditSection[] = [
  {
    title: '项目',
    items: [
      '小欧输入法（opi）源代码以 MIT 许可开源；词库数据与视觉资产按各自许可分发。',
    ],
  },
  {
    title: 'Rust 与开源 crate',
    items: [
      'Rust 语言、标准库与 Cargo 工具链（Rust 项目及其贡献者）',
      'serde / serde_json（数据序列化）',
      'memmap2（内存映射词库）',
      'jni（Android JNI 绑定）',
      'snow / x25519-dalek（Noise 协议与密钥交换）',
      'postcard / sha2 / hex / getrandom / socket2（跨设备剪贴板）',
      'thiserror（错误处理）',
      'windows-rs（Windows TSF / IMM32 绑定）',
      'sherpa-rs（sherpa-onnx 的 Rust 绑定）',
      'proptest / hound（测试与音频读取，开发期）',
      '以及 Cargo.lock 中的全部传递依赖及其贡献者',
    ],
  },
  {
    title: '词库与数据来源',
    items: [
      'rime-ice（雾凇拼音）、rime-essay、rime-luna-pinyin、rime-pinyin-simp、' +
        'rime-terra-pinyin、rime-stroke、rime-wubi、rime-cangjie',
      'chinese-xinhua（成语与歇后语）',
      'THUOCL（清华大学开放中文词库）',
      '康熙字典数据（shunshi-kangxi-core，Shunshi.AI）',
      'OpenCC（简繁转换）、Unicode Unihan 与 Unicode 字符数据库',
      'Google Books Ngram Frequency（英文联想词源）',
      '百家姓（公有领域）与项目人工整理表',
      '及相关贡献者；逐条来源与许可见 data/raw/LICENSES.md',
    ],
  },
  {
    title: '平台与框架',
    items: [
      'Android：Kotlin、Jetpack Compose、AndroidX',
      'Apple：Swift、UIKit、InputMethodKit、AppKit',
      'HarmonyOS：ArkTS、ArkUI、HarmonyOS SDK',
      'Linux：fcitx5',
      'Windows：Text Services Framework（TSF）、IMM32、Win32',
      '桌面：Compose for Desktop、Skiko、JNA',
    ],
  },
  {
    title: '表情与颜文字',
    items: [
      'OpenMoji（CC BY-SA 4.0，OpenMoji 项目及贡献者）',
      'rime_kaomoji_dict（MIT，Copyright (c) 2023 aoguai）',
      'Unicode emoji 数据（Unicode 联盟）',
    ],
  },
  {
    title: '语音识别',
    items: [
      'sherpa-onnx（k2-fsa 项目及贡献者）',
      'SenseVoice（FunAudioLLM / 阿里通义实验室及贡献者）',
      'Whisper（OpenAI 及贡献者）',
      'Silero VAD（Silero 团队及贡献者）',
      'ONNX Runtime（Microsoft 及贡献者）',
    ],
  },
  {
    title: '跨设备传输',
    items: [
      'LocalSend（跨设备传输的交互与协议思路参考；LocalSend 项目及贡献者）',
    ],
  },
  {
    title: '其他',
    items: [
      '感谢所有在 GitHub、crates.io、Maven Central 等社区提交代码、数据与问题报告的贡献者。',
    ],
  },
];
