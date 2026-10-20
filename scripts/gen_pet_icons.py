#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""宠物「小欧」图标产物生成器。

覆盖全部图标位：

  1. Android 旧版启动图标（API 21–25 的 mipmap-*/ic_launcher.png，方/圆各 5 桶）
     + API 26+ 的圆形自适应 XML（`mipmap-anydpi-v26/ic_launcher_round.xml`）
  2. 鸿蒙应用图标 `harmony/AppScope/resources/base/media/app_icon.png`（`app.json5`
     的 `$media:app_icon`）与输入法子类型图标
     `harmony/entry/src/main/resources/base/media/icon.png`（512²，`input_method_config.json`
     三个 subtype 的 `$media:icon` 指向它）
  3. 桌面应用图标 `desktop/icons/opi-pet.png`（256²）、`opi-pet.ico`
     （16/32/48/64/128/256）与 `opi-pet.icns`（128/256/512/1024）—— Windows
     EXE/MSI、Linux jpackage、macOS .app（`CFBundleIconFile`）、iOS 宿主 App
     （`scripts/apple/package-ios.sh` 用 sips 从 PNG 缩）
  4. Linux 菜单/桌面入口的矢量图标 `docs/opi-pet-app.svg`（hicolor scalable）
  5. macOS：`macos/opi.tiff`（32×32 @144dpi = 16pt@2x，`Info.plist` 的
     `tsInputMethodIconFileKey` 指向它）
  6. Windows：`crates/tsf-opi/assets/opi-langbar.ico`（16+32）—— 由
     `crates/tsf-opi/src/langbar_icon.rs` 以 `include_bytes!` 嵌进 DLL，
     运行期造 HICON 给语言栏按钮（见该文件头注释的两条格式约定）

**为什么存在**：这些都是**生成物**，而「改矢量要重出」只靠注释提醒是没有执行点的
警告（本仓 2026-09-28 之前正好相反 —— 那 5 个 Android 桶是脚手架带来的 **Flutter
默认图标**，注释却写着「两者同为键帽精灵形象」，一直没人看得见）。所以生成逻辑进仓库，
并带 `--check` 让「漂了」变成一条会红的命令。

**几何来源**：
  · Android 位图、鸿蒙两张、macOS/Windows 模板只读 R4
    （`android/.../drawable/ic_launcher_foreground.xml`），把它**机械转成 SVG**
    （`android:pathData` 本就是 SVG path 语法；`android:viewportWidth/Height` 定
    viewBox），**不新写几何、不改 R4**。
  · 桌面 PNG/ICO 与 Linux 菜单矢量从完整设计稿 `docs/opi-pet.svg` 取景：只加一层
    纸白背景矩形、把 viewBox 改成方形 `-5 0 250 250`，宠物几何一个数不改。

**底色**（2026-10-20 用户裁决「所有端安装后默认的图标底色是浅色，不要黑色」）：
**应用/启动图标一律铺项目纸白 `PET_PAPER`（#FBFAF6）**。2026-10-09 的「应用图标
就不要加绿色背景，直接放 logo」被本条取代（透明底在深色启动器/任务栏上会读成黑
底）。**两个模板/服务图标仍必须透明**，铺底会坏：
  · `macos/opi.tiff` —— InputMethodKit 输入源图标按模板渲染，只看 alpha；
  · `crates/tsf-opi/assets/opi-langbar.ico` —— 语言栏按钮直接贴 HICON。
fcitx5 的输入法图标 `docs/opi-pet.svg` 也保持透明（服务图标）；菜单/桌面入口改用
本脚本生成的 `docs/opi-pet-app.svg`（同一只小欧 + 纸白底）。

**可见区**：自适应图标渲染时把 108×108 画布裁到正中 72×72 再放大到图标尺寸，所以
Android 旧版位图要铺满的正是那 72×72。**其余三端再裁一步**：R4 的 72 窗口自带约 15%
内边距（是给自适应遮罩留的），在别端只是白边 ⇒ 按 alpha 裁到内容 bbox，等比放进
带 `PAD_RATIO` 留白的正方画布 —— 视觉大小与 Android 启动图标一致。

**圆形（Android）**：同一口径再加 r=36 的圆遮罩，即 72 可见区的**内切圆** ——
这是系统圆形遮罩的真实尺寸（推导见 `ROUND_RADIUS`），**不是** 66dp 安全圈（r=33，
那是 `getSafeZone()` 的设计指引，当遮罩用会比系统实际显示的小一圈，与 API 26+ 那套
分叉）。朱砂天线最远到 r=34.87：真遮罩 r=36 内 1.13 ⇒ **裁不到**；若按 r=33 则会削掉
1.87（对照数，不是本脚本的行为）。详见 R4 的注释。

用法：
    python3 scripts/gen_pet_icons.py            # 重出全部产物（19 个）
    python3 scripts/gen_pet_icons.py --check    # 只比对，不写；不一致则退出码 1
"""
# ponytail: 依赖 rsvg-convert + PIL，与本仓既有 gen_*.py 同档（都是本机脚本，不是 CI 依赖）
import io
import struct
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
RES = ROOT / "android/app/src/main/res"
FOREGROUND = RES / "drawable/ic_launcher_foreground.xml"
AND = "{http://schemas.android.com/apk/res/android}"
HARMONY_ICON = ROOT / "harmony/entry/src/main/resources/base/media/icon.png"
HARMONY_APP_ICON = ROOT / "harmony/AppScope/resources/base/media/app_icon.png"
DOCS_PET = ROOT / "docs/opi-pet.svg"
APP_SVG = ROOT / "docs/opi-pet-app.svg"
DESKTOP_PNG = ROOT / "desktop/icons/opi-pet.png"
DESKTOP_ICO = ROOT / "desktop/icons/opi-pet.ico"
DESKTOP_ICNS = ROOT / "desktop/icons/opi-pet.icns"
MACOS_TIFF = ROOT / "macos/opi.tiff"
TSF_ICO = ROOT / "crates/tsf-opi/assets/opi-langbar.ico"

# 应用图标底色 = 项目宣纸白（docs/opi-pet.svg 的 ground）。
PET_PAPER = "#FBFAF6"
PET_PAPER_RGBA = tuple(int(PET_PAPER[i:i + 2], 16) for i in (1, 3, 5)) + (255,)

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

# 非安卓三端的留白比例（见文件头「其余三端再裁一步」）。
# 6% 是**手感常数**：0 会让天线顶到画布边、与安卓那套比显得偏大；调它就改这一个数。
PAD_RATIO = 0.06

# 产物：文件名 → 遮罩半径（None = 方形）。
VARIANTS = {"ic_launcher": None, "ic_launcher_round": ROUND_RADIUS}


def adaptive_xml() -> str:
    """API 26+ 的自适应圆图标。方形的 ic_launcher.xml 是等价的孪生（手写在 res 里）。"""
    return f"""<?xml version="1.0" encoding="utf-8"?>
<!--
  项目宠物「小欧」自适应启动图标（API 26+）的**圆形**孪生，供 manifest 的
  android:roundIcon 使用 —— 图形与 ic_launcher.xml 完全相同（同一前景 + 纸白背景），
  圆形由系统遮罩给出，所以这里**不需要**任何几何改动。
  **背景层是纸白 `@color/ic_launcher_background`**（#FBFAF6；2026-10-20 用户裁决
  「所有端安装后默认的图标底色是浅色，不要黑色」—— 取代 2026-10-09 的透明底裁决；
  透明底在深色启动器上读成黑底）。
  这是**生成物**：改 R4 前景后跑 `python3 scripts/gen_pet_icons.py` 重出。

  API 25（roundIcon 正是这一版引入的）用不了自适应图标，落回
  mipmap-*/ic_launcher_round.png：72dp 可见区的 r=36 内切圆，与圆形系统遮罩同径。
  API 24 及以下不读 roundIcon，走 android:icon 的方形那套。
-->
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background"/>
    <foreground android:drawable="@drawable/{FOREGROUND.stem}"/>
</adaptive-icon>
"""


def foreground_svg(size: int, mask_radius: float | None = None, crop: bool = True,
                   plate: bool = True) -> str:
    """R4 → SVG。属性名映射而已，坐标一个数不改。

    `plate=True` 铺一层纸白矩形（应用图标）；模板/服务图标传 `False` 保持透明。
    """
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
    ]
    if plate:
        # 自适应图标那层纯色 background：旧版位图里得自己铺满（PNG 没有 background 层）。
        # 圆形的矩形四角随后被 clipPath 裁掉，圆外仍是透明。
        out.append(f'  <rect x="0" y="0" width="{vw}" height="{vh}" fill="{PET_PAPER}"/>\n')

    def emit(node, depth: int) -> None:
        pad = "  " * depth
        tag = node.tag
        a = node.attrib
        if tag.endswith("path"):
            attrs = [
                f'{dst}="{a[AND + src]}"'
                for src, dst in (
                    ("fillColor", "fill"),
                    ("strokeColor", "stroke"),
                    ("strokeWidth", "stroke-width"),
                    ("strokeLineCap", "stroke-linecap"),
                    ("strokeLineJoin", "stroke-linejoin"),
                    ("fillAlpha", "fill-opacity"),
                    ("strokeAlpha", "stroke-opacity"),
                )
                if AND + src in a
            ]
            # ⚠️ 缺省值两边**不一样**：Android 的 fillColor 缺省 = 透明，SVG 的 fill 缺省 = 黑。
            # R4 的嘴只有 strokeColor（圆环 = 字母 O），不显式 fill="none" 就渲成一块实心黑饼。
            # 这脚 2026-09-28 踩过一次，且**外框 bbox 检查抓不到**（黑填充不改外框），只能靠看图。
            if AND + "fillColor" not in a:
                attrs.append('fill="none"')
            attrs.append(f'd="{a[AND + "pathData"]}"')
            out.append(f"{pad}<path " + " ".join(attrs) + "/>\n")
            return
        if tag.endswith("group"):
            # 2026-10-09：R4 改版后前景有一层分组变换（设计空间 → 108 画布的居中缩放）。
            # VectorDrawable 的组变换 = translate ∘ rotate ∘ scale（pivot 0 时即 T + S·p），
            # 按同样的式子转成 SVG 的 transform（translate 在前、scale 在后 = 先缩后移）。
            sx = float(a.get(AND + "scaleX", 1.0))
            sy = float(a.get(AND + "scaleY", 1.0))
            tx = float(a.get(AND + "translateX", 0.0))
            ty = float(a.get(AND + "translateY", 0.0))
            if "rotation" in a or "pivotX" in a or "pivotY" in a:
                raise SystemExit("R4 出现了旋转/轴心的组变换 —— 转换只实现了 translate+scale")
            out.append(f'{pad}<g transform="translate({tx} {ty}) scale({sx} {sy})">\n')
            for child in node:
                emit(child, depth + 1)
            out.append(f"{pad}</g>\n")
            return
        # 其余节点（clip-path 等）本文件不用；出现即报错，别静默丢图形。
        if tag is not ET.Comment and tag.endswith(("path", "group")) is False:
            raise SystemExit(f"R4 里有没实现的节点：{tag}")

    for child in root:
        emit(child, 1)
    if mask_radius is not None:
        out.append("  </g>\n")
    return "".join(out) + "</svg>\n"


def raster_svg(svg_text: str, size: int) -> Image.Image:
    """栅格化一段 SVG。rsvg-convert 由矢量直出（不靠放大位图），返回 RGBA 图。"""
    with tempfile.TemporaryDirectory() as td:
        svg, png = Path(td) / "i.svg", Path(td) / "i.png"
        svg.write_text(svg_text, encoding="utf-8")
        subprocess.run(
            ["rsvg-convert", "-w", str(size), "-h", str(size), str(svg), "-o", str(png)],
            check=True,
        )
        return Image.open(png).convert("RGBA")


def raster(size: int, mask_radius: float | None = None, plate: bool = True) -> Image.Image:
    """R4 栅格化（`plate` 语义见 [foreground_svg]）。"""
    return raster_svg(foreground_svg(size, mask_radius, plate=plate), size)


def app_svg() -> str:
    """`docs/opi-pet.svg` + 纸白底 + 方形取景（生成物，别手改）。

    只改两处：viewBox 从 240×250 变 `-5 0 250 250`（宠物满高、左右各留 5 单位，
    与旧桌面 PNG 的取景等价），以及在图形最前面插一层底色矩形。宠物几何一个数不改。
    """
    svg = DOCS_PET.read_text(encoding="utf-8")
    view = 'viewBox="0 0 240 250" width="240" height="250"'
    anchor = "  <!-- 落地投影 -->"
    xml_head = '<?xml version="1.0" encoding="UTF-8"?>\n'
    if svg.count(view) != 1 or svg.count(anchor) != 1 or svg.count(xml_head) != 1:
        raise SystemExit("docs/opi-pet.svg 的取景/锚点变了 —— 更新 app_svg()")
    svg = svg.replace(view, 'viewBox="-5 0 250 250" width="250" height="250"', 1)
    svg = svg.replace(
        anchor,
        f'  <!-- 应用图标底色（2026-10-20 用户裁决：浅色，不要黑色） -->\n'
        f'  <rect x="-5" y="0" width="250" height="250" fill="{PET_PAPER}"/>\n\n' + anchor,
        1,
    )
    return svg.replace(
        xml_head,
        xml_head + "<!-- 生成物：python3 scripts/gen_pet_icons.py（源头 docs/opi-pet.svg） "
                   "—— 别手改 -->\n",
        1,
    )


def png_bytes(img: Image.Image, mode: str = "RGBA") -> bytes:
    """PIL 写 PNG（确定性：无 tIME 块）。"""
    buf = io.BytesIO()
    img.convert(mode).save(buf, format="PNG")
    return buf.getvalue()


def render(size: int, mask_radius: float | None = None) -> bytes:
    """Android 旧版桶：铺满纸白底（2026-10-20 用户裁决「底色是浅色，不要黑色」）。

    方形版四角也是纸白；圆形版圆外保持透明（系统圆遮罩之外不该有底色，否则
    方形 PNG 的角会在圆形启动器里露出来）。
    """
    return png_bytes(raster(size, mask_radius, plate=True), "RGBA")


def square_mark(size: int, plate: bool = False) -> Image.Image:
    """R4 艺术 → 裁到内容 bbox → 等比放进带 PAD_RATIO 留白的正方画布。

    `plate=True` 用于应用图标（纸白底）；默认透明，供 macOS 模板 / Windows
    语言栏这两个服务图标使用。
    """
    # 先在 4× 目标尺寸上裁（裁剪 bbox 与缩放都在高分辨率下做，32px 也不糊）。
    img = raster(max(size, 128) * 4, plate=False)
    bbox = img.split()[3].getbbox()  # 以 **alpha** 为准（RGB 通道在透明区无意义）
    if bbox is None:
        raise SystemExit("R4 渲出来是空的？alpha 全 0 —— 检查前景矢量")
    img = img.crop(bbox)
    inner = size * (1 - 2 * PAD_RATIO)
    k = inner / max(img.width, img.height)
    img = img.resize((max(1, round(img.width * k)), max(1, round(img.height * k))), Image.LANCZOS)
    canvas = Image.new("RGBA", (size, size), PET_PAPER_RGBA if plate else (0, 0, 0, 0))
    canvas.alpha_composite(img, ((size - img.width) // 2, (size - img.height) // 2))
    return canvas


def tiff_bytes(img: Image.Image, dpi: int) -> bytes:
    """带 alpha 的 TIFF + 显式 dpi（macOS 输入源图标 = 16pt@2x ⇒ dpi=144）。"""
    buf = io.BytesIO()
    img.save(buf, format="TIFF", dpi=(dpi, dpi))
    return buf.getvalue()


def ico_dib(img: Image.Image) -> bytes:
    """一张 32bpp ico 条目 = RT_ICON 的 ICONIMAGE 布局：
    `BITMAPINFOHEADER(biHeight=2h) + 自下而上的 BGRA 位图 + 1bpp AND 掩码`。

    **不用 PIL 的 ICO writer**：它对 32bpp 只写 XOR 部分（不加掩码）却把 biHeight
    改成 2×h，而 Windows 侧按 biHeight 找掩码偏移 —— 数据比声明短一截。字节序号
    1:1 对应，手写反而没有版本行为可漂。"""
    w, h = img.size
    px = img.convert("RGBA").load()
    xor = bytearray()
    for y in range(h - 1, -1, -1):  # 位图行自下而上
        for x in range(w):
            r, g, b, a = px[x, y]
            xor += bytes((b, g, r, a))
    stride = ((w + 31) // 32) * 4
    and_mask = bytearray()
    for y in range(h - 1, -1, -1):
        row = bytearray(stride)
        for x in range(w):
            if px[x, y][3] < 128:  # 掩码位 1 = 透明
                row[x // 8] |= 0x80 >> (x % 8)
        and_mask += row
    header = struct.pack("<IiiHHIIiiII", 40, w, h * 2, 1, 32, 0,
                         len(xor) + len(and_mask), 0, 0, 0, 0)
    return bytes(header + xor + and_mask)


def ico_bytes(img: Image.Image, sizes: list[int]) -> bytes:
    """多尺寸 .ico。**条目必须是 DIB、不能是 PNG 条目** ——
    `crates/tsf-opi/src/langbar_icon.rs` 把条目字节直接当 RT_ICON 喂给
    CreateIconFromResourceEx，PNG 条目在那里是非法输入。写完当场验一遍。"""
    entries = []
    for s in sizes:
        entries.append(img.resize((s, s), Image.LANCZOS) if img.size != (s, s) else img)
    blobs = [ico_dib(e) for e in entries]
    out = bytearray(struct.pack("<HHH", 0, 1, len(blobs)))  # ICONDIR
    off = 6 + 16 * len(blobs)
    for e, blob in zip(entries, blobs):
        w = e.width if e.width < 256 else 0
        out += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), off)  # ICONDIRENTRY
        off += len(blob)
    for blob in blobs:
        out += blob
    data = bytes(out)

    # 自检：与 langbar_icon.rs 的解析契约（只认 DIB、biHeight=2h、长度自洽）。
    for i in range(len(blobs)):
        size, off = struct.unpack_from("<II", data, 6 + i * 16 + 8)
        blob = data[off:off + size]
        bi_size, bi_w, bi_h = struct.unpack_from("<Iii", blob, 0)
        h = bi_h // 2
        stride = ((bi_w + 31) // 32) * 4
        want = 40 + bi_w * h * 4 + stride * h  # XOR(32bpp) + AND(1bpp)，行自下而上
        if bi_size != 40:
            raise SystemExit(f"第 {i} 个条目不是 BITMAPINFOHEADER —— 解析侧会拒绝")
        if bi_h != bi_w * 2 or len(blob) != want:
            raise SystemExit(f"第 {i} 个条目长度不自洽（biHeight={bi_h}，{len(blob)}≠{want}）")
    return data


def icns_bytes(img: Image.Image) -> bytes:
    """macOS 应用包图标（`CFBundleIconFile`）。ic07–ic10 = 128/256/512/1024 的
    PNG 数据，10.7+ 直接可读，小尺寸由 Finder 缩；不用 icp4/icp5（那几档是裸 RGB，
    不认 PNG）。"""
    entries = []
    for kind, size in (("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024)):
        data = png_bytes(img.resize((size, size), Image.LANCZOS), "RGB")
        entries.append(kind.encode() + struct.pack(">I", 8 + len(data)) + data)
    out = b"icns" + struct.pack(">I", 8 + sum(map(len, entries))) + b"".join(entries)

    # 自检：总长与每条长度自洽、每条都是 PNG —— 长度写错时 Finder 不显示图标且不报错。
    off = 8
    while off < len(out):
        length = struct.unpack_from(">I", out, off + 4)[0]
        if length < 16 or off + length > len(out) or out[off + 8:off + 16] != b"\x89PNG\r\n\x1a\n":
            raise SystemExit(f"icns 条目在偏移 {off} 处不自洽（length={length}）")
        off += length
    if struct.unpack_from(">I", out, 4)[0] != len(out) or off != len(out):
        raise SystemExit("icns 总长与条目长度不符")
    return out


def artifacts() -> list[tuple[Path, bytes]]:
    """全部生成物（19 个）：Android 11 + 鸿蒙 2 + 桌面 4 + macOS/Windows 模板 2。"""
    out = [
        (RES / f"mipmap-{bucket}/{name}.png", render(size, radius))
        for name, radius in VARIANTS.items()
        for bucket, size in BUCKETS.items()
    ]
    out.append((RES / "mipmap-anydpi-v26/ic_launcher_round.xml", adaptive_xml().encode()))
    # 鸿蒙：应用图标与输入法子类型图标同图同底（纸白）。
    out.append((HARMONY_APP_ICON, png_bytes(square_mark(512, plate=True))))
    out.append((HARMONY_ICON, png_bytes(square_mark(512, plate=True))))
    # 桌面：Windows EXE/MSI（ico）、Linux jpackage / iOS 宿主（png）、macOS .app（icns）。
    # PNG 存 RGB（无 alpha）—— iOS 应用图标不接受透明通道。ICO 各档直接从 1042² 缩，
    # 免得 128 以下先经 256 再缩一遍。
    desktop = raster_svg(app_svg(), 1042)
    out.append((DESKTOP_PNG, png_bytes(desktop.resize((256, 256), Image.LANCZOS), "RGB")))
    out.append((DESKTOP_ICO, ico_bytes(desktop, sizes=[16, 32, 48, 64, 128, 256])))
    out.append((DESKTOP_ICNS, icns_bytes(desktop)))
    # Linux 菜单/桌面入口的矢量应用图标（hicolor scalable）。
    out.append((APP_SVG, app_svg().encode()))
    # macOS：16pt@2x 输入源模板 —— 只按 alpha 着色，必须透明。
    out.append((MACOS_TIFF, tiff_bytes(square_mark(32), dpi=144)))
    # Windows：语言栏按钮的 HICON 源（16 = SM_CXSMICON，32 = 高 DPI 兜底）—— 必须透明。
    out.append((TSF_ICO, ico_bytes(square_mark(32), sizes=[16, 32])))
    return out


def main() -> int:
    check = "--check" in sys.argv[1:]
    bad = []
    for target, new in artifacts():
        rel = target.relative_to(ROOT)
        if check:
            old = target.read_bytes() if target.exists() else b""
            if old != new:
                bad.append(f"{rel} 与源矢量不一致（重出会变）")
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(new)
        print(f"{target.parent.name:18s} {target.name:22s} → {rel}")
    if check:
        if bad:
            print("图标与矢量已经分叉，跑 `python3 scripts/gen_pet_icons.py` 重出：")
            for b in bad:
                print("  " + b)
            return 1
        print(f"{len(artifacts())} 个产物与源矢量一致")
    return 0


if __name__ == "__main__":
    sys.exit(main())
