# Code coverage

Coverage tells you which parts of your design's code ran during a simulation. It is a
two-step workflow:

1. **Run** with `yavs run --cov`. Each run writes one database file (`.ycov`) with the counts.
2. **Report** with `yavs cov`: merge the databases of several runs, then print, export or rank.

You get a `.ycov` database per run, and from it a text summary, an HTML site, Cobertura XML
for CI, or a list of items that never ran. Without `--cov` there are no counters and no cost.

```{contents} On this page
:local:
```

## Example

```
yavs analyze --work=lib:out/lib rtl/*.vhd tb/tb.vhd
yavs run --work=lib:out/lib --cov=a.ycov --cov-kind=statement,branch,toggle,condition tb -gtest=a
yavs run --work=lib:out/lib --cov=b.ycov --cov-kind=statement,branch,toggle,condition tb -gtest=b
yavs cov merge a.ycov b.ycov -o all.ycov
yavs cov report all.ycov --html cov-html --cobertura coverage.xml
yavs cov report all.ycov --uncovered=yaml > exclusions.yaml
yavs cov report all.ycov --total
yavs cov report all.ycov --fail-under=80
yavs cov rank all.ycov
```

The two runs write `a.ycov` and `b.ycov` (named `tb(test=a)` and `tb(test=b)` in the database,
unless you pass `--cov-test`). The report prints one line each for statements, branches,
toggles and conditions with hit/total and a percentage, then a table per instance, then the items that
never ran, with the source text that says what each one is:

```
not covered:
  tb.vhd:6   toggle     rst fall (1->0)                                        tb
  tb.vhd:24  branch     if rst = '1'                                           tb
  tb.vhd:25  statement  count <= 0;                                            tb
  tb.vhd:26  condition  count < 15 is 0 (result 0) in en = '1' and count < 15  tb
```

Source paths are relative to the current directory (`--root DIR` changes that; a file outside it keeps
its full path). A branch is named by its arm in the source: `if cond`, `elsif cond`, `else`, `when
choice` for a `case` alternative or a `when` of an assignment, `(no arm taken)` for the implicit arm of
an `if` or conditional assignment without `else`. Open
`cov-html/index.html` to click through the missed lines. Edit `exclusions.yaml`, deleting the
entries that must stay as gaps, and pass it back with `--exclude exclusions.yaml`. `rank`
tells you whether one of the two tests adds nothing the other lacks.

## What is counted

| Kind | What is counted |
|------|-----------------|
| `statement` | every sequential and concurrent statement, each time it runs (declarations are not counted) |
| `branch` | each arm of `if`/`elsif`/`else` (an `if` without `else` has a "no condition held" arm), each `case` alternative, each choice of a conditional or selected assignment, and the body of a `for`/`while` loop per iteration |
| `toggle` | for every `bit` and `std_ulogic`/`std_logic` scalar of every signal and port, the 0 to 1 and 1 to 0 changes |
| `condition` | for every `if`/`elsif`, conditional assignment, `while`, `exit when` or `next when` with two or more inputs, one row per input and value |
| `expression` | the same for the logic expression assigned to a signal or variable |
| `combination` | for the same conditions and logic expressions, each combination of the values of the inputs that can occur (see Combinations); opt in with `--cov-kind` |

## Flags for `yavs run`

| Flag | Meaning |
|------|---------|
| `--cov[=FILE]` | turn coverage on; write the database to `FILE` (default `yavs.ycov` in the working directory). Write the `=` form: `--cov FILE` does not take the file |
| `--cov-kind=LIST` | comma separated kinds from the table above; default `statement,branch,toggle` |
| `--coverage-directives=MODE` | `apply` (default) checks the `-- yavs coverage exclude` directives in the sources and waives what they name; `ignore` reads none of them and says how many it left out. Also on `yavs analyze` and `yavs elab` |
| `--cov-toggle-max=N` | leave signals with more than `N` scalars (default 65536, for example large memories) out of toggle coverage |
| `--cov-test=NAME` | the name of this run's test in the database |
| `--cov-exclude-translate-off` | also leave `translate_off` ... `translate_on` regions out |

`--cov` cannot be combined with a debug session (`--dap`, `--interactive`, `--do`) yet: the run stops before time 0 with a "not implemented" error (exit 2).

## Commands

| Command | What it does |
|---------|--------------|
| `yavs cov merge a.ycov b.ycov -o all.ycov` | adds up the counts of the runs and keeps, for every item, the tests that hit it |
| `yavs cov report all.ycov [options]` | merges its inputs, then reports (options below) |
| `yavs cov rank all.ycov [--exclude FILE]` | prints a small set of tests that reaches the same coverage, the tests that set does not need, and per test how many items it hit and how many no other test hit |

Report options:

| Option | Output |
|--------|--------|
| (none) | totals such as `statements 18/24 75.0%` for each kind the database holds, one line per instance or package, then the excluded items with their reasons |
| `--cobertura FILE.xml` | Cobertura XML for CI services, one entry per source line (it has no toggles, so those are only in the text and HTML reports) |
| `--html DIR` | static pages that open from disk (`index.html`) |
| `--root DIR` | the prefix stripped from the source paths in every report (default: the current directory) |
| `--kind=LIST` (newer than 0.11.0) | report only these kinds (`statement`, `branch`, `toggle`, `condition`, `expression`, `combination`, comma separated); `--total` and `--fail-under` then count only them |
| `--total` (newer than 0.11.0) | print only the total coverage as one percentage such as `74.3%`: the items that ran over all items, after exclusions; cannot be combined with `--uncovered` or `--tests` |
| `--fail-under=PCT` (newer than 0.11.0) | exit with status 1 when the total is below PCT (`80`, `92.5`); the report is printed as usual and `error[E0114]: coverage 70.0% is below the required 80.0%` follows on standard error. With `--total` the percentage is printed too |
| `--uncovered[=yaml]` | one exclusion row per item that never ran, with file, line and the statement as a comment; `=yaml` prints a YAML exclusion file |
| `--tests` | one row per item that ran, followed by `# tests:` and the tests that hit it; cannot be combined with `--uncovered` |
| `--exclude FILE` | apply an exclusion file (see Exclusions) |
| `--exclusions[=json]` | print the exclusion report instead of the summary (see The exclusion report); the HTML report gets the same page |

## Coverage in CI

`--fail-under` (a release newer than 0.11.0) makes the coverage a gate and `--total` gives a number for a badge
or a log line. A whole job is in [Continuous integration](ci.rst).

```yaml
- run: yavs run --work=lib:out/lib --cov=cov/a.ycov --cov-kind=statement,branch tb
- run: yavs cov merge cov/*.ycov -o cov/all.ycov
- run: yavs cov report cov/all.ycov --html cov-html --cobertura coverage.xml --fail-under=85
- run: echo "coverage $(yavs cov report cov/all.ycov --total)"
```

The percentages are rounded down, so a gate never passes by rounding up: 99.95% is `99.9%`, and
`--fail-under=100` passes only when every item ran. Count a few kinds only with `--kind`, for
example `--kind=statement,branch --fail-under=90`. A report with nothing to count, such as
`--kind=toggle` on a database without toggles, is the error `E0115` rather than 0% or 100%.

## Coverage from VUnit

With the `vunit-yavs` package (a release newer than 0.11.0) the test runner collects, merges and checks coverage for you:

```python
vu.set_sim_option("enable_coverage", True)      # or pass --yavs-cov to run.py
vu.set_sim_option("yavs.cov_kind", ["statement", "branch"])   # optional, as --cov-kind
vu.main(post_run=lambda results: results.merge_coverage("coverage"))
```

Every simulation writes a database named after its test; `merge_coverage("coverage")` merges them
into `coverage.ycov`. `python run.py --yavs-cov-html cov-html --yavs-cov-fail-under 85` then also
writes the HTML report and fails the run (exit status 1, after the test results are written) when
the total is below 85%. Further arguments for `yavs cov report` go in
`merge_coverage("coverage", args=["--cobertura", "coverage.xml"])`. Coverage is not available
under `--yavs-debug`.

## Combinations

`--cov-kind=combination` measures, for every condition or logic expression with two or more inputs,
which combinations of the input values occurred. The inputs are the operands of the logic
operators (`and`, `or`, `nand`, `nor`, `xor`, `xnor`, `not`): in `count > 3 and ready` they are
`count > 3` and `ready`, left to right as written. Each input is *false*, *true* or *not
evaluated*: `and`, `or`, `nand` and `nor` on `boolean` and `bit` do not evaluate the right operand when the left
one decides, and that is a combination of its own, not a false. `std_ulogic` operators evaluate both
operands. For `if valid and ready` on booleans the requirements are `valid` false (with `ready`
not evaluated), `valid` true and `ready` false, and both true. A `std_ulogic` input counts when it is
`'0'`, `'L'`, `'1'` or `'H'`; any other value is no combination. Two readings of one object in a
condition (`a and not a`) agree. Nothing is evaluated extra and no operand's order changes. Combinations are separate from
the `condition` and `expression` rows (focused expression coverage), which stay as they are.

A decision with an input that is constant after elaboration (a generic, a constant), more than 8
inputs, or more than 512 combinations is not measured; with `--cov-kind=combination` that is warning
`W2023` (its other coverage is unaffected).

## Excluding combinations in the source

A requirement that cannot occur is waived with a comment before the decision or assignment
(sequential or concurrent), each exclusion on a line of its own, with an optional reason after
`:`. A directive after code on the same line is `E2024`, since it would apply to what follows,
not to that code:

```vhdl
-- yavs coverage exclude condition (valid=false, ready=true): impossible handshake
-- yavs coverage exclude condition (valid=false, ready=false): idle state
if valid and ready then
  transfer_data;
end if;
```

The short forms are the same exclusions:

```vhdl
-- yavs cov exclude 01: impossible handshake       -- one symbol per input, in source order
-- yavs cov exclude (valid=false, ready=true)      -- named inputs; the others are unrestricted
-- yavs cov exclude 1--: reset handled elsewhere   -- for: if reset or (valid and ready) then
active <= enable and not fault;                    -- an assignment: exclude expression ... or the short form
```

| Symbol | Meaning |
|--------|---------|
| `0` | the input was evaluated and false |
| `1` | the input was evaluated and true |
| `-` | anything, also not evaluated |

- The positions are the inputs in source order, before any optimisation, and values refer to the input
  itself, before a `not` in front of it: for `enable and not fault`, `01` is `enable` false,
  `fault` true. Reordering the operands changes what a positional exclusion means; the named form does not
  care. Exactly one symbol per input: a pattern is never padded.
- A named input must be a plain name (`valid`); other operands (a relation, a function call) are
  addressed by position. A name that is two inputs, or not an input, is an error.
- Consecutive comments (comments and blank lines may stand between them) apply to the construct
  that follows. Before an `if` they apply to its condition only, not to its `elsif`s or nested
  decisions; a comment right before an `elsif` applies to that condition. A directive before a
  conditional or selected assignment is ambiguous and is an error.
- A waiver changes the requirements only: statements, branches and the other kinds count as before.
  The count of a waived combination is still recorded; if it runs, that is shown, not an error.
  Waived requirements leave the totals, which show `-` when none are left.
- The directives and their reasons are saved in the `.ycov` file, so a report never reads the source
  again. Merging a database that waives a combination with one that requires it is an error (`E0137`).
  Editing a comment never changes the identity of a decision.

Directives are checked when the sources are analysed, with or without coverage:

```
error[E2021]: exclusion has 2 positions, but the expression has 3 inputs
  --> src/fifo.vhd:41:5
   |
41 |     -- yavs cov exclude 01
   |     ^^^^^^^^^^^^^^^^^^^^^^ wrong number of positions
42 |     if reset or (valid and ready) then
   |        ------------------------------ the expression
   |
   = note: expected order: reset, valid, ready
   = help: provide one symbol per input; use `-` for an unrestricted input
```

A malformed directive is an error (codes `E2020` to `E2025`: syntax, pattern, operand name, value,
target, command), with a hint when a name is close (`vlaid` for `valid`). It is never repaired or
partly applied. A duplicate or already covered exclusion is warning `W2020`, exclusions that waive
every combination `W2021`, one that matches no combination `W2022` (on booleans the right operand
is not evaluated when the left one decides, so `(valid=false, ready=true)` matches nothing: write
`0-`), and `W2023` is a decision that is not measured. Warnings follow `-W`/`-Werror`.
Only comments that start `-- yavs cov` or `-- yavs coverage` are read: other comments, text in a
string and block comments are not. `--coverage-directives=ignore` reads none of them and prints a note with
the number it left out.

## The exclusion report

`yavs cov report DB --exclusions` lists every rule that takes requirements out of the totals, the
directives in the source and the rows of `--exclude FILE` together:

```
coverage exclusions: 2 rules, 3 requirements excluded

  1. src/fifo.vhd:41  (in the source)  applied
       rule:    condition (valid=false, ready=true)
       reason:  impossible handshake
       matches: 1 requirement, 0 of them ran
         tb.dut  src/fifo.vhd:43  valid is false, ready is true  in valid and ready  [0 hits]
  ...
kind         before       excluded   after
combination  5/8 62.5%    3 (1 ran)  2/5 40.0%
```

For each rule: where it is, its text and reason, what it matched (with the operand values written out for a
combination), the hits and the tests that ran it, and whether it was applied, is redundant (every
requirement is excluded by an earlier rule), matches nothing, or is about a metric that these runs
did not collect. A requirement more than one rule matches is listed under each and counted once. `--exclusions=json`
prints the same as versioned JSON (`"version": 1`), the HTML report gets an `exclusions.html` page. A database written by an
older yavs does not record in-source exclusions and the report says that instead of showing none.
An exclusion file can also waive combinations: `combination INSTANCE HEX12.OCC PATTERN [FILE:LINE]`
(YAML: `kind: combination`, `pattern`), where the pattern uses `0`, `1` and `-` as above and may
match several requirements; `--uncovered` prints these rows.

## Excluding code

In the source:

```vhdl
-- coverage off: debug only
debug_counter <= debug_counter + 1;
-- coverage on
x <= y; -- coverage off-line
```

`coverage off` (reason optional) starts a region whose statements are left out, `coverage on`
ends it, and `coverage off-line` leaves out the statements starting on its line.
`-- yavs coverage ...` is accepted as a synonym.

Outside the source, give `yavs cov report` or `yavs cov rank` an `--exclude FILE`. Copy rows
from `--uncovered`, optionally with a reason after `#`:

```
branch tb.dut.u_fsm  c41e09a7b3d2.0.3  src/fsm.vhd:88   # unreachable: state is one-hot
toggle tb.dut data(*) fall
instance tb.u_debug*                                    # debug logic
file */generated/*.vhd
unit work.pkg_tools
```

Row types are `statement`, `branch`, `condition`, `expression`, `combination`, `toggle`, `instance`, `unit`
and `file`; `*` and `?` match in instance paths, toggle paths, signal names and file
patterns. A file whose name ends in `.yaml` or `.yml` is read as YAML:

```yaml
exclude:
  - kind: branch
    instance: "tb.dut.u_fsm"
    hash: "c41e09a7b3d2.0.3"
    reason: unreachable, state is one-hot
  - kind: toggle
    instance: "tb.dut"
    signal: "data(*)"
    direction: fall        # optional: rise or fall, default both
  - kind: instance
    instance: "tb.u_debug*"
```

Excluded items are left out of the totals and Cobertura and listed under "excluded" with their
reasons; the database keeps the raw counts. Rows match by instance, kind and the hash of the
statement's text, so an exclusion stays valid when the statement moves; the file and line are only
there to read (a stale line is warning `W0110`). `yavs cov report --uncovered DB` and
`--uncovered=yaml` print rows ready to copy:

| Row | Fields |
|-----|--------|
| `statement`, `branch` | `INSTANCE HASH [FILE:LINE]` (YAML: `instance`, `hash`, `line`) |
| `condition`, `expression` | `INSTANCE HEX12.OCC ROW` (YAML: plus `row: N`) |
| `combination` | `INSTANCE HEX12.OCC PATTERN [FILE:LINE]` (YAML: `pattern`) |
| `toggle` | `PATH SCALAR [rise\|fall]` (YAML: `instance`, `signal`, `direction`) |
| `instance` | an instance path and everything below it |
| `unit` | a package, such as `work.pkg_tools` |
| `file` | a file glob, such as `*/generated/*.vhd` |

In YAML, quote values that start with `*` or contain `: `.

## Which tests cover what

Each database names its test: `--cov-test=NAME`, by default the enabled test case of a
`runner_cfg` generic, else the top level with its `-g` generics (`tb(n=0)`). `yavs cov merge`
keeps, for every item, the tests that hit it. `yavs cov rank` prints a small set of tests that
reaches the same coverage and the tests that set does not need; `yavs cov report --tests` lists
the tests that hit each item.

## Details

**Where counts go.** Code is counted per instance and shown under its path (`tb.u_dut`);
package code once under `library.package`. The `std` and `ieee` libraries are not counted.
Items are identified by their path and the text of their statement, not their position, so
databases stay mergeable after edits elsewhere in the file. A statement exclusion works the same
way: it is matched by path and text hash, so it stays excluded when the statement moves, and the
line number in the row is only for reading.

**Toggle wording.** A toggle is `rise (0->1)` or `fall (1->0)` in every report; exclusion rows use
`rise` and `fall`.

**Toggle values.** `'L'` counts as 0 and `'H'` as 1; other values are neither, so a change
through `'Z'` or `'X'` counts nothing.

**Condition and expression coverage.** An input is covered at value `v` when some evaluation
flipped it alone and the result changed. In `if a and (b or c)` with `a` true, `b` false and `c`
true, flipping `a` or `c` alone changes the result and flipping `b` does not: that evaluation covers
`a = 1` and `c = 1`. Short-circuited inputs count as masked. Not measured: `assert` and `wait until`
conditions, vector operands, and trees of more than 31 inputs. These two kinds cost time in every
evaluation of a multi-input condition, so ask for them only when you need them.

**Clocked processes.** A process that does nothing unless a clock edge happens runs only at
that edge, with or without `--cov`. Its edge test therefore counts one run at initialisation
plus one per edge.

**The database.** The `.ycov` file is written at the end of the run whatever its outcome, and
overwritten, never appended to. Its format may change between yavs versions.

**HTML report.** Static pages that open from disk. Missed lines are red, lines that ran in only
some instances purple, lines with a missed branch arm or toggle yellow; click a line to see its
items per instance, with counts and the tests that hit them. Each file also has a page with only
its gaps. Sources are read from the paths stored in the database, so run the command where they
resolve.

**Percentages.** Every percentage is rounded down to one decimal (a gate must not round a missing
item up). The total counts every item of every kind alike: toggles are one item per direction.

**Exit status and diagnostics.** `yavs cov` exits 0, or 1 on an error, when `-Werror` turns a
warning into an error, or when the total is below `--fail-under`; a malformed option, such as a
`--fail-under` that is not a number from 0 to 100, is a usage error with exit status 2.

| Code | Meaning |
|------|---------|
| `E0110` | cannot write the output |
| `E0111` | cannot read a database |
| `E0112` | bad exclusion file |
| `E0113` | HTML report cannot find a source file at its stored path; nothing is written |
| `E0114` | the total coverage is below `--fail-under` (the report was printed) |
| `E0115` | `--total` or `--fail-under` has no item to count: the database holds none of the `--kind`s |
| `W0110` | an exclusion row's line number is stale (the statement is still matched by its hash) |
| `W0111` | an exclusion row matches nothing |
| `W0112` | a `coverage off` region is never closed, so it excludes the rest of the file |
| `W0113` | a `coverage on` without a preceding `coverage off` |
| `W0114` | an excluded item did run |
| `W0115` | a signal was left out of toggle coverage because of `--cov-toggle-max` |

**Tiers and threads.** Counts are the same in every tier and at any `--threads`.

## See also

- [Using yavs from VUnit](vunit.rst): coverage in a run script.
- [Continuous integration](ci.rst): a coverage gate in a CI job.
- [Error codes](error-codes.rst): every diagnostic code.
