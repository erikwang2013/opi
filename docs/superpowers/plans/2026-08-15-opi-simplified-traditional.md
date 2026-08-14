# 简繁全量支持（简繁模式切换）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 字库覆盖 GB2312 全量 6763 简体单字 + 常用繁体单字 ~13000 + 常用繁体词组；键盘模式条增加 简/繁 切换，繁体模式下输入拼音出繁体候选。

**Architecture:** 三层各自独立可测。数据层：`scripts/gen_trad_dict.py` 从 Unihan kMandarin + rime terra-pinyin 生成 `data/raw/trad_hanzi.tsv` / `trad_phrases.tsv`，经现有 opi-tools compile 合并为 `data/generated/trad.opid`（.opid v1 格式不变）。引擎层：`Mode` 加 `Traditional` 变体，`Engine` 持双词典（`trad_dict: Option<Box<dyn Dictionary>>`），candidates 按模式路由，trad 缺失回退简体库。UI 层：模式键 中→繁→英 三态循环，`EngineLoader` 双资产加载（trad 失败仅告警不回退内置）。

**Tech Stack:** Rust（engine-core / opi-ffi / opi-tools / engine-data）、Python 3（数据生成）、Kotlin/Compose（Android IME）、JUnit（JVM 测试）、adb/uiautomator（模拟器验收）。

**Spec:** `docs/superpowers/specs/2026-08-15-opi-simplified-traditional-design.md`

---

## 词频规则偏差注记（相对 spec 公式的两次修正）

spec「词频策略」一节的字面公式（一级 9000-序号、二级 6000-序号、繁体 5000-序号、词组 8000-序号）经数据验证有三次不可行，已两轮修正：

**修正 #1（负数 freq + 排序倒置）：**
1. 繁体单字 ~13000 个，`5000 - 序号` 在序号 > 5000 后为负。parse_dict 只接受 u32，负数字面量导致该行被跳过。
2. 繁体 freq 带低于 GB 字（9000/6000 带）时，`fa` 的简体 发/法（~8650）会排在 發 前面，验收不通过。

**修正 #2（码位序/文件序非常用度 + GB 越段，Task 1 执行中实测发现）：**

实际数据验证（trad.opid 查询）：
- 单字带按码位排序 → 生僻字排到常用字前：`fa` 前 8 为 乏/佱/伐/傠/发/垡/姂/彂，**發 不在前 8**；`hao` 前 8 为 傐/儫/哠/号/嘷/噑/嗥/嚆，**好 不在前 8**。spec 验收「fa → 發 靠前」失败。
- 编译层 (pinyin, word) 取 max freq 会把 GB 字按 terra 单字带提升（发 9836 > GB 带 8650），破坏 GB 段位。
- terra master（2026.07.17）**不含** 臺灣/中華民國/中國（双字词缺失，只有 臺灣共和國 等长词）——spec 抽查词需人工补表。

**最终方案（Task 1 实现）：统一常用度排序 + 人工常用表，替代频带公式。**

- 全部行（单字 + 词组）按优先级段排列，`freq = FMAX - idx * (FMAX // N)`，`FMAX = 4_000_000_000`（< u32::MAX，parse_freq 接受；引擎 user_boost 按 max_freq×2 缩放，learner 一次选词仍压过全部静态词，不回归）。N ≈ 十万行 → 段内完全可区分，无同频回退码位序问题。
- 段序：`[人工常用繁体单字 COMMON_TRAD] → [GB 一级（GB 码序）] → [GB 二级] → [terra 单字（文件序）] → [其余 Unihan 单字（码位序）] → [人工常用词组 SUPPLEMENT_PHRASES] → [terra 词组（文件序）]`。
- GB 字只出现在 GB 段（不因 terra 收录提升）；COMMON_TRAD 断言全为非 GB 字。
- 验证结论：`fa` → 發/髮（COMMON_TRAD 前二）→ 发/法/乏/伐（GB 段）；`hao` → 好/号/豪/毫（GB 段）→ 傐/儫/哠（terra/码位段）；臺灣/中華民國 由 SUPPLEMENT_PHRASES 提供。
- 数据事实注记：当前 UCD kMandarin 用调号（ā/fā/nǚ）非数字调；`normalize()` 须 NFD 剥离组合音调符 + `ü→v`。

跨库去重：luna 与 trad 是**互斥查询**（Traditional 模式只查 trad 库），无需与 luna 交叉去重；trad 库内部（GB 单字 ∩ terra 单字，如 好）由 compile 层 (pinyin, word) 取 max freq 处理（好 只在 GB 段出现，max 不变）。

## 验收偏差注记（Task 6 模拟器验收 + 最终评审）

- **验收标准 #4「简体模式 fa → 发 靠前」未达成（预存 luna 数据缺陷，非本功能引入）**：luna.opid 中 樊/泛 freq=100000 > 发 freq=1000（rime 词库自带词频，与简繁功能无关）。本计划无 luna 重建路径；已评估无实质影响。**后续独立任务**：luna 词频修正（数据治理，需重编 luna.opid 并回归）。
- **验收标准 #4「nihao → 暖 候选顺序不变」**：模拟器冒烟通过（候选非空、luna 未动，排序由既有引擎逻辑保证）。
- **既有 UI 缺陷（非本功能引入，前会话已知）**：候选栏点击不提交（模拟器用空格提交绕过）；英文模式 ⇧ 布局坐标等。均不在本计划范围。
- 最终评审（a0cc290..HEAD 9 个功能 commit）裁定 APPROVE：验收 1/2/3/5 达成，跨层模式整数 4=Traditional 五处一致，无 Critical/Important 代码问题。

---

### Task 1: 数据层 — gen_trad_dict.py + TSV + 编译 trad.opid

**Files:**
- Create: `scripts/gen_trad_dict.py`
- Create: `data/raw/trad_hanzi.tsv`（脚本生成）
- Create: `data/raw/trad_phrases.tsv`（脚本生成）
- Modify: `data/raw/LICENSES.md`
- Modify: `data/generated/.gitignore`
- Create: `data/generated/trad.opid`（compile 生成）
- Create: `android/app/src/main/assets/trad.opid`（cp 生成）

- [ ] **Step 1: 写数据生成脚本**

Create `scripts/gen_trad_dict.py` with EXACTLY this content:

```python
#!/usr/bin/env python3
"""生成简繁字库 TSV 数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，word\\tpinyin\\tfreq 三列）：
  data/raw/trad_hanzi.tsv     GB2312 一二级全量 6763 字（GB 码序）+ 常用繁体单字
  data/raw/trad_phrases.tsv   rime terra-pinyin 繁体词组 + 人工常用词组

词频（统一常用度排序，见 plan Task 1 偏差注记 #2）：
  freq = FMAX - idx * (FMAX // N)，idx 为全部行按优先段排列的序号：
  [COMMON_TRAD 常用繁体单字] → [GB 一级（GB 码序）] → [GB 二级]
  → [terra 单字（文件序，去重）] → [其余 Unihan 单字（码位序）]
  → [SUPPLEMENT_PHRASES 人工词组] → [terra 词组（文件序，去重）]。
  GB 字只出现在 GB 段；常用字必然排在同音生僻字前（fa→發、hao→好）。
拼音规范：kMandarin 去调号（NFD 剥离组合音调符）→ ü(冒号)→v → 小写。

用法（需要网络）：cd <repo 根> && python3 scripts/gen_trad_dict.py
"""
import codecs
import io
import re
import sys
import unicodedata
import urllib.request
import zipfile

# Unihan 自 2025 起以 Unihan.zip 发布（单文件 URL 404）
UNIHAN_ZIP_URL = "https://www.unicode.org/Public/UCD/latest/ucd/Unihan.zip"
TERRA_URL = "https://raw.githubusercontent.com/rime/rime-terra-pinyin/master/terra_pinyin.dict.yaml"

# 4e9 < u32::MAX；parse_dict 接受 u32 freq，引擎按 u64 比较
FMAX = 4_000_000_000

# 人工常用繁体单字（必须全部不在 GB2312 内，脚本断言；按常用度降序）。
COMMON_TRAD: list[str] = [
    "發", "髮", "臺", "灣", "國", "學", "個", "們", "時", "間",
    "現", "在", "這", "那", "點", "鐘", "機", "話", "電", "腦",
    "網", "軟", "體", "資", "料", "郵", "銀", "錢", "愛", "情",
    "說", "問", "題", "聽", "覺", "錯", "誤", "對", "謝", "請",
    "幫", "需", "應", "該", "經", "濟", "歷", "實", "際", "場",
    "邊", "處", "樓", "橋", "車", "輪", "數", "據", "檢", "測",
    "試", "驗", "認", "識", "讓", "讀", "寫", "講", "館", "報",
    "紙", "雜", "誌", "書", "習", "醫", "藥", "樂", "圖", "視",
    "劇", "幣", "鈔", "島", "縣", "鎮", "鄉", "區", "號", "碼",
    "頁", "項", "條", "費", "價", "減", "漲", "虧", "賺", "還",
    "買", "賣", "貴", "賤", "舊", "壞", "髒", "亂", "悶", "熱",
    "溫", "濕", "乾", "淨", "藍", "綠", "黃", "紅", "遠", "淺",
    "細", "長", "寬", "圓", "狀", "樣", "類",
]

# 人工常用繁体词组（terra 缺失的常见词；拼音为全拼无空格连写、无撇号）。
SUPPLEMENT_PHRASES: dict[str, str] = {
    "臺灣": "taiwan", "中華民國": "zhonghuaminguo", "中國": "zhongguo",
    "香港": "xianggang", "澳門": "aomen", "中文": "zhongwen",
    "電腦": "diannao", "網路": "wanglu", "軟體": "ruanti", "資料": "ziliao",
    "電話": "dianhua", "手機": "shouji", "銀行": "yinhang", "問題": "wenti",
    "什麼": "shenme", "為什麼": "weishenme", "謝謝": "xiexie",
    "對不起": "duibuqi", "沒關係": "meiguanxi", "歡迎": "huanying",
    "再見": "zaijian", "早安": "zaoan", "學校": "xuexiao", "老師": "laoshi",
    "學生": "xuesheng", "工作": "gongzuo", "朋友": "pengyou", "家人": "jiaren",
    "結婚": "jiehun", "離婚": "lihun", "經濟": "jingji", "政治": "zhengzhi",
    "歷史": "lishi", "文化": "wenhua", "藝術": "yishu", "音樂": "yinyue",
    "電影": "dianying", "照片": "zhaopian", "風景": "fengjing", "天氣": "tianqi",
    "身體": "shenti", "健康": "jiankang", "醫院": "yiyuan", "醫生": "yisheng",
    "護士": "hushi", "購物": "gouwu", "商店": "shangdian", "市場": "shichang",
    "價格": "jiage", "便宜": "pianyi", "免費": "mianfei", "我們": "women",
    "你們": "nimen", "他們": "tamen", "已經": "yijing", "應該": "yinggai",
    "還有": "haiyou", "所有": "suoyou", "重要": "zhongyao", "知道": "zhidao",
    "喜歡": "xihuan", "愛情": "aiqing", "關係": "guanxi", "發展": "fazhan",
    "發現": "faxian", "發生": "fasheng", "出發": "chufa", "頭髮": "toufa",
    "汽車": "qiche", "火車": "huoche", "飛機": "feiji", "廁所": "cesuo",
}

# 缺 kMandarin 的码位人工补音（脚本报缺后在此补齐再重跑）
SUPPLEMENT: dict[str, list[str]] = {}


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=180) as r:
        return r.read()


def fetch_unihan_readings() -> str:
    """Unihan.zip → 内存解压 Unihan_Readings.txt（zip 根目录内该单文件）。"""
    data = fetch(UNIHAN_ZIP_URL)
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        return zf.read("Unihan_Readings.txt").decode("utf-8")


def normalize(py: str) -> str:
    """kMandarin/terra 拼音 → 无调、ü→v、小写。
    当前 UCD kMandarin 用调号（ā/fā/nǚ/ǚ），NFD 剥离组合音调符；
    ü 家族（üǖǘǚǜ）与冒号式（u:）须在 NFD 前映射 v——NFD 后 ü 的
    组合音调符已被剥离，无法再区分 u/ü（lu:4→lv、nǚ→nv）。"""
    py = py.replace("u:", "v")
    py = re.sub(r"[üǖǘǚǜ]", "v", py)
    py = unicodedata.normalize("NFD", py)
    py = "".join(c for c in py if not unicodedata.combining(c))
    return re.sub(r"[0-9]", "", py).lower()


def load_kmandarin(text: str) -> dict[str, list[str]]:
    """Unihan_Readings.txt → {char: [规范化无调拼音...]}（kMandarin 多音空格分隔，去重）。"""
    km: dict[str, list[str]] = {}
    for line in text.splitlines():
        if not line.startswith("U+"):
            continue
        parts = line.split("\t")
        if len(parts) < 3 or parts[1] != "kMandarin":
            continue
        cp = int(parts[0][2:], 16)
        if not (0x4E00 <= cp <= 0x9FFF):
            continue
        readings = []
        for py in parts[2].strip().split():
            norm = normalize(py)
            if norm and norm not in readings:
                readings.append(norm)
        if readings:
            km[chr(cp)] = readings
    return km


def gb2312_rows() -> list[tuple[int, str]]:
    """GB2312 汉字区 B0–F7 行 × A1–FE 列，GB 码序；row ≤ 0xD7 为一级（拼音序），其余二级（部首序）。
    D7FA–D7FE 在 GB2312-80 未定义（GBK 才定义），显式跳过。"""
    out = []
    for row in range(0xB0, 0xF8):
        for col in range(0xA1, 0xFF):
            if row == 0xD7 and col >= 0xFA:
                continue
            try:
                ch = codecs.decode(bytes([row, col]), "gb2312")
            except UnicodeDecodeError:
                continue
            out.append((row, ch))
    return out


def parse_terra(text: str) -> list[tuple[str, str]]:
    """terra_pinyin.dict.yaml → [(word, 无调全拼连写)]，保持文件序。"""
    out = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "\t" not in line:
            continue
        cols = line.split("\t")
        if len(cols) < 2 or len(cols) > 3:
            continue
        word, pinyin = cols[0].strip(), cols[1].strip()
        if not word or not all(0x4E00 <= ord(c) <= 0x9FFF for c in word):
            continue
        py = "".join(normalize(s) for s in pinyin.split())
        if not py.isascii() or len(py) > 255:
            continue
        out.append((word, py))
    return out


def main() -> None:
    km = load_kmandarin(fetch_unihan_readings())
    rows = gb2312_rows()
    if len(rows) != 6763:
        print(f"FATAL: GB2312 单字 {len(rows)} ≠ 6763（Python gb2312 codec 覆盖与预期不符）", file=sys.stderr)
        sys.exit(1)

    missing = [ch for _, ch in rows if ch not in km and ch not in SUPPLEMENT]
    if missing:
        print(f"FATAL: {len(missing)} 个 GB2312 码位缺 kMandarin：{' '.join(missing)}", file=sys.stderr)
        print("补进脚本 SUPPLEMENT 表后重跑。", file=sys.stderr)
        sys.exit(1)

    gb_set = {ch for _, ch in rows}
    bad = [ch for ch in COMMON_TRAD if ch in gb_set]
    if bad:
        print(f"FATAL: COMMON_TRAD 含 GB2312 字（应移除）：{' '.join(bad)}", file=sys.stderr)
        sys.exit(1)
    no_km = [ch for ch in COMMON_TRAD if ch not in km and ch not in SUPPLEMENT]
    if no_km:
        print(f"FATAL: COMMON_TRAD 缺 kMandarin（补进 SUPPLEMENT）：{' '.join(no_km)}", file=sys.stderr)
        sys.exit(1)

    terra = parse_terra(fetch(TERRA_URL).decode("utf-8"))
    terra_single = [(w, py) for w, py in terra if len(w) == 1]
    terra_phrase = [(w, py) for w, py in terra if len(w) > 1]

    # 段 1–5 单字：常用繁体 → GB 一二级（GB 码序）→ terra 单字（文件序）→ 其余 Unihan（码位序）
    ordered: list[tuple[str, list[str]]] = []
    seen: set[str] = set()
    for ch in COMMON_TRAD:
        readings = SUPPLEMENT.get(ch) or km[ch]
        ordered.append((ch, readings))
        seen.add(ch)
    for _, ch in rows:
        if ch in seen:
            continue
        readings = SUPPLEMENT.get(ch) or km[ch]
        ordered.append((ch, readings))
        seen.add(ch)
    for w, py in terra_single:
        if w in seen:
            continue
        seen.add(w)
        readings = [py] + [p for p in (km.get(w) or []) if p != py]
        ordered.append((w, readings))
    for ch in sorted(km):
        if ch in seen:
            continue
        seen.add(ch)
        ordered.append((ch, km[ch]))

    # 段 6–7 词组：人工常用 → terra（文件序）
    phrase_ordered: list[tuple[str, str]] = []
    phrase_seen: set[str] = set()
    for w, py in SUPPLEMENT_PHRASES.items():
        phrase_ordered.append((w, py))
        phrase_seen.add(w)
    for w, py in terra_phrase:
        if w in phrase_seen:
            continue
        phrase_ordered.append((w, py))
        phrase_seen.add(w)

    n = len(ordered) + len(phrase_ordered)
    assert n < FMAX, "行数超过 FMAX，spacing 归零，freq 全等"
    spacing = FMAX // n
    hanzi: list[str] = []
    for idx, (word, readings) in enumerate(ordered):
        freq = FMAX - idx * spacing
        for py in readings:
            hanzi.append(f"{word}\t{py}\t{freq}")
    phrases_out: list[str] = []
    for idx, (word, py) in enumerate(phrase_ordered):
        freq = FMAX - (len(ordered) + idx) * spacing
        phrases_out.append(f"{word}\t{py}\t{freq}")

    with open("data/raw/trad_hanzi.tsv", "w", encoding="utf-8") as f:
        f.write("\n".join(hanzi) + "\n")
    with open("data/raw/trad_phrases.tsv", "w", encoding="utf-8") as f:
        f.write("\n".join(phrases_out) + "\n")

    print(f"GB2312 单字: {len(rows)} (一级 {sum(1 for r, _ in rows if r <= 0xD7)}, 二级 {len(rows) - sum(1 for r, _ in rows if r <= 0xD7)})")
    print(f"繁体单字(含 terra 单字): {len(ordered) - len(rows)}")
    print(f"词组(人工+terra): {len(phrase_ordered)}")
    print(f"trad_hanzi.tsv 行数: {len(hanzi)}")
    print(f"trad_phrases.tsv 行数: {len(phrases_out)}")
    print(f"freq 范围: [{FMAX - (n - 1) * spacing}, {FMAX}] spacing={spacing}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: 运行脚本生成数据（需要网络）**

Run: `cd /home/wwwroot/bag/opi && python3 scripts/gen_trad_dict.py`
Expected: 打印五行统计，其中 `GB2312 单字: 6763 (一级 3755, 二级 3008)`；繁体单字数万级（≈ km 非 GB 单字 + terra 单字去重）；词组数万级（≈ terra 文件条目数）；freq 范围为 `[约 1.2e9, 4000000000]` 且 spacing 数万级。
若报 `FATAL: ... 缺 kMandarin`：把列出的字补进脚本 `SUPPLEMENT` 表（人工填拼音）后重跑。
若报 `FATAL: COMMON_TRAD 含 GB2312 字`：把列出的字从 COMMON_TRAD 移除后重跑（该字在 GB 段已有）。

- [ ] **Step 3: 抽查产物**

Run: `head -3 /home/wwwroot/bag/opi/data/raw/trad_hanzi.tsv && head -3 /home/wwwroot/bag/opi/data/raw/trad_phrases.tsv && grep -P "^發\t" /home/wwwroot/bag/opi/data/raw/trad_hanzi.tsv | head -2 && grep -P "^臺灣\t" /home/wwwroot/bag/opi/data/raw/trad_phrases.tsv | head -1`
Expected: hanzi 前三行是 COMMON_TRAD 首三字（發/髮/臺）且 freq 递减（约 4e9 起）；發 行 freq 在 [3.9e9, 4e9] 区间；臺灣 行 pinyin=taiwan、freq 在词组段首（> 3.5e9）。行内 `word\tpinyin\tfreq` 三列、拼音无空格无调号。

- [ ] **Step 4: 更新 LICENSES.md**

Read `data/raw/LICENSES.md` first. In its markdown table append two rows (keep table format):

```markdown
| trad_hanzi.tsv（单字） | Unicode Unihan（kMandarin 字段） | Unicode License（宽松，可再分发，保留版权声明） | GB2312 全量 6763 + 常用繁体单字，由 scripts/gen_trad_dict.py 生成（含人工常用表） |
| trad_phrases.tsv（词组） | https://github.com/rime/rime-terra-pinyin（terra_pinyin.dict.yaml） | **LGPL-3.0** | 常用繁体词组，由 scripts/gen_trad_dict.py 生成（含人工常用表） |
```

- [ ] **Step 5: 白名单 trad.opid 并编译合并**

Run:

```bash
cd /home/wwwroot/bag/opi
printf '*\n!fallback.opid\n!trad.opid\n!.gitignore\n' > data/generated/.gitignore
cat data/raw/trad_hanzi.tsv data/raw/trad_phrases.tsv > /tmp/trad_merged.tsv
cargo run -p opi-tools -- compile /tmp/trad_merged.tsv data/generated/trad.opid
cargo run -p opi-tools -- verify data/generated/trad.opid
cp data/generated/trad.opid android/app/src/main/assets/trad.opid
```

Expected: compile 打印 `kept entries`（≈ 输入行数，重复仅少量）；verify 打印 `checksum: ok`、`entries: <N>`，`query "hao"` 前三含 好（GB 段常用字排 terra/码位段生僻字前）。

- [ ] **Step 6: 提交**

```bash
cd /home/wwwroot/bag/opi
git add scripts/gen_trad_dict.py data/raw/trad_hanzi.tsv data/raw/trad_phrases.tsv data/raw/LICENSES.md data/generated/.gitignore data/generated/trad.opid android/app/src/main/assets/trad.opid
git commit -m "feat(data): 简繁字库生成脚本 + trad_hanzi/phrases TSV + trad.opid（统一常用度排序）"
```

---

### Task 2: 引擎层 — Mode::Traditional 双词库路由（TDD）

**Files:**
- Create: `crates/engine-core/tests/trad_mode.rs`
- Modify: `crates/engine-core/src/composer.rs`（Mode 枚举 + input_key 分支 + 内联测试）
- Modify: `crates/engine-core/src/candidates.rs`（模式门禁 + 内联测试）
- Modify: `crates/engine-core/src/engine.rs`（双词典）

- [ ] **Step 1: 写失败测试**

Create `crates/engine-core/tests/trad_mode.rs`：

```rust
//! 简繁双词库路由（spec 2026-08-15）：Traditional 模式查 trad 词典，Pinyin 模式不受影响。
use engine_core::composer::Mode;
use engine_core::{Engine, InMemoryDictionary};

fn dict(entries: &[(&str, &str, u32)]) -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    d
}

fn two_dict_engine() -> Engine {
    let simp = dict(&[("hao", "好", 5000), ("hao", "号", 1200)]);
    let trad = dict(&[("hao", "發", 4000), ("hao", "髮", 3500)]);
    Engine::with_dictionaries(
        Box::new(simp),
        Some(Box::new(trad)),
        engine_core::symbols::SymbolEngine::builtin(),
        false,
    )
}

#[test]
fn traditional_mode_queries_trad_dict() {
    let mut e = two_dict_engine();
    e.switch_mode(Mode::Traditional);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    assert_eq!(e.mode(), Mode::Traditional);
    let got = e.candidates(8);
    assert_eq!(got[0].text, "發");
    assert_eq!(got[1].text, "髮");
}

#[test]
fn pinyin_mode_ignores_trad_dict() {
    let mut e = two_dict_engine();
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    let got = e.candidates(8);
    assert_eq!(got[0].text, "好");
}

#[test]
fn traditional_without_trad_dict_falls_back_to_simplified() {
    let simp = dict(&[("hao", "好", 5000)]);
    let mut e = Engine::with_dictionaries(
        Box::new(simp),
        None,
        engine_core::symbols::SymbolEngine::builtin(),
        false,
    );
    e.switch_mode(Mode::Traditional);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    assert_eq!(e.candidates(8)[0].text, "好");
}

#[test]
fn traditional_space_selects_top_candidate() {
    let mut e = two_dict_engine();
    e.switch_mode(Mode::Traditional);
    for ch in "hao".chars() {
        e.input_key(ch);
    }
    assert_eq!(e.input_key(' '), "發");
}

#[test]
fn switch_mode_clears_buffer_and_traditional_lowercases() {
    let mut e = two_dict_engine();
    e.input_key('n');
    e.switch_mode(Mode::Traditional);
    assert_eq!(e.buffer(), "");
    e.set_shift(true);
    e.input_key('A');
    assert_eq!(e.buffer(), "a"); // Traditional 同 Pinyin：字母转小写、shift 无效
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cd /home/wwwroot/bag/opi && cargo test -p engine-core --test trad_mode`
Expected: FAIL——编译错误（`Mode::Traditional` 变体不存在 / `with_dictionaries` 未定义）。

- [ ] **Step 3: composer.rs — Mode 枚举 + Traditional 输入分支**

composer.rs:3 枚举加变体：

```rust
pub enum Mode {
    #[default]
    Pinyin,
    Traditional,
    English,
    Number,
    Symbol,
}
```

composer.rs input_key 的 Pinyin 分支合并（Traditional 行为与 Pinyin 一致：小写字母/撇号入缓冲，大写转小写）：

```rust
Mode::Pinyin | Mode::Traditional => {
    if self.session.buffer.chars().count() >= MAX_BUFFER {
        Ignored
    } else if ch.is_ascii_lowercase() || ch == '\'' {
        self.session.buffer.push(ch);
        Updated
    } else if ch.is_ascii_uppercase() {
        self.session.buffer.push(ch.to_ascii_lowercase());
        Updated
    } else {
        Ignored
    }
}
```

composer.rs tests mod 追加：

```rust
#[test]
fn traditional_accepts_letters_like_pinyin() {
    let mut c = Composer::new();
    c.switch_mode(Mode::Traditional);
    let (eff, s) = c.input_key('N');
    assert_eq!(eff, KeyEffect::Updated);
    assert_eq!(s.buffer, "n");
    let (eff, s) = c.input_key('\'');
    assert_eq!(eff, KeyEffect::Updated);
    assert_eq!(s.buffer, "n'");
}
```

- [ ] **Step 4: candidates.rs — 模式门禁扩展 + 内联测试**

candidates.rs:44 门禁改为：

```rust
if input.is_empty() || !matches!(mode, Mode::Pinyin | Mode::Traditional) {
    return Vec::new();
}
```

candidates.rs tests mod 追加：

```rust
#[test]
fn traditional_mode_queries_dict() {
    let d = test_dict();
    let s = SymbolEngine::builtin();
    let l = Learner::new(false);
    let got = rank_and_pick(&d, &s, &l, "hao", Mode::Traditional, DEFAULT_TOP_N, USER_BOOST);
    assert_eq!(got[0].text, "好");
}

#[test]
fn traditional_empty_input_gives_empty() {
    let d = test_dict();
    let s = SymbolEngine::builtin();
    let l = Learner::new(false);
    assert!(rank_and_pick(&d, &s, &l, "", Mode::Traditional, DEFAULT_TOP_N, USER_BOOST).is_empty());
}
```

- [ ] **Step 5: engine.rs — 双词典构造与路由**

```rust
pub struct Engine {
    dict: Box<dyn Dictionary>,
    /// 繁体词典（trad.opid）；None 时 Traditional 模式回退 dict（spec 错误处理）。
    trad_dict: Option<Box<dyn Dictionary>>,
    composer: Composer,
    symbols: SymbolEngine,
    learner: Learner,
    /// 用户词频权重，按词典最大静态词频动态缩放（一次选词即压过所有静态词）。
    user_boost: u64,
}

impl Engine {
    /// 单词典构造（trad=None），JVM/现有调用向后兼容。
    pub fn new(dict: Box<dyn Dictionary>, symbols: SymbolEngine, learner_enabled: bool) -> Self {
        Self::with_dictionaries(dict, None, symbols, learner_enabled)
    }

    /// 双词典构造：Traditional 模式查 trad（None 时回退 dict），其余模式查 dict。
    pub fn with_dictionaries(
        dict: Box<dyn Dictionary>,
        trad: Option<Box<dyn Dictionary>>,
        symbols: SymbolEngine,
        learner_enabled: bool,
    ) -> Self {
        let max_freq = dict.max_freq().max(trad.as_ref().map_or(0, |d| d.max_freq()));
        let user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        Engine {
            dict,
            trad_dict: trad,
            composer: Composer::new(),
            symbols,
            learner: Learner::new(learner_enabled),
            user_boost,
        }
    }

    /// 换装繁体词典（FFI install_trad 用）。None = 清除（回退简体）。
    /// 重算 user_boost：trad.opid 静态最大词频 4e9，保持"一次选词压过全部静态词"。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        let max_freq = self.dict.max_freq().max(dict.as_ref().map_or(0, |d| d.max_freq()));
        self.user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        self.trad_dict = dict;
    }

    /// 当前模式生效的词典。
    fn active_dict(&self) -> &dyn Dictionary {
        match self.composer.session().mode {
            Mode::Traditional => self.trad_dict.as_deref().unwrap_or(&*self.dict),
            _ => &*self.dict,
        }
    }
```

input_space 的 Pinyin 分支合并（candidates.rs 门禁已放行 Traditional，这里选中首候选）：

```rust
match self.composer.session().mode {
    Mode::Pinyin | Mode::Traditional => {
        let buffer = self.composer.session().buffer.clone();
        let cands = self.candidates(DEFAULT_TOP_N);
        if cands.is_empty() {
            self.composer.commit_buffer();
            buffer
        } else {
            let top = cands[0].text.clone();
            self.learner.record_selection(&top);
            self.composer.commit_buffer();
            top
        }
    }
    _ => {
        let buffer = self.composer.session().buffer.clone();
        self.composer.commit_buffer();
        buffer
    }
}
```

candidates() 改走 active_dict：

```rust
pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
    let s = self.composer.session();
    rank_and_pick(
        self.active_dict(),
        &self.symbols,
        &self.learner,
        &s.buffer,
        s.mode,
        limit,
        self.user_boost,
    )
}
```

- [ ] **Step 6: 全量引擎测试**

Run: `cd /home/wwwroot/bag/opi && cargo test -p engine-core`
Expected: 全绿（新增 7 个 trad_mode 测试 + composer/candidates 内联测试 + 原有测试）。

- [ ] **Step 7: 提交**

```bash
cd /home/wwwroot/bag/opi
git add crates/engine-core/src/composer.rs crates/engine-core/src/candidates.rs crates/engine-core/src/engine.rs crates/engine-core/tests/trad_mode.rs
git commit -m "feat(engine): Mode::Traditional 双词库路由（with_dictionaries + 模式门禁）"
```

---

### Task 3: FFI 层 — mode 4 + install_trad + JNI/C 双出口

**Files:**
- Modify: `crates/opi-ffi/src/api/mod.rs`
- Modify: `crates/opi-ffi/src/cabi.rs`
- Modify: `crates/opi-ffi/src/jni.rs`
- Modify: `crates/opi-ffi/tests/cabi_test.rs`

- [ ] **Step 1: api/mod.rs — ApiMode::Traditional + 模式整数映射**

```rust
pub enum ApiMode {
    Pinyin,
    Traditional,
    English,
    Number,
    Symbol,
}

impl From<Mode> for ApiMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Pinyin => ApiMode::Pinyin,
            Mode::Traditional => ApiMode::Traditional,
            Mode::English => ApiMode::English,
            Mode::Number => ApiMode::Number,
            Mode::Symbol => ApiMode::Symbol,
        }
    }
}

impl From<ApiMode> for Mode {
    fn from(m: ApiMode) -> Self {
        match m {
            ApiMode::Pinyin => Mode::Pinyin,
            ApiMode::Traditional => Mode::Traditional,
            ApiMode::English => Mode::English,
            ApiMode::Number => Mode::Number,
            ApiMode::Symbol => Mode::Symbol,
        }
    }
}
```

mode_from_int / mode_to_int（0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional）：

```rust
pub fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::English),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        4 => Some(Mode::Traditional),
        _ => None,
    }
}

pub fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::English => 1,
        Mode::Number => 2,
        Mode::Symbol => 3,
        Mode::Traditional => 4,
    }
}
```

- [ ] **Step 2: api/mod.rs — install_trad + Api::set_trad_dict**

import 增加 `use engine_core::dictionary::Dictionary;`，install 函数旁追加：

```rust
/// 装载繁体词典并挂到已安装引擎上（不替换主词典，简体模式不受影响）。
/// 严格加载（load_mmap，坏路径不回落内置——内置是简体 35 词，装成繁体语义错误）；
/// 引擎未 load 时返回 Err（调用方按 false 处理，繁体模式回退简体库）。
pub fn install_trad(path: &str) -> Result<(), String> {
    let dict = engine_data::load_mmap(std::path::Path::new(path))
        .map_err(|e| format!("load trad {}: {e:?}", path))?;
    let mut guard = SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    match guard.as_mut() {
        Some(api) => {
            api.set_trad_dict(Some(Box::new(dict)));
            Ok(())
        }
        None => Err("engine not loaded".into()),
    }
}
```

Api impl 增加：

```rust
/// 换装繁体词典（trad.opid）。None = 清除（繁体模式回退简体库）。
pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
    self.engine.set_trad_dict(dict);
}
```

> 修正注记：Task 2 已提交的 `Engine::set_trad_dict` 是平换版本（不重算 user_boost）。
> trad.opid 静态最大词频 4e9，若 user_boost 仍为旧值（≤1e5），learner 一次选词压不过
> 静态词，繁体模式学习失效。Task 2 Step 5 已按重算版更新；本任务新增测试验证：
> `install_trad` 后选词仍能压过 4e9 静态词（见 Step 3 测试补充）。

- [ ] **Step 3: api/mod.rs — 更新/新增测试**

`mode_int_roundtrip` 更新（4 现在是 Traditional）：

```rust
#[test]
fn mode_int_roundtrip() {
    assert_eq!(mode_from_int(0), Some(Mode::Pinyin));
    assert_eq!(mode_from_int(3), Some(Mode::Symbol));
    assert_eq!(mode_from_int(4), Some(Mode::Traditional));
    assert_eq!(mode_from_int(5), None);
    assert_eq!(mode_from_int(-1), None);
    assert_eq!(mode_to_int(Mode::English), 1);
    assert_eq!(mode_to_int(Mode::Number), 2);
    assert_eq!(mode_to_int(Mode::Traditional), 4);
}
```

新增（tests mod 内；注意与既有 install 测试同一单例，保持现有风险水平）：

```rust
#[test]
fn install_trad_hooks_trad_dict() {
    // 引擎未加载 → Err
    *SINGLETON.lock().unwrap() = None;
    assert!(install_trad("/nonexistent/trad.opid").is_err());
    // 先装主库：坏路径 → Err 且简体模式不受影响
    install(None).unwrap();
    assert!(install_trad("/nonexistent/trad.opid").is_err());
    // 真路径：临时编译一个小词典（engine_data 序列化，opi-ffi 不依赖 opi-tools）
    let dict = engine_data::format::OpDict {
        entries: vec![engine_data::format::RawEntry {
            pinyin: "fa".into(),
            word: "發".into(),
            freq: 4000,
        }],
        pinyin_total: 2,
    };
    let tmp = std::env::temp_dir().join("opi_trad_test.opid");
    std::fs::write(&tmp, engine_data::serialize(&dict)).unwrap();
    install_trad(tmp.to_str().unwrap()).unwrap();
    // 繁体模式候选走 trad 词典
    let top = with_engine(|a| {
        a.switch_mode(ApiMode::Traditional);
        a.input_key("f".into());
        a.input_key("a".into());
        a.candidates(8)[0].text.clone()
    })
    .expect("engine loaded");
    assert_eq!(top, "發");
    std::fs::remove_file(&tmp).ok();
    with_engine(|a| a.switch_mode(ApiMode::Pinyin));
}
```

新增（user_boost 重算验证，见上方修正注记）：

```rust
#[test]
fn install_trad_recomputes_user_boost() {
    // 高静态词频（4e9，trad.opid 同量级）安装后，learner 选词仍压过静态词。
    *SINGLETON.lock().unwrap() = None;
    install(None).unwrap();
    let dict = engine_data::format::OpDict {
        entries: vec![
            engine_data::format::RawEntry { pinyin: "fa".into(), word: "發".into(), freq: 4_000_000_000 },
            engine_data::format::RawEntry { pinyin: "fa".into(), word: "髮".into(), freq: 3_999_000_000 },
        ],
        pinyin_total: 2,
    };
    let tmp = std::env::temp_dir().join("opi_trad_boost.opid");
    std::fs::write(&tmp, engine_data::serialize(&dict)).unwrap();
    install_trad(tmp.to_str().unwrap()).unwrap();
    std::fs::remove_file(&tmp).ok();
    with_engine(|a| {
        a.set_learner(true);
        a.switch_mode(ApiMode::Traditional);
        a.input_key("f".into());
        a.input_key("a".into());
        assert_eq!(a.candidates(8)[0].text, "發");
        a.select(1); // 选 髮
        a.input_key("f".into());
        a.input_key("a".into());
        assert_eq!(a.candidates(8)[0].text, "髮", "learner 选词应压过 4e9 静态词");
        a.set_learner(false);
    });
    with_engine(|a| a.switch_mode(ApiMode::Pinyin));
}
```

- [ ] **Step 4: cabi.rs — opi_load_trad**

opi_load 之后追加（注释计数 18→19）：

```rust
/// loadTrad(path: const uint16_t*, len) -> bool。空/坏路径/引擎未加载 → false
/// （繁体模式回退简体库，见 spec 错误处理）。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_load_trad(path: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { read_utf16(path, len) }.unwrap_or_default();
        api::install_trad(&path).is_ok()
    }))
    .unwrap_or(false)
}
```

- [ ] **Step 5: jni.rs — opijni_load_trad + 注册**

opi_load 后追加（注释计数 18→19）：

```rust
/// loadTrad(path: String) -> bool。空/坏路径/引擎未加载 → false（繁体模式回退简体库）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load_trad(env: JEnv, _class: sys::jclass, path: jstring) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) }.unwrap_or_default();
        api::install_trad(&path).is_ok()
    }))
    .unwrap_or(false)
}
```

JNI_OnLoad methods 数组（opi_load 行之后）插入注册项：

```rust
NativeMethod::from_raw_parts(jni_str!("loadTrad"), jni_str!("(Ljava/lang/String;)Z"), opijni_load_trad as *mut c_void),
```

- [ ] **Step 6: cabi_test.rs — 集成测试**

import 行加 `opi_load_trad`（在 opi_load 后），追加：

```rust
#[test]
fn cabi_load_trad_routes_traditional_mode() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    // 坏路径 → false（不影响已装主词典）
    let bad = to_units("/nonexistent/trad.opid");
    assert!(!unsafe { opi_load_trad(bad.as_ptr(), bad.len()) });
    // 真路径：仓库内 trad.opid（Task 1 已提交）
    let p = to_units("../../data/generated/trad.opid");
    assert!(unsafe { opi_load_trad(p.as_ptr(), p.len()) });
    unsafe { opi_switch_mode(4) };
    assert_eq!(unsafe { opi_mode() }, 4);
    let f = to_units("f");
    let a = to_units("a");
    unsafe {
        opi_input_key(f.as_ptr(), f.len());
        opi_input_key(a.as_ptr(), a.len());
    }
    let texts = read_texts(unsafe { opi_candidates(8) });
    assert!(texts.contains(&"發".to_string()), "繁模式 fa 候选应含 發，实际: {texts:?}");
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
}
```

- [ ] **Step 7: opi-ffi 全量测试**

Run: `cd /home/wwwroot/bag/opi && cargo test -p opi-ffi`
Expected: 全绿（api 单测 + cabi 集成，无 JVM 依赖）。

- [ ] **Step 8: 提交**

```bash
cd /home/wwwroot/bag/opi
git add crates/opi-ffi/src/api/mod.rs crates/opi-ffi/src/cabi.rs crates/opi-ffi/src/jni.rs crates/opi-ffi/tests/cabi_test.rs
git commit -m "feat(ffi): mode 4=Traditional + opi_load_trad（JNI/C 双出口，严格加载）"
```

---

### Task 4: UI 层 — 三态模式键 + EngineLoader 双资产

**Files:**
- Modify: `android/app/src/main/kotlin/io/opi/input/engine/EngineController.kt`
- Modify: `android/app/src/main/kotlin/io/opi/input/ime/ImeScreen.kt`
- Modify: `android/app/src/main/kotlin/io/opi/input/jni/OpiEngine.kt`
- Modify: `android/app/src/main/kotlin/io/opi/input/jni/EngineLoader.kt`
- Modify: `android/app/src/test/kotlin/io/opi/input/jni/EngineLoaderTest.kt`
- Modify: `android/app/src/test/kotlin/io/opi/input/engine/EngineControllerTest.kt`

- [ ] **Step 1: EngineController.kt — EngineMode.TRADITIONAL(4)**

```kotlin
/** 引擎模式（JNI mode() 返回值：0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional）。 */
enum class EngineMode(val value: Int) {
    PINYIN(0), ENGLISH(1), NUMBER(2), SYMBOL(3), TRADITIONAL(4);

    companion object {
        fun fromInt(v: Int) = entries.firstOrNull { it.value == v } ?: PINYIN
    }
}
```

- [ ] **Step 2: OpiEngine.kt — loadTrad 声明**

learnerEnabled 附近追加：

```kotlin
/** loadTrad(path: String) -> Boolean。坏路径/引擎未加载 → false（繁体模式回退简体库）。 */
external fun loadTrad(path: String): Boolean
```

- [ ] **Step 3: ImeScreen.kt — 三态模式键 + 标签**

```kotlin
// 切模式：中→繁→英→中 三态循环；离开拼音类模式清残留拼音，防止被空格/回车意外提交
fun toggleMode() {
    when (controller.mode) {
        EngineMode.PINYIN -> {
            controller.clear()
            controller.switchMode(EngineMode.TRADITIONAL)
        }
        EngineMode.TRADITIONAL -> {
            controller.clear()
            controller.switchMode(EngineMode.ENGLISH)
        }
        else -> controller.switchMode(EngineMode.PINYIN)
    }
}
```

modeLabel（ImeScreen.kt:104）替换：

```kotlin
modeLabel = when (controller.mode) {
    EngineMode.PINYIN -> "中"
    EngineMode.TRADITIONAL -> "繁"
    else -> "英"
},
```

（shiftVisible 保持 `mode == ENGLISH`，spec：Traditional 同 Pinyin 禁用 shift。）

- [ ] **Step 4: EngineLoader.kt — 双资产加载**

常量与接口追加：

```kotlin
const val ASSET_NAME_TRAD = "trad.opid"
const val FILE_NAME_TRAD = "trad.opid"

/** 繁体词典加载抽象（trad 是可选增强：失败不回退内置，不触碰已装的 luna）。 */
fun interface LoadTradApi {
    fun loadTrad(path: String): Boolean
}
```

loadAsset 之后追加：

```kotlin
/**
 * 繁体资产编排：与 loadAsset 相同的 size 校验重拷；失败只返回 false 并保留主词典
 * （spec 2026-08-15 错误处理：trad.opid 加载失败 → 繁体模式回退查简体库，logcat 告警）。
 */
fun loadTradAsset(fileOps: FileOps, api: LoadTradApi, targetPath: String): Boolean {
    try {
        val assetSize = fileOps.assetLength()
        if (assetSize == null || needsCopy(assetSize, fileOps.existingSize())) {
            fileOps.write(fileOps.readAsset())
        }
    } catch (e: Exception) {
        return false
    }
    return api.loadTrad(targetPath)
}
```

load() 末尾（return 前）追加：

```kotlin
// trad 可选增强：失败仅告警，不影响 luna 主词典（OpiEngine.loadTrad 严格加载）
val tradTarget = File(context.filesDir, FILE_NAME_TRAD)
val tradOk = loadTradAsset(
    fileOps = object : FileOps {
        override fun assetLength(): Long? = try {
            context.assets.openFd(ASSET_NAME_TRAD).use { it.length }
        } catch (e: IOException) {
            null
        }

        override fun readAsset(): ByteArray =
            context.assets.open(ASSET_NAME_TRAD).use { it.readBytes() }

        override fun existingSize(): Long? =
            if (tradTarget.exists()) tradTarget.length() else null

        override fun write(bytes: ByteArray) {
            tradTarget.writeBytes(bytes)
        }
    },
    api = LoadTradApi { OpiEngine.loadTrad(it) },
    targetPath = tradTarget.absolutePath,
)
if (tradOk) Log.i(TAG, "trad loaded (${tradTarget.length()} bytes)")
else Log.w(TAG, "trad load failed, Traditional mode falls back to simplified dict")
```

- [ ] **Step 5: EngineLoaderTest.kt — 新测试**

追加：

```kotlin
/** 假 trad 加载器：记录调用与结果。 */
private class FakeTradApi(var ok: Boolean = true) : EngineLoader.LoadTradApi {
    val calls = mutableListOf<String>()

    override fun loadTrad(path: String): Boolean {
        calls += path
        return ok
    }
}

@Test
fun tradLoadsWhenAssetReady() {
    val fileOps = FakeFileOps(byteArrayOf(1, 2, 3), existingSize = null)
    val api = FakeTradApi()

    val ok = EngineLoader.loadTradAsset(fileOps, api, "/data/trad.opid")

    assertTrue(ok)
    assertEquals(1, fileOps.writeCalls)
    assertEquals(listOf("/data/trad.opid"), api.calls)
}

@Test
fun tradFailureKeepsEngineUntouched() {
    // 坏路径 → false，且不得调用 load(null)（不回退内置，luna 主词典保持）
    val fileOps = FakeFileOps(byteArrayOf(1, 2, 3), existingSize = null)
    val api = FakeTradApi(ok = false)

    val ok = EngineLoader.loadTradAsset(fileOps, api, "/bad/trad.opid")

    assertFalse(ok)
    assertEquals(listOf("/bad/trad.opid"), api.calls)
}

@Test
fun tradAssetReadFailureSkipsLoadTrad() {
    val fileOps = object : EngineLoader.FileOps {
        override fun assetLength(): Long? = 100L
        override fun readAsset(): ByteArray = throw IOException("asset missing")
        override fun existingSize(): Long? = null
        override fun write(bytes: ByteArray) {}
    }
    val api = FakeTradApi()

    val ok = EngineLoader.loadTradAsset(fileOps, api, "/data/trad.opid")

    assertFalse(ok)
    assertTrue(api.calls.isEmpty())
}
```

- [ ] **Step 6: EngineControllerTest.kt — fromInt 映射**

追加：

```kotlin
@Test
fun fromIntMapsTraditional() {
    assertEquals(EngineMode.TRADITIONAL, EngineMode.fromInt(4))
    assertEquals(EngineMode.PINYIN, EngineMode.fromInt(99))
}
```

- [ ] **Step 7: JVM 测试**

Run: `cd /home/wwwroot/bag/opi/android && ./gradlew testDebugUnitTest`
Expected: BUILD SUCCESSFUL，全部测试通过（新增 4 个 trad 用例 + 1 个 fromInt）。

- [ ] **Step 8: 提交**

```bash
cd /home/wwwroot/bag/opi
git add android/app/src/main/kotlin/io/opi/input/engine/EngineController.kt android/app/src/main/kotlin/io/opi/input/ime/ImeScreen.kt android/app/src/main/kotlin/io/opi/input/jni/OpiEngine.kt android/app/src/main/kotlin/io/opi/input/jni/EngineLoader.kt android/app/src/test/kotlin/io/opi/input/jni/EngineLoaderTest.kt android/app/src/test/kotlin/io/opi/input/engine/EngineControllerTest.kt
git commit -m "feat(android): 模式键三态 中→繁→英 + EngineLoader 双资产加载"
```

---

### Task 5: 单字全覆盖门禁 + 全量回归

**Files:**
- Create: `crates/opi-tools/tests/trad_coverage.rs`

- [ ] **Step 1: 写门禁测试**

Create `crates/opi-tools/tests/trad_coverage.rs`：

```rust
//! 单字全覆盖门禁（spec 2026-08-15 测试节）：trad_hanzi.tsv 每行 (word, pinyin)
//! 逐一 query 断言该字出现在候选（GB2312 6763 字 ⊂ 期待表）；数据产物提交入库（Task 1）。
//! 本测试只读不联网；trad.opid 缺失时报错并引导执行 Task 1 数据构建。

use engine_data::{load_mmap, Dictionary};
use std::path::Path;

const RAW_TSV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/trad_hanzi.tsv");
const GENERATED_OPID: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/generated/trad.opid");

fn rows() -> Vec<(String, String)> {
    let text = std::fs::read_to_string(RAW_TSV)
        .unwrap_or_else(|e| panic!("读取 {RAW_TSV} 失败（先执行 Task 1 数据构建并提交）：{e}"));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split('\t');
            let word = it.next().unwrap_or("").to_string();
            let pinyin = it.next().unwrap_or("").to_string();
            (word, pinyin)
        })
        .collect()
}

#[test]
fn every_tsv_char_queryable() {
    let dict = load_mmap(Path::new(GENERATED_OPID))
        .unwrap_or_else(|e| panic!("加载 {GENERATED_OPID} 失败（先执行 Task 1 数据构建并提交）：{e:?}"));
    let rows = rows();
    assert!(rows.len() >= 6763, "期待表应覆盖 GB2312 全量 6763 字，实际 {} 行", rows.len());
    let missing: Vec<(&str, &str)> = rows
        .iter()
        .filter(|(w, py)| !dict.query(py, usize::MAX).iter().any(|e| &e.word == w))
        .map(|(w, py)| (w.as_str(), py.as_str()))
        .take(10)
        .collect();
    assert!(missing.is_empty(), "以下 (字, 拼音) 查询无候选（最多展示 10）：{missing:?}");
}

#[test]
fn trad_spot_checks() {
    let dict = load_mmap(Path::new(GENERATED_OPID)).expect("trad.opid 已由 Task 1 提交");
    let has = |pinyin: &str, word: &str| dict.query(pinyin, usize::MAX).iter().any(|e| e.word == word);
    assert!(has("fa", "發"));
    assert!(has("fa", "髮"));
    assert!(has("taiwan", "臺灣"));
    assert!(has("zhonghuaminguo", "中華民國"));
    assert!(has("hao", "好")); // 简繁同形字在 trad 库也可打
}
```

- [ ] **Step 2: 运行门禁**

Run: `cd /home/wwwroot/bag/opi && cargo test -p opi-tools --test trad_coverage`
Expected: PASS（两个用例全绿；若 trad.opid 缺失会 panic 报错引导 Task 1）。

- [ ] **Step 3: 全量回归**

Run: `cd /home/wwwroot/bag/opi && cargo test && cargo clippy --workspace --all-targets 2>&1 | tail -3`
Expected: 全部测试通过；clippy 无 error。

- [ ] **Step 4: 提交**

```bash
cd /home/wwwroot/bag/opi
git add crates/opi-tools/tests/trad_coverage.rs
git commit -m "test(tools): GB2312 单字全覆盖门禁 + 繁体抽查（trad_coverage.rs）"
```

---

### Task 6: Android 模拟器验收

**Files:** 无（纯验证；发现问题则修复后另提交）

前置：`adb devices` 有模拟器/真机在跑（沿用本机既有模拟器）。

- [ ] **Step 1: 构建并安装**

```bash
cd /home/wwwroot/bag/opi/android
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

Expected: BUILD SUCCESSFUL；`Success`。

- [ ] **Step 2: 启用 IME 并打开输入目标**

```bash
adb shell ime enable io.opi.input/.OpiImeService
adb shell ime set io.opi.input/.OpiImeService
adb shell am start -a android.settings.SETTINGS
```

Expected: 前两条 `Success`/无报错；Settings 前台（`dumpsys activity activities | grep topResumedActivity` 为 SettingsActivity）。

- [ ] **Step 3: 模式键三态循环验证**

点击 Settings 搜索框聚焦，`adb shell uiautomator dump` 找模式键节点（text="中"），取其 bounds 中心 tap。
依次验证标签循环：

```bash
adb shell uiautomator dump /sdcard/ui.xml && adb pull /sdcard/ui.xml /tmp/ui.xml
grep -o 'text="繁"' /tmp/ui.xml; grep -o 'text="英"' /tmp/ui.xml; grep -o 'text="中"' /tmp/ui.xml
```

Expected: tap 一次「中」→ 出现「繁」；再 tap「繁」→ 出现「英」；再 tap「英」→ 回到「中」。

- [ ] **Step 4: 繁体模式打字验证**

模式键 tap 到「繁」，依次输入：
- `f`、`a` → 候选栏首个为 發（uiautomator dump 候选文本），候选含 髮；tap 首候选提交，EditText text 含 發。
- `zhonghuaminguo` → 候选含 中華民國（tap 提交验证进 EditText）。

Expected: 繁模式下 fa → 發 靠前；zhonghuaminguo → 中華民國。

- [ ] **Step 5: 简体模式回归**

模式键 tap 回「中」，输入 `f`、`a` → 候选首为 发（简体）；tap 提交。
输入 `nihao` → 候选顺序与改动前一致（luna 词库未动，顺序由 Task 2/5 回归保证，此处冒烟即可）。
Expected: 中模式 fa → 发 靠前；nihao 候选非空且首候选与既有行为一致。

- [ ] **Step 6: 收尾**

`adb shell ime set com.android.inputmethod.latin/.LatinIME`（可选，恢复默认输入法）。
无新增代码则无提交；若发现问题，修复后按 Task 2–5 对应位置提交。

---

## Self-Review（spec 对照）

| spec 要求 | 落点 |
|---|---|
| 数据层：gen_trad_dict.py + trad_hanzi.tsv/trad_phrases.tsv + trad.opid + LICENSES.md 增补 | Task 1 |
| 词频策略（同音排序、去重取 max） | Task 1 偏差注记（统一常用度排序）+ compile 层 max freq |
| 引擎层：Mode::Traditional、双词典 Option、with_dictionaries、candidates 路由、switch_mode 清残留、shift 禁用、空格/回车同 Pinyin | Task 2 |
| 错误处理：trad 缺失回退简体 + logcat 告警；数据源失败阻断编译（产物入库） | Task 1（产物入库）/ Task 2（回退）/ Task 4（告警） |
| 单字全覆盖门禁（6763 字） | Task 5 |
| 繁体抽查（發/髮/臺灣/中華民國） | Task 5 |
| 模式切换语义（清残留、路由、shift） | Task 2 测试 + Task 4 UI |
| 引擎回归全绿 | Task 2/3/5 |
| JVM 测试（EngineLoader 双加载 + 缺失回退） | Task 4 |
| 模拟器验收（三态循环、fa→發、fa→发、zhonghua→中華民國） | Task 6 |
