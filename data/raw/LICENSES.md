# 词库数据来源与许可证

> 本文件只记录**数据**的许可证：仓库源码（`crates/`、`android/`、`desktop/`、`scripts/` 等）为 **MIT**，见 [`../../LICENSE`](../../LICENSE)；
> `data/raw/*.tsv` 及由其编译出的 `.opid` **不适用 MIT**，各自按下表（`fallback.tsv` 例外：本项目自建，故为 MIT）。

| 文件 | 来源 | 许可证 | 说明 |
|---|---|---|---|
| fallback.tsv | OPI 项目自建 | MIT | 内置回退词库，由 opi-tools 编译为 data/generated/fallback.opid |
| luna_pinyin.dict.yaml（M2 验证用） | https://github.com/rime/rime-luna-pinyin | **LGPL-3.0** | 官方拼音词库，889KB ~70771 行；由 scripts/gen_luna_dict.py 重排后编译为 luna.opid（产物不直接入库）：单字第三列是读音概率（非词频），旧版据此以 GB2312 码序为常用度替身、次读音排目标读音组尾、0% 读音剔除；现改以 Unihan kHanyuPinlu 逐读音词次排序，且**有词次证据的读音一律保留**（上游 0% 只是份额为 0，不等于读音不存在；0% 过滤器只对无证据读音生效），词组仍排在单字之后，详见脚本头注释 |
| trad_hanzi.tsv（单字） | Unicode Unihan（kMandarin 读音 + kHanyuPinlu 词次 + kTraditionalVariant/kSimplifiedVariant 变体） | Unicode License（宽松，可再分发，保留版权声明） | GB2312 全量 6763 + terra 单字 + 其余 Unihan 单字，由 scripts/gen_trad_dict.py 生成（人工白名单 COMMON_TRAD 已退役，见脚本头注释） |
| trad_phrases.tsv（词组） | https://github.com/rime/rime-terra-pinyin（terra_pinyin.dict.yaml） | **LGPL-3.0** | 常用繁体词组 + 人工常用词组（SUPPLEMENT_PHRASES，terra 缺 臺灣/電話/謝謝 等），由 scripts/gen_trad_dict.py 生成 |
| symbol_blocks.tsv（符号区块） | Unicode UCD（UnicodeData.txt 的区块范围与字符名） | Unicode License（同 Unihan 行） | 面板 5 个区块的 id/范围/名，由 scripts/gen_symbols.py 生成；**符号面板的搜索关键字不来自上游**，约 280 条为 OPI 项目人工撰写（MIT），其余按 Unicode 名分词兜底 |
| symbols.tsv（符号条目，583 条） | 同上（UnicodeData.txt，逐码位字符名） | Unicode License（同上，不新增来源、不新增许可证） | 同生成器产出。**消费侧是编译期读取**：engine-core/src/symbols.rs `builtin()` 用 `include_str!("../../../data/raw/symbols.tsv")` 嵌入，故路径固定在 data/raw/ 而非 data/generated/（后者只放 .opid 二进制）；改目录必须两处同改，否则编不过 |

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
| UnicodeData.txt | Unicode **18.0.0**（与 Unihan.zip 同版本目录）`https://www.unicode.org/Public/18.0.0/ucd/UnicodeData.txt` | `0736451de439ae7baf1425136617da495e09ee5afbe6e394374db7009ea08950`（2243593 字节） | scripts/gen_symbols.py |

另：luna_pinyin.dict.yaml **仍未 pin**（上游取 rime-luna-pinyin master）。本次实测所用版本
sha256 `75bcf6eb3ff62b129882ed89cc22b2d1b80a5347aa72bcfa2ccc839bac298e7314`
（889896 字节 / 70771 行，与上表行内记载一致），用它重跑脚本产出的 luna.opid 与入库副本
逐字节相同 —— 当前 master 恰好可复现，但下次上游一动就断。补 pin 应照 terra 的做法做。

升级路径：改上表对应脚本里的版本号/commit + SHA-256 两个常量，重跑生成脚本，
再跑 `cargo test -p opi-tools --test ranking_quality` 与 `trad_coverage` 验收。
代价：**不再自动获得上游修正**（新增字、词次修订、读音订正、上游删词都要手动升级才进来）。

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
id 5 空出不重编号：改编号会让仍认为「5 = 扩展 A」的旧代码**静默**取到平假名。
