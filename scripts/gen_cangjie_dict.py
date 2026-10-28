#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""生成仓颉五代码表 cangjie5（扩展 7 数据阶段，2026-10-28）。

数据源（钉 commit + sha256；缓存 data/generated/ 未入库、每次校验；许可结论见
data/raw/LICENSES.md「仓颉码表」节）：
  * rime/rime-cangjie @ 52d90a1b1312e74042b38c1cbc8142defbc53171
      cangjie5.base.dict.yaml      基本区单字
      cangjie5.extended.dict.yaml  扩展区/兼容区单字
  * cangjie5.stem.dict.yaml 只用于「跳过不丢行」的断言，**不进产物**：上游把带
    stem 列的 3 列行拆出来供词组编码用，本表只要 (字,码)。断言其每个 (字,码) 都
    已在 base 中（2026-10-28 本 pin 实测 855/855 行、844/844 个去重对全在 base；
    任务书的「784/855」与实测不符，故按性质断言、不写死条数）—— 将来上游出现
    base 之外的 stem 对时这里红，而不是静默丢映射。

仓内形状：`word<TAB>code<TAB>freq`（列序真源 engine-data/src/dict_text.rs：
cols[0]=word、cols[1]=码）。码长 1–5、只用 a-z；x/z 前缀行原样保留（上游只把它们
排除在词组编码之外，见 LICENSES）。

词频：上游无权重。逐字取 data/raw/trad_hanzi.tsv 的**最大** freq（多音字取最高
读音）；查不到的字用低档常数 FALLBACK_FREQ（= parse_dict 的缺列缺省 1000）。
这是**排序信号**，不是词频统计；跨码/跨表比较无意义。trad_hanzi.tsv 重编后本表
freq 数值会变（同码内相对序不变），需重跑本生成器保持一致。

上游有 1 行 word 是全角空格 U+3000（码 zxaa）：`parse_dict` 的 `trim()` 会把 word
抹成空串、按 unusable 整条丢弃（compile 会打「warning: 跳过」）⇒ 生成侧提前剔除
并报数，保证 kept entries == 生成行数、stderr 零跳过。

自洽校验（stderr，条数每次实测、不许写死）：码字符集与码长；去重后条数；同码
冲突分布；freq 覆盖率与统计；人工 top-N（a→日、aa→昌、ab→明 —— 期望值手写、
不由同一份 freq 反推）；编译自检（opi-tools compile 的 kept entries == 生成行数、
且 stderr 零「跳过」警告 —— 任何静默丢行都在这条上红）。

产物：data/raw/cangjie5.tsv（入库）→ data/generated/cangjie5.opid（入库，见
data/generated/.gitignore 白名单，照 wubi 先例）。脚本幂等（同输入 → 逐字节同产物）。

用法：cd <repo 根> && python3 scripts/gen_cangjie_dict.py
升级路径：改 URL/COMMIT/SHA256 → 重跑 → 按 stderr 的覆盖率与 top-N 回归信号核对变化。
"""
import hashlib
import sys
import urllib.request
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_wubi_dict import compile_opid, write_tsv  # noqa: E402  同一套编译自检与原子写
from hanzi_freq import is_cjk  # noqa: E402  收录范围口径的唯一来源

REPO = Path(__file__).resolve().parent.parent
RAW = REPO / "data/raw"
GEN = REPO / "data/generated"

# ---- 上游 pin（2026-06-01）。升级 = 改这里 + 重跑，别用浮动引用 ----
COMMIT = "52d90a1b1312e74042b38c1cbc8142defbc53171"
SOURCES = {
    "base": (
        GEN / "cangjie5.base.dict.yaml",
        f"https://raw.githubusercontent.com/rime/rime-cangjie/{COMMIT}"
        "/cangjie5.base.dict.yaml",
        "8690f2ad8aafd38780846881aa916b5779e6d9247a351a1da426d3f3257afca4",
    ),
    "extended": (
        GEN / "cangjie5.extended.dict.yaml",
        f"https://raw.githubusercontent.com/rime/rime-cangjie/{COMMIT}"
        "/cangjie5.extended.dict.yaml",
        "a6a0b06427c599d9fa7fc1e02a618bf234eb1dc9fc9b6b92f3292fc02213b3cd",
    ),
    "stem": (
        GEN / "cangjie5.stem.dict.yaml",
        f"https://raw.githubusercontent.com/rime/rime-cangjie/{COMMIT}"
        "/cangjie5.stem.dict.yaml",
        "a15208351dbc33d5ade8d16ece75907ef2a7310e3962f4e54003dab9bbd49b8a",
    ),
}
FALLBACK_FREQ = 1000  # 低档兜底：与 parse_dict 的缺列缺省同值（真实 freq 均 >1e9）
TOP_N = {"a": "日", "aa": "昌", "ab": "明"}  # 人工期望值（写明，不从 freq 反推）
ALPHABET = frozenset("abcdefghijklmnopqrstuvwxyz")


def ensure_source(name: str) -> bytes:
    """取上游文件：优先 data/generated/ 的缓存副本（照样逐次校验 sha256），没有才按 pin 拉。"""
    path, url, sha = SOURCES[name]
    if path.exists():
        data = path.read_bytes()
        src = f"缓存 {path.relative_to(REPO)}"
    else:
        print(f"下载 {url}", file=sys.stderr)
        with urllib.request.urlopen(url, timeout=180) as r:
            data = r.read()
        src = "下载"
    got = hashlib.sha256(data).hexdigest()
    if got != sha:
        sys.exit(
            f"!! {name} 摘要与 pin 不符（{src}）：期望 {sha[:16]}…，实得 {got[:16]}…。\n"
            f"   上游内容变了 —— 确认后更新 {Path(__file__).name} 的 pin 再重跑。"
        )
    if not path.exists():
        GEN.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    print(f"源 {name}: {src}，sha256 ✓（{len(data)} 字节）", file=sys.stderr)
    return data


def parse_pairs(text: str, name: str) -> list[tuple[str, str]]:
    """rime dict.yaml → [(word, code)]；front-matter/注释/空行跳过，坏行响亮报错。"""
    rows: list[tuple[str, str]] = []
    in_fm, started = False, False
    for i, line in enumerate(text.splitlines(), 1):
        s = line.strip()
        if s == "---" and not started:
            in_fm, started = True, True
            continue
        if in_fm:
            if s in ("---", "..."):
                in_fm = False
            continue
        if not s or s.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) < 2:
            sys.exit(f"!! {name} 第 {i} 行不是 TAB 分隔的两列：{line[:60]!r}")
        rows.append((cols[0].strip(), cols[1].strip()))
    if in_fm:
        sys.exit(f"!! {name} 的 front-matter 没闭合（缺 '...'）")
    return rows


def load_trad_hanzi_freq() -> dict[str, int]:
    """data/raw/trad_hanzi.tsv → {字: 最大 freq}（多音字取最高读音）。"""
    freq: dict[str, int] = {}
    path = RAW / "trad_hanzi.tsv"
    for i, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        cols = line.split("\t")
        if len(cols) != 3:
            sys.exit(f"!! {path.name} 第 {i} 行不是三列：{line[:60]!r}")
        w, _py, f = cols
        freq[w] = max(freq.get(w, 0), int(f))
    return freq


def top_n_gate(rows: list[tuple[str, str, int]]) -> None:
    """人工 top-N：码 → 期望首字（手写常量）。并列 top 也红 —— 引擎次序未定义。"""
    by_code: dict[str, list[tuple[int, str]]] = defaultdict(list)
    for w, c, f in rows:
        by_code[c].append((f, w))
    print("人工 top-N 门禁：", file=sys.stderr)
    for code, expected in TOP_N.items():
        ranked = sorted(by_code.get(code, []), key=lambda t: (-t[0], t[1]))
        if not ranked:
            sys.exit(f"!! 码 {code} 没有候选（期望 {expected}）")
        tops = [w for f, w in ranked if f == ranked[0][0]]
        if len(tops) > 1 or expected not in tops:
            sys.exit(f"!! 人工 top-N 红：码 {code} 实测 top = {tops}，期望唯一 top = {expected}")
        tail = " > ".join(f"{w}({f})" for f, w in ranked[:3])
        print(f"  {code} → {expected}：{tail}", file=sys.stderr)


def audit(rows: list[tuple[str, str, int]], char_freq: dict[str, int]) -> None:
    """自洽校验：全部打印、条数实测（别在注释里写死）。"""
    out = lambda *a: print(*a, file=sys.stderr)  # noqa: E731
    out("\n== cangjie5 自洽校验 ==")
    out(f"行数 {len(rows)}（(字,码) 去重后；单字 {sum(1 for w, _, _ in rows if len(w) == 1)}）")

    lens = Counter(len(c) for _, c, _ in rows)
    out("码长分布 " + " ".join(f"{n}键:{lens[n]}" for n in sorted(lens)))
    letters = Counter(ch for _, c, _ in rows for ch in c)
    out(f"码字符 {len(letters)} 个：{''.join(sorted(letters))}")
    xrows = sum(1 for _, c, _ in rows if c.startswith("x"))
    zrows = sum(1 for _, c, _ in rows if c.startswith("z"))
    out(f"x/z 前缀行：{xrows} / {zrows}（有意保留，见 LICENSES）")

    by_code: dict[str, set[str]] = defaultdict(set)
    for w, c, _ in rows:
        by_code[c].add(w)
    dist = Counter(len(v) for v in by_code.values())
    out(f"同码冲突：码 {len(by_code)} 个；" + " ".join(f"{n}字:{dist[n]}" for n in sorted(dist)[:8]))
    for c, ws in sorted(by_code.items(), key=lambda kv: (-len(kv[1]), kv[0]))[:5]:
        out(f"  冲突最多 {c}: {len(ws)} 字，如 {sorted(ws)[:5]}")

    covered = sum(1 for w, _, _ in rows if w in char_freq)
    chars = {w for w, _, _ in rows}
    covered_chars = len(chars & set(char_freq))
    freqs = [f for _, _, f in rows]
    out(
        f"freq：覆盖 {covered}/{len(rows)}（{covered / len(rows):.1%}）行、"
        f"{covered_chars}/{len(chars)}（{covered_chars / len(chars):.1%}）字；"
        f"兜底 {len(rows) - covered} 行 = {FALLBACK_FREQ}；min {min(freqs)} / max {max(freqs)}"
    )

    outside = [w for w, _, _ in rows if not is_cjk(w)]
    out(f"收录范围外字符：{len(outside)} 行（如 {''.join(outside[:10])}）—— 有意保留（tofu 局限见 LICENSES）")


def main() -> int:
    base = parse_pairs(ensure_source("base").decode("utf-8"), "cangjie5.base")
    ext = parse_pairs(ensure_source("extended").decode("utf-8"), "cangjie5.extended")
    stem = parse_pairs(ensure_source("stem").decode("utf-8"), "cangjie5.stem")

    # stem 只做「跳过不丢」断言：其 (字,码) 必须全部已在 base 中。
    base_pairs = set(base)
    missing = sorted({p for p in stem if p not in base_pairs})
    if missing:
        sys.exit(
            f"!! stem 有 {len(missing)} 个 (字,码) 不在 base（前 3：{missing[:3]}）—— "
            "跳过 stem 会丢映射，不能静默继续"
        )
    print(
        f"stem 不进产物：{len(stem)} 行 / {len(set(stem))} 个去重 (字,码)，全部已在 base"
        f"（{sum(1 for p in stem if p in base_pairs)}/{len(stem)} 行重复）",
        file=sys.stderr,
    )

    pairs = list(dict.fromkeys(base + ext))  # 去重保序：base 内部实测有 11 个重复对
    empty = [p for p in pairs if not p[0]]
    if empty:
        print(
            f"上游不可承载行：{len(empty)} 行 word 为空白（码 {', '.join(c for _, c in empty)}）"
            "—— parse_dict 按空 word 整条丢弃，生成侧提前剔除",
            file=sys.stderr,
        )
        pairs = [p for p in pairs if p[0]]
    bad = [p for p in pairs if not p[1] or any(ch not in ALPHABET for ch in p[1])]
    if bad:
        sys.exit(f"!! 码含 a-z 之外字符：{len(bad)} 条（前 3：{bad[:3]}）")
    over = {n: k for n, k in Counter(len(c) for _, c in pairs).items() if n > 5}
    if over:
        sys.exit(f"!! 码长 >5 键：{over} —— 仓颉五代码最长 5")

    char_freq = load_trad_hanzi_freq()
    rows = [(w, c, char_freq.get(w, FALLBACK_FREQ)) for w, c in pairs]

    top_n_gate(rows)
    audit(rows, char_freq)

    tsv = RAW / "cangjie5.tsv"
    write_tsv(tsv, rows)
    print(f"产物 {tsv.relative_to(REPO)}：{tsv.stat().st_size} 字节", file=sys.stderr)
    compile_opid(tsv, GEN / "cangjie5.opid", len(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main())
