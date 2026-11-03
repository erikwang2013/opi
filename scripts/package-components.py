#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""三档打包的组件准备（2026-11-01）。

产出：
  <out>/asr-bundle/asr/{sensevoice,whisper-small.en,vad}/...  完整包/桌面镜像用
  <out>/components/dicts/opi-dicts-<版本>.zip                  基础包设置页下载（含拼音主词库）
  <out>/components/asr/*.zip                                  普通/基础包按需下载（可选）

模型不进 git；源目录由 --models 指到本机已下载的模型根（含 sensevoice、
whisper-small.en、vad 三个子目录与各自的 manifest.json）。本脚本只搬运与打包，
不联网、不改模型。字典包直接取 Android 分发副本。
"""
import argparse
import hashlib
import json
import os
import shutil
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# 基础包只含 app 代码（2026-11-03 用户裁决）：拼音主词库也在这里下载。
DICT_FILES = ["luna.opid", "trad.opid", "stroke.opid", "wubi86.opid", "wubi98.opid",
              "rare_index.tsv", "kangxi.tsv"]
MODEL_PACKS = ["sensevoice", "whisper-small.en", "vad"]
LICENSE = """# 语音模型许可与署名

本目录的模型随“完整包”分发；普通包/基础包从设置页下载同一批文件。

- SenseVoiceSmall（sherpa-onnx 转换版）：FunASR Model Open Source License v1.1，
  上游 https://github.com/FunAudioLLM/SenseVoice 与
  https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17
- Whisper small.en（int8 ONNX，sherpa-onnx 转换版）：OpenAI Whisper 权重 MIT，
  https://huggingface.co/csukuangfj/sherpa-onnx-whisper-small.en
- Silero VAD：MIT，https://github.com/snakers4/silero-vad
- sherpa-onnx 运行时：Apache-2.0，https://github.com/k2-fsa/sherpa-onnx

模型文件由下载器逐文件 SHA-256 校验；本目录另附 manifest.json（Rust 加载器读取）。
"""


def copy_file(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists():
        dst.unlink()
    try:
        os.link(src, dst)
    except OSError:
        shutil.copy2(src, dst)


def stage_models(models: Path, out: Path) -> None:
    root = out / "asr-bundle" / "asr"
    for pack in MODEL_PACKS:
        src = models / pack
        if not src.is_dir():
            raise SystemExit(f"缺少模型目录：{src}")
        for f in src.iterdir():
            if f.is_file() and not f.name.endswith(".aria2"):
                copy_file(f, root / pack / f.name)
    (root / "LICENSES.md").write_text(LICENSE, encoding="utf-8")


def build_dict_zip(out: Path, version: str) -> Path:
    assets = ROOT / "android" / "app" / "src" / "main" / "assets"
    dst = out / "components" / "dicts" / f"opi-dicts-{version}.zip"
    dst.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for name in DICT_FILES:
            src = assets / name
            if not src.is_file():
                raise SystemExit(f"缺少词库文件：{src}")
            z.write(src, arcname=name)
    return dst


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", type=Path, required=True)
    ap.add_argument("--out", type=Path, default=ROOT / "dist" / "components")
    ap.add_argument("--version", default=(ROOT / "site" / "version.json").exists()
                    and json.loads((ROOT / "site" / "version.json").read_text())["version"]
                    or "0.0.0")
    args = ap.parse_args()
    out = args.out
    stage_models(args.models, out)
    z = build_dict_zip(out, args.version)
    info = {
        "version": args.version,
        "dicts": {"name": z.name, "size": z.stat().st_size, "sha256": sha256(z)},
    }
    print(json.dumps(info, ensure_ascii=False, indent=2))
    print(f"模型目录：{(out / 'asr-bundle' / 'asr')}")


if __name__ == "__main__":
    main()
