#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""语义联想小表生成器的离线单测（Task 10）。

不依赖 33MB 词库：根选择与后缀收集全用纯函数夹具；唯一的集成点是
`data/generated/fallback.opid`（35 条，文件缺失则跳过）。

运行：
    python3 scripts/tests/test_gen_assoc_table.py -v
    # 或
    python3 -m unittest discover -s scripts/tests -v
"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))

import gen_assoc_table as g  # noqa: E402

FALLBACK = ROOT / "data" / "generated" / "fallback.opid"


class RootSelectionTest(unittest.TestCase):
    def test_roots_are_2_3_cjk_ranked_by_phrase_occurrences(self):
        words = {"今天": 900, "中国": 800, "中国人": 700, "的": 1}
        phrases = ["今天回家", "今天下午", "中国声音", "中国政策"]
        roots = g.select_roots(words, phrases, roots=2)
        self.assertEqual(roots, ["中国", "今天"])  # 同次数按字节序


class LoadPhrasesTest(unittest.TestCase):
    def test_takes_first_column_and_skips_non_cjk(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "p.tsv"
            path.write_text(
                "今天回家\tjintianhuijia\t100\nabc\tx\t1\n\tx\t1\n好的\thao\t1\n",
                encoding="utf-8",
            )
            phrases, skipped = g.load_phrases(path)
        self.assertEqual(phrases, ["今天回家", "好的"])
        self.assertEqual(skipped, 2, "非 CJK 与空首列各计一次")


class CollectRowsTest(unittest.TestCase):
    def test_suffixes_skip_particles_and_dictionary_leaves(self):
        rows = g.collect_rows(
            roots=["今天"],
            words={"今天回家": 600, "今天": 900},
            phrases=["今天回家", "今天了", "今天的", "今天下午"],
            cap=3,
        )
        pairs = [(r.root, r.suffix) for r in rows]
        self.assertNotIn(("今天", "了"), pairs)
        self.assertNotIn(("今天", "的"), pairs)
        # 今天回家 已是词库词 → 不写入
        self.assertNotIn(("今天", "回家"), pairs)
        self.assertIn(("今天", "下午"), pairs)

    def test_output_is_sorted_and_unique(self):
        rows = g.collect_rows(
            roots=["今天", "中国"], words={}, phrases=["今天下午", "中国政策"], cap=3
        )
        lines = g.format_rows(rows)
        self.assertEqual(lines, sorted(lines))
        pairs = [tuple(line.split("\t")[:2]) for line in lines]
        self.assertEqual(len(pairs), len(set(pairs)), "root+suffix 不得重复")


class ReadOpidTest(unittest.TestCase):
    def test_reads_entries_and_ignores_tail(self):
        if not FALLBACK.is_file():
            self.skipTest(f"缺 {FALLBACK}")
        rows = g.read_opid(FALLBACK)
        self.assertEqual(len(rows), 35)


if __name__ == "__main__":
    unittest.main()
