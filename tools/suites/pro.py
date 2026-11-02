#!/usr/bin/env python3
"""Run a verification library's own regression (its .pro build scripts) on yavs.

    pro.py [--std 2008|2019] [--root DIR] [--out DIR] [--yavs BIN] [--jobs N]
           [--filter SUBSTR] [--list] [--analyze-only]

Reads OsvvmLibraries.pro (the library build) and RunAllTests.pro (the tests) from the corpus
checkout `osvvm-libraries` (default ~/.cache/rsim-corpus/osvvm-libraries) with pyEDAA.OSVVM, the
OSVVM project reader: it runs the scripts in a real Tcl interpreter and keeps libraries, files, test
cases and generics in its model (needs Python 3.12+ with tkinter; `python3 -m pip install -r
tools/suites/requirements.txt`, the only dependency). A command it does not know is an error naming
the script and line. Library, testbench and test files are analyzed in script order into per-library
directories under `--out`; each test is then simulated with `yavs run` under timeout and systemd-run
(MemoryMax=4G). Pass = exit status 0 and the library's own summary line with zero errors. Results go to
`OUT/results.tsv`, logs to `OUT/run/<n>/log.txt`. Nothing is copied into the repo.
The scripts analyze each test right before running it; here a test whose units a later test file defines
again (TbStream_SendGetRandom1 of AxiStream) runs with the later file's units, the ones the library holds.
"""
import argparse, os, re, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REQUIREMENTS = Path(__file__).resolve().parent / "requirements.txt"
SKIP_DIRS = {"CoSim": "needs a native co-simulation host", "CoSimPCIe": "needs a native co-simulation host"}
# Commands that only affect reporting or the vendor tool; accepted and ignored on purpose (pyEDAA.OSVVM
# already ignores the transcript, resolution and coverage ones).
NOOP = ("SetExtendedSimulateOptions", "SetExtendedAnalyzeOptions", "SetExtendedOptimizeOptions", "SetLogSignals",
        "SetInteractive")


class ProError(Exception):
    pass


def load_pyedaa():
    """Import pyEDAA.OSVVM or stop with the install command."""
    try:
        import tkinter  # noqa: F401  (pyEDAA.OSVVM runs the scripts in tkinter's Tcl interpreter)
    except ImportError:
        sys.exit("pro.py: this Python has no tkinter, which pyEDAA.OSVVM needs for its Tcl interpreter "
                 "(Debian and Ubuntu: apt install python3-tk; python.org and uv builds include it)")
    try:
        from pyEDAA.OSVVM.Project import osvvmContext
        from pyEDAA.OSVVM.Project.Procedures import noop
        from pyEDAA.OSVVM.Project.TCL import OsvvmProFileProcessor, OsvvmVariables
        from pyVHDLModel import VHDLVersion
        from pyTooling.Versioning import YearMonthVersion
    except Exception as e:  # ImportError, or an incompatible pyTooling that fails while importing
        sys.exit(f"pro.py: pyEDAA.OSVVM (Python 3.12 or newer) reads the .pro scripts and is not usable here "
                 f"({type(e).__name__}: {str(e).splitlines()[0] if str(e) else ''}).\n"
                 f"Install it with: python3 -m pip install -r {REQUIREMENTS}")
    return osvvmContext, noop, OsvvmProFileProcessor, OsvvmVariables, VHDLVersion, YearMonthVersion


class Plan:
    """Run the scripts in pyEDAA.OSVVM's Tcl environment and record the analyze and test actions in script order.

    pyEDAA.OSVVM executes the .pro files (a real Tcl interpreter, its own `include`, `library`, `analyze`, `RunTest`,
    `simulate`, `generic`, `TestSuite`, `TestName` and `ChangeWorkingDirectory`) and keeps the libraries, files, test
    cases and generics in its project model. Its model is per library and per suite, with no order between a file and
    a test and no per-test library mapping, so each command is wrapped: the original runs, then the new file or test
    case is read back from the model and appended to `actions`. What the package lacks (the generated settings and
    test-case packages, the tool-only no-ops, the two skipped directories) is registered here."""

    def __init__(self, root, std, out):
        (self.ctx, noop, Processor, Variables, V, YM) = load_pyedaa()
        if std not in ("2008", "2019"):
            raise ProError(f"--std {std}: the scripts are run for 2008 or 2019")
        self.root, self.std, self.out = root, std, out
        self.suite, self.suite_no, self.pending_name = None, 0, None
        self.phase = "build"
        self.build_libs = set()
        self.libdirs = {}      # library name -> directory (latest declaration)
        self.actions = []      # ("analyze"|"test", dict)
        self.skipped = []      # (what, reason)
        self.ctx.Clear()       # the context's working directory is the current one: set by build_plan's chdir
        self.proc = Processor(self.ctx, Variables(
            osvvmVersion=YM.Parse("2026.09"), vhdlVersion=V.VHDL2019 if std == "2019" else V.VHDL2008,
            toolVendor="yavs", toolName="yavs", supports2019Interface="true", supports2019Generics="true",
            supports2019ImpureFunctions="true", supports2019FilePath="true", supports2019AssertAPI="true",
            supports2019Integer64Bits="true"))
        for name in ("UseMemoryPkg01", "OsvvmDevDerivePackageHeaders"):  # not in the package's variable set
            self.proc[f"::osvvm::{name}"] = "false"
        reg = self.proc.RegisterPythonFunctionAsTclProcedure
        for name in NOOP:
            reg(noop, name)
        reg(self.script_settings, "FindOsvvmScriptSettingsPkg")
        reg(lambda base: f"{base}_default.vhd", "FindSettingsPkgBody")
        reg(self.common_pkg, "CreateTestCaseCommonPkg")
        for name, after in (("library", self.on_library), ("analyze", self.on_analyze), ("TestSuite", self.on_suite),
                            ("TestName", self.on_testname), ("RunTest", self.on_runtest)):
            self.wrap(name, after)
        self.wrap("simulate", self.on_test, self.before_simulate)
        self.wrap("include", None, self.before_include)

    def wrap(self, name, after, before=None):
        orig = self.proc.Procedures[name]

        def run(*a):
            if before and before(*a) is False:
                return
            orig(*a)
            if after:
                after(*a)
        self.proc.RegisterPythonFunctionAsTclProcedure(run, name)

    # --- where am I

    @property
    def cwd(self):
        return (self.root / self.ctx.CurrentDirectory).resolve()

    def where(self):
        """File and line of the script command being run, relative to the corpus root."""
        t = self.proc.TCL
        for i in range(int(t.eval("info frame")), 0, -1):
            f = t.eval(f"info frame {i}")
            if f.startswith("type source "):
                file = Path(t.eval(f"dict get [info frame {i}] file"))
                return f"{os.path.relpath(file, self.root)}:{t.eval(f'dict get [info frame {i}] line')}"
        return "?"

    # --- commands pyEDAA.OSVVM does not have (OSVVM's own Tcl library does)

    def script_settings(self):
        """What the scripts generate for OsvvmScriptSettingsPkg's body: the default one with the
        scripts' release as OSVVM_REVISION and OSVVM_SETTINGS_REVISION (the settings that
        follow it, such as the transcript layout, are those of the validated results)."""
        text = (self.cwd / "OsvvmScriptSettingsPkg_default.vhd").read_text()
        for c in ("OSVVM_REVISION", "OSVVM_SETTINGS_REVISION"):
            text, n = re.subn(rf'({c}\s*:\s*string\s*:=\s*)"[^"]*"', r'\g<1>"2026.09"', text)
            if n != 1:
                raise ProError(f"{self.where()}: no {c} in OsvvmScriptSettingsPkg_default.vhd")
        f = self.out / "gen" / "OsvvmScriptSettingsPkg_generated.vhd"
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(text)
        return str(f)

    def common_pkg(self, name="TestCaseCommonPkg", validated="../ValidatedResults"):
        """What the library's CreateTestCaseCommonPkg writes (the 2008 form), into OUT/gen."""
        d = self.out / "gen" / f"{self.suite_no}_{self.suite}"
        f = d / f"{name}.vhd"
        d.mkdir(parents=True, exist_ok=True)
        f.write_text(f'''-- generated by pro.py
library osvvm ;
context osvvm.OsvvmContext ;
package {name} is
  constant PATH_TO_TEST_SRC            : string := "{self.cwd}/" ;
  constant PATH_TO_VALIDATED_RESULTS   : string  := PATH_TO_TEST_SRC & "{validated}/" ;
  constant CHECK_TRANSCRIPT            : boolean := PATH_TO_TEST_SRC'length > 0 ;
  constant BUILD_NAME                  : string  := "yavs" ;
  constant TEST_SUITE_NAME             : string  := "{self.suite}" ;
  constant PATH_TO_RESULTS             : string := OSVVM_BASE_DIRECTORY & BUILD_NAME & "/results/" & TEST_SUITE_NAME & "/" ;
  constant RESULTS_DIR             : string := PATH_TO_RESULTS ;
  constant VALIDATED_RESULTS_DIR   : string := PATH_TO_VALIDATED_RESULTS ;
end package {name} ;
''')
        return str(f)

    # --- recording

    def before_include(self, p):
        t = (self.cwd / p).resolve()
        if t.name in SKIP_DIRS:
            self.skipped.append((str(t.relative_to(self.root)), SKIP_DIRS[t.name]))
            return False

    def before_simulate(self, top, *_):
        if self.pending_name is None:  # no TestName: the test is named after the toplevel (pyEDAA.OSVVM would reuse the previous test case)
            self.ctx.AddTestcase(top)

    def on_library(self, name, *_):
        lib = self.ctx.VHDLLibrary.Name
        if self.phase == "tests" and lib.lower() not in self.build_libs:
            d = self.out / "tb" / f"{self.suite_no}_{self.suite or 'none'}" / lib
        else:
            d = self.out / "libs" / lib
        self.libdirs[lib.lower()] = d

    def on_analyze(self, *_):
        lib = self.ctx.VHDLLibrary
        self.actions.append(("analyze", dict(lib=lib.Name, file=(self.ctx.WorkingDirectory / lib.Files[-1].Path).resolve(),
                                             where=self.where(), suite=self.suite_no, libs=dict(self.libdirs))))

    def on_suite(self, name):
        self.suite, self.suite_no = name, self.suite_no + 1

    def on_testname(self, name):
        self.pending_name = name

    def on_runtest(self, *_):
        self.on_analyze()
        self.on_test()

    def on_test(self, *_):
        tc = self.ctx.TestCase
        if self.suite is None:  # a test with no TestSuite before it: the suite is the library
            self.suite, self.suite_no = self.ctx.VHDLLibrary.Name, self.suite_no + 1
        self.actions.append(("test", dict(name=self.pending_name or tc.Name, sim=tc.ToplevelName, lib=self.ctx.VHDLLibrary.Name,
                                          where=self.where(), suite=self.suite, suite_no=self.suite_no,
                                          generics=list(tc.Generics.items()), libs=dict(self.libdirs))))
        self.pending_name = None


def build_plan(root, std, out):
    """Plan of OsvvmLibraries.pro (the library build) followed by RunAllTests.pro (the tests) under root."""
    cwd = os.getcwd()
    os.chdir(root)  # pyEDAA.OSVVM's context resolves script paths against the process directory
    try:
        p = Plan(root, std, out)
        for phase, script in (("build", "OsvvmLibraries.pro"), ("tests", "RunAllTests.pro")):
            if phase == "tests":
                p.build_libs = set(p.libdirs)
                p.phase = "tests"
                p.suite, p.suite_no = None, 0
            try:
                p.proc.LoadBuildFile(Path(script))
            except Exception as e:
                chain = []
                while e is not None:
                    chain.append(str(e).splitlines()[0] if str(e) else type(e).__name__)
                    e = e.__cause__
                try:  # Tcl's stack trace names the innermost script and line
                    at = re.search(r'\(file "([^"]+)" line (\d+)\)', p.proc.TCL.getvar("errorInfo"))
                except Exception:
                    at = None
                where = f"{os.path.relpath(at[1], root)}:{at[2]}: " if at else f"{script}: "
                why = [c for c in chain if not c.startswith("Caught TclError")]
                raise ProError(f"{where}{' / '.join(dict.fromkeys(why[:1] + why[-1:]))} (in the command that starts at this line)") from None
        return p
    finally:
        os.chdir(cwd)


def sandbox(cmd, log, cwd, timeout=300):
    """Run cmd under timeout and systemd-run; return (status, seconds). Output appended to log."""
    full = ["timeout", str(timeout)] + ([] if NO_WRAP else ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=4G"]) + cmd
    t = time.time()
    with open(log, "ab") as f:
        r = subprocess.run(full, stdout=f, stderr=subprocess.STDOUT, cwd=cwd)
    return r.returncode, time.time() - t


NO_WRAP = False  # --no-wrap: CI runners have no systemd user session
SUMMARY = re.compile(r"^%%.*\bDONE\s+PASSED\b", re.M)
FAILSUM = re.compile(r"^%%.*\bDONE\s+FAILED\b", re.M)


def verdict(status, text):
    """Pass = exit 0 and the library's summary line reports zero errors."""
    if status == 124:
        return "TIMEOUT"
    if status == 0 and SUMMARY.search(text) and not FAILSUM.search(text):
        return "PASS"
    if status == 0:
        return "NOSUMMARY"
    return "FAIL"


def first_cause(text):
    """First error diagnostic, else the first failed check line, else the last output line."""
    for pat in (r"^(error|fatal)", r"^%%.*(ERROR|FAILED|FAILURE)", r"panicked|Killed"):
        for line in text.splitlines():
            if re.search(pat, line):
                return line.strip()[:200]
    lines = [l for l in text.splitlines() if l.strip()]
    return "no diagnostic: " + (lines[-1].strip()[:150] if lines else "no output")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--std", default="2008")
    ap.add_argument("--root", default=os.path.expanduser("~/.cache/rsim-corpus/osvvm-libraries"))
    ap.add_argument("--out", required=True)
    ap.add_argument("--yavs", default="yavs")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--filter", default="")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--analyze-only", action="store_true")
    ap.add_argument("--no-wrap", action="store_true", help="run without systemd-run (CI)")
    o = ap.parse_args()
    global NO_WRAP
    NO_WRAP = o.no_wrap
    if o.jobs > 4:
        sys.exit("pro.py: at most 4 parallel runs")
    out, root = Path(o.out).resolve(), Path(o.root).resolve()
    # the shared std/ieee store (#564) is private to this tool, never the user's
    os.environ["YAVS_STD_CACHE"] = str(out / "std-cache")
    try:
        plan = build_plan(root, o.std, out)
    except ProError as e:
        sys.exit(f"pro.py: {e}")
    tests = [d for k, d in plan.actions if k == "test"]
    print(f"{sum(1 for k, _ in plan.actions if k == 'analyze')} analyze, {len(tests)} tests; skipped: {plan.skipped}")
    if o.list:
        for t in tests:
            print(t["suite"], t["sim"], t["generics"])
        return
    out.mkdir(parents=True, exist_ok=True)
    (out / "run").mkdir(exist_ok=True)
    # phase 1: analyze everything in script order; consecutive files of one library with the same
    # library mapping go in one call (one analysis session sees the units of the files before it)
    failed, nan = {}, 0
    batches = []
    for k, d in plan.actions:
        if k != "analyze":
            continue
        key = (d["lib"].lower(), tuple(sorted(d["libs"].items())), d["suite"])
        if batches and batches[-1][0] == key:
            batches[-1][1].append(d)
        else:
            batches.append((key, [d]))
    log = out / "analyze.log"

    def analyze(group):
        d = group[0]
        dirs = dict(d["libs"])
        cmd = [o.yavs, "analyze", f"--std={o.std}", f"--work={d['lib']}:{dirs[d['lib'].lower()]}"]
        cmd += [f"--map={n}:{p}" for n, p in dirs.items()]
        with open(log, "ab") as f:
            for x in group:
                f.write(f"== {x['where']} {x['file'].name}\n".encode())
        return sandbox(cmd + [str(x["file"]) for x in group], log, out)[0]

    for _, group in batches:
        nan += len(group)
        if analyze(group) == 0:
            continue
        for x in group:  # a failing batch stops at its first error: find the files that fail
            if len(group) > 1 and analyze([x]) == 0:
                continue
            failed.setdefault(x["suite"], []).append(f"{x['where']} {x['file'].name}")
    print(f"analyzed {nan} files; failures: {sum(len(v) for v in failed.values())}")
    if o.analyze_only:
        for v in failed.values():
            print(*v, sep="\n")
        return

    def one(item):
        n, t = item
        rd = out / "run" / str(n)
        rd.mkdir(parents=True, exist_ok=True)
        log = rd / "log.txt"
        log.write_bytes(b"")
        (rd / "yavs" / "results" / t["suite"]).mkdir(parents=True, exist_ok=True)
        (rd / "bmp_logs").mkdir(exist_ok=True)
        cmd = [o.yavs, "run", f"--std={o.std}", f"--work={t['lib']}:{t['libs'][t['lib'].lower()]}"]
        cmd += [f"--map={a}:{b}" for a, b in t["libs"].items()] + [t["sim"]] + [f"-g{a}={b}" for a, b in t["generics"]]
        status, secs = sandbox(cmd, log, rd)
        text = log.read_text(errors="replace")
        v = verdict(status, text)
        return (n, t["suite"], t["name"], v, status, f"{secs:.1f}", first_cause(text) if v != "PASS" else "")

    items = [(n, t) for n, t in enumerate(tests) if o.filter in f"{t['suite']}/{t['name']}"]
    with ThreadPoolExecutor(o.jobs) as ex:
        rows = list(ex.map(one, items))
    with open(out / "results.tsv", "w") as f:
        for r in rows:
            f.write("\t".join(map(str, r)) + "\n")
    from collections import Counter
    print(Counter(r[3] for r in rows))
    causes = Counter(re.sub(r"`[^`]*`|:[\w.:()]+", "`X`", r[6]) for r in rows if r[3] != "PASS")
    for k, v in causes.most_common():
        print(f"{v:5} {k}")
    for suite in sorted({r[1] for r in rows}):
        c = Counter(r[3] for r in rows if r[1] == suite)
        print(f"{suite:32} " + " ".join(f"{k}={v}" for k, v in sorted(c.items())))
    sys.exit(0 if all(r[3] == "PASS" for r in rows) else 1)


if __name__ == "__main__":
    main()
