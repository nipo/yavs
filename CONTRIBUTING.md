# Contributing to yavs

Pull requests are welcome. Small fixes can go straight to a pull request. For anything larger,
open an [issue](https://github.com/ru551n/yavs/issues) first, so nobody builds something that gets
declined. Usage questions go to [Discussions](https://github.com/ru551n/yavs/discussions); issues
are for bugs and planned work. Security problems are ordinary issues too.

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in
yavs, as defined in the Apache-2.0 license, is licensed as MIT OR Apache-2.0, at the user's option,
without any additional terms or conditions.

## Build

You need a recent stable Rust toolchain with rustfmt and clippy, and a C compiler. The test tools
and the documentation need Python 3.

```sh
git clone https://github.com/ru551n/yavs
cd yavs
cargo build                            # target/debug/yavs, interpreter only
cargo build -p yavs --features clift   # adds the Cranelift tier, as in the release archives
cargo build -p yavs --features llvm    # adds the LLVM tier; needs LLVM 21 installed
```

## Test

These need nothing else installed, and CI runs them on every pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p yavs-clift -p yavs --features clift   # when you touch code generation
```

Rendered diagnostics are snapshots: `YAVS_BLESS=1 cargo test -p yavs` updates them, then read the
diff. To compare yavs's simulation output with the outputs checked in under `tests/corpus`:

```sh
cargo build -p yavs
YAVS_BIN=$PWD/target/debug/yavs python3 tools/difftest/difftest.py --candidate yavs --no-sandbox --filter '^arith/'
```

You don't need a reference simulator or any commercial tool, and CI covers the remaining checks.
Generating a new `.expected` file under `tests/corpus` does need a reference simulator
(`difftest.py --bless`), and so does running `difftest.py` without `--candidate yavs`. Prefer a test
that you can finish without one: an integration test in `crates/yavs/tests`, or a case in
`tests/diag/sem`. If only a corpus test will do, put the testbench in your pull request description.
A maintainer runs reference simulator comparisons on request and checks new expected outputs then.

## Pull requests

A pull request needs:

- A test. Every behaviour gets one, and an error gets a negative test with its code and position.
- A release note fragment for a user-visible change: see
  [`release_notes/news.d/README.md`](release_notes/news.d/README.md).
- Documentation when behaviour changes: [`docs/README.md`](docs/README.md) says where it goes and
  how to build the site.

Pull requests are squash merged, so you needn't tidy your history; the maintainer edits the final
message. CI on a first pull request starts after a maintainer approves it.

AI tools are allowed, and agents should read [`AGENTS.md`](AGENTS.md). You don't have to say you
used one, but you, the person submitting, must have run the checks and be able to explain the change.

## Where the rules live

[`AGENTS.md`](AGENTS.md) has the repository map and the project's rules, and each crate and tool
directory has an `AGENTS.md` for its own area: read the one for what you touch. Parts of them are
for the maintainer's own agents and don't apply to you: pushing straight to `main` (you open a pull
request), claiming issues and modules, commit identity, and the limits of the shared build machine.

The standards contributors most often trip over:

- **Errors are diagnostics** with a stable code, the cause and the fix. No silent fallbacks.
- **Every place that takes a name says "did you mean"** on a miss, through the one helper
  `yavs_diag::suggest`.
- **Follow the LRM where it is clear.** Where it is unclear or silent, be permissive: accept what
  other major simulators accept.
- **Write it fresh.** Never copy code or tests from another simulator or VHDL tool. In tracked text,
  say "the reference simulator" rather than naming one.

## Reporting a bug

Open an [issue](https://github.com/ru551n/yavs/issues) with the smallest VHDL that shows it, the
exact command, the output, what you expected, and `yavs --version`. An exit status of 3 is always a
bug in yavs.
