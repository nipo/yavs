#!/usr/bin/env python3
"""Did the final build of a profile-guided build use the profile? Fails naming the cause.

  python3 -I tools/pgo/check.py FINAL_BUILD_LOG PROFDATA_SHOW INSTRUMENTED_BIN FINAL_BIN
  python3 -I tools/pgo/check.py --selftest

FINAL_BUILD_LOG is the compiler's output with `-pgo-warn-missing-function`, PROFDATA_SHOW the output of `llvm-profdata show`.
The compiler warns about a function the profile does not list, and says nothing else when the profile matches nothing at all
(tools/pgo/AGENTS.md section 4). The profile lists every instrumented function, called or not, so a build that fits it has none
of those warnings: the count is the proof.
"""
import re
import sys
from pathlib import Path

MISMATCH = re.compile(r"hash mismatch|control flow change|malformed|profile data may be|counter overflow", re.I)
MISSING = re.compile(r"no profile data available for function")
MIN_FUNCTIONS = 1000  # a real training of yavs lists tens of thousands
RUNTIME = b"LLVM_PROFILE_FILE"  # the profile runtime reads this variable: in an instrumented binary only


def judge(log, show, instrumented, final):
    """The list of problems, empty when the build used the profile."""
    bad = [f"the compiler complains about the profile: {x.strip()[:300]}" for x in log.splitlines() if MISMATCH.search(x)][:5]
    m = re.search(r"^Total functions: (\d+)", show, re.M)
    total = int(m.group(1)) if m else 0
    if total < MIN_FUNCTIONS:
        bad.append(f"the profile is empty or nearly so: {total} functions (at least {MIN_FUNCTIONS} expected); did the training run?")
    missing = len(MISSING.findall(log))
    print(f"pgo: the final build compiled {missing} functions the profile does not list; the profile has {total}")
    if missing:
        bad.append(f"the profile does not fit the final build: {missing} functions are not in it. Did both builds run one command? "
                   "Cargo mixes --target and its linker setting into the symbol names.")
    if RUNTIME not in instrumented:
        bad.append("the instrumented binary has no profile runtime: was it built with -C profile-generate?")
    if RUNTIME in final:
        bad.append("the final binary still has the profile runtime: it is an instrumented build")
    return bad


def selftest():
    show = "Instrumentation level: IR\nTotal functions: 50000\n"
    assert judge("Finished\n", show, RUNTIME, b"x") == []
    miss = "warning: x: no profile data available for function f Hash = 1\n"
    assert "does not fit" in judge(miss, show, RUNTIME, b"x")[0]
    assert "complains" in judge("warning: x: function control flow change detected (hash mismatch)\n", show, RUNTIME, b"x")[0]
    assert "empty" in judge("", "Total functions: 3\n", RUNTIME, b"x")[0]
    assert "empty" in judge("", "", RUNTIME, b"x")[0]
    assert "no profile runtime" in judge("", show, b"x", b"x")[0]
    assert "still has" in judge("", show, RUNTIME, RUNTIME)[0]
    print("pgo: check selftest ok")


def main():
    if sys.argv[1:] == ["--selftest"]:
        return selftest()
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    log, show, instrumented, final = sys.argv[1:]
    bad = judge(Path(log).read_text("utf-8", "replace"), Path(show).read_text("utf-8", "replace"), Path(instrumented).read_bytes(), Path(final).read_bytes())
    for b in bad:
        print(f"pgo: {b}", file=sys.stderr)
    if bad:
        sys.exit(f"pgo: the final build did not use the profile properly, see {log}")


if __name__ == "__main__":
    main()
