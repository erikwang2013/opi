#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""外部词库转换器的离线单测（2026-10-26）。

不联网：用 `crates/engine-data/tests/fixtures/external/` 的几十行摘录和临时文件。
运行：
    python3 -m unittest discover -s scripts/tests -v
    # 或
    python3 scripts/tests/test_external_dict_converters.py
"""
from __future__ import annotations

import sqlite3
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))

import convert_custom_pinyin_dictionary as cpd  # noqa: E402
import convert_rime_dict as rime  # noqa: E402
import external_dict_common as common  # noqa: E402

FIXTURES = ROOT / "crates" / "engine-data" / "tests" / "fixtures" / "external"


def data_lines(path: Path) -> list[str]:
    return [line for line in path.read_text(encoding="utf-8").splitlines() if not line.startswith("#")]


def raw_entries(path: Path) -> list[tuple[str, str]]:
    return [tuple(line.split("\t", 1)) for line in data_lines(path)]


class CommonRulesTest(unittest.TestCase):
    def test_normalize_handles_tones_umlaut_and_separators(self):
        self.assertEqual(common.normalize_pinyin("nǐ hǎo"), "nihao")
        self.assertEqual(common.normalize_pinyin("nǚ ér"), "nver")
        self.assertEqual(common.normalize_pinyin("lü4 lin2"), "lvlin")
        self.assertEqual(common.normalize_pinyin("lu:4"), "lv")
        self.assertEqual(common.normalize_pinyin("xi’an"), "xian")

    def test_weight_matches_rust_round_direction(self):
        self.assertEqual(common.parse_weight("76730"), (76730, True))
        self.assertEqual(common.parse_weight("99.93%"), (99930, True))
        # Rust f64::round 是半数远离零；Python round() 是银行家舍入。
        self.assertEqual(common.parse_weight("0.0005%"), (1, True))
        self.assertEqual(common.parse_weight("6N"), (1000, False))
        self.assertEqual(common.parse_weight(""), (1000, True))
        self.assertEqual(common.parse_weight("4294967296"), (1000, False))

    def test_field_byte_limits_match_loader(self):
        self.assertTrue(common.valid_word("好" * 85))  # 255 B
        self.assertFalse(common.valid_word("好" * 86))  # 258 B
        self.assertTrue(common.valid_pinyin("a" * 255))
        self.assertFalse(common.valid_pinyin("a" * 256))
        self.assertFalse(common.valid_pinyin("ni3hao"))
        self.assertFalse(common.valid_pinyin("ni hao"))

    def test_dedup_keeps_max_freq_and_all_readings(self):
        best: dict[tuple[str, str], common.Entry] = {}
        self.assertTrue(common.add_entry(best, common.Entry("行", "hang", 10, 0)))
        self.assertFalse(common.add_entry(best, common.Entry("行", "hang", 20, 1)))
        self.assertTrue(common.add_entry(best, common.Entry("行", "xing", 30, 2)))
        self.assertEqual(len(best), 2)
        self.assertEqual(best[("行", "hang")].freq, 20)

    def test_byte_cap_truncates_and_counts(self):
        entries = [common.Entry(f"词{i}", f"ci{i}", 1000, i) for i in range(20)]
        out = Path(tempfile.mkdtemp()) / "cap.tsv"

        def header(written: int, drop_entry: int, drop_bytes: int) -> list[str]:
            return ["# test header", f"# written={written} entry={drop_entry} bytes={drop_bytes}"]

        stats = common.write_tsv(
            out,
            entries,
            header,
            max_entries=0,
            max_bytes=128,
            priority=common.shortest_first,
            three_columns=False,
        )
        self.assertGreater(stats.dropped_by_bytes, 0)
        self.assertEqual(len(data_lines(out)), stats.written)
        self.assertLessEqual(out.stat().st_size, 128)
        self.assertIn(f"written={stats.written}", out.read_text(encoding="utf-8"))


class CpdConverterTest(unittest.TestCase):
    def test_fixture_regenerates_byte_identical(self):
        src = FIXTURES / "cpd_dump_tiny.txt"
        expected = (FIXTURES / "cpd_tiny.tsv").read_bytes()
        with tempfile.TemporaryDirectory() as td:
            out = Path(td) / "cpd_tiny.tsv"
            stats, result = cpd.convert(src, out)
            self.assertEqual(result.written, 7)
            self.assertEqual(stats.parsed, 7)
            self.assertEqual(out.read_bytes(), expected, "夹具或转换器改了就必须同步重生成")
        text = expected.decode("utf-8")
        self.assertIn("CC BY-SA 4.0", text)
        self.assertIn(cpd.PIN_COMMIT, text)
        self.assertNotIn("ni'hao", text)
        entries = raw_entries(FIXTURES / "cpd_tiny.tsv")
        self.assertIn(("你好", "nihao"), entries)
        self.assertIn(("女儿", "nver"), entries)
        self.assertTrue(all(len(line.split("\t")) == 2 for line in data_lines(FIXTURES / "cpd_tiny.tsv")))

    def test_sqlite_and_zip_inputs_match_dump_text(self):
        rows = [("你好", "ni'hao"), ("女儿", "nv'er"), ("爸爸", "ba'ba")]
        with tempfile.TemporaryDirectory() as td:
            td = Path(td)
            db = td / "db"
            con = sqlite3.connect(db)
            con.execute("CREATE TABLE entry(_id INTEGER PRIMARY KEY, word TEXT, shortcut TEXT, locale TEXT)")
            con.executemany(
                "INSERT INTO entry(word, shortcut, locale) VALUES (?,?,'zh-CN')", rows
            )
            con.commit()
            con.close()

            dump = td / "rows.txt"
            dump.write_text("".join(f"{w} {p} 0\n" for w, p in rows), encoding="utf-8")
            from_dump = td / "from-dump.tsv"
            from_db = td / "from-db.tsv"
            from_zip = td / "from-zip.tsv"
            cpd.convert(dump, from_dump, max_entries=0, max_output_mb=0)
            cpd.convert(db, from_db, max_entries=0, max_output_mb=0)

            z = td / "gboard.zip"
            with zipfile.ZipFile(z, "w") as zf:
                zf.write(db, "dict/db")
            cpd.convert(z, from_zip, max_entries=0, max_output_mb=0)

            self.assertEqual(data_lines(from_db), data_lines(from_dump))
            self.assertEqual(data_lines(from_zip), data_lines(from_dump))
            self.assertIn("read SQLite entry table", " ".join(
                cpd.convert(db, td / "again.tsv", max_entries=0, max_output_mb=0)[0].notes
            ))

    def test_entry_cap_is_deterministic_and_documented(self):
        src = FIXTURES / "cpd_dump_tiny.txt"
        with tempfile.TemporaryDirectory() as td:
            a, b = Path(td) / "a.tsv", Path(td) / "b.tsv"
            cpd.convert(src, a, max_entries=3, max_output_mb=0)
            cpd.convert(src, b, max_entries=3, max_output_mb=0)
            self.assertEqual(a.read_bytes(), b.read_bytes())
            self.assertEqual(len(data_lines(a)), 3)
            self.assertIn("dropped: 4 (entry cap)", a.read_text(encoding="utf-8"))


class RimeConverterTest(unittest.TestCase):
    def test_fixture_regenerates_byte_identical(self):
        src = FIXTURES / "wanxiang_tiny.dict.yaml"
        expected = (FIXTURES / "wanxiang_tiny.tsv").read_bytes()
        with tempfile.TemporaryDirectory() as td:
            out = Path(td) / "wanxiang_tiny.tsv"
            stats, result = rime.convert([src], out)
            self.assertEqual(stats.parsed, 14)
            self.assertEqual(stats.duplicate, 1)
            self.assertEqual(stats.bad_weight, 1)
            self.assertEqual(result.written, 13)
            self.assertEqual(out.read_bytes(), expected, "夹具或转换器改了就必须同步重生成")
        text = expected.decode("utf-8")
        self.assertIn("CC BY 4.0", text)
        self.assertIn(rime.PIN_COMMIT, text)
        self.assertIn(("你好", "nihao", "76730"), [tuple(x.split("\t")) for x in data_lines(FIXTURES / "wanxiang_tiny.tsv")])
        self.assertIn(("行", "hang", "16297"), [tuple(x.split("\t")) for x in data_lines(FIXTURES / "wanxiang_tiny.tsv")])
        self.assertIn(("行", "xing", "92900"), [tuple(x.split("\t")) for x in data_lines(FIXTURES / "wanxiang_tiny.tsv")])
        self.assertIn(("啊", "a", "759839"), [tuple(x.split("\t")) for x in data_lines(FIXTURES / "wanxiang_tiny.tsv")])
        self.assertTrue(all(len(line.split("\t")) == 3 for line in data_lines(FIXTURES / "wanxiang_tiny.tsv")))

    def test_parser_rules_and_duplicate_weight(self):
        text = """# comment
---
name: t
import_tables:
  - x
...
你好\tni hao\t10
你好\tnǐ hǎo\t20
贝克\tbèi kè
百分比\tbǎi fēn bǐ\t12.345%
坏权重\thuài quán zhòng\t6N
行\tháng\t5
行\txíng\t6
反\\斜杠\tfanxie\t1
坏\tni~hao\t1
"""
        with tempfile.TemporaryDirectory() as td:
            src = Path(td) / "t.dict.yaml"
            src.write_text(text, encoding="utf-8")
            stats, result = rime.convert([src], Path(td) / "t.tsv", max_entries=0, max_output_mb=0)
            self.assertEqual(stats.parsed, 9)
            self.assertEqual(stats.duplicate, 1)  # 你好 tone variant
            self.assertEqual(stats.bad_weight, 1)
            self.assertEqual(stats.bad_word, 1)  # 词里含反斜杠（Rime 展开转义）
            self.assertEqual(stats.bad_pinyin, 1)  # ni~hao 去调后仍是非法字符
            self.assertEqual(result.written, 6)
            entries = {tuple(line.split("\t")) for line in data_lines(Path(td) / "t.tsv")}
            self.assertIn(("你好", "nihao", "20"), entries)
            self.assertIn(("贝克", "beike", "1000"), entries)
            self.assertIn(("百分比", "baifenbi", "12345"), entries)
            self.assertIn(("坏权重", "huaiquanzhong", "1000"), entries)
            self.assertIn(("行", "hang", "5"), entries)
            self.assertIn(("行", "xing", "6"), entries)

    def test_unclosed_front_matter_and_empty_output_are_errors(self):
        with tempfile.TemporaryDirectory() as td:
            bad = Path(td) / "bad.dict.yaml"
            bad.write_text("---\nname: x\n你好\tni hao\t1\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "front-matter"):
                rime.convert([bad], Path(td) / "bad.tsv")
            empty = Path(td) / "empty.dict.yaml"
            empty.write_text("---\nname: x\n...\n这行没有tab\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "没有一条可用词条"):
                rime.convert([empty], Path(td) / "empty.tsv")

    def test_entry_cap_keeps_highest_weight(self):
        with tempfile.TemporaryDirectory() as td:
            src = Path(td) / "t.dict.yaml"
            src.write_text(
                "---\nname: t\n...\n低\t di\t1\n高\tgao\t999\n中\tzhong\t50\n",
                encoding="utf-8",
            )
            out = Path(td) / "t.tsv"
            rime.convert([src], out, max_entries=2, max_output_mb=0)
            self.assertEqual(data_lines(out), ["高\tgao\t999", "中\tzhong\t50"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
