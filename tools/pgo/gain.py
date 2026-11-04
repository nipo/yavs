#!/usr/bin/env python3
"""A profile-guided yavs against a plain build of the same flags, on designs the training never saw.

  python3 -I tools/pgo/gain.py PLAIN PGO [--rounds N] [--designs a,b] [--scratch DIR] [--out FILE.json]
  python3 -I tools/pgo/gain.py --selftest

Three arms take turns in every round, so a slow minute of the machine hits all of them: PLAIN, PGO, and PLAIN again as a
control whose ratio shows how far this machine's noise goes. Each arm runs a fresh copy of its binary: timing the linker's output
against the unpacked archive read 1.12 for analysis on Linux runners, and copies of both read 1.00 (why is not established). A round times, in a fresh work directory, `analyze` (repeated), the first
`run` at `--tier=auto` (cold cache) and a second one (warm). Each arm's output and exit status must equal PLAIN's: a difference
fails the script. Where `perf stat` can count user instructions (Linux, a counter in the machine) one more pass counts them.
"""
import argparse
import json
import math
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

DESIGNS = Path(__file__).resolve().parents[1] / "bench" / "designs"
# tools/bench/designs is never in the training (tools/pgo/AGENTS.md): RTL, records, protected types, text, numeric_std, calls. The
# generics make a run last about a second or more on a CI runner: the default sizes end too fast to time there. `elab_heavy` is left
# out: a training case was written after looking at it (docs/decisions.md "Profile-guided release archives"), so it is not held out.
DESIGNS_RUN = {
    "rtl_bus": "CYC=200000", "rtl_chain": "CYC=20000", "rtl_ram": "CYC=1000000", "rec_pair": "CYCLES=400000",
    "protected_queue": "N=40000", "textio_heavy": "N=1000000", "ieee_heavy": "N=2000000", "strings_heavy": "N=200000",
    "calls": "N=50000000", "compute_h2h": "N=100000000",
}
ANALYZE_REPEATS = 20  # one analysis of a small file is a few ms, mostly start-up
PHASES = ("analyze", "run-cold", "run-warm")
TIMEOUT = 600


def commands(yavs, design, work):
    w = f"--work=work:{work / 'lib'}"
    gen = DESIGNS_RUN.get(design)
    run_cmd = [yavs, "run", "--std=2008", w, "--tier=auto", *([f"-g{gen}"] if gen else []), f"bench_{design}"]
    return {"analyze": [yavs, "analyze", "--std=2008", w, str(DESIGNS / f"{design}.vhd")], "run-cold": run_cmd, "run-warm": run_cmd}


def fresh(path):
    shutil.rmtree(path, ignore_errors=True)
    path.mkdir(parents=True)
    return path


def run(cmd, cwd, env, prefix=(), repeat=1):
    t0 = time.perf_counter()
    try:
        for _ in range(repeat):
            r = subprocess.run([*prefix, *cmd], cwd=cwd, env=env, capture_output=True, timeout=TIMEOUT)
            if r.returncode:
                break
    except subprocess.TimeoutExpired:
        sys.exit(f"gain: timeout after {TIMEOUT} s: {' '.join(cmd)}")
    return time.perf_counter() - t0, r.returncode, r.stdout


def perf_prefix(out):
    """`perf stat` writing user instructions to `out`, or nothing where this machine has no such counter."""
    if not shutil.which("perf"):
        return ()
    r = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", sys.executable, "-c", "pass"], capture_output=True, text=True)
    if r.returncode or not re.match(r"\d+,", r.stderr.strip()):
        return ()
    return ("perf", "stat", "-x,", "-e", "instructions:u", "-o", str(out))


def cpu_name():
    try:
        m = re.search(r"^model name\s*:\s*(.*)$", Path("/proc/cpuinfo").read_text(), re.M)
    except OSError:
        m = None
    return f"{m.group(1) if m else platform.processor()}, {os.cpu_count()} logical CPUs"


def summarise(a, b):
    """Paired ratios b/a of one cell: the median, and the spread of the pairs."""
    pairs = [y / x for x, y in zip(a, b)]
    return {"ratio": statistics.median(pairs), "lo": min(pairs), "hi": max(pairs)}


def geomean(xs):
    return math.exp(sum(math.log(x) for x in xs) / len(xs))


def measure(plain, pgo, names, rounds, scratch):
    bins = fresh(scratch / "bin")
    arms = {}
    for k, y in (("plain", plain), ("pgo", pgo), ("control", plain)):
        arms[k] = str(bins / f"{k}{Path(y).suffix}")
        shutil.copy2(y, arms[k])
    envs = {k: dict(os.environ, NO_COLOR="1", YAVS_STD_CACHE=str(fresh(scratch / f"std-{k}"))) for k in arms}
    times = {d: {p: {k: [] for k in arms} for p in PHASES} for d in names}
    order = list(arms)
    for k in arms:  # fill the std/ieee stores, so that no arm times them
        run(commands(arms[k], names[0], fresh(scratch / "w"))["analyze"], scratch / "w", envs[k])
    for n in range(rounds):
        for d in names:
            base = None
            for k in order[n % 3:] + order[: n % 3]:
                work = fresh(scratch / "w")
                outs = {}
                for phase, cmd in commands(arms[k], d, work).items():
                    t, rc, out = run(cmd, work, envs[k], repeat=ANALYZE_REPEATS if phase == "analyze" else 1)
                    times[d][phase][k].append(t)
                    outs[phase] = (rc, out)
                    if rc:
                        sys.exit(f"gain: {k} {d} {phase}: exit status {rc}")
                # Compare the runs only: `analyze` prints nothing to compare, and a different `run` is a wrong result.
                if base is None:
                    base = (k, outs)
                elif outs["run-warm"] != base[1]["run-warm"] or outs["run-cold"] != base[1]["run-cold"]:
                    sys.exit(f"gain: {d}: the output of {k} differs from {base[0]}")
        print(f"gain: round {n + 1}/{rounds} done", flush=True)
    return times, arms


def count_instructions(arms, names, scratch):
    stat = scratch / "perf.txt"
    prefix = perf_prefix(stat)
    if not prefix:
        return None
    arms = {k: arms[k] for k in ("plain", "pgo")}
    counts = {d: {p: {} for p in PHASES} for d in names}
    for d in names:
        for k, y in arms.items():
            env = dict(os.environ, NO_COLOR="1", YAVS_STD_CACHE=str(scratch / f"std-{k}"))
            work = fresh(scratch / "w")
            for phase, cmd in commands(y, d, work).items():
                run(cmd, work, env, prefix)
                counts[d][phase][k] = int(re.search(r"^(\d+),", stat.read_text(), re.M).group(1))
    return counts


def report(times, counts, rounds):
    res = {"cpu": cpu_name(), "rounds": rounds, "cells": {}, "geomean": {}}
    lines = [f"CPU: {res['cpu']}", "", f"Wall-clock ratio PGO / plain (median of {rounds} paired runs, lowest to highest pair); control is plain / plain, the noise.", "",
             "| design | phase | plain s | PGO / plain | pairs | control | instructions PGO / plain |", "|---|---|---:|---:|---:|---:|---:|"]
    for d, phases in times.items():
        for p, t in phases.items():
            s, c = summarise(t["plain"], t["pgo"]), summarise(t["plain"], t["control"])
            ins = counts[d][p]["pgo"] / counts[d][p]["plain"] if counts else None
            res["cells"][f"{d}/{p}"] = {"plain_s": statistics.median(t["plain"]), **s, "control": c["ratio"], "instructions": ins}
            lines.append(f"| {d} | {p} | {statistics.median(t['plain']):.3f} | {s['ratio']:.3f} | {s['lo']:.3f} to {s['hi']:.3f} | {c['ratio']:.3f} | "
                         + (f"{ins:.3f}" if ins else "no counter") + " |")
    lines += ["", "| phase | geometric mean PGO / plain | worst design | control | instructions |", "|---|---:|---:|---:|---:|"]
    for p in PHASES:
        cells = [res["cells"][f"{d}/{p}"] for d in times]
        g = {"ratio": geomean([c["ratio"] for c in cells]), "worst": max(c["ratio"] for c in cells),
             "control": geomean([c["control"] for c in cells]),
             "instructions": geomean([c["instructions"] for c in cells]) if counts else None}
        res["geomean"][p] = g
        lines.append(f"| {p} | {g['ratio']:.3f} | {g['worst']:.3f} | {g['control']:.3f} | " + (f"{g['instructions']:.3f}" if counts else "no counter") + " |")
    return res, "\n".join(lines)


def selftest():
    s = summarise([2.0, 4.0, 10.0], [1.0, 3.0, 10.0])
    assert s == {"ratio": 0.75, "lo": 0.5, "hi": 1.0}, s
    assert abs(geomean([0.5, 2.0]) - 1.0) < 1e-12
    t = {"d": {p: {"plain": [1.0], "pgo": [0.5], "control": [1.0]} for p in PHASES}}
    res, text = report(t, None, 1)
    assert res["geomean"]["analyze"]["ratio"] == 0.5 and "no counter" in text
    print("gain: selftest ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("plain", nargs="?")
    ap.add_argument("pgo", nargs="?")
    ap.add_argument("--rounds", type=int, default=7)
    ap.add_argument("--designs", default=",".join(DESIGNS_RUN))
    ap.add_argument("--scratch", help="work directory (default: a temporary one)")
    ap.add_argument("--out", help="write the results as JSON here")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if not (a.plain and a.pgo):
        ap.error("needs PLAIN and PGO")
    plain, pgo = str(Path(a.plain).resolve()), str(Path(a.pgo).resolve())
    names = a.designs.split(",")
    for d in names:
        if not (DESIGNS / f"{d}.vhd").is_file():
            sys.exit(f"gain: no design {d} in {DESIGNS}")
    scratch = Path(a.scratch).resolve() if a.scratch else Path(tempfile.mkdtemp(prefix="yavs-gain-"))
    fresh(scratch)
    t0 = time.time()
    times, arms = measure(plain, pgo, names, a.rounds, scratch)
    counts = count_instructions(arms, names, scratch)
    res, text = report(times, counts, a.rounds)
    print(text)
    print(f"gain: {time.time() - t0:.0f} s")
    if a.out:
        Path(a.out).write_text(json.dumps(res, indent=1) + "\n", encoding="utf-8")
    shutil.rmtree(scratch, ignore_errors=True)


if __name__ == "__main__":
    main()
