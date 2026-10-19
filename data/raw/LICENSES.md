# 词库数据来源与许可证

> 本文件只记录**数据**的许可证：仓库源码（`crates/`、`android/`、`desktop/`、`scripts/` 等）为 **MIT**，见 [`../../LICENSE`](../../LICENSE)；
> `data/raw/*.tsv` 及由其编译出的 `.opid` **不适用 MIT**，各自按下表（`fallback.tsv` 例外：本项目自建，故为 MIT）。
> 视觉资产（小欧形象与贴纸、应用图标、文档与 README 展示图等）**不适用 MIT，也不是开源内容**：专有，商业使用需书面授权，见 [`../../ASSETS-LICENSE.md`](../../ASSETS-LICENSE.md)。

| 文件 | 来源 | 许可证 | 说明 |
|---|---|---|---|
| fallback.tsv | OPI 项目自建 | MIT | 内置回退词库，由 opi-tools 编译为 data/generated/fallback.opid |
| luna_pinyin.dict.yaml（M2 验证用） | https://github.com/rime/rime-luna-pinyin | **LGPL-3.0** | 官方拼音词库，889KB ~70771 行；由 scripts/gen_luna_dict.py 重排后编译为 luna.opid（产物不直接入库）：单字第三列是读音概率（非词频），旧版据此以 GB2312 码序为常用度替身、次读音排目标读音组尾、0% 读音剔除；现改以 Unihan kHanyuPinlu 逐读音词次排序，且**有词次证据的读音一律保留**（上游 0% 只是份额为 0，不等于读音不存在；0% 过滤器只对无证据读音生效），词组仍排在单字之后，详见脚本头注释。**2026-09-27：luna.opid 的**词组段**改由 pinyin_simp.dict.yaml（Apache-2.0，下一行）提供，本表只贡献单字层；上游自带的繁体成语段已整段退役（见下节）** |
| pinyin_simp.dict.yaml（简体词组段，2026-09-27 加） | https://github.com/rime/rime-pinyin-simp | **Apache-2.0** | 简体常用词组词源（用户反馈「常用词组无法拼音或模糊拼音显示」的修复）。48078 条 2–4 字词组，**原生简体**（无需繁→简，无字形歧义）、拼音连写、带真实词频（我们 322329 / 什么 295403）；来源是 AOSP PinyinIME（同为 Apache-2.0）。由 scripts/gen_luna_dict.py 并入 luna.opid；2026-10-11 外部词库并入前曾是产物的**唯一**词组来源，现与人工段/批量段构成三段序（见下节「外部词库并入（2026-10-11）」）—— 本节的理由与实测代价仍逐条有效 |
| trad_hanzi.tsv（单字） | Unicode Unihan（kMandarin 读音 + kHanyuPinlu 词次 + kTraditionalVariant/kSimplifiedVariant 变体） | Unicode License（宽松，可再分发，保留版权声明） | GB2312 全量 6763 + terra 单字 + 其余 Unihan 单字，由 scripts/gen_trad_dict.py 生成（人工白名单 COMMON_TRAD 已退役，见脚本头注释） |
| trad_phrases.tsv（词组） | https://github.com/rime/rime-terra-pinyin（terra_pinyin.dict.yaml） | **LGPL-3.0** | 常用繁体词组 + 人工常用词组（SUPPLEMENT_PHRASES，terra 缺 臺灣/電話/謝謝 等），由 scripts/gen_trad_dict.py 生成 |
| trad_modern.tsv（现代繁体词组层，2026-10-18 加） | 简体词组：rime-pinyin-simp（同一 commit + SHA-256 pin，见 `gen_luna_dict.py`）；人工段：`extra_manual.tsv`；转换：OpenCC `s2t.json`（TSPhrases/TSCharacters） | **Apache-2.0**（pinyin_simp / OpenCC） + **MIT**（人工段） | 现代汉语常用词（2–4 字，另含人工表）的词组级简→繁转换，剔除 `trad_phrases.tsv` 已有 (词形,拼音) 后约 4.2 万条；**固定词频 50000**（低于 terra 词组段），由 `scripts/gen_trad_modern.py` 生成（需 OpenCC CLI，产物入库）。选 pinyin_simp 而不用 luna.opid 的高频词，是为了不把 rime-ice（GPL-3.0-only）引进 trad.opid；trad.opid 仍按 LGPL-3.0 分发 |
| kangxi_trad.tsv / kangxi_luna.tsv（康熙差集，2026-10-18 加） | 同 `kangxi.tsv`（shunshi-kangxi-core@0.1.1，MIT） | **MIT**（保留 Copyright (c) Shunshi.AI） | 从 `kangxi.tsv` 中只取目标默认库还没有的 (字,读音)：trad 622 条、luna 1279 条（**条数以生成器每次打印的为准**）；词频常数 1e9（排序取舍，非上游数据）。由 `scripts/gen_kangxi_extra.py` 生成；`trad.opid` 用 `cat` 直接纳入，`luna.opid` 由 `scripts/merge_luna_kangxi.py` 合并。康熙**只有单字条目**，没有多字词（上游 `chars.json` 实测） |
| stroke.dict.yaml（笔画序，2026-10-09 加） | https://github.com/rime/rime-stroke | **LGPL-3.0** | 五笔画码表（h 横 / s 竖 / p 撇 / n 捺 / z 折；主码表源自 CNS11643 全字库，超集扩充至扩展 J），由 scripts/gen_stroke_dict.py **钉 commit** 拉取后**原样**编译为 `android/app/src/main/assets/stroke.opid`（解析器按 rime 通用的「字\<TAB\>码」形状读，产出的键就是笔画串 —— 见脚本头注释）。移动端「笔画键盘」用它打字、「生僻字键盘」用它按笔画查字。**Linux 端不发它**（桌面三轨本轮没有笔画入口） |
| rare_index.tsv（部首/笔画索引，2026-10-09 加） | Unicode Unihan（Unihan_IRGSources.txt 的 kRSUnicode + kTotalStrokes） | Unicode License（同 Unihan 行） | 生僻字键盘的「选部首 → 选笔画数 → 列字」索引（`字\<TAB\>部首号(1..214)\<TAB\>总笔画数`；**条数以生成器每次打印的为准，别写死数字**），由 scripts/gen_rare_index.py 生成；Unihan 下载与 SHA-256 pin 复用 scripts/hanzi_freq.py 的 unihan_member。随 Android 资产分发 |
| polyphone.tsv（多音字补读音，2026-10-15 加） | Unicode Unihan（kXHC1983 / kTGHZ2013 / kMandarin 现代词典读音 + kHanyuPinlu 词次 + Unihan_Variants 繁简孪生）+ **本仓词库取键转储**（词频列） | Unicode License（同 Unihan 行） | `字\<TAB\>读音\<TAB\>词频`：**只收词库真缺的读音**（生成器拿 `opi-tools dump-keys` 的转储做差集）—— 上游 luna 读音表本身缺这些（`么` 没有 `mo`、`绿` 没有 `lu`、`咯` 没有 `ka`），而这类缺口有限、可枚举、逐行能被 Unihan 字段原文复核。**`kHanyuPinyin`（全量历史读音表）故意不收**：按它会补出 `百(mò)`、`貌(mò)` 这类只有古读、现代词典一字不载的读音。由 scripts/gen_polyphone.py 生成（Unihan 下载与 SHA-256 pin 复用 scripts/hanzi_freq.py）；引擎侧作**纠偏层** include_str! 进二进制（`crates/engine-core/src/polyphone.rs`，先例 trad_chars.rs）|
| symbol_blocks.tsv（符号区块） | Unicode UCD（UnicodeData.txt 的区块范围与字符名；2026-09-28 起新增的 emoji 区块范围取自 UTS#51 §2.1 区块表，仍是同一份 UCD 字符名） | Unicode License（同 Unihan 行） | 面板区块的 id/范围/名（**块数以 `data/raw/symbol_blocks.tsv` 的行数为准，别写死数字**；**id 5 是有意空号**），由 scripts/gen_symbols.py 生成；**符号面板的搜索关键字不来自上游**，357 条为 OPI 项目人工撰写（MIT，2026-09-28 起该手写表在 scripts/symbol_keywords.py —— 生成器撞 500 行门禁拆出去的，内容未改），86 条由区块名罗马字推导，其余 2880 条按 Unicode 名分词兜底 |
| symbols.tsv（符号条目；**条数与 emoji 条数以生成器每次打印的为准，别写死数字**） | 同上（UnicodeData.txt 逐码位字符名 + **emoji-data.txt 的 `Emoji` 属性**判 emoji 列，2026-09-28 起） | Unicode License（同上，不新增来源、不新增许可证） | 同生成器产出。**emoji 列 = UTS#51 `Emoji` 属性**（含 BMP 区的 ☺♥⌚⭐），不再是「非 BMP」代理指标 —— 判据与复算命令见 scripts/gen_symbols.py 头与 crates/engine-core/tests/symbol_coverage.rs 的 `UTS51_EMOJI` 快照。**消费侧是编译期读取**：engine-core/src/symbols.rs `builtin()` 用 `include_str!("../../../data/raw/symbols.tsv")` 嵌入，故路径固定在 data/raw/ 而非 data/generated/（后者只放 .opid 二进制）；改目录必须两处同改，否则编不过 |
| en_words.tsv（英文联想，10000 条，2026-09-28 加） | https://github.com/orgtre/google-books-ngram-frequency（Google Books Ngrams v3 英文 1-gram 的清洗产物） | **CC-BY 3.0** | 英文模式单词联想的数据源 —— spec §1「V1 仅做单词联想」承诺过、仓库此前没兑现（`candidates.rs` 对非拼音模式 `return Vec::new()`）。由 scripts/gen_en_dict.py 生成，`word\tpinyin\tfreq` 三列与 trad_hanzi.tsv 同形（第二列是 ASCII 小写查询键，多数与词形相同）。**候选逻辑 2026-10-18 已接线**（`engine-core/src/english_words.rs`，`include_str!` 嵌入；不是牛津原版 —— OUP 数据无授权）。CC-BY 要求署名 + 保留许可声明：**本表就是署名载体**，分发含本数据的产品时须一并保留 |
| brand_terms.tsv / new_words.tsv / reading_overrides.tsv（人工表，2026-10-11 加） | OPI 项目人工撰写（生成器只做机械校验：音节合法性按 `pinyin.rs` 的 SYLLABLES 全切分、长度/字面/繁形过滤） | MIT | 品牌白名单（609 条）、网络用语/新词（391 条，含 2020–2023 时期词汇）与词级注音覆盖表（422 条，拼音人工写死）。⚠️ 覆盖表另有对上游真值的自动复核：词在 base/ext 出现时其读音必须是上游读音之一（实测揪出 8 条笔误/选读错误，见生成器 stderr 与本节下「外部词库并入」）。由 `scripts/gen_extra_phrases.py` 合并进 extra_manual.tsv |
| surnames.tsv / slang.tsv（人工表，2026-10-18 加） | 百家姓：传统《百家姓》字表（公有领域）+ 项目人工标注姓氏读音；网络流行语：项目自撰（参考 2015–2026 公开网络用法，未复制第三方词表） | MIT | 姓氏 508 条（单姓 446 + 复姓 62；频次档 12000/9000）、流行语 597 条（档 6000/4000）。拼音**逐条人工写**（多音字取姓氏读音/实际读法：单 shan、解 xie、仇 qiu、区 ou、查 zha、朴 piao、乐 yue、覃 qin、尉迟 yuchi、万俟 moqi、澹台 tantai、长孙 zhangsun 等），生成器按 `pinyin.rs` 的 SYLLABLES 全切分校验；ASCII 字面键（yyds、栓Q、City不City、i人、打call、嗑CP）是引擎精确字面匹配例外。两表接在 extra_phrases.tsv **批量段尾**（不进 extra_manual 前置段，避免压过同键常用词），放置理由与校验见 `scripts/manual_tables.py` 头注 |
| extra_manual.tsv / extra_phrases.tsv（生成物，2026-10-11 加） | 复合来源（逐源见「外部词库并入（2026-10-11）」节） | **合并产物按 GPL-3.0 分发**（含 GPL-3.0-only 的 rime-ice 层）；源码（生成器）仍 MIT | 词组三段序的前置段与批量段，由 `scripts/gen_extra_phrases.py` 生成、`gen_luna_dict.py` 并入 luna.opid。行数/体积**以生成器每次打印的为准**，别写死数字 |
| GPL-3.0.txt（2026-10-11 加） | https://www.gnu.org/licenses/gpl-3.0.txt （与 Debian `/usr/share/common-licenses/GPL-3` 逐字节同文，双源 sha256 核过） | GPL-3.0 全文 | 随 .deb/.rpm 分发（`scripts/nfpm.yaml` 装进 `/usr/share/doc/opi/`）—— 合并产物按 GPL-3.0 分发时的许可文本义务 |
| wubi86.tsv（2026-10-14 加） | https://github.com/rime/rime-wubi @ commit `152a0d3f3efe40cae216d1e3b338242446848d07`（wubi86.dict.yaml） | **LGPL-3.0**（按 rime 仓声明；上游链路的残留不一致见下「五笔码表」节） | 五笔 86 码表（极点码表转 rime 格式）。由 scripts/gen_wubi_dict.py **钉 commit** 拉取后把 rime 四列转成仓内三列（`词\t码\t频次`；第 4 列 stem 丢弃），编译为 `data/generated/wubi86.opid`（入库）。weight 直接作词频（最大 4.25e9 < u32::MAX；引擎侧 u64 比较，无溢出）。**条数以生成器每次打印的为准，别写死数字** |
| wubi98.tsv（2026-10-14 加） | https://github.com/yanhuacuo/98wubi-tables @ commit `6b8b6fb9d3c34e0d5e3b17211e1f1c100e7eb697`（`超集-rime格式码表.txt` 单字段 + `98五笔含词表-【单义】.txt` 词组段） | **Unlicense**（仓库 LICENSE 实测为 Unlicense 全文，blob `fdddb29aa445`；README「已无版权问题」的依据是讲**专利**的视频，不作版权依据——见下节） | 五笔 98 码表：单字取超集-rime 格式（全单字、**字覆盖为含词表的 3.6 倍**）、词组取含词表（唯一词组来源）。两份拼的理由、权重语义与自洽校验见下「五笔码表」节；编译产物 `data/generated/wubi98.opid`（入库） |
| kangxi.tsv（康熙字典词库，2026-10-16 加） | npm 包 `shunshi-kangxi-core@0.1.1`（Shunshi.AI，https://github.com/shunshi-ai/kangxi-mcp；tarball SHA-256 **钉在生成器里**，不可变包） | **MIT**（须随分发保留版权声明：Copyright (c) Shunshi.AI） | 康熙字典全字头（20794 字）的**读音**提取：`字\t读音\t词频`（词频是低位常数 4000，非上游数据），由 scripts/gen_kangxi_dict.py 生成 —— 读音规范化（去调、ü=v、lue/nue→lve/nve 归一）后过 `pinyin.rs::SYLLABLES`，多音字**全部读音**展开（多音字如 乄 = shī/mē 两条）；条数以生成器每次打印的为准，别写死数字。编译产物 `data/generated/kangxi.opid`（入库）。四端经**导入词库契约**分发（随包 TSV + 首启种子一次；见生成器头注）。**只提取读音**；部首/笔画/五行/释义/康熙原文等字段未用（留档备用） |
| emoji.tsv（表情面板浏览表，2026-10-18 重写） | https://github.com/hfg-gmuend/openmoji （OpenMoji `main` 分支；2026-10-18 取得的 `data/openmoji.json`，4565 条，sha256 `b3a07c1c41b6b500423d01fb91c5d41e9c9337a53d217af20a3539d7df8438e4`） | **CC BY-SA 4.0**（须署名；改动作品与衍生数据按相同许可分发） | 表情面板的**常用**表情子集（400–700 条；**条数以 `scripts/gen_emoji_kaomoji.py` 每次打印的为准，别写死数字**）：`text\<TAB\>name\<TAB\>中文组名`（无表头），组序 笑脸→人物→动物→植物→食物→旅行→活动→物品→符号；排除 `flags`/`component`/`extras-*` 整组、`skintone` 非空的肤色变体、含 ZWJ 的组合序列与生僻子类。`name` 取 OpenMoji `annotation` 小写。由 `scripts/gen_emoji_kaomoji.py` 生成（源 SHA-256 钉在脚本里；离线不重跑）。**本表是 OpenMoji 的改动作品（adapted from OpenMoji, © OpenMoji 项目及其贡献者），按 CC BY-SA 4.0 分发**；许可全文 https://creativecommons.org/licenses/by-sa/4.0/ |
| emoji_keywords.tsv（输入匹配表情关键字表，2026-10-18 加） | 关键字：OPI 项目人工撰写（80 条沿用 `scripts/symbol_keywords.py` 的 MANUAL 关键字，其余手写在 `scripts/gen_emoji_keywords.py` 的 EXTRA 表）；emoji 文本：取自 `emoji.tsv`（OpenMoji 常用子集，渊源同上行；MANUAL 里 10 个不在该子集的取码位字符本身） | **关键字 MIT**（项目自撰）；**emoji 文本仍按 emoji.tsv 的 CC BY-SA 4.0** | 输入即得表情的数据源（打 `weixiao` / `smile` 出 😊 / 😄）。两列 `emoji\<TAB\>关键字(逗号分隔,无空格)`，150–300 条（**条数以生成器每次打印的为准，别写死数字**），由 `scripts/gen_emoji_keywords.py` 生成（离线、读已入库数据，不联网）。消费侧 `engine-core/src/emoji_keywords.rs` 用 `include_str!` 编译期嵌入；匹配 = 缓冲（拼音/英文小写字母串）是关键字前缀，候选**追加在文本候选之后**、受 limit 约束（不参与 `rank_and_pick` 既有排序） |
| kaomoji.tsv（颜文字表，2026-10-18 加） | https://github.com/aoguai/rime_kaomoji_dict @ commit `2f10ddca83f4cd87f41672442d27943f733b488f`（`data/` 下 5 份素材：A_kaomoji_dict / custom_phrase_dict / lmeee_dict / sougou_dict / Temreg_dict） | **MIT**（Copyright (c) 2023 aoguai；分发时保留版权与许可声明） | 颜文字常用子集（200–400 条；**条数以生成器每次打印的为准，别写死数字**）：`text\<TAB\>中文分类`（无表头），分类 开心/难过/生气/惊讶/害羞/动作/其他；保留原文（不插入空格、不把特殊空格转成普通空格），剥掉行首/行尾标签与中文描述后按文本去重，按源优先序 lmeee→sougou→A→custom→Temreg 取到每类上限。由 `scripts/gen_emoji_kaomoji.py` 生成（5 份源各钉 SHA-256）；`data/raw/kaomoji.tsv` 与 `android/app/src/main/assets/kaomoji.tsv` 为同一次运行的逐字节副本。许可全文见上游仓库 LICENSE（commit 同上） |
| shared/pet/stickers/*.png、shared/pet/stickers-gif/*.gif、shared/pet/emoji-font/*（小欧贴纸/动图/映射表，2026-10-18 加，2026-10-19 扩，2026-10-20 删字体） | OPI 项目自绘：由 `shared/pet` 的 Compose 绘制（`OpiPet` 十二形态）生成，**不是第三方 emoji/贴纸资产** | **专有（非开源）**：版权归项目作者，商业使用需书面授权，个人非商业可随未修改的官方发行版原样使用（见 [`../../ASSETS-LICENSE.md`](../../ASSETS-LICENSE.md)） | 「自有」表情页使用的 12 组 280×280 透明底 PNG 与 200×200 十帧动画 GIF，以及 `map.tsv` / `shortcodes.tsv` 映射表；生成器 `scripts/gen_pet_stickers.py` / `scripts/gen_pet_emoji_table.py`（桌面渲染入口 `OpiPetStickerRenderTest`），文件名契约 `PetStickerAssets`。构建期拷进 Android assets `pet/`、`pet-gif/`、`pet-font/`（仅 TSV）与桌面 resources 同名目录；`opi-emoji.ttf` 与 iOS/鸿蒙字体副本已于 2026-10-20 删除。**不含任何 Unicode emoji 上屏**。生成器脚本与 `shared/pet` 源码仍为 MIT，产物按本行许可 |

## 英文联想词源（2026-09-28）

排序键是**该词在 Google Books 2010–2019 英文图书语料里的原始出现次数**（the 13616383631
→ 缩放后 4000000000；最低 1398875 → 410939）。上游 README **逐字声明 content 为
CC-BY 3.0**，且上游自身就是 Google Books Ngrams（同一 CC-BY 3.0）的清洗产物 ——
**许可链一致**，不存在「第三方替原作者发许可证」的问题。

**缩放不是可选项**：原始 counts 超过 `u32::MAX`，而 `compiler.rs` 的 `parse_freq` 对
`parse::<u32>()` 失败的行按 unusable **整条丢弃**（compile 只打一行 warning 就继续），
照抄数字会把 the/of/and 这些最高频词全丢掉。缩放到 `FMAX = 4_000_000_000`（中文库同一
天花板）**是刻意的**：`Engine::with_dictionaries` 的 `user_boost = max(USER_BOOST,
max_freq × 2)` 跨词典取全局值，英文库抬高 max_freq 会连带改中文侧的学习标度。

**曾评估并否掉的替代源**（均实测，详细取证见 scripts/gen_en_dict.py 头注释，勿重复调研）：

| 替代源 | 许可实况 | 否掉的原因 |
|---|---|---|
| first20hours/google-10000-english | **无 LICENSE 文件**（raw 404、API=NOASSERTION） | 上游是 Norvig count_1w.txt 去掉词频列，再上是 LDC 受限分发的 Google Trillion Word Corpus。既无授权又无词频 |
| AOSP LatinIME `en_wordlist.combined.gz` | Apache-2.0 仓库，**但 NOTICE 末行** `Includes Dictionaries © Lexiteria LLC.  Used by permission.` | 「used by permission」是给 AOSP 的许可，不是可向下再分发的授权；dictionaries/ 下只有这些词表。另：其 f= 已量化（the=222），排序分辨率差 |
| hermitdave/FrequencyWords `en_50k.txt` | LICENSE 文件写 MIT，**README 写「MIT for code. CC-by-sa-4.0 for content.」** | 数据是**共享演绎**许可（要进 APK 分发的产物冲突），且两处自相矛盾 |
| rspeer/wordfreq | Apache-2.0 是代码，数据 CC-BY-SA-4.0 | 作者 README 明确拒绝「转成 CSV 分发」（理由正是 CSV 没地方放署名），并声明数据不可与其代码分离 |
| words/subtlex-word-frequencies | 仓库自称 ISC | SUBTLEX 原始数据是 Ghent 大学研究用途数据，第三方替原作者发许可证 = 许可洗白 |

## 字频来源（spec 2026-08-15 验收偏差 #4：候选排序）

排序键的词次来自 **Unihan `kHanyuPinlu`**（Unihan.zip → Unihan_Readings.txt），
《现代汉语频率词典》逐读音词次统计，3799 字，`好 hao 6317 / 是 shi 28130 / 些 xie 3689`。
繁体字形各自成条（會/电 等），另有 `kSimplifiedVariant` 继承兜底（裏/髮/為 自身无词次时
按简体字 里/发/为 的词次）。

- 许可证：与 Unihan.zip 同源，**Unicode License**（上表 trad_hanzi.tsv 行），不新增来源。
- 获取方式：构建时由 scripts/hanzi_freq.py 下载（与 gen_trad_dict.py 原行为一致），
  **不入库**（Unihan.zip 8MB，且 UCD latest 会漂移）—— 现已 pin，见下节。
- 繁体字形取向（terra 语料字形计数）取自同一份已记录的 terra_pinyin.dict.yaml（LGPL-3.0）。
- 曾评估的替代源：schirp/rwstats（CC0-1.0，语料词频 CSV）——许可证干净但 1f.csv 的
  11371 字**零繁体字形**，繁体库无覆盖，故不采用。

> 注意：rime 社区数据许可证为 LGPL-3.0（不是 BSD/GPL 混合）。使用前确认源码树内 LICENSE 文件。

## 上游 pin（pin 日期 2026-09-26）

两份下载型上游原先指向浮动引用（UCD `latest`、GitHub `master`），产物不可复现：
2026-08-15 → 09-26 之间 terra master 已自行漂移（删「怎麽」、改「怎麼搞的」拼音
zemegaode→zenmegaode）。现钉死到本次实际取到的版本，脚本内置 SHA-256 校验，
不符即报错（宁可失败也不静默换数据）。

| 上游 | pin 到 | 内容摘要（sha256） | 位置 |
|---|---|---|---|
| Unihan.zip | Unicode **18.0.0**（Unihan_Readings.txt 头：`Date: 2026-07-31`）`https://www.unicode.org/Public/18.0.0/ucd/Unihan.zip` | `4c93ea9c1f636451729a840978f1667a53886af37ba854fdcce109721c63d43e` | scripts/hanzi_freq.py |
| terra_pinyin.dict.yaml | commit `723e51bc266cf9464530c1ddedb856aa18e3da34`（rime/rime-terra-pinyin） | `2f881a239e09a61e5993e79257655f98f81c3789dc8aeea5f33477f710345b27` | scripts/gen_trad_dict.py |
| pinyin_simp.dict.yaml | commit `0c6861ef7420ee780270ca6d993d18d4101049d0`（rime/rime-pinyin-simp） | `e341598343a0f0f2035bb1aafc34a7f3bb7887deeecb3f60796262aaa2983e6b`（1266216 字节） | scripts/gen_luna_dict.py |
| UnicodeData.txt | Unicode **18.0.0**（与 Unihan.zip 同版本目录）`https://www.unicode.org/Public/18.0.0/ucd/UnicodeData.txt` | `0736451de439ae7baf1425136617da495e09ee5afbe6e394374db7009ea08950`（2243593 字节） | scripts/gen_symbols.py |
| emoji-data.txt（2026-09-28 加） | Unicode **18.0.0**（文件头 `Date: 2026-01-30`，与上一行同版本目录、同一份 Unicode License）`https://www.unicode.org/Public/18.0.0/ucd/emoji/emoji-data.txt` | `80d00f8e616a0ef27fd6b8de3b758c06383b5d917e2977709578e68baf733bf1`（108719 字节） | scripts/gen_symbols.py（emoji 列判据） |
| 1grams_english.csv（2026-09-28 加） | commit `e20471c15a758be3362b16d07870b34df4f7ccc3`（orgtre/google-books-ngram-frequency） | `f7f63aa08f0bb2f7f654cddff3a7ced4968b27887b2c0e7cd4b4d108221e197d`（225089 字节） | scripts/gen_en_dict.py |
| rime-ice base.dict.yaml + ext.dict.yaml（2026-10-11 加） | commit `da1fbe602e38f26db846fa10120ee64c2b0324c0`（iDvel/rime-ice） | base `0418d5103d8e40c8fff4b686a873ad03e9836cd0419ecae0b7e1e2526312dba0`（16602296 字节）/ ext `8e49a0294c0139e63c815cc33a2d438cc144eb7889297d30cec899a368697c06`（11922913 字节）；与 GitHub API 的 git-blob-sha1 逐字节核过 | scripts/gen_extra_phrases.py |
| rime-essay essay.txt（2026-10-11 加） | commit `054920de4f54c9e5994276a96a4fc2a35cb51aa3`（rime/rime-essay） | `3e8512ebaa6657e35961b1f5762060717c647e3bf471d5e3b86e783b4e934ffb`（5887319 字节） | 同上 |
| chinese-xinhua idiom.json + xiehouyu.json（2026-10-11 加） | commit `fe6d6c2e8baa82187f4c96bbe042e43f96c05666`（pwxcoo/chinese-xinhua） | idiom `1d4b4f454ce1c416d6a1ab2369d6e66c0ff99e04390172eef70790499e21ce19` / xiehouyu `22e3c4aa31c7d93765eaa842fe91983e6a109b29aec25956e1efacd9f71418d9`；blob-sha1 对过 API | 同上 |
| THUOCL 五表 IT/医疗/法律/财经/成语（2026-10-11 加） | commit `a30ce79d895d01ab5132a5c74c29703ff7efb4cc`（thunlp/THUOCL） | 五份 sha256 全部写死在脚本常量（照本表体例，不在此复写）；blob-sha1 对过 API | 同上 |
| OpenCC TSCharacters.txt（2026-10-11 加，**判繁判据**、非词源） | tag `ver.1.1.9`（BYVoid/OpenCC） | `6b5a0a799bea2bb22c001f635eaa3fc2904310f0c08addbff275477a80ecf09a`（4113 条） | 同上 |
| wubi86.dict.yaml（2026-10-14 加） | commit `152a0d3f3efe40cae216d1e3b338242446848d07`（rime/rime-wubi） | `f833d86b72341fe82e069a425b6625f29ef85f1bc0f34f6fb7975fe514888b5a`（2289545 字节）；blob-sha1 `58d8f794…` | scripts/gen_wubi_dict.py |
| 98 码表两份（2026-10-14 加） | commit `6b8b6fb9d3c34e0d5e3b17211e1f1c100e7eb697`（yanhuacuo/98wubi-tables） | 超集-rime格式码表 `728acbcc1c7ae57f6bfd88f0176754b5770d3af42490bc124bac6669e1913a53`（4692084 字节）/ 98五笔含词表-【单义】 `1b5a4c22eddae08d8e0e6aa8f926f92d45d6cdaa591e5cee0e8aad29271da1cc`（1988020 字节，UTF-16LE+CRLF）；两份 blob-sha1（`52bd9c2e…` / `8500e3b9…`）对过 GitHub API | 同上 |

另：luna_pinyin.dict.yaml **仍未 pin**（上游取 rime-luna-pinyin master）。本次实测所用版本
sha256 `75bcf6eb3ff62b129882ed89cc22b2d1b80a5347aa72bcfa2ccc839bac298e7314`
（889896 字节 / 70771 行，与上表行内记载一致），用它重跑脚本产出的 luna.opid 与入库副本
逐字节相同 —— 当前 master 恰好可复现，但下次上游一动就断。补 pin 应照 terra 的做法做。

升级路径：改上表对应脚本里的版本号/commit + SHA-256 两个常量，重跑生成脚本，
再跑 `cargo test -p opi-tools --test ranking_quality` 与 `trad_coverage` 验收。
代价：**不再自动获得上游修正**（新增字、词次修订、读音订正、上游删词都要手动升级才进来）。

## 简体词组段（2026-09-27）

用户真机反馈「常用词组无法拼音或模糊拼音显示」。根因**不在模糊拼音层**（模糊已实现并
验证），在**词源**：luna 的词组段全部是繁体文言成语、拼音**刻意空格分隔**，引擎 buffer
是连续字母，整串查询永远不命中 —— 实测「你好」「中国」「谢谢」「什么」「我们」在
luna.opid 里 **0 条**（`open(...).read().count("你好".encode())` = 0）。

**先评估过「复用已有繁体表做繁→简」（trad_phrases.tsv），三条实测否定它：**

1. terra 表里**没有「你好」**（`grep -cP '^你好\t'` = 0；只有长成语 `任你好漢沒處躲`）。
   用户点名的第一个词就缺，这条路补不上。
2. 繁→简**不是多对一**：Unihan `kSimplifiedVariant` 有 50 个字给出多个简形，且首位未必
   是现在的标准写法（瀋 → `[沈, 渖]`，取首位会把「瀋陽」写成「沈阳」）。未裁决时只能
   整条丢弃（实测 42 条因此丢弃）。
3. **覆盖率不够**：terra 繁→简后对现代常用词基准（mozillazg/phrase-pinyin-data 的 47111
   条，**仅作基准、不入库、未新增许可证**）只命中 **30.6%** —— terra 是文言/成语/地名表，
   缺 一些/一代/补助/能干/自发 这类日常词。

改用 **rime-pinyin-simp 的 pinyin_simp.dict.yaml**（Apache-2.0，见上表）：原生简体（整类
字形歧义消失）、拼音连写、带真实词频。实测 48078 条词组（2–4 字，上游无更长条目），
**含 GB 级繁体专形的 0 条** —— 判据是 `字不在 GB2312 且 kSimplifiedVariant 指向 GB2312 字`
（不用「有异构简形」判：那会把 著/覆/乾/藉/瑙/阪 误判成繁体，实测误报 35 条）。该判据
**写进了生成器**（`parse_simp` 的 `trad_only` 守卫），命中即报错拒绝产出，不是静默丢弃。

代价（同一台机器、同一 debug 测试二进制、**新旧两份在同一进程内交错各 9 次取最小值**；
机器当时有多个并发编译，绝对值不可比，只看比值）：

| 产物 | 条目数 | 体积 | 装载（min，n=9） |
|---|---|---|---|
| luna.opid | 70014 → **118092**（+68.7%） | 1.71MB → **3.10MB**（+81%） | 26.3ms → **47.0ms**（×1.79） |
| trad.opid | **unchanged 96429** | **unchanged 2.58MB** | 未改 |

「除新增词组外逐条等价」的取证（新旧产物逐条 dump 后比对）：旧 70014 条**一条不丢**、
单字集合完全相同（48255 条），新增的 48078 条**恰好等于**新词源的连写拼音词组；
19919 个拼音组里按 `(-freq, word)` 排的旧条目相对顺序**0 组被打乱**。

排序不受影响：新词组整体排在**全部单字之后**（与旧词组段同一条约定），故
`ranking_quality` 的 STRICT/NEAR/REACH 三档前后完全一致（luna 首位命中 62/67、读音可达
37/37，改前改后同值）。新词组排在旧繁体成语段**之前**：两组都在单字之后，让简体常用词
先于打不出来的繁体成语，用户打 `zhong` 时第一个词组建议才是「中国」而非「中國」。

### luna 繁体成语段退役（2026-09-27，与上一节同一任务的后半）

上一节落地当天，上游自带的词组段（luna_pinyin.dict.yaml 的 2 字以上条目，21759 条）
**整段删除**，不再进产物。它存在的唯一理由写在旧注释里 ——「需换简体常用词源，另立任务」，
那个任务已随简体段完成；留着就成了半成品：同一个库里一边有繁体字形守卫，一边收着 21759
条繁体词组。

删之前先核实「用户真机看得到吗」（把 21759 条的**全部可达查询**穷举了一遍）：

- 拼音含空格 ⇒ 查询打不出空格 ⇒ 只有**首音节的前缀**能命中它。21759 条的首音节前缀并集
  = **484 个查询**。
- 这 484 个查询里，被删词组出现在**旧产物前 8 槽**（候选栏槽位数）的：**2 个** ——
  `seng` 第 7 位「僧伽」、`zhuai` 第 6 位「拽布披麻」。
- 其余位置极深：`ren` 第 425 位、`zhong` 第 596 位、`yi` 第 2885 位（旧列表分别共
  478/1073/3337 条）。即「候选尾巴」比 8 槽深两个数量级，正常使用翻不到。

结论：**清掉是对的方向，但「用户经常看到繁体字形」不成立** —— 真实可见面是 484 个查询里
的 2 个；收益主体是体积与「半成品状态」，不是可见性。另需记一笔：这 2 条里「僧伽」并**不是**
繁体字形（僧/伽 均在 GB2312），它是被整段退役顺带丢掉的正常词，属本次**已知代价**。

| 产物 | 条目数 | 体积 | 装载（min，n=9，同进程交错） |
|---|---|---|---|
| luna.opid | 118092 → **96333**（-21759） | 3099275 → **2370999** 字节（-23.5%） | 7461µs → **5641µs**（×0.76） |
| trad.opid | **unchanged 96429** | **unchanged 2.58MB** | 未改 |

「除这次删除外逐条等价」的取证（旧 118092 条 dump vs 新 96333 条 dump）：

```
old entries: 118092  new entries: 96333  delta: -21759
丢失(旧有新无): 21759   新增(新有旧无): 0
丢失集 == 独立重算的 luna 繁体词组段（21767 条 - is_cjk 挡掉的 8 条 = 21759）: True
丢失条目中拼音不带空格的: 0        # 旧段特征（空格拼音）无一例外
新增条目含非 GB2312 字的: 0
单字层: old 48255  new 48255  完全一致: True
词组层: old 69837  new 48078   新词组层 == 旧词组层剔除丢失集后的原序子序列: True
新序列是旧序列的保序子序列: True
```

重算时对不上的 8 条是 `亞當·斯密` 一类含 `·`（U+00B7）与 `𰻞𰻞麪` 的条目 —— 生成器的
`all(is_cjk(c) for c in word)` 旧版就已挡掉，从未进过产物，不是本次新丢的。

单字层的繁体字形**一个没动**（國/學/電/說 等照旧在单字候选中）—— 那是 luna 既有行为，
`ranking_quality` 的 NEAR 档就是按它设计的。排序门禁前后**完全同值**（luna 首位命中
62/67、读音可达 37/37；trad 61/67、37/37；3 passed / 0 failed），无一条门禁项是靠被删的
繁体词组满足的。

### ü 记法归一 + 词组语料读音证据（2026-09-27，lead 全量抽检牵出）

lead 用单字读音表反查全部 48078 条词组的拼音（**全量**，非抽样），48078 条里 **54 条
（0.11%）** 拆不开，分成两类，已全修。产物 2370999 → 2371159 字节（+160B）、条目
96333 → 96341。

**A 类 42 条：ü 记法不一致。** 本源（AOSP PinyinIME）把 üe 写成 `lue`/`nue`（42+8 处），
而引擎规范是 `v`：`pinyin.rs` 的 SYLLABLES 只有 `lv/lve/nv/nve`、**没有** `lue/nue`，
单字层也全用 `v`。不归一 ⇒ **同一批键分两层两种写法**：打 `lve` 出得了单字「略」、
出不了词组「策略」；打 `lue` 反过来。修法是按**音节**映射 `lue→lve`、`nue→nve`
（不整串 `str.replace`：连写串里 `lue` 还可能是 `lu|e` 跨音节）。

- **`lu`/`nu` 的 ü 情形：查了，不存在，故不做 `lu→lv`** —— 普通话里 `lu`/`nu` 与
  `lü`/`nü` 是不同音节（路 lù vs 吕 lǚ），本源也确实分开写。取证：本源 339 个 `lu` +
  29 个 `nu` 音节**没有一个是 ü 字**（拿单字层的 ü 字集合反查），阳性对照 242 个 `lv`
  全部对得上 ü 字（**检查非空转**）。
- 归一后 42 条新键 `celve`/`zhanlve`/… 全部**只对应 1 个词**，与既有键**零撞键**、
  零 `(词,拼音)` 重复 ⇒ 不需要去重。
- 逐条等价（旧 dump 的拼音按同一规则归一后与新 dump 比对）：**集合完全相同、多重集完全
  相同、39006 个拼音组内词序 0 组不一致**。注：**不能按行号逐位比** —— `.opid` 条目按
  拼音字节排序（`compiler.rs` `sort_by(pinyin)`），改拼音必然重排，那是格式要求不是漂移。

**B 类 8 条：单字层缺读音。** 根因是「有 kHanyuPinlu 词次证据就保留」这条规则的**未覆盖
残余**：kHanyuPinlu 只 3799 字，够不到的字仍只剩上游第三列可依，于是上游标 0% 的常用
读音被丢 —— `腊 la 0%/xi 100%`、`南 na 0%/nan 100%`、「柠」上游干脆把 `chu` 当主读音。
修法：**词组语料当第二条读音证据**（词组的拼音是逐字标注的，实测 48078/48078 字数==
音节数 ⇒ 「词 W 拼音 P」=「W[i] 有读音 P[i]」），与既有的 kHanyuPinlu 是**同一条原则、
换一个证据源**，且是数据自证。两种生效方式：

| 机制 | 条数 | 例 |
|---|---|---|
| 救回被 0% 丢掉的既有读音 | 4 | 腊 `la`、南 `na`、柠 `ning`、泞 `ning` |
| 新增上游表里根本没有的读音 | 4 | 行 `heng`、挝 `wo`、槟 `bin`、琢 `zuo` |

**只做加法**：`柠` 的错读音 `chu` **原样保留**（删读音是产品判断，见下）。
实测效果（单音节查询下该字的位次，旧 → 新）：`la`→腊 **缺 → 9**、`ning`→柠 **缺 → 8**、
`ning`→泞 **缺 → 10**、`heng`→行 **缺 → 10**、`wo`→挝 **缺 → 12**、`na`→南 108 → 13、
`bin`→槟 86 → 10、`zuo`→琢 299 → 7。
⚠️ **lead 报的症状描述不成立**：「打 `larou` 出不了「腊肉」」——实测 **11 个词组在改前
全部已经是第 1 位**。词组键是自包含的（`larou` 直接命中词组条目），单字读音缺不缺**不
影响整串打词组**。真正受影响的是**单音节查询**（打 `la` 出不出「腊」）。修的是这个。
「语料里有、单字表里没有的字」跳过 0 个（这类要新增单字候选 = 改排序面，不在本条范围）。

**「错读音」的规模：给了两个数，但都不是可直接用的缺陷判据。**
- 尖锐判据（可用）：语料出现过的 4001 字里，**luna 的读音与语料完全不交集的只有 4 个**
  （挝 柠 泞 腊）—— 即这些字**只能靠语料不认的读音打出来**。本次补读音后降为 **0**。
  这个判据能直接指出「该字按真实读音打不出来」，是 decision-relevant 的。
- 宽松判据（**不可用**）：GB2312 内「语料没用过 且 Unihan kMandarin 不认」的读音
  **1081 / 7936（13.6%）**。这个数**不能当缺陷率** —— `kMandarin` 是**单值**权威，任何
  合法异读都会中招（丁 zheng、乘 sheng、伯 ba、伽 ga 全是正常的）。故**没有**得出
  「有多少错读音」的可靠数字，需要产品提供更好的权威表或人工裁决。已按 lead 要求**只报
  不自行删改**。

## 收录范围（2026-09-26 放开 CJK 扩展区）

此前 `0x4E00..=0x9FFF` 的过滤写死在四处（scripts/hanzi_freq.py 两个加载器、
gen_trad_dict.py 的 parse_terra、gen_luna_dict.py 的 parse_rime），把 **CJK 扩展 A
（U+3400–4DBF）与扩展 B（U+20000–2A6DF）整段丢弃**。这不是排序问题，是数据不存在 ——
词库里根本没有这些字，翻多少页候选也翻不出来（README 缘起第一条）。
现统一口径到 scripts/hanzi_freq.py 的 `CJK_RANGES`，四处共用 `is_cjk()` 判定。

实测代价（同一台机器、同一 release 二进制、**新旧两份交错各跑 9 次取最小值**）：

| 产物 | 条目数 | 体积 | 装载（min / 中位，n=9） |
|---|---|---|---|
| trad.opid | 73715 → **96429**（+30.8%） | 2.10MB → **2.58MB**（+22.3%） | 18.0 / 20.3ms → **23.5 / 26.7ms** |
| luna.opid | 47422 → **70014**（+47.6%） | 1.19MB → **1.71MB**（+38%） | 10.9 / 11.7ms → **15.6 / 17.8ms** |

基线不是估算：把旧口径（`CJK_RANGES` 只留基本区）在脚本副本里复原后重跑两个生成器，
得到 24070+49645=73715 行 / 47422 行，编译出的 opid 条目数与字节数与放开前的记录
**完全一致**（73715 / 2109734、47422 / 1243463），确认基线就是放开前的产物。
装载耗时与条目数近似成正比（trad 条目 ×1.308 → 耗时 ×1.306；luna ×1.477 → ×1.431），
即新增耗时全部来自新增条目本身。注意单次测量噪声很大（同一文件不同次可差 60%），
比较必须新旧交错多跑取最小，勿用单次数字。

（trad 明细：单字 24070 → 46757 行，词组 49645 → 49672 行；luna 明细：单字 20902 → 41289，
词组 21719 → 21759。）

排序不受影响：扩展区字没有 kHanyuPinlu 词次，全部落入「无词次」段（段位 2、码位序），
仍排在 GB2312 与常用字之后，既有候选顺序不变，只是 freq 间距因行数增加而变密。
门禁 `rare_coverage` 锁扩展区代表字的**可达性**（存在性），`ranking_quality` 继续锁名次。

**局限：收录 ≠ 可见。** 扩展 B 依赖设备字体，各厂商覆盖差异很大（Noto Sans CJK /
思源对扩展 A 尚可，扩展 B 常见 tofu 空白框）。打出来是空白框，用户只会认为是输入法 bug。
扩展 C 及以后（另有约 3024 个带 kMandarin 的码位）有意**不**收，理由见 `CJK_RANGES` 注释。

符号面板侧：**未收「CJK 扩展 A」区块**（原 `builtin()` 声明了 U+3400–4DBF 却零条目）。
非「砍数据」而是该区块在面板里本就没有可用性：其 Unicode 名一律是 `CJK UNIFIED
IDEOGRAPH-3400` 这类码位标签，分词后的关键字彼此不可区分，搜索与浏览双双失效，
而「不知道读音」正是扩展 A 的常态 —— 任务一已按拼音让它可打（打 qiu 出 㐀）。
id 5 空出不编号：改编号会让认为「5 = 扩展 A」的旧代码**静默**取到平假名。

## 外部词库并入（2026-10-11；用户裁决「主流开源词库去重整合」）

**一句话**：`luna.opid` 从 96,341 条扩到 **936,254 条**（词组改三段序），体积
2.37MB → 33.27MB；**合并产物自此按 GPL-3.0 分发**（并入源含 GPL-3.0-only 的
rime-ice），生成器源码仍 MIT。

### 来源与取舍（全部 pin + sha256 + 行数，见上「上游 pin」表）

| 源 | 许可 | 层 | 并入内容 |
|---|---|---|---|
| rime-ice base + ext（雾凇拼音） | **GPL-3.0-only** | 批量 | 520,282 + 317,532 条（现代词覆盖主力：微信/内卷/芯片/比亚迪…） |
| rime-essay | LGPL-3.0 | 批量 | 31,781 条（无拼音，按字注音；繁形丢弃 281,016） |
| chinese-xinhua idiom / xiehouyu | MIT | 批量 | 成语 2,903 + 歇后语 12,969 条 |
| THUOCL IT/医疗/法律/财经 | MIT | 批量 | 89 + 701 + 57 + 161 条（与 base 大量重复——base 本身就是从 THUOCL 拼装的，净增 ~1,000） |
| 品牌 / 新词 / 注音覆盖表 | MIT（本项目自撰） | **前置段** | 609 + 391 + 422 条 |

**不收**（逐条核证）：rime-ice 的 tencent 层（腾讯词向量官方条款已不可得）、
8105/41448/others 层（拼音含 zh.wiktionary CC BY-SA 成分）；jieba（数据来源未说明，
issue #559 作者仅答「统计过来的」）；fcitx5-pinyin-zhwiki 全家（数据=维基百科派生，
CC BY-SA 4.0 + GFDL）；libpinyin（GPL 壳 + 数据无独立声明）；SunPinyin（仓库内无
词典数据）；白霜/GPL；rime-wanxiang（CC BY 4.0 但未声明词库上游、署名义务无从履行）；
phrase-pinyin-data（63% 汉典抓取 + 21% CEDICT，剥离后仍脏）。

### 层序与判据（改动前先读）

- 词组三段序：**extra_manual（品牌/新词）→ 既有 simp 词组 → extra_phrases（批量）**。
  人工前置的理由是实测反例：`威信`（simp 层权重 3.71e9）会把批量段里的「微信」压到第 3。
  simp 段自身相对序不变（整体顺延人工段行数）；13 处首位钉与 m2/cabi 硬位置均已
  扫描过与人工表零撞键（测试 `crates/opi-tools/tests/phrase_quality.rs`）。
  姓氏/流行语（2026-10-18）接在 extra_phrases **队尾**、不进前置人工段：流行语要求
  不压同键常用词（实测 dazi 有 大字/打字、eren 有 恶人、shangtou 有 伤透/上头），
  单姓键若进人工段还会污染 top_keys 把品牌词误降级。
- **人工段降级规则**（2026-10-11，第二版）：人工词是某条**高频短词**的延续时
  （中国邮政←中国、打工人←打工、腾讯视频←腾讯），移出人工段、放批量段队首。
  来由是 `zongguo`（「中国」的模糊输入）实测：人工段的 中国邮政/中国黄金
  （频率 3.8026e9）在**模糊比较**（无精确加成那条路）里把「中国」（3.8003e9）
  顶出前 8。判据 = 键的真前缀 ∈ top_keys（人工段全部词键 ∪ simp 段前 **4096**
  名的词键）；4096 是校准边界，锚点：中国（#51）触发、weixi 一族（微细/尾溪，
  约 #4 万名）不触发（错误的第一版把 `微信` 也降走、weixin 首位钉直接红）。
  降级只影响无加成比较与同键排序 —— 直接打全拼音走精确层（有加成）不受影响，
  同键无竞争者的降级条目（打工人/显眼包/特斯拉）仍是该键首位。
- 层内键（只影响同键先后）：base = 权重>1 先、`-权重,len,word`（6.2 万条权重=1 沉底）；
  ext = `len,word`；essay = freq>0 先、`-freq,len,word`；成语/歇后语 = `len,word`；
  术语 = `-DF,word`；品牌/新词 = 文件序；姓氏/流行语 = `-freq,len,word`。
  **不做跨源词频校准**（候选竞争只在同键发生；两表频次列只是层内档位，真实频率
  由 gen_luna_dict 按行位置编码）。
- **繁形判据（2026-10-11 升级）**：`OpenCC TSCharacters（剔除自靶行）∪ 旧 Unihan 判据`，
  两者共用 GB2312 门。来由与边界：Unihan 的 kSimplifiedVariant **不是繁→简表**
  （線→[缐]、喫/遊/錄 没有），单用它会漏 essay 层 **7,644 条**繁体词形（字频榜前 30
  全是 線/喫/遊/羣…）；TSCharacters 的含自靶行（`乾→干 乾`）保护乾隆/覆盖类；
  GB 门保护 後/於/吒（GB2312 成员，按本仓「GB 归属=简体字形」权威判据保留 ——
  哪吒 曾因 `吒→咤` 无自靶被误杀，门后修复）。**丢弃不转换**（繁→简非单射是先例）。
- 注音三级：词级人工覆盖 → kHanyuPinlu 词次 argmax → kMandarin 首读。**内建自测
  92.9%**（vs base/ext 真拼音剥掉重注，n=5000，下限 90%）+ 错字榜（地/长/行/重/调…）
  驱动覆盖表增长；覆盖表 422 条中 8 条经上游真值复核修正（了却 liaogue→liaoque、
  弹道 daodao→dandao 等笔误）。**如实记：**被注音源 ~7% 的词键是猜的（多音字无
  语境），打对了的键不受影响，猜错的表现为「该词在该拼音下查不到」。
- 音节校验读单源：解析 `crates/engine-core/src/pinyin.rs` 的 SYLLABLES（断言 410）
  做全切分，不复制音节表（跨语言镜像常量会静默漂）。长度上限：批量 ≤6 字、成语 ≤8、
  人工表 ≤12、歇后语 ≤24；全源 ≥2 字（单字层不动；姓氏/流行语例外：允许单字，
  如姓氏 单、流行语 润）。歇后语整条为词（含 `——`），
  键只由 CJK 字生成 —— 如实记：**档案价值为主**（长句极少被整条打）。

### 逐层实测（**以生成器每次打印的 stderr 为准**，下表是 2026-10-11 快照）

| 层 | 原始 | 繁形丢弃 | 其他丢弃 | 最终 |
|---|---|---|---|---|
| 品牌 | 600 | 0 | 0 | 600 |
| 新词 | 235 | 0 | 0 | 235 |
| base | 543,118 | 247 | 长度 21,421 / 对齐 392 / 字面 78 / 音节 137 / 重复 561 | 520,282 |
| ext | 339,186 | 41 | 长度 16,536 / 对齐 4,975 / 字面 17 / 音节 1 / 重复 84 | 317,532 |
| essay | 442,688 | 281,016 | 长度 4,410 / 字面 20,785 | 31,781 |
| 成语 | 30,895 | 146 | 长度 486 / 对齐 109 / 音节 49 / 重复 27,202 | 2,903 |
| 术语（四表） | 48,475 | 0 | 与 base/其它重复为主 | 1,008 |
| 歇后语 | 14,032 | 1 | 长度 442 / 字面 261 | 12,969 |

「对齐」= 上游音节数与 CJK 字数不等（多为含 `·`/`-` 的译名，实测多为 〇 U+3007 与
扩展 B 以后的字 —— 与单字层收录域一致，**有意丢弃**）。注音计数：kph 29,254 /
kmandarin 16,559 / override 5 / **suppressed 141,729**（注音层的词已在有拼音层出现
⇒ 整条抑制）/ miss 1 / sylbad 364。总量：人工 1,000 条中 743 条留前置段、257 条按
降级规则并入批量段队首 ⇒ **743 + 48,078 + 886,623 = 935,444 词组** + 48,263 单字行
= 983,707 行 → compile 去重后 **936,310 条目 / 33,274,020 字节**，`verify` load
96.1ms（旧库 15.6ms；一次性、随文件线性）。

> **2026-10-18 增补**：并入姓氏/流行语两表（原始 484 + 597 条，经本层与跨层去重后
> 净新增 191 条）后，当前条目数看 `opi-tools verify` 的 `entries:`，字节数看
> `stat -c %s`；去重计数看生成器 stderr。上面的表格与合计是 2026-10-11 快照，
> 保留作历史对照。

### 单字母前缀性能（头号风险，已解决）

未加保护前：新库单字母前缀 80,502 条 / **41–50ms**（其中 `dict.query` 物化占 24.6ms；
旧库同输入 3.5ms）。解法（同日用户裁决）：`dictionary::PREFIX_MATCH_CAP = 9,216` ——
**前缀扩展层**按静态词频截断，**精确层不截**（「候选总数不设上限」的承诺保留在打全
拼音的精确查询上）；`MmapDictionary` 流式 top-N（第一遍只读 freq 列做 select_nth，
第二遍只物化 exact + top-N）。9,216 = 旧库最大单字母前缀原始命中 8,613 + 余量 ⇒
**旧库行为逐字不变**（探针自证：z 仍 8,447 条，与新代码前逐字相同）。如实边界：
学习词借 boost 跳进前缀 top-8 的机会只在 top-N 池内有。9/14 键的**按键类层**同族
（`dictionary.rs` 的 `SCHEME_MATCH_CAP = 2048` + loader 的区间枚举，单键 268→1.9ms）——
全端性能预算与完整实测表在 [`docs/performance.md`](../../docs/performance.md)。

实测（新旧同进程交错取最小，release）：单字母 z/y/x/w/s/d/zh **3.3–3.8ms**（旧
1.0–2.9ms）、`ni` 6.7ms（主层+模糊层两个 capped 层）、其余常用输入 ≤0.4ms；全部
单字母候选 ≤9,200。⚠️ **9/14 键方案（`visit_entries` 全表扫描）与候选上限无关，
本次未测** —— 936k 张表上 9 键首键的扫描耗时待真机量。

### 许可证义务（合并产物按 GPL-3.0）

`luna.opid`（及 extra_manual.tsv / extra_phrases.tsv）含 rime-ice 的 GPL-3.0-only
内容 ⇒ **合并产物整体按 GPL-3.0 分发**；.deb/.rpm 随包带 GPL-3.0 全文
（`data/raw/GPL-3.0.txt`，`scripts/nfpm.yaml` 装进 `/usr/share/doc/opi/`）。生成器
与其它源码仍 MIT。如实记录（不做法律论断）：rime-ice 自述为多源拼装（华宇野风 +
THUOCL + 现代汉语常用词表 + 腾讯词向量），本次只收 base+ext（其 tencent 层与
含 wiktionary 拼音的 8105/41448 层不收），**base+ext 的注音上游链路上游未逐条声明**。

## 五笔码表 wubi86 / wubi98（2026-10-14；M4 计划 3a）

**裁决**：86 + 98 入库，新世纪（06）不做（无合规源）。两表由 `scripts/gen_wubi_dict.py`
一处生成（钉 commit + sha256 逐次校验；上游文件缓存在 `data/generated/`、未入库；产物幂等）。
**条数以生成器每次打印的为准。**

**98 为什么是两份拼的**：`超集-rime格式码表.txt` **一条词组都没有**（全表 109,842 行全是
单字，含简码形态，字覆盖 99,050），而 `98五笔含词表-【单义】.txt` 的词组（76,150 行）是唯一
来源；含词表的单字实测全部 ⊂ 超集-rime 那份（0 例外，脚本里是断言）⇒ 单字段取 rime 格式、
词组段取含词表，零丢失。

**权重语义（两表不同，别当同一把尺）**：86 的 weight 是真实词频（含 42,724 行 0 值 —— 多为
扩展区生僻字，排序沉底但仍可查）；98 单字的「权重」实测为**文件序的稠密排名**（1098420−i），
只保证同码内相对序、**不是词频**；98 词组无权重，按 `parse_dict` 的缺省 1000。⇒ 同码内的
相对序有意义，**跨表、跨码的 freq 比较无意义**。浮点权重（86 实测 1 条 χ/zzxx `450.5`）在
生成器里取整 —— 原样落盘会被 `parse_freq` 按 unusable 整条丢弃（χ 从 zzxx 静默消失）；
取整计数打在 stderr。

**残留不一致（如实记，不做法律论断）**：
- 86 的上游链路：rime-wubi 仓自述 LGPL-3.0，但原始码表提供者 acevery 仓的 COPYING 实测是
  GPL-3.0 全文（rime 的 AUTHORS 只记了 LGPL 侧）。两者同属 copyleft 家族；随包分发要求的
  许可文本义务由已在仓库的 `GPL-3.0.txt` 覆盖。
- 98 的「已无版权问题」：README 该节只挂了一条 bilibili 视频（讲的是**专利**到期，不是版权
  授权）；版权依据取仓库自己的 Unlicense 全文 LICENSE（blob `fdddb29aa445`）。**视频不作依据。**

**有意保留、生成侧不过滤的两类行**（lead 已裁决保留，2026-10-14：与本仓 stroke 态度一致
「超集照收」，可见性由显示侧的 `GlyphCoverage` 过滤 —— 词表层不替显示层做减法；改之前先读
生成器文件头）：
- 86 的 z 前缀行（实测 677 条：zzjp 片假名 / zzpj 平假名 / zzsx 数学符号 / zzpp 部首…）：
  上游的「分类表」约定，不是五笔编码；z 在五笔里通常是万能键，两者会不会打架是 wubi-engine
  的设计问题，数据侧原样保留并报数。
- 词含 CJK 基本区/扩展 A/B 之外字符的行（86 830 行、98 30,135 行：扩展 C+、康熙部首、假名、
  符号）。本仓收录范围口径（`hanzi_freq.CJK_RANGES`）是给拼音词库定的；五笔表是**码表**、
  超集字是它的卖点，故不过滤。**局限：收录 ≠ 可见** —— 扩展 C+ 与假名多数字体不覆盖，
  打出来是 tofu（与上面「收录范围」节同一结论）。

**自洽校验**（每次生成打 stderr，条数实测、不许写死）：码只准 a-z、码长 1–4；同码冲突分布
（「四码唯一自动上屏」的判据输入）；词组码规则核验（2 字 = 各取前两码、3 字 = 前两字首码 +
末字前两码、4 字 = 各首码、>4 字 = 前 3 首码 + 末字首码）；编译自检（`opi-tools compile` 的
kept entries == 生成行数、且 stderr 零「跳过」警告 —— 任何静默丢行都在这条上红）。核不到的
如实记：词组组成字不在单字表里的行数、组成字多全码（取字典序首个）的行数，以及上游自身的
28 条词组码不符规则（86；如 蹒跚 表内 `kakm`、按规则 `khkh`；占 0.05%）—— **原样保留**
（数据保真优先，不替上游修码）。

**分发面**：两表暂不随任何端分发（`.opid` 已入库 `data/generated/`）；接入端时要改的清单
（fcitx5 安装落点/`opi_locate_check.cpp`/CI grep、Android assets、nfpm/MSI 装载清单）由接入
任务列出。
