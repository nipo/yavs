#!/usr/bin/env bash
# Build yavs for benchmarking and profiling: the release profile plus 64-byte aligned functions (#576),
# so code layout does not move the cycles of the interpreter loop between builds. Release builds and CI
# artefacts are NOT aligned. Same arguments as `cargo build --release -p yavs`; pass --features llvm etc.
# Put the output in its own target dir (CARGO_TARGET_DIR), RUSTFLAGS differ from a plain release build.
# Line tables are on by default here (profiling with perf/gdb needs them; they change no code and no timing);
# release artefacts have none (#600). Set CARGO_PROFILE_RELEASE_DEBUG=false for a build without.
#
#   tools/build-bench.sh --pgo OUT [cargo args]   profile-guided bench build (tools/pgo/build.sh) copied to OUT,
#                                                 with --target HOST; profiles and builds in PGO_DIR (default
#                                                 $CARGO_TARGET_DIR/pgo). Compare it only with another PGO bench
#                                                 build, or with `tools/build-bench.sh --target HOST ...` (the
#                                                 flag alone moves cycles): tools/bench/AGENTS.md section 7.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ${1:-} == --pgo ]]; then
  out=${2:?usage: tools/build-bench.sh --pgo OUT [cargo args]}
  shift 2
  case $out in /*) ;; *) out=$PWD/$out ;; esac
  host=$(rustc -vV | sed -n 's/^host: //p')
  export PGO_DIR=${PGO_DIR:-${CARGO_TARGET_DIR:-target}/pgo}
  PGO_BUILD="tools/build-bench.sh $* --target $host" exec tools/pgo/build.sh "$out"
fi
export CARGO_PROFILE_RELEASE_DEBUG="${CARGO_PROFILE_RELEASE_DEBUG:-line-tables-only}"
RUSTFLAGS="${RUSTFLAGS:-} -C llvm-args=-align-all-functions=6" exec cargo build --release -p yavs "$@"
