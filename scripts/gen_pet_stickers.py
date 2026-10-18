#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""小欧表情**贴纸** PNG 生成器（「自有」表情页插入的图片）。

真源 = `shared/pet` 的 Compose 绘制（`OpiPet` 十二形态），**不是**第二套几何：
渲染入口是 `desktop/src/test/.../OpiPetStickerRenderTest`（280×280、透明底），
本脚本跑它、校验产物、把 12 张 PNG 拷进 `shared/pet/stickers/`。

文件名契约 = `PetStickerAssets.stickerFile`（Android/桌面共用）：
`PetMood.name` 小写、下划线换连字符 + `.png`。构建期 desktop/android 的
`copyPetStickers` 再原样拷进资源/assets —— 盘上没有第二份入库副本。
iOS/鸿蒙消费同一批 PNG 生成内嵌数据。

校验（每张）：280×280、RGBA、四角 alpha=0（透明底）、中心 alpha=255、
不透明像素 > 3000（不是空表）。校验不过就不落盘。

用法：
    python3 scripts/gen_pet_stickers.py              # 渲染 + 校验 + 拷贝（常规）
    python3 scripts/gen_pet_stickers.py --no-render  # 跳过 Gradle，用 /tmp 上次结果
"""
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / "desktop"
PROBE = Path("/tmp/opi-render-probe")
STICKERS = ROOT / "shared/pet/stickers"
TEST = "xyz.erik.opi.pet.OpiPetStickerRenderTest"
SIZE = 280


def render() -> None:
    """跑桌面渲染测试（透明底 PNG 落到 /tmp/opi-render-probe/sticker-*.png）。

    `--rerun` 强制测试执行：输入没变时 `test` 会 UP-TO-DATE、不重写 /tmp 的图。
    """
    cmd = [
        str(DESKTOP / "gradlew"), "--console=plain",
        "test", "--tests", TEST, "--rerun",
    ]
    print("→", " ".join(cmd))
    subprocess.run(cmd, cwd=DESKTOP, check=True)


def verify(path: Path) -> None:
    """贴纸特有的硬判据；不合格直接退出，不留下半批产物。"""
    with Image.open(path) as im:
        im.load()
        if im.size != (SIZE, SIZE):
            raise SystemExit(f"{path.name}: 尺寸 {im.size}，应为 {SIZE}×{SIZE}")
        rgba = im.convert("RGBA")
    if rgba.getpixel((0, 0))[3] != 0 or rgba.getpixel((SIZE - 1, SIZE - 1))[3] != 0:
        raise SystemExit(f"{path.name}: 四角不透明 —— 白底漏进来了，透明底没达成")
    if rgba.getpixel((SIZE // 2, SIZE // 2))[3] != 255:
        raise SystemExit(f"{path.name}: 中心透明 —— 小欧没画出来")
    # alpha 直方图：0 号桶是全透明，其余各桶之和 = 有内容的像素数。
    opaque = sum(rgba.getchannel("A").histogram()[1:])
    if opaque <= 3000:
        raise SystemExit(f"{path.name}: 不透明像素只有 {opaque}，脸是空的")


def main() -> int:
    if "--no-render" not in sys.argv[1:]:
        # 先清旧图：不然上一轮的命名/多出的文件会混进这一批。
        PROBE.mkdir(parents=True, exist_ok=True)
        for old in PROBE.glob("sticker-*.png"):
            old.unlink()
        render()

    srcs = sorted(PROBE.glob("sticker-*.png"))
    if len(srcs) != 12:
        raise SystemExit(
            f"{PROBE} 下有 {len(srcs)} 张 sticker-*.png，应为 12 张；"
            "先跑渲染（去掉 --no-render）"
        )
    for p in srcs:
        verify(p)

    STICKERS.mkdir(parents=True, exist_ok=True)
    expected = set()
    for p in srcs:
        # /tmp 的 `sticker-` 前缀只是探针产物的命名空间，契约文件名没有它。
        dest = STICKERS / p.name.removeprefix("sticker-")
        shutil.copyfile(p, dest)
        expected.add(dest.name)
        print(f"  {dest.name}")
    # 该目录是 assets 的整目录拷贝源：多一张不该有的 PNG 就会被打进包/资源。
    extra = {p.name for p in STICKERS.glob("*.png")} - expected
    if extra:
        raise SystemExit(f"{STICKERS} 里有契约外的 PNG（先清理）：{sorted(extra)}")
    print(f"{len(srcs)} 张贴纸 → {STICKERS.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
