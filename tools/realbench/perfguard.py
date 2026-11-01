#!/usr/bin/env python3
"""Performance guard, run by hand: do the instructions of the nine window.py tests still match a stored reference?

  perfguard.py check [COMMIT]       build COMMIT (default origin/main) with Cranelift only, run the windows at --tier=auto, compare
  perfguard.py reference [COMMIT]   the same run, stored as the new reference (the only command that writes it)
  perfguard.py --self-test

Exit 0: every test within its bound. 1: a test moved past its bound (the issue text is printed, nothing is posted). 2: it could not
run (missing tool, source tree, reference or build); it never passes by default. See AGENTS.md section 8a."""
import argparse, datetime, importlib.util, json, os, shutil, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("realbench_window", HERE / "window.py")
W = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(W)
R, Fail = W.R, W.Fail
REPO = R.REPO
HOME = Path(os.environ.get("YAVS_PERFGUARD_DIR", "~/.cache/yavs-perfguard")).expanduser()
REFERENCE = HOME / "reference.json"
BOUND = 0.03  # largest relative move of a window's instructions, either way
# the auto tier's compile outcome depends on timing (#673): identical runs of one binary moved these two by up to 7.5 and 13.5 percent
BOUNDS = {"hdlm_lfsr": 0.15, "vai_window_gen_interlock": 0.20}
TOOLS = ("git", "perf", "taskset", "systemd-run", "cargo")


def preflight():
    """Everything a run needs, checked before the build: tools, perf access, the source checkouts of the nine tests."""
    missing = [t for t in TOOLS if not shutil.which(t)]
    if missing:
        raise Fail(f"tool(s) not on PATH: {', '.join(missing)}")
    p = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", "--", "true"], capture_output=True, text=True)
    if W.parse_perf(p.stderr)["instructions"] is None:
        raise Fail(f"perf cannot count instructions:u here (kernel.perf_event_paranoid?): {p.stderr.strip()[-200:]}")
    all_tests = {t["name"]: t for t in R.load_tests()}
    if set(W.WINDOWS) - set(all_tests):
        raise Fail(f"window of an unknown test: {sorted(set(W.WINDOWS) - set(all_tests))}")
    R.check_sources([all_tests[n] for n in W.WINDOWS])  # raises Fail naming every missing or unpinned checkout


def checkout(commit):
    """The commit in a detached worktree of its own under the guard directory; returns (path, full hash)."""
    git = lambda *a, cwd=REPO: subprocess.run(["git", *a], cwd=cwd, capture_output=True, text=True)
    git("fetch", "-q", "origin")
    r = git("rev-parse", "--verify", f"{commit}^{{commit}}")
    if r.returncode:
        raise Fail(f"`{commit}` is not a commit of this repository (after git fetch origin): {r.stderr.strip()}")
    sha = r.stdout.strip()
    tree = HOME / "tree"
    if (tree / ".git").exists():
        r = git("checkout", "-q", "--detach", sha, cwd=tree)
    else:
        HOME.mkdir(parents=True, exist_ok=True)
        r = git("worktree", "add", "-q", "--detach", str(tree), sha)
    if r.returncode:
        raise Fail(f"cannot check out {sha[:10]} in {tree}: {r.stderr.strip()}")
    return tree, sha


def measure(commit):
    """{test: window instructions} of a Cranelift build of commit at --tier=auto, from window.py (one variant, no sim1)."""
    preflight()
    tree, sha = checkout(commit)
    binary = W.build(tree)
    out = Path(tempfile.mkdtemp(prefix="run-", dir=HOME))
    env = {**os.environ, "YAVS_REALBENCH_CACHE": os.environ.get("YAVS_REALBENCH_CACHE", str(HOME / "rb"))}
    try:
        subprocess.run([sys.executable, "-I", str(HERE / "window.py"), f"{binary}:auto", "--no-sim1", "--json", str(out / "w.json")], env=env, check=False)
        if not (out / "w.json").exists():
            raise Fail("window.py wrote no result; its output is above")
        res = json.loads((out / "w.json").read_text())["results"]
    finally:
        shutil.rmtree(out, ignore_errors=True)
    bad = [f"{t}: {r['error']}" for t, r in res.items() if "error" in r]
    if bad or set(res) != set(W.WINDOWS):
        raise Fail("window.py could not measure every test:\n  " + "\n  ".join(bad or [f"missing {sorted(set(W.WINDOWS) - set(res))}"]))
    return sha, {t: r["rows"][0]["instructions"] for t, r in res.items()}


def compare(ref, now):
    """Rows (test, reference, now, ratio, bound, verdict) and whether all pass. A move of either sign past the bound fails: less work
    can be a test that quietly stopped doing it. A reference taken with other windows is stale and cannot judge."""
    if ref["windows"] != {t: list(w) for t, w in W.WINDOWS.items()}:
        raise Fail("the reference was taken with other windows than WINDOWS in window.py; refresh it: perfguard.py reference")
    rows, ok = [], True
    for t in W.WINDOWS:
        a, b = ref["instructions"].get(t), now.get(t)
        bound = BOUNDS.get(t, BOUND)
        if not a or not b:
            rows.append((t, a, b, None, bound, "FAIL no value"))
            ok = False
            continue
        x = b / a
        v = "pass" if abs(x - 1) <= bound else "FAIL slower" if x > 1 else "FAIL faster"
        ok &= v == "pass"
        rows.append((t, a, b, x, bound, v))
    return rows, ok


def table(rows):
    f = lambda x, s: "-" if x is None else format(x, s)
    lines = [["test", "ref G", "now G", "now/ref", "bound", "verdict"]]
    lines += [[t, f(a and a / 1e9, ".3f"), f(b and b / 1e9, ".3f"), f(x, ".4f"), f"{bound * 100:.0f}%", v] for t, a, b, x, bound, v in rows]
    w = [max(len(r[i]) for r in lines) for i in range(6)]
    return "\n".join("  ".join(c.ljust(n) if i in (0, 5) else c.rjust(n) for i, (c, n) in enumerate(zip(r, w))).rstrip() for r in lines)


def issue_text(rows, ref, sha):
    """What an issue should say about a failure. Printed only."""
    bad = [r for r in rows if r[5] != "pass"]
    names = ", ".join(r[0] for r in bad)
    return (f"TITLE: perf: window instructions moved past the guard bound ({names})\nLABELS: perf, regression\n\n"
            f"perfguard.py found {len(bad)} of {len(rows)} tests past their bound at {sha[:10]}; the reference is {ref['commit'][:10]} of {ref['date']}.\n\n"
            f"{table(rows)}\n\nInstructions are the simulation thread's, over a fixed simulated-time window at --tier=auto on a Cranelift build "
            "(tools/realbench/AGENTS.md section 8a). hdlm_lfsr and vai_window_gen_interlock vary by 7 to 14 percent between identical runs (#673): repeat before blaming a commit. "
            "Bisect with `perfguard.py check <commit>`.\n\nWritten by Claude.")


def cmd_check(a):
    if not REFERENCE.exists():
        raise Fail(f"no reference at {REFERENCE}; create it with: perfguard.py reference [COMMIT]")
    ref = json.loads(REFERENCE.read_text())
    sha, now = measure(a.commit)
    rows, ok = compare(ref, now)
    print(f"\nperfguard: {sha[:10]} against reference {ref['commit'][:10]} of {ref['date']}\n\n{table(rows)}")
    if not ok:
        print(f"\nFAIL\n\n{issue_text(rows, ref, sha)}\n\nNothing was posted.")
    return 0 if ok else 1


def cmd_reference(a):
    sha, now = measure(a.commit)
    HOME.mkdir(parents=True, exist_ok=True)
    REFERENCE.write_text(json.dumps({"commit": sha, "date": datetime.date.today().isoformat(), "windows": W.WINDOWS, "instructions": now}, indent=1))
    print("\nreference written: " + str(REFERENCE) + f" ({sha[:10]})\n" + "\n".join(f"  {t:28} {v / 1e9:8.3f} G" for t, v in now.items()))
    return 0


def self_test():
    ref = {"commit": "a" * 40, "date": "2026-11-01", "windows": json.loads(json.dumps(W.WINDOWS)), "instructions": {t: 1e9 for t in W.WINDOWS}}
    now = dict(ref["instructions"])
    rows, ok = compare(ref, now)
    assert ok and all(r[5] == "pass" for r in rows)
    now["sc5_timestamp"] = 1.031e9  # past 3 percent
    now["hdlm_lfsr"] = 1.10e9  # inside its own bound
    now["vai_window_gen_interlock"] = 1.15e9
    now["grlib_leon3mp"] = 0.96e9  # less work is a finding too
    rows, ok = compare(ref, now)
    v = {r[0]: r[5] for r in rows}
    assert not ok and v["sc5_timestamp"] == "FAIL slower" and v["hdlm_lfsr"] == v["vai_window_gen_interlock"] == "pass" and v["grlib_leon3mp"] == "FAIL faster", v
    now["hdlm_lfsr"] = None  # a test with no value cannot pass
    assert not compare(ref, now)[1]
    assert "sc5_timestamp" in issue_text(compare(ref, dict(ref["instructions"], sc5_timestamp=2e9))[0], ref, "b" * 40)
    try:
        compare(dict(ref, windows={}), ref["instructions"])
        raise AssertionError("a stale reference judged")
    except Fail:
        pass
    old = os.environ.get("PATH")
    os.environ["PATH"] = ""
    try:
        preflight()
        raise AssertionError("preflight passed without tools")
    except Fail as e:
        assert "not on PATH" in str(e)
    finally:
        os.environ["PATH"] = old
    print("perfguard self-test: ok")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    for n in ("check", "reference"):
        sub.add_parser(n).add_argument("commit", nargs="?", default="origin/main")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if not a.cmd:
        ap.error("a command is needed: check, reference or --self-test")
    return {"check": cmd_check, "reference": cmd_reference}[a.cmd](a)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Fail as e:
        sys.exit(f"perfguard: {e}")
