#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""单字读音覆盖表的解析/合并（离线）。"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

import single_readings as sr  # noqa: E402


class LoadTest(unittest.TestCase):
    def test_luna_rows(self):
        got = sr.load("luna", {"儿", "兒", "嗯", "琢", "寻"})
        self.assertEqual(got["儿"], ["r"])
        self.assertEqual(got["嗯"], ["n", "ng"])
        self.assertEqual(set(got), {"儿", "兒", "嗯", "琢", "寻"})

    def test_trad_rows(self):
        got = sr.load("trad", {"似", "匙", "臂", "抹", "嗯", "吶", "尋"})
        self.assertEqual(set(got), {"似", "匙", "臂", "抹", "嗯", "吶", "尋"})
        self.assertEqual(got["嗯"], ["n"])

    def test_unknown_char_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "不在 luna 单字表"):
            sr.load("luna", {"儿"})  # 表里还有别的字没有列进 known

    def test_merge_only_new_pairs_sink_to_tail(self):
        tail: set[tuple[str, str]] = set()
        readings, added = sr.merge(
            "儿", ["er"], {"ren"}, {"儿": ["r", "er", "ren"]}, tail
        )
        self.assertEqual(readings, ["er", "r", "ren"])
        self.assertEqual(added, 1)
        self.assertEqual(tail, {("儿", "r")})


if __name__ == "__main__":
    unittest.main()
