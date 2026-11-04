# AGENTS.md: tools/install

Read the root `AGENTS.md` and `tools/AGENTS.md` first. This is the test of the one-line installers
`install.sh` and `install.ps1` at the repository root (user guide: `docs/getting-started.rst`,
"One-line install").

## Map

| Path | What |
|---|---|
| `../../install.sh` | POSIX `sh`, Linux x86_64 and aarch64 |
| `../../install.ps1` | Windows PowerShell 5.1 and PowerShell 7, Windows x86_64 |
| `test.sh [install.sh]` | Real installs of 0.13.0, 0.14.0 and the newest release (network needed): fresh, update no-op, `--force`, replacing a running file, downgrade, `--compat`, bad checksum (a `curl` wrapper that corrupts `SHA256SUMS`), no checksum tool, `wget` only, truncated scripts at every line and many byte offsets, the PATH question with a terminal (`script`) and without (`setsid`) |
| `test.ps1 [install.ps1]` | The same on Windows, through `Invoke-Expression` like `irm \| iex`; answers the PATH question itself and puts the user PATH back at the end; a child run with `-NonInteractive` tests the real console check |
| `../../.github/workflows/install.yml` | Runs both, on Linux x86_64 and aarch64 (also `shellcheck`) and on Windows in `powershell` and `pwsh`, only when these files change |

## Design decisions

- **Served from `main`** (`https://raw.githubusercontent.com/ru551n/yavs/main/install.sh`), not as a release asset:
  every yavs release is a GitHub pre-release and `/releases/latest/download/` skips those (2026-11-04).
- **Newest version** is the highest plain `vX.Y.Z` tag among the newest 100 releases of the API (no token needed;
  `GITHUB_TOKEN` is used if set; HTTP 403 and 429 name the rate limit and the fix: `--version`). The `vunit-yavs-v*` and
  `vscode-v*` tags are other products. Same method as `yavs-action`.
- **The script is one function called on the last line**, with `--installer-complete` as the last argument: a download cut
  anywhere runs nothing (the truncation test cuts at every line and at many byte offsets).
- **Replacement is a rename**: Linux `mv -f` of a staged file in the same directory (a running file keeps its inode);
  Windows renames the running `yavs.exe` to `yavs.old.exe` first.
- **PATH**: asked only with a terminal (`/dev/tty` on Linux, a console on Windows), default no, never without one or with
  `YAVS_NO_MODIFY_PATH=1`. `install.ps1` writes the user `Path` through the registry with its value kind kept
  (`[Environment]::SetEnvironmentVariable` would turn an expandable `%VAR%` entry into plain text) and broadcasts the
  change.
- **Windows CPU check** is `IsProcessorFeaturePresent(PF_AVX2_INSTRUCTIONS_AVAILABLE)` only; the new `yavs --version` is run
  before it is put in place and checks the rest itself.
- **No sha256 tool**: refuse unless `--allow-unverified`; a mismatch is never overridable.

## Pitfalls

- The tests download about 15 MB per archive, a dozen times. Keep `TMPDIR` on a disk with space.
- `install.ps1` cannot be run on this project's Linux hosts; edit it with that in mind and run the workflow on a branch.
