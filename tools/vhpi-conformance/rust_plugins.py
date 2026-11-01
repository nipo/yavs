#!/usr/bin/env python3
"""Second VHPI client, nothing to install by hand: the plugins and testbenches of the Rust `vhpi`
crate's repository, from a pinned commit in the cache, on yavs and on the primary reference.

    YAVS_BIN=path/to/yavs tools/vhpi-conformance/rust_plugins.py [--only NAME[,..]] [--show] [--out DIR]

Marker lists and testbench names are read from the checkout's own run scripts. A plugin passes when
every marker is printed and yavs exits 0, as the reference does. The dumper's output is also diffed
against the reference (tool name, version and message prefixes normalised). Building needs cargo,
git and libclang (the crate's bindings are generated with bindgen).
"""
import argparse
import difflib
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import simconf  # noqa: E402

CRATE = "vhpi@0.6.0"
COMMIT = "65484e320b3830eb0eff1acd7f2dcce635a7bf34"
# The run scripts with plugins that run here. run_dumper_mixed.sh: a Verilog design, not for yavs.
SCRIPTS = ["run_test_simple", "run_stringindexing", "run_cb_toggle_checks", "run_foreignf_checks", "run_dumper_checks"]
SANDBOX = ["timeout", "120", "systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=2G"]
# What only says who printed it: the reference's prefixes, the tool's identity, yavs's own warnings.
PREFIX = re.compile(r"^\*\* (Note|Warning): (\([a-z]+\): |\d+[a-z]*\+\d+: )?|^VHPI assertion Warning: ")
NOISE = re.compile(r"^(loading plugin|   Process |warning\[W09|  \||  = note|$)|^simulator (name|version|capabilities|random seed):")


def run(cmd, cwd=None, env=None):
    return subprocess.run(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")


def repository():
    """The repository of the crate, from its metadata on crates.io."""
    p = run(["cargo", "info", CRATE])
    m = re.search(r"^repository: (\S+)", p.stdout, re.M)
    if not m:
        sys.exit(f"error: cannot read the repository of {CRATE}: {p.stdout.strip()}\nhelp: check the network")
    return m.group(1)


def checkout(cache):
    """The pinned commit under `cache/src`, built; returns (source, directory of the plugins)."""
    src = cache / "src"
    if not src.exists():
        repo = repository()
        p = run(["git", "clone", "--quiet", repo, str(src)])
        if p.returncode:
            sys.exit(f"error: cannot clone {repo}: {p.stdout.strip()}\nhelp: check the network, or clone it to {src} by hand")
    p = run(["git", "-C", str(src), "checkout", "--quiet", "--detach", COMMIT])
    if p.returncode:
        sys.exit(f"error: commit {COMMIT} is not in {src}: {p.stdout.strip()}\nhelp: delete {src} to clone again")
    target = cache / "target"
    p = run(["cargo", "build", "--locked", "--workspace"], cwd=src, env={**os.environ, "CARGO_TARGET_DIR": str(target)})
    if p.returncode:
        last = p.stdout.strip().splitlines()[-20:]
        hint = "libclang is needed to generate the crate's bindings: install the libclang package" if "libclang" in p.stdout else "see the output above"
        sys.exit("\n".join(last) + f"\nerror: cannot build the plugins in {src}\nhelp: {hint}")
    return src, target / "debug"


def cases(src):
    """(plugin, testbench, markers) from the checkout's run scripts."""
    out = []
    for script in SCRIPTS:
        text = (src / "scripts" / f"{script}.sh").read_text()
        plugin = re.search(r'^PLUGIN_CRATE="(\w+)"', text, re.M).group(1)
        m = re.search(r"^EXPECTED_MARKERS=\((.*?)^\)", text, re.M | re.S)
        markers = re.findall(r'"([^"\n]+)"', m.group(1)) if m else []
        one = re.search(r'^TEST_BENCH="(\w+)"', text, re.M)
        many = re.search(r"^TEST_BENCHES=\((.*?)^\)", text, re.M | re.S)
        for tb in [one.group(1)] if one else many.group(1).split():
            out.append((plugin, tb, markers))
    return out


def run_yavs(so, file, tb, work):
    yavs = os.environ.get("YAVS_BIN", "yavs")
    lib = f"--work=work:{work}"
    p = run(SANDBOX + [yavs, "analyze", "--std=2008", lib, str(file)])
    if p.returncode:
        return p.returncode, p.stdout
    p = run(SANDBOX + [yavs, "run", "--std=2008", lib, f"--vhpi={so}", tb])
    return p.returncode, p.stdout


def run_ref(so, file, tb, work):
    ana, go = simconf.get("sim1", "vhpi_analyse"), simconf.get("sim1", "vhpi_run")
    if ana is None or go is None:
        return None, "the reference is not configured (tools/sims.local.conf)"
    work.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "SO": str(so), "FILE": str(file), "TOP": tb, "WORK": str(work), "RELAXED": "", "FOREIGN": ""}
    p = run(SANDBOX + ["bash", "-c", ana], cwd=work, env=env)
    if p.returncode:
        return p.returncode, p.stdout
    p = run(SANDBOX + ["bash", "-c", go], cwd=work, env=env)
    return p.returncode, p.stdout


def normal(text):
    lines = (PREFIX.sub("", l).rstrip() for l in text.splitlines())
    return [l for l in lines if not NOISE.match(l)]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--only", help="comma separated plugin names")
    ap.add_argument("--show", action="store_true", help="print the dumper diffs")
    ap.add_argument("--out", default=os.path.expanduser("~/.cache/yavs-vhpi-rust"))
    a = ap.parse_args()
    if sys.platform != "linux":
        sys.exit("error: this run is written for Linux\nhelp: the plugins' `dynamic` feature on Windows is not wired in here yet (#273)")
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    os.environ["YAVS_STD_CACHE"] = str(out.resolve() / "std-cache")
    src, plugins = checkout(out)
    runs = out / "runs"
    bad = 0
    print(f"{'plugin':<16}{'testbench':<20}{'yavs':<16}{'reference':<14}markers  diff to reference")
    for plugin, tb, markers in cases(src):
        if a.only and plugin not in a.only.split(","):
            continue
        so = plugins / f"lib{plugin}.so"
        file = src / "test_examples" / f"{tb}.vhdl"
        tag = f"{plugin}.{tb}"
        rc, got = run_yavs(so, file, tb, runs / f"{tag}.yavs")
        rrc, ref = run_ref(so, file, tb, runs / f"{tag}.sim1")
        runs.mkdir(parents=True, exist_ok=True)
        (runs / f"{tag}.yavs.log").write_text(got)
        (runs / f"{tag}.sim1.log").write_text(ref)
        have = sum(bool(re.search(m, got)) for m in markers)
        diff = ""
        if plugin == "dumper" and rrc is not None:
            d = list(difflib.unified_diff(normal(ref), normal(got), "reference", "yavs", lineterm="", n=0))
            (runs / f"{tag}.diff").write_text("\n".join(d) + "\n")
            hunks = sum(l.startswith("@@") for l in d)
            diff = "equal" if not d else f"{hunks} hunks"
            if a.show and d:
                print("\n".join(d))
        ok = rc == 0 and have == len(markers)
        bad += not ok
        print(f"{plugin:<16}{tb:<20}{('pass' if ok else 'FAIL') + f' (exit {rc})':<16}{f'exit {rrc}':<14}{f'{have}/{len(markers)}':<9}{diff}")
    print(f"logs and diffs in {runs}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
