#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""生成 data/raw/name_keywords.tsv：符号/标点/颜文字的**名称关键字**表。

输入法里输入「zuokuohao / leftbracket / kaixin」时，引擎用这张表把名称前缀
匹配到对应文本（标点、符号、颜文字）。表情仍走 emoji_keywords.tsv，不在这里。

表格式（UTF-8、无表头、制表符分隔）：
    text \t name \t keywords(逗号分隔) \t kind \t lang

- kind: punct / symbol / kaomoji
- lang: zh（中文模式，拼音关键字）/ en（英文模式，英文关键字）/ both
- keywords 一律小写 ASCII 字母数字（缓冲只收这些；`'` 在匹配时会被去掉）。

用法：
    python3 scripts/gen_name_keywords.py           # 写 data/raw/name_keywords.tsv
    python3 scripts/gen_name_keywords.py --check   # 与现有产物对账
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KAOMOJI = ROOT / "data" / "raw" / "kaomoji.tsv"
OUT = ROOT / "data" / "raw" / "name_keywords.tsv"

# (text, name, pinyin keywords, english keywords, kind, lang)
# 中文标点用 zh（拼音名），ASCII 标点用 en（英文名），符号两边都能搜。
PUNCT = [
    # 中文标点
    ("，", "逗号", "douhao", "comma", "punct", "zh"),
    ("。", "句号", "juhao", "period,fullstop", "punct", "zh"),
    ("、", "顿号", "dunhao", "enumerationcomma,listcomma", "punct", "zh"),
    ("；", "分号", "fenhao", "semicolon", "punct", "zh"),
    ("：", "冒号", "maohao", "colon", "punct", "zh"),
    ("？", "问号", "wenhao", "questionmark,question", "punct", "zh"),
    ("！", "感叹号", "gantanhao", "exclamation,exclamationmark", "punct", "zh"),
    ("（", "左括号", "zuokuohao,kuohao", "leftparen,leftparenthesis,openparen", "punct", "zh"),
    ("）", "右括号", "youkuohao,kuohao", "rightparen,rightparenthesis,closeparen", "punct", "zh"),
    ("【", "左方括号", "zuofangkuohao,kuohao", "leftsquarebracket", "punct", "zh"),
    ("】", "右方括号", "youfangkuohao,kuohao", "rightsquarebracket", "punct", "zh"),
    ("《", "左书名号", "zuoshuminghao,kuohao,shuminghao", "lefttitlemark", "punct", "zh"),
    ("》", "右书名号", "youshuminghao,kuohao,shuminghao", "righttitlemark", "punct", "zh"),
    ("“", "左引号", "zuoyinhao,yinhao", "leftquote,openquote", "punct", "zh"),
    ("”", "右引号", "youyinhao,yinhao", "rightquote,closequote", "punct", "zh"),
    ("‘", "左单引号", "zuodanyinhao,yinhao", "leftsinglequote", "punct", "zh"),
    ("’", "右单引号", "youdanyinhao,yinhao", "rightsinglequote", "punct", "zh"),
    ("…", "省略号", "shengluehao", "ellipsis", "punct", "zh"),
    ("—", "破折号", "pozhehao", "dash,emdash", "punct", "zh"),
    ("·", "间隔号", "jiangehao", "middot", "punct", "zh"),
    ("～", "波浪号", "bolanghao", "tilde", "punct", "zh"),
    ("￥", "人民币", "renminbi,yuan,rmb", "yuan,rmb", "symbol", "zh"),
    # ASCII 标点（英文模式）
    ("(", "left parenthesis", "", "leftparen,leftparenthesis,openparen,paren,parenthesis,bracket", "punct", "en"),
    (")", "right parenthesis", "", "rightparen,rightparenthesis,closeparen,paren,parenthesis,bracket", "punct", "en"),
    ("[", "left bracket", "", "leftbracket,leftsquarebracket,bracket", "punct", "en"),
    ("]", "right bracket", "", "rightbracket,rightsquarebracket,bracket", "punct", "en"),
    ("{", "left brace", "", "leftbrace,leftcurly,brace,bracket", "punct", "en"),
    ("}", "right brace", "", "rightbrace,rightcurly,brace,bracket", "punct", "en"),
    (",", "comma", "", "comma", "punct", "en"),
    (".", "period", "", "period,dot,fullstop", "punct", "en"),
    (";", "semicolon", "", "semicolon", "punct", "en"),
    (":", "colon", "", "colon", "punct", "en"),
    ("?", "question mark", "", "questionmark,question", "punct", "en"),
    ("!", "exclamation mark", "", "exclamation,exclamationmark,bang", "punct", "en"),
    ("'", "apostrophe", "", "apostrophe,singlequote,quote", "punct", "en"),
    ('"', "quotation mark", "", "quote,quotationmark,doublequote", "punct", "en"),
    ("/", "slash", "", "slash,forwardslash", "punct", "en"),
    ("\\", "backslash", "", "backslash", "punct", "en"),
    ("|", "vertical bar", "", "pipe,verticalbar", "punct", "en"),
    ("-", "hyphen", "", "hyphen,minus,dash", "punct", "en"),
    ("_", "underscore", "", "underscore,underbar", "punct", "en"),
    ("@", "at sign", "aite", "at,atsign", "symbol", "both"),
    ("#", "hash", "jinghao", "hash,numbersign", "symbol", "both"),
    ("$", "dollar", "meiyuan", "dollar", "symbol", "both"),
    ("%", "percent", "baifenhao", "percent", "symbol", "both"),
    ("&", "ampersand", "hehao", "ampersand,and", "symbol", "both"),
    ("*", "asterisk", "xinghao", "asterisk,star", "symbol", "both"),
    ("+", "plus", "jiahao", "plus", "symbol", "both"),
    ("=", "equals", "denghao", "equals,equal", "symbol", "both"),
    ("<", "less than", "xiaoyuhao", "lessthan", "symbol", "both"),
    (">", "greater than", "dayuhao", "greaterthan", "symbol", "both"),
    ("^", "caret", "tuozifu", "caret", "symbol", "both"),
    ("~", "tilde", "bolanghao", "tilde", "symbol", "both"),
    # 常用符号 / 数学 / 箭头 / 星心
    ("°", "度", "du", "degree", "symbol", "both"),
    ("℃", "摄氏度", "sheshidu", "celsius", "symbol", "both"),
    ("±", "正负号", "zhengfuhao", "plusminus", "symbol", "both"),
    ("×", "乘号", "chenghao", "multiply,times", "symbol", "both"),
    ("÷", "除号", "chuhao", "divide,division", "symbol", "both"),
    ("√", "根号", "genhao", "squareroot,root", "symbol", "both"),
    ("∞", "无穷", "wuqiong", "infinity", "symbol", "both"),
    ("≠", "不等号", "budengyu", "notequal", "symbol", "both"),
    ("≤", "小于等于", "xiaoyudengyu", "lessthanorequal", "symbol", "both"),
    ("≥", "大于等于", "dayudengyu", "greaterthanorequal", "symbol", "both"),
    ("≈", "约等于", "yuedengyu", "approx,almostequal", "symbol", "both"),
    ("←", "左箭头", "zuojiantou,jiantou", "leftarrow", "symbol", "both"),
    ("→", "右箭头", "youjiantou,jiantou", "rightarrow,arrow", "symbol", "both"),
    ("↑", "上箭头", "shangjiantou,jiantou", "uparrow", "symbol", "both"),
    ("↓", "下箭头", "xiajiantou,jiantou", "downarrow", "symbol", "both"),
    ("↔", "左右箭头", "zuoyoujiantou,jiantou", "leftrightarrow", "symbol", "both"),
    ("★", "实心星", "xingxing,xing", "star,blackstar", "symbol", "both"),
    ("☆", "空心星", "kongxinxing,xingxing", "emptystar,whitestar", "symbol", "both"),
    ("♥", "爱心", "aixin,xin", "heart,love", "symbol", "both"),
    ("♡", "空心爱心", "kongxinaixin,aixin", "emptyheart,whiteheart", "symbol", "both"),
    ("✓", "对勾", "duigou", "check,checkmark,tick", "symbol", "both"),
    ("✔", "粗对勾", "cuduigou", "heavycheck,checkmark", "symbol", "both"),
    ("✗", "叉", "cha", "cross,xmark", "symbol", "both"),
    ("✘", "粗叉", "cucha", "heavyx,xmark", "symbol", "both"),
    ("☀", "太阳", "taiyang", "sun", "symbol", "both"),
    ("☁", "云", "yun", "cloud", "symbol", "both"),
    ("☂", "雨伞", "yusan", "umbrella", "symbol", "both"),
    ("☎", "电话", "dianhua", "telephone,phone", "symbol", "both"),
    ("✉", "信封", "xinfeng", "envelope,mail", "symbol", "both"),
    ("✈", "飞机", "feiji", "airplane,plane", "symbol", "both"),
    ("♠", "黑桃", "heitao", "spade", "symbol", "both"),
    ("♣", "梅花", "meihua", "club", "symbol", "both"),
    ("♦", "方块", "fangkuai", "diamond", "symbol", "both"),
    ("♪", "音符", "yinfu", "note,music", "symbol", "both"),
    ("♫", "双音符", "shuangyinfu", "musicnotes", "symbol", "both"),
    ("©", "版权", "banquan", "copyright", "symbol", "both"),
    ("®", "注册商标", "zhuce", "registered", "symbol", "both"),
    ("™", "商标", "shangbiao", "trademark", "symbol", "both"),
    ("§", "章节", "zhangjie", "section", "symbol", "both"),
    ("¶", "段落", "duanluo", "paragraph", "symbol", "both"),
    ("•", "圆点", "yuandian", "bullet", "symbol", "both"),
    ("‰", "千分号", "qianfenhao", "permille", "symbol", "both"),
]

KAOMOJI_GROUPS = {
    "开心": "kaixin,happy,smile,joy,haha",
    "难过": "nanguo,sad,cry,unhappy",
    "生气": "shengqi,angry,mad",
    "惊讶": "jingya,surprised,wow,shock",
    "害羞": "haixiu,shy,blush",
    "动作": "dongzuo,action,move,hug,wave",
    "其他": "qita,other,misc",
}


def load_kaomoji() -> list[tuple[str, str, str, str, str]]:
    rows: list[tuple[str, str, str, str, str]] = []
    for line in KAOMOJI.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        text, group = line.split("\t")
        keywords = KAOMOJI_GROUPS.get(group)
        if keywords is None:
            raise SystemExit(f"未知颜文字分类：{group}")
        rows.append((text, group, keywords, "", "kaomoji", "both"))
    return rows


def build() -> str:
    lines: list[str] = []
    for text, name, pinyin, english, kind, lang in PUNCT + load_kaomoji():
        keywords = ",".join(k for k in (pinyin, english) if k)
        lines.append(f"{text}\t{name}\t{keywords}\t{kind}\t{lang}")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description="生成 name_keywords.tsv")
    ap.add_argument("--check", action="store_true", help="只对账，不写文件")
    args = ap.parse_args()
    text = build()
    if args.check:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != text:
            print(f"{OUT} 与生成器不一致；重跑本脚本", file=sys.stderr)
            return 1
        print(f"name_keywords.tsv 一致（{text.count(chr(10))} 条）")
        return 0
    OUT.write_text(text, encoding="utf-8")
    print(f"name_keywords.tsv {text.count(chr(10))} 条 → {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
