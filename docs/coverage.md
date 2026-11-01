# Code coverage

Coverage tells you which parts of your design's code ran during a simulation. It is a
two-step workflow:

1. **Run** with `yavs run --cov`. Each run writes one database file (`.ycov`) with the counts.
2. **Report** with `yavs cov`: merge the databases of several runs, then print, export or rank.

You get a `.ycov` database per run, and from it a text summary, an HTML site, Cobertura, JSON, LCOV or
SonarQube exports for CI, or a list of items that never ran. Without `--cov` there are no counters and no cost.

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
unless you pass `--cov-test`). The report has a table of the kinds the database holds, a line per
instance or package, and how much never ran. On `tests/cli/cov_report.vhd`:

```
Kind         Items  Hit  Missed  Coverage
-----------  -----  ---  ------  --------
statement       23   19       4     82.6%
branch          16   10       6     62.5%
toggle          10    6       4     60.0%
condition        4    3       1     75.0%
combination      3    2       1     66.6%
total           56   40      16     71.4%

Instance    Total        statement    branch       toggle      condition  combination
----------  -----------  -----------  -----------  ----------  ---------  -----------
cov_report  40/56 71.4%  19/23 82.6%  10/16 62.5%  6/10 60.0%  3/4 75.0%  2/3 66.6%

16 items not covered; `--detailed` shows them.
```

A `-` in the instance table means that path has no item of the kind. The text is plain ASCII, so it
stays readable in logs and CI.

### What is left to do

`yavs cov report all.ycov --below=100 --detailed` is the debugging view: the instances not fully
covered, worst first, each with a table per kind and then an entry for what never ran. An instance
section starts with the tables, covered and missed items alike:

```
=== cov_report  (cov_report.vhd) ===

statement:
  Line  Item                              Hits  Status
  ----  --------------------------------  ----  -------
  ...
  25    if rst = '1' then                    3  covered
  26    n <= 0;                              0  MISSED
      exclude with this row in an `--exclude` file: statement cov_report c7099ba2c6e1.0 cov_report.vhd:26
  ...

condition:
Line 27: en = '1' and n < 15   (condition)
Input totals: 1 of 2 inputs covered, 50.0%
  Input     Covered  Reason not covered  Hint
  --------  -------  ------------------  ----------------------------------
  en = '1'  yes
  n < 15    no       value 0 not hit     make `n < 15` 0 with en = '1' is 1

  Row  Hits  Target         Other inputs must be
  ---  ----  -------------  --------------------  -----------------------------------------------------------------------
    1     2  en = '1' is 0  n < 15 is 1
    2     1  en = '1' is 1  n < 15 is 1
    3     0  n < 15 is 0    en = '1' is 1         <- missed (exclude with -- yavs cov exclude row -0)
    4     1  n < 15 is 1    en = '1' is 1

combination:
Line 27: en = '1' and n < 15
  Combination                                 Hits  Status
  ------------------------------------------  ----  ----------------------------------------------------------------
  en = '1' is false, n < 15 is not evaluated     2  covered
  en = '1' is true, n < 15 is false              0  MISSED (exclude with -- yavs cov exclude 10)
  en = '1' is true, n < 15 is true               1  covered
```

- Hits of a toggle row: bits in a run of one direction and one count share a row, `data(0..7)`, whatever
  direction the signal is declared in.
- A condition or expression not fully covered gets the table above, one row per input and value: the
  target, its hits, and what the other inputs must be for this input to decide the result. A fully covered
  one is a single line. A row is hit only when flipping that input alone changes the result, so a value
  masked by a short circuit stays missed.
- Beside a missed item is the exact text to exclude it. A combination takes the comment
  `-- yavs cov exclude 10` and a condition or expression row `-- yavs cov exclude row -0`, one symbol per input in
  source order, before the decision. Adding the inputs after it makes yavs check them against reordering;
  a conditional or selected assignment, where a comment is ambiguous, and the other kinds take a row for an
  `--exclude` file. These texts are tested by pasting them.
- Excluded items show as `excluded`, with the reason, not as missed.

The entries follow, worst first, in the form of the diagnostics. Where one cause explains several items,
such as a branch arm with statements inside it, they are listed under the cause:

```
note: signal did not toggle fully: `rst`
 --> cov_report.vhd:7:14
  |
7 |   signal en, rst : bit := '0';
  |              ^^^ declared here
  |
  = note: rst: no transition, and it ended at 0: stuck at 0
  = help: drive rst to 1 and back to 0

note: branch arm never taken: `if rst = '1'`
  --> cov_report.vhd:25:8
   |
25 |     if rst = '1' then
   |        ^^^^^^^^^ never taken
   |
   = note: the other arms: `elsif en = '1' and n < 15` ran 1 time, `elsif n > 100` ran 0 times, `(no arm taken)` ran 2 times
   = note: this explains 1 item more, which cannot run until it does:
           line 26: statement `n <= 0;`
   = help: `rst = '1'` was never true when this ran: make it true

note: condition not fully covered: `en = '1' and n < 15`
  --> cov_report.vhd:27:24
   |
27 |     elsif en = '1' and n < 15 then
   |                        ^^^^^^ here
   |
   = note: seen deciding the result: en = '1' at 0, en = '1' at 1, n < 15 at 1
   = help: make `n < 15` 0 with en = '1' is 1

cov_report: 16 of 56 items not covered (6 branch, 1 combination, 1 condition, 4 statement, 4 toggle)
```

A state or transition of a state machine has a row in its own table and an entry; the database holds its file and
line, not a column, so it says so where the source would be. A tip says only what the data supports. Where the data does not say, the tip says so: a statement that
is in no branch arm and never ran says the data does not tell what kept it from running. A toggle reports
the level the scalar ended at; merged runs that ended differently are not called stuck. With more than one
test, an entry names the tests that came closest, which reached the decision and went elsewhere. Without
the sources, embedded or on disk, an entry says so and prints the rest, and a file edited since the run
is not underlined at the wrong place.

`--uncovered` is the row format to paste into an exclusion file, one row per item, and with the HTML report
it also gives the result an input decides alone.

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
| `condition` | for every `if`/`elsif`, conditional assignment, `while`, `exit when`, `next when` or `wait until` with two or more inputs, one row per input and value |
| `assert` | the same rows for the condition of every `assert`, sequential or concurrent; opt in with `--cov-kind`, because half of these rows need a failing run (see Conditions of `wait until` and `assert`). They report as `condition` |
| `expression` | the same for the logic expression assigned to a signal or variable |
| `combination` | for the same conditions (not `wait until`) and logic expressions, each combination of the values of the inputs that can occur (see Combinations); opt in with `--cov-kind` |
| `fsm` | for every detected state machine, per instance, each state reached and each transition taken (see State machines); opt in with `--cov-kind` |

## Flags for `yavs run`

| Flag | Meaning |
|------|---------|
| `--cov[=FILE]` | turn coverage on; write the database to `FILE` (default `yavs.ycov` in the working directory). Write the `=` form: `--cov FILE` does not take the file |
| `--cov-kind=LIST` | comma separated kinds from the table above; default `statement,branch,toggle` |
| `--coverage-directives=MODE` | `apply` (default) checks the `-- yavs coverage exclude` directives in the sources and waives what they name; `ignore` reads none of them and says how many it left out. Also on `yavs analyze` and `yavs elab` |
| `--cov-toggle-max=N` | leave signals with more than `N` scalars (default 65536, for example large memories) out of toggle coverage |
| `--cov-test=NAME` | the name of this run's test in the database |
| `--cov-exclude-translate-off` | also leave `translate_off` ... `translate_on` regions out |
| `--cov-embed-sources` | store the text of the source files in the database, so `yavs cov report --html` needs no checkout. Off by default: **the source text then travels with the database**, so think of who gets the file. Needs `--cov` |

`--cov` cannot be combined with a debug session (`--dap`, `--interactive`, `--do`) yet: the run stops before time 0 with a "not implemented" error (exit 2).

## Commands

| Command | What it does |
|---------|--------------|
| `yavs cov merge a.ycov b.ycov -o all.ycov` | adds up the counts of the runs and keeps, for every item, the tests that hit it |
| `yavs cov report all.ycov [options]` | merges its inputs, then reports (options below) |
| `yavs cov rank all.ycov [--exclude FILE]... [--config FILE]` | prints a small set of tests that reaches the same coverage, the tests that set does not need, and per test how many items it hit and how many no other test hit |

Report options:

| Option | Output |
|--------|--------|
| (none) | a table with `Kind`, `Items`, `Hit`, `Missed` and `Coverage` for each kind the database holds and the total; one line per instance or package with its total and each kind; then a count of what never ran and a count of what is excluded |
| `--detailed` | also, per instance after `--below`, `--kind` and the exclusions, a table per kind with every item and an entry with source and tip for what never ran (see What is left to do). Cannot be combined with `--total`, `--uncovered`, `--tests` or `--exclusions` |
| `--cobertura FILE.xml` | Cobertura XML for CI services, one entry per source line (it has no toggles, so those are only in the text and HTML reports). A line whose statement is left out by `--kind` or an exclusion takes its hits from its branches and condition rows |
| `--html DIR` | static pages that open from disk (`index.html`) |
| `--json FILE` | JSON with every kind and the tests (see Exports) |
| `--lcov FILE` | an LCOV tracefile: lines and branches (see Exports) |
| `--sonarqube FILE` | a SonarQube generic coverage report: lines and branches (see Exports). Not validated against a real scanner |
| `--root DIR` | the prefix stripped from the source paths in every report (default: the current directory) |
| `--kind=LIST` (newer than 0.11.0) | report only these kinds (`statement`, `branch`, `toggle`, `condition`, `expression`, `combination`, `fsm`, comma separated); `--total` and `--fail-under` then count only them |
| `--total` (newer than 0.11.0) | print only the total coverage as one percentage such as `74.3%`: the items that ran over all items, after exclusions; cannot be combined with `--uncovered` or `--tests` |
| `--fail-under=PCT` (newer than 0.11.0) | exit with status 1 when the total is below PCT (`80`, `92.5`); the report is printed as usual and `error[E0114]: coverage 70.0% is below the required 80.0%` follows on standard error. With `--total` the percentage is printed too. `--fail-under=statement:90,branch:80` sets a percentage per kind and names each kind that fails; a plain number may stand beside them (`--fail-under=85,branch:80`) |
| `--below=PCT` | limit the table of instances and packages to those below PCT, worst first, with a total column and a line counting the rest (`2 of 4 at or above 100.0% left out`), or `no instance or package is below 100.0%`. Counts what the report counts (`--kind`, exclusions), rounded down. Totals and `--fail-under` do not change. Cannot be combined with `--total`, `--uncovered`, `--tests` or `--exclusions` |
| `--uncovered[=yaml]` | one exclusion row per item that never ran, with file, line and the statement as a comment; `=yaml` prints a YAML exclusion file |
| `--tests` | one row per item that ran, followed by `# tests:` and the tests that hit it; cannot be combined with `--uncovered` |
| `--exclude FILE` | apply an exclusion file (see Excluding code); repeat it to apply several: the rules add up |
| `--config FILE` | read the options below from a YAML file (see Configuration file) |
| `--exclusions[=json]` | print the exclusion report instead of the summary (see The exclusion report); the HTML report gets the same page |

## Configuration file

`--config FILE` on `yavs cov report` and `yavs cov rank` reads what a project always asks for from one
file, so a CI job and a person at a terminal count the same. No file is ever found on its own: without
the option nothing is read.

```yaml
exclude: [coverage/generated.yaml, coverage/waived.txt]  # exclusion files, as --exclude
root: ..                                                  # as --root
kind: [statement, branch]                                 # as --kind
```

`exclude` and `root` are relative to the configuration file; each value is one word or a list. Files of
`--exclude` add up with those of the file, which come first. `--root` and `--kind` on the command line
win over the file. `yavs cov rank` uses `exclude` and checks the other keys. Where the output goes and
`--fail-under` stay on the command line. An unknown key or kind, or an unreadable file, is `E0138`, with
the line and a near name.

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

A gate per kind is `--fail-under=statement:90,branch:80`. Each kind is counted on its own, rounded
down the same way, and every kind that falls short is named:

```
error[E0114]: statement coverage 82.6% is below the required 90.0%; branch coverage 62.5% is below the required 80.0%
```

A kind in that list that the database does not hold is `E0115` too.

`yavs cov report all.ycov --below=100 --detailed` is the "what is left to do" view: every instance and package
not fully covered, worst first, with its items. An instance at 99.95% is listed, as `99.9%`, because percentages round down;
one at exactly 100% or with nothing to count is not. The text report has no per-file view, so the limit
applies to instances only.

## Coverage from VUnit

With the `vunit-yavs` package (a release newer than 0.11.0) the test runner collects, merges and checks coverage for you:

```python
vu.set_sim_option("enable_coverage", True)      # or pass --yavs-cov to run.py
vu.set_sim_option("yavs.cov_kind", ["statement", "branch"])   # optional, as --cov-kind
vu.set_sim_option("yavs.cov_embed_sources", True)             # optional, as --cov-embed-sources
vu.main(post_run=lambda results: results.merge_coverage("coverage"))
```

Every simulation writes a database named after its test; `merge_coverage("coverage")` merges them
into `coverage.ycov`. `--yavs-cov-kind fsm,assert` on the command line gives the kinds of tests without that option. `python run.py --yavs-cov-html cov-html --yavs-cov-fail-under 85` then also
writes the HTML report and fails the run (exit status 1, after the test results are written) when
the total is below 85%; `--yavs-cov-fail-under statement:90,branch:80` sets a percentage per kind; `--yavs-cov-below 100 --yavs-cov-detailed` is the view above, and in a script these go through `args`. Further arguments for `yavs cov report` go in
`merge_coverage("coverage", args=["--cobertura", "coverage.xml"])`. Exclusions go in
`args` too (`["--exclude", "waived.yaml", "--config", "coverage.yaml"]`) or on the command line as
`--yavs-cov-exclude FILE` (repeatable) and `--yavs-cov-config FILE`. Coverage is not available
under `--yavs-debug`.

## Exports

Every export leaves excluded items out, follows `--kind`, and has its paths after `--root`.

**JSON** (`--json FILE`) holds every kind and the tests. The database itself is private to yavs and can change;
this schema is what a tool should read, and its `version` goes up when a field changes meaning or goes away. On
`tests/cli/cov_report.vhd`, cut short at `...`:

```
{
  "version": 1,
  "tests": ["cov_report"],
  "totals": [
    {"kind": "statement", "items": 23, "hit": 19, "missed": 4, "coverage": 82.6},
    {"kind": "branch", "items": 16, "hit": 10, "missed": 6, "coverage": 62.5},
    {"kind": "total", "items": 39, "hit": 29, "missed": 10, "coverage": 74.3}
  ],
  "items": [
    {"kind": "statement", "instance": "cov_report", "file": "cov_report.vhd", "line": 39, "column": 21, "hits": 1, "tests": ["cov_report"], "statement": "v := 2;"},
    {"kind": "branch", "instance": "cov_report", "file": "cov_report.vhd", "line": 53, "column": 18, "hits": 2, "tests": ["cov_report"], "arm": 0, "arm_text": "when sel < 4"},
    ...
  ],
  "unmeasured": []
}
```

| Field of an item | Kinds | Meaning |
|------------------|-------|---------|
| `kind`, `instance`, `file`, `line`, `hits`, `tests` | all | what and where, the count, the tests that hit it |
| `column` | all | 1-based; 0 where the database holds no position |
| `statement` | statement | the first line of its text |
| `arm`, `arm_text` | branch | its number and how the source names it |
| `signal`, `direction` | toggle | the scalar, `rise` or `fall` |
| `input`, `value`, `expression_text`, `other_inputs` | condition, expression | the row: an input at 0 or 1, the whole decision, what the other inputs must be |
| `inputs`, `pattern`, `expression_text` | combination | the inputs in source order, one symbol per input (`0`, `1`, `-`), the decision |
| `signal`, `state`, or `signal`, `from`, `to` | fsm | the register and a state, or a transition |

`coverage` is a percentage rounded down to one decimal, like every percentage here.
Beside `items`, `unmeasured` lists what looks like a state machine and is not measured: `file`, `line` and `text`.
Rows of `assert` and `wait until` conditions (`--cov-kind=assert`) are `condition` items.

**LCOV** (`--lcov FILE`) has `DA` per line with a statement and `BRDA` per branch arm, summed over instances. It
cannot hold toggles, conditions, expressions, combinations, state machines, instances or tests. This export is **not validated**: it
follows the tracefile format of the LCOV documentation and no LCOV tool has read it here.

**SonarQube** (`--sonarqube FILE`) is the generic coverage format of SonarSource's documentation, for
`sonar.coverageReportPaths`: a `lineToCover` per line, with its branches. It cannot hold what LCOV cannot, and neither lists what was not measured as a state machine. This export is
**not validated**: it follows the published format and no scanner has read it here. A scanner also ignores a path it has
not indexed, so check that it indexes your VHDL files.
## State machines

`--cov-kind=fsm` counts, per instance, how often the register of a state machine entered each state and
each transition it took. It only reads the register's value as the simulation changes it, so it adds no
counter to the code and a run prints what it prints without coverage. It costs a look at each signal
that changes, in every cycle.

**What counts as a machine.** The same machines as `lint_710`, `lint_711` and `yavs lint --facts`: a signal of an enumeration type (declared anywhere, but not by the `std`
or `ieee` libraries) in an architecture, block or generate, when

- a process or concurrent assignment with an edge test drives it, and
- a `case` of a process in the same architecture selects on it, with the signal itself as the selector.

Each such signal is one machine, whatever else drives it. The forms covered are one process, two
processes (`state <= next_state` in the clocked one, `case state is ... next_state <= B` in the other,
with or without a default assignment) and `wait until rising_edge(clk)`. The next-state signal is not a
machine of its own: it may change several times within a time step, the register only once.

**What is reported as not measured.** These have no `fsm` items. Each is warning `W0120` at the run,
and `yavs cov report` lists it under `not measured as state machines:`

- a register of an enumeration type that reads itself and that no `case` selects on (`if state = a then`),
- a `case` on a part of a signal (`case r.state is`, `case states(i) is`),
- a `case` on a port that the architecture drives, and on a variable of a clocked process.

The warning for a variable is a guess: a variable set from a port and then selected on is no machine.

**What is not detected at all.** Nothing says these are state machines, so nothing warns: a state held in
a `std_logic_vector`, an integer or a constant-encoded type, a `case` in a subprogram with a register that
does not read itself in the process, and a register driven only through a port map of another instance.

**States.** All literals of the type are items, including those the signal's subtype leaves out. The
initial value counts as reached once, at time 0.

**Transitions.** The items are the transitions a `case` alternative names: an assignment under `when A`
of the literal `B` to the register or to a signal a clocked process copies into it, nested in an `if` or
not. A value that is not a literal names no transition. A transition that happens and is not named, such
as a reset from `busy` to `idle`, is added to the database when it happens, and its line is the signal's
declaration. So it only ever counts as hit: the total grows with it, and a run with more resets shows a
slightly higher percentage. A transition from a state to itself is never an item, since the value does
not change and there is nothing to observe.

In `--detailed`, a table lists each state and transition of an instance, and what never happened gets an entry:

```
fsm:
  Line  Item                 Hits  Status
  ----  -------------------  ----  -------
  16    one: done               1  covered
  16    one: idle               2  covered
  16    one: spare              0  MISSED
      exclude with this row in an `--exclude` file: fsm cov_fsm.u_far one spare
  ...
```

An exclusion row names the instance, the signal and the state or transition, and `*` matches in all three:

```
fsm tb.dut state spare                # unused state
fsm tb.dut state spare->*
```

A text row cannot hold a character literal with a space or a `#` (`' '`): match it with `*`, or use a YAML file.

Cobertura has no notion of these items and leaves them out; the text and HTML reports show them. The
items are identified by those names, so an edit keeps them mergeable, and renaming a state makes new ones.

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

## Excluding combinations and rows in the source

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
-- yavs cov exclude 01 (valid, ready)              -- the same, and yavs checks the inputs
-- yavs cov exclude (valid=false, ready=true)      -- named inputs; the others are unrestricted
-- yavs cov exclude (count > 3 = false)            -- any operand, by its text
-- yavs cov exclude 1--: reset handled elsewhere   -- for: if reset or (valid and ready) then
active <= enable and not fault;                    -- an assignment: exclude expression ... or the short form
```

| Symbol | Meaning |
|--------|---------|
| `0` | the input was evaluated and false |
| `1` | the input was evaluated and true |
| `-` | anything, also not evaluated |

A pattern waives combinations. With the word `row` in front it waives one `condition` or `expression` row
(one input at one value) instead, so the two can never be read as each other:

```vhdl
-- yavs cov exclude row -0-: never low in practice     -- one symbol per input, exactly one `0` or `1`
-- yavs cov exclude row -0- (valid, count < 15, ready) -- the same, and yavs checks the inputs
-- yavs cov exclude row count < 15 = 0                 -- any operand by its text; value 0, 1, false or true
if valid and count < 15 and ready then
```

`condition row ...` and `expression row ...` say which kind you mean and fit the same constructs as the
kind words below. A report suggests the bare form, `row -0-`, for a missed row; add the list, `row -0- (valid, count < 15, ready)`, if a reordering of the inputs must be caught. A pattern without `row` always means
combinations, even with one symbol set, and `-- yavs coverage exclude condition (valid=false)` keeps meaning the
combinations with `valid` false: the word `condition` there says which decision, not which kind of requirement.

- The positions are the inputs in source order, before any optimisation, and values refer to the input
  itself, before a `not` in front of it: for `enable and not fault`, `01` is `enable` false,
  `fault` true. Reordering the operands changes what a positional exclusion means; the named form does not
  care. Exactly one symbol per input: a pattern is never padded.
- A named input is the operand's text in the source: `valid`, `count > 3`, `f(a, b)`, `en = '1'`. The value
  follows the last `=`, so `(en = '1' = true)` is the operand `en = '1'`. Case, blanks and outer parentheses do not
  count, so `(COUNT>3 = false)` is the same input; case in a string, character literal or extended identifier does.
  Text that is two inputs, or not an input, is an error that lists the inputs, with a near one if there is one.
- A pattern may be followed by its inputs in source order: `01 (en, count < 15)`, `row -0- (a, b, c)`, then the reason.
  yavs compares them with the decision when it analyses the file, with or without `--cov`: inputs that no longer match,
  in order or text, are the error `E2022`, which shows what it found and expected and the directive to write, with the
  symbols moved to follow their inputs. Case, blanks, comments and outer parentheses do not count; case in a string,
  character literal or extended identifier does. The list cannot be read as the named form, which starts with `(` where
  a pattern would stand, and its entries have no value, so `=` in one is operand text.
- A pattern without the list, `01` or `row -0-`, is a full form with no warning. Nothing checks it against reordering:
  if the operands are reordered, its positions mean other inputs, and yavs cannot tell. Write the list, or the named form,
  which does not depend on the order, where that matters. `--coverage-directives=ignore` reads none of these.
- Consecutive comments (comments and blank lines may stand between them) apply to the construct
  that follows. Before an `if` they apply to its condition only, not to its `elsif`s or nested
  decisions; a comment right before an `elsif` applies to that condition. A directive before a
  conditional or selected assignment is ambiguous and is an error.
- A waiver changes only the requirements it names: a combination waiver takes out combinations and a `row`
  waiver takes out one condition or expression row; every other row, combination, statement and branch counts as
  before. The count of a waived requirement is still recorded; if it runs, that is shown in the exclusion report,
  not an error. Waived requirements leave the totals, which show `-` when none are left. A decision with a
  row but too many inputs for combinations can still have that row waived.
- The directives and their reasons are saved in the `.ycov` file, so a report never reads the source
  again. Merging a database that waives a requirement with one that requires it is an error (`E0137`).
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

A malformed directive is an error (codes `E2020` to `E2025`: syntax, pattern, operand, value,
target, command), with a hint when a name is close (`vlaid` for `valid`). It is never repaired or
partly applied. A duplicate or already covered exclusion is warning `W2020`, exclusions that waive
every combination or every row `W2021`, one that matches no combination or names an input with no rows `W2022` (on booleans the right operand
is not evaluated when the left one decides, so `(valid=false, ready=true)` matches nothing: write
`0-`), and `W2023` is a decision that is not measured. Warnings follow `-W`/`-Werror`.
Only comments that start `-- yavs cov` or `-- yavs coverage` are read: other comments, text in a
string and block comments are not. `--coverage-directives=ignore` reads none of them and prints a note with
the number it left out.

## The exclusion report

`yavs cov report DB --exclusions` lists every rule that takes requirements out of the totals, the
directives in the source and the rows of every `--exclude FILE` together, each with the file and line it
came from:

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

Outside the source, give `yavs cov report` or `yavs cov rank` an `--exclude FILE`, as many as you
like: the rules of all the files add up, in the order given, and the exclusion report names the file
and line of each. Copy rows from `--uncovered`, optionally with a reason after `#`:

```
branch tb.dut.u_fsm  c41e09a7b3d2.0.3  src/fsm.vhd:88   # unreachable: state is one-hot
toggle tb.dut data(*) fall
instance tb.u_debug*                                    # debug logic
file */generated/*.vhd
unit work.pkg_tools
entity fifo_debug                                       # every instance of it
process tb.dut.u_fsm p_trace                            # a labelled process
library vendor_models
```

Row types are `statement`, `branch`, `condition`, `expression`, `combination`, `toggle`, `fsm`, `instance`, `unit`,
`file`, `files`, `files_exclude`, `library`, `entity` and `process`; `*` and `?` match in instance paths, toggle paths, signal names and file
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
  - kind: file
    files: [rtl/generated, "rtl/**/old_*.vhd"]
    files_exclude: rtl/generated/keep.vhd
    reason: generated code
```

`files` and `files_exclude` work as they do in a [lint waiver](lint.rst): one word or a list, each a
file, a plain directory or a pattern where `*` and `?` stop at `/` and `**` does not, relative to the
directory of the exclusion file. `files_exclude` wins, and without `files` it means every file except
those. A pattern reaches only below that directory (`..` does not leave it); for a file elsewhere, start it with
`/` or `**/`. They also match the path as the database holds it, and a bare file name matches that file in any
directory. In a text file the same rule is one row, without spaces in the patterns:
`files rtl/generated rtl/old.vhd files_exclude rtl/generated/keep.vhd`. A pattern that covers no source file
of the database is warning `W0111`, with a near name if there is one. The older `file GLOB` row keeps its
matching (any case, `*` crosses `/`, any trailing part of the path) and also takes a plain directory.

Excluded items are left out of the totals and Cobertura; the report counts them and `--detailed` shows them as `excluded`, with their
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
| `fsm` | `INSTANCE SIGNAL STATE` or `INSTANCE SIGNAL FROM->TO` (YAML: `instance`, `signal`, `item`) |
| `instance` | an instance path and everything below it |
| `unit` | a package, such as `work.pkg_tools` |
| `library` | a library: the items of the instances of its entities and of its packages (YAML: `library`) |
| `entity` | the items of every instance of an entity, `NAME` or `LIBRARY.NAME`, toggles included but not the instances below them (YAML: `entity`) |
| `process` | `INSTANCE LABEL`: the statements, branches, conditions and expressions of a labelled process, also a labelled concurrent statement. An unlabelled one has no name and cannot be matched; its items stay reachable by hash. Toggles belong to signals, not processes (YAML: `instance`, `process`) |
| `file` | a file glob or directory, such as `*/generated/*.vhd` |
| `files`, `files_exclude` | lists of files, directories and patterns relative to the exclusion file (YAML: under `kind: file`) |

In YAML, quote values that start with `*` or contain `: `.

`--uncovered` prints one row per item, since that is what can be checked and kept, and adds the process to the
comment (`# y <= 11;  [process decode]`) so a `process` row is easy to write; a process, entity or library that
never ran at all has no row of its own. These three rows need names only databases of this release record: over
an older database they are the error `E0112`, which says to run the simulations again.

When a statement is edited its hash changes, so a row for it matches nothing and warning `W0111` says so. A
row with a line also names what stands at or near that line now, of the same kind and instance, with the row
that would exclude it. It is never excluded on its own: read it, and copy the row if the exclusion still holds:

```
warning[W0111]: ex.txt:1: the exclusion matches nothing in the coverage database
 |
 = note: line 45 of cov_report.vhd now holds `v := 7;` (cov_report): it probably replaced the excluded one
 = help: if the requirement is still waived, the row is now: statement cov_report 000a6bd22275.0 cov_report.vhd:45
```

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

**Toggle wording.** A toggle is `rise (0->1)` or `fall (1->0)` in the reports; exclusion rows use `rise` and `fall`.

**Toggle values.** `'L'` counts as 0 and `'H'` as 1; other values are neither, so a change
through `'Z'` or `'X'` counts nothing.

**Condition and expression coverage.** An input is covered at value `v` when some evaluation
flipped it alone and the result changed. In `if a and (b or c)` with `a` true, `b` false and `c`
true, flipping `a` or `c` alone changes the result and flipping `b` does not: that evaluation covers
`a = 1` and `c = 1`. Short-circuited inputs count as masked. Not measured: vector operands and trees of more than 31
inputs. These kinds cost time in every evaluation of a multi-input condition, so ask for them only
when you need them.

**Conditions of `wait until` and `assert`.** The condition of a `wait until` has `condition` rows. It
is evaluated when it would be without coverage, at each event of its signals, and only the counting is
added: no evaluation more and no resumption at another time. A wait that ends by its timeout counts
nothing for that end. An edge test in the condition has no rows: it is what resumes the wait, and a
clocked process then runs only at its edge, so its false row could not be hit. In
`wait until rising_edge(clk) and en = '1'` that leaves one input, so no rows at all, as for any
condition with one input.

The condition of an `assert` has the same rows under its own kind, `--cov-kind=assert`, and they report
as `condition`. It is not part of `condition`, for two reasons. An assertion that holds never takes
its false result, so in `assert a or b` the rows `a = 0` and `b = 0` are hit only by a failing run: on a
passing regression at most half of these rows can be hit, and a gate on `condition` would drop for code
that is fine. And testbench assertions are often the most evaluated conditions of a run, each with the
cost of any condition. Exclude the rows of the failing result, or read them as a list of what no test
provoked. Neither form has `combination` items, and a `-- yavs cov exclude` comment right before one
is error `E2024`: exclude a row of these with an exclusion row instead.

**Clocked processes.** A process that does nothing unless a clock edge happens runs only at
that edge, with or without `--cov`. Its edge test therefore counts one run at initialisation
plus one per edge.

**The database.** The `.ycov` file is written at the end of the run whatever its outcome, and
overwritten, never appended to. Its format is private to yavs and may change between versions; for
another tool use an export. yavs reads the databases of older versions, and a database a newer yavs
wrote is an error that names both format versions (`E0111`): re-run the simulation with this yavs, or
report with the yavs that wrote it.

**HTML report.** Static pages that open from disk. Missed lines are red, lines that ran in only
some instances purple, lines with a missed branch arm or toggle yellow; click a line to see its
items per instance, with counts and the tests that hit them. Each file also has a page with only
its gaps. Sources come from the database when the run had `--cov-embed-sources`; otherwise they are read
from the paths stored in the database, so run the command where they resolve (`E0113` if not). Merged
databases of an edited file warn (`W0121`) and show the last text.

**Percentages.** Every percentage is rounded down to one decimal (a gate must not round a missing
item up). The total counts every item of every kind alike: toggles are one item per direction.

**Exit status and diagnostics.** `yavs cov` exits 0, or 1 on an error, when `-Werror` turns a
warning into an error, or when the total or a kind is below `--fail-under`; a malformed option, such as a
`--fail-under` that is not a number from 0 to 100, is a usage error with exit status 2.

| Code | Meaning |
|------|---------|
| `E0110` | cannot write the output |
| `E0111` | cannot read a database: it is unreadable, is not a database, or has a format version this yavs does not read |
| `E0112` | an exclusion file cannot be read or has a bad row, or a `library`, `entity` or `process` row meets a database too old to hold those names; the message names the file and the line |
| `E0138` | the file of `--config` cannot be read or has a bad key; the message names the file and the line |
| `E0113` | HTML report cannot find a source file at its stored path and the database does not embed it; nothing is written |
| `E0114` | the total coverage, or a kind in `--fail-under=KIND:PCT`, is below its requirement (the report was printed) |
| `E0115` | `--total` or `--fail-under` has no item to count: the database holds none of the `--kind`s, or none of a kind `--fail-under` names |
| `W0110` | an exclusion row's line number is stale (the statement is still matched by its hash) |
| `W0111` | an exclusion row matches nothing, with the statement that probably replaced it (see Excluding code) |
| `W0112` | a `coverage off` region is never closed, so it excludes the rest of the file |
| `W0113` | a `coverage on` without a preceding `coverage off` |
| `W0114` | what a rule excludes did run: one warning per rule, with the number of items and the first one (`--exclusions` lists them) |
| `W0115` | a signal was left out of toggle coverage because of `--cov-toggle-max` |
| `W0120` | something looks like a state machine but is not one `--cov-kind=fsm` measures |
| `W0121` | merged databases embed different text for one source file; the HTML report shows the last |

**Tiers and threads.** Counts are the same in every tier and at any `--threads`.

## See also

- [Using yavs from VUnit](vunit.rst): coverage in a run script.
- [Continuous integration](ci.rst): a coverage gate in a CI job.
- [Error codes](error-codes.rst): every diagnostic code.
