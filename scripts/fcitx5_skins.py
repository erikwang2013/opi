# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""OPI 皮肤调色板（fcitx5 classicui 主题的唯一数据源）。

从 `gen_fcitx5_themes.py` 拆出（2026-10-10，500 行门禁；纯搬移，值一字未动）。
消费方：
- `gen_fcitx5_themes.py`（本目录）——生成主题目录；
- 桌面 `SettingsTest` 的两条锁同步门禁**按文本读本文件**（`SKINS` 的键序与
  bg/text/accent 字面）——搬走/改格式都会让门禁失明，动之前先看那两条测试。
"""

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
        "bg": "#FBEE7E",
        "border": "#3A300022",
        "text": "#3A3000",
        "accent": "#7E6200",
        "hl_bg": "#7E620026",
        "menu_bg": "#F9EA6C",
        "sep": "#D1C462",
    },
    "sunset": {
        "title": "opi输入法 晚霞",
        "desc": "暖橙粉底 + 深赭文字",
        "bg": "#F7C79C",
        "border": "#4A231722",
        "text": "#4A2317",
        "accent": "#A63412",
        "hl_bg": "#A6341226",
        "menu_bg": "#F4BC8A",
        "sep": "#D1A37F",
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
    # 2026-10-13 加两款（用户裁决「增加新的皮肤不要重复」）。挑色判据是**明度/色相不撞**：
    #   紫藤 H258/L95 —— 十款里唯一偏紫的色相（最近的星光是 H228 但 L16，明度差 79）；
    #   松林 H156/L17 —— 与清爽同色相族的**深色版**（清爽 L93，明度差 76）：不是重复而是对照。
    # 两款 text×bg 10.3/10.2、accent×bg 5.1/6.9（≥4.5 / ≥3，与本表其余款同一档；--check 不测）。
    "wisteria": {
        "title": "opi输入法 紫藤",
        "desc": "淡紫底 + 藤紫高亮",
        "bg": "#EDE7FB",
        "border": "#3A2A6322",
        "text": "#3A2A63",
        "accent": "#6B4BC4",
        "hl_bg": "#6B4BC426",
        "menu_bg": "#E3D9F8",
        "sep": "#CFC4EC",
    },
    "pine": {
        "title": "opi输入法 松林",
        "desc": "松林绿底 + 浅苔文字",
        "bg": "#1E3A2F",
        "border": "#FFFFFF1F",
        "text": "#DCEEE2",
        "accent": "#7FD3A2",
        "hl_bg": "#7FD3A233",
        "menu_bg": "#26473A",
        "sep": "#3C5C4C",
    },
}
