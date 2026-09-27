#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成符号面板数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，无表头，制表符分隔）：
  data/raw/symbol_blocks.tsv   id \\t start(十六进制) \\t end \\t name \\t common(0/1)
  data/raw/symbols.tsv         text \\t name \\t keywords(逗号分隔,无空格) \\t block_id \\t emoji(0/1)

落在 `data/raw/` 而非任务书写的 `data/generated/`：消费侧
`engine-core/src/symbols.rs::builtin()` 用 `include_str!("../../../data/raw/symbols.tsv")`
**编译期**嵌入这两份表，路径写死；且 `data/generated/` 在本仓库只放 .opid 二进制，
脚本生成的 TSV（trad_hanzi.tsv / trad_phrases.tsv）一向在 `data/raw/`。
两处必须同改才编得过 —— 改目录时同时改本行与 symbols.rs，否则 cargo 直接编译失败。

数据来源：**UnicodeData.txt**（UCD 18.0.0，与 scripts/hanzi_freq.py 同一版本目录、
同一份 Unicode License，见 data/raw/LICENSES.md —— 零新增上游、零新增许可证）。
区块范围与字符名系统性取自它，**不手工维护码位表**：升级 UCD 只需换版本号 + SHA。

## 区块
沿用引擎内置声明（engine-core/src/symbols.rs `builtin()`）6 个区块中的 **5 个**，
id 原样保留（1/2/3/4/6，见 BLOCKS 注释为什么不重编号）。**id 5「CJK 扩展 A」
（U+3400–4DBF）被移除** —— **扩展 A 与生僻字任务重叠，故不单列符号区块**（扩展 A
已按拼音进词库，见任务一：打 qiu 出 㐀）。详细理由：
  1. 扩展 A 的 Unicode 名一律是 `CJK UNIFIED IDEOGRAPH-3400` 这种码位标签，分词后
     keywords 只能是 cjk/unified/ideograph/3400 —— 6592 条彼此不可区分，
     面板的**搜索**（SymbolEngine::search 是关键字前缀匹配）对它完全失效；
  2. 面板的**浏览**同样失效：8 列网格里 6592 个无标签汉字，用户不可能"看到才想起"；
     且 Android 的「全部」页取 `search_symbols("")` 全量，等于每次开面板多传
     6592 个字符串过 JNI；
  3. 它与本次任务一重叠且任务一做得更好：扩展 A 已按拼音进词库（打 qiu 出 㐀），
     而**不知道读音**正是扩展 A 的常态 —— 塞进符号面板只是把"翻三页候选"
     换成"翻一千屏网格"。
要恢复：往 BLOCKS 加回 `(5, 0x3400, 0x4DBF, "CJK 扩展 A", 0)`，本脚本照常产出
（约 +6592 条），但请先解决上面第 1、2 条，否则只是把空声明换成噪声。

## 关键字（搜索靠它）
* `MANUAL` 表**人工写**（拼音 + 英文），覆盖常用符号：CJK 符号全量、
  1F600–1F64F 那 80 个表情全量、几何图形与杂项符号的常用形、平假名全量
  （训令式与黑本式罗马字都给：si/shi、ti/chi、tu/tsu、hu/fu、zi/ji）。共约 280 条。
* 未人工写的条目按 Unicode 名兜底分词：`BLACK UP-POINTING TRIANGLE` →
  black,up,pointing,triangle（按非字母数字切分，去 WITH/AND/OF 等虚词）。
  **局限：兜底只保证英文可搜，拼音搜不到** —— 生僻条目（几何图形的象限填充、
  杂项符号里的交通标志/占星符号）体验取决于用户是否碰巧输英文词。
  这是兜底不是等价替代；要提升就得继续往 MANUAL 里补。
  ⚠️ 2026-09-28 扩块后**这条局限成了主路径**：新增的约 2750 条里绝大多数只有
  英文名分词关键字 —— 打 `xiao` 出 😄 仍成立（那 80 条在 MANUAL 里），
  但打 `huojian` 出 🚀 **不成立**，得打 `rocket`。中文/拼音侧是 **CLDR 名称
  搜索**那个任务（spec 第 60/86 行），本轮有意不做：它要么新引一份上游
  （`cldr-json` 的 annotations/zh，~1MB，另有 LICENSE 与版本 pin 要接），
  要么手写三千条中文名（不可复核）。两者都超出「扩覆盖」这一半。
* 旧内置样例的首字母缩写约定（上三角 sjx）**不再机械生成**：机械生成要配一张
  几千字的拼音首字母表，而引擎是前缀匹配 —— 输 "s" 已经能命中 "sanjiao"，
  缩写只在用户恰好输全缩写时有用。需要的那几条（▲ sjx）手写进 MANUAL 即可。
* emoji 列与 Android 侧判定必须一致：`SymbolCatalog.isEmoji()` 用"含代理对"（非 BMP）
  判表情，故 emoji=1 ⟺ 非 BMP，生成期断言，`rare_coverage.rs` 再锁一遍。
  **代价（如实记）**：这条口径是「非 BMP」而非 UTS#51 的 Emoji 属性，那批 **BMP 里的
  真 emoji**（⌚ U+231A、⏰ U+23F0、⭐ U+2B50、✂ U+2702）数据里 emoji=0，**不进
  Android「表情」页**，只能从「全部」页/关键字搜到。要修得先改 Android 的判定口径
  （`isEmoji` 改查表或改走 JNI 的 emoji 标记），是跨端改动，不在本脚本范围内。

用法（需要网络下载 UnicodeData.txt）：
  python3 scripts/gen_symbols.py
"""
import hashlib
import os
import re
import sys
import urllib.request

# ---- 上游 pin（与 hanzi_freq.py 同版本目录）----
UNIHAN_VERSION = "18.0.0"
UNICODE_DATA_URL = f"https://www.unicode.org/Public/{UNIHAN_VERSION}/ucd/UnicodeData.txt"
UNICODE_DATA_SHA256 = "0736451de439ae7baf1425136617da495e09ee5afbe6e394374db7009ea08950"

# (id, start, end, name, common)。id 与 engine-core `builtin()` 逐一对应，
# **id 5 空出不重编号**：平假名若改成 5，任何仍认为「5 = 扩展 A」的旧代码会
# **静默**取到平假名（错得看不出来）；留空则它拿到空列表，错得响。
#
# 1–6 是原有块，**顺序与内容不许动**（决定 symbols.tsv 行序 = 面板内条目序，
# 且被 symbol_mode.rs 等用例钉着）；7 起为 2026-09-28 补的 emoji 覆盖块，
# 一律追加在尾部，既有条目的行号因此逐字节不变（这条在生成器里是结构保证：
# 新区块只 append，不改写前缀）。
#
# 新增块取自 UTS#51 §2.1 的 emoji 区块表（Unicode 15.1），与既有块 2/3 同一口径：
# 一个 Unicode 区块进表就是整块，不按「这块里有多少个是 emoji」筛。
#
# ⚠️ 规范里的「~3700」是 emoji-test.txt 的完全限定**序列**数（15.1 实测 3776），
# **不是码位数**，本表到不了：条目 text 必须单字符（rare_coverage.rs
# `symbol_data_wellformed`），且 emoji ⟺ 非 BMP。本表给的是 emoji 区块内全部已分配码位。
#
# ⚠️ 非 BMP 的「非图形」块（麻将 1F000 / 多米诺 1F030 / 扑克 1F0A0 / 带圈字母数字
# 补充 1F100 / 带圈表意文字补充 1F200）有意不收：emoji 口径是「非 BMP」，而
# `candidates.rs` 单键走 `search_emoji` 且命中按码位升序平局排序，这五块**必然**
# 占满每个字母键的前 8 槽（实测打 s 出麻将牌而非 😀）。代价：那 58 条带 Emoji 属性的
# 字符（🀄🃏🅰🅱🈚🈯…，含区域指示符）不在表内；要恢复先解决上面那条平局排序。
# BMP 的 emoji 块不受此影响（emoji=0，不进 emoji 通道），故 7–13 整块收。
BLOCKS = [
    (1, 0x3000, 0x303F, "CJK 符号", 1),
    (2, 0x25A0, 0x25FF, "几何图形", 0),
    (3, 0x2600, 0x26FF, "杂项符号", 0),
    (4, 0x1F600, 0x1F64F, "表情符号", 0),
    (6, 0x3040, 0x309F, "平假名", 0),
    # ---- 2026-09-28 追加：UTS#51 emoji 区块（BMP 段，emoji=0）----
    (7, 0x2190, 0x21FF, "箭头", 0),
    (8, 0x2300, 0x23FF, "杂项技术符号", 0),
    (9, 0x2460, 0x24FF, "带圈字母数字", 0),
    (10, 0x2700, 0x27BF, "装饰符号", 0),
    (11, 0x2900, 0x297F, "补充箭头-B", 0),
    (12, 0x2B00, 0x2BFF, "杂项符号与箭头", 0),
    (13, 0x3200, 0x32FF, "带圈中日韩字母", 0),
    # ---- 2026-09-28 追加：UTS#51 emoji 区块（非 BMP 图形段，emoji=1）----
    (14, 0x1F300, 0x1F5FF, "杂项符号与图形", 0),
    (15, 0x1F680, 0x1F6FF, "交通与地图符号", 0),
    (16, 0x1F780, 0x1F7FF, "几何图形扩展", 0),
    (17, 0x1F900, 0x1F9FF, "补充符号与图形", 0),
    (18, 0x1FA70, 0x1FAFF, "符号与图形扩展-A", 0),
]

# 旧硬编码符号表的 8 条 —— 生成器是「在现有基础上扩充」，不是整表替换：
# 这些字与其 keywords **一条都不能少**。钉的是**用户可见行为**，不是数据美观：
# opi-ffi 的 `cabi_symbols_blocks_and_search` 与 api `symbol_search_and_blocks`
# 都断言「搜索 he 命中 ♥」。丢关键字不会编译失败，只会让远处的 ffi 测试红。
# 码位 → 必须保留的 keywords（旧表原样 + ♥ 的 he）。
LEGACY: dict[int, tuple[str, ...]] = {
    0x3001: ("dun", "comma"),
    0x3002: ("ju", "period"),
    0x3008: ("shu",),
    0x25B2: ("sjx", "triangle"),
    0x2665: ("heart", "ai", "xin", "he"),
    0x2605: ("star", "xing"),
    0x1F604: ("xiao", "smile", "laugh"),
    0x3042: ("a",),
}

# 不入库的字符类别：Cc/Cf/Cs/Co 不可见；Mn/Me 是**组合**记号 ——
# 在 8 列网格里它是附在上一个字上的浮点，单独插入也只会粘住前一个字符，
# 在符号面板里没有独立意义。本范围内命中 6 个：302A–302D（声调符）、3099–309A（浊点）。
# Zs 保留（U+3000 全角空格是常用符号）。
EXCLUDE_CATEGORIES = {"Cc", "Cf", "Cs", "Co", "Mn", "Me", "Zl", "Zp"}

# 人工关键字表：码位 → (中文名, "拼音,英文,...")。**用码位而非字符作键**：
# U+3000 这类字符在源码里看不出来，写错一个不可见字符很难发现。
# 未列出的条目 name 取 Unicode 名、keywords 取名字分词。
MANUAL: dict[int, tuple[str, str]] = {
    # ---- CJK 符号（全量 60 条）----
    0x3000: ("全角空格", "quanjiao,kongge,space"), 0x3001: ("顿号", "dun,comma"),
    0x3002: ("句号", "ju,period,fullstop"), 0x3003: ("同上号", "tongshang,ditto"),
    0x3004: ("工业标准记号", "jis,industry"), 0x3005: ("叠字记号", "diezi,iteration"),
    0x3006: ("缔结记号", "dijie,closing"), 0x3007: ("〇", "ling,zero"),
    0x3008: ("左尖括号", "jiankuohao,jian,shu,angle,left"), 0x3009: ("右尖括号", "jiankuohao,jian,angle,right"),
    0x300A: ("左书名号", "shuminghao,book,left"), 0x300B: ("右书名号", "shuminghao,book,right"),
    0x300C: ("左直角引号", "zhijiao,yinhao,corner,left"), 0x300D: ("右直角引号", "zhijiao,yinhao,corner,right"),
    0x300E: ("左双直角引号", "shuangzhijiao,white,corner,left"), 0x300F: ("右双直角引号", "shuangzhijiao,white,corner,right"),
    0x3010: ("左方头括号", "fangtou,lenticular,left"), 0x3011: ("右方头括号", "fangtou,lenticular,right"),
    0x3012: ("邮便记号", "youbian,postal"), 0x3013: ("下驮记号", "geta"),
    0x3014: ("左六角括号", "liujiao,tortoise,left"), 0x3015: ("右六角括号", "liujiao,tortoise,right"),
    0x3016: ("左白方头括号", "baifangtou,white,lenticular,left"), 0x3017: ("右白方头括号", "baifangtou,white,lenticular,right"),
    0x3018: ("左白六角括号", "bailiujiao,white,tortoise,left"), 0x3019: ("右白六角括号", "bailiujiao,white,tortoise,right"),
    0x301A: ("左双方括号", "shuangfang,white,square,left"), 0x301B: ("右双方括号", "shuangfang,white,square,right"),
    0x301C: ("波浪线", "bolang,wave,dash"), 0x301D: ("反向双撇号", "fanxiang,prime,quote"),
    0x301E: ("双撇号", "shuangpie,prime,quote"), 0x301F: ("低双撇号", "dishuangpie,low,prime"),
    0x3020: ("邮便记号笑脸", "youbianlian,postal,face"), 0x3021: ("苏州码一", "suzhouma,yi,hangzhou,one"),
    0x3022: ("苏州码二", "suzhouma,er,hangzhou,two"), 0x3023: ("苏州码三", "suzhouma,san,hangzhou,three"),
    0x3024: ("苏州码四", "suzhouma,si,hangzhou,four"), 0x3025: ("苏州码五", "suzhouma,wu,hangzhou,five"),
    0x3026: ("苏州码六", "suzhouma,liu,hangzhou,six"), 0x3027: ("苏州码七", "suzhouma,qi,hangzhou,seven"),
    0x3028: ("苏州码八", "suzhouma,ba,hangzhou,eight"), 0x3029: ("苏州码九", "suzhouma,jiu,hangzhou,nine"),
    0x302E: ("韩文单点声调", "hanwen,dian,hangul,tone"), 0x302F: ("韩文双点声调", "hanwen,shuangdian,hangul,tone"),
    0x3030: ("波浪破折号", "bolang,wavy,dash"), 0x3031: ("竖排重复记号", "shupai,chongfu,vertical,repeat"),
    0x3032: ("竖排浊音重复", "shupai,zhuoyin,vertical,voiced"), 0x3033: ("竖排重复上半", "shupai,shangban,upper,repeat"),
    0x3034: ("竖排浊音上半", "shupai,zhuoyin,upper,voiced"), 0x3035: ("竖排重复下半", "shupai,xiaban,lower,repeat"),
    0x3036: ("带圈邮便记号", "quanyoubian,circled,postal"), 0x3037: ("电报换行号", "dianbao,huanhang,telegraph"),
    0x3038: ("苏州码十", "suzhouma,shi,hangzhou,ten"), 0x3039: ("苏州码廿", "suzhouma,nian,hangzhou,twenty"),
    0x303A: ("苏州码卅", "suzhouma,sa,hangzhou,thirty"), 0x303B: ("竖排叠字记号", "shupai,diezi,vertical,iteration"),
    0x303C: ("升记号", "sheng,masu"), 0x303D: ("歌记号", "ge,part,alternation"),
    0x303E: ("异体字指示符", "yiti,variation,indicator"), 0x303F: ("半填空格", "bantian,half,fill,space"),
    # ---- 几何图形（常用 46；其余 50 按 Unicode 名兜底）----
    0x25A0: ("实心方块", "fangkuai,square,black"), 0x25A1: ("空心方块", "fangkuai,square,white"),
    0x25A2: ("圆角方块", "yuanjiao,rounded,square"), 0x25AA: ("小实心方块", "xiaofangkuai,small,square,black"),
    0x25AB: ("小空心方块", "xiaofangkuai,small,square,white"), 0x25AC: ("实心横长方形", "changfangxing,rectangle,black"),
    0x25AD: ("空心横长方形", "changfangxing,rectangle,white"), 0x25AE: ("实心竖长方形", "shuchangfangxing,vertical,rectangle"),
    0x25AF: ("空心竖长方形", "shuchangfangxing,vertical,rectangle,white"),
    0x25B2: ("实心上三角", "sanjiao,sjx,shangsanjiao,triangle,up"),
    0x25B3: ("空心上三角", "sanjiao,sjx,shangsanjiao,triangle,up,white"),
    0x25B4: ("小实心上三角", "xiaosanjiao,up,small,triangle"), 0x25B5: ("小空心上三角", "xiaosanjiao,up,small,triangle,white"),
    0x25B6: ("实心右三角", "sanjiao,you,sanjiao,triangle,right,play"),
    0x25B7: ("空心右三角", "sanjiao,you,sanjiao,triangle,right,white"), 0x25B8: ("小实心右三角", "xiaosanjiao,right,small"),
    0x25B9: ("小空心右三角", "xiaosanjiao,right,small,white"), 0x25BA: ("实心右指针", "youzhizhen,pointer,right"),
    0x25BB: ("空心右指针", "youzhizhen,pointer,right,white"), 0x25BC: ("实心下三角", "sanjiao,xia,xiasanjiao,triangle,down"),
    0x25BD: ("空心下三角", "sanjiao,xia,xiasanjiao,triangle,down,white"), 0x25BE: ("小实心下三角", "xiaosanjiao,down,small"),
    0x25BF: ("小空心下三角", "xiaosanjiao,down,small,white"), 0x25C0: ("实心左三角", "sanjiao,zuo,zuosanjiao,triangle,left"),
    0x25C1: ("空心左三角", "sanjiao,zuo,zuosanjiao,triangle,left,white"), 0x25C2: ("小实心左三角", "xiaosanjiao,left,small"),
    0x25C3: ("小空心左三角", "xiaosanjiao,left,small,white"), 0x25C4: ("实心左指针", "zuozhizhen,pointer,left"),
    0x25C5: ("空心左指针", "zuozhizhen,pointer,left,white"), 0x25C6: ("实心菱形", "lingxing,diamond,black"),
    0x25C7: ("空心菱形", "lingxing,diamond,white"), 0x25C8: ("套小菱形", "lingxing,diamond,containing"),
    0x25C9: ("鱼眼", "yuyan,fisheye"), 0x25CA: ("斜方形", "xiefangxing,lozenge"),
    0x25CB: ("空心圆", "yuan,circle,white"), 0x25CE: ("靶心", "baxin,bullseye,target"),
    0x25CF: ("实心圆", "yuan,circle,black"), 0x25D0: ("左半黑圆", "banheiyuan,left,half,circle"),
    0x25D1: ("右半黑圆", "banheiyuan,right,half,circle"), 0x25E0: ("上半圆", "banyuan,upper,half,circle"),
    0x25E1: ("下半圆", "banyuan,lower,half,circle"), 0x25E2: ("实心右下三角", "sanjiao,lower,right,triangle"),
    0x25E3: ("实心左下三角", "sanjiao,lower,left,triangle"), 0x25E4: ("实心左上三角", "sanjiao,upper,left,triangle"),
    0x25E5: ("实心右上三角", "sanjiao,upper,right,triangle"), 0x25EF: ("大空心圆", "dayuan,large,circle"),
    0x25FB: ("中空心方块", "fangkuai,medium,square,white"), 0x25FC: ("中实心方块", "fangkuai,medium,square,black"),
    0x25FD: ("中小空心方块", "fangkuai,small,medium,white"), 0x25FE: ("中小实心方块", "fangkuai,small,medium,black"),
    # ---- 杂项符号（常用 92；其余 164 按 Unicode 名兜底）----
    0x2600: ("太阳", "taiyang,sun,sunny"), 0x2601: ("云", "yun,cloud"),
    0x2602: ("雨伞", "yusan,umbrella"), 0x2603: ("雪人", "xueren,snowman"),
    0x2604: ("彗星", "huixing,comet"), 0x2605: ("实心星", "xingxing,xing,star,black"),
    0x2606: ("空心星", "xingxing,xing,star,white"), 0x2607: ("闪电", "shandian,lightning"),
    0x2608: ("雷雨", "leiyu,thunderstorm"), 0x2609: ("日", "ri,sun"),
    0x260E: ("黑电话", "dianhua,telephone,phone"), 0x260F: ("白电话", "dianhua,telephone,phone,white"),
    0x2610: ("空方框", "fangkuang,ballot,box"), 0x2611: ("打勾方框", "dagou,check,ballot,tick"),
    0x2612: ("打叉方框", "dacha,cross,ballot,x"), 0x2614: ("带雨雨伞", "yusan,daiyu,rain,umbrella"),
    0x2615: ("热饮", "reyin,coffee,hot,beverage"), 0x2618: ("三叶草", "sanyecao,shamrock,clover"),
    0x261A: ("黑左指手", "zhishou,index,left,pointing,black"), 0x261B: ("黑右指手", "zhishou,index,right,pointing,black"),
    0x261C: ("白左指手", "zhishou,white,left,pointing"), 0x261D: ("白上指手", "zhishou,up,pointing,index,white"),
    0x261E: ("白右指手", "zhishou,white,right,pointing"), 0x261F: ("白下指手", "zhishou,down,pointing,index,white"),
    0x2620: ("骷髅", "kulou,skull,crossbones"), 0x2621: ("警告", "jinggao,caution,warning"),
    0x2622: ("辐射", "fushe,radioactive"), 0x2623: ("生物危害", "shengwu,weihai,biohazard"),
    0x2624: ("双蛇杖", "shuangshezhang,caduceus"), 0x2625: ("安卡", "anka,ankh"),
    0x2626: ("东正教十字", "dongzhengjiao,cross,orthodox"), 0x2627: ("凯乐符号", "kaile,chi,rho"),
    0x2628: ("洛林十字", "luolin,cross,lorraine"), 0x2629: ("耶路撒冷十字", "yelusaleng,cross,jerusalem"),
    0x262A: ("星月", "xingyue,star,crescent,islam"), 0x262B: ("波斯符号", "bosi,farsi"),
    0x262C: ("阿迪沙克提", "adishakti,khanda"), 0x262D: ("镰刀锤子", "liandao,chuizi,hammer,sickle"),
    0x262E: ("和平", "heping,peace"), 0x262F: ("阴阳", "yinyang,tao,taichi"),
    0x2630: ("乾卦", "qiangua,trigram,heaven"), 0x2631: ("兑卦", "duigua,trigram,lake"),
    0x2632: ("离卦", "ligua,trigram,fire"), 0x2633: ("震卦", "zhengua,trigram,thunder"),
    0x2634: ("巽卦", "xungua,trigram,wind"), 0x2635: ("坎卦", "kangua,trigram,water"),
    0x2636: ("艮卦", "gengua,trigram,mountain"), 0x2637: ("坤卦", "kungua,trigram,earth"),
    0x2638: ("法轮", "falun,dharma,wheel"), 0x2639: ("哭脸", "kulian,frowning,frown"),
    0x263A: ("笑脸", "xiaolian,smiling,smile"), 0x263B: ("黑笑脸", "heixiaolian,smiling,black"),
    0x263C: ("带光芒太阳", "taiyang,sun,rays,white"), 0x263D: ("上弦月", "shangxianyue,moon,quarter"),
    0x263E: ("下弦月", "xiaxianyue,moon,quarter,last"), 0x2640: ("女性", "nvxing,female,woman"),
    0x2641: ("地球", "diqiu,earth"), 0x2642: ("男性", "nanxing,male,man"),
    0x2643: ("木星", "muxing,jupiter"), 0x2644: ("土星", "tuxing,saturn"),
    0x2645: ("天王星", "tianwangxing,uranus"), 0x2646: ("海王星", "haiwangxing,neptune"),
    0x2647: ("冥王星", "mingwangxing,pluto"), 0x2648: ("白羊座", "baiyang,aries,zodiac"),
    0x2649: ("金牛座", "jinniu,taurus,zodiac"), 0x264A: ("双子座", "shuangzi,gemini,zodiac"),
    0x264B: ("巨蟹座", "juxie,cancer,zodiac"), 0x264C: ("狮子座", "shizi,leo,zodiac"),
    0x264D: ("处女座", "chunv,virgo,zodiac"), 0x264E: ("天秤座", "tiancheng,libra,zodiac"),
    0x264F: ("天蝎座", "tianxie,scorpius,zodiac"), 0x2650: ("射手座", "sheshou,sagittarius,zodiac"),
    0x2651: ("摩羯座", "mojie,capricorn,zodiac"), 0x2652: ("水瓶座", "shuiping,aquarius,zodiac"),
    0x2653: ("双鱼座", "shuangyu,pisces,zodiac"), 0x2654: ("白王", "baiwang,chess,king,white"),
    0x2655: ("白后", "baihou,chess,queen,white"), 0x2656: ("白车", "baiju,chess,rook,white"),
    0x2657: ("白象", "baixiang,chess,bishop,white"), 0x2658: ("白马", "baima,chess,knight,white"),
    0x2659: ("白兵", "baibing,chess,pawn,white"), 0x265A: ("黑王", "heiwang,chess,king,black"),
    0x265B: ("黑后", "heihou,chess,queen,black"), 0x265C: ("黑车", "heiju,chess,rook,black"),
    0x265D: ("黑象", "heixiang,chess,bishop,black"), 0x265E: ("黑马", "heima,chess,knight,black"),
    0x265F: ("黑兵", "heibing,chess,pawn,black"), 0x2660: ("黑桃", "heitao,spade,suit"),
    0x2661: ("红桃", "hongtao,heart,suit,white"), 0x2662: ("方块", "fangkuai,diamond,suit"),
    0x2663: ("梅花", "meihua,club,suit"), 0x2664: ("白桃", "baitao,spade,suit,white"),
    0x2665: ("红心", "hongxin,heart,he,ai,xin,suit,love"), 0x2666: ("实心方块", "fangkuai,diamond,suit,black"),
    0x2667: ("白梅花", "baimeihua,club,suit,white"), 0x2668: ("温泉", "wenquan,hot,springs,onsen"),
    0x2669: ("四分音符", "yinfu,note,quarter,music"), 0x266A: ("八分音符", "yinfu,note,eighth,music"),
    0x266B: ("双八分音符", "yinfu,note,beamed,music"), 0x266C: ("双十六分音符", "yinfu,note,sixteenth,music"),
    0x266D: ("降号", "jianghao,flat,music"), 0x266E: ("还原号", "huanyuanhao,natural,music"),
    0x266F: ("升号", "shenghao,sharp,music"), 0x2672: ("循环回收", "xunhuan,huishou,recycle"),
    0x267B: ("黑循环回收", "huishou,recycle,black"), 0x267E: ("永久纸", "yongjiu,permanent,paper"),
    0x267F: ("轮椅", "lunyi,wheelchair,accessible"), 0x2680: ("骰子一", "touzi,dice,one"),
    0x2681: ("骰子二", "touzi,dice,two"), 0x2682: ("骰子三", "touzi,dice,three"),
    0x2683: ("骰子四", "touzi,dice,four"), 0x2684: ("骰子五", "touzi,dice,five"),
    0x2685: ("骰子六", "touzi,dice,six"), 0x2690: ("白旗", "baiqi,flag,white"),
    0x2691: ("黑旗", "heiqi,flag,black"), 0x2692: ("锤镐", "chuigao,hammer,pick"),
    0x2693: ("锚", "mao,anchor"), 0x2694: ("交叉剑", "jiaochajian,swords,crossed"),
    0x2695: ("医神杖", "yishenzhang,aesculapius,staff"), 0x2696: ("天平", "tianping,scales,justice"),
    0x2697: ("蒸馏器", "zhengliuqi,alembic"), 0x2698: ("花", "hua,flower"),
    0x2699: ("齿轮", "chilun,gear,settings"), 0x269A: ("商神杖", "shangshenzhang,hermes,staff"),
    0x269B: ("原子", "yuanzi,atom"), 0x269C: ("鸢尾花", "yuanweihua,fleur,lis"),
    0x269D: ("描边白星", "xingxing,outlined,star,white"), 0x26A0: ("警告", "jinggao,warning"),
    0x26A1: ("高压", "gaoya,high,voltage,lightning"), 0x26AA: ("中空心圆", "yuan,medium,circle,white"),
    0x26AB: ("中实心圆", "yuan,medium,circle,black"), 0x26AD: ("婚姻", "hunyin,marriage"),
    0x26AE: ("离婚", "lihun,divorce"), 0x26B0: ("棺材", "guancai,coffin"),
    0x26B1: ("骨灰瓮", "guhuiweng,funeral,urn"), 0x26BD: ("足球", "zuqiu,soccer,football"),
    0x26BE: ("棒球", "bangqiu,baseball"), 0x26C4: ("无雪雪人", "xueren,snowman"),
    0x26C5: ("多云", "duoyun,sun,cloud"), 0x26C6: ("下雨", "xiayu,rain"),
    0x26C8: ("雷阵雨", "leizhenyu,thunder,cloud,rain"), 0x26CE: ("蛇夫座", "shefuzuo,ophiuchus,zodiac"),
    0x26CF: ("镐", "gao,pick,mining"), 0x26D1: ("安全帽", "anquanmao,helmet,cross"),
    0x26D3: ("锁链", "suolian,chains"), 0x26D4: ("禁止通行", "jinzhi,no,entry,stop"),
    0x26E4: ("五角星", "wujiaoxing,pentagram"), 0x26E9: ("神社", "shenshe,shinto,shrine"),
    0x26EA: ("教堂", "jiaotang,church"), 0x26EB: ("城堡", "chengbao,castle"),
    0x26EC: ("古迹", "guji,historic,site"), 0x26F0: ("山", "shan,mountain"),
    0x26F1: ("沙滩伞", "shatan,san,umbrella,beach"), 0x26F2: ("喷泉", "penquan,fountain"),
    0x26F3: ("高尔夫", "gaoerfu,flag,hole,golf"), 0x26F4: ("渡轮", "dulun,ferry"),
    0x26F5: ("帆船", "fanchuan,sailboat"), 0x26F7: ("滑雪", "huaxue,skier,ski"),
    0x26F8: ("溜冰鞋", "liubingxie,ice,skate"), 0x26F9: ("打球的人", "daqiu,person,ball"),
    0x26FA: ("帐篷", "zhangpeng,tent,camp"), 0x26FD: ("加油站", "jiayouzhan,fuel,pump,gas"),
    # ---- 表情符号（全量 80 条）----
    0x1F600: ("咧嘴笑", "liezuixiao,daxiao,grin,grinning,smile"), 0x1F601: ("露齿笑", "luchixiao,grin,smile,teeth"),
    0x1F602: ("笑哭", "xiaoku,joy,tears,laugh"), 0x1F603: ("开口笑", "kaikouxiao,smile,open,mouth"),
    0x1F604: ("眯眼笑", "miyanxiao,xiao,smile,laugh,happy,smiling"), 0x1F605: ("冷汗笑", "lenghanxiao,sweat,smile"),
    0x1F606: ("露齿眯眼笑", "luchixiao,grin,closed,eyes"), 0x1F607: ("天使笑", "tianshixiao,halo,angel,innocent"),
    0x1F608: ("恶魔笑", "emoxiaoxiao,devil,horns,smiling"), 0x1F609: ("眨眼", "zhayan,wink,flirt"),
    0x1F60A: ("微笑", "weixiao,smile,blush,happy"), 0x1F60B: ("好吃", "haochi,yummy,delicious,tongue"),
    0x1F60C: ("放松", "fangsong,relieved,relax"), 0x1F60D: ("花痴", "huachi,love,heart,eyes"),
    0x1F60E: ("墨镜", "mojing,cool,sunglasses"), 0x1F60F: ("得意", "deyi,smirk,smug"),
    0x1F610: ("无表情", "wubiaoqing,neutral"), 0x1F611: ("面无表情", "wubiaoqing,expressionless,blank"),
    0x1F612: ("不爽", "bushuang,unamused,meh"), 0x1F613: ("冷汗", "lenghan,sweat,cold"),
    0x1F614: ("沉思", "chensi,pensive,sad"), 0x1F615: ("困惑", "kunhuo,confused,puzzled"),
    0x1F616: ("难受", "nanshou,confounded"), 0x1F617: ("亲吻", "qinwen,kissing,kiss"),
    0x1F618: ("飞吻", "feiwen,kiss,blow"), 0x1F619: ("笑着亲", "xiaozheqin,kissing,smiling"),
    0x1F61A: ("闭眼亲", "biyanqin,kissing,closed"), 0x1F61B: ("吐舌", "tushe,tongue,stuck"),
    0x1F61C: ("眨眼吐舌", "tushe,zhayan,tongue,wink,squint"), 0x1F61D: ("眯眼吐舌", "tushe,tongue,closed,eyes"),
    0x1F61E: ("失望", "shiwang,disappointed,sad"), 0x1F61F: ("担忧", "danyou,worried,concerned"),
    0x1F620: ("生气", "shengqi,angry,mad"), 0x1F621: ("愤怒", "fennu,pouting,rage"),
    0x1F622: ("流泪", "liulei,crying,cry,sad,tears"), 0x1F623: ("忍耐", "rennai,persevering,struggle"),
    0x1F624: ("得意洋洋", "deyiyangyang,triumph,huff"), 0x1F625: ("苦笑", "kuxiao,disappointed,relieved,sad"),
    0x1F626: ("皱眉张嘴", "zhoumei,frowning,frown"), 0x1F627: ("痛苦", "tongku,anguished"),
    0x1F628: ("害怕", "haipa,fearful,scared"), 0x1F629: ("疲倦", "pijuan,weary,tired"),
    0x1F62A: ("困", "kun,sleepy,sleep"), 0x1F62B: ("累", "lei,tired,exhausted"),
    0x1F62C: ("龇牙", "ziya,grimacing,awkward"), 0x1F62D: ("大哭", "daku,loudly,crying,sob"),
    0x1F62E: ("惊讶", "jingya,surprised,open,mouth"), 0x1F62F: ("静默", "jingmo,hushed,silence"),
    0x1F630: ("冷汗张口", "lenghan,sweat,open,mouth,anxious"), 0x1F631: ("惊恐", "jingkong,scream,fear,shock"),
    0x1F632: ("震惊", "zhenjing,astonished,surprised"), 0x1F633: ("脸红", "lianhong,flushed,blush,shy"),
    0x1F634: ("睡觉", "shuijiao,sleeping,sleep,zzz"), 0x1F635: ("晕", "yun,dizzy,dead"),
    0x1F636: ("无嘴", "wuzui,no,mouth,silent"), 0x1F637: ("口罩", "kouzhao,medical,mask,sick"),
    0x1F638: ("猫咧嘴笑", "mao,grinning,cat,smile"), 0x1F639: ("猫笑哭", "mao,cat,tears,joy"),
    0x1F63A: ("猫开口笑", "mao,cat,smiling,open"), 0x1F63B: ("猫花痴", "mao,cat,heart,love"),
    0x1F63C: ("猫坏笑", "mao,cat,wry,smile"), 0x1F63D: ("猫亲吻", "mao,cat,kissing"),
    0x1F63E: ("猫愤怒", "mao,cat,pouting,angry"), 0x1F63F: ("猫流泪", "mao,cat,crying"),
    0x1F640: ("猫疲倦", "mao,cat,weary"), 0x1F641: ("轻皱眉", "qingzhoumei,slightly,frowning,sad"),
    0x1F642: ("轻微笑", "qingweixiao,slightly,smiling,happy"), 0x1F643: ("倒脸", "daolian,upside,down,face"),
    0x1F644: ("翻白眼", "fanbaiyan,rolling,eyes"), 0x1F645: ("不行手势", "buxing,no,good,gesture"),
    0x1F646: ("好的手势", "haode,ok,gesture"), 0x1F647: ("鞠躬", "jugong,bowing,person,sorry"),
    0x1F648: ("非礼勿视", "feiliwushi,see,no,evil,monkey"), 0x1F649: ("非礼勿听", "feiliwuting,hear,no,evil,monkey"),
    0x1F64A: ("非礼勿言", "feiliwuyan,speak,no,evil,monkey"), 0x1F64B: ("举手", "jushou,raising,hand,happy"),
    0x1F64C: ("欢呼", "huanhu,raising,both,hands,celebration"), 0x1F64D: ("皱眉的人", "zhoumei,person,frowning"),
    0x1F64E: ("撅嘴的人", "juezui,person,pouting"), 0x1F64F: ("祈祷", "qidao,folded,hands,please,thanks"),
    # ---- 平假名块里非「字母」的 5 条（MANUAL 优先于 kana_row 的罗马字推导）----
    0x309B: ("浊音符", "zhuoyinfu,zhuoyin,voiced,sound,mark"),
    0x309C: ("半浊音符", "banzhuoyinfu,handakuten,semi,voiced,mark"),
    0x309D: ("平假名叠字符", "pingjiadie,diezi,iteration,hiragana"),
    0x309E: ("平假名浊音叠字符", "zhuoyindie,voiced,iteration,hiragana"), 0x309F: ("平假名合字yori", "yori,hezi,digraph,hiragana"),
}

# 平假名罗马字例外表：Unicode 名用训令式（SI/TI/TU/HU/ZI/DI/DU），
# 输入法习惯是黑本式（shi/chi/tsu/fu/ji）—— 名字给不了的按这里覆盖。
# 未列出的直接取 Unicode 名末段（あ 取 "A"→a，ん 取 "N"→n）。
KANA_ROMAJI: dict[int, str] = {
    0x3057: "shi", 0x3058: "ji", 0x3061: "chi", 0x3062: "ji",
    0x3063: "tsu", 0x3064: "tsu", 0x3065: "zu", 0x3075: "fu",
    0x3090: "wi", 0x3091: "we",
}
# 促音 っ 的另一个常见写法（xtu/ltu 与 xtsu/ltsu 都有人打）。
KANA_EXTRA = {0x3063: ["xtu", "ltu"]}


def fetch(url: str) -> str:
    with urllib.request.urlopen(url, timeout=180) as r:
        return r.read().decode("utf-8")


def parse_unicode_data(text: str) -> dict[int, tuple[str, str]]:
    """UnicodeData.txt → {码位: (字符名, 总类)}。First/Last 区间标记按区间展开。"""
    out: dict[int, tuple[str, str]] = {}
    pending: tuple[int, str, str] | None = None
    for line in text.splitlines():
        fields = line.split(";")
        if len(fields) < 3:
            continue
        cp = int(fields[0], 16)
        name, cat = fields[1], fields[2]
        if name.endswith(", First>"):
            pending = (cp, cat, name[: -len(", First>")])
        elif name.endswith(", Last>"):
            assert pending is not None, "Last 无对应 First"
            start, cat0, base = pending
            for c in range(start, cp + 1):
                out[c] = (base, cat0)
            pending = None
        else:
            out[cp] = (name, cat)
    return out


_STOPWORDS = {"with", "and", "or", "for", "the", "of", "a", "an"}


def name_keywords(uname: str) -> list[str]:
    """Unicode 名 → 小写词元（'BLACK UP-POINTING TRIANGLE' → black,up,pointing,triangle）。"""
    words = re.split(r"[^a-z0-9]+", uname.lower())
    return [w for w in words if w and w not in _STOPWORDS]


def kana_row(cp: int, uname: str) -> tuple[str, list[str]]:
    """平假名条目：中文名沿用旧内置样例 `平假名a` 的形式（小写假名加「小」），
    关键字 = 罗马字（含小写的 x/l 输入法写法）+ 全名 + 块名。"""
    tail = uname.split("HIRAGANA LETTER ", 1)[-1].lower()
    small = tail.startswith("small ")
    letter = tail[len("small "):] if small else tail
    romaji = KANA_ROMAJI.get(cp, letter)
    keys = [romaji, "hiragana", "pingjia", "riyu"] + name_keywords(uname)
    if small:
        keys += [f"x{romaji}", f"l{romaji}"]
    keys += KANA_EXTRA.get(cp, [])
    return f"平假名{'小' if small else ''}{romaji}", keys


def rows_for_block(block: tuple[int, int, int, str, int], chars: dict[int, tuple[str, str]]):
    """区块内逐码位产出 (text, name, keywords, block_id, emoji)。"""
    bid, start, end, _name, _common = block
    for cp in range(start, end + 1):
        if cp not in chars:
            continue  # 未分配码位
        uname, cat = chars[cp]
        if cat in EXCLUDE_CATEGORIES:
            continue
        if cp in MANUAL:
            name, kw = MANUAL[cp]
            keys = kw.split(",") + name_keywords(uname)
        elif bid == 6:
            name, keys = kana_row(cp, uname)
        else:
            name, keys = uname, name_keywords(uname)
        # emoji 标记 = **非 BMP**，不是「区块 4」：扩块后 1F000 以上有 10 个区块，
        # 逐个列 id 只会漏。判定口径与 Android `SymbolCatalog.isEmoji()`（含代理对）
        # 同源，生成期下面的断言与 rare_coverage.rs 各锁一遍。
        yield chr(cp), name, list(dict.fromkeys(keys)), bid, 1 if cp > 0xFFFF else 0


def write_atomic(path: str, lines: list[str]) -> None:
    tmp = f"{path}.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    os.replace(tmp, path)


def main() -> None:
    raw = fetch(UNICODE_DATA_URL).encode("utf-8")
    got = hashlib.sha256(raw).hexdigest()
    if got != UNICODE_DATA_SHA256:
        raise RuntimeError(
            f"UnicodeData.txt 摘要与 pin 不符：期望 {UNICODE_DATA_SHA256}，实得 {got}。"
            f"上游内容变了，请确认后更新 scripts/gen_symbols.py 的 pin 并重跑门禁。"
        )
    chars = parse_unicode_data(raw.decode("utf-8"))

    per_block = {b[0]: list(rows_for_block(b, chars)) for b in BLOCKS}

    # 自检（生成期门禁，与 crates/opi-tools/tests/rare_coverage.rs 的产物门禁同义）：
    # 1) 每个 common=1 区块 ≥ 32 条（8 列面板 4 行）：防「声明了区块却几乎没内容」——
    #    改动前 CJK 扩展 A 就是这么白声明的。32 是下限而非目标，实测最小块 60 条。
    # 2) emoji=1 ⟺ 非 BMP：Android SymbolCatalog.isEmoji() 用代理对判定，两者必须一致，
    #    否则「表情」页与数据里的 emoji 标记会各说各话。
    for bid, _s, _e, name, common in BLOCKS:
        rows = per_block[bid]
        if common and len(rows) < 32:
            raise SystemExit(f"FATAL: 常用区块 {bid}「{name}」只有 {len(rows)} 条（下限 32）")
        for text, _n, keys, _b, emoji in rows:
            if emoji != (ord(text) > 0xFFFF):
                raise SystemExit(f"FATAL: U+{ord(text):04X} emoji={emoji} 与非 BMP 判定不符")
            # 关键字必须是小写 ASCII 字母数字：SymbolEngine::search 把查询转小写后
            # 按**字节前缀**比较，含大写/空格/非 ASCII 的关键字永远搜不到。
            # （这条抓到过真错：手写表里的 fangkUang / yinFu / DIE FACE-1 的 "1"。）
            for k in keys:
                if not re.fullmatch(r"[a-z0-9]+", k):
                    raise SystemExit(f"FATAL: U+{ord(text):04X} 关键字 {k!r} 非小写 ASCII 字母数字")
            assert "\t" not in text and " " not in text, f"U+{ord(text):04X} text 含空白"

    # 旧表 8 条必须原样活着（丢了只会让 opi-ffi 的断言在远处红，这里当场拦下）
    have = {t: set(ks) for rows in per_block.values() for t, _n, ks, _b, _e in rows}
    for cp, need in LEGACY.items():
        ch = chr(cp)
        if ch not in have:
            raise SystemExit(f"FATAL: 旧内置条目 U+{cp:04X} {ch} 在新数据里消失了")
        lost = [k for k in need if k not in have[ch]]
        if lost:
            raise SystemExit(f"FATAL: U+{cp:04X} {ch} 丢了旧关键字 {lost}（opi-ffi 断言依赖）")

    blocks_tsv = [
        f"{bid}\t{start:04X}\t{end:04X}\t{name}\t{common}"
        for bid, start, end, name, common in BLOCKS
    ]
    symbols_tsv = [
        f"{text}\t{name}\t{','.join(keys)}\t{bid}\t{emoji}"
        for bid in per_block
        for text, name, keys, _b, emoji in per_block[bid]
    ]
    write_atomic("data/raw/symbol_blocks.tsv", blocks_tsv)
    write_atomic("data/raw/symbols.tsv", symbols_tsv)

    print(f"UCD {UNIHAN_VERSION} UnicodeData.txt sha256 校验通过")
    total = 0
    for bid, start, end, name, common in BLOCKS:
        n = len(per_block[bid])
        total += n
        hand = sum(1 for cp in range(start, end + 1) if cp in MANUAL)
        derived = n - hand if bid == 6 else 0  # 平假名：罗马字由字符名推导，非手写
        print(f"  区块 {bid} {name:10s} U+{start:04X}-{end:04X} {n:4d} 条"
              f"（手写 {hand}，罗马字推导 {derived}，名字分词兜底 {n - hand - derived}）"
              f"common={common}")
    print(f"合计 {total} 条 → data/raw/symbols.tsv")


if __name__ == "__main__":
    main()
