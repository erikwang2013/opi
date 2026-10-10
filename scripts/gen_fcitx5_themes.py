#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""生成 OPI 皮肤（fcitx5 classicui 主题目录）。

产物：crates/fcitx5-opi/data/themes/opi-{light,dark,paper,cinnabar,
fresh,azure,lemon,sunset,starlight}/
  theme.conf            配色。**段落结构对照本机装着的 deepin-light**（那是我
                        们能核实的、真跑在用户机器上的主题格式），值换成本项目
                        的色板；缺段落的主题并非一定坏，但没人验过的省写就是
                        赌 —— 不省。
  panel.png  prev/next  面板背景与翻页箭头。背景走 9-slice：四角 24px 不拉伸
                        （[InputPanel/Background/Margin] = 24），故圆角半径
                        (14) 必须 ≤ 24，改这两个数时一起看。
                        绘制做 4x 超采样再缩 —— PIL 的 rounded_rectangle 无
                        抗锯齿，直接画是锯齿角。

用法：
  python3 scripts/gen_fcitx5_themes.py          # 生成 / 覆盖
  python3 scripts/gen_fcitx5_themes.py --check  # 只验：逐字节等于重新生成、
                                                # 且圆角/底色结构断言成立

⚠️ 目录名 = opi_config.h 的 OpiSkin 枚举映射（opi_skin.h 的 opiSkinThemeName）、
CMakeLists 的安装清单、本脚本 —— **四处**必须同名；改名 = 皮肤静默失效
（classicui 找不到主题目录时会回落默认主题，不报错）。
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parent.parent
OUT_ROOT = REPO / "crates" / "fcitx5-opi" / "data" / "themes"

# 9-slice 角尺寸与圆角半径（见文件头：半径 ≤ 角尺寸）。
CORNER = 24
RADIUS = 14
PANEL_PX = 80
ARROW_PX = 24
SUPERSAMPLE = 4

SKINS = {
    "light": {
        "title": "opi输入法 浅色",
        "desc": "白底 + 朱砂高亮",
        "bg": "#FFFFFF",
        "border": "#00000022",
        "text": "#1F1F1F",
        "accent": "#C0392B",
        "hl_bg": "#C0392B1F",
        "menu_bg": "#FFFFFF",
        "sep": "#D8D8D8",
    },
    "dark": {
        "title": "opi输入法 深色",
        "desc": "深底 + 浅朱砂高亮",
        "bg": "#1F1F1F",
        "border": "#FFFFFF22",
        "text": "#E6E6E6",
        "accent": "#FF8A80",
        "hl_bg": "#FF8A8026",
        "menu_bg": "#262626",
        "sep": "#4A4A4A",
    },
    "paper": {
        "title": "opi输入法 宣纸",
        "desc": "米色底 + 茶褐高亮",
        "bg": "#F5EFE1",
        "border": "#3D322222",
        "text": "#3D3222",
        "accent": "#8A5A2B",
        "hl_bg": "#8A5A2B1F",
        "menu_bg": "#F5EFE1",
        "sep": "#D8CDB4",
    },
    "cinnabar": {
        "title": "opi输入法 朱砂",
        "desc": "朱砂底 + 米白文字",
        "bg": "#B3272D",
        "border": "#00000033",
        "text": "#FFF6EC",
        "accent": "#FFD9A0",
        "hl_bg": "#FFFFFF26",
        "menu_bg": "#9E2228",
        "sep": "#CC5A5F",
    },
    # 2026-10-10 加五款（用户裁决「所有端皮肤增加颜色」）；键名/顺序与
    # desktop 的 SkinOption、Android 的 SkinOption.id、opi_config.h 的 OpiSkin
    # 一处不错位（SettingsTest 的 `skin names stay in lockstep` 现读现比）。
    # 每款 text×bg ≥ 4.5、accent×bg ≥ 3（--check 不测这个，值定稿时用 WCAG 手核过）。
    "fresh": {
        "title": "opi输入法 清爽",
        "desc": "浅青绿底 + 墨绿文字",
        "bg": "#E6F5EE",
        "border": "#16332B22",
        "text": "#16332B",
        "accent": "#2E7D6B",
        "hl_bg": "#2E7D6B26",
        "menu_bg": "#D8EEE3",
        "sep": "#B9D8CB",
    },
    "azure": {
        "title": "opi输入法 湛蓝",
        "desc": "深天蓝底 + 月白文字",
        "bg": "#3A679E",
        "border": "#00000033",
        "text": "#F4F8FD",
        "accent": "#FFE3A8",
        "hl_bg": "#FFFFFF26",
        "menu_bg": "#33598A",
        "sep": "#6E96C4",
    },
    "lemon": {
        "title": "opi输入法 柠檬",
        "desc": "柠檬黄底 + 深橄榄文字",
        "bg": "#FBF6C8",
        "border": "#3A300022",
        "text": "#3A3000",
        "accent": "#6B5700",
        "hl_bg": "#6B570026",
        "menu_bg": "#F2E9A4",
        "sep": "#D9C97E",
    },
    "sunset": {
        "title": "opi输入法 晚霞",
        "desc": "暖橙粉底 + 深赭文字",
        "bg": "#FBE0CD",
        "border": "#5A2C1E22",
        "text": "#5A2C1E",
        "accent": "#B8431F",
        "hl_bg": "#B8431F26",
        "menu_bg": "#F3C6A8",
        "sep": "#E2AC8C",
    },
    "starlight": {
        "title": "opi输入法 星光",
        "desc": "靛蓝夜空底 + 星金高亮",
        "bg": "#1B2138",
        "border": "#FFFFFF1F",
        "text": "#E8EBF7",
        "accent": "#FFD966",
        "hl_bg": "#FFD96633",
        "menu_bg": "#232A44",
        "sep": "#3A4468",
    },
}

# 段落结构对照 /usr/share/fcitx5/themes/deepin-light/theme.conf（本机实装）。
# {xxx} 占位由 SKINS 表填；其余一个值都不动。
THEME_CONF = """# OPI 皮肤 —— 由 scripts/gen_fcitx5_themes.py 生成，别手改（--check 会对账）。
# 面板由 fcitx5 的 classicui 绘制；本文件是 classicui 的主题格式。
[Metadata]
Name={title}
Version=1
Author=OPI
Description=opi输入法 {desc}
ScaleWithDPI=True

[InputPanel]
NormalColor={text}
HighlightColor=#FFFFFF
HighlightBackgroundColor=#FFFFFF
HighlightCandidateColor={accent}
EnableBlur=False
BlurMask=
FullWidthHighlight=True
PageButtonAlignment=Bottom

[InputPanel/BlurMargin]
Left=0
Right=0
Top=0
Bottom=0

[InputPanel/Background]
Image=panel.png
Color={bg}
BorderColor={border}
BorderWidth=0
Overlay=
Gravity=Center
OverlayOffsetX=0
OverlayOffsetY=0
HideOverlayIfOversize=True

[InputPanel/Background/Margin]
Left={corner}
Right={corner}
Top={corner}
Bottom={corner}

[InputPanel/Background/OverlayClipMargin]
Left=0
Right=0
Top=0
Bottom=0

[InputPanel/Highlight]
Image=
Color={hl_bg}
BorderColor=#ffffff00
BorderWidth=0
Overlay=
Gravity=Center
OverlayOffsetX=0
OverlayOffsetY=0
HideOverlayIfOversize=False

[InputPanel/Highlight/Margin]
Left=12
Right=12
Top=10
Bottom=10

[InputPanel/Highlight/OverlayClipMargin]
Left=0
Right=0
Top=0
Bottom=0

[InputPanel/Highlight/HighlightClickMargin]
Left=0
Right=0
Top=0
Bottom=0

[InputPanel/ContentMargin]
Left=12
Right=12
Top=12
Bottom=13

[InputPanel/TextMargin]
Left=12
Right=12
Top=6
Bottom=8

[InputPanel/PrevPage]
Image=prev.png

[InputPanel/PrevPage/ClickMargin]
Left=10
Right=0
Top=4
Bottom=4

[InputPanel/NextPage]
Image=next.png

[InputPanel/NextPage/ClickMargin]
Left=5
Right=5
Top=4
Bottom=4

[InputPanel/ShadowMargin]
Left=2
Right=2
Top=2
Bottom=2

[Menu]
NormalColor={text}
HighlightCandidateColor=#FFFFFF
Spacing=0

[Menu/Background]
Image=
Color={menu_bg}
BorderColor={border}
BorderWidth=0
Overlay=
Gravity="Top Left"
OverlayOffsetX=0
OverlayOffsetY=0
HideOverlayIfOversize=False

[Menu/Background/Margin]
Left=2
Right=2
Top=2
Bottom=2

[Menu/Background/OverlayClipMargin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/Highlight]
Image=
Color={hl_bg}
BorderColor=#ffffff00
BorderWidth=0
Overlay=
Gravity="Top Left"
OverlayOffsetX=0
OverlayOffsetY=0
HideOverlayIfOversize=False

[Menu/Highlight/Margin]
Left=5
Right=5
Top=5
Bottom=5

[Menu/Highlight/OverlayClipMargin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/Separator]
Image=
Color={sep}
BorderColor=#ffffff00
BorderWidth=0
Overlay=
Gravity=Center
OverlayOffsetX=0
OverlayOffsetY=0
HideOverlayIfOversize=False

[Menu/Separator/Margin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/Separator/OverlayClipMargin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/CheckBox]
Image=
Color={menu_bg}
BorderColor=#ffffff00
BorderWidth=0
Overlay=
Gravity="Top Left"
OverlayOffsetX=0
OverlayOffsetY=0
HideOverlayIfOversize=False

[Menu/CheckBox/Margin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/CheckBox/OverlayClipMargin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/SubMenu]
Image=
Color={menu_bg}
BorderColor=#ffffff00
BorderWidth=0
Overlay=
Gravity="Top Left"
OverlayOffsetX=0
OverlayOffsetY=0
HideOverlayIfOversize=False

[Menu/SubMenu/Margin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/SubMenu/OverlayClipMargin]
Left=0
Right=0
Top=0
Bottom=0

[Menu/ContentMargin]
Left=2
Right=2
Top=2
Bottom=2

[Menu/TextMargin]
Left=5
Right=5
Top=5
Bottom=5

[AccentColorField]
0="Input Panel Border"
1="Input Panel Highlight Candidate Background"
2="Input Panel Highlight"
3="Menu Border"
4="Menu Separator"
5="Menu Selected Item Background"
"""


def rgba(spec: str):
    """#RRGGBB 或 #RRGGBBAA → (r, g, b, a)。classicui 的 8 位写法与它同序。"""
    spec = spec.lstrip("#")
    r, g, b = int(spec[0:2], 16), int(spec[2:4], 16), int(spec[4:6], 16)
    a = int(spec[6:8], 16) if len(spec) == 8 else 255
    return (r, g, b, a)


def rounded_panel(skin) -> Image.Image:
    n = PANEL_PX * SUPERSAMPLE
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle(
        [0, 0, n - 1, n - 1],
        radius=RADIUS * SUPERSAMPLE,
        fill=rgba(skin["bg"]),
        outline=rgba(skin["border"]),
        width=SUPERSAMPLE,
    )
    return img.resize((PANEL_PX, PANEL_PX), Image.LANCZOS)


def arrow(skin, direction: str) -> Image.Image:
    n = ARROW_PX * SUPERSAMPLE
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    s = SUPERSAMPLE
    if direction == "prev":
        pts = [(16 * s, 6 * s), (8 * s, 12 * s), (16 * s, 18 * s)]
    else:
        pts = [(8 * s, 6 * s), (16 * s, 12 * s), (8 * s, 18 * s)]
    d.polygon(pts, fill=rgba(skin["text"]))
    return img.resize((ARROW_PX, ARROW_PX), Image.LANCZOS)


def build(name: str, skin: dict):
    """返回 {相对文件名: bytes}——生成与校验共用同一份内容。"""
    import io

    conf = THEME_CONF.format(corner=CORNER, **skin).encode("utf-8")

    def png_bytes(img: Image.Image) -> bytes:
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        return buf.getvalue()

    return {
        "theme.conf": conf,
        "panel.png": png_bytes(rounded_panel(skin)),
        "prev.png": png_bytes(arrow(skin, "prev")),
        "next.png": png_bytes(arrow(skin, "next")),
    }


def check_one(name: str, skin: dict, files: dict) -> list:
    """逐字节对账 + 结构断言。返回失败描述列表。"""
    fails = []
    d = OUT_ROOT / f"opi-{name}"
    for fname, want in files.items():
        p = d / fname
        if not p.exists():
            fails.append(f"{p}: 不存在")
            continue
        got = p.read_bytes()
        if got != want:
            fails.append(f"{p}: 与重新生成的不一致（{len(got)}B vs {len(want)}B）")
    # 结构断言：圆角四角透明、中心是底色 —— 「数字对但图错」的形态
    # （跨格式缺省值类缺陷就藏在这种「值全对、图变样」里）。
    p = d / "panel.png"
    if p.exists():
        img = Image.open(p).convert("RGBA")
        w, h = img.size
        if w != PANEL_PX or h != PANEL_PX:
            fails.append(f"{p}: 尺寸 {w}x{h} != {PANEL_PX}")
        if img.getpixel((0, 0))[3] != 0:
            fails.append(f"{p}: 左上角不透明（圆角没生成出来）")
        if img.getpixel((w // 2, h // 2)) != rgba(skin["bg"]):
            fails.append(
                f"{p}: 中心色 {img.getpixel((w // 2, h // 2))} != 底色 {rgba(skin['bg'])}")
    return fails


def main() -> int:
    check = "--check" in sys.argv[1:]
    fails = []
    for name, skin in SKINS.items():
        files = build(name, skin)
        d = OUT_ROOT / f"opi-{name}"
        if check:
            fails += check_one(name, skin, files)
            continue
        d.mkdir(parents=True, exist_ok=True)
        for fname, data in files.items():
            (d / fname).write_bytes(data)
        print(f"生成 {d.relative_to(REPO)}/  ({', '.join(files)})")
    if check:
        if fails:
            print("!! --check 失败:")
            for f in fails:
                print("  -", f)
            return 1
        print(f"--check 通过：{len(SKINS)} 张皮肤逐字节一致，结构断言全过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
