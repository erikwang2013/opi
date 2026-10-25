<!-- SPDX-FileCopyrightText: 2026 erik.xyz -->
<!-- SPDX-License-Identifier: MIT（本 README 的文字；下面的数据夹具不是 MIT） -->

# 外部词库夹具（测试专用）

本目录只放**几十行级**的摘录，用于验证两个转换器（`scripts/convert_custom_pinyin_dictionary.py`、
`scripts/convert_rime_dict.py`）的输出能被 `engine-data::dict_text::load_extra_dict` 装载。
**不随应用分发**，也不是完整词库。完整词库的生成方式、上限与各端加载路径见
[`docs/external-dictionaries.md`](../../../../../docs/external-dictionaries.md)。

| 文件 | 内容 | 许可 |
|---|---|---|
| `cpd_dump_tiny.txt` | CustomPinyinDictionary 的 `libime_pinyindict -d` 文本摘录（7 行） | **CC BY-SA 4.0** |
| `cpd_tiny.tsv` | 由上面的文件转换出的 OPI 导入词库（2 列） | **CC BY-SA 4.0**（改编数据） |
| `wanxiang_tiny.dict.yaml` | rime-wanxiang 的 `dicts/jichu.dict.yaml` + `dicts/zi.dict.yaml` 摘录（14 行） | **CC BY 4.0** |
| `wanxiang_tiny.tsv` | 由上面的文件转换出的 OPI 导入词库（3 列） | **CC BY 4.0**（改编数据） |

## 上游与署名

### CustomPinyinDictionary

- 仓库：<https://github.com/wuhgit/CustomPinyinDictionary>
- 仓库 commit：`cf17f96af885cb818c2fad87184f383a52482351`（2026-09-30）
- 数据构建：release tag `assets`，`v20260101`（`status.json`: `updateDate=2026-01-01`、
  `totalWords=1,498,781`）
- 完整产物：`CustomPinyinDictionary_Fcitx.dict`（29,688,969 B，
  SHA-256 `63677b0e1bcd9276e8eeef41553ab532bf6061278558d9efa3629b0ebe8836e5`）
- 许可证：**CC BY-SA 4.0** — <https://creativecommons.org/licenses/by-sa/4.0/>
- 署名：dictionary data by wuhgit and contributors。上游 README 还感谢了 FREEMDICT、
  Administrative-divisions-of-China、海关总署、国家统计局等第三方来源；再利用前请自行核对
  这些来源的条款。
- **改动声明**：`cpd_tiny.tsv` 由 `cpd_dump_tiny.txt` 去掉 `'` 音节分隔、按
  `(词, 拼音)` 去重后生成；上游数据没有可用词频（libime dump 第三列恒 0），故输出为两列（OPI 装载器默认 1000）。
- **ShareAlike**：`cpd_tiny.tsv` 是 CC BY-SA 4.0 的改编数据；再分发必须保留署名、
  标明改动，并以 CC BY-SA 4.0 授权。它**不是** MIT。

### rime-wanxiang

- 仓库：<https://github.com/amzxyz/rime-wanxiang>
- 仓库 commit：`b7d4ee298c0c933ee5cfd2521f98cee8b9d4bde4`（2026-10-25，分支 `wanxiang`）
- 摘录文件：
  - `dicts/jichu.dict.yaml`（49,485,140 B，SHA-256 `548b64b77dc4bec1902a3f865a096dbe85e341fbff2c285d984dea18ba50bec1`）
  - `dicts/zi.dict.yaml`（616,559 B，SHA-256 `7e23bbe3e601bc3a047aaf2e5d9b8f620ec4bd5d125a59169e8104440a860b66`）
- 许可证：**CC BY 4.0** — <https://creativecommons.org/licenses/by/4.0/>
- 署名：rime-wanxiang data by amzxyz and contributors。
- **改动声明**：`wanxiang_tiny.tsv` 去掉声调（NFD 去组合符）、ü→v、去掉音节空格、
  按 `(词, 拼音)` 取最大权重去重；`6N` 这类非法权重降级为 1000。
- CC BY 4.0 **没有** ShareAlike 义务，但必须保留署名、许可证链接并标明改动。

## 重新生成（不要手改 `*.tsv`）

在仓库根运行：

```bash
python3 scripts/convert_custom_pinyin_dictionary.py \
  crates/engine-data/tests/fixtures/external/cpd_dump_tiny.txt \
  -o crates/engine-data/tests/fixtures/external/cpd_tiny.tsv

python3 scripts/convert_rime_dict.py \
  crates/engine-data/tests/fixtures/external/wanxiang_tiny.dict.yaml \
  -o crates/engine-data/tests/fixtures/external/wanxiang_tiny.tsv
```

`scripts/tests/test_external_dict_converters.py` 会重跑这两条并**逐字节**比对已提交的
`*.tsv`（表头含源文件 SHA-256，所以夹具一改，输出也必须同步改）。
