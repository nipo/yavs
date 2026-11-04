#!/bin/sh
# Install or update yavs on Linux (x86_64 or aarch64) without root:
#
#   curl -fsSL https://raw.githubusercontent.com/ru551n/yavs/main/install.sh | sh
#
# Options go after `sh -s --`, for example `... | sh -s -- --compat`; `--help` lists them.
# Read this file first if you like: it downloads a release archive from github.com, checks it
# against the release's SHA256SUMS and copies yavs into a directory of yours. docs/getting-started.rst
# ("One-line install") describes it. All logic is in functions and main runs on the last line, so a
# download that stops early runs nothing.

repo=ru551n/yavs
releases=https://github.com/$repo/releases
guide=https://yavs.readthedocs.io/en/latest/getting-started.html
tmp=
bindir=
sharedir=

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" > /dev/null 2>&1; }

usage() {
  cat <<EOF
Install or update yavs. Run it again to update.

Usage: install.sh [options]
  --version X.Y.Z     install that version (default: the newest release); older ones are allowed
  --compat            use the -compat archive (x86_64 CPUs without AVX2; needs yavs 0.14.0 or newer)
  --prefix DIR        install into DIR/bin and DIR/share/yavs (default: \$YAVS_INSTALL_DIR, else ~/.local)
  --force             install even if that version is already installed
  --allow-unverified  install without checking SHA256SUMS when this system has no sha256sum or shasum
  -h, --help          this text

Needs curl or wget, tar and a sha256 tool. Environment: YAVS_INSTALL_DIR (as --prefix),
GITHUB_TOKEN (used for the lookup of the newest version only, to avoid GitHub's rate limit),
YAVS_NO_MODIFY_PATH=1 (never offer to edit your shell's startup file).

If the install directory is not on PATH and a terminal is available, the installer asks before adding
one marked line to the startup file of your shell (default: no). Without a terminal it only prints the line.
EOF
}

cleanup() {
  [ -z "$tmp" ] || rm -rf "$tmp"
  [ -z "$bindir" ] || rm -f "$bindir/.yavs.$$.new"
  [ -z "$sharedir" ] || rm -f "$sharedir"/.*."$$"
}

# fetch URL FILE [TOKEN]: HTTP status in $http; a transport failure stops here.
fetch() {
  rc=0
  if have curl; then
    if [ -n "${3:-}" ]; then
      http=$(printf 'header = "Authorization: Bearer %s"\n' "$3" |
        curl -sSL --proto '=https' --tlsv1.2 --retry 3 --config - -o "$2" -w '%{http_code}' "$1") || rc=$?
    else
      http=$(curl -sSL --proto '=https' --tlsv1.2 --retry 3 -o "$2" -w '%{http_code}' "$1") || rc=$?
    fi
    [ "$rc" = 0 ] || die "could not download $1: curl exit status $rc. Check the network connection and any proxy settings, then run the installer again."
  else
    if [ -n "${3:-}" ]; then
      wget -q --header="Authorization: Bearer $3" -O "$2" "$1" || rc=$?
    else
      wget -q -O "$2" "$1" || rc=$?
    fi
    case $rc in
      0) http=200 ;;
      8) http=error ;; # wget does not say which; a missing release is the usual cause
      *) die "could not download $1: wget exit status $rc. Check the network connection and any proxy settings, then run the installer again." ;;
    esac
  fi
}

# vlt A B: A is an older version than B.
vlt() {
  [ "$1" != "$2" ] &&
    [ "$(printf '%s\n%s\n' "$1" "$2" | sort -t. -k1,1n -k2,2n -k3,3n | head -n 1)" = "$1" ]
}

# Stable means a plain vX.Y.Z tag. All yavs releases carry GitHub's pre-release mark while yavs is
# early, so /releases/latest (which skips them) cannot be used; list them instead. The vunit-yavs-v*
# and vscode-v* tags are other products.
# ponytail: reads the newest 100 releases, enough while a page holds at least one stable release.
latest_version() {
  fetch "https://api.github.com/repos/$repo/releases?per_page=100" "$tmp/releases.json" "${GITHUB_TOKEN:-}"
  case $http in
    200) ;;
    403 | 429) die "GitHub's rate limit stopped the lookup of the newest yavs release (HTTP $http). Wait a while and run again, set GITHUB_TOKEN to a token for a higher limit, or name the version: --version X.Y.Z needs no lookup (versions are listed at $releases)." ;;
    *) die "could not look up the newest yavs release: api.github.com answered HTTP $http (403 or 429 is its rate limit). Run again later, or name the version: --version X.Y.Z needs no lookup (versions are listed at $releases)." ;;
  esac
  version=$(grep -oE '"tag_name": *"v[0-9]+\.[0-9]+\.[0-9]+"' "$tmp/releases.json" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' |
    sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1) || true
  [ -n "$version" ] || die "found no yavs release at $releases. Name the version with --version X.Y.Z."
}

sha256_of() {
  if have sha256sum; then sha256sum "$1" | awk '{print $1}'
  else shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# cpu_missing: the x86-64-v3 features /proc/cpuinfo does not list (abm is LZCNT).
cpu_missing() {
  missing=
  [ -r /proc/cpuinfo ] || { unreadable=1; return; }
  unreadable=
  flags=$(sed -n '/^flags/{p;q;}' /proc/cpuinfo)
  for f in avx2 bmi1 bmi2 fma f16c abm movbe; do
    case " $flags " in
      *" $f "*) ;;
      *) missing="$missing $f" ;;
    esac
  done
}

main() {
  want_version=latest compat='' force='' unverified='' prefix=${YAVS_INSTALL_DIR:-} complete=''
  while [ "$#" -gt 0 ]; do
    case $1 in
      --version) [ "$#" -ge 2 ] || die "--version needs a value, for example --version 0.14.0."; want_version=$2; shift ;;
      --version=*) want_version=${1#--version=} ;;
      --prefix) [ "$#" -ge 2 ] || die "--prefix needs a directory."; prefix=$2; shift ;;
      --prefix=*) prefix=${1#--prefix=} ;;
      --compat) compat=1 ;;
      --force) force=1 ;;
      --allow-unverified) unverified=1 ;;
      -h | --help) usage; exit 0 ;;
      --installer-complete) complete=1 ;;
      *) die "unknown option $1. The options are listed by --help." ;;
    esac
    shift
  done
  [ -n "$complete" ] || die "this installer script is incomplete (the download stopped early), so nothing was done. Download it again."

  trap cleanup EXIT
  trap 'exit 1' HUP INT TERM

  # Platform.
  case $(uname -s) in
    Linux) ;;
    Darwin) die "yavs has no macOS archive. Build it from source: see \"From source\" at $guide" ;;
    MINGW* | MSYS* | CYGWIN*) die "this is Windows. In PowerShell run: powershell -c \"irm https://raw.githubusercontent.com/$repo/main/install.ps1 | iex\", or download the zip from $releases." ;;
    *) die "yavs has archives for Linux only here (x86_64 and aarch64; Windows has install.ps1). Build it from source: see \"From source\" at $guide" ;;
  esac
  case $(uname -m) in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) die "yavs has no archive for the CPU architecture $(uname -m): only x86_64 and aarch64. Build it from source: see \"From source\" at $guide" ;;
  esac
  target=$arch-unknown-linux-gnu

  # Tools, before anything is downloaded.
  have curl || have wget || die "found neither curl nor wget. Install one of them, then run the installer again."
  for t in tar awk sed grep sort head tail mktemp cp mv rm mkdir chmod; do
    have "$t" || die "this system has no $t, which the installer needs. Install it, then run the installer again."
  done
  if have sha256sum || have shasum; then
    verify=1
  elif [ -n "$unverified" ]; then
    verify=
    say "warning: no sha256sum or shasum here: the download will not be verified (--allow-unverified)."
  else
    die "found neither sha256sum nor shasum, so the download cannot be checked against SHA256SUMS. Install coreutils, or run again with --allow-unverified to install without the check."
  fi

  # Where to.
  if [ -z "$prefix" ]; then
    [ -n "${HOME:-}" ] || die "HOME is not set, so there is no default install directory. Pass --prefix DIR or set YAVS_INSTALL_DIR."
    prefix=$HOME/.local
  fi
  prefix=${prefix%/}
  [ -n "$prefix" ] || die "the install prefix is the root directory. Pass a directory of your own with --prefix DIR."
  bindir=$prefix/bin
  sharedir=$prefix/share/yavs

  tmp=$(mktemp -d "${TMPDIR:-/tmp}/yavs-install.XXXXXX") ||
    die "could not make a temporary directory in ${TMPDIR:-/tmp}. Check that it is writable and has free space, or set TMPDIR."

  # Which version.
  case $want_version in
    latest) latest_version ;;
    *)
      version=${want_version#v}
      printf '%s\n' "$version" | grep -Eqx '[0-9]+\.[0-9]+\.[0-9]+' ||
        die "'$want_version' is not a version. Write an exact version such as 0.14.0, or leave --version out for the newest."
      ;;
  esac

  # Which archive.
  say "yavs installer: Linux $arch"
  variant=x86-64-v3
  if [ "$arch" = x86_64 ]; then
    if vlt "$version" 0.14.0; then
      [ -z "$compat" ] || die "--compat needs yavs 0.14.0 or newer: yavs $version has one x86_64 archive and no -compat one. Use --version 0.14.0 or newer, or leave --compat out."
      say "yavs $version has one x86_64 archive, so no CPU check is made."
    elif [ -n "$compat" ]; then
      variant=-compat
      say "--compat: using the -compat archive, which runs on any x86-64 CPU."
    else
      cpu_missing
      if [ -n "$unreadable" ]; then
        variant=-compat
        say "cannot read /proc/cpuinfo to check for AVX2: using the -compat archive, which runs on any x86-64 CPU."
      elif [ -n "$missing" ]; then
        variant=-compat
        say "this CPU lacks$(printf '%s' "$missing" | sed 's/ abm/ lzcnt/'): using the -compat archive, which runs on any x86-64 CPU."
      else
        say "this CPU has AVX2, BMI1, BMI2, FMA, F16C, LZCNT and MOVBE: using the x86-64-v3 archive."
      fi
    fi
  elif [ -n "$compat" ]; then
    say "--compat: aarch64 has one archive, so it changes nothing."
  fi
  suffix=
  [ "$variant" = -compat ] && suffix=-compat
  name=yavs-$version-$target$suffix

  # What is installed now.
  current='' cur_variant=''
  if [ -x "$bindir/yavs" ]; then
    out=$("$bindir/yavs" --version 2> /dev/null | head -n 2) || true
    current=$(printf '%s\n' "$out" | sed -n '1s/^yavs \([0-9][0-9.]*\).*/\1/p')
    case $out in
      *"x86-64-v3"*) cur_variant=x86-64-v3 ;;
      *"x86-64 baseline"*) cur_variant=-compat ;;
    esac
    [ "$arch" = x86_64 ] || cur_variant=
  fi
  if [ -z "$current" ]; then
    if [ -e "$bindir/yavs" ]; then
      say "replacing $bindir/yavs, which does not report a version, with yavs $version"
    else
      say "installing yavs $version into $bindir"
    fi
  elif [ "$current" = "$version" ] && { [ -z "$cur_variant" ] || [ "$cur_variant" = "$variant" ]; }; then
    if [ -z "$force" ]; then
      say "already at $version ($bindir/yavs). Nothing to do; --force installs it again."
      path_note
      exit 0
    fi
    say "reinstalling yavs $version (--force)"
  elif [ "$current" = "$version" ]; then
    say "yavs $current ($cur_variant archive) -> $version ($variant archive): same version, other archive"
  elif vlt "$version" "$current"; then
    say "yavs $current -> $version (a downgrade)"
  else
    say "yavs $current -> $version"
  fi

  # Download and verify.
  base=$releases/download/v$version
  file=$name.tar.gz
  if [ -n "$verify" ]; then
    fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS"
    case $http in
      200) ;;
      404 | error) die "there is no yavs release v$version (HTTP $http for $base/SHA256SUMS). Choose a version listed at $releases, or leave --version out for the newest." ;;
      *) die "could not download $base/SHA256SUMS: HTTP $http. Run again later; if it keeps failing, check $releases and your access to github.com." ;;
    esac
    expected=$(awk -v n="$file" '$2 == n {print $1}' "$tmp/SHA256SUMS")
    [ -n "$expected" ] || die "SHA256SUMS of yavs v$version does not list $file, so it cannot be verified. Choose another version listed at $releases."
  fi
  say "downloading $file"
  fetch "$base/$file" "$tmp/$file"
  case $http in
    200) ;;
    404 | error) die "yavs v$version has no archive $file (HTTP $http). Choose a version at $releases that has one for this system." ;;
    *) die "could not download $base/$file: HTTP $http. Run again later; if it keeps failing, check $releases and your access to github.com." ;;
  esac
  if [ -n "$verify" ]; then
    got=$(sha256_of "$tmp/$file")
    [ "$got" = "$expected" ] ||
      die "$file does not match its line in SHA256SUMS (expected $expected, got $got): the download is damaged or was changed. Nothing was installed. Run the installer again; if it keeps failing, report it at https://github.com/$repo/issues."
    say "checksum ok (SHA-256 from the release's SHA256SUMS)"
  fi

  # Unpack, try the new binary where it is, then put it in place.
  if ! { mkdir "$tmp/x" && tar xzf "$tmp/$file" -C "$tmp/x"; }; then
    die "could not unpack $file. Check the free space in ${TMPDIR:-/tmp} and run again."
  fi
  src=$tmp/x/$name
  [ -f "$src/yavs" ] || die "$file does not hold $name/yavs. Report it at https://github.com/$repo/issues."
  mkdir -p "$bindir" "$sharedir" || die "could not create $bindir and $sharedir. Check the permissions of $prefix, or choose a directory of your own with --prefix DIR."
  new=$bindir/.yavs.$$.new
  if ! { cp "$src/yavs" "$new" && chmod 755 "$new"; }; then
    die "could not write to $bindir. Check its permissions and free space, or choose another directory with --prefix DIR."
  fi
  said=$("$new" --version 2>&1) ||
    die "the downloaded yavs does not run here: ${said:-no output}. If it names a missing CPU feature, run again with --compat; on a musl system such as Alpine install gcompat first. Requirements: $guide. Your installed yavs, if any, is untouched."
  case $said in
    "yavs $version" | "yavs $version "*) ;;
    *) die "asked for yavs $version, but the archive's yavs --version says: $(printf '%s\n' "$said" | head -n 1). Nothing was installed. Report it at https://github.com/$repo/issues." ;;
  esac
  # README and licences first; the binary last, by rename, so a running yavs keeps its old file and an
  # interrupted update leaves the old one working.
  for f in "$src"/README* "$src"/LICENSE*; do
    [ -f "$f" ] || continue
    b=${f##*/}
    if ! { cp "$f" "$sharedir/.$b.$$" && mv -f "$sharedir/.$b.$$" "$sharedir/$b"; }; then
      die "could not write $sharedir/$b. Check the permissions and free space, or choose another directory with --prefix DIR."
    fi
  done
  mv -f "$new" "$bindir/yavs" || die "could not replace $bindir/yavs. Check the permissions of $bindir."
  say "installed yavs $version: $bindir/yavs"
  say "README and licences: $sharedir"
  path_note
}

# path_note: say if yavs is found elsewhere through PATH; if the directory is not on PATH, offer to add
# it to the shell's startup file when a terminal can be asked, else print the line to add.
path_note() {
  case ":$PATH:" in
    *":$bindir:"*)
      other=$(command -v yavs) || true
      if [ -n "$other" ] && [ "$other" != "$bindir/yavs" ]; then
        say "note: $other comes before it on your PATH, so that one runs when you type yavs."
      fi
      return
      ;;
  esac
  shown=$bindir
  [ -z "${HOME:-}" ] || case $bindir in "$HOME"/*) shown=\$HOME/${bindir#"$HOME"/} ;; esac
  marker="# added by the yavs installer"
  case ${SHELL:-} in
    */fish)
      rc=${XDG_CONFIG_HOME:-${HOME:-}/.config}/fish/config.fish
      line="fish_add_path \"$shown\"" ;;
    */zsh) rc=${ZDOTDIR:-${HOME:-}}/.zshrc ;;
    */bash) rc=${HOME:-}/.bashrc ;;
    *) rc=${HOME:-}/.profile ;;
  esac
  [ -n "${line:-}" ] || line="export PATH=\"$shown:\$PATH\""
  say ""
  say "$bindir is not on your PATH."
  case ${YAVS_NO_MODIFY_PATH:-0} in
    0 | "") ;;
    *) say "YAVS_NO_MODIFY_PATH is set, so no startup file is changed. Add this line to your shell's startup file and open a new terminal:"
      say "  $line"
      return ;;
  esac
  if [ -z "${HOME:-}" ] || ! ( : < /dev/tty ) 2> /dev/null; then
    say "Add this line to your shell's startup file (for example ~/.profile or ~/.bashrc) and open a new terminal:"
    say "  $line"
    return
  fi
  rcshown=$rc
  [ -z "${HOME:-}" ] || case $rc in "$HOME"/*) rcshown=\~/${rc#"$HOME"/} ;; esac
  if grep -qxF "$line $marker" "$rc" 2> /dev/null; then
    say "$rcshown already has the line that adds it; open a new terminal."
    return
  fi
  printf 'Add %s to PATH in %s? [y/N] ' "$shown" "$rcshown" > /dev/tty
  ans=
  read -r ans < /dev/tty || true
  case $ans in
    y | Y | yes | YES | Yes)
      if ! { mkdir -p "${rc%/*}" && printf '\n%s %s\n' "$line" "$marker" >> "$rc"; }; then
        die "could not write $rc. Add this line to your shell's startup file yourself: $line"
      fi
      say "added to $rcshown: $line $marker"
      say "Open a new terminal to use yavs by name." ;;
    *)
      say "Not changed. To add it yourself, put this line in your shell's startup file and open a new terminal:"
      say "  $line" ;;
  esac
}

main "$@" --installer-complete
