#!/usr/bin/env bash
# Tests install.sh against the real releases (it downloads yavs 0.13.0 and 0.14.0 and needs network):
#   tools/install/test.sh [PATH-TO-install.sh]
# Linux x86_64 and aarch64. Exit status 1 if any check fails; each check prints ok or FAIL.
set -u -o pipefail
here=$(cd "$(dirname "$0")" && pwd)
script=${1:-$here/../../install.sh}
work=$(mktemp -d "${TMPDIR:-/tmp}/yavs-install-test.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT
arch=$(uname -m)
SETSID=$(command -v setsid) SH=$(command -v sh)
fails=0 cur=
out=''
rc=0
E=() # extra environment for run

ok() { echo "ok: $cur: $*"; }
bad() { echo "FAIL: $cur: $*"; fails=$((fails + 1)); printf '%s\n' "$out" | sed 's/^/    | /'; }
has() { if [[ $out == *"$1"* ]]; then ok "output has '$1'"; else bad "output lacks '$1'"; fi; }
lacks() { if [[ $out != *"$1"* ]]; then ok "output lacks '$1'"; else bad "output has '$1'"; fi; }
rcis() { if [ "$rc" = "$1" ]; then ok "exit status $1"; else bad "exit status $rc, wanted $1"; fi; }
cond() { if eval "$2"; then ok "$1"; else bad "$1 (failed: $2)"; fi; }
# run ARGS: install.sh in a session without a terminal, home $H, on PATH $PATHV.
run() { out=$(env -u XDG_CONFIG_HOME -u ZDOTDIR HOME="$H" PATH="$PATHV" "${E[@]}" "$SETSID" -w "$SH" "$script" "$@" < /dev/null 2>&1); rc=$?; }
# term ANSWER ARGS: the same with a terminal (script), the answer typed ahead.
term() { local ans=$1; shift; local cmd; cmd=$(printf '%q ' sh "$script" "$@")
  out=$(printf '%s\n' "$ans" | env -u XDG_CONFIG_HOME -u ZDOTDIR HOME="$H" PATH="$PATHV" "${E[@]}" script -qec "$cmd" /dev/null 2>&1 | tr -d '\r'); rc=$?; }
first() { "$1" --version 2> /dev/null | head -n 1; }

H=$work/home; mkdir -p "$H"; PATHV=$PATH
P=$work/p

cur="fresh install of 0.14.0"
run --version 0.14.0 --prefix "$P"
rcis 0; has "installing yavs 0.14.0"; has "checksum ok"; has "is not on your PATH"; has "export PATH="
[ "$arch" != x86_64 ] || has "using the x86-64-v3 archive"
cond "yavs runs and is 0.14.0" '[ "$(first "$P/bin/yavs")" = "yavs 0.14.0 (compiled tier: cranelift)" ]'
cond "README.txt and a licence are in the share directory" '[ -f "$P/share/yavs/README.txt" ] && [ -f "$P/share/yavs/LICENSE-MIT" ]'
cond "no staging files are left" '[ -z "$(ls -A "$P/bin" | grep -v "^yavs$")" ]'
# shellcheck disable=SC2034 # used in the eval of cond
id1=$(stat -c %i "$P/bin/yavs")

cur="update no-op"
run --version 0.14.0 --prefix "$P"
rcis 0; has "already at 0.14.0"; lacks "downloading"
cond "the binary was not replaced" '[ "$(stat -c %i "$P/bin/yavs")" = "$id1" ]'
PATHV=$P/bin:$PATH run --version 0.14.0 --prefix "$P"
cur="on PATH, nothing about PATH"; lacks "PATH"
PATHV=$PATH

cur="--force"
run --version 0.14.0 --prefix "$P" --force
rcis 0; has "reinstalling yavs 0.14.0"; has "checksum ok"

cur="replacing a running yavs"
cp "$(command -v bash)" "$P/bin/yavs"
"$P/bin/yavs" -c 'sleep 30; true' & running=$!
sleep 1
run --version 0.14.0 --prefix "$P"
rcis 0; has "does not report a version"
cond "the running process was not disturbed" 'kill -0 $running'
kill "$running"; wait "$running" 2> /dev/null
cond "the new yavs is 0.14.0" '[ "$(first "$P/bin/yavs")" = "yavs 0.14.0 (compiled tier: cranelift)" ]'

cur="downgrade to 0.13.0"
run --version 0.13.0 --prefix "$P"
rcis 0; has "yavs 0.14.0 -> 0.13.0 (a downgrade)"
[ "$arch" != x86_64 ] || has "has one x86_64 archive"
cond "yavs is 0.13.0" '[ "$(first "$P/bin/yavs")" = "yavs 0.13.0 (compiled tier: cranelift)" ]'

cur="--compat on 0.13.0"
if [ "$arch" = x86_64 ]; then
  run --version 0.13.0 --prefix "$P" --compat
  rcis 1; has "--compat needs yavs 0.14.0 or newer"
  cond "the installed yavs still runs" '[ "$(first "$P/bin/yavs")" = "yavs 0.13.0 (compiled tier: cranelift)" ]'
fi

cur="newest version"
run --prefix "$P"
rcis 0; has "yavs 0.13.0 -> "
newest=$(first "$P/bin/yavs")
cond "yavs is now 0.14.0 or newer ($newest)" '[ "$(printf "%s\n" "yavs 0.14.0" "${newest%% (*}" | sort -t. -k1,1n -k2,2n -k3,3n | head -n 1)" = "yavs 0.14.0" ]'

cur="--compat"
run --version 0.14.0 --prefix "$P"
run --version 0.14.0 --prefix "$P" --compat
if [ "$arch" = x86_64 ]; then
  rcis 0; has "using the -compat archive"; has "x86-64-v3 archive) -> 0.14.0 (-compat archive)"
  cond "yavs is the baseline build" '"$P/bin/yavs" --version | grep -q "Build: x86-64 baseline"'
  run --version 0.14.0 --prefix "$P"
  cur="back to the plain archive"; has "-compat archive) -> 0.14.0 (x86-64-v3 archive)"
  cond "yavs is the v3 build" '"$P/bin/yavs" --version | grep -q "Build: x86-64-v3"'
else
  rcis 0; has "aarch64 has one archive"
fi

cur="bad checksum"
mkdir "$work/shim"
cat > "$work/shim/curl" <<SHIM
#!/bin/sh
for a; do out=\${prev:+\$out}; [ "\$prev" = -o ] && out=\$a; prev=\$a; last=\$a; done
$(command -v curl) "\$@"; rc=\$?
case \$last in */SHA256SUMS) sed -i 's/^../xx/' "\$out" ;; esac
exit \$rc
SHIM
chmod +x "$work/shim/curl"
PATHV=$work/shim:$PATH run --version 0.14.0 --prefix "$work/pbad"
rcis 1; has "does not match its line in SHA256SUMS"; has "Nothing was installed"
cond "nothing was installed" '[ ! -e "$work/pbad/bin/yavs" ]'

# A PATH that holds only the tools the script needs.
tools="curl wget tar gzip awk sed grep sort head tail mktemp cp mv rm mkdir chmod uname cat sha256sum"
mkpath() { # DIR TOOLS-TO-LEAVE-OUT...
  local d=$1 t skip x; shift; mkdir -p "$d"
  for t in $tools; do skip=; for x in "$@"; do [ "$x" = "$t" ] && skip=1; done
    [ -n "$skip" ] || { command -v "$t" > /dev/null && ln -s "$(command -v "$t")" "$d/$t"; }; done; }

cur="no sha256 tool"
mkpath "$work/nosum" sha256sum
PATHV=$work/nosum run --version 0.14.0 --prefix "$work/pnosum"
rcis 1; has "neither sha256sum nor shasum"; has "--allow-unverified"
cond "nothing was installed" '[ ! -e "$work/pnosum/bin/yavs" ]'
PATHV=$work/nosum run --version 0.14.0 --prefix "$work/pnosum" --allow-unverified
rcis 0; has "will not be verified"; lacks "checksum ok"
cond "yavs runs" '[ "$(first "$work/pnosum/bin/yavs")" = "yavs 0.14.0 (compiled tier: cranelift)" ]'

cur="wget only"
if command -v wget > /dev/null; then
  mkpath "$work/nocurl" curl
  PATHV=$work/nocurl run --version 0.14.0 --prefix "$work/pwget"
  rcis 0; has "checksum ok"
  cond "yavs runs" '[ "$(first "$work/pwget/bin/yavs")" = "yavs 0.14.0 (compiled tier: cranelift)" ]'
else echo "skipped: $cur: no wget here"; fi

cur="unknown version"
run --version 9.9.9 --prefix "$work/pnone"
rcis 1; has "there is no yavs release v9.9.9"

cur="truncated script"
n=$(wc -l < "$script"); bytes=$(wc -c < "$script"); cut=0
check_cut() { # file what
  rm -rf "$work/ptr"
  out=$(env HOME="$H" PATH="$PATH" "$SETSID" -w "$SH" "$1" --prefix "$work/ptr" --version 0.14.0 < /dev/null 2>&1); rc=$?
  if [ -e "$work/ptr" ] || [[ $out == *"downloading"* || $out == *"installing"* ]]; then bad "$2 ran"; fi
  cut=$((cut + 1)); }
for k in $(seq 1 $((n - 1))); do head -n "$k" "$script" > "$work/trunc.sh"; check_cut "$work/trunc.sh" "the first $k lines"; done
for k in $(seq 1 31); do head -c $((bytes * k / 32)) "$script" > "$work/trunc.sh"; check_cut "$work/trunc.sh" "the first $((bytes * k / 32)) bytes"; done
for k in 2 3 5 8 13 21 30; do head -c $((bytes - k)) "$script" > "$work/trunc.sh"; check_cut "$work/trunc.sh" "all but the last $k bytes"; done
{ head -n $((n - 1)) "$script"; echo 'main "$@"'; } > "$work/trunc.sh"; check_cut "$work/trunc.sh" "the call without its end mark"
cond "$cut truncations ran nothing" 'true'

# The PATH question, with a terminal. Needs `script` (util-linux).
cur="PATH question"
if command -v script > /dev/null; then
  H=$work/h2; mkdir -p "$H"; E=(SHELL=/bin/bash)
  run --version 0.14.0
  rcis 0; has "installing yavs 0.14.0 into $H/.local/bin"; has "is not on your PATH"; has "export PATH=\"\$HOME/.local/bin:\$PATH\""; lacks "added by the yavs installer"
  cond "no terminal: no startup file was written" '[ ! -e "$H/.bashrc" ]'
  cur="PATH question: yes"
  term y --version 0.14.0
  has "already at 0.14.0"; has "Add \$HOME/.local/bin to PATH in ~/.bashrc? [y/N]"; has "added to ~/.bashrc"
  cond ".bashrc has the line once" '[ "$(grep -c "added by the yavs installer" "$H/.bashrc")" = 1 ]'
  term y --version 0.14.0
  # shellcheck disable=SC2088 # a literal tilde
  has "~/.bashrc already has the line"
  cond ".bashrc still has the line once" '[ "$(grep -c "added by the yavs installer" "$H/.bashrc")" = 1 ]'
  cur="PATH question: no"
  H=$work/h3; mkdir -p "$H"; cp -r "$work/h2/.local" "$H/.local"; rm -f "$H/.bashrc"
  term n --version 0.14.0
  has "Not changed"
  cond "no startup file was written" '[ ! -e "$H/.bashrc" ]'
  term "" --version 0.14.0
  has "Not changed"
  cur="PATH question: other shells name their file"
  mkdir "$work/fake"; ln -s "$SH" "$work/fake/zsh"; ln -s "$SH" "$work/fake/fish"
  E=(SHELL="$work/fake/zsh"); term n --version 0.14.0; has "in ~/.zshrc?"
  E=(SHELL="$work/fake/fish"); term n --version 0.14.0; has "in ~/.config/fish/config.fish?"; has "fish_add_path"
  E=(SHELL=/bin/bash)
  cur="PATH question: already on PATH"
  PATHV=$H/.local/bin:$PATH term y --version 0.14.0
  lacks "[y/N]"; lacks "not on your PATH"
  cur="PATH question: YAVS_NO_MODIFY_PATH"
  E=(SHELL=/bin/bash YAVS_NO_MODIFY_PATH=1); term y --version 0.14.0
  lacks "[y/N]"; has "YAVS_NO_MODIFY_PATH is set"
  cond "no startup file was written" '[ ! -e "$H/.bashrc" ]'
else echo "skipped: $cur: no script command here"; fi

echo
if [ "$fails" = 0 ]; then echo "install.sh: all checks passed"; else echo "install.sh: $fails checks FAILED"; exit 1; fi
