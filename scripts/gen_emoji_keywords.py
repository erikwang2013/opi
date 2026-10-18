#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成 `data/raw/emoji_keywords.tsv`（输入即得表情的关键字表，产物入库、可复跑）。

产物（UTF-8，无表头，制表符分隔，**两列**）：
    emoji \\t keywords(逗号分隔,无空格)

## 来源与许可
* **emoji 文本**取自 `data/raw/emoji.tsv`（OpenMoji 常用子集，CC BY-SA 4.0，
  见 `data/raw/LICENSES.md`）；MANUAL 里那 10 个不在 OpenMoji 子集里的表情
  文本直接取 `scripts/symbol_keywords.py` 的码位（仍是 OpenMoji 的字符本身）。
* **关键字表**是本项目人工撰写（MIT，不是上游数据）：
  - 80 个 1F600–1F64F 表情沿用 `symbol_keywords.py` 的 `MANUAL` 关键字
    （与符号面板/搜索同一套词），只按需补少量同义拼音（`MANUAL_EXTRA`）；
  - 其余常用表情的拼音 + 英文 annotation 在下面的 `EXTRA` 表手写。
  - 键用**码位**而不是字符：`❤️` 这类带 VS16 的序列肉眼分不出 `❤` 与 `❤️`，
    写错一个码位很难发现。输出文本一律取 emoji.tsv 的原文（带不带 VS16 由面板表定）。

## 为什么只收常用子集
689 条全量硬塞会让每个短前缀命中几十个 emoji（候选尾部噪声），且大部分表情
没有常用拼音词。目标 150–300 条；`EXTRA` 按「日常聊天会用的」挑。

## 排序（引擎侧）
运行期匹配 = 缓冲是关键字前缀；排序键 = (是否精确, 关键字长度, 关键字序号, 文件序)。
文件序 = emoji.tsv 的分组/常用序，故本脚本**按 emoji.tsv 行序输出**，缺失的
MANUAL 条目追加在尾部。改行序 = 改同分时的先后，别随手重排。

用法（离线，读仓库内已入库数据，不联网）：
  python3 scripts/gen_emoji_keywords.py
"""
import re
import sys

from symbol_keywords import MANUAL

EMOJI_TSV = "data/raw/emoji.tsv"
OUT_TSV = "data/raw/emoji_keywords.tsv"

# MANUAL 那 80 个的补充同义拼音（只补用户真会打的短词，别堆长尾）。
MANUAL_EXTRA: dict[int, str] = {
    0x1F600: "xiao,kaixin",
    0x1F601: "xiao",
    0x1F602: "daxiao,hehe",
    0x1F603: "xiao,kaixin",
    0x1F604: "kaixin",
    0x1F60A: "xiao,kaixin",
    0x1F60D: "xihuan",
    0x1F60E: "shuai",
    0x1F614: "nanguo",
    0x1F622: "ku,shangxin",
    0x1F62D: "ku,kule,shangxin",
    0x1F644: "wuyu",
    0x1F647: "duibuqi",
    0x1F64C: "taihaole",
    0x1F64F: "xiexie,ganxie",
}

# 补充常用表情：码位 → 关键字（拼音在前、英文 annotation 在后）。
EXTRA: dict[int, str] = {
    # ---- 笑脸（1F900+ 的常用新脸）----
    0x1F910: "bizui,zipper,mouth,silent",
    0x1F914: "sikao,xiang,think,thinking,hmm",
    0x1F917: "yongbao,hug,hugging",
    0x1F925: "shuohuang,lie,lying",
    0x1F929: "jingxi,xingxingyan,star,struck,amazed",
    0x1F92A: "fengle,guaizhang,zany,crazy",
    0x1F92B: "xu,shh,quiet",
    0x1F92C: "ma,curse,swear,angry",
    0x1F92D: "touxiao,giggle,hand,mouth",
    0x1F92E: "outu,vomit,sick",
    0x1F92F: "zhenjing,exploding,head,mindblown",
    0x1F970: "xihuan,aixin,love,hearts,smiling",
    0x1F972: "hanleixiao,smile,tear",
    0x1F973: "paidui,partying,party,celebrate",
    0x1F974: "zui,woozy,drunk",
    0x1F978: "weizhuang,disguise",
    0x1F97A: "kelian,qiu,pleading,puppy,please",
    0x1F9D0: "yanshi,monocle",
    0x1FAE0: "ronghua,melt",
    0x1FAE1: "jingli,salute",
    0x1FAE2: "wuzui,hand,mouth",
    0x1FAE3: "toukan,peek",
    # ---- 人物 / 手势 ----
    0x1F44D: "zan,haoping,thumbs,up,like",
    0x1F44E: "cha,buhao,thumbs,down,dislike",
    0x1F44F: "guzhang,clap,applause",
    0x1F44B: "nihao,zaijian,baibai,wave,hello,goodbye",
    0x1F44C: "ok,hao,keyi,hand",
    0x1F4AA: "liliang,qiangzhuang,muscle,strong,power",
    0x1F91D: "woshou,handshake,deal",
    0x270C: "shengli,peace,victory",
    0x1F926: "wunai,facepalm",
    0x1F440: "yanjing,kan,eyes,look",
    # ---- 心 ----
    0x2764: "aixin,xin,hongxin,heart,love",
    0x1F494: "shilian,xinsui,heartbreak,broken,heart",
    0x1F495: "xin,hearts,love,two",
    0x1F496: "xin,hearts,love,sparkling",
    0x1F497: "xin,hearts,love,growing",
    0x1F498: "xin,hearts,love,arrow,cupid",
    0x1F499: "xin,blue,heart,love",
    0x1F49A: "xin,green,heart,love",
    0x1F49B: "xin,yellow,heart,love",
    0x1F493: "xintiao,heart,beating,love",
    # ---- 动物 ----
    0x1F436: "gou,dog",
    0x1F431: "mao,cat",
    0x1F42D: "shu,mouse",
    0x1F43C: "xiongmao,panda",
    0x1F43B: "xiong,bear",
    0x1F430: "tuzi,rabbit",
    0x1F98A: "huli,fox",
    0x1F981: "shizi,lion",
    0x1F42F: "laohu,tiger",
    0x1F437: "zhu,pig",
    0x1F438: "qingwa,frog",
    0x1F435: "houzi,monkey",
    0x1F414: "ji,chicken,rooster",
    0x1F427: "qie,penguin",
    0x1F984: "qilin,unicorn",
    0x1F41D: "mifeng,bee",
    0x1F41B: "chong,bug",
    0x1F40D: "she,snake",
    0x1F42C: "haitun,dolphin",
    0x1F422: "wugui,turtle",
    # ---- 植物 ----
    0x1F339: "meigui,rose",
    0x1F338: "yinghua,sakura,cherry,blossom",
    0x1F33B: "xiangrikui,sunflower",
    0x1F337: "tulip,flower",
    0x1F33A: "hua,flower,hibiscus",
    0x1F490: "hua,flower,bouquet",
    0x1F331: "miaoshu,seedling,plant,sprout",
    0x1F332: "songshu,pine,evergreen,tree",
    0x1F333: "shu,tree",
    0x1F334: "yezishu,palm,tree",
    0x1F335: "xianrenzhang,cactus",
    0x1F340: "sijicao,lucky,clover",
    0x1F341: "fengye,maple,leaf",
    0x1F342: "luoye,fallen,leaf",
    0x1F344: "mogu,mushroom",
    # ---- 食物 ----
    0x1F34E: "pingguo,apple",
    0x1F349: "xigua,watermelon",
    0x1F34C: "xiangjiao,banana",
    0x1F347: "putao,grapes",
    0x1F353: "caomei,strawberry",
    0x1F352: "yingtao,cherry,cherries",
    0x1F351: "taozi,peach",
    0x1F34B: "ningmeng,lemon",
    0x1F34D: "boluo,pineapple",
    0x1F96D: "mangguo,mango",
    0x1F345: "fanqie,tomato",
    0x1F354: "hanbao,hamburger,burger",
    0x1F355: "pisa,pizza",
    0x1F35F: "shutiao,fries",
    0x1F35C: "mian,noodles,bowl",
    0x1F35A: "mi,rice",
    0x1F363: "shousi,sushi",
    0x1F370: "dangao,cake",
    0x1F382: "shengri,birthday,cake",
    0x1F369: "tiantianquan,doughnut,donut",
    0x1F36A: "binggan,cookie",
    0x1F36B: "qiaokeli,chocolate",
    0x1F36C: "tang,candy,sweet",
    0x1F36D: "bangbangtang,lollipop",
    0x2615: "kafei,coffee,tea,hot",
    0x1F375: "cha,tea,green",
    0x1F37A: "pijiu,beer",
    0x1F37B: "ganbei,cheers,beer",
    0x1F377: "hongjiu,wine,glass",
    0x1F942: "ganbei,cheers,glasses",
    0x1F374: "chazi,fork,knife",
    # ---- 活动 ----
    0x1F389: "qingzhu,celebrate,party,popper",
    0x1F38A: "caidai,confetti,party",
    0x1F381: "liwu,gift,present",
    0x1F388: "qiqiu,balloon",
    0x1F383: "nangua,halloween,pumpkin",
    0x1F384: "shengdan,christmas,tree",
    0x1F386: "yanhua,fireworks",
    0x2728: "shanshuo,sparkles,star,shine",
    0x1F3A4: "maikefeng,microphone,mic,ktv",
    0x1F3A7: "erji,headphone,music",
    0x1F3B8: "jita,guitar,music",
    0x1F3B9: "gangqin,piano,music",
    0x1F3AE: "youxi,game,controller",
    0x1F3AF: "mubiao,target,bullseye",
    0x1F3C6: "jiangbei,trophy,champion",
    0x1F947: "jinpai,gold,medal,first",
    0x26BD: "zuqiu,soccer,football",
    0x1F3C0: "lanqiu,basketball",
    0x1F3BE: "wangqiu,tennis",
    0x1F3B3: "baoling,bowling",
    0x1F004: "majiang,mahjong",
    0x1F0CF: "poker,joker,card",
    # ---- 旅行 / 地点 / 天气 ----
    0x1F697: "qiche,car,automobile",
    0x1F695: "chuzuche,taxi,cab",
    0x1F68C: "gongjiao,bus",
    0x1F6B2: "zixingche,bike,bicycle",
    0x2708: "feiji,airplane,plane",
    0x1F680: "huojian,rocket,launch",
    0x1F681: "zhishengji,helicopter",
    0x1F6A2: "lunchuan,ship,boat",
    0x26F5: "fanchuan,sailboat,boat",
    0x2693: "mao,anchor",
    0x1F682: "huoche,train,locomotive",
    0x1F684: "gaotie,train,highspeed",
    0x1F3E0: "fangzi,house,home",
    0x1F3E5: "yiyuan,hospital",
    0x1F3EB: "xuexiao,school",
    0x1F3E6: "yinhang,bank",
    0x26EA: "jiaotang,church",
    0x1F3D6: "shatan,beach",
    0x1F3D4: "xueshan,mountain,snow",
    0x1F30B: "huoshan,volcano",
    0x1F30D: "diqiu,earth,globe",
    0x1F319: "yueliang,moon",
    0x2600: "taiyang,sun",
    0x2B50: "xingxing,star",
    0x1F308: "caihong,rainbow",
    0x2744: "xuehua,snowflake,snow",
    0x26A1: "shandian,lightning,power",
    0x1F525: "huo,fire,hot",
    0x1F30A: "lang,wave,water",
    0x23F0: "naozhong,alarm,clock",
    0x231A: "shoubiao,watch,clock",
    # ---- 物品 ----
    0x1F4F1: "shouji,phone,mobile",
    0x1F4BB: "diannao,laptop,computer",
    0x2328: "jianpan,keyboard",
    0x1F5A5: "diannao,desktop,computer",
    0x1F4F7: "zhaoxiangji,camera,photo",
    0x1F3A5: "sheyingji,camera,movie",
    0x1F4A1: "dengpao,light,bulb,idea",
    0x1F511: "yaoshi,key",
    0x1F512: "suo,lock",
    0x2702: "jiandao,scissors,cut",
    0x1F4DD: "biji,memo,note",
    0x270F: "qianbi,pencil",
    0x1F4DA: "shu,books",
    0x1F4B0: "qian,money,bag",
    0x1F4B3: "xinyongka,credit,card",
    0x1F4B5: "meiyuan,dollar,money",
    0x1F48D: "jiezhi,ring",
    0x1F451: "wangguan,crown,king",
    0x1F393: "biye,graduation,cap",
    0x1F3A9: "maozi,hat,top",
    0x1F453: "yanjing,glasses",
    0x1F454: "lingdai,tie,necktie",
    0x1F45F: "xie,shoe,running",
    0x1F392: "shubao,backpack,bag",
    0x1F4BC: "gongwenbao,briefcase",
    0x1F4C5: "rili,calendar",
    0x1F6AA: "men,door",
    0x1F6BD: "cesuo,toilet",
    0x1F6C1: "zaopeng,bathtub",
    0x1F48A: "yao,pill,medicine",
    0x1F489: "zhushe,syringe,injection",
    0x1FA78: "xue,blood",
    0x1F528: "chui,hammer",
    0x2699: "chilun,gear,settings",
    # ---- 符号 ----
    0x2705: "dui,check,correct,ok",
    0x274C: "cuo,cross,wrong",
    0x2714: "dui,check",
    0x2716: "cuo,multiply,times",
    0x2753: "wenhao,question",
    0x2757: "jingtan,exclamation",
    0x26A0: "jinggao,warning",
    0x267B: "huishou,recycle",
    0x1F4AF: "manfen,hundred,perfect",
    0x1F4A4: "shuijiao,sleep,zzz",
    0x1F4AC: "duihua,speech,chat",
    0x1F4AD: "xiang,thought,think",
    0x23F3: "shajian,hourglass,time",
    0x2795: "jia,plus",
    0x1F4A5: "pengzhuang,boom,collision",
    0x1F4A6: "han,sweat,water",
    0x1F4A8: "pao,dash,wind",
    0x1F4AB: "yun,dizzy,star",
    0x1F4A2: "shengqi,anger",
    0x1F48B: "qinwen,kiss,lip",
}

# 关键字只允许小写 ASCII 字母数字：引擎按字节前缀匹配，其它形态永远搜不到。
KEY_RE = re.compile(r"[a-z0-9]+")


def keywords_for(cp: int) -> list[str] | None:
    """该码位的最终关键字：MANUAL 原词 + 补充，或 EXTRA。未收录 → None。"""
    if 0x1F600 <= cp <= 0x1F64F:  # MANUAL 的 80 个表情全量
        assert cp in MANUAL, f"U+{cp:04X} 在 MANUAL 范围内却没有条目"
        keys = MANUAL[cp][1].split(",")
        keys += MANUAL_EXTRA.get(cp, "").split(",")
    elif cp in EXTRA:
        keys = EXTRA[cp].split(",")
    else:
        return None
    return list(dict.fromkeys(k for k in keys if k))  # 去重保序


def main() -> None:
    # emoji.tsv 行序 = 面板分组序；base 码位 → 原文（可能带 VS16）
    order: list[tuple[int, str]] = []
    seen_cp: set[int] = set()
    with open(EMOJI_TSV, encoding="utf-8") as f:
        for line in f:
            text = line.split("\t", 1)[0]
            if not text:
                continue
            base = [c for c in text if c != "️"]
            if len(base) != 1:  # 组合序列（键帽/ZWJ）不收
                continue
            cp = ord(base[0])
            if cp not in seen_cp:
                seen_cp.add(cp)
                order.append((cp, text))

    # EXTRA 的每条都必须能在 emoji.tsv 找到：文本唯一来源是面板表，键错一个码位
    # 会静默丢掉一条（这正是用码位作键要防的错，故在这里响亮地拦下）。
    missing_extra = sorted(set(EXTRA) - seen_cp)
    assert not missing_extra, (
        "EXTRA 里这些码位不在 emoji.tsv：" + ", ".join(f"U+{c:04X}" for c in missing_extra)
    )

    rows: list[tuple[str, list[str]]] = []
    used: set[int] = set()
    for cp, text in order:
        keys = keywords_for(cp)
        if keys:
            rows.append((text, keys))
            used.add(cp)
    # MANUAL 里不在 OpenMoji 常用子集的 10 个：文本取码位本身，追加在尾部
    for cp in sorted(c for c in MANUAL if 0x1F600 <= c <= 0x1F64F and c not in used):
        rows.append((chr(cp), keywords_for(cp) or []))

    # ---- 生成期自检 ----
    assert 150 <= len(rows) <= 300, f"条数 {len(rows)} 超出 150–300（控制体积/噪声）"
    texts = [t for t, _ in rows]
    assert len(texts) == len(set(texts)), "emoji 文本重复"
    all_keys: dict[str, list[str]] = {}
    for text, keys in rows:
        assert keys, f"{text} 关键字为空"
        assert len(keys) == len(set(keys)), f"{text} 关键字重复：{keys}"
        for k in keys:
            assert KEY_RE.fullmatch(k), f"{text} 关键字 {k!r} 非小写 ASCII 字母数字"
        for k in keys:
            all_keys.setdefault(k, []).append(text)

    # 用户可感知的必需映射（spec 例：weixiao→😊、smile→😄、ku→😭；hello/nihao→👋）
    for key, want in [("weixiao", "😊"), ("smile", "😄"), ("ku", "😭"),
                      ("nihao", "👋"), ("hello", "👋")]:
        assert want in all_keys.get(key, []), f"{key} 必须命中 {want}"
    smileys = [t for t, ks in rows if "xiao" in ks]
    assert len(smileys) >= 2, f"xiao 应命中多个笑脸，实得 {smileys}"

    with open(OUT_TSV + ".tmp", "w", encoding="utf-8") as f:
        for text, keys in rows:
            f.write(f"{text}\t{','.join(keys)}\n")
    import os
    os.replace(OUT_TSV + ".tmp", OUT_TSV)

    print(f"{len(rows)} 条 → {OUT_TSV}"
          f"（MANUAL {sum(1 for t, _ in rows if 0x1F600 <= ord(t[0]) <= 0x1F64F)} 条）")


if __name__ == "__main__":
    sys.exit(main())
