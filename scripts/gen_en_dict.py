#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成英文联想词库 TSV 数据（产物提交入库；离线构建不重跑本脚本）。

产物（UTF-8，word\\tpinyin\\tfreq 三列，与 trad_hanzi.tsv / fallback.tsv 同形）：
  data/raw/en_words.tsv    Google Books Ngrams 英文 1-gram 前 10000 词

**本脚本只产数据。** 消费侧（`candidates.rs` 对非拼音模式 `return Vec::new()`）
不在本脚本范围内 —— 见 data/raw/LICENSES.md 与 spec「V1 仅做单词联想」。
把 TSV 变成引擎可读产物不需要改任何 Rust 代码：

    cargo run -p opi-tools -- compile data/raw/en_words.tsv data/generated/en.opid
    cargo run -p opi-tools -- verify data/generated/en.opid

（`compile` 的解析器收 `word\\tpinyin\\tfreq`，第二列必须 ASCII —— 本脚本已保证；
`verify` 顺带报装载耗时。注意 `.opid` 落在 data/generated/ 不入库，别指望 include_str!。）

## 词源与许可（选源判据：许可 > 一切）

上游 orgtre/google-books-ngram-frequency，**README 逐字声明 content 为 CC-BY 3.0**
（<https://creativecommons.org/licenses/by/3.0/>），且上游本身是 Google Books Ngrams
（同一 CC-BY 3.0）的清洗产物 —— **许可链一致，无「第三方替原作者发许可证」问题**。
CC-BY 属可进本仓库的许可（与 Unihan 的 Unicode License 同为「可再分发 + 保留声明」），
登记见 data/raw/LICENSES.md。freq 是该词在 2010–2019 英文图书语料里的**原始出现次数**。

**被否掉的候选（都实测过，勿重复调研）**：
  * first20hours/google-10000-english（业内最常被抄的那份）：**仓库没有 LICENSE 文件**
    （raw LICENSE 404，GitHub API license=NOASSERTION），README 自述是从 Norvig 的
    count_1w.txt 去掉了词频列而来，而 count_1w.txt 又源自 LDC 分发的 Google Trillion
    Word Corpus（那份是受限许可）。**既无授权、又无词频**，两条都占。
  * AOSP LatinIME 的 en_wordlist.combined.gz（形如 ` word=the,f=222,flags=,originalFreq=222`
    的**纯文本**，165544 行，Apache-2.0 仓库）：差点选它 —— AOSP 与本仓库既有的
    pinyin_simp（Apache-2.0，来自 AOSP PinyinIME）同源，看着最"合规矩"。**但 LatinIME
    的 NOTICE 文件最后一行写着 `Includes Dictionaries © Lexiteria LLC.  Used by
    permission.`** ——「used by permission」是给 AOSP 的许可，不是可向下再分发的授权；
    而 LatinIME 的 dictionaries/ 下只有这些词表，无法把它解释成别的组件。**弃**。
    另一条代价：它的 f= 是量化过的（the=222、to=215、of=214），排序分辨率远低于真实词次。
  * hermitdave/FrequencyWords（en_50k.txt，50000 词，真实字幕词次）：仓库 LICENSE 文件
    是 MIT，**但 README 明写「MIT License for code. CC-by-sa-4.0 for content.」** ——
    数据是**共享演绎**许可，与本仓库产物（要进 APK 分发）冲突；且两处自相矛盾。
  * rspeer/wordfreq：Apache-2.0 是**代码**，数据 README 自述 CC-BY-SA-4.0，并**明确拒绝**
    「转成 CSV 方便使用」这种做法（理由就是 CSV 没地方放署名）。作者本人说不，就不碰。
  * words/subtlex-word-frequencies：仓库自称 ISC，但 SUBTLEX 原始数据是 Ghent 大学的
    研究用途数据 —— 第三方替原作者发许可证，属许可洗白，不采纳。

## 词频缩放：为什么必须缩

上游 counts 最大值 13616383631（the）**超过 u32::MAX**，而 `compiler.rs` 的 `parse_freq`
对 `s.parse::<u32>()` 失败的行按 **unusable 整条丢弃** —— 照抄数字会把 the / of / and
这些最高频词全部丢掉（且 compile 只打一行 warning，不会失败）。
故线性缩放到 **FMAX = 4_000_000_000**（`hanzi_freq.FMAX`，中文库的天花板）：
`freq = round(原始 counts × FMAX / max_counts)`。

**上限必须正好是 FMAX，不能更大**：`Engine::with_dictionaries` 的
`user_boost = max(USER_BOOST, max_freq × 2)` 是**跨词典取一个全局值**，英文库若把
max_freq 抬到 FMAX 之上，会连带改变中文侧「一次选词压过全部静态词」的标度。
定成 FMAX 则两种词库上限相同，boost 不动（中文侧已是 4e9，见 engine.rs 注释）。

线性缩放是单调的，但 `round()` 在低频端会产生**相邻等值**（实测 20 处，如
`welcoming` 1407656 / `cinnamon` 1407207 之比已低于缩放分辨率）。等值会让这批词的
先后变成引擎的任意 tie-break，故加一道守卫：按词次降序扫一遍，
`freq[i] = min(freq[i], freq[i-1] - 1)` —— 保证严格递减、**不改变任何一对词的相对顺序**。

## 键与词形

第二列是**查询键**，第一列是**候选文本**。两者多数相同，不同只在两处：
  * 大写开头的词（I / God / China，共 178 条）：键取小写 → 打 `china` 出 `China`。
  * 非 ASCII 词（`café`）：键按 NFKD 剥离组合音调符 → `cafe`（与 `hanzi_freq.normalize`
    去调号同法）。**不丢词** —— `compiler.rs` 只要求第二列 ASCII，第一列可以非 ASCII。
键一律小写 + ASCII，`compiler.rs` 会对第二列再做一次 `.to_lowercase()`，幂等。

`ok` / `OK` 两条键相同（全表唯一一组大小写重名，实测）。**有意不合并**：trie 按
(键, 词) 存，两条都留得住；合并要发明「取高频者/词次相加」的规则，那是候选层的判断。

## 用法（需要网络；产物已入库，正常构建不跑）

    cd <repo 根> && python3 scripts/gen_en_dict.py

升级路径：换 SRC_URL 的 commit + SRC_SHA256（`curl -sSL <url> | sha256sum`），重跑本脚本，
再重编译 `.opid` 看装载耗时。摘要不符即报错 —— 宁可失败也不静默换数据（同 gen_luna_dict）。
"""
import csv
import hashlib
import io
import sys
import unicodedata
import urllib.request

from hanzi_freq import FMAX

REPO = "orgtre/google-books-ngram-frequency"
# pin 到 commit（不是 main）：上游一改，产物就不可复现。sha256 是本次实际抓下来的。
SRC_URL = (
    f"https://raw.githubusercontent.com/{REPO}/"
    "e20471c15a758be3362b16d07870b34df4f7ccc3/ngrams/1grams_english.csv"
)
SRC_SHA256 = "f7f63aa08f0bb2f7f654cddff3a7ced4968b27887b2c0e7cd4b4d108221e197d"
OUT = "data/raw/en_words.tsv"
# 上游「10000 most frequent 1-grams」的承诺；不符即报错（说明上游改了列表规模）。
EXPECT_ROWS = 10000
# compiler.rs 对 word / pinyin 都写死了 ≤255 字节。
MAX_FIELD = 255


def fetch(url: str, sha256: str) -> str:
    """下载并校验 pin 的摘要，返回 UTF-8 文本。摘要不符即报错 —— 宁可失败也不静默换数据。"""
    with urllib.request.urlopen(url, timeout=180) as r:
        data = r.read()
    got = hashlib.sha256(data).hexdigest()
    if got != sha256:
        raise RuntimeError(
            f"{url} 摘要与 pin 不符：期望 {sha256}，实得 {got}。"
            f"上游内容变了，请确认许可与词表未变后更新 scripts/gen_en_dict.py 的 pin 并重跑。"
        )
    return data.decode("utf-8-sig")  # utf-8-sig：上游带不带 BOM 都能读


def ascii_key(word: str) -> str:
    """查询键：NFKD 剥组合音调符 → 小写。café→cafe，China→china，the→the。"""
    return "".join(c for c in unicodedata.normalize("NFKD", word) if not unicodedata.combining(c)).lower()


def parse_csv(text: str) -> list[tuple[str, int]]:
    """上游 CSV（ngram,freq,cumshare）→ [(词, 原始词次)]，按词次降序。cumshare 不用。"""
    rows: list[tuple[str, int]] = []
    for i, rec in enumerate(csv.reader(io.StringIO(text))):
        if i == 0:
            continue  # 表头
        if len(rec) != 3:
            raise RuntimeError(f"第 {i + 1} 行不是 3 列（上游 CSV 格式变了）：{rec!r}")
        rows.append((rec[0], int(rec[1])))
    rows.sort(key=lambda r: -r[1])
    return rows


def scaled_freqs(rows: list[tuple[str, int]]) -> list[int]:
    """词次降序 → 严格递减的 u32 安全 freq（见文件头「词频缩放」）。"""
    top = rows[0][1]
    if top <= FMAX:
        raise RuntimeError(f"上游最大词次 {top} 未超过 FMAX，缩放规则的前提已不成立")
    out = [max(1, round(f * FMAX / top)) for _, f in rows]
    for i in range(1, len(out)):
        # 等值 → 压到前一条之下：保严格递减，且不改变相对顺序。
        out[i] = min(out[i], out[i - 1] - 1)
    assert out[-1] >= 1, "freq 压到 0：行数太多或缩放写错了"
    return out


def write_atomic(path: str, lines: list[str]) -> None:
    """先写 .tmp 再 os.replace：中途崩溃不留半截 TSV（同 gen_trad_dict.py）。"""
    tmp = f"{path}.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    import os

    os.replace(tmp, path)


def main() -> None:
    rows = parse_csv(fetch(SRC_URL, SRC_SHA256))
    if len(rows) != EXPECT_ROWS:
        print(f"FATAL: 上游 {len(rows)} 行，预期 {EXPECT_ROWS}", file=sys.stderr)
        sys.exit(1)
    freqs = scaled_freqs(rows)

    entries: list[tuple[str, str, int]] = []
    folded = 0
    keyed_same = 0
    for (word, _), freq in zip(rows, freqs):
        key = ascii_key(word)
        # 不静默丢词：键为空或非 ASCII 是生成器的 bug，不是数据问题，直接报错。
        if not key or not key.isascii():
            raise RuntimeError(f"词 {word!r} 的键 {key!r} 为空或非 ASCII")
        if len(key.encode()) > MAX_FIELD or len(word.encode()) > MAX_FIELD:
            raise RuntimeError(f"词 {word!r} 或键 {key!r} 超过 {MAX_FIELD} 字节，compiler 会丢")
        if key != word:
            folded += 1
        else:
            keyed_same += 1
        entries.append((word, key, freq))

    # 输出按 (键, 词) 排：文件 diff 友好，且与 compiler 的排序一致（同 trad_hanzi.tsv）。
    entries.sort(key=lambda e: (e[1], e[0]))
    write_atomic(OUT, [f"{w}\t{k}\t{f}" for w, k, f in entries])

    dup_keys = len(entries) - len({k for _, k, _ in entries})

    # 落盘后回读自检：产物是别人唯一看得到的东西，emit 路径写错没人会知道。
    # 钉住「词集不丢」+「freq 大小顺序 == 源词次顺序」（上面那道守卫的实测，不是重述）。
    back = [l.rstrip("\n").split("\t") for l in open(OUT, encoding="utf-8")]
    assert len(back) == len(entries), f"回读 {len(back)} 行，写入 {len(entries)} 行"
    assert {r[0] for r in back} == {w for w, _, _ in entries}, "回读词集与源词集不等"
    by_back = [r[0] for r in sorted(back, key=lambda r: -int(r[2]))]
    by_src = [w for w, _, _ in sorted(entries, key=lambda e: -e[2])]
    assert by_back == by_src, f"回读 freq 顺序与源词次顺序不符：首个差异 {set(by_back) ^ set(by_src)}"

    print(f"上游: {REPO} @ e20471c（CC-BY 3.0）")
    print(f"词条: {len(entries)}（键即词形 {keyed_same}、键经折叠 {folded}）")
    print(f"重复键: {dup_keys} 组（大小写重名，有意不合并）")
    print(f"freq 范围: [{entries and min(f for _, _, f in entries)}, {max(f for _, _, f in entries)}]")
    print(f"{OUT} 行数: {len(entries)}")


if __name__ == "__main__":
    main()
