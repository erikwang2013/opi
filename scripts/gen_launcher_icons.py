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

用法：
    python3 scripts/gen_launcher_icons.py            # 重出 5 个桶
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

# 密度桶 → 边长（px）。安卓约定的 48dp 基准 × 各桶倍率。
BUCKETS = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}

# 自适应图标的可见窗口 = 108 画布的正中 72×72（外圈 18 是留给遮罩动画的余量）。
VISIBLE = 72.0


def background_color() -> str:
    """R6：ic_launcher_background。**读出来**，不抄一份常量。"""
    root = ET.parse(COLORS).getroot()
    for c in root:
        if c.tag == "color" and c.get("name") == "ic_launcher_background":
            return c.text.strip()
    raise SystemExit(f"{COLORS} 里找不到 ic_launcher_background")


def foreground_svg(size: int, crop: bool = True) -> str:
    """R4 → SVG。属性名映射而已，坐标一个数不改。"""
    root = ET.parse(FOREGROUND).getroot()
    if not root.tag.endswith("vector"):
        raise SystemExit(f"{FOREGROUND} 不是 <vector>（{root.tag}）")
    vw = float(root.get(AND + "viewportWidth"))
    vh = float(root.get(AND + "viewportHeight"))
    x, y, w, h = ((vw - VISIBLE) / 2, (vh - VISIBLE) / 2, VISIBLE, VISIBLE) if crop \
        else (0.0, 0.0, vw, vh)

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {w} {h}"'
        f' width="{size}" height="{size}">\n',
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
    return "".join(out) + "</svg>\n"


def render(size: int) -> bytes:
    """栅格化。rsvg-convert 由矢量直出（不靠放大位图），PIL 写 PNG（确定性：无 tIME 块）。"""
    with tempfile.TemporaryDirectory() as td:
        svg, png = Path(td) / "i.svg", Path(td) / "i.png"
        svg.write_text(foreground_svg(size), encoding="utf-8")
        subprocess.run(
            ["rsvg-convert", "-w", str(size), "-h", str(size), str(svg), "-o", str(png)],
            check=True,
        )
        buf = io.BytesIO()
        Image.open(png).convert("RGB").save(buf, format="PNG")
        return buf.getvalue()


def main() -> int:
    check = "--check" in sys.argv[1:]
    bad = []
    for bucket, size in BUCKETS.items():
        target = RES / f"mipmap-{bucket}/ic_launcher.png"
        new = render(size)
        if check:
            old = target.read_bytes() if target.exists() else b""
            if old != new:
                bad.append(f"{bucket}: {target.name} 与 R4/R6 不一致（重出会变）")
            continue
        target.write_bytes(new)
        print(f"{bucket:8s} {size}x{size} → {target.relative_to(ROOT)}")
    if check:
        if bad:
            print("图标与矢量/底色已经分叉，跑 `python3 scripts/gen_launcher_icons.py` 重出：")
            for b in bad:
                print("  " + b)
            return 1
        print(f"{len(BUCKETS)} 个桶与 R4/R6 一致")
    return 0


if __name__ == "__main__":
    sys.exit(main())
