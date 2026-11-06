#!/usr/bin/env python3
"""Verifier sweep: every realbench test compiled whole (`--tier=clift`) and run to 1 ns with
`YAVS_VERIFY_IR=1`, so the IR verifier checks the finished code of every function the design
has. Meant for a binary with debug assertions (the gate of an optimiser or lowering change);
on a plain build the switch runs the same verifier checks. A test whose source is not there
(a private checkout, a program image that needs a cross compiler) is listed as not run.

  tools/realbench/sweep.py BIN [--jobs 4] [--filter RE] [--no-sandbox]

Exit 1 when a run reports a broken function or panics, or exits with a status the run to
1 ns does not explain. Stdlib only (the OSVVM driver tests need suites/requirements.txt).
"""
import argparse, concurrent.futures as cf, os, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run as R  # noqa: E402

NO_SANDBOX = False
BAD = re.compile(r"broke |panicked|read before it is written|this is a bug in yavs")


def one(t, B, key, worlds):
    n = t["name"]
    d = worlds / n
    try:
        if t.get("image"):
            t["image_dir"] = str(R.P.build(t["image"]))
        R.check_sources([t])
        R.make_pristine(t, "yavs", B, key, d)
        s = R.yavs_script(B, dict(t, quiet=False), d, "clift", stop="1ns", wrap="exec ")
        cap = [] if NO_SANDBOX else ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=8G"]
        p = subprocess.run(["timeout", "1500", *cap, "bash", "-c", s], cwd=d, capture_output=True, text=True, errors="replace",
                           env={**os.environ, "YAVS_VERIFY_IR": "1"})
        text = p.stdout + p.stderr + "".join((d / x).read_text(errors="replace") for x in (R.OUT, R.ERR) if (d / x).exists())
        bad = [l[:300] for l in text.splitlines() if BAD.search(l)]
        return n, ("broken" if bad else "ok"), p.returncode, bad[:3]
    except R.Fail as e:
        return n, "not run", None, [str(e).splitlines()[-1][:300]]
    finally:
        shutil.rmtree(d, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("bin")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--filter", default="")
    ap.add_argument("--no-sandbox", action="store_true", help="no systemd-run memory cap (CI)")
    a = ap.parse_args()
    global NO_SANDBOX
    NO_SANDBOX = a.no_sandbox
    B = str(Path(a.bin).resolve())
    key = R.B.binary_key(B)
    tests = [t for t in R.load_tests() if re.search(a.filter, t["name"]) and t["category"] != "cpubench"]
    for t in tests:  # libraries one at a time: tests share them
        try:
            for m in t["corpus"]:
                R.ensure_base("yavs", m, B, key)
        except R.Fail as e:
            print(f"{t['name']}: libraries not built: {str(e).splitlines()[-1][:200]}", flush=True)
    worlds = R.CACHE / "sweep" / str(os.getpid())
    worlds.mkdir(parents=True)
    rows = []
    try:
        with cf.ThreadPoolExecutor(a.jobs) as ex:
            for r in ex.map(lambda t: one(t, B, key, worlds), tests):
                rows.append(r)
                print(f"{r[0]:<34} {r[1]:<8} {'' if r[2] is None else 'exit ' + str(r[2])}  {' | '.join(r[3])}", flush=True)
    finally:
        shutil.rmtree(worlds, ignore_errors=True)
    by = {k: sum(1 for r in rows if r[1] == k) for k in ("ok", "broken", "not run")}
    print(f"{len(rows)} tests: {by['ok']} ok, {by['broken']} broken, {by['not run']} not run")
    sys.exit(1 if by["broken"] else 0)


if __name__ == "__main__":
    main()
