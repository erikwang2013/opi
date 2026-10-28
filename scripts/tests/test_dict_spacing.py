#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""词库 freq 位置编码的离线门禁（`dict_spacing.section_freqs`，2026-10-28 优化4）。

背景：旧式 `spacing = FMAX // n` 让条目数一变全表 freq 重算 —— 补 7 条读音，
trad 两个 TSV 各 ~9.6 万行 diff。本门禁钉住三件事：
  1. **层尾追加**条目不改变任何既有 (字,读音)/(词,拼音) 的 freq（含「追加单字不顺延词组」）；
  2. freq 严格递减 ⇒ 排序键顺序不变，且无并列（MmapDictionary::query_prefix_top 的阈值法前提）；
  3. 越界/交叠**报错**而不是截断或归零。

运行：python3 -m unittest discover -s scripts/tests -v
"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

import dict_spacing as ds  # noqa: E402
import gen_trad_dict as trad  # noqa: E402

SINGLES = [("你", "ni"), ("泥", "ni"), ("擬", "ni"), ("呢", "ni"), ("腻", "ni")]
PHRASES = [("你好", "nihao"), ("泥土", "nitu"), ("擬定", "niding"), ("呢喃", "ninan")]


def freqs(n_s: int, n_p: int, cap: int = 1000, base: int = 100):
    return ds.section_freqs(n_s, n_p, cap, base)


class AppendStabilityTest(unittest.TestCase):
    def test_appended_phrases_keep_every_existing_freq(self):
        before_s, before_p = freqs(len(SINGLES), len(PHRASES))
        extra_phrases = [("泥石流", "nishiliu"), ("拟人", "niren")]
        after_s, after_p = freqs(len(SINGLES), len(PHRASES) + len(extra_phrases))

        self.assertEqual(before_s, after_s, "追加词组不应改动单字层 freq")
        self.assertEqual(
            dict(zip(PHRASES, before_p)),
            dict(zip(PHRASES, after_p[: len(PHRASES)])),
            "追加词组不应改动既有词组的 freq",
        )

    def test_appended_single_readings_keep_phrase_freqs(self):
        """单字层尾追加（luna 的 B 类读音走 tail 段）不得顺延词组层。"""
        before_s, before_p = freqs(len(SINGLES), len(PHRASES))
        after_s, after_p = freqs(len(SINGLES) + 2, len(PHRASES))

        self.assertEqual(
            dict(zip(SINGLES, before_s)),
            dict(zip(SINGLES, after_s[: len(SINGLES)])),
            "追加单字读音不应改动既有读音的 freq",
        )
        self.assertEqual(before_p, after_p, "单字层增长不应顺延词组层 freq")


class OrderTest(unittest.TestCase):
    def test_freqs_strictly_decreasing_and_layers_do_not_overlap(self):
        single, phrase = freqs(len(SINGLES), len(PHRASES), cap=1000, base=100)
        self.assertEqual(len(single), len(SINGLES))
        self.assertEqual(len(phrase), len(PHRASES))
        self.assertTrue(all(a > b for a, b in zip(single, single[1:])))
        self.assertTrue(all(a > b for a, b in zip(phrase, phrase[1:])))
        self.assertGreater(single[-1], phrase[0], "词组必须整体排在单字之后")
        self.assertLessEqual(max(single), ds.FMAX)
        self.assertGreater(min(phrase), 0)

    def test_order_of_rows_is_unchanged(self):
        single, phrase = freqs(len(SINGLES), len(PHRASES))
        ranked = sorted(zip(SINGLES, single), key=lambda t: -t[1]) + sorted(
            zip(PHRASES, phrase), key=lambda t: -t[1]
        )
        self.assertEqual(ranked, list(zip(SINGLES, single)) + list(zip(PHRASES, phrase)))


class OverflowTest(unittest.TestCase):
    def test_phrase_layer_overflow_raises_not_truncates(self):
        with self.assertRaisesRegex(RuntimeError, "固定容量"):
            freqs(10, 100, cap=200, base=100)  # 100 + 100 = 200 ≥ cap

    def test_singles_over_phrase_base_raises(self):
        with self.assertRaisesRegex(RuntimeError, "基址"):
            freqs(101, 1, cap=1000, base=100)


class GeneratorConstantsTest(unittest.TestCase):
    def test_committed_trad_tsvs_fit_trad_capacity(self):
        """入库 TSV 的行数必须仍在生成器的固定容量内（否则下次重编才炸，见越界报错）。"""
        raw = SCRIPTS.parent / "data" / "raw"
        n_singles = len((raw / "trad_hanzi.tsv").read_text(encoding="utf-8").splitlines())
        n_phrases = len((raw / "trad_phrases.tsv").read_text(encoding="utf-8").splitlines())
        ds.section_freqs(n_singles, n_phrases, trad.TRAD_MAX_ENTRIES, trad.TRAD_PHRASE_BASE)


if __name__ == "__main__":
    unittest.main()
