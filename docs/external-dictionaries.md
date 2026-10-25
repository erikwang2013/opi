<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT（本文；文中数据产物另有许可） -->

# 外部词库导入：CustomPinyinDictionary / rime-wanxiang

> **这是可选功能，不是随包数据。** 本仓库不包含、也不在构建时下载这两份词库。
> 下面的脚本把上游数据转成 OPI 的**导入词库**格式；生成的 `.tsv` 由用户自己放进
> 词库目录。仓库只提交几十行级的测试夹具（见
> [`crates/engine-data/tests/fixtures/external/README.md`](../crates/engine-data/tests/fixtures/external/README.md)）。

## 1. 产物格式（装载契约）

`engine-data::dict_text::load_extra_dict`（`crates/engine-data/src/dict_text.rs`）读的是
UTF-8 文本，每行：

```
词<TAB>拼音[<TAB>频次]
```

- `#` 注释、空行、rime front-matter（文件开头的 `---` … `---`/`...`）会被跳过；
  两个转换器都把**署名/许可/改动说明写在 `#` 头部**，装载器不会把它当词条。
- 拼音必须是 ASCII、≤255 字节；词 ≤255 字节；空字段跳过。
- 频次可省略（默认 1000）；整数或 `NN.NN%`（按 `round(pct × 1000)`）。
- 同 `(拼音, 词)` 重复取**最大**频次；同词不同读音都保留。

示例（`crates/engine-data/tests/fixtures/external/cpd_tiny.tsv` 的正文）：

```
爸爸	baba
女儿	nver
```

## 2. 源一：CustomPinyinDictionary

- 仓库：<https://github.com/wuhgit/CustomPinyinDictionary>
- 许可：**CC BY-SA 4.0**（数据）— <https://creativecommons.org/licenses/by-sa/4.0/>
- Pin：仓库 commit `cf17f96af885cb818c2fad87184f383a52482351`；release tag `assets`；
  数据构建 `v20260101`（`status.json` 的 `updateDate=2026-01-01`、
  `totalWords=1,498,781`）
- **仓库树里没有词库文本**。数据只在 release 资产里：

| 资产 | 大小 | SHA-256（实测） | 实际格式 |
|---|---:|---|---|
| `CustomPinyinDictionary_Fcitx.dict` | 29,688,969 B | `63677b0e1bcd9276e8eeef41553ab532bf6061278558d9efa3629b0ebe8836e5` | libime 二进制词典（偏移 8 起是 zstd 帧），**无可用词频**（dump 第三列恒 0） |
| `CustomPinyinDictionary_Gboard_Magisk_20260101.zip` | 30,270,378 B | `7619637bb522931702d676df03448e9419bd8c4baa269d8ad8a9391cadd6dac2` | zip；`dict/db` 是 SQLite `entry(_id, word, shortcut, locale)`，同样 1,498,781 行、无频次列 |
| `CustomPinyinDictionary_Fcitx_Magisk_20260101.zip` | 29,847,455 B | `d3e75af0d7a9b2d4164776082518281796c6a24a85471a8b7cf292f443f26ba9` | zip；`dict/data` 仍是 libime 二进制 |

`libime_pinyindict -d <in> <out>` 转出的文本是 `词<空格>音节'音节<空格>0`。实测
1,498,781 行全部 3 字段、code 只含 `[a-z']`、无重复。**同一 500 条前缀摘录**分别从
libime dump、libime 二进制、Gboard SQLite zip 转出的正文逐字节一致（三条路都走同一
转换器；完整 1,498,781 条的产物见下表）。

### 转换

```bash
# A. 纯 Python（推荐；不需要 libime）——Gboard zip 里的 SQLite
curl -L -o /tmp/CustomPinyinDictionary_Gboard.zip \
  https://github.com/wuhgit/CustomPinyinDictionary/releases/download/assets/CustomPinyinDictionary_Gboard_Magisk_20260101.zip
python3 scripts/convert_custom_pinyin_dictionary.py /tmp/CustomPinyinDictionary_Gboard.zip -o cpd.tsv

# B. Fcitx 二进制（需要 libime-bin 的 libime_pinyindict；Debian/Ubuntu apt install libime-bin）
curl -L -o /tmp/CustomPinyinDictionary_Fcitx.dict \
  https://github.com/wuhgit/CustomPinyinDictionary/releases/download/assets/CustomPinyinDictionary_Fcitx.dict
python3 scripts/convert_custom_pinyin_dictionary.py /tmp/CustomPinyinDictionary_Fcitx.dict -o cpd.tsv

# C. 已经 dump 成文本
python3 scripts/convert_custom_pinyin_dictionary.py /tmp/cpd-dump.txt -o cpd.tsv
```

脚本自动识别 zip / SQLite / libime 二进制 / dump 文本；zip 优先取 `dict/db`（纯标准库），
没有才用 `dict/data`。产物是**两列**（上游没有可用词频，写第三列只会伪造数字），装载器
按默认 1000 处理。

| 模式 | 条目 | 大小（实测） | 说明 |
|---|---:|---:|---|
| 缺省（`--max-entries 200000 --max-output-mb 8`） | 200,000 | 2.72 MiB | 走 Android/macOS 设置窗导入面（见 §4） |
| 完整（`--max-entries 0 --max-output-mb 0`） | 1,498,781 | 34.42 MiB | 只能走 fcitx5/Windows 的词库目录直载 |

无权重源的截断顺序 = **短词优先**，同长度按拼音/词稳定排序；被丢弃的条数会写进
stderr 和产物头部。这是确定性丢弃，不是随机采样。

## 3. 源二：rime-wanxiang

- 仓库：<https://github.com/amzxyz/rime-wanxiang>
- 许可：**CC BY 4.0** — <https://creativecommons.org/licenses/by/4.0/>
- Pin：commit `b7d4ee298c0c933ee5cfd2521f98cee8b9d4bde4`（2026-10-25，分支 `wanxiang`）
- **仓库约 34 GB**（含模型/音频），不要整仓 clone。只按 raw URL 取表。`wanxiang.dict.yaml`
  的 `import_tables` 列出的核心表：

| 路径 | 大小 | 角色 |
|---|---:|---|
| `dicts/zi.dict.yaml` | 616,559 B | 基础字表（一字多读音） |
| `dicts/jichu.dict.yaml` | 49,485,140 B | 基础词库（主力；本页实测只转 zi+jichu） |
| `dicts/cuoyin.dict.yaml` | 8,116 B | 错音错字兼容 |
| `dicts/duoyin.dict.yaml` | 137,474 B | 多音兼容词 |
| `dicts/diming.dict.yaml` | 3,307,073 B | 地名 |
| `dicts/mingren.dict.yaml` | 667,181 B | 名人 |
| `dicts/yiren.dict.yaml` | 423,957 B | 艺人 |

（`yixue`/`huaxue`/`yaopin`/`wuzhong` 等在源 `wanxiang.dict.yaml` 里是注释掉的。）

### 转换

```bash
BASE=https://raw.githubusercontent.com/amzxyz/rime-wanxiang/b7d4ee298c0c933ee5cfd2521f98cee8b9d4bde4/dicts
curl -L -o /tmp/wx-zi.dict.yaml    "$BASE/zi.dict.yaml"
curl -L -o /tmp/wx-jichu.dict.yaml "$BASE/jichu.dict.yaml"

# 核心两份合并（自动跨文件去重）
python3 scripts/convert_rime_dict.py /tmp/wx-zi.dict.yaml /tmp/wx-jichu.dict.yaml -o wx.tsv

# 想要更大的覆盖：把 diming/mingren/yiren/duoyin 也加上，仍是一条命令
curl -L -o /tmp/wx-duoyin.dict.yaml "$BASE/duoyin.dict.yaml"
python3 scripts/convert_rime_dict.py /tmp/wx-zi.dict.yaml /tmp/wx-jichu.dict.yaml \
  /tmp/wx-duoyin.dict.yaml -o wx.tsv

# 完整（不做 200k/8MiB 截断）
python3 scripts/convert_rime_dict.py /tmp/wx-zi.dict.yaml /tmp/wx-jichu.dict.yaml \
  -o wx-full.tsv --max-entries 0 --max-output-mb 0
```

转换口径（与装载器逐条对齐）：

- 跳过 `#` 注释、空行、front-matter；**front-matter 未闭合直接报错**，不把配置行当词条。
- `词<TAB>code[<TAB>权重]`：两列 = 1000；三列整数直取；`NN.NN%` 按
  `round(pct × 1000)`（舍入方向与 Rust 的 `f64::round` 对齐）。
- code 去声调（NFD 去组合符）、`ü`→`v`、小写、去音节空格与 `'`：
  `nǐ hǎo` → `nihao`、`nǚ ér` → `nver`。
- 同 `(词, 拼音)` 取最大权重；不同读音（`行 háng/xíng`）都保留。
- 非法权重（上游实测 15 处 `6N`/`45N`/`10s`）降级为 1000 并计数，不丢词。
- 词里含 `\` 的条目跳过（那是 Rime 自定义短语的展开转义，如 `静夜思\n...`）。

| 模式 | 条目 | 大小（实测） | 说明 |
|---|---:|---:|---|
| 缺省（200000 条 / 8 MiB） | 200,000 | 4.69 MiB | 走设置窗导入面 |
| 完整 zi+jichu | 1,543,055 | 39.22 MiB | 走 fcitx5/Windows 目录直载 |

完整实测的中间数：解析 1,544,753 行、按 `(词, 拼音)` 去重 1,698 条、非法权重 15 条、
坏行/坏词/坏拼音各 0。

## 4. 各端怎么加载

导入词库的名字 = **文件名去 `.tsv`**；停用名单写的就是这个名字。各端的目录契约与
设置入口：

| 端 | 词库目录 | 入口 | 单文件上限 |
|---|---|---|---|
| **Android** | `filesDir/dicts/*.tsv`（应用私有目录） | 设置页「词库」一节（导入 / 启停 / 删除）；启动时 `DictStore.loadAll` 逐个 `importDict` | **8 MiB / 200,000 条**（`opi-ffi` 的 `MAX_IMPORT_DICT_BYTES` / `MAX_IMPORT_DICT_ENTRIES`），同时启用 ≤8 个、总计 ≤500,000 条 |
| **Linux（fcitx5）** | `$XDG_DATA_HOME/opi/dicts/*.tsv`，缺省 `~/.local/share/opi/dicts`；随包的康熙字典种子也在这里 | 桌面设置窗「词库」页管同一目录；fcitx5 配置的 `DisabledDicts=` 控制停用；配置生效时重扫 | 本轨**直接**调 `engine_data::load_extra_dict`，没有 8 MiB/200k 这两条限制（完整词库可用） |
| **Windows（TSF）** | `%LOCALAPPDATA%\opi\dicts\*.tsv` | 桌面设置窗「词库」页（导入即复制进目录）；TSF 在 `Activate`/`reload` 时重扫 | TSF 轨直接 `load_extra_dict`，无 8 MiB/200k 限制 |
| **macOS** | `~/Library/Application Support/opi/dicts/*.tsv` | 设置窗「词库」页；启动时 `OpiEngine.loadImportedDicts` 重放 | 走 `opi_import_dict`（与 Android 同一份 FFI 校验）⇒ **8 MiB / 200,000 条**；真机未验收 |
| **iOS** | — | 没有导入词库入口（键盘扩展只读随包词库） | 不可用 |
| **鸿蒙** | — | 导入词库/康熙仍是未接线通路（见 `docs/install/harmony.md` 的词库行） | 不可用 |

设置窗的「导入词库」按钮在 Linux/Windows 上**只复制文件到目录**（`Dicts.kt::importDict`，
重名自动 `-2` 后缀，不覆盖），随后请输入法重扫；Android 在复制后调 FFI `importDict`，
macOS 也是先复制、再调 `OpiEngine.importDict`（失败会删掉刚复制的文件）。所以
Android/macOS 会撞上面那两条上限。完整 34–39 MiB 的词库在 Android/macOS 上会被
拒（Android 明说「词库文件达到上限（8 MiB）」/「词库条目达到上限（200000 条）」；
macOS 走 FFI 的通用拒收文案），请用缺省截断产物，或改用有目录直载的 Linux/Windows。

### 装进去之后的行为（如实记）

- 导入层**抢位**：重复词取 `max(主库词频, 加成)`，独有词也按加成抬升（见
  `engine-core/src/dict_layers.rs` 的 `boosted_import_freq`）。所以导入一份大词库会
  改变候选顺序，这是「是词库就要抢位」的既有裁决，不是本转换器引入的。
- 导入层扫描有**每键共享预算**（`IMPORT_SCAN_BUDGET` = 初始导入 20ms；2026-10-22 起
  总账 30ms，base 查询后按剩余时间动态收紧）。宽前缀下超时会**整体丢弃导入层**、
  本轮纯内置，不粘滞。超大词库的内存/首击成本由用户自己权衡；随时可在词库页停用。
- 导入层只叠**主库**；繁体模式仍查 `trad.opid`，不会看到导入层（`dict_layers.rs` 裁决 3；
  `trad.opid` 缺失而回退主库时，导入层自然可见）。
- 学习词仍优先于导入层；用户词不会因为导入大词库而失效。

## 5. 许可义务（分发产物前必读）

两个转换器本身是 MIT（`scripts/convert_*.py` 的 SPDX 头）。**转换出来的数据不是 MIT**：

### CustomPinyinDictionary（CC BY-SA 4.0）

- 必须署名（dictionary data by wuhgit and contributors）、给出许可证链接、**标明改动**。
- **ShareAlike**：改编数据再分发时必须以 CC BY-SA 4.0（或兼容许可）授权，不能加额外限制。
- 上游 README 还感谢了 FREEMDICT、Administrative-divisions-of-China、海关总署、国家统计局
  等第三方来源；仓库声明的许可是 CC BY-SA 4.0，但再利用前请自行核对这些来源的条款。
- 产物头部的 `#` 块就是署名/改动声明；**别删**。把数据并进别的库时同样要保留。

### rime-wanxiang（CC BY 4.0）

- 必须署名（rime-wanxiang data by amzxyz and contributors）、给出许可证链接、标明改动。
- **没有** ShareAlike 义务，但署名与改动声明仍是强制项。
- 同样别删产物头部。

本仓库的 `LICENSE`（MIT）只覆盖代码与项目自撰数据；上述产物是用户在本机生成的
第三方改编数据，不进入仓库、不随包分发。

## 6. 测试与复现

```bash
# 转换器（离线；逐字节比对已提交的夹具产物）
python3 -m unittest discover -s scripts/tests -v

# 装载器（Rust；把两份转换产物真的装进 InMemoryDictionary 并查询）
cargo test -p engine-data --test external_dict_fixtures
```

夹具与上游摘录的署名/改动声明见
[`crates/engine-data/tests/fixtures/external/README.md`](../crates/engine-data/tests/fixtures/external/README.md)。
本页的数字来自 2026-10-26 本机实测（pin commit 见上）；上游更新后请重跑脚本，
**不要手改**产物或把数字抄进别的文档。

## 7. 已知限制

- **没有改引擎排序**：转换器只产出数据；候选顺序仍由 `rank_and_pick` 与导入层裁决决定。
- CPD 无可用词频（dump 第三列恒 0），只有默认 1000；同键排序靠词长/字典序，不代表真实使用频率。
- rime-wanxiang 的权重是上游 `sort: by_weight` 的值（可达数千万），导入后会被引擎的
  抢位加成/用户档共同约束；不要把它当「绝对优先级」。
- 截断是**确定性丢弃**：缺省 200k 条/8 MiB 只适合设置窗导入面；要全量请显式关掉
  两个上限，并确认目标端走的是目录直载。
- 转换器不解析 Rime 的 `lua`、模型、音频、`custom/*.txt` 码表；只针对
  `*.dict.yaml` 的 `词<TAB>拼音[<TAB>权重]`。
- CPD 数据在 release 资产里，**不受 commit pin 保护**（tag `assets` 可能被上游重传）；
  下载后请对照 §2 的 SHA-256，再转换。
- 各端 GUI 导入路径**未在本机 GUI 实测**；本页引用的是源码与各端安装文档的契约。
  真机行为以各端文档的「证据档位」为准。
