#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""gen_luna_dict 的 luna 上游 pin 校验（离线）。

运行：
    python3 -m unittest discover -s scripts/tests -v
"""
from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

import gen_luna_dict as g  # noqa: E402


class VerifyFileTest(unittest.TestCase):
    def _write(self, data: bytes) -> str:
        with tempfile.NamedTemporaryFile("wb", delete=False) as f:
            f.write(data)
            return f.name

    def test_matching_digest_passes(self):
        path = self._write(b"luna")
        try:
            g.verify_file(path, hashlib.sha256(b"luna").hexdigest())
        finally:
            Path(path).unlink()

    def test_mismatch_raises(self):
        path = self._write(b"changed upstream")
        try:
            with self.assertRaisesRegex(RuntimeError, "摘要与 pin 不符"):
                g.verify_file(path, g.LUNA_SHA256)
        finally:
            Path(path).unlink()


if __name__ == "__main__":
    unittest.main()
