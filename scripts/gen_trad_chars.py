#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""生成「繁体专属字」表（产物提交入库；离线构建不重跑本脚本）。

产物：`data/raw/trad_chars.tsv`，两列 `繁体字\t对应的简体字`，按码位升序。
用途：引擎在**简体模式**下把含繁体专属字的候选排到简体候选之后 ——
用户裁决（2026-10-13）：「所有端切换简体就用简体数据，切换繁体就用繁体数据，
默认简体，如果是混合就优先显示简体」。

判据 = **仓里既有的 `hanzi_freq.trad_only`**（字**不在 GB2312** 且 `kSimplifiedVariant`
指向 GB2312 字）—— 一个字都不新造：
⚠️ 别用「变体与自身不同」那条更松的写法 —— 它会把 著/覆/乾/藉/瑙/阪 判成繁体
（它们有生僻异构简形，但本身在简体里照常用），`hanzi_freq.trad_only` 的文档注明实测误报 35 条。
同一个判据在 `gen_luna_dict.py` 里是**拒收**（简体库不得收繁体字形），在 `gen_extra_phrases.py`
里也是拒收 —— 那份守卫只覆盖 pinyin_simp 那一路（2026-10-11 词库整合后 rime-ice 等来源
绕过了它，实测简体库候选里出现 國/學/車/電）。本表是**运行期**的兜底：数据侧漏了就靠它排后。

⚠️ 为什么不在引擎里硬编一张小表：本仓退役过一张 120 字的 `COMMON_TRAD` 白名单
（见 `gen_trad_dict.py` 的「人工表退役记录」）—— 理由同一条：手表覆盖不全、且会静默过期。
数据源与版本 pin 在 `hanzi_freq.py`（Unihan 18.0.0 + SHA-256），换版本时本表随之重生成。

范围：与词库同一份 `CJK_RANGES`（到扩展 B 为止，理由见 `hanzi_freq.py` 注释）。

用法（离线可用；Unihan.zip 可来自缓存）：
    python3 scripts/gen_trad_chars.py [/path/to/Unihan.zip]
    缺省从 www.unicode.org 下（校验 pin 的 SHA-256）。
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from hanzi_freq import CJK_RANGES, gb2312_levels, load_variants, trad_only, unihan_member  # noqa: E402


def trad_only_rows(text: str) -> list[tuple[str, str]]:
    """Unihan_Variants.txt 文本 → [(繁体字, 简体字)]（升序、去重、只收 CJK 范围内）。"""
    _trad_of, simp_of = load_variants(text)
    # gb2312_levels() 回 (一级, 二级) 两张表 —— 合起来才是 GB2312 汉字集。
    level1, level2 = gb2312_levels()
    gb = set(level1) | set(level2)
    rows: list[tuple[str, str]] = []
    for ch in simp_of:
        if not any(lo <= ord(ch) <= hi for lo, hi in CJK_RANGES):
            continue
        if not trad_only(ch, gb, simp_of):
            continue
        # 简体代表形：变体表里**在 GB 集合内**的第一个（trad_only 已保证存在；
        # 列表按权威位次排，取首个，与 gen_trad_dict 的「字形取向」同一取法）。
        simp = next(v for v in simp_of[ch] if v in gb)
        rows.append((ch, simp))
    return sorted(set(rows))


def _prime_from_local_zip(zip_path: str) -> None:
    """把本地 Unihan.zip 预置进 `hanzi_freq` 的进程内缓存（离线复现用）。
    仍**校验 pin 的 SHA-256** —— 缓存的字节与本表的可复现性是同一件事。"""
    import hashlib
    import io
    import zipfile

    import hanzi_freq

    data = open(zip_path, "rb").read()
    got = hashlib.sha256(data).hexdigest()
    if got != hanzi_freq.UNIHAN_ZIP_SHA256:
        raise RuntimeError(f"本地 Unihan.zip 摘要与 pin 不符：{got}")
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        hanzi_freq._ZIP["Unihan_Variants.txt"] = zf.read("Unihan_Variants.txt").decode("utf-8")


def main() -> int:
    if len(sys.argv) > 1:
        _prime_from_local_zip(sys.argv[1])
    text = unihan_member("Unihan_Variants.txt")
    rows = trad_only_rows(text)
    out = "".join(f"{t}\t{s}\n" for t, s in rows)
    dest = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "raw", "trad_chars.tsv")
    with open(dest, "w", encoding="utf-8") as f:
        f.write(out)
    print(f"写出 {dest}：{len(rows)} 条")
    # 抽查（写进输出，方便肉眼确认这份表没跑偏）
    probe = {"國": "国", "學": "学", "臺": "台", "發": "发", "裏": "里", "你": None, "的": None}
    table = dict(rows)
    for t, want in probe.items():
        got = table.get(t)
        ok = "✓" if got == want else "✗"
        print(f"  {ok} {t} → {got}（期望 {want}）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
