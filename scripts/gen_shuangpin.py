#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成双拼方案的**键位表**（产物提交入库；离线构建不重跑本脚本）。

产物：`crates/engine-core/src/shuangpin_tables.rs` —— 每套方案：声母键表、韵母键表、
零声母标记。

数据源：**fcitx/libime 的内置方案表**（`src/libime/pinyin/shuangpindata.h` +
装配语义 `shuangpinprofile.cpp`），pin 到 commit
`171edcf137001e8eb8274f53ec010058cda70b09`（2026-10-14 取）。
选它为源的理由：机器可读、覆盖**搜狗同款全套**（自然码/微软/紫光/智能ABC/小鹤/
拼音加加；libime 另有中文之星与 GB，不在搜狗列表内，本表不收），且是 fcitx5 正
在用的实现语义。r 用 rime 的 double_pinyin 方案文本**对过一遍**（自然码的
`iu→q/ian→m/…` 与 libime 的 C 表逐条一致）。

装配语义（逐条抄 libime `shuangpinprofile.cpp`，别"改进"）：
- 零声母标记：自然码/拼音加加 = `"o*"`、小鹤 = `"*"`（无标记，裸韵母键）、
  微软/紫光/智能ABC = 默认 `"o"`。`*` = **裸韵母键也接受**（自然码 `oa` 与 `a` 都行）。
- **单字符韵母自动补表**：`a/e/i/o/u/v` 六个单字符韵母若不在方案表里，按自身补
  （`singleCharFinal` 循环）——不补的话 `ni`/`bu` 这类常用音节会没有编码。

用法（离线可用；文件可来自缓存）：
    python3 scripts/gen_shuangpin.py [/path/to/shuangpindata.h]
    缺省从 pinned commit 的 raw 地址下（校验 sha256）。
"""
import hashlib
import os
import re
import sys
import urllib.request

PIN_COMMIT = "171edcf137001e8eb8274f53ec010058cda70b09"
SRC_URL = (
    "https://raw.githubusercontent.com/fcitx/libime/"
    f"{PIN_COMMIT}/src/libime/pinyin/shuangpindata.h"
)
SRC_SHA256 = "fd5de31afaeefb1c"  # 前缀（全值在下方自检里打印）

# 方案：产物名 → (C 表名, 显示名, 零声母标记)
# 零声母标记的来源：shuangpinprofile.cpp 的构造函数（370-403 行）；
# 未显式赋值的走 ShuangpinProfilePrivate 的默认 "o"（该文件 37 行）。
LAYOUTS = [
    ("ZIRANMA", "SPMap_C_Ziranma", "自然码", "o*"),
    ("MS", "SPMap_C_MS", "微软", "o"),
    ("XIAOHE", "SPMap_C_XIAOHE", "小鹤", "*"),
    ("ABC", "SPMap_C_ABC", "智能ABC", "o"),
    ("ZIGUANG", "SPMap_C_Ziguang", "紫光", "o"),
    ("PYJJ", "SPMap_C_PinyinJiaJia", "拼音加加", "o*"),
]

SINGLE_CHAR_FINALS = ["a", "e", "i", "o", "u", "v"]


def parse_table(text: str, name: str) -> list[tuple[str, str]]:
    """解析 `static const SP_C SPMap_C_X[] = { {"ai", 'l'}, … };` → [(key, 韵母)]。"""
    m = re.search(rf"{name}\[\]\s*=\s*\{{(.*?)\}};", text, re.S)
    if not m:
        sys.exit(f"!! 源里找不到表 {name} —— 上游格式变了？")
    rows = re.findall(r'\{"([a-z]+)",\s*\'(.?)\'\}', m.group(1))
    return [(key, final) for final, key in rows if key]  # C 表是 (韵母, 键)，翻过来


def parse_initials(text: str, name: str) -> list[tuple[str, str]]:
    """SPMap_S_X：只有 zh/ch/sh 三条（其余声母即字母自身，不列表）。"""
    rows = parse_table(text, name)
    return rows


def fill_single_char_finals(rows: list[tuple[str, str]]) -> list[tuple[str, str]]:
    """单字符韵母自动补表 —— 判据是 **(键, 韵母) 对**去重（libime 在 multimap 上
    `emplace`，同键不同韵母照加）：微软的 `v` 键已有 ui/ve 两条，仍要补一条 `v→v`
    （ü 的裸形），按键去重会把它漏掉（起草版就这么漏了，靠对读 libime 代码抓回）。"""
    pairs = set(rows)
    out = list(rows)
    for f in SINGLE_CHAR_FINALS:
        if (f, f) not in pairs:
            out.append((f, f))
    return sorted(out)


def main() -> int:
    if len(sys.argv) > 1:
        data = open(sys.argv[1], "rb").read()
    else:
        data = urllib.request.urlopen(SRC_URL, timeout=60).read()
    got = hashlib.sha256(data).hexdigest()
    if not got.startswith(SRC_SHA256):
        sys.exit(f"!! shuangpindata.h 摘要与 pin 不符：{got}（期望 {SRC_SHA256}…）")
    text = data.decode()

    out = [
        "// SPDX-FileCopyrightText: 2026 erik.xyz",
        "// SPDX-License-Identifier: MIT",
        "",
        "//! 双拼键位表（**生成物，勿手改**；生成器 `scripts/gen_shuangpin.py`）。",
        "//!",
        "//! 源：fcitx/libime `src/libime/pinyin/shuangpindata.h`，pin commit",
        f"//! `{PIN_COMMIT}`（装配语义见同仓 `shuangpinprofile.cpp`：零声母标记与",
        "//! 单字符韵母自动补表都逐条镜像）。覆盖**搜狗同款**六套（自然码/微软/小鹤/",
        "//! 智能ABC/紫光/拼音加加）；libime 另有中文之星与 GB，不在搜狗列表内，未收。",
        "//!",
        "//! 结构：`initials` = 声母键（只有 zh/ch/sh 需要列表，单字母声母即字母自身）；",
        "//! `finals` = 韵母键（含自动补齐的 a/e/i/o/u/v）；`zero` = 零声母标记，",
        "//! `*` 表示裸韵母键也接受（如小鹤）。",
        "",
        "/// 一套双拼方案的表。键都是小写字母（宿主送进来的也只会是小写）。",
        "pub struct ShuangpinTable {",
        "    pub name: &'static str,",
        "    /// (键, 韵母)：打进这个键 = 这个韵母。",
        "    pub finals: &'static [(char, &'static str)],",
        "    /// (键, 声母)：只在 zh/ch/sh 三个多字母声母上非空。",
        "    pub initials: &'static [(char, &'static str)],",
        "    /// 零声母标记串；含 `*` = 裸韵母键也可作零声母音节。",
        "    pub zero: &'static str,",
        "}",
        "",
        "/// 全部方案（顺序 = 设置界面的展示序；索引即 FFI 的口径）。",
        "pub static SHUANGPIN_TABLES: &[ShuangpinTable] = &[",
    ]
    for var, cname, label, zero in LAYOUTS:
        finals = fill_single_char_finals(parse_table(text, cname))
        initials = parse_initials(text, cname.replace("SPMap_C_", "SPMap_S_"))
        fin = ", ".join(f"('{k}', \"{v}\")" for k, v in finals)
        ini = ", ".join(f"('{k}', \"{v}\")" for k, v in initials)
        out += [
            "    ShuangpinTable {",
            f'        name: "{label}",',
            f"        finals: &[{fin}],",
            f"        initials: &[{ini}],",
            f'        zero: "{zero}",',
            "    },",
        ]
    out += ["];", ""]
    dest = os.path.join(
        os.path.dirname(os.path.abspath(__file__)),
        "..",
        "crates",
        "engine-core",
        "src",
        "shuangpin_tables.rs",
    )
    with open(dest, "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    print(f"写出 {dest}：{len(LAYOUTS)} 套方案")
    for var, cname, label, zero in LAYOUTS:
        finals = fill_single_char_finals(parse_table(text, cname))
        print(f"  {label}: 韵母 {len(finals)} 键，零声母 {zero!r}，zh/ch/sh → "
              + ", ".join(f"{k}={v}" for k, v in parse_initials(text, cname.replace("SPMap_C_", "SPMap_S_"))))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
