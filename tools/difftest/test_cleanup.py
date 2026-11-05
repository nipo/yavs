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


class Setup(unittest.TestCase):
    def fail(self, cand, **env):
        with tempfile.TemporaryDirectory() as tmp:
            corpus = Path(tmp) / "corpus"
            corpus.mkdir()
            (corpus / "a.vhd").write_text("-- difftest: top=t\n")
            conf = Path(tmp) / "none.conf"
            e = {k: v for k, v in os.environ.items() if k != "YAVS_BIN"}
            e.update(YAVS_SIMS_CONF=str(conf), PATH="/nonexistent", **env)
            p = subprocess.run([sys.executable, str(DIFFTEST), "--candidate", cand, "--no-sandbox", "--corpus", str(corpus)],
                               env=e, capture_output=True, text=True)
            self.assertEqual(p.returncode, 1, p.stdout)
            self.assertNotIn("FAIL", p.stdout)  # stopped at setup, ran no test
            return p.stderr

    def test_unconfigured_label_and_missing_yavs_stop_at_setup(self):
        self.assertIn("no `difftest` key in tools/sims.local.conf", self.fail("sim1"))
        self.assertIn("did you mean yavs-interp?", self.fail("yavs-intrp"))
        self.assertIn("YAVS_BIN", self.fail("yavs"))
        self.assertIn("YAVS_BIN", self.fail("yavs", YAVS_BIN="/nonexistent/yavs"))


if __name__ == "__main__":
    unittest.main()
