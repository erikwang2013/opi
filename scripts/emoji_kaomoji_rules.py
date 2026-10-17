# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""`gen_emoji_kaomoji.py` 的选取/分类规则表（纯数据，无逻辑）。

拆出去的原因同 `symbol_keywords.py`：生成器贴着仓库的 500 行门禁。
provenance、筛选理由与源 SHA-256 pin 都在 `gen_emoji_kaomoji.py` 头部；
改这里的表之前先读那份说明。
"""

# OpenMoji group → 面板分类。flags / component / extras-* 不列 = 整组不进。
EMOJI_GROUP_CN = {
    "smileys-emotion": "笑脸",
    "people-body": "人物",
    "animals-nature": "动物",  # plant-* 子类改判「植物」
    "food-drink": "食物",
    "travel-places": "旅行",
    "activities": "活动",
    "objects": "物品",
    "symbols": "符号",
}
EMOJI_ORDER = ["笑脸", "人物", "动物", "植物", "食物", "旅行", "活动", "物品", "符号"]

# 每个 OpenMoji 子类保留的条数（码位升序取前 N）。0 = 整类不要。
# 取舍口径：常用物件/面孔/手势/动植物/食物优先；生僻子类（纹章、宗教场所、
# 地图、手工、幻想人物、家庭组合）整类不进。数字按 2026-10-18 的 OpenMoji
# 全量核对过，改上游后未知子类会被下面的断言拦下。
EMOJI_SUBGROUP_CAP = {
    # 笑脸
    "face-smiling": 12, "face-affection": 9, "face-tongue": 6, "face-hand": 7,
    "face-neutral-skeptical": 10, "face-concerned": 12, "face-negative": 8,
    "face-costume": 4, "cat-face": 6, "monkey-face": 3, "heart": 10,
    "emotion": 10, "face-unwell": 8, "face-sleepy": 5, "face-hat": 2,
    "face-glasses": 3,
    # 人物
    "body-parts": 5, "person": 7, "person-gesture": 6, "person-role": 5,
    "person-fantasy": 0, "person-activity": 6, "person-sport": 5,
    "person-symbol": 3, "person-resting": 2, "hand-fingers-open": 6,
    "hand-fingers-partial": 5, "hand-fingers-closed": 5,
    "hand-single-finger": 5, "hands": 4, "hand-prop": 0, "family": 0,
    # 动物 / 植物
    "animal-mammal": 21, "animal-bird": 10, "animal-marine": 8,
    "animal-bug": 6, "animal-reptile": 6, "animal-amphibian": 1,
    "plant-flower": 10, "plant-other": 12,
    # 食物
    "food-prepared": 12, "drink": 10, "food-fruit": 9, "food-vegetable": 9,
    "food-asian": 7, "food-sweet": 7, "dishware": 3,
    # 旅行
    "transport-ground": 18, "sky-weather": 12, "time": 8,
    "place-building": 9, "transport-air": 8, "place-geographic": 5,
    "transport-water": 5, "place-other": 0, "place-map": 0,
    "place-religious": 0, "hotel": 1,
    # 活动
    "sport": 10, "game": 9, "event": 8, "award-medal": 3, "arts-crafts": 0,
    # 物品
    "clothing": 10, "tool": 6, "household": 5, "office": 5, "book-paper": 5,
    "light-video": 6, "computer": 4, "musical-instrument": 4, "mail": 2,
    "money": 3, "sound": 3, "music": 3, "other-object": 0, "writing": 3,
    "science": 2, "medical": 3, "phone": 3, "lock": 3,
    # 符号
    "alphanum": 7, "geometric": 5, "av-symbol": 6, "other-symbol": 5,
    "arrow": 6, "transport-sign": 3, "warning": 4, "religion": 3,
    "zodiac": 12, "keycap": 5, "punctuation": 3, "math": 3, "gender": 2,
    "currency": 2,
}

# 不管上限、**一定保留**的常用表情（按 OpenMoji annotation 精确匹配）。
# 这是「常用度」里唯一敢写死的部分：上限按码位序取「较早 = 较普及」只是代理，
# 代理会漏掉 🚗/📱 这种码位靠后但人尽皆知的条目，所以这些条目单独钉住。
# 生成期断言：名字必须都在上游出现（上游改名会在这里大声失败）。
EMOJI_MUST_HAVE = {
    # 笑脸
    "grinning face", "face with tears of joy", "smiling face with heart-eyes",
    "smiling face with hearts", "winking face", "face with hand over mouth",
    "thinking face", "smiling face with sunglasses", "loudly crying face",
    "crying face", "enraged face", "face screaming in fear", "sleeping face",
    "smiling face with open hands", "partying face", "smiling face with halo",
    "face blowing a kiss", "grinning face with sweat", "upside-down face",
    "face savoring food", "star-struck", "flushed face", "pleading face",
    "face with steam from nose", "face with medical mask", "smiling face",
    # 人物 / 手势
    "waving hand", "thumbs up", "thumbs down", "ok hand", "victory hand",
    "folded hands", "clapping hands", "raising hands", "flexed biceps",
    "handshake", "eyes", "people hugging",
    # 动物 / 植物
    "dog face", "cat face", "mouse face", "hamster", "rabbit face", "fox",
    "bear", "panda", "koala", "tiger face", "lion", "cow face", "pig face",
    "frog", "monkey face", "chicken", "penguin", "unicorn", "giraffe",
    "elephant", "rose", "sunflower", "cherry blossom", "tulip", "cactus",
    "evergreen tree", "seedling", "four leaf clover", "maple leaf",
    # 食物
    "red apple", "tangerine", "lemon", "banana", "watermelon", "grapes",
    "strawberry", "cherries", "peach", "pineapple", "tomato", "avocado",
    "bread", "cheese wedge", "poultry leg", "hamburger", "french fries",
    "pizza", "taco", "steaming bowl", "sushi", "shortcake", "birthday cake",
    "chocolate bar", "popcorn", "hot beverage", "beer mug",
    # 旅行
    "automobile", "taxi", "bus", "bicycle", "airplane", "rocket",
    "helicopter", "ship", "house", "office building", "hospital", "school",
    "church", "globe showing europe-africa", "crescent moon", "sun", "star",
    "rainbow", "umbrella with rain drops", "high voltage", "fire",
    "snowflake",
    # 活动
    "party popper", "confetti ball", "wrapped gift", "balloon", "soccer ball",
    "basketball", "tennis", "video game", "bullseye", "artist palette",
    "microphone", "headphone", "trophy", "1st place medal",
    # 物品
    "mobile phone", "laptop", "keyboard", "desktop computer", "camera",
    "light bulb", "key", "locked", "books", "pencil", "memo", "calendar",
    "alarm clock", "money bag", "credit card", "glasses", "t-shirt",
    "jeans", "running shoe", "crown", "ring",
    # 符号
    "red heart", "broken heart", "hundred points", "check mark button",
    "cross mark", "red exclamation mark", "red question mark", "right arrow",
    "left arrow", "up arrow", "down arrow", "copyright", "registered",
    "trade mark",
}

# 颜文字每类上限（源优先序：lmeee → sougou → A → custom → Temreg）。
KAOMOJI_CAP = {
    "开心": 70, "难过": 45, "生气": 35, "惊讶": 55,
    "害羞": 45, "动作": 75, "其他": 65,
}
KAOMOJI_ORDER = ["开心", "难过", "生气", "惊讶", "害羞", "动作", "其他"]

# 中文描述关键词（按此顺序先命中先分类）。顺序即优先级：同样是「哭」，
# 「气哭了」归生气比归难过更贴用户预期。
CN_RULES = [
    ("生气", "生气 怒 火大 恼怒 恨 讨厌 烦 暴躁 揍人 打人 打你 打死 抽你 抽脸 "
             "踢人 砸 摔东西 掀桌 翻桌 炸弹 爆炸 开枪 射击 杀人 砍人 刀子 枪支 "
             "拳头 鄙视 凸 中指 哼 切 不服 挑衅 挑战 骂人 滚开 滚蛋 闭嘴 威胁 凶狠 "
             "邪恶 腹黑 黑化 angry mad"),
    ("难过", "哭 泪 难过 伤心 委屈 沮丧 悲 忧伤 心疼 失落 绝望 心碎 可怜 "
             "呜呜 555 唉 哎 叹气 无奈 累 疲惫 思念 孤独 寂寞 愁 烦恼 郁闷 "
             "抑郁 丧 对不起 抱歉 道歉 安慰 摸摸 哭死 难受 痛苦 崩溃 sad cry"),
    ("害羞", "害羞 羞涩 羞答答 脸红 亲 啵 抱抱 拥抱 抱 爱 恋 心动 偷看 偷瞄 "
             "捂脸 遮脸 不好意思 腼腆 娇羞 喜欢你 爱你 么么 贴贴 shy kiss mua"),
    ("惊讶", "惊 吓 震 汗 呆 愣 瞪 晕 无语 疑 惑 不懂 问号 什么 咦 哇 "
             "卧槽 我靠 天哪 妈呀 竟然 居然 意外 突然 懵 尴尬 汗颜 黑线 "
             "流汗 紧张 害怕 恐惧 瑟瑟 发抖 慌张 慌乱 石化 裂开 吃惊 "
             "震惊 受惊 惊吓 可怕 恐怖 wow what omg surprise"),
    ("开心", "开心 高兴 笑 好耶 喜欢 幸福 满意 真好 棒 赞 得意 陶醉 庆祝 "
             "鼓掌 耶 乐 哈哈 呵呵 嘻嘻 嘿嘿 好吃 顶 支持 同意 赞成 满足 "
             "兴奋 愉快 快乐 开森 爽 美 萌 可爱 牛 强 给力 太好了 好棒 "
             "哦耶 噢耶 啦啦 嗷 噗 笑眯眯 福 美好 激动 期待 憧憬 爱心 "
             "振奋 欢呼 万岁 获胜 赢了 胜利 成功 大涨 自恋 花痴 舒服 享受 "
             "骄傲 自信 酷 帅 good great nice yes yeah bingo 答对 不错 可以 "
             "真棒 赞美 恭喜 祝贺 干杯 666 欢迎 high happy 好"),
    ("动作", "挥手 招手 拜 鞠躬 握 加油 冲 跑 跳 唱 舞 睡 吃 喝 抽烟 攻击 "
             "射击 魔法 变身 点赞 点踩 摇头 点头 拍 走 坐 递 举 闪 转 溜 逃 "
             "上班 下班 工作 学习 看书 听歌 运动 bye 拜拜 晚安 早安 打人 "
             "揍人 拖走 拉走 抱走 围观 看戏 吃瓜 递茶 拍桌 敲 撞 翻 滚 爬 "
             "跪 蹲 躺 站 飞 游 骑 开车 打伞 下雨 打电话 玩 游戏 写 画 弹 "
             "吹 喝酒 吃饭 喂 摸头 拍拍 握手 敬礼 膜拜 拜佛 祈祷 许愿 比心 "
             "剪刀手 冲刺 发功 中枪 倒地 逃跑 摇摆 转圈 跳舞 唱歌 弹琴 "
             "睡觉 打呼 打哈欠 起床 刷牙 洗澡 出门 回家 上网 喝茶 喝咖啡 "
             "卡啦ok 卡拉ok 滚来滚去 指"),
]
# 拼音描述（custom_phrase / Temreg 只有拼音）。**按整音节短语**匹配：多音节词
# 必须连续出现。早先的实现把短语按空格拆成单音节，`yi yu` 里的 `yi` 会命中
# `de yi`（得意被判成难过）、`mo mo` 里的 `mo` 会命中 `mo fa`（魔法被判成难过）。
# 所以这里只收语义明确的短语；有歧义的单音节（ai/bei/yi/yu/da/fan/chou…）宁可不收，
# 让它落到「其他」—— 错分比不分更糟。
PY_RULES = [
    ("生气", (
        "sheng qi", "nu", "huo da", "nao", "hen", "tao yan", "fan zao",
        "fan nao", "fan ren", "ma fan", "bao zao", "da ren", "da si",
        "chou da", "chou ren", "ti ren", "za", "shuai dong xi", "xian zhuo",
        "fan zhuo", "zha dan", "bao zha", "kai qiang", "she ji", "sha ren",
        "sha si", "kan ren", "dao zi", "qiang sha", "quan tou", "bi shi",
        "zhong zhi", "heng", "qie", "bu fu", "tiao xin", "tiao zhan",
        "ma ren", "gun", "gun kai", "bi zui", "wei xie", "xiong hen",
        "xie e", "fu hei", "hei hua", "leng xiao", "qu si", "bu man",
        "pai zhuo", "cao", "angry", "mad",
    )),
    ("难过", (
        "ku", "ku qi", "lei", "lei ben", "nan guo", "shang xin", "wei qu",
        "ju sang", "bei shang", "you shang", "xin teng", "shi luo",
        "jue wang", "xin sui", "ke lian", "wu wu", "tan qi", "wu nai",
        "pi bei", "si nian", "gu du", "ji mo", "fan nao", "yu men",
        "yi yu", "sang", "bu kai xin", "nan shou", "dui bu qi", "dao qian",
        "an wei", "mo mo", "ming ku", "can le", "kun", "kun le", "wo cuo le",
        "sad", "cry",
    )),
    ("害羞", (
        "hai xiu", "xiu", "hong lian", "lian hong", "qin", "qin qin",
        "qin wen", "bo bo", "bao bao", "yong bao", "ai lian", "ai ni",
        "xi huan ni", "xin dong", "tou kan", "tou miao", "wu lian",
        "zhe lian", "yan mian", "bu hao yi si", "mian tian", "jiao xiu",
        "shy", "kiss", "mua", "me me", "tie tie", "fei wen", "wen bie",
    )),
    ("惊讶", (
        "jing", "jing xia", "chi jing", "zhen han", "dai", "leng",
        "leng zhu", "deng", "yun", "yun le", "wu yu", "yi huo", "huai yi",
        "bu dong", "wen hao", "shen me", "shi ma", "wa", "wo cao",
        "wo kao", "tian na", "ma ya", "jing ran", "ju ran", "yi wai",
        "tu ran", "fa meng", "meng bi", "gan ga", "han", "han si",
        "liu han", "hei xian", "jin zhang", "ji zhang", "hai pa",
        "kong ju", "se se", "fa dou", "huang zhang", "huang luan",
        "shi hua", "lie kai", "shou jing", "ke pa", "kong bu", "zao gao",
        "wow", "what", "omg", "surprise",
    )),
    ("开心", (
        "kai xin", "gao xing", "xiao", "da xiao", "wei xiao", "hao",
        "hao a", "hao de", "hao ye", "hao zhu yi", "tai hao le",
        "zhen hao", "hao bang", "zan", "bang", "xi huan", "xing fu",
        "man yi", "man zu", "de yi", "de se", "qing zhu", "gu zhang",
        "pai shou", "ye", "ha", "ha ha", "he he", "hei hei", "ho ho",
        "hi", "hia hia", "la la la", "xi xi", "chi hao", "ding",
        "zhi chi", "tong yi", "shi a", "niu", "qiang", "gei li",
        "shuang", "mei li", "mei wei", "mei hao", "meng meng", "chao meng",
        "ke ai", "ji dong", "qi dai", "sheng li", "ying le", "cheng gong",
        "gan bei", "huan hu", "wan sui", "huo sheng", "gao chao",
        "xing fen", "yu kuai", "kuai le", "shu fu", "xiang shou",
        "zi lian", "hua chi", "xing xing yan", "shuai ge", "shuai qi",
        "bao xiao", "sa hua", "wu di", "tao zui", "xin wei", "ni hao",
        "good", "great", "nice", "yes", "yeah", "bingo", "666",
    )),
    ("动作", (
        "hui shou", "zhao shou", "bai", "bai bai", "ju gong", "wo shou",
        "jia you", "chong", "pao", "pao bu", "tiao", "tiao wu",
        "tiao yue", "chang ge", "wu dao", "shui jiao", "shui zhao",
        "chi fan", "chi dong xi", "he shui", "he jiu", "he cha",
        "he ka fei", "chou yan", "gong ji", "she ji", "mo fa",
        "feng mo fa", "bian shen", "dian zan", "yao tou", "dian tou",
        "mo tou", "pai pai", "jing li", "mo bai", "bai fo", "qi dao",
        "xu yuan", "bi xin", "jian dao shou", "chong ci", "fa gong",
        "zhong qiang", "dao di", "tao pao", "liu zou", "yao bai",
        "zhuan quan", "zhuan shen", "tan qin", "da hu", "qi chuang",
        "shua ya", "xi zao", "chu men", "hui jia", "shang wang",
        "da dian hua", "da san", "xia yu", "kai che", "wan you xi",
        "kan shu", "ting ge", "yun dong", "kan xi", "wei guan", "chi gua",
        "di cha", "pai zhuo", "qiao", "zhuang", "fan", "pa", "gui",
        "dun", "tang", "zhan li", "zhan zhe", "fei", "you yong",
        "fang yu",
        "qi ma", "qi che", "shang ban", "xia ban", "gong zuo", "xue xi",
        "wan an", "zao an", "zai jian", "bye", "bao zou", "tuo zou",
        "la zou", "zou", "zou zhe", "duo", "nie", "chuo", "zhi",
        "ju shou", "ju qi", "tan shou", "tou xiang", "die dao",
        "mian bi", "fu qiang", "gui lian", "tu she tou", "hui shou pa",
        "yin shen", "nian zhou", "zhui sha", "lie dui", "ka la ok",
        "wo hui lai la", "wo lai le", "di tou", "gao zhuang",
    )),
]
