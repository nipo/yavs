# yavs

[![Documentation](https://img.shields.io/readthedocs/yavs?label=documentation)](https://yavs.readthedocs.io)
[![Repository](https://img.shields.io/badge/github-ru551n%2Fyavs-blue?logo=github)](https://github.com/ru551n/yavs)
[![Release](https://img.shields.io/github/v/release/ru551n/yavs?include_prereleases)](https://github.com/ru551n/yavs/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![CI status](https://github.com/ru551n/yavs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ru551n/yavs/actions/workflows/ci.yml)

yavs (Yet Another VHDL Simulator) is a VHDL simulator written in Rust. It runs VHDL-1993, 2008 and 2019, mixed in one project, from one executable with nothing else to install, on Linux x86_64, Linux aarch64 and Windows x86_64.

## Debug a VHDL test from a prompt or from VS Code

```
$ yavs run --work=work:lib tb --interactive
yavs> break assert error
stop at assertions and reports of severity error, failure
yavs> run
** Error: 25000000fs+1: q is not high
    Process tb.check
assertion of severity error at 25 ns, delta 1, /work/tb.vhd:53: q is not high
yavs> trace x q
tb.q = 'U', never changed, declared at tb.vhd:30
  tb.u_reg.q = 'U', never changed, declared at tb.vhd:7
    driven 'U' by process tb.u_reg.flop, at tb.vhd:15
      tb.u_reg.d = 'U', never changed, declared at tb.vhd:7
        tb.enable = 'U', never changed, declared at tb.vhd:29
Where it starts:
  tb.enable: nothing drives it, it keeps its initial value (tb.vhd:29)
It follows drivers and the signals their processes name. It cannot see variables,
values from files, subprogram parameters, or which signal read made this value.
```

The [debugger tutorial](https://yavs.readthedocs.io/en/latest/debugger-tutorial.html) debugs a failing FIFO test step by step, in VS Code with the waves in Surfer, then at this prompt, with files you can run. The prompt, Surfer, attaching from VS Code without a configuration, `yavs lint`, combination coverage, `--assert-summary` and `--activity` need a release newer than 0.11.0; the [release notes](https://yavs.readthedocs.io/en/latest/release_notes.html) list everything unreleased.

## What it does

- **[Interactive console](https://yavs.readthedocs.io/en/latest/interactive.html):** breakpoints on lines, signal values and failed assertions, stepping, `examine` (also of a past value), `force`, `drivers`, `trace x`, `waits`, `find`, and `restart` to time 0 without recompiling. Tab completes names, and a typo gets a "did you mean". [Every command](https://yavs.readthedocs.io/en/latest/console-reference.html); `--do` runs the same commands as a script.
- **[VS Code debugger](https://yavs.readthedocs.io/en/latest/debugging.html):** `python run.py lib.tb.test --yavs-debug`, set breakpoints in your own files, press F5. No configuration. The Debug Console takes the console's commands, and other Debug Adapter Protocol clients work with `yavs run --dap`.
- **[Live waveforms in Surfer](https://yavs.readthedocs.io/en/latest/waveforms.html#watching-in-surfer):** `--surfer` shows the waves while the run writes them, the cursor follows the debugger's stops, and "Add to Wave" adds a signal or process variable with its history.
- **[VUnit](https://yavs.readthedocs.io/en/latest/vunit.html):** the `vunit-yavs` package adds yavs as a simulator, with `--yavs-debug`, `--yavs-interactive`, `--yavs-surfer`, `--yavs-lint`, coverage flags and a gate, a token server for `-p`, and hooks for packages that call native libraries.
- **[Static analysis](https://yavs.readthedocs.io/en/latest/lint.html):** `yavs lint` reports what can be proved wrong before a simulation runs, with waivers and SARIF, JUnit, Code Climate and SonarQube reports.
- **[Coverage](docs/coverage.md):** statement, branch, toggle, condition, expression and condition-combination coverage; merged databases, text, HTML and Cobertura reports, source comments that waive what cannot occur, `--fail-under` for CI and `yavs cov rank` for a small set of tests.
- **[Answers at the end of a run](https://yavs.readthedocs.io/en/latest/cli.html#control-a-run):** `--assert-summary` (what failed, where), `--activity` (what keeps it busy), `--hang-report` (what each process waits for), `--profile` (where the time goes).
- **[Diagnostics](https://yavs.readthedocs.io/en/latest/troubleshooting.html):** a stable code and the source for every problem, and `yavs --explain E1100`:

  ```
  error[E1100]: `cnt` is not declared
   --> hello_error.vhd:7:12
    |
  5 |   signal count : natural;
    |          ----- similar name declared here
  6 | begin
  7 |   count <= cnt + 1;
    |            ^^^ not found in this scope
    |
    = help: a signal with a similar name exists: `count`
  ```
- **[VHPI](https://yavs.readthedocs.io/en/latest/vhpi.html) and foreign calls:** `--vhpi` loads plugins, with most of the standard there and [the gaps](https://yavs.readthedocs.io/en/latest/vhpi.html#at-a-glance) listed; `--load` calls C functions from VHDL.
- **Languages:** [VHDL-1993, 2008 and 2019](https://yavs.readthedocs.io/en/latest/language-support.html), each file under its own standard, the IEEE, VITAL and Synopsys packages built in, and a [subset of PSL](https://yavs.readthedocs.io/en/latest/psl.html). No Verilog yet ([planned](https://github.com/ru551n/yavs/issues/776)).

## Speed

On 41 third-party testbenches, counting analysis, elaboration and the run, a repeated run took about a fifth of the time of an established open-source VHDL simulator and a first run a little over half (measured 2026-10-28 at commit `1a8edb76`; yavs was slower on 4 of them; [performance](docs/performance.md)). A run starts in the interpreter and compiles hot code with the bundled [Cranelift](https://cranelift.dev/) in the background ([running faster](https://yavs.readthedocs.io/en/latest/running-faster.html)).

`--threads=N` (default 1) helps large, flat RTL designs: up to 2.4 times faster at 4 threads, whole run, measured 2026-10-28. Processor models and testbench-style designs stay at about their one-thread speed ([threads](https://yavs.readthedocs.io/en/latest/threads.html)). Output is identical on every tier and at any thread count.

## Results on other projects' test suites

| Suite | Passed / total |
| --- | ---: |
| OSVVM, VHDL-2008 and VHDL-2019 | 314/314 each |
| VUnit | 1338/1338 |
| UVVM | 131/137 |
| Open Logic | 6867/6867 |
| All suites of the report | 14902/15029 |

From the [conformance report](docs/conformance.md) of 2026-10-29, commit `a3e95822df`, which has every suite, what ran where, and the [known differences between simulators](docs/design/simulator-discrepancies.md).

## Quickstart

Download the archive for your platform from the [releases](https://github.com/ru551n/yavs/releases) page, unpack it and put the directory on `PATH`. Every platform has one archive, with the Cranelift compiled tier built in and nothing else to install. See [`docs/install.rst`](docs/install.rst) for checks, the build variants and building from source. In a GitHub Actions workflow, [yavs-action](https://github.com/ru551n/yavs-action) installs it.

Then analyze files into a library (a directory) and run a top level:

```
yavs analyze --work=work:build/work hello_tb.vhd
yavs run --work=work:build/work hello_tb
```

The example files are in [`docs/examples/`](docs/examples); [Getting started](https://yavs.readthedocs.io/en/latest/getting-started.html) goes on from here.

## With VUnit

Install the `vunit-yavs` package next to VUnit; it needs yavs 0.11.0 or later on `PATH`:

```
pip install vunit-yavs
```

Add it in `run.py` before anything else, then select yavs when you run:

```python
vu = VUnit.from_argv()
vu.add_package("vunit-yavs", allow_setup=True)  # first: it provides the simulator
```

```
VUNIT_SIMULATOR=yavs python run.py
```

## Documentation

The [user guide](https://yavs.readthedocs.io), with [the command-line reference](https://yavs.readthedocs.io/en/latest/cli-reference.html), [error codes](https://yavs.readthedocs.io/en/latest/error-codes.html), [a page for moving from another simulator](https://yavs.readthedocs.io/en/latest/migrating.html), [known limitations](https://yavs.readthedocs.io/en/latest/faq.html) and [release notes](https://yavs.readthedocs.io/en/latest/release_notes.html). It is also available to coding assistants through [Context7](https://context7.com/ru551n/yavs).

yavs is a pre-release: expect gaps, and please [report](https://github.com/ru551n/yavs/issues) what breaks.

## Contributing

Pull requests and bug reports are welcome: see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

yavs is licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your
option. The IEEE `std` and `ieee` sources in `lib/` are Apache-2.0 (`lib/LICENSE`,
`lib/SOURCES.md`). The built-in IEEE 1076.4 VITAL packages (`ieee.vital_timing`,
`ieee.vital_primitives`, `ieee.vital_memory`) are IEEE-copyrighted and carry no explicit licence
grant; they are redistributed unmodified, as other open-source VHDL simulators do (`lib/LICENSE-IEEE-VITAL`). The
built-in Synopsys packages (`ieee.std_logic_arith`, `std_logic_unsigned`, `std_logic_signed`,
`std_logic_misc`) are Synopsys copyright, redistributable without restriction when the notice is
kept, and are also shipped unmodified (`lib/LICENSE-SYNOPSYS`). The VHDL-1993 `numeric_std` and
`numeric_bit` are derived from the Apache-2.0 sources (`lib/SOURCES.md`).
