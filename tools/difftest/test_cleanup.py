#!/usr/bin/env python3
"""Check that difftest.py leaves no scratch directory after a passing run and keeps only the
failing case after a failing one. Needs no simulator: the candidate is `echo hi`."""
import os, subprocess, sys, tempfile, unittest
from pathlib import Path

DIFFTEST = Path(__file__).resolve().parent / "difftest.py"


def run(corpus, cache):
    env = {**os.environ, "DIFFTEST_CACHE": str(cache)}
    return subprocess.run([sys.executable, str(DIFFTEST), "--candidate", "echo hi", "--no-sandbox", "--corpus", str(corpus)],
                          env=env, capture_output=True, text=True)


class Cleanup(unittest.TestCase):
    def case(self, corpus, name, expected):
        (corpus / f"{name}.vhd").write_text("-- difftest: top=t\n")
        (corpus / f"{name}.expected").write_text(expected)

    def test_pass_leaves_nothing_and_fail_keeps_only_itself(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus, cache = Path(tmp) / "corpus", Path(tmp) / "cache"
            corpus.mkdir()
            self.case(corpus, "good", "exit\t0\nout\thi\n")
            p = run(corpus, cache)
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
            self.assertFalse(cache.exists(), list(cache.rglob("*")))
            self.case(corpus, "bad", "exit\t0\nout\tno\n")
            p = run(corpus, cache)
            self.assertEqual(p.returncode, 1, p.stdout + p.stderr)
            self.assertIn(str(cache), p.stderr)
            kept = {str(f.relative_to(cache).parts[1]) for f in cache.rglob("stdout.txt")}
            self.assertEqual(kept, {"bad"})


if __name__ == "__main__":
    unittest.main()
