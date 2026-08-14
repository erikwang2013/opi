# 简繁全量支持（简繁模式切换）设计

**目标**：字库覆盖 GB2312 一二级全量 6763 简体单字 + 常用繁体单字约 13000 + 常用繁体词组；键盘模式条增加 简/繁 切换，繁体模式下输入拼音出繁体候选。

**非目标（YAGNI）**：
- 不做 Unicode CJK 扩展区（A/B/C…）生僻字全量
- 不做简→繁运行时转换（用户已选定独立繁体拼音库方案）
- 不做简繁同屏候选
- 不改变 .opid v1 格式（无格式升级）

## 架构总览

三层，各自独立可测：

```
数据层    opi-tools compile → data/generated/trad.opid（新增）
引擎层    engine-core：Mode 加 Traditional 变体；Engine 持有双词典 luna+trad，按模式路由
UI 层     Compose：模式键 中→繁→英 三态循环；Android EngineLoader 双资产加载
```

## 数据层

### 数据源与许可

| 数据 | 来源 | 许可 | 说明 |
|---|---|---|---|
| GB2312 全量 6763 单字拼音 | Unicode Unihan（kMandarin 字段）或公开 GBK 拼音表 | Unicode License（宽松，可再分发，保留版权声明） | 一级 3755 按拼音序（常用）、二级 3008 按部首序（次常用） |
| 常用繁体单字 ~13000 | Unihan kMandarin，筛选规则：CJK 基本区有 kMandarin 值的码位 − GB2312 已有码位 | 同上 | 简繁同形共码位字（如 中/大/小）只收一次 |
| 常用繁体词组 | rime terra-pinyin | LGPL-3.0（与现有 luna 同源同许可，LICENSES.md 已记录先例） | 台湾/中華民國/公司 等常用词组 |

### 词频策略（解决现有同频排序问题）

现状：luna dict.yaml 无词频列 → parse_dict 默认 freq=1000 → 同音字按字节序排列（query "hao" 前三 蒿/貉/鎬）。

trad.opid 词频规则（compile 前在 opi-tools 内做）：
- GB2312 一级字（3755）：freq = 9000 - 序号（按字表序，常用优先）
- GB2312 二级字（3008）：freq = 6000 - 序号
- 繁体单字：freq = 5000 - 序号（Unihan 排序）
- terra 词组：freq = 8000 - 序号 或沿用 rime 词序
- 与 luna 同拼音同字去重取最大 freq

### 产出物

- `data/raw/trad_hanzi.tsv`（单字，`word\tpinyin`，UTF-8；由 `scripts/gen_trad_dict.py` 从 Unihan 生成后提交，脚本入库）
- `data/raw/trad_phrases.tsv`（terra 词组）
- `data/generated/trad.opid`（opi-tools compile 合并产物，提交入库）
- `data/raw/LICENSES.md` 增补 Unihan + terra 条目

## 引擎层

### Mode 枚举（composer.rs:3）

```rust
pub enum Mode {
    #[default]
    Pinyin,      // 简体中文（现状）
    Traditional, // 新增：繁体中文
    English,
    Number,
    Symbol,
}
```

### Engine 双词典（engine.rs）

- `Engine` 增加第二词典字段 `trad_dict: Option<Box<dyn Dictionary>>`（Option：trad.opid 加载失败时简体模式不受影响）
- 新构造 `Engine::with_dictionaries(pinyin: Box<dyn Dictionary>, trad: Option<Box<dyn Dictionary>>)`
- 现有 `Engine::with_dictionary` 保持（trad=None，JVM 测试向后兼容）
- candidates 查询路由：`candidates()` 内按 `mode == Traditional` 选择查询 `trad_dict`（None 时回退 pinyin dict），其余逻辑（合并、learner 调整、翻页）不变

### 模式切换语义（对齐现有 switchMode）

- `switch_mode(Traditional)` 与切 English 同规则：清残留拼音（不提交）、重置候选页码
- shift 禁用：Traditional 模式 shiftVisible=false（同 Pinyin）
- 空格/回车/退格行为与 Pinyin 一致

## UI 层（Android Compose）

### ImeScreen 模式键（ImeScreen.kt:104）

- `modeLabel` 三态：`Pinyin → "中"`、`Traditional → "繁"`、`English → "英"`
- 模式键循环：`Pinyin → Traditional → English → Pinyin`（tap 一次前进一格）
- 切换时清残留拼音（沿用现有 :33 逻辑，扩展到 Traditional）

### EngineLoader 双资产（EngineLoader.kt）

- 新增 `ASSET_NAME_TRAD = "trad.opid"`、`FILE_NAME_TRAD = "trad.opid"`
- 加载顺序：luna 失败不影响 trad 加载；trad 失败仅繁体模式退化（繁体模式查 luna 简体词，单字缺失），logcat 告警
- app/src/main/assets/ 添加 trad.opid

## 错误处理

| 场景 | 行为 |
|---|---|
| trad.opid 缺失/损坏 | 加载返回 Err，trad_dict=None；繁体模式回退查询简体库，日志告警 |
| Unihan/terra 数据源获取失败（离线构建） | 阻断编译，opid 不生成（提交产物已入库，CI 用仓库内产物） |
| 同 pinyin 超 255 字节 | parse_dict 现有规则跳过（词组 pinyin 连写，最长常见词 <30 字节，无实际影响） |

## 测试

- **单字全覆盖门禁**：Rust 集成测试（crates/opi-tools/tests/trad_coverage.rs）：GB2312 6763 字逐一 `query(pinyin)` 断言有候选（期待表生成自 trad_hanzi.tsv）
- 繁体单字抽查：發(fa)、愛(ai)、臺灣(tai wan)、中華民國(zhong hua min guo)
- 模式切换：Pinyin↔Traditional 清残留拼音、候选路由切换、shift 禁用
- 引擎回归：现有 engine-core/opi-ffi 测试全绿
- Android 模拟器验收：模式键 中→繁→英 循环；繁模式下 fa→發/髮；简体模式 fa→发；luna 词组简体不受影响
- JVM 测试：EngineLoader 双加载（trad 缺失回退路径）

## 验收标准

1. 简/繁模式键三态循环，标签 中/繁/英 正确
2. GB2312 6763 字逐一拼音可打（门禁测试通过）
3. 繁体模式：fa → 發 靠前（含 髮）；zhong hua min guo → 中華民國
4. 简体模式：fa → 发 靠前；现有 nihao→暖 候选顺序不变
5. 双词库回归：现有候选排序、翻页、learner 学习不回归（全量测试绿）
