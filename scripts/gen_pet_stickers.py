#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""小欧表情**贴纸**生成器（PNG + 动画 GIF + 移动端内嵌数据）。

真源 = `shared/pet` 的 Compose 绘制（`OpiPet` 十二形态），**不是**第二套几何：
渲染入口是 `desktop/src/test/.../OpiPetStickerRenderTest`（280×280、透明底），
本脚本跑它、校验产物、把 12 张 PNG 拷进 `shared/pet/stickers/`。

动画 GIF（2026-10-19 用户要求「实在不行用 GIF」）：对同一批 PNG 做轻微弹跳 /
缩放 / 摆动，10 帧、200×200、90ms/帧、透明底，写到 `shared/pet/stickers-gif/`。
公式与 `shared/pet/PetMotion.kt` 的 `petMotionSpec` 同源（每帧取一个相位）。

iOS/鸿蒙没有可挂资源的构建工程，沿用既有的 **base64 常量内嵌**交付形态：
本脚本同时重写 `ios/PetStickerImages.swift`、`ios/PetStickerGifs.swift` 与
`harmony/ets/InputMethodExtensionAbility/PetStickerImages.ets`。别手改这些产物。

文件名契约 = `PetStickerAssets.stickerFile`（Android/桌面共用）：
`PetMood.name` 小写、下划线换连字符 + `.png` / `.gif`。构建期 desktop/android 的
`copyPetStickers` 再原样拷进资源/assets —— 盘上没有第二份入库副本。

校验（每张 PNG）：280×280、RGBA、四角 alpha=0（透明底）、中心 alpha=255、
不透明像素 > 3000（不是空表）。校验不过就不落盘。

用法：
    python3 scripts/gen_pet_stickers.py              # 渲染 + 校验 + 拷贝 + 重生成内嵌数据
    python3 scripts/gen_pet_stickers.py --no-render  # 跳过 Gradle，用 /tmp 上次结果
    python3 scripts/gen_pet_stickers.py --check      # 只校验，不渲染、不写盘
"""
import base64
import io
import math
import re
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / "desktop"
PROBE = Path("/tmp/opi-render-probe")
STICKERS = ROOT / "shared/pet/stickers"
GIFS = ROOT / "shared/pet/stickers-gif"
TEST = "xyz.erik.opi.pet.OpiPetStickerRenderTest"
SIZE = 280

GIF_SIZE = 200
GIF_FRAMES = 10
GIF_MS = 90

PET_KT = ROOT / "shared/pet/OpiPet.kt"
MAP_KT = ROOT / "shared/pet/PetEmojiMap.kt"
IOS_IMAGES = ROOT / "ios/PetStickerImages.swift"
IOS_GIFS = ROOT / "ios/PetStickerGifs.swift"
HARMONY_IMAGES = ROOT / "harmony/ets/InputMethodExtensionAbility/PetStickerImages.ets"


def gif_name(png_file: str) -> str:
    return png_file.removesuffix(".png") + ".gif"


def load_forms() -> list[tuple[str, str, str]]:
    """(mood, label, file) 按 PetMood 枚举序，排除 SLEEPY / DEGRADED。"""
    pet = PET_KT.read_text(encoding="utf-8")
    enum_match = re.search(r"enum class PetMood\s*\{(.*?)\n\}", pet, re.S)
    if not enum_match:
        raise SystemExit(f"{PET_KT} 里找不到 enum class PetMood")
    moods = re.findall(r"^\s*([A-Z][A-Z0-9_]*)\s*,", enum_match.group(1), re.M)

    pairs = re.findall(
        r'PetMood\.([A-Z][A-Z0-9_]*)\s+to\s+\("([^"]+)"\s+to\s+"([^"]+)"\)',
        MAP_KT.read_text(encoding="utf-8"),
    )
    labels = {mood: label for mood, label, _emoji in pairs}
    forms = []
    for mood in moods:
        if mood not in labels:
            continue
        file = mood.lower().replace("_", "-") + ".png"
        forms.append((mood, labels[mood], file))
    if len(forms) != 12:
        raise SystemExit(f"应有 12 个贴纸形态，解析出 {len(forms)} 个")
    unique = {file for _, _, file in forms}
    if len(unique) != len(forms):
        raise SystemExit("贴纸文件名重复")
    return forms


# 每个形态一个周期的位移/缩放/旋转。振幅单位是 280 设计像素。
# GIF 只有 10 帧：频率高于 ~3 周/周期会在采样点上混叠（实测哭脸只剩 2 帧），
# 所以这里用低倍频 + sin/cos 正交分量，保证每个采样点都不同。Compose 端的
# 高频抖动仍由 shared/pet/PetMotion.kt 在 60fps 下负责。
def motion(stem: str, phase: float) -> tuple[float, float, float, float, float]:
    a = 2 * math.pi * phase
    if stem == "idle":
        return 0, math.sin(a) * 2.0, 1, 1, math.cos(a) * 1.2
    if stem == "waiting":
        hop = abs(math.sin(a))
        return 0, -hop * 4.0, 1 + hop * 0.04, 1 - hop * 0.04, math.cos(a) * 1.5
    if stem == "crying":
        return math.sin(a * 2) * 1.5, 1, 1, 1, math.cos(a * 3) * 1.4
    if stem == "puzzled":
        return 0, 1 + math.sin(a) * 1.5, 1, 1, math.cos(a) * 2.2
    if stem == "terrified":
        return math.sin(a * 2) * 2.0, 0, 1, 1, math.cos(a * 3) * 1.5
    if stem == "angry":
        return math.sin(a * 2) * 3.0, 0, 1, 1, math.cos(a * 2) * 1.2
    if stem == "coquettish":
        return math.sin(a) * 1.5, 0, 1, 1, math.cos(a) * 2.0
    if stem == "cute":
        hop = abs(math.sin(a))
        return 0, -hop * 3.2, 1 + hop * 0.03, 1 - hop * 0.03, math.cos(a) * 2.0
    if stem == "pleasing":
        return math.cos(a) * 0.8, abs(math.sin(a)) * 2.4, 1, 1, math.cos(a) * 1.0
    if stem == "knocking":
        return 0, -abs(math.sin(a * 2)) * 2.4, 1, 1, math.sin(a * 2) * 1.2
    if stem == "face-palm":
        return math.sin(a * 2) * 1.6, 0, 1, 1, math.cos(a * 2) * 1.4
    if stem == "tongue-out":
        return 0, math.sin(a * 2) * 1.2, 1, 1, math.cos(a * 2) * 1.8
    return 0, 0, 1, 1, 0


def to_gif_frame(rgba: Image.Image) -> Image.Image:
    """RGBA → P 帧：索引 0 = 透明，其余 255 色（GIF 只有 1 位 alpha）。"""
    rgb = rgba.convert("RGB")
    q = rgb.quantize(colors=255, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    frame = Image.new("P", rgba.size, 0)
    frame.putpalette([0, 0, 0] + q.getpalette()[:765])
    frame.paste(q.point(lambda v: v + 1))
    transparent = rgba.getchannel("A").point(lambda v: 255 if v < 128 else 0)
    frame.paste(0, mask=transparent)
    return frame


def gif_bytes(path: Path) -> bytes:
    """从一张 280×280 PNG 生成确定性的动画 GIF 字节。"""
    with Image.open(path) as source:
        source.load()
        base = source.convert("RGBA")
    stem = path.stem
    scale = GIF_SIZE / SIZE
    frames = []
    for i in range(GIF_FRAMES):
        dx, dy, sx, sy, rot = motion(stem, i / GIF_FRAMES)
        width = max(1, round(GIF_SIZE * sx))
        height = max(1, round(GIF_SIZE * sy))
        canvas = Image.new("RGBA", (GIF_SIZE, GIF_SIZE), (0, 0, 0, 0))
        art = base.resize((width, height), Image.Resampling.LANCZOS)
        if rot:
            art = art.rotate(rot, resample=Image.Resampling.BICUBIC, expand=False)
        x = (GIF_SIZE - width) // 2 + round(dx * scale)
        y = (GIF_SIZE - height) // 2 + round(dy * scale)
        canvas.alpha_composite(art, (x, y))
        frames.append(to_gif_frame(canvas))
    buffer = io.BytesIO()
    frames[0].save(
        buffer,
        format="GIF",
        save_all=True,
        append_images=frames[1:],
        duration=GIF_MS,
        loop=0,
        disposal=2,
        # optimize=True 会合并相同的帧（waiting 实测只剩 8 帧），动画节奏会变。
        optimize=False,
        transparency=0,
    )
    return buffer.getvalue()


def verify_gif(data: bytes, name: str) -> None:
    with Image.open(io.BytesIO(data)) as gif:
        if gif.size != (GIF_SIZE, GIF_SIZE):
            raise SystemExit(f"{name}: GIF 尺寸 {gif.size}，应为 {GIF_SIZE}×{GIF_SIZE}")
        if gif.n_frames != GIF_FRAMES:
            raise SystemExit(f"{name}: GIF 帧数 {gif.n_frames}，应为 {GIF_FRAMES}")
        if gif.info.get("loop") != 0:
            raise SystemExit(f"{name}: GIF 不是无限循环")
        if gif.info.get("duration") != GIF_MS:
            raise SystemExit(f"{name}: GIF 帧时长 {gif.info.get('duration')}，应为 {GIF_MS}ms")
        gif.seek(0)
        first = gif.convert("RGBA")
        if first.getpixel((0, 0))[3] != 0 or first.getpixel((GIF_SIZE - 1, GIF_SIZE - 1))[3] != 0:
            raise SystemExit(f"{name}: GIF 四角不透明 —— 透明底没达成")
        if first.getpixel((GIF_SIZE // 2, GIF_SIZE // 2))[3] != 255:
            raise SystemExit(f"{name}: GIF 中心透明 —— 小欧没画出来")


def swift_base64_literal(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def embedded_swift(forms, pngs, gifs) -> dict[Path, bytes]:
    png_lines = [
        f'        "{file.removesuffix(".png")}": "{swift_base64_literal(pngs[file])}",'
        for _, _, file in forms
    ]
    gif_lines = [
        f'        "{file.removesuffix(".png")}": '
        f'"{swift_base64_literal(gifs[gif_name(file)])}",'
        for _, _, file in forms
    ]
    images = f'''// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// **生成文件 —— 别手改。** 源：shared/pet/stickers/*.png
// 重新生成：python3 scripts/gen_pet_stickers.py
//
// 「自有」表情的 12 张小欧贴纸 PNG（280×280、透明底）以 base64 内嵌。本仓没有
// Xcode 工程，资源没有落点，常量内嵌与 KaomojiData.swift 是同一种交付形态。

import UIKit

enum PetStickerImages {{

    /// 文件名（不含 `.png`）→ 280×280 透明底 PNG 的 base64。
    static let base64: [String: String] = [
{chr(10).join(png_lines)}
    ]

    /// base64 → `UIImage`；缺条目或数据损坏返回 nil（调用方提示缺失，不画占位字形）。
    static func image(named name: String) -> UIImage? {{
        guard let text = base64[name], let data = Data(base64Encoded: text) else {{ return nil }}
        return UIImage(data: data)
    }}
}}
'''
    gif_file = f'''// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// **生成文件 —— 别手改。** 源：shared/pet/stickers-gif/*.gif
// 重新生成：python3 scripts/gen_pet_stickers.py
//
// 12 张动画 GIF（200×200、10 帧、透明底）以 base64 内嵌：面板里用
// `animatedImage(named:)` 播动画，剪贴板用 `data(named:)` 写 `com.compuserve.gif`
// （UIPasteboard 支持 GIF；接收方不支持时仍可退回 `PetStickerImages.image`）。

import ImageIO
import UIKit

enum PetStickerGifs {{

    /// 文件名（不含 `.gif`）→ GIF 的 base64。
    static let base64: [String: String] = [
{chr(10).join(gif_lines)}
    ]

    /// base64 → GIF 原始字节；缺条目/损坏返回 nil（调用方退回 PNG）。
    static func data(named name: String) -> Data? {{
        guard let text = base64[name] else {{ return nil }}
        return Data(base64Encoded: text)
    }}

    /// GIF → 可动画 `UIImage`；解不出时退回静态首帧（至少不是空白）。
    static func animatedImage(named name: String) -> UIImage? {{
        guard let data = data(named: name) else {{ return nil }}
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else {{
            return UIImage(data: data)
        }}
        let count = CGImageSourceGetCount(source)
        var frames: [UIImage] = []
        var duration: Double = 0
        for index in 0..<count {{
            guard let cg = CGImageSourceCreateImageAtIndex(source, index, nil) else {{ continue }}
            frames.append(UIImage(cgImage: cg))
            duration += frameDuration(source, index)
        }}
        guard !frames.isEmpty else {{ return UIImage(data: data) }}
        return UIImage.animatedImage(with: frames, duration: max(duration, 0.1))
    }}

    /// GIF 帧时长；浏览器常见的 0 延迟按 100ms 处理（避免单帧 10ms 的闪烁）。
    private static func frameDuration(_ source: CGImageSource, _ index: Int) -> Double {{
        guard
            let props = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any],
            let gif = props[kCGImagePropertyGIFDictionary] as? [CFString: Any]
        else {{ return 0.1 }}
        let delay = (gif[kCGImagePropertyGIFUnclampedDelayTime] as? Double)
            ?? (gif[kCGImagePropertyGIFDelayTime] as? Double)
            ?? 0.1
        return delay < 0.02 ? 0.1 : delay
    }}
}}
'''
    return {IOS_IMAGES: images.encode("utf-8"), IOS_GIFS: gif_file.encode("utf-8")}


def embedded_harmony(forms, pngs, gifs) -> bytes:
    form_lines = "\n".join(
        f"  new PetStickerForm('{label}', '{file.removesuffix('.png')}'),"
        for _, label, file in forms
    )
    png_lines = "\n".join(
        f"  '{file.removesuffix('.png')}': '{swift_base64_literal(pngs[file])}',"
        for _, _, file in forms
    )
    gif_lines = "\n".join(
        f"  '{file.removesuffix('.png')}': "
        f"'{swift_base64_literal(gifs[gif_name(file)])}',"
        for _, _, file in forms
    )
    return f'''// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// **生成文件 —— 别手改。** 源：shared/pet/stickers/*.png + stickers-gif/*.gif
// 重新生成：python3 scripts/gen_pet_stickers.py
//
// 「自有」表情的 12 张小欧贴纸：PNG（280×280）与动画 GIF（200×200、10 帧）
// 都以 base64 内嵌。本端没有可构建的工程资源拷贝链路，沿用 KaomojiData.ets
// 的交付形态。
//
// 渲染：`Image(stickerGifDataUri(name))`（优先动图；ArkUI 是否逐帧播放 GIF
// 未上机验证）。剪贴板：优先 `image/gif` 字节，失败退回 PixelMap（见
// PetStickerClipboard.ets）。

/** 一个可点的自有贴纸：标签、文件名（不含扩展名）。 */
export class PetStickerForm {{
  label: string;
  sticker: string;

  constructor(label: string, sticker: string) {{
    this.label = label;
    this.sticker = sticker;
  }}
}}

/** 12 个形态（顺序 = PetEmojiMap.forms；文件 = PetStickerAssets.stickerFile）。 */
export const PET_STICKER_FORMS: PetStickerForm[] = [
{form_lines}
];

/** 文件名 → PNG base64。 */
const PNG_BASE64: Record<string, string> = {{
{png_lines}
}};

/** 文件名 → GIF base64。 */
const GIF_BASE64: Record<string, string> = {{
{gif_lines}
}};

/** 贴纸 PNG 的 base64（剪贴板退回 PixelMap 时用）。 */
export function stickerBase64(name: string): string {{
  return PNG_BASE64[name];
}}

/** 贴纸 PNG 的 data URI。 */
export function stickerDataUri(name: string): string {{
  return 'data:image/png;base64,' + PNG_BASE64[name];
}}

/** 贴纸 GIF 的 base64（剪贴板优先 GIF 时用）。 */
export function stickerGifBase64(name: string): string {{
  return GIF_BASE64[name];
}}

/** 贴纸 GIF 的 data URI（面板优先动图）。 */
export function stickerGifDataUri(name: string): string {{
  return 'data:image/gif;base64,' + GIF_BASE64[name];
}}
'''.encode("utf-8")


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
    forms = load_forms()
    expected_png = {file for _, _, file in forms}
    check_only = "--check" in sys.argv[1:]

    if not check_only and "--no-render" not in sys.argv[1:]:
        # 先清旧图：不然上一轮的命名/多出的文件会混进这一批。
        PROBE.mkdir(parents=True, exist_ok=True)
        for old in PROBE.glob("sticker-*.png"):
            old.unlink()
        render()

    if check_only:
        srcs = sorted(STICKERS.glob("*.png"))
        if {p.name for p in srcs} != expected_png:
            raise SystemExit(
                f"{STICKERS} 的 PNG 集合与契约不符："
                f"缺 {sorted(expected_png - {p.name for p in srcs})}，"
                f"多 {sorted({p.name for p in srcs} - expected_png)}"
            )
        for p in srcs:
            verify(p)
    else:
        srcs = sorted(PROBE.glob("sticker-*.png"))
        if len(srcs) != 12:
            raise SystemExit(
                f"{PROBE} 下有 {len(srcs)} 张 sticker-*.png，应为 12 张；"
                "先跑渲染（去掉 --no-render）"
            )
        for p in srcs:
            verify(p)
        STICKERS.mkdir(parents=True, exist_ok=True)
        for p in srcs:
            # /tmp 的 `sticker-` 前缀只是探针产物的命名空间，契约文件名没有它。
            shutil.copyfile(p, STICKERS / p.name.removeprefix("sticker-"))
        # 该目录是 assets 的整目录拷贝源：多一张不该有的 PNG 就会被打进包/资源。
        extra = {p.name for p in STICKERS.glob("*.png")} - expected_png
        if extra:
            raise SystemExit(f"{STICKERS} 里有契约外的 PNG（先清理）：{sorted(extra)}")

    pngs = {file: (STICKERS / file).read_bytes() for _, _, file in forms}
    gifs = {gif_name(file): gif_bytes(STICKERS / file) for _, _, file in forms}
    for name, data in gifs.items():
        verify_gif(data, name)

    if check_only:
        actual_gifs = {p.name for p in GIFS.glob("*.gif")}
        if actual_gifs != set(gifs):
            raise SystemExit(
                f"{GIFS} 的 GIF 集合与契约不符："
                f"缺 {sorted(set(gifs) - actual_gifs)}，多 {sorted(actual_gifs - set(gifs))}"
            )
        for name, data in gifs.items():
            if (GIFS / name).read_bytes() != data:
                raise SystemExit(f"{GIFS / name} 与 PNG 源不一致（重跑生成器）")
        embedded = {**embedded_swift(forms, pngs, gifs), HARMONY_IMAGES: embedded_harmony(forms, pngs, gifs)}
        stale = [str(p.relative_to(ROOT)) for p, data in embedded.items() if not p.is_file() or p.read_bytes() != data]
        if stale:
            raise SystemExit("内嵌数据缺失或过期：" + ", ".join(stale) + "（重跑生成器）")
        print(f"OK: {len(pngs)} 张 PNG、{len(gifs)} 张 GIF 与内嵌数据均与源一致")
        return 0

    GIFS.mkdir(parents=True, exist_ok=True)
    for name, data in gifs.items():
        (GIFS / name).write_bytes(data)
        print(f"  {name}")
    extra = {p.name for p in GIFS.glob("*.gif")} - set(gifs)
    if extra:
        raise SystemExit(f"{GIFS} 里有契约外的 GIF（先清理）：{sorted(extra)}")

    embedded = {**embedded_swift(forms, pngs, gifs), HARMONY_IMAGES: embedded_harmony(forms, pngs, gifs)}
    for path, data in embedded.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    print(f"{len(srcs)} 张贴纸 → {STICKERS.relative_to(ROOT)}")
    print(f"{len(gifs)} 张动画 GIF → {GIFS.relative_to(ROOT)}")
    print("内嵌数据 → " + ", ".join(str(p.relative_to(ROOT)) for p in embedded))
    return 0


if __name__ == "__main__":
    sys.exit(main())
