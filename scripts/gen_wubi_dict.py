#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""生成五笔 86 / 98 码表（M4 计划 3a，2026-10-14；裁决：86 + 98 入库，新世纪 06 不做）。

数据源（均钉 commit + sha256；许可结论与「残留不一致」见 data/raw/LICENSES.md「五笔码表」节）：
  * 86 = rime/rime-wubi 的 wubi86.dict.yaml —— 上游是极点码表转 rime 格式。
  * 98 = yanhuacuo/98wubi-tables 两份：
      「超集-rime格式码表.txt」**全是单字**（含简码形态，覆盖 99,050 字；带权重列，
        实测为文件序的稠密排名 1098420-i —— 只保证同码内的相对序，不是词频）；
      「98五笔含词表-【单义】.txt」单字 + 词组（词组无权重列）。
    为什么两份拼：词组**只有含词表那份有**（rime 格式那份词组 0 行），而含词表的单字
    实测全部 ⊂ rime 格式那份（0 例外）⇒ 单字段取 rime（字覆盖大 3.6 倍）、词组段取
    含词表。脚本内断言这条包含关系，上游或拼法漂了就红。

仓内形状：`word<TAB>code<TAB>freq` 三列（列序真源 = engine-data/src/dict_text.rs，
cols[0]=word、cols[1]=码；写反 = 全表废掉）。上游 rime 四列的 stem 列**丢弃**（86 的
25 条一级简码行、98 的简码行带它；它与 (word, code) 的对应关系无关）。freq 口径 =
dict_text::parse_freq：整数直取；缺列缺省 1000；**浮点权重在生成器里四舍五入**
（86 实测 1 条：χ/zzxx 450.5）—— 原样落盘会被 parse_freq 按 unusable 整条丢弃
（χ 从 zzxx 里静默消失），故取整并把计数打出来。

有意**不**过滤、原样保留的两类行（决定与理由记在 LICENSES.md，改之前先读）：
  * 86 的 677 条 z 前缀行（zzjp 片假名 / zzpj 平假名 / zzsx 数学符号 / zzpp 部首…）
    —— 上游的「分类表」约定，不是五笔编码；z 在五笔里通常是万能键，两者会不会打架
    是 wubi-engine（下一波）的设计问题，数据侧报出、不替它裁决。
  * 词含 CJK 基本区/扩展 A/B 之外字符的行（扩展 C+、康熙部首、假名、符号）。本仓的
    收录范围口径（hanzi_freq.CJK_RANGES）是给拼音词库定的；五笔表是**码表**、超集字
    正是它的卖点，生成侧不过滤（tofu 局限如实写进 LICENSES.md）。

自洽校验（全打 stderr，条数每次实测、不许写死）：码只准 a-z、码长 1–4；同码冲突分布
（「四码唯一自动上屏」的判据输入）；词组码规则核验（2 字 = 各取前两码、3 字 = 前两字
首码 + 末字前两码、4 字 = 各首码、>4 字 = 前 3 字首码 + 末字首码；组成字缺全码 / 多全码
的如实计数）；freq 统计；编译自检（opi-tools compile 的 kept entries == 生成行数，且
stderr 不得出现「warning: 跳过」—— 任何静默丢行都在这条上红）。

产物：data/raw/wubi86.tsv / wubi98.tsv（入库）→ 编译为 data/generated/wubi86.opid /
wubi98.opid（**入库**：与 fallback/trad 同列进 data/generated/.gitignore 白名单；
luna.opid 不入库是因为 android 资产另有部署副本，wubi 暂无部署副本）。脚本幂等
（同输入 → 逐字节同产物）；上游文件缓存在 data/generated/（未入库），每次校验 sha256。

用法：cd <repo 根> && python3 scripts/gen_wubi_dict.py
升级路径：改下面 URL/SHA256 常量 → 重跑 → 按 stderr 校验结论核对变化。
"""
import hashlib
import re
import subprocess
import sys
import urllib.request
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_trad_dict import write_atomic  # noqa: E402  先写 .tmp 再 os.replace，防半截 TSV
from hanzi_freq import FMAX, is_cjk  # noqa: E402  收录范围口径的唯一来源

REPO = Path(__file__).resolve().parent.parent
RAW = REPO / "data/raw"
GEN = REPO / "data/generated"

# ---- 上游 pin（2026-10-14）。升级 = 改这里 + 重跑，别用浮动引用 ----
SOURCES = {
    "wubi86": (
        GEN / "wubi86.dict.yaml",
        "https://raw.githubusercontent.com/rime/rime-wubi/"
        "152a0d3f3efe40cae216d1e3b338242446848d07/wubi86.dict.yaml",
        "f833d86b72341fe82e069a425b6625f29ef85f1bc0f34f6fb7975fe514888b5a",
    ),
    # 98 单字源：超集-rime 格式（URL 里的中文是 percent-encoding）
    "wubi98_rime": (
        GEN / "wubi98_rime.txt",
        "https://raw.githubusercontent.com/yanhuacuo/98wubi-tables/"
        "6b8b6fb9d3c34e0d5e3b17211e1f1c100e7eb697/"
        "%E8%B6%85%E9%9B%86-rime%E6%A0%BC%E5%BC%8F%E7%A0%81%E8%A1%A8.txt",
        "728acbcc1c7ae57f6bfd88f0176754b5770d3af42490bc124bac6669e1913a53",
    ),
    # 98 词组源：98五笔含词表-【单义】（UTF-16LE + CRLF；本脚本的 hanci 解析只取词组行）
    "wubi98_hanci": (
        GEN / "wubi98_hanci.txt",
        "https://raw.githubusercontent.com/yanhuacuo/98wubi-tables/"
        "6b8b6fb9d3c34e0d5e3b17211e1f1c100e7eb697/"
        "98%E4%BA%94%E7%AC%94%E5%90%AB%E8%AF%8D%E8%A1%A8-%E3%80%90%E5%8D%95%E4%B9%89%E3%80%91.txt",
        "1b5a4c22eddae08d8e0e6aa8f926f92d45d6cdaa591e5cee0e8aad29271da1cc",
    ),
}
DEFAULT_FREQ = 1000  # parse_dict 的缺列缺省（口径跟 engine-data，不自己发明）
_FLOAT = re.compile(r"\d+\.\d+")
_Z = re.compile(r"^z")


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
            f"   上游内容变了 —— 确认后更新 {__file__} 的 pin 再重跑。"
        )
    if not path.exists():
        GEN.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    print(f"源 {name}: {src}，sha256 ✓（{len(data)} 字节）", file=sys.stderr)
    return data


def parse_wubi86(text: str) -> tuple[list[tuple[str, str, int]], dict]:
    """rime dict.yaml → (word, code, freq)。跳过规则与 dict_text.rs 逐条对齐。"""
    rows: list[tuple[str, str, int]] = []
    stat: Counter = Counter()
    in_fm, started = False, False
    for line in text.splitlines():
        s = line.strip()
        if s == "---" and not started:  # front-matter 整段跳过
            in_fm, started = True, True
            continue
        if in_fm:
            if s in ("---", "..."):
                in_fm = False
            continue
        if not s or s.startswith("#"):
            continue
        cols = line.split("\t")
        word, code = cols[0].strip(), cols[1].strip() if len(cols) > 1 else ""
        if len(cols) == 2:  # 上游没给权重（实测 163 行，多为扩展 E 字）→ 缺省 1000
            stat["权重缺列→1000"] += 1
            rows.append((word, code, DEFAULT_FREQ))
            continue
        w = cols[2].strip()
        if w.isdigit():
            freq = int(w)
        elif _FLOAT.fullmatch(w):  # parse_freq 会整条丢，这里取整保住（见文件头）
            freq = int(float(w) + 0.5)
            stat["浮点权重取整"] += 1
            print(f"  浮点权重取整：{word}\t{code}\t{w} → {freq}", file=sys.stderr)
        else:
            stat[f"权重不可解析:{w[:10]}"] += 1
            continue
        rows.append((word, code, freq))
    return rows, dict(stat)


def parse_wubi98(rime: str, hanci: str) -> tuple[list[tuple[str, str, int]], dict]:
    """98：单字（超集-rime 格式，4 列取前 3）+ 词组（含词表，2 列，缺省 1000）。"""
    stat: Counter = Counter()
    rows: list[tuple[str, str, int]] = []
    for line in rime.splitlines():
        if not line.strip():
            continue
        c = line.split("\t")
        assert len(c) == 4, f"rime 格式表应为 4 列：{line[:40]!r}"
        rows.append((c[0], c[1], int(c[2])))
    stat["单字行"] = len(rows)

    han_phrases: set[tuple[str, str]] = set()
    han_singles: set[tuple[str, str]] = set()
    for line in hanci.splitlines():
        if not line.strip():
            continue
        c = line.split("\t")
        assert len(c) == 2, f"含词表应为 2 列：{line[:40]!r}"
        (han_singles if len(c[0]) == 1 else han_phrases).add((c[0], c[1]))
    stat["词组行(含词表)"] = len(han_phrases)

    # 自洽断言：含词表单字 ⊂ rime 单字（拼法的前提）。不成立 = 上游漂了，别静默拼。
    rime_set = set((w, c) for w, c, _ in rows)
    extra = [x for x in sorted(han_singles) if x not in rime_set]
    if extra:
        sys.exit(f"!! 含词表单字有 {len(extra)} 条不在 rime 单字表里（前 3：{extra[:3]}）—— 拼法前提破了")
    stat["含词表单字(⊂rime)"] = len(han_singles)

    rows += [(w, c, DEFAULT_FREQ) for w, c in sorted(han_phrases)]
    return rows, dict(stat)


def audit(name: str, rows: list[tuple[str, str, int]]) -> None:
    """自洽校验：全部打印、条数实测（别在注释里写死）。"""
    out = lambda *a: print(*a, file=sys.stderr)  # noqa: E731
    out(f"\n== {name} 自洽校验 ==")
    singles = [r for r in rows if len(r[0]) == 1]
    phrases = [r for r in rows if len(r[0]) > 1]
    out(f"行数 {len(rows)}（单字 {len(singles)} / 词组 {len(phrases)}）")

    pairs = [(w, c) for w, c, _ in rows]
    if len(set(pairs)) != len(pairs):  # 去重是 compile 的事，但两源拼接不许自带重
        dup = [p for p, n in Counter(pairs).items() if n > 1]
        sys.exit(f"!! (word, code) 有重复 {len(dup)} 组（前 3：{dup[:3]}）—— 拼接/解析出错")

    # 1) 码字符合法性 + 码长
    bad = [p for p in pairs if not p[1] or any(ch not in "abcdefghijklmnopqrstuvwxyz" for ch in p[1])]
    if bad:
        sys.exit(f"!! 码含 a-z 之外字符：{len(bad)} 条（前 3：{bad[:3]}）")
    lens = Counter(len(c) for _, c in pairs)
    over = {n: k for n, k in lens.items() if n > 4}
    if over:
        sys.exit(f"!! 码长 >4 键：{over} —— 五笔码最长 4（上游混入简码/超码形态会让判据失准）")
    out("码长分布 " + " ".join(f"{n}键:{lens[n]}" for n in sorted(lens)))

    # 2) z 前缀行（86 的分类表；98 实测 0）
    zrows = [(w, c) for w, c in pairs if _Z.match(c)]
    zcodes = Counter(c for _, c in zrows)
    out(f"z 前缀行 {len(zrows)}" + (f"：{', '.join(f'{c}({n})' for c, n in sorted(zcodes.items(), key=lambda kv: (-kv[1], kv[0]))[:6])}" if zrows else ""))

    # 3) 同码冲突（同码不同词）——「四码唯一自动上屏」的直接判据输入
    by_code: dict[str, set[str]] = defaultdict(set)
    for w, c in pairs:
        by_code[c].add(w)
    dist = Counter(len(v) for v in by_code.values())
    out(f"同码冲突：码 {len(by_code)} 个；" + " ".join(f"{n}词:{dist[n]}" for n in sorted(dist)[:8]) + (" …" if len(dist) > 8 else ""))
    four = Counter(len(v) for c, v in by_code.items() if len(c) == 4)
    out(f"  四键码 {sum(four.values())} 个：唯一 {four[1]}、冲突 {sum(four.values()) - four[1]}"
        f"（其中 {sum(1 for c, v in by_code.items() if len(c) == 4 and len(v) == 2)} 个恰 2 词）")
    top = sorted(by_code.items(), key=lambda kv: (-len(kv[1]), kv[0]))[:5]
    for c, ws in top:
        out(f"  冲突最多 {c}: {len(ws)} 词，如 {sorted(ws)[:5]}")

    # 4) 词组码规则核验：2 字各取前两码、3 字前两字首码+末字前两码、4 字各首码、>4 字前 3 首码+末字首码
    codes: dict[str, list[str]] = defaultdict(list)
    for w, c in pairs:
        codes[w].append(c)
    full: dict[str, list[str]] = {}  # 全码 = 最长码（同长多个 = 歧义，取字典序首个并计数）
    for w, cs in codes.items():
        mx = max(len(c) for c in cs)
        full[w] = sorted(set(c for c in cs if len(c) == mx))
    res: Counter = Counter()
    for w, c in pairs:
        n = len(w)
        if n == 1:
            continue
        fs = [full.get(ch) for ch in w]
        if any(f is None for f in fs):
            res["组成字缺全码"] += 1
            continue
        amb = any(len(f) > 1 for f in fs)
        f = [x[0] for x in fs]
        need = [2, 2] if n == 2 else ([1, 1, 2] if n == 3 else [1] * n)
        if any(len(f[i]) < need[i] for i in range(n)):
            res["全码短于所需"] += 1
            continue
        if n == 2:
            exp = f[0][:2] + f[1][:2]
        elif n == 3:
            exp = f[0][:1] + f[1][:1] + f[2][:2]
        elif n == 4:
            exp = f[0][:1] + f[1][:1] + f[2][:1] + f[3][:1]
        else:
            exp = f[0][:1] + f[1][:1] + f[2][:1] + f[-1][:1]
        key = "匹配" + ("(全码歧义,取前者)" if amb else "")
        res[key if c == exp else key.replace("匹配", "不匹配")] += 1
        if c != exp and res["不匹配(全码歧义,取前者)"] + res["不匹配"] <= 5:
            out(f"  词组规则不匹配：{w}\t表内 {c}\t按规则 {exp}{'（全码歧义）' if amb else ''}")
    out("词组码规则核验：" + " ".join(f"{k}:{v}" for k, v in sorted(res.items())))

    # 5) freq 统计
    freqs = [f for _, _, f in rows]
    zeros = sum(1 for f in freqs if f == 0)
    above = sum(1 for f in freqs if f > FMAX)
    out(f"freq：0 值 {zeros} 行；max {max(freqs)}；> FMAX({FMAX}) {above} 行；min {min(freqs)}")

    # 6) 收录范围（基本区/扩展 A/B 之外的字；有意保留，见文件头）
    outside = [(w, c) for w, c, _ in rows if not all(is_cjk(ch) for ch in w)]
    chars = sorted({ch for w, _ in outside for ch in w if not is_cjk(ch)})
    sample = " ".join(w for w, _ in outside[:6])
    out(f"收录范围外字符：{len(outside)} 行 / {len(chars)} 个字符（如 {sample}）—— 有意保留（见文件头）")


def compile_opid(tsv: Path, opid: Path, expect_rows: int) -> None:
    """真命令编译 + 两条自检：条数 == 生成行数；stderr 不许有「跳过」警告。"""
    for step, args in (("compile", ["compile", str(tsv), str(opid)]), ("verify", ["verify", str(opid)])):
        proc = subprocess.run(
            # --bin 必给：opi-tools 包里有 opi-ctl + opi-tools 两个二进制，缺它 cargo 直接拒绝
            ["cargo", "run", "--release", "-p", "opi-tools", "--bin", "opi-tools", "--", *args],
            cwd=REPO, capture_output=True, text=True,
        )
        tail = "\n".join(l for l in proc.stdout.strip().splitlines() if not l.startswith(" "))
        print(f"opi-tools {step} {tsv.name}: exit={proc.returncode}\n{tail[-400:]}", file=sys.stderr)
        if proc.returncode != 0:
            sys.exit(f"!! opi-tools {step} 失败：\n{proc.stderr[-1200:]}")
        if step == "compile":
            m = re.search(r"kept entries: (\d+)", proc.stdout)
            if not m or int(m.group(1)) != expect_rows:
                sys.exit(f"!! compile 条数 {m and m.group(1)} != 生成行数 {expect_rows} —— 有行被丢了")
            if "warning: 跳过" in proc.stderr:
                sys.exit(f"!! compile 跳过了行（不许静默丢）：\n{proc.stderr}")
    print(f"产物 {opid.relative_to(REPO)}：{opid.stat().st_size} 字节", file=sys.stderr)


def write_tsv(path: Path, rows: list[tuple[str, str, int]]) -> None:
    write_atomic(str(path), [f"{w}\t{c}\t{f}" for w, c, f in rows])
    print(f"写出 {path.relative_to(REPO)}：{len(rows)} 行", file=sys.stderr)


def main() -> int:
    raw86 = ensure_source("wubi86")
    rows86, stat86 = parse_wubi86(raw86.decode("utf-8", errors="strict"))
    print(f"wubi86 解析：{stat86}", file=sys.stderr)

    raw_rime = ensure_source("wubi98_rime")
    raw_hanci = ensure_source("wubi98_hanci")
    assert raw_rime[:2] == b"\xff\xfe" and raw_hanci[:2] == b"\xff\xfe", "98 源应为 UTF-16LE(带 BOM)"
    rows98, stat98 = parse_wubi98(raw_rime.decode("utf-16"), raw_hanci.decode("utf-16"))
    print(f"wubi98 解析：{stat98}", file=sys.stderr)

    for name, rows, tsv_name in (("wubi86", rows86, "wubi86.tsv"), ("wubi98", rows98, "wubi98.tsv")):
        audit(name, rows)
        tsv = RAW / tsv_name
        write_tsv(tsv, rows)
        compile_opid(tsv, GEN / f"{name}.opid", len(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main())
