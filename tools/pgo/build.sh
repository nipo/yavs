#!/usr/bin/env bash
# Profile-guided release build of yavs for the host target: instrumented build, training runs
# (tools/pgo/train.py), profile merge, final build with the profile. See tools/pgo/AGENTS.md.
#
#   tools/pgo/build.sh OUT_BINARY
#
# Env: RUSTFLAGS      flags both builds share (for example `-C target-cpu=x86-64-v3`)
#      PGO_DIR        profile, scratch and both target directories (default target/pgo)
#      PGO_BUILD      the build command (default `cargo build --release --locked -p yavs --features clift --target HOST`;
#                     it names the target so that RUSTFLAGS stay off build scripts and proc macros, which no training runs)
#      PGO_BIN_REL    binary below a target directory (default HOST/release/yavs, with .exe on Windows)
#      PGO_INSTR_RUSTFLAGS  added to the instrumented build only (`-C linker=cc` where the build's own linker cannot link the
#                     profile runtime). Both builds run one command: cargo mixes --target and its linker setting into the
#                     symbol names, and a profile of a build that differs there matches nothing (tools/pgo/AGENTS.md).
#      PGO_JOBS       training cases in parallel (default 2)
set -euo pipefail
cd "$(dirname "$0")/../.."
out=${1:?usage: tools/pgo/build.sh OUT_BINARY}
dir=$(mkdir -p "${PGO_DIR:-target/pgo}" && cd "${PGO_DIR:-target/pgo}" && pwd)
host=$(rustc -vV | sed -n 's/^host: //p')
build=${PGO_BUILD:-cargo build --release --locked -p yavs --features clift --target $host}
exe=
# The native rustc does not read an MSYS path such as /d/a/yavs.
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) exe=.exe dir=$(cygpath -m "$dir") ;; esac
case $dir in *' '*) echo "pgo: PGO_DIR $dir has a space: RUSTFLAGS cannot carry it; choose another" >&2; exit 1 ;; esac
py=python3
"$py" -V > /dev/null 2>&1 || py=python
rel=${PGO_BIN_REL:-$host/release/yavs$exe}
shared=${RUSTFLAGS:-}

profdata="$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-profdata$exe"
[[ -x $profdata ]] || { echo "pgo: $profdata is missing: run \`rustup component add llvm-tools\`" >&2; exit 1; }

secs() { date +%s; }
t0=$(secs)

echo "pgo: 1/4 instrumented build"
t=$(secs)
CARGO_TARGET_DIR=$dir/target-instr RUSTFLAGS="$shared -C profile-generate=$dir/raw ${PGO_INSTR_RUSTFLAGS:-}" $build
echo "pgo: instrumented build $(($(secs) - t)) s"

echo "pgo: 2/4 training"
t=$(secs)
"$py" -I tools/pgo/train.py "$dir/target-instr/$rel" --profile-dir "$dir/raw" --scratch "$dir/scratch" --jobs "${PGO_JOBS:-2}"
echo "pgo: training $(($(secs) - t)) s"

echo "pgo: 3/4 merge"
"$profdata" merge -o "$dir/merged.profdata" "$dir"/raw/*.profraw
sha256sum "$dir/merged.profdata" 2> /dev/null || shasum -a 256 "$dir/merged.profdata"
"$profdata" show "$dir/merged.profdata" > "$dir/merged.profdata.txt"

echo "pgo: 4/4 final build"
t=$(secs)
# Cargo does not see the content of a profile, so nothing of an earlier final build may stay.
rm -rf "$dir/target-use"
# A compiler warning is all that a wrong profile gives, and none at all for a function it does not list unless asked.
if ! CARGO_TARGET_DIR=$dir/target-use RUSTFLAGS="$shared -C profile-use=$dir/merged.profdata -C llvm-args=-pgo-warn-missing-function" \
  $build > "$dir/final-build.log" 2>&1; then
  tail -n 50 "$dir/final-build.log" >&2
  echo "pgo: the final build failed, see above" >&2
  exit 1
fi
"$py" -I tools/pgo/check.py "$dir/final-build.log" "$dir/merged.profdata.txt" "$dir/target-instr/$rel" "$dir/target-use/$rel"
echo "pgo: final build $(($(secs) - t)) s"
cp "$dir/target-use/$rel" "$out"
echo "pgo: $out, total $(($(secs) - t0)) s"
