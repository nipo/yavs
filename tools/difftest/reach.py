#!/usr/bin/env python3
"""Reach check: did a change's new path run under test? Runs the difftest corpus with
`YAVS_COUNTERS=1` on one or more candidate templates and sums the named work counters
(`counters: NAME=VALUE` on stderr). Exits 1 if a named counter is zero over the whole set.
Output is not compared here: difftest does that. Stdlib only.

  tools/difftest/reach.py --counters spec_subs,gate_checked [--candidate yavs-clift-check ...] [--filter RE]
"""
import argparse, concurrent.futures as cf, os, re, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import difftest as D  # noqa: E402

LINE = re.compile(r"^counters: (\S+?)=(\d+)$", re.M)


def counters(err):
    """Sum of each counter in one run's stderr (a counter printed twice, by two runs of a template, adds)."""
    out = {}
    for k, v in LINE.findall(err):
        out[k] = out.get(k, 0) + int(v)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--counters", required=True, help="comma list of counter names that must be above zero somewhere")
    ap.add_argument("--candidate", action="append", help="difftest template name (repeatable); default yavs-clift-check")
    ap.add_argument("--filter", default="", help="regex on the corpus-relative path")
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    names = [n for n in a.counters.split(",") if n]
    cands = a.candidate or ["yavs-clift-check"]
    for c in cands:
        if c not in D.TEMPLATES:
            raise SystemExit(f"reach: unknown candidate {c!r}; known: {', '.join(sorted(D.TEMPLATES))}")
    tests = sorted(p for p in D.CORPUS.rglob("*.vhd") if re.search(a.filter, str(p.relative_to(D.CORPUS))))
    if not tests:
        raise SystemExit(f"reach: no corpus test matches {a.filter!r}")
    total = {(c, n): [0, 0] for c in cands for n in names}  # sum, tests where it is above zero

    def job(c, vhd):
        hdr = D.parse_header(vhd)
        work = D.CACHE / "reach" / c / vhd.relative_to(D.CORPUS).with_suffix("")
        _, _, err = D.run("export YAVS_COUNTERS=1; " + D.TEMPLATES[c], vhd, hdr, work, 1, None)
        D.drop(work)
        return c, counters(err)

    D.guard(D.CACHE)
    with cf.ThreadPoolExecutor(min(a.jobs, D.MAX_JOBS)) as ex:
        for c, got in ex.map(lambda cv: job(*cv), [(c, v) for c in cands for v in tests]):
            for n in names:
                if got.get(n, 0):
                    total[(c, n)][0] += got[n]
                    total[(c, n)][1] += 1
    w = max(len(n) for n in names)
    print(f"{len(tests)} corpus tests")
    print(f"{'counter':<{w}}  {'candidate':<20}  {'total':>14}  tests where above zero")
    for (c, n), (s, k) in total.items():
        print(f"{n:<{w}}  {c:<20}  {s:>14}  {k}")
    missing = sorted({n for n in names if all(total[(c, n)][0] == 0 for c in cands)})
    if missing:
        print(f"reach: never above zero: {', '.join(missing)} (the path did not run under test, or the name is wrong: "
              "`YAVS_COUNTERS=1 yavs run ...` prints every counter)", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    if os.environ.get("REACH_SELFTEST"):
        assert counters("counters: a=1\nx\ncounters: b=0\ncounters: a=2\n") == {"a": 3, "b": 0}
        print("ok")
    else:
        main()
