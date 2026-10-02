#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""旧版启动图标（API 21–25 的 mipmap-*/ic_launcher.png）生成器。

**为什么存在**：`minSdk = 21` ⇒ API 26+ 走自适应图标
（`res/mipmap-anydpi-v26/ic_launcher.xml`：R4 前景 + R6 底色），旧设备只能落回
PNG 密度桶。这两套形象必须一致，而 PNG 是**生成物**：只靠一句注释提醒「改矢量要重出
PNG」是没有执行点的警告（本仓 2026-09-28 之前正好相反 —— 那 5 个桶是脚手架带来的
**Flutter 默认图标**，注释却写着「两者同为键帽精灵形象」，一直没人看得见）。
所以生成逻辑进仓库，并带 `--check` 让「漂了」变成一条会红的命令。

**几何来源**：只读 R4（`android/.../drawable/ic_launcher_foreground.xml`），
把它**机械转成 SVG**（`android:pathData` 本就是 SVG path 语法；`android:viewportWidth/
Height` 定 viewBox），**不新写几何、不改 R4**。底色读 R6（`values/colors.xml` 的
`ic_launcher_background`），不硬编码 —— 否则又是一个真值面。

**可见区**：自适应图标渲染时把 108×108 画布裁到正中 72×72 再放大到图标尺寸，所以旧版
位图要铺满的正是那 72×72（不是整个 108 画布；照后者铺，旧设备上小欧会小一圈）。

**圆形**（API 25 的 `roundIcon`、API 26+ 的自适应圆形）：同一口径再加 r=36 的圆遮罩，
即 72 可见区的**内切圆** —— 这是系统圆形遮罩的真实尺寸（推导见 `ROUND_RADIUS`），
**不是** 66dp 安全圈（r=33，那是 `getSafeZone()` 的设计指引，当遮罩用会比系统实际显示的
小一圈，与 API 26+ 那套分叉）。朱砂天线最远到 r=34.87：真遮罩 r=36 内 1.13 ⇒ **裁不到**；
若按 r=33 则会削掉 1.87（对照数，不是本脚本的行为）。详见 R4 的注释。

用法：
    python3 scripts/gen_launcher_icons.py            # 重出 5 个方桶 + 5 个圆桶 + round XML
    python3 scripts/gen_launcher_icons.py --check    # 只比对，不写；不一致则退出码 1
"""
# ponytail: 依赖 rsvg-convert + PIL，与本仓既有 gen_*.py 同档（都是本机脚本，不是 CI 依赖）
import io
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
RES = ROOT / "android/app/src/main/res"
FOREGROUND = RES / "drawable/ic_launcher_foreground.xml"
COLORS = RES / "values/colors.xml"
AND = "{http://schemas.android.com/apk/res/android}"
BACKGROUND_NAME = "ic_launcher_background"

# 密度桶 → 边长（px）。安卓约定的 48dp 基准 × 各桶倍率。
BUCKETS = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}

# 自适应图标的可见窗口 = 108 画布的正中 72×72（外圈 18 是留给遮罩动画的余量）。
VISIBLE = 72.0

# 圆形遮罩半径（108 画布单位）：72 可见区的内切圆 = 36。
# 不是拍脑袋 —— AOSP AdaptiveIconDrawable 把 100×100 的 `config_icon_mask`
# 直接缩放到 bounds（= 可见区 72）上（`updateMaskBoundsInternal`：
# `setScale(b.width() / MASK_SIZE)`，`MASK_SIZE = 100`），
# 所以圆形遮罩 = mask 空间里**内切于 bounds** 的 r=50 圆 → 72 × 50/100 = 36。
# 出处：本机 SDK 源码 `sources/android-34/android/graphics/drawable/AdaptiveIconDrawable.java`
# 的 `MASK_SIZE` / `updateLayerBoundsInternal` / `updateMaskBoundsInternal`。
# ⚠️ `getSafeZone()` = 同一个遮罩 × `SAFEZONE_SCALE`(66/72) → r=33，那是**设计指引**
# （保证任何遮罩形状下都可见），拿它当遮罩会渲出比系统实际显示小一圈的圆。
ROUND_RADIUS = 36.0

# 产物：文件名 → 遮罩半径（None = 方形）。
VARIANTS = {"ic_launcher": None, "ic_launcher_round": ROUND_RADIUS}


def background_color() -> str:
    """R6：ic_launcher_background。**读出来**，不抄一份常量。"""
    root = ET.parse(COLORS).getroot()
    for c in root:
        if c.tag == "color" and c.get("name") == BACKGROUND_NAME:
            return c.text.strip()
    raise SystemExit(f"{COLORS} 里找不到 {BACKGROUND_NAME}")


def adaptive_xml() -> str:
    """API 26+ 的自适应圆图标。方形的 ic_launcher.xml 是等价的孪生（手写在 res 里）。"""
    return f"""<?xml version="1.0" encoding="utf-8"?>
<!--
  项目宠物「小欧」自适应启动图标（API 26+）的**圆形**孪生，供 manifest 的
  android:roundIcon 使用 —— 图形与 ic_launcher.xml 完全相同（同一前景 + 同一底色），
  圆形由系统遮罩给出，所以这里**不需要**任何几何改动。
  这是**生成物**：改 R4 前景或 R6 底色后跑 `python3 scripts/gen_launcher_icons.py` 重出。

  API 25（roundIcon 正是这一版引入的）用不了自适应图标，落回
  mipmap-*/ic_launcher_round.png：72dp 可见区的 r=36 内切圆，与圆形系统遮罩同径。
  API 24 及以下不读 roundIcon，走 android:icon 的方形那套。
-->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/{BACKGROUND_NAME}"/>
    <foreground android:drawable="@drawable/{FOREGROUND.stem}"/>
</adaptive-icon>
"""


def foreground_svg(size: int, mask_radius: float | None = None, crop: bool = True) -> str:
    """R4 → SVG。属性名映射而已，坐标一个数不改。"""
    root = ET.parse(FOREGROUND).getroot()
    if not root.tag.endswith("vector"):
        raise SystemExit(f"{FOREGROUND} 不是 <vector>（{root.tag}）")
    vw = float(root.get(AND + "viewportWidth"))
    vh = float(root.get(AND + "viewportHeight"))
    x, y, w, h = ((vw - VISIBLE) / 2, (vh - VISIBLE) / 2, VISIBLE, VISIBLE) if crop \
        else (0.0, 0.0, vw, vh)

    # 圆形版：把整张（含底色矩形的四角）裁进同心圆。圆画布中心 = viewBox 中心。
    clip = "" if mask_radius is None else (
        f'  <defs><clipPath id="m"><circle cx="{vw / 2}" cy="{vh / 2}"'
        f' r="{mask_radius}"/></clipPath></defs>\n'
        f'  <g clip-path="url(#m)">\n'
    )
    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {w} {h}"'
        f' width="{size}" height="{size}">\n',
        clip,
        # 自适应图标那层纯色 background：旧版位图里得自己铺满（PNG 没有 background 层）
        f'  <rect x="0" y="0" width="{vw}" height="{vh}" fill="{background_color()}"/>\n',
    ]
    for p in root:
        if not p.tag.endswith("path"):
            continue
        a = p.attrib
        attrs = [
            f'{dst}="{a[AND + src]}"'
            for src, dst in (
                ("fillColor", "fill"),
                ("strokeColor", "stroke"),
                ("strokeWidth", "stroke-width"),
                ("strokeLineCap", "stroke-linecap"),
                ("strokeLineJoin", "stroke-linejoin"),
            )
            if AND + src in a
        ]
        # ⚠️ 缺省值两边**不一样**：Android 的 fillColor 缺省 = 透明，SVG 的 fill 缺省 = 黑。
        # R4 的嘴只有 strokeColor（圆环 = 字母 O），不显式 fill="none" 就渲成一块实心黑饼。
        # 这脚 2026-09-28 踩过一次，且**外框 bbox 检查抓不到**（黑填充不改外框），只能靠看图。
        if AND + "fillColor" not in a:
            attrs.append('fill="none"')
        attrs.append(f'd="{a[AND + "pathData"]}"')
        out.append("  <path " + " ".join(attrs) + "/>\n")
    if mask_radius is not None:
        out.append("  </g>\n")
    return "".join(out) + "</svg>\n"


def render(size: int, mask_radius: float | None = None) -> bytes:
    """栅格化。rsvg-convert 由矢量直出（不靠放大位图），PIL 写 PNG（确定性：无 tIME 块）。"""
    with tempfile.TemporaryDirectory() as td:
        svg, png = Path(td) / "i.svg", Path(td) / "i.png"
        svg.write_text(foreground_svg(size, mask_radius), encoding="utf-8")
        subprocess.run(
            ["rsvg-convert", "-w", str(size), "-h", str(size), str(svg), "-o", str(png)],
            check=True,
        )
        buf = io.BytesIO()
        # 圆形版靠 alpha 让四角透掉；方形版维持不带 alpha 的 RGB（旧桶字节不变）。
        mode = "RGB" if mask_radius is None else "RGBA"
        Image.open(png).convert(mode).save(buf, format="PNG")
        return buf.getvalue()


def artifacts() -> list[tuple[Path, bytes]]:
    """全部生成物：方/圆 × 5 个密度桶 + API 26+ 的圆形自适应 XML。"""
    out = [
        (RES / f"mipmap-{bucket}/{name}.png", render(size, radius))
        for name, radius in VARIANTS.items()
        for bucket, size in BUCKETS.items()
    ]
    out.append((RES / "mipmap-anydpi-v26/ic_launcher_round.xml", adaptive_xml().encode()))
    return out


def main() -> int:
    check = "--check" in sys.argv[1:]
    bad = []
    for target, new in artifacts():
        rel = target.relative_to(ROOT)
        if check:
            old = target.read_bytes() if target.exists() else b""
            if old != new:
                bad.append(f"{rel} 与 R4/R6 不一致（重出会变）")
            continue
        target.write_bytes(new)
        print(f"{target.parent.name:18s} {target.name:22s} → {rel}")
    if check:
        if bad:
            print("图标与矢量/底色已经分叉，跑 `python3 scripts/gen_launcher_icons.py` 重出：")
            for b in bad:
                print("  " + b)
            return 1
        print(f"{len(VARIANTS) * len(BUCKETS) + 1} 个产物与 R4/R6 一致")
    return 0


if __name__ == "__main__":
    sys.exit(main())
