# yavs

[![Documentation](https://img.shields.io/readthedocs/yavs?label=documentation)](https://yavs.readthedocs.io)
[![Repository](https://img.shields.io/badge/github-ru551n%2Fyavs-blue?logo=github)](https://github.com/ru551n/yavs)
[![Release](https://img.shields.io/github/v/release/ru551n/yavs?include_prereleases)](https://github.com/ru551n/yavs/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![CI status](https://github.com/ru551n/yavs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ru551n/yavs/actions/workflows/ci.yml)

yavs is a VHDL simulator written in Rust, for VHDL-1993, 2008 and 2019.

## What it does

- [VHDL support](https://yavs.readthedocs.io/en/latest/language-support.html): VHDL-1993, 2008 and 2019, mixed in one project, with the IEEE packages built in.
- [VS Code debugger](https://yavs.readthedocs.io/en/latest/debugging.html): `python run.py lib.tb.test --yavs-debug`, set breakpoints, press F5, no configuration; a [tutorial](https://yavs.readthedocs.io/en/latest/debugger-tutorial.html) walks through it.
- [Interactive console](https://yavs.readthedocs.io/en/latest/interactive.html): breakpoints, stepping, `examine`, `force`, `drivers`, `trace x` and `restart` at a `yavs>` prompt or in a script.
- [Sane error messages, Rust style](https://yavs.readthedocs.io/en/latest/troubleshooting.html)
- [Live waveforms](https://yavs.readthedocs.io/en/latest/waveforms.html): Surfer follows the run and the debugger's stops; FST files for any viewer.
- [Coverage](https://yavs.readthedocs.io/en/latest/coverage.html): from statements to condition combinations, with HTML and Cobertura reports and a CI gate.
- [Static analysis](https://yavs.readthedocs.io/en/latest/lint.html): `yavs lint` reports what can be proved wrong before a simulation.
- [VHPI](https://yavs.readthedocs.io/en/latest/vhpi.html): plugins and C functions called from VHDL.
- [Platforms](https://yavs.readthedocs.io/en/latest/install.html): Linux x86_64, Linux aarch64 and Windows x86_64, one archive with nothing else to install.

Some of this needs a release newer than 0.11.0: see the [release notes](https://yavs.readthedocs.io/en/latest/release_notes.html).

## Quickstart

```
yavs analyze --work=work:build/work hello_tb.vhd
yavs run --work=work:build/work hello_tb
```

[Install yavs](https://yavs.readthedocs.io/en/latest/install.html), then follow [Getting started](https://yavs.readthedocs.io/en/latest/getting-started.html).

### In GitHub Actions

```yaml
- uses: ru551n/yavs-action@main
  with:
    version: 0.11.0
```

[yavs-action](https://github.com/ru551n/yavs-action) installs a release; [Continuous integration](https://yavs.readthedocs.io/en/latest/ci.html) has a whole job.

## With VUnit

The `vunit-yavs` package adds yavs to VUnit as a simulator; it needs yavs 0.11.0 or later: [Using yavs from VUnit](https://yavs.readthedocs.io/en/latest/vunit.html).

## Speed

On 41 third-party testbenches, a repeated run took about a fifth of the time of an established open-source VHDL simulator and a first run a little over half (measured 2026-10-28): [performance](https://yavs.readthedocs.io/en/latest/performance.html).

## Test suites

The test suites of OSVVM, UVVM, VUnit and Open Logic and the VESTs language suite run on yavs: results per suite in the [conformance report](https://yavs.readthedocs.io/en/latest/conformance.html) of 2026-10-29.

## Documentation

- [User guide](https://yavs.readthedocs.io)
- [Command-line reference](https://yavs.readthedocs.io/en/latest/cli-reference.html)
- [Error codes](https://yavs.readthedocs.io/en/latest/error-codes.html)
- [Moving from another simulator](https://yavs.readthedocs.io/en/latest/migrating.html)
- [Known limitations](https://yavs.readthedocs.io/en/latest/faq.html)
- [Release notes](https://yavs.readthedocs.io/en/latest/release_notes.html)
- [Context7](https://context7.com/ru551n/yavs), for coding assistants

yavs is a pre-release: expect gaps, and please [report](https://github.com/ru551n/yavs/issues) what breaks.

## Contributing

Pull requests and bug reports are welcome: see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

yavs is licensed under either of [Apache-2.0](https://github.com/ru551n/yavs/blob/main/LICENSE-APACHE) or [MIT](https://github.com/ru551n/yavs/blob/main/LICENSE-MIT), at your option. The bundled IEEE, VITAL and Synopsys packages have their own terms: see [`lib/SOURCES.md`](https://github.com/ru551n/yavs/blob/main/lib/SOURCES.md), [`lib/LICENSE`](https://github.com/ru551n/yavs/blob/main/lib/LICENSE), [`lib/LICENSE-IEEE-VITAL`](https://github.com/ru551n/yavs/blob/main/lib/LICENSE-IEEE-VITAL) and [`lib/LICENSE-SYNOPSYS`](https://github.com/ru551n/yavs/blob/main/lib/LICENSE-SYNOPSYS).
