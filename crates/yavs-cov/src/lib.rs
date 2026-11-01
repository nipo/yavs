//! Code coverage (docs/plan.md rule 10, M9): statement and branch counts of a run, keyed by
//! hierarchical path and a hash of the source text, in a database that runs merge into, with a
//! text summary and a Cobertura XML report.
//!
//! Lowering emits the counters as ordinary IR (`Inst::Cover`, `LowerOptions::coverage`), so
//! every tier counts them; the runtime holds one row of counters per region
//! (`Elaborated::cov_rows`). [`collect`] turns the rows into a [`Database`]:
//!
//! - an item of an architecture or entity is counted per instance, under the instance's path
//!   (`:top:u1`), summed over its blocks and generate iterations;
//! - an item of a package (subprograms) is counted once, under `library.package`, summed over
//!   every caller.
//!
//! An item's identity ([`Key`]) is that path, its kind and branch arm, a hash of its
//! statement's source text (whitespace runs count as one space) and, for statements with the
//! same text in one unit, their order. Byte offsets never enter it, so an edit elsewhere in
//! the file keeps the databases of older runs mergeable.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use yavs_diag::Diagnostic;
use yavs_elab::{Case, Elaborated, ScopeId, ScopeKind};
use yavs_sem::Db;
use yavs_sem::loader::UnitKind;

/// What an item counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Statement,
    /// Arm `n` of an `if`, `case`, conditional or selected assignment, or the body of a loop
    /// (arm 0). An `if` without `else` and a conditional assignment without a final `else`
    /// have one more arm: no condition held.
    Branch(u32),
    /// A `bit`/`std_ulogic` scalar of a signal or port went 0 -> 1 (`true`) or 1 -> 0.
    Toggle(bool),
    /// Row `n` of a condition: input `n / 2` at value `n % 2` (focused expression coverage).
    Condition(u32),
    /// The same for a logic expression on the right side of an assignment.
    Expression(u32),
    /// One combination of the inputs of a condition or logic expression: two bits per input of
    /// `Key::sig` (0 not evaluated, 1 false, 2 true).
    Combination(u32),
    /// A state of a state machine reached (`false`) or a transition taken (`true`): `Key::sig` is
    /// `SIGNAL:STATE` or `SIGNAL:FROM->TO`; hash and occ are 0.
    Fsm(bool),
}

impl Kind {
    /// The word `--kind` and the reports use.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Statement => "statement",
            Kind::Branch(_) => "branch",
            Kind::Toggle(_) => "toggle",
            Kind::Condition(_) => "condition",
            Kind::Expression(_) => "expression",
            Kind::Combination(_) => "combination",
            Kind::Fsm(_) => "fsm",
        }
    }

    /// A condition or expression row.
    pub fn is_fec(self) -> bool {
        matches!(self, Kind::Condition(_) | Kind::Expression(_))
    }

    /// The row of a condition or expression: its input and value.
    pub fn row(self) -> Option<(u32, bool)> {
        match self {
            Kind::Condition(r) | Kind::Expression(r) => Some((r / 2, r % 2 == 1)),
            _ => None,
        }
    }
}

/// The identity of an item, the same in every run of the design.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    /// Instance path (`:top:u1`) or package (`work.pkg`), stored form; [`Key::shown`] for texts.
    pub path: String,
    pub kind: Kind,
    /// FNV-1a of the statement's source text.
    pub hash: u64,
    /// Among statements of one unit with the same text: the position in source order.
    pub occ: u32,
    /// Toggles: the scalar's name below `path` (`data(3)`, `r.valid`); hash and occ are 0.
    /// Conditions and expressions: the text of the row's input. Empty for statements and
    /// branches.
    pub sig: String,
}

impl Key {
    /// The path as reports show it: `top.u1`. The database keeps the stored form, which tells an
    /// instance from a package and stays mergeable with older databases.
    pub fn shown(&self) -> String {
        yavs_diag::dotted(&self.path)
    }
}

/// Where an item is and how often it ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub file: String,
    pub line: u32,
    pub count: u64,
    /// First line of the statement, whitespace runs as one space, cut at 60 characters:
    /// display only (reports), never part of the identity. A condition or expression row: the
    /// result the row's input decides (`0`, `1`, or `?` when other inputs take part), a space
    /// and the text of the whole condition.
    pub text: String,
    /// The tests that hit it: indices into [`Database::tests`], sorted.
    pub tests: Vec<u32>,
    /// The in-source `-- yavs coverage exclude` directives that waive it (combinations and
    /// condition and expression rows).
    /// The count stays the raw one.
    pub waivers: Vec<Waiver>,
    /// `library.unit` of what it belongs to: the entity of its instance, or the package. Empty in
    /// a database that does not record it. Not part of the identity.
    pub unit: String,
    /// The label of the process it is in; empty when it is in none or the process has no label.
    pub process: String,
    /// A condition, expression or combination item: the inputs of its decision in source order, in
    /// full and on one line, what a directive names. Empty elsewhere, and in a database older than
    /// version 11.
    pub operands: Vec<String>,
    /// What `--detailed` says about it (none in a database older than version 12).
    pub detail: Detail,
}

/// Where an item is and what it depends on, for `yavs cov report --detailed`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detail {
    /// The code to underline: its line and column (1-based; 0 unknown), byte offsets in the
    /// source text, and the hash of those bytes, to tell an edited file.
    pub line: u32,
    pub col: u32,
    pub start: u32,
    pub end: u32,
    pub digest: u64,
    /// The branch arm around it, same path: statement hash, occurrence, arm.
    pub under: Option<(u64, u32, u32)>,
    /// The statement it belongs to (arms, condition rows), same path: hash, occurrence.
    pub owner: Option<(u64, u32)>,
    /// A condition or expression row: what the other inputs must be for this input to decide
    /// the result; empty when none matters.
    pub needs: String,
    /// A condition or expression row or a combination: an in-source directive before the
    /// statement can waive it (a conditional or selected assignment is ambiguous, so a directive
    /// there is an error).
    pub directive: bool,
    /// A toggle: the level the scalar ended at, `0`, `1`, `-` (neither), `?` (differs between
    /// merged runs), or a space when not recorded.
    pub level: char,
}

/// An in-source exclusion of a requirement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Waiver {
    /// The directive as written after `exclude`, without the reason.
    pub rule: String,
    pub reason: String,
    /// Where the directive is.
    pub file: String,
    pub line: u32,
}

pub mod detail;
pub mod exclude;
pub mod exclusions;
mod export;
pub mod fsm;
pub mod html;
pub mod toggle;

/// An unclosed `-- coverage off` or a `-- coverage on` without one.
pub const PRAGMA_UNCLOSED: &str = "W0112";
pub const PRAGMA_UNOPENED: &str = "W0113";

/// What `yavs cov report` adds to its text report.
#[derive(Clone, Copy, Debug, Default)]
pub struct SummaryOptions<'a> {
    /// Only the paths under this percentage (in tenths, rounded down as everywhere), worst first.
    pub below: Option<u64>,
    /// Also every item of each of those paths, and entries for what never ran.
    pub detailed: bool,
    /// The excluded items and their reasons, shown as `excluded` in the tables of `detailed`.
    pub excluded: Option<&'a [(Key, Entry, String)]>,
    /// The source text the entries of `detailed` show.
    pub sources: Option<&'a detail::Sources>,
    pub color: bool,
}

/// Rows in aligned columns, plain ASCII, a rule under the first; the columns in `right` align
/// right. Each line starts with `indent`.
pub(crate) fn table(rows: &[Vec<String>], right: &[usize], indent: &str) -> String {
    // A newline in a cell of the last column: what follows goes on lines under the row.
    let first = |cell: &'_ String| cell.split('\n').next().unwrap_or("").chars().count();
    let widths: Vec<usize> = (0..rows[0].len())
        .map(|c| rows.iter().map(|r| first(&r[c])).max().unwrap_or(0))
        .collect();
    let mut s = String::new();
    for (n, row) in rows.iter().enumerate() {
        let mut under = "";
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .enumerate()
            .map(|(c, (cell, w))| {
                let (cell, rest) = cell.split_once('\n').unwrap_or((cell, ""));
                if !rest.is_empty() {
                    under = rest;
                }
                if right.contains(&c) {
                    format!("{cell:>w$}")
                } else {
                    format!("{cell:w$}")
                }
            })
            .collect();
        let _ = writeln!(s, "{indent}{}", cells.join("  ").trim_end());
        for l in under.lines() {
            let _ = writeln!(s, "{indent}    {l}");
        }
        if n == 0 {
            let rule: Vec<String> = widths
                .iter()
                .filter(|w| **w > 0)
                .map(|w| "-".repeat(*w))
                .collect();
            let _ = writeln!(s, "{indent}{}", rule.join("  "));
        }
    }
    s
}

/// [`table`], indented as the focused tables are.
pub(crate) fn ruled(rows: &[Vec<String>], right: &[usize]) -> String {
    table(rows, right, "  ")
}

/// The coverage of one run, or of several merged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Database {
    pub items: BTreeMap<Key, Entry>,
    /// The names of the tests the runs were (`yavs run --cov-test`), in the order seen.
    pub tests: Vec<String>,
    /// A database read from a file older than version 6 is in it: such files do not record
    /// in-source exclusions, so there is nothing to say about them.
    pub legacy: bool,
    /// Some of its items do not say their unit and process: a database older than version 7 is in
    /// it, so `library`, `entity` and `process` exclusions cannot be applied.
    pub no_units: bool,
    /// The text of source files, by the path in [`Entry::file`], when the run embedded them
    /// (`--cov-embed-sources`).
    pub sources: BTreeMap<String, String>,
    /// What looks like a state machine and is not measured: file, line and what it says, for the
    /// reports (`--cov-kind=fsm`). Display only.
    pub unmeasured: BTreeSet<(String, u32, String)>,
}

const MAGIC: &str = "yavs-coverage 12";
/// Version 11 has no embedded sources and no `at` lines.
const V11: &str = "yavs-coverage 11";
/// Older versions [`Database::parse`] still reads: 2 has no toggles, 2 and 3 no tests, 4 no
/// conditions, 5 no combinations and no in-source exclusions, 6 no units and process labels, 7 no
/// operand texts of its waivers, 8 no waivers and input counts of condition and expression rows, 9 no state machines.
const OLD: &[&str] = &["yavs-coverage 2", "yavs-coverage 3"];
const V4: &str = "yavs-coverage 4";
const V5: &str = "yavs-coverage 5";
const V6: &str = "yavs-coverage 6";
const V7: &str = "yavs-coverage 7";
const V8: &str = "yavs-coverage 8";
const V9: &str = "yavs-coverage 9";
const V10: &str = "yavs-coverage 10";

use yavs_diag::operand::uncommented;

/// The source text a condition tree of `nodes` needs of the inputs other than `input` for that
/// input to decide the result; empty when no other input matters.
fn needs(nodes: &[yavs_ir::CovNode], input: u8, names: &[Option<String>]) -> Option<String> {
    use yavs_ir::CovNode as N;
    enum T {
        In(u8),
        Not(Box<T>),
        Bin(N, Box<T>, Box<T>),
    }
    let mut stack = Vec::new();
    for &n in nodes {
        let t = match n {
            N::Input(i) => T::In(i),
            N::Not => T::Not(Box::new(stack.pop()?)),
            _ => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                T::Bin(n, Box::new(a), Box::new(b))
            }
        };
        stack.push(t);
    }
    fn has(t: &T, i: u8) -> bool {
        match t {
            T::In(x) => *x == i,
            T::Not(a) => has(a, i),
            T::Bin(_, a, b) => has(a, i) || has(b, i),
        }
    }
    /// What makes `t` have value `v`, in the words of its inputs.
    fn want(t: &T, v: bool, names: &[Option<String>]) -> String {
        let both = |a: &T, b: &T, x: bool, word: &str| {
            let (a, b) = (want(a, x, names), want(b, x, names));
            format!("{a} {word} {b}")
        };
        match t {
            T::In(i) => match names.get(*i as usize).and_then(|n| n.as_ref()) {
                Some(n) => format!("{n} is {}", u8::from(v)),
                None => format!("a constant is {}", u8::from(v)),
            },
            T::Not(a) => want(a, !v, names),
            T::Bin(N::And, a, b) if v => both(a, b, true, "and"),
            T::Bin(N::And, a, b) => both(a, b, false, "or"),
            T::Bin(N::Nand, a, b) if v => both(a, b, false, "or"),
            T::Bin(N::Nand, a, b) => both(a, b, true, "and"),
            T::Bin(N::Or, a, b) if v => both(a, b, true, "or"),
            T::Bin(N::Or, a, b) => both(a, b, false, "and"),
            T::Bin(N::Nor, a, b) if v => both(a, b, false, "and"),
            T::Bin(N::Nor, a, b) => both(a, b, true, "or"),
            T::Bin(N::Xor, a, b) | T::Bin(N::Xnor, a, b) => {
                let same = matches!(t, T::Bin(N::Xnor, ..)) == v;
                let (a, b) = (want(a, true, names), want(b, true, names));
                format!(
                    "({a}) {} ({b})",
                    if same { "equals" } else { "differs from" }
                )
            }
            T::Bin(..) => String::new(),
        }
    }
    fn first(t: &T) -> u8 {
        match t {
            T::In(i) => *i,
            T::Not(a) | T::Bin(_, a, _) => first(a),
        }
    }
    fn path(t: &T, i: u8, names: &[Option<String>], out: &mut Vec<(u8, String)>) {
        match t {
            T::In(_) => {}
            T::Not(a) => path(a, i, names, out),
            T::Bin(n, a, b) => {
                let (inner, other) = if has(a, i) { (a, b) } else { (b, a) };
                match n {
                    N::And | N::Nand => out.push((first(other), want(other, true, names))),
                    N::Or | N::Nor => out.push((first(other), want(other, false, names))),
                    // Either value of the other operand lets this input decide.
                    _ => {}
                }
                path(inner, i, names, out);
            }
        }
    }
    let mut out = Vec::new();
    path(stack.last()?, input, names, &mut out);
    // In source order.
    out.sort();
    let words: Vec<String> = out.into_iter().map(|x| x.1).collect();
    Some(words.join(" and "))
}

/// FNV-1a of the words of `text`, comments left out: an edit of a comment keeps the identity.
pub(crate) fn fnv1a(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut first = true;
    for w in text.lines().flat_map(|l| uncommented(l).split_whitespace()) {
        for b in (!first).then_some(b' ').into_iter().chain(w.bytes()) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        first = false;
    }
    h
}

/// One counter of the program: its unit (file and the start of the unit's name) and identity.
struct Item {
    unit: (yavs_diag::FileId, u32),
    kind: Kind,
    hash: u64,
    start: u32,
    file: String,
    line: u32,
    text: String,
    /// Inside a `-- coverage off` region: counted but left out of the database.
    skip: bool,
    /// A condition row: the text of its input; a combination: the texts of all its inputs.
    input: String,
    waivers: Vec<Waiver>,
    detail: Detail,
    /// The arm around it and the statement it belongs to, as item indices.
    under: Option<usize>,
    owner: Option<usize>,
    /// The label of the process around it.
    process: String,
    /// A condition, expression or combination item: the inputs of its decision, on one line each.
    operands: Vec<String>,
}

/// Between the texts of the inputs of a combination in `Key::sig`.
pub const INPUT_SEP: &str = "\u{1f}";

/// The inputs of a combination item, as written in the source.
pub fn inputs(k: &Key) -> Vec<&str> {
    k.sig.split(INPUT_SEP).collect()
}

/// A combination as a pattern, one symbol per input: `0` false, `1` true, `-` not evaluated.
pub fn pattern(k: &Key) -> String {
    let Kind::Combination(s) = k.kind else {
        return String::new();
    };
    (0..inputs(k).len())
        .map(|i| match s >> (2 * i) & 3 {
            1 => '0',
            2 => '1',
            _ => '-',
        })
        .collect()
}

/// A combination in words: `valid is false, ready is not evaluated`.
pub fn combination_words(k: &Key) -> String {
    let Kind::Combination(s) = k.kind else {
        return String::new();
    };
    inputs(k)
        .iter()
        .enumerate()
        .map(|(i, x)| match s >> (2 * i) & 3 {
            1 => format!("{x} is false"),
            2 => format!("{x} is true"),
            _ => format!("{x} is not evaluated"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// First line of a statement's text, for display.
fn display(text: &str) -> String {
    let l = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let s = l.split_whitespace().collect::<Vec<_>>().join(" ");
    match s.char_indices().nth(60) {
        Some((i, _)) => format!("{}...", &s[..i]),
        None => s,
    }
}

/// A branch arm as a report names it: `if rst = '1'`, `elsif en`, `else`, `when IDLE`,
/// `(no arm taken)`. `source` is the text of a span of the file.
fn arm_name(
    arm: yavs_ir::CovArm,
    k: u32,
    in_if: bool,
    source: &dyn Fn(yavs_diag::Span) -> String,
) -> String {
    use yavs_ir::CovArm::{Body, Choice, Cond, Else, Implicit, Others};
    let flat = |sp| {
        let s = source(sp).split_whitespace().collect::<Vec<_>>().join(" ");
        match s.char_indices().nth(60) {
            Some((i, _)) => format!("{}...", &s[..i]),
            None => s,
        }
    };
    match arm {
        Cond(sp) if in_if => format!("{} {}", if k == 0 { "if" } else { "elsif" }, flat(sp)),
        Cond(sp) | Choice(Some(sp)) => format!("when {}", flat(sp)),
        Choice(None) => "when (choices)".into(),
        Others => "when others".into(),
        Else => "else".into(),
        Implicit => "(no arm taken)".into(),
        Body => "loop body".into(),
    }
}

/// The 1-based line ranges of a source text that `-- coverage off`/`on` pragmas (alias
/// `-- yavs coverage ...`) leave out; `off-line` is its own line. With `translate_off` the
/// synthesis `translate_off`/`translate_on` pragmas count as `off`/`on`. ponytail: a `--`
/// inside a string literal is read as a comment start.
fn regions(
    path: &str,
    text: &str,
    translate_off: bool,
    warns: &mut Vec<Diagnostic>,
) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut open: Option<u32> = None;
    for (i, l) in text.lines().enumerate() {
        let n = i as u32 + 1;
        let Some(at) = l.find("--") else { continue };
        let c = l[at + 2..].trim().to_ascii_lowercase();
        let c = c.strip_prefix("yavs ").unwrap_or(&c).trim_start();
        let word = |s: &str| {
            s.split(|ch: char| ch == ':' || ch.is_whitespace())
                .next()
                .unwrap_or("")
                .to_string()
        };
        let (off, on) = match c.strip_prefix("coverage") {
            Some(r) if r.starts_with(char::is_whitespace) => match word(r.trim_start()).as_str() {
                "off-line" => {
                    out.push((n, n));
                    continue;
                }
                "off" => (true, false),
                "on" => (false, true),
                _ => continue,
            },
            _ if translate_off => {
                let w: Vec<&str> = c.split_whitespace().collect();
                match w[..] {
                    [_, "translate_off", ..]
                        if matches!(w[0], "synthesis" | "pragma" | "synopsys") =>
                    {
                        (true, false)
                    }
                    [_, "translate_on", ..]
                        if matches!(w[0], "synthesis" | "pragma" | "synopsys") =>
                    {
                        (false, true)
                    }
                    _ => continue,
                }
            }
            _ => continue,
        };
        if off && open.is_none() {
            open = Some(n);
        } else if on {
            match open.take() {
                Some(o) => out.push((o, n)),
                None => warns.push(Diagnostic::warning(
                    PRAGMA_UNOPENED,
                    format!("{path}:{n}: coverage `on` without a matching `off`"),
                )),
            }
        }
    }
    if let Some(o) = open {
        out.push((o, u32::MAX));
        warns.push(Diagnostic::warning(
            PRAGMA_UNCLOSED,
            format!("{path}:{o}: coverage `off` is never closed; it excludes the rest of the file"),
        ));
    }
    out
}

/// The database of a run: `counts` is `Runtime::coverage` of the design `e` built after
/// `Elaborated::lower_with` with coverage on. Items inside `-- coverage off` regions are left
/// out; the warnings are about unbalanced pragmas.
pub fn collect(
    db: &Db,
    e: &Elaborated,
    counts: &[u64],
    translate_off: bool,
) -> (Database, Vec<Diagnostic>) {
    let mut out = Database::default();
    let mut warns = Vec::new();
    let Some(ir) = &e.ir else {
        return (out, warns);
    };
    let prog = &ir.program;
    let n = prog.cov.len();
    if n == 0 {
        return (out, warns);
    }
    let mut regs: HashMap<yavs_diag::FileId, Vec<(u32, u32)>> = HashMap::new();
    // Units do not nest: a statement belongs to the unit whose name is the last one before it
    // in its file.
    let mut units: HashMap<yavs_diag::FileId, Vec<(u32, yavs_sem::UnitId)>> = HashMap::new();
    for (id, u) in db.units.iter() {
        units.entry(u.file).or_default().push((u.span.start, id));
    }
    for v in units.values_mut() {
        v.sort();
    }
    // The labelled processes of each file: (start, end, label), sorted.
    let mut labelled: HashMap<yavs_diag::FileId, Vec<(u32, u32, String)>> = HashMap::new();
    for p in &e.procs {
        let s = &db.stmts[p.stmt];
        if let Some(l) = &s.label {
            let span = (s.span.start, s.span.end, l.text.to_string());
            labelled.entry(s.span.file).or_default().push(span);
        }
    }
    for v in labelled.values_mut() {
        v.sort();
        v.dedup();
    }
    let process_at = |span: yavs_diag::Span| {
        let v = labelled.get(&span.file)?;
        let (_, end, label) = v[..v.partition_point(|x| x.0 <= span.start)].last()?;
        (span.start < *end).then(|| label.clone())
    };
    let mut items = Vec::with_capacity(n);
    let mut owner = HashMap::new();
    // Per stmt, the item of its statement counter.
    let mut stmt_item: HashMap<yavs_ir::StmtId, usize> = HashMap::new();
    for (i, (_, c)) in prog.cov.iter().enumerate() {
        if matches!(c.kind, yavs_ir::CovKind::Statement) {
            stmt_item.insert(c.stmt, i);
        }
    }
    // Per condition row, what the other inputs must be.
    let row_needs: HashMap<usize, String> = {
        let mut map = HashMap::new();
        let span_text = |sp: yavs_diag::Span| {
            let f = db.sources.get(sp.file);
            f.text()
                .get(f.text_offset(sp.start)..f.text_offset(sp.end))
                .map(display)
        };
        let infos: Vec<&yavs_ir::CovInfo> = prog.cov.iter().map(|x| x.1).collect();
        for c in &prog.cov_conds {
            let Some(first) = c.first else { continue };
            let mut k = first.0 as usize;
            let mut names = vec![None; c.inputs as usize];
            let mut rows = Vec::new();
            for i in 0..c.inputs {
                if c.consts >> i & 1 == 1 {
                    continue;
                }
                names[i as usize] = infos
                    .get(k)
                    .and_then(|x| x.row.as_ref())
                    .and_then(|r| span_text(r.input));
                rows.push((i, k));
                k += 2;
            }
            for (i, at) in rows {
                if let Some(text) = needs(&c.nodes, i, &names) {
                    map.insert(at, text.clone());
                    map.insert(at + 1, text);
                }
            }
        }
        map
    };
    for (index, (_, c)) in prog.cov.iter().enumerate() {
        // A condition row belongs to the condition's text, not to its statement.
        let span = match &c.row {
            Some(r) => r.whole,
            None => prog.stmts.get(c.stmt).map(|s| s.span).unwrap_or_default(),
        };
        let unit = units
            .get(&span.file)
            .and_then(|v| v[..v.partition_point(|&(s, _)| s <= span.start)].last())
            .map(|&(s, id)| {
                owner.insert((span.file, s), id);
                (span.file, s)
            })
            .unwrap_or((span.file, 0));
        let f = db.sources.get(span.file);
        let text = f
            .text()
            .get(f.text_offset(span.start)..f.text_offset(span.end))
            .unwrap_or("");
        let line = f.line_col(span.start).0 as u32;
        let rg = regs
            .entry(span.file)
            .or_insert_with(|| regions(&f.path, f.text(), translate_off, &mut warns));
        let at = |s: yavs_diag::Span| {
            f.text()
                .get(f.text_offset(s.start)..f.text_offset(s.end))
                .unwrap_or("")
        };
        // A combination: its decision's inputs, and the directives that waive its state.
        let comb = c
            .cond
            .and_then(|i| prog.cov_conds.get(i as usize)?.comb.as_ref());
        let state = match c.kind {
            yavs_ir::CovKind::Combination(s) => s,
            _ => 0,
        };
        // The directives of a combination, or of a condition or expression row, and the inputs of
        // its decision.
        let directives = comb
            .map(|k| (&k.waivers, &k.inputs))
            .or(c.row.as_ref().map(|r| (&r.waivers, &r.inputs)));
        let operands = directives.map_or(Vec::new(), |(_, spans)| {
            let text = |s: &yavs_diag::Span| yavs_diag::operand::flat(at(*s));
            spans.iter().map(text).collect()
        });
        let waivers = directives
            .map(|(all, _)| {
                all.iter()
                    .filter(|w| state & w.care == w.want)
                    .map(|w| Waiver {
                        rule: w.rule.clone(),
                        reason: w.reason.replace('\t', " "),
                        file: db.sources.get(w.at.file).path.clone(),
                        line: db.sources.get(w.at.file).line_col(w.at.start).0 as u32,
                    })
                    .collect()
            })
            .unwrap_or_default();
        // What a report underlines: the arm's condition, the row's input, else the item's text.
        let shown = match (c.arm, &c.row) {
            (Some(yavs_ir::CovArm::Cond(sp) | yavs_ir::CovArm::Choice(Some(sp))), _) => sp,
            (_, Some(r)) if comb.is_none() => r.input,
            _ => span,
        };
        let (sline, scol) = f.line_col(shown.start);
        let (ss, se) = (f.text_offset(shown.start), f.text_offset(shown.end));
        let detail = Detail {
            line: sline as u32,
            col: scol as u32,
            start: ss as u32,
            end: se as u32,
            digest: fnv1a(f.text().get(ss..se).unwrap_or("")),
            needs: row_needs.get(&index).cloned().unwrap_or_default(),
            directive: c.row.is_some() && {
                let st = prog.stmts.get(c.stmt);
                let t = st.map_or("", |s| at(s.span)).to_ascii_lowercase();
                let words = |w: &str| {
                    t.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                        .any(|x| x == w)
                };
                let selective = words("when") || words("select");
                st.is_some_and(|s| s.kind == yavs_ir::StmtKind::If) || !selective
            },
            ..Detail::default()
        };
        items.push(Item {
            operands,
            detail,
            under: c.under.map(|u| u.0 as usize),
            // The statement being lowered can be a nested one that came before an `elsif`.
            owner: stmt_item.get(&c.stmt).copied().filter(|&o| {
                o != index
                    && prog
                        .stmts
                        .get(c.stmt)
                        .is_some_and(|st| st.span.start <= span.start && span.end <= st.span.end)
            }),
            process: process_at(span).unwrap_or_default(),
            waivers,
            input: match (comb, &c.row) {
                (Some(k), _) => k
                    .inputs
                    .iter()
                    .map(|s| display(at(*s)))
                    .collect::<Vec<_>>()
                    .join(INPUT_SEP),
                (None, Some(r)) => display(at(r.input)),
                _ => String::new(),
            },
            text: match (&c.row, c.arm, c.kind) {
                (Some(_), ..) if comb.is_some() => display(text),
                (Some(r), ..) => format!("{} {}", r.result, display(text)),
                (None, Some(a), yavs_ir::CovKind::Branch(k)) => {
                    let in_if = prog
                        .stmts
                        .get(c.stmt)
                        .is_some_and(|s| s.kind == yavs_ir::StmtKind::If);
                    arm_name(a, k, in_if, &|sp| at(sp).to_string())
                }
                _ => display(text),
            },
            skip: rg.iter().any(|&(a, b)| a <= line && line <= b),
            unit,
            kind: match c.kind {
                yavs_ir::CovKind::Statement => Kind::Statement,
                yavs_ir::CovKind::Branch(a) => Kind::Branch(a),
                yavs_ir::CovKind::Condition(r) => Kind::Condition(r),
                yavs_ir::CovKind::Expression(r) => Kind::Expression(r),
                yavs_ir::CovKind::Combination(s) => Kind::Combination(s),
            },
            // The rows of a wait or an assert are not asked for together with those of an `if` of
            // the same text: a hash of their own keeps the `if`'s occurrence whatever was asked.
            hash: match (&c.row, prog.stmts.get(c.stmt).map(|s| s.kind)) {
                (Some(_), Some(yavs_ir::StmtKind::Wait)) => fnv1a(&format!("wait until {text}")),
                (Some(_), Some(yavs_ir::StmtKind::Assert)) => fnv1a(&format!("assert {text}")),
                _ => fnv1a(text),
            },
            start: span.start,
            file: f.path.clone(),
            line,
        });
    }
    // Statements with the same text in one unit, numbered in source order (the same
    // statement lowered twice, in two package instances say, is one item).
    let mut starts: BTreeMap<(_, Kind, u64), Vec<u32>> = BTreeMap::new();
    for it in &items {
        starts
            .entry((it.unit, it.kind, it.hash))
            .or_default()
            .push(it.start);
    }
    for v in starts.values_mut() {
        v.sort();
        v.dedup();
    }
    let occ: Vec<u32> = items
        .iter()
        .map(|it| {
            let v = &starts[&(it.unit, it.kind, it.hash)];
            v.partition_point(|&s| s < it.start) as u32
        })
        .collect();
    let design_entity = |unit: (yavs_diag::FileId, u32)| {
        owner.get(&unit).is_some_and(|&id| {
            matches!(db.units[id].kind, UnitKind::Entity | UnitKind::Architecture)
        })
    };
    let unit_path = |unit: (yavs_diag::FileId, u32)| match owner.get(&unit) {
        Some(&id) => {
            let u = &db.units[id];
            format!("{}.{}", db.libs[u.lib].name.text, u.name.text).to_ascii_lowercase()
        }
        None => String::from("?"),
    };
    let loc = |id: yavs_sem::UnitId| (db.units[id].file, db.units[id].span.start);
    let unit_name = |id: yavs_sem::UnitId| {
        let u = &db.units[id];
        format!("{}.{}", db.libs[u.lib].name.text, u.name.text).to_ascii_lowercase()
    };
    // An instance: its path, the units whose items it counts, and its entity.
    let instance = |s: ScopeId| match e.hier.scope(s).kind {
        ScopeKind::Instance { entity, arch } => {
            let p = e.hier.path_name(s, Case::Lrm);
            let p = p.strip_suffix(':').filter(|p| !p.is_empty()).unwrap_or(&p);
            Some((p.to_string(), [loc(entity), loc(arch)], unit_name(entity)))
        }
        _ => None,
    };
    let key = |path: String, i: usize| Key {
        path,
        kind: items[i].kind,
        hash: items[i].hash,
        occ: occ[i],
        sig: items[i].input.clone(),
    };
    let mut add = |k: Key, i: usize, c: u64, unit: &str| {
        if items[i].skip {
            return;
        }
        let e = out.items.entry(k).or_insert_with(|| Entry {
            unit: unit.to_string(),
            process: items[i].process.clone(),
            operands: items[i].operands.clone(),
            file: items[i].file.clone(),
            line: items[i].line,
            count: 0,
            text: items[i].text.clone(),
            tests: Vec::new(),
            waivers: items[i].waivers.clone(),
            detail: {
                let mut d = items[i].detail.clone();
                d.under = items[i].under.and_then(|u| match items[u].kind {
                    Kind::Branch(arm) => Some((items[u].hash, occ[u], arm)),
                    _ => None,
                });
                d.owner = items[i].owner.map(|o| (items[o].hash, occ[o]));
                d
            },
        });
        e.count = e.count.saturating_add(c);
    };
    // Every item, with no count yet: per instance for design entities, else per unit.
    for s in 0..e.hier.scopes.len() {
        if let Some((path, units, entity)) = instance(ScopeId(s as u32)) {
            for (i, it) in items.iter().enumerate() {
                if units.contains(&it.unit) {
                    add(key(path.clone(), i), i, 0, &entity);
                }
            }
        }
    }
    for (i, it) in items.iter().enumerate() {
        if !design_entity(it.unit) {
            let path = unit_path(it.unit);
            add(key(path.clone(), i), i, 0, &path);
        }
    }
    // Row 0 has no instance; row r + 1 is region r's instance data.
    for (row, cs) in counts.chunks(n).enumerate() {
        let inst = row
            .checked_sub(1)
            .and_then(|r| instance(e.instance_of(ScopeId(r as u32))));
        for (i, &c) in cs.iter().enumerate().filter(|(_, c)| **c > 0) {
            let unit = items[i].unit;
            match &inst {
                Some((p, units, entity)) if units.contains(&unit) => {
                    add(key(p.clone(), i), i, c, entity);
                }
                // A package subprogram (or one passed as a generic subprogram): counted under
                // the unit declaring it.
                _ => {
                    let path = unit_path(unit);
                    add(key(path.clone(), i), i, c, &path);
                }
            }
        }
    }
    (out, warns)
}

/// Between the operands in a field of a database line.
const OPERAND_SEP: &str = "\u{1d}";

/// The `at` line of an item, after its own line: line, column, start, end, digest, arm around
/// it, statement, what the other inputs must be, level. Nothing for an item without detail.
fn push_at(s: &mut String, d: &Detail) {
    if d.line == 0 {
        return;
    }
    let _ = write!(
        s,
        "at\t{}\t{}\t{}\t{}\t{:x}\t",
        d.line, d.col, d.start, d.end, d.digest
    );
    match d.under {
        Some((h, o, a)) => {
            let _ = write!(s, "{h:x}.{o}.{a}");
        }
        None => s.push('-'),
    }
    s.push('\t');
    match d.owner {
        Some((h, o)) => {
            let _ = write!(s, "{h:x}.{o}");
        }
        None => s.push('-'),
    }
    let directive = if d.directive { "d" } else { "-" };
    let _ = writeln!(s, "\t{}\t{}\t{directive}", one_line(&d.needs), d.level);
}

/// The fields of an `at` line.
fn parse_at(t: &str) -> Option<Detail> {
    let f: Vec<&str> = t.split('\t').collect();
    let [
        line,
        col,
        start,
        end,
        digest,
        under,
        owner,
        needs,
        level,
        directive,
    ] = f[..]
    else {
        return None;
    };
    let under = match under {
        "-" => None,
        u => {
            let mut p = u.split('.');
            Some((
                u64::from_str_radix(p.next()?, 16).ok()?,
                p.next()?.parse().ok()?,
                p.next()?.parse().ok()?,
            ))
        }
    };
    let owner = match owner {
        "-" => None,
        u => {
            let (h, o) = u.split_once('.')?;
            Some((u64::from_str_radix(h, 16).ok()?, o.parse().ok()?))
        }
    };
    Some(Detail {
        line: line.parse().ok()?,
        col: col.parse().ok()?,
        start: start.parse().ok()?,
        end: end.parse().ok()?,
        digest: u64::from_str_radix(digest, 16).ok()?,
        under,
        owner,
        needs: unescaped(needs),
        level: level.chars().next().unwrap_or(' '),
        directive: directive == "d",
    })
}

/// A source text as one field of a database line.
fn one_line(text: &str) -> String {
    let mut o = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o
}

/// The inverse of [`one_line`].
fn unescaped(field: &str) -> String {
    let mut o = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => o.push('\n'),
            Some('r') => o.push('\r'),
            Some('t') => o.push('\t'),
            Some(x) => o.push(x),
            None => {}
        }
    }
    o
}

/// The waivers of an item as one field of a database line; `-` for none.
fn waiver_field(waivers: &[Waiver]) -> String {
    let w: Vec<String> = waivers
        .iter()
        .map(|w| {
            format!(
                "{}\u{1f}{}\u{1f}{}\u{1f}{}",
                w.rule, w.reason, w.file, w.line
            )
        })
        .collect();
    if w.is_empty() {
        "-".to_string()
    } else {
        w.join("\u{1e}")
    }
}

/// A field that may be empty, as a database line writes it.
fn dash(s: &str) -> &str {
    if s.is_empty() { "-" } else { s }
}

/// The field [`dash`] wrote.
fn undash(s: &str) -> &str {
    if s == "-" { "" } else { s }
}

/// `v` in decimal.
fn push_num(s: &mut String, mut v: u64) {
    let mut b = [0u8; 20];
    let mut i = b.len();
    loop {
        i -= 1;
        b[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    s.extend(b[i..].iter().map(|&c| c as char));
}

/// The ids of `tests` (renumbered by `id`) in sorted order, comma-separated; `-` for none.
fn push_tests(s: &mut String, tests: &[u32], id: &[usize]) {
    match tests {
        [] => s.push('-'),
        [one] => push_num(s, id[*one as usize] as u64),
        _ => {
            let mut t: Vec<usize> = tests.iter().map(|&i| id[i as usize]).collect();
            t.sort_unstable();
            for (n, v) in t.iter().enumerate() {
                if n > 0 {
                    s.push(',');
                }
                push_num(s, *v as u64);
            }
        }
    }
}

impl Database {
    /// The text form: a header line, the tests section (`test`, id, name; ids number the
    /// names in sorted order), then one tab-separated line per item, sorted by key.
    /// Statements and branches: kind, path, hash, occurrence, count, tests, line, file, text;
    /// conditions (`c<row>`) and expressions (`e<row>`) the same with the row's input before
    /// the text;
    /// toggles (`t1` rise, `t0` fall): kind, path, scalar name, count, tests, line, file.
    /// state machines (`f0` a state, `f1` a transition): the same fields as a toggle, `SIGNAL:ITEM`
    /// for the scalar name; `unmeasured`, line, file, text: what was not measured.
    /// `tests` is the comma-separated ids of the tests that hit the item, `-` for none. Every line
    /// ends in the unit (`library.entity` or `library.package`), and all but toggles then in the
    /// process label; `-` for none.
    pub fn write(&self) -> String {
        let mut s = format!("{MAGIC}\n");
        let mut order: Vec<usize> = (0..self.tests.len()).collect();
        order.sort_by(|&a, &b| self.tests[a].cmp(&self.tests[b]));
        let mut id = vec![0; self.tests.len()];
        for (new, &old) in order.iter().enumerate() {
            id[old] = new;
            let _ = writeln!(s, "test\t{new}\t{}", self.tests[old]);
        }
        for (path, text) in &self.sources {
            let _ = writeln!(s, "source\t{path}\t{}", one_line(text));
        }
        for (file, line, text) in &self.unmeasured {
            let _ = writeln!(s, "unmeasured\t{line}\t{file}\t{text}");
        }
        // About 150 bytes a line: one allocation for the database of a large design.
        s.reserve(self.items.len() * 150);
        for (k, e) in &self.items {
            if let Kind::Toggle(r) | Kind::Fsm(r) = k.kind {
                // The common line, written without the formatting machinery.
                s.push(if matches!(k.kind, Kind::Fsm(_)) {
                    'f'
                } else {
                    't'
                });
                s.push(if r { '1' } else { '0' });
                s.push('\t');
                s.push_str(&k.path);
                s.push('\t');
                s.push_str(&k.sig);
                s.push('\t');
                push_num(&mut s, e.count);
                s.push('\t');
                push_tests(&mut s, &e.tests, &id);
                s.push('\t');
                push_num(&mut s, u64::from(e.line));
                s.push('\t');
                s.push_str(&e.file);
                s.push('\t');
                s.push_str(dash(&e.unit));
                s.push('\n');
                push_at(&mut s, &e.detail);
                continue;
            }
            let mut t = String::new();
            push_tests(&mut t, &e.tests, &id);
            let kind = match k.kind {
                Kind::Statement => "s".to_string(),
                Kind::Branch(a) => format!("b{a}"),
                Kind::Condition(r) | Kind::Expression(r) => {
                    let c = if matches!(k.kind, Kind::Condition(_)) {
                        'c'
                    } else {
                        'e'
                    };
                    let _ = writeln!(
                        s,
                        "{c}{r}\t{}\t{:016x}\t{}\t{}\t{t}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        k.path,
                        k.hash,
                        k.occ,
                        e.count,
                        e.line,
                        e.file,
                        k.sig,
                        e.text,
                        waiver_field(&e.waivers),
                        dash(&e.operands.join(OPERAND_SEP)),
                        dash(&e.unit),
                        dash(&e.process)
                    );
                    push_at(&mut s, &e.detail);
                    continue;
                }
                Kind::Combination(st) => {
                    let _ = writeln!(
                        s,
                        "m{st}\t{}\t{:016x}\t{}\t{}\t{t}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        k.path,
                        k.hash,
                        k.occ,
                        e.count,
                        e.line,
                        e.file,
                        k.sig,
                        e.text,
                        waiver_field(&e.waivers),
                        dash(&e.operands.join(OPERAND_SEP)),
                        dash(&e.unit),
                        dash(&e.process)
                    );
                    push_at(&mut s, &e.detail);
                    continue;
                }
                Kind::Toggle(_) | Kind::Fsm(_) => continue,
            };
            let _ = writeln!(
                s,
                "{kind}\t{}\t{:016x}\t{}\t{}\t{t}\t{}\t{}\t{}\t{}\t{}",
                k.path,
                k.hash,
                k.occ,
                e.count,
                e.line,
                e.file,
                e.text,
                dash(&e.unit),
                dash(&e.process)
            );
            push_at(&mut s, &e.detail);
        }
        s
    }

    /// Read the text form of [`Database::write`] (or of an older version: version 2 has no
    /// toggles, versions 2 and 3 no tests).
    pub fn parse(text: &str) -> Result<Database, String> {
        let mut lines = text.lines();
        let head = lines.next();
        let old = head.is_some_and(|h| OLD.contains(&h));
        let known = [MAGIC, V11, V10, V9, V8, V7, V6, V5, V4]
            .iter()
            .any(|v| head == Some(*v));
        if !known && !old {
            let version = head
                .and_then(|h| h.strip_prefix("yavs-coverage "))
                .filter(|v| v.parse::<u32>().is_ok());
            return Err(match version {
                Some(v) => format!(
                    "it has format version {v} and this yavs ({}) reads versions 2 to {}; re-run \
                     the simulation with this yavs, or report with the yavs that wrote it",
                    env!("CARGO_PKG_VERSION"),
                    MAGIC.trim_start_matches("yavs-coverage "),
                ),
                None => format!("not a yavs coverage database (no `{MAGIC}` line)"),
            });
        }
        let with_units = [MAGIC, V11, V10, V9, V8, V7]
            .iter()
            .any(|v| head == Some(*v));
        // A row line of these has a count of inputs where this version has the operands.
        let row_counts = head == Some(V10) || head == Some(V9);
        let with_operands = head == Some(MAGIC) || head == Some(V11);
        let row_waivers = with_operands || row_counts;
        let mut db = Database {
            legacy: !with_units && head != Some(V6),
            no_units: !with_units,
            ..Database::default()
        };
        let mut last: Option<Key> = None;
        for (n, l) in lines.enumerate() {
            let bad = || format!("line {}: malformed item `{l}`", n + 2);
            if let Some(t) = l.strip_prefix("at\t") {
                let d = parse_at(t).ok_or_else(bad)?;
                let e = last.as_ref().and_then(|k| db.items.get_mut(k));
                e.ok_or_else(bad)?.detail = d;
                continue;
            }
            if let Some(t) = l.strip_prefix("test\t") {
                let (id, name) = t.split_once('\t').ok_or_else(bad)?;
                if id.parse::<usize>().ok() != Some(db.tests.len()) {
                    return Err(bad());
                }
                db.tests.push(name.to_string());
                continue;
            }
            if let Some(t) = l.strip_prefix("source\t") {
                let (path, text) = t.split_once('\t').ok_or_else(bad)?;
                db.sources.insert(path.to_string(), unescaped(text));
                continue;
            }
            if let Some(t) = l.strip_prefix("unmeasured\t") {
                let p: Vec<&str> = t.splitn(3, '\t').collect();
                let [line, file, text] = p[..] else {
                    return Err(bad());
                };
                db.unmeasured.insert((
                    file.to_string(),
                    line.parse().map_err(|_| bad())?,
                    text.to_string(),
                ));
                continue;
            }
            let mut f: Vec<&str> = l.split('\t').collect();
            let named = l.starts_with('t') || l.starts_with('f');
            // The last fields of this version: the process (not of toggles), then the unit.
            let mut process = String::new();
            let mut unit = String::new();
            if with_units {
                if !named {
                    let p = f.pop().ok_or_else(bad)?;
                    process = undash(p).to_string();
                }
                unit = undash(f.pop().ok_or_else(bad)?).to_string();
                db.no_units |= unit.is_empty();
            }
            // Versions before 4 have no tests field (after the count).
            let at = if named { 4 } else { 5 };
            if old && f.len() >= at {
                f.insert(at, "-");
            }
            let tests = match f.get(at) {
                Some(&"-") => Vec::new(),
                Some(t) => t
                    .split(',')
                    .map(|i| {
                        i.parse()
                            .ok()
                            .filter(|&i: &u32| (i as usize) < db.tests.len())
                    })
                    .collect::<Option<Vec<u32>>>()
                    .ok_or_else(bad)?,
                None => return Err(bad()),
            };
            let (key, entry) = if named {
                let [kind, path, sig, count, _, line, file] = f[..] else {
                    return Err(bad());
                };
                let kind = match kind {
                    "t1" => Kind::Toggle(true),
                    "t0" => Kind::Toggle(false),
                    "f1" => Kind::Fsm(true),
                    "f0" => Kind::Fsm(false),
                    _ => return Err(bad()),
                };
                let key = Key {
                    path: path.to_string(),
                    kind,
                    hash: 0,
                    occ: 0,
                    sig: sig.to_string(),
                };
                let entry = Entry {
                    file: file.to_string(),
                    line: line.parse().map_err(|_| bad())?,
                    count: count.parse().map_err(|_| bad())?,
                    text: String::new(),
                    tests,
                    waivers: Vec::new(),
                    unit,
                    process,
                    operands: Vec::new(),
                    detail: Detail::default(),
                };
                (key, entry)
            } else {
                let fec = l.starts_with('c') || l.starts_with('e');
                let comb = l.starts_with('m');
                let mut waivers = Vec::new();
                let mut operands = Vec::new();
                if (fec || comb) && with_operands {
                    let o = undash(f.pop().ok_or_else(bad)?);
                    operands = o
                        .split(OPERAND_SEP)
                        .filter(|o| !o.is_empty())
                        .map(str::to_string)
                        .collect();
                } else if fec && row_counts {
                    f.pop().ok_or_else(bad)?;
                }
                if comb || (fec && row_waivers) {
                    let Some(w) = f.pop() else {
                        return Err(bad());
                    };
                    for x in w.split('\u{1e}').filter(|x| *x != "-") {
                        let p: Vec<&str> = x.split('\u{1f}').collect();
                        // Versions 8 to 10 have the operands of the directive after these.
                        let (rule, reason, file, line) = match p[..] {
                            [r, why, f, n] | [r, why, f, n, _] => (r, why, f, n),
                            _ => return Err(bad()),
                        };
                        waivers.push(Waiver {
                            rule: rule.into(),
                            reason: reason.into(),
                            file: file.into(),
                            line: line.parse().map_err(|_| bad())?,
                        });
                    }
                }
                let (sig, f) = match (fec || comb, &f[..]) {
                    (true, [a, b, c, d, e, g, h, i, sig, text]) => {
                        (*sig, vec![*a, *b, *c, *d, *e, *g, *h, *i, *text])
                    }
                    (false, _) => ("", f),
                    _ => return Err(bad()),
                };
                let [kind, path, hash, occ, count, _, line, file, text] = f[..] else {
                    return Err(bad());
                };
                let row = |p: char| {
                    kind.strip_prefix(p)
                        .and_then(|a| a.parse().ok())
                        .ok_or_else(bad)
                };
                let kind = match kind {
                    "s" => Kind::Statement,
                    _ if comb => Kind::Combination(row('m')?),
                    _ if fec && kind.starts_with('c') => Kind::Condition(row('c')?),
                    _ if fec => Kind::Expression(row('e')?),
                    _ => Kind::Branch(row('b')?),
                };
                let key = Key {
                    path: path.to_string(),
                    kind,
                    hash: u64::from_str_radix(hash, 16).map_err(|_| bad())?,
                    occ: occ.parse().map_err(|_| bad())?,
                    sig: sig.to_string(),
                };
                let entry = Entry {
                    file: file.to_string(),
                    line: line.parse().map_err(|_| bad())?,
                    count: count.parse().map_err(|_| bad())?,
                    text: text.to_string(),
                    tests,
                    waivers,
                    unit,
                    process,
                    operands,
                    detail: Detail::default(),
                };
                (key, entry)
            };
            last = Some(key.clone());
            if db.items.insert(key, entry).is_some() {
                return Err(format!("line {}: item listed twice", n + 2));
            }
        }
        Ok(db)
    }

    /// Add the counts of `other` and the tests that hit each item (tests are matched by
    /// name); its items not in `self` are added as they are. Returns a line for each item that
    /// one side waives with an in-source directive and the other does not: such a waiver would
    /// hide a requirement the other run kept (the item is merged without the waiver).
    pub fn merge(&mut self, other: Database) -> Vec<String> {
        let mut conflicts = Vec::new();
        self.legacy |= other.legacy;
        self.sources.extend(other.sources);
        self.no_units |= other.no_units;
        self.unmeasured.extend(other.unmeasured);
        let id: Vec<u32> = other
            .tests
            .iter()
            .map(|t| match self.tests.iter().position(|x| x == t) {
                Some(i) => i as u32,
                None => {
                    self.tests.push(t.clone());
                    self.tests.len() as u32 - 1
                }
            })
            .collect();
        for (k, mut e) in other.items {
            for t in &mut e.tests {
                *t = id[*t as usize];
            }
            match self.items.get_mut(&k) {
                Some(x) => {
                    if x.detail.level != e.detail.level {
                        x.detail.level = '?';
                    }
                    if x.detail.line == 0 {
                        x.detail = e.detail.clone();
                    }
                    if x.waivers.is_empty() != e.waivers.is_empty() {
                        let w = if x.waivers.is_empty() {
                            &e.waivers
                        } else {
                            &x.waivers
                        };
                        let what = match k.kind.row() {
                            Some((_, v)) => format!("{} = {}", k.sig, u8::from(v)),
                            None => pattern(&k),
                        };
                        conflicts.push(format!(
                            "{} {what} of {} ({}:{}) is excluded at {}:{} in one database and required in the other",
                            k.kind.name(),
                            k.shown(),
                            x.file,
                            x.line,
                            w[0].file,
                            w[0].line
                        ));
                        x.waivers.clear();
                    } else {
                        for w in e.waivers {
                            if !x.waivers.contains(&w) {
                                x.waivers.push(w);
                            }
                        }
                        x.waivers.sort();
                    }
                    if x.operands.is_empty() {
                        x.operands = e.operands;
                    }
                    x.count = x.count.saturating_add(e.count);
                    x.tests.extend(e.tests);
                    x.tests.sort_unstable();
                    x.tests.dedup();
                }
                None => {
                    e.tests.sort_unstable();
                    self.items.insert(k, e);
                }
            }
        }
        conflicts
    }

    /// Make this the database of one run of test `name`: every item that ran was hit by it.
    pub fn attribute(&mut self, name: &str) {
        self.tests = vec![name.to_string()];
        for e in self.items.values_mut() {
            e.tests = if e.count > 0 { vec![0] } else { Vec::new() };
        }
    }

    /// `yavs cov rank`: a greedy set cover (the test hitting the most items not yet hit,
    /// until no test adds one; ties go to the first name), then per test its items and the
    /// items no other test hits. Items no test hit are not counted.
    pub fn rank(&self) -> String {
        let n = self.tests.len();
        let mut per: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut hit = 0;
        for (i, e) in self.items.values().enumerate() {
            for &t in &e.tests {
                per[t as usize].push(i);
            }
            hit += usize::from(!e.tests.is_empty());
        }
        let mut names: Vec<usize> = (0..n).collect();
        names.sort_by(|&a, &b| self.tests[a].cmp(&self.tests[b]));
        let mut done = vec![false; self.items.len()];
        let mut so_far = 0;
        let mut s = format!(
            "{} hit {hit} of {}\n\nminimal set (greedy):\n",
            count(n, "test", "tests"),
            count(self.items.len(), "item", "items")
        );
        let w = self.tests.iter().map(String::len).max().unwrap_or(0).max(4);
        let mut picked = vec![false; n];
        loop {
            let best = names
                .iter()
                .map(|&t| (per[t].iter().filter(|&&i| !done[i]).count(), t))
                .filter(|&(c, t)| c > 0 && !picked[t])
                .fold(None, |b: Option<(usize, usize)>, x| match b {
                    Some(b) if b.0 >= x.0 => Some(b),
                    _ => Some(x),
                });
            let Some((c, t)) = best else { break };
            picked[t] = true;
            so_far += c;
            for &i in &per[t] {
                done[i] = true;
            }
            let _ = writeln!(s, "  {:w$}  +{c}  {so_far}/{hit}", self.tests[t]);
        }
        let rest: Vec<&str> = names
            .iter()
            .filter(|&&t| !picked[t])
            .map(|&t| self.tests[t].as_str())
            .collect();
        if !rest.is_empty() {
            let _ = writeln!(s, "\nnot needed: {}", rest.join(", "));
        }
        let _ = writeln!(s, "\n{:w$}  {:>8}  {:>8}", "test", "items", "unique");
        let items: Vec<&Entry> = self.items.values().collect();
        for &t in &names {
            let unique = per[t]
                .iter()
                .filter(|&&i| items[i].tests.len() == 1)
                .count();
            let _ = writeln!(s, "{:w$}  {:>8}  {unique:>8}", self.tests[t], per[t].len());
        }
        s
    }

    /// The report with no options: tables of the kinds and the paths, how much never ran.
    pub fn summary(&self) -> String {
        self.summary_with(&SummaryOptions::default())
    }

    /// The text report. Only the kinds the database holds show. Totals and gates do not depend
    /// on the options.
    pub fn summary_with(&self, o: &SummaryOptions) -> String {
        const KINDS: [&str; 7] = [
            "statement",
            "branch",
            "toggle",
            "condition",
            "expression",
            "combination",
            "fsm",
        ];
        let col = |h: usize, t: usize| format!("{h}/{t} {}", percent(h, t));
        // Per path and kind (the order of `KINDS`): hit, total.
        let mut per: BTreeMap<&str, [(usize, usize); 7]> = BTreeMap::new();
        for (k, e) in &self.items {
            let at = KINDS.iter().position(|x| *x == k.kind.name());
            let c = &mut per.entry(&k.path).or_default()[at.unwrap_or(0)];
            c.1 += 1;
            c.0 += usize::from(e.count > 0);
        }
        let mut t = [(0, 0); 7];
        for c in per.values() {
            for (a, b) in t.iter_mut().zip(c) {
                a.0 += b.0;
                a.1 += b.1;
            }
        }
        let held: Vec<usize> = (0..7).filter(|&i| t[i].1 > 0).collect();
        let excluded_only: Vec<&str> = o
            .excluded
            .unwrap_or(&[])
            .iter()
            .map(|x| x.0.path.as_str())
            .filter(|p| !per.contains_key(p))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if held.is_empty() && !(o.detailed && !excluded_only.is_empty()) {
            return "no coverage items\n".to_string();
        }
        let (hit, all) = self.totals();
        let row = |name: &str, h: usize, a: usize| {
            vec![
                name.to_string(),
                a.to_string(),
                h.to_string(),
                (a - h).to_string(),
                percent(h, a),
            ]
        };
        let mut rows = vec![
            ["Kind", "Items", "Hit", "Missed", "Coverage"]
                .map(String::from)
                .to_vec(),
        ];
        rows.extend(held.iter().map(|&i| row(KINDS[i], t[i].0, t[i].1)));
        rows.push(row("total", hit, all));
        let mut s = table(&rows, &[1, 2, 3, 4], "");
        // The line of a path: its hits and items over every kind.
        let sum = |c: &[(usize, usize); 7]| c.iter().fold((0, 0), |x, y| (x.0 + y.0, x.1 + y.1));
        let mut paths: Vec<_> = per.iter().collect();
        if let Some(limit) = o.below {
            let at = |c: &[(usize, usize); 7]| {
                let (h, a) = sum(c);
                tenths(h, a).unwrap_or(0)
            };
            paths.retain(|(_, c)| at(c) < limit);
            paths.sort_by_key(|(p, c)| (at(c), **p));
        }
        s.push('\n');
        if !paths.is_empty() {
            let mut table_rows = vec![vec!["Instance".to_string(), "Total".to_string()]];
            table_rows[0].extend(held.iter().map(|&i| KINDS[i].to_string()));
            for (p, c) in &paths {
                let (h, a) = sum(c);
                let mut row = vec![yavs_diag::dotted(p), col(h, a)];
                row.extend(held.iter().map(|&i| match c[i] {
                    (_, 0) => "-".to_string(),
                    (h, a) => col(h, a),
                }));
                table_rows.push(row);
            }
            s.push_str(&table(&table_rows, &[], ""));
        }
        if let Some(limit) = o.below {
            let need = format!("{}.{}%", limit / 10, limit % 10);
            let left = per.len() - paths.len();
            if paths.is_empty() {
                let _ = writeln!(s, "no instance or package is below {need}");
            } else if left > 0 {
                let _ = writeln!(s, "{left} of {} at or above {need} left out", per.len());
            }
        }
        if o.detailed {
            let mut order: Vec<&str> = paths.iter().map(|(p, _)| **p).collect();
            if o.below.is_none() {
                order.extend(excluded_only);
            }
            s.push_str(&detail::report(self, &order, o));
        } else {
            match all - hit {
                0 => s.push_str("\nevery item is covered\n"),
                n => {
                    let n = count(n, "item", "items");
                    let _ = writeln!(s, "\n{n} not covered; `--detailed` shows them.");
                }
            }
        }
        if !self.unmeasured.is_empty() {
            s.push_str("\nnot measured as state machines:\n");
            for (file, line, text) in &self.unmeasured {
                let _ = writeln!(s, "  {file}:{line}  {text}");
            }
        }
        s
    }

    /// The items of the kinds named (`Kind::name`) only.
    pub fn only(&self, kinds: &[String]) -> Database {
        Database {
            items: self
                .items
                .iter()
                .filter(|(k, _)| kinds.iter().any(|n| n == k.kind.name()))
                .map(|(k, e)| (k.clone(), e.clone()))
                .collect(),
            no_units: self.no_units,
            tests: self.tests.clone(),
            unmeasured: if kinds.iter().any(|n| n == "fsm") {
                self.unmeasured.clone()
            } else {
                BTreeSet::new()
            },
            legacy: self.legacy,
            sources: self.sources.clone(),
        }
    }

    /// Store the text of the source files the items are in (`--cov-embed-sources`).
    pub fn embed_sources(&mut self, db: &Db) {
        let files: std::collections::BTreeSet<&str> =
            self.items.values().map(|e| e.file.as_str()).collect();
        for i in 0..db.sources.len() {
            let f = db.sources.get(yavs_diag::FileId(i as u32));
            if files.contains(f.path.as_str()) {
                self.sources.insert(f.path.clone(), f.text().to_string());
            }
        }
    }

    /// The items that ran and all items: what `--total` and `--fail-under` count.
    pub fn totals(&self) -> (usize, usize) {
        let hit = self.items.values().filter(|e| e.count > 0).count();
        (hit, self.items.len())
    }

    /// A Cobertura XML report: one class per source file, counts summed over every path. A
    /// line's hits are those of its most executed statement; a line with branches lists how
    /// many of their arms ran. Toggles are not in it (Cobertura has no notion of them). The
    /// timestamp is 0, so equal databases give equal reports.
    pub fn cobertura(&self) -> String {
        #[derive(Default)]
        struct Line {
            hits: u64,
            arms: BTreeMap<(u64, u32, u32), u64>,
        }
        // Counts per source statement (or arm), summed over paths.
        let mut stmts: BTreeMap<(&str, u32, u64, u32), u64> = BTreeMap::new();
        let mut files: BTreeMap<&str, BTreeMap<u32, Line>> = BTreeMap::new();
        for (k, e) in self.items.iter().filter(|(k, _)| {
            !matches!(
                k.kind,
                Kind::Toggle(_) | Kind::Combination(_) | Kind::Fsm(_)
            )
        }) {
            let line = files.entry(&e.file).or_default().entry(e.line).or_default();
            match k.kind {
                Kind::Statement => {
                    let c = stmts.entry((&e.file, e.line, k.hash, k.occ)).or_default();
                    *c = c.saturating_add(e.count);
                }
                Kind::Branch(a) => {
                    let c = line.arms.entry((k.hash, k.occ, a)).or_default();
                    *c = c.saturating_add(e.count);
                }
                // A condition row is a condition of the line, apart from the branch arms
                // (arm numbers from 2^31; an expression row from 2^30 more).
                Kind::Condition(r) | Kind::Expression(r) => {
                    let a = (1 << 31)
                        | r
                        | if matches!(k.kind, Kind::Expression(_)) {
                            1 << 30
                        } else {
                            0
                        };
                    let c = line.arms.entry((k.hash, k.occ, a)).or_default();
                    *c = c.saturating_add(e.count);
                }
                // Combinations are not branches: Cobertura has no notion of them.
                Kind::Toggle(_) | Kind::Combination(_) | Kind::Fsm(_) => {}
            }
        }
        for ((f, l, _, _), c) in &stmts {
            if let Some(line) = files.get_mut(f).and_then(|m| m.get_mut(l)) {
                line.hits = line.hits.max(*c);
            }
        }
        // A line whose statement is not in this report (`--kind`, an exclusion) ran as often as
        // its arms and rows say: 0 would call it missed (#1023).
        let stated: std::collections::BTreeSet<(&str, u32)> =
            stmts.keys().map(|&(f, l, ..)| (f, l)).collect();
        for (f, lines) in &mut files {
            for (l, line) in lines.iter_mut() {
                if !stated.contains(&(*f, *l)) {
                    line.hits = line.arms.values().copied().max().unwrap_or(0);
                }
            }
        }
        let rate = |h: usize, t: usize| if t == 0 { 0.0 } else { h as f64 / t as f64 };
        let count = |lines: &BTreeMap<u32, Line>| {
            let mut c = [0; 4];
            for l in lines.values() {
                c[0] += usize::from(l.hits > 0);
                c[1] += 1;
                c[2] += l.arms.values().filter(|&&n| n > 0).count();
                c[3] += l.arms.len();
            }
            c
        };
        let mut classes = String::new();
        let mut t = [0; 4];
        for (f, lines) in &files {
            let c = count(lines);
            for (a, b) in t.iter_mut().zip(c) {
                *a += b;
            }
            let name = std::path::Path::new(f)
                .file_stem()
                .map_or(f.to_string(), |s| s.to_string_lossy().into_owned());
            let _ = writeln!(
                classes,
                "        <class name=\"{}\" filename=\"{}\" line-rate=\"{}\" branch-rate=\"{}\" \
                 complexity=\"0\">\n          <methods/>\n          <lines>",
                xml(&name),
                xml(f),
                rate(c[0], c[1]),
                rate(c[2], c[3])
            );
            for (n, l) in lines {
                let _ = write!(
                    classes,
                    "            <line number=\"{n}\" hits=\"{}\"",
                    l.hits
                );
                if l.arms.is_empty() {
                    classes.push_str(" branch=\"false\"/>\n");
                } else {
                    let (h, t) = (l.arms.values().filter(|&&n| n > 0).count(), l.arms.len());
                    let _ = writeln!(
                        classes,
                        " branch=\"true\" condition-coverage=\"{}% ({h}/{t})\"/>",
                        100 * h / t
                    );
                }
            }
            classes.push_str("          </lines>\n        </class>\n");
        }
        format!(
            "<?xml version=\"1.0\" ?>\n\
             <!DOCTYPE coverage SYSTEM \"http://cobertura.sourceforge.net/xml/coverage-04.dtd\">\n\
             <coverage line-rate=\"{lr}\" branch-rate=\"{br}\" lines-covered=\"{}\" \
             lines-valid=\"{}\" branches-covered=\"{}\" branches-valid=\"{}\" complexity=\"0\" \
             version=\"yavs {}\" timestamp=\"0\">\n  <sources>\n    <source>.</source>\n  \
             </sources>\n  <packages>\n    <package name=\"design\" line-rate=\"{lr}\" \
             branch-rate=\"{br}\" complexity=\"0\">\n      <classes>\n{classes}      </classes>\n    \
             </package>\n  </packages>\n</coverage>\n",
            t[0],
            t[1],
            t[2],
            t[3],
            env!("CARGO_PKG_VERSION"),
            lr = rate(t[0], t[1]),
            br = rate(t[2], t[3]),
        )
    }
}

/// What a condition or expression row is about: `a is 1 (result 1) in a and b` (input, value,
/// the result that input decides alone, the whole condition).
pub fn fec_text(k: &Key, e: &Entry) -> String {
    let (res, whole) = e.text.split_once(' ').unwrap_or(("?", &e.text));
    let v = k.kind.row().map_or(0, |r| u8::from(r.1));
    let res = if res == "?" {
        String::new()
    } else {
        format!(" (result {res})")
    };
    format!("{} is {v}{res} in {whole}", k.sig)
}

/// What a combination is about: `valid is false, ready is true in valid and ready`.
/// A state or transition of a state machine as reports show it: `state: idle -> busy`.
pub fn fsm_text(k: &Key) -> String {
    let (signal, item) = k.sig.split_once(':').unwrap_or(("", &k.sig));
    match item.split_once("->") {
        Some((from, to)) => format!("{signal}: {from} -> {to}"),
        None => format!("{signal}: {item}"),
    }
}

/// A combination in words, for a report.
pub fn comb_text(k: &Key, e: &Entry) -> String {
    format!("{} in {}", combination_words(k), e.text)
}

/// `rise (0->1)` or `fall (1->0)`.
pub fn arrow(rise: bool) -> &'static str {
    if rise { "rise (0->1)" } else { "fall (1->0)" }
}

/// A percentage in tenths, rounded down: a gate never rounds a missing item up (99.95 is
/// 99.9). `None` without items.
pub fn tenths(hit: usize, all: usize) -> Option<u64> {
    (all > 0).then(|| (hit as u64 * 1000) / all as u64)
}

/// `n` with the noun in the right number.
pub(crate) fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// [`tenths`] as `88.8%`, or `-` without items.
pub fn percent(hit: usize, all: usize) -> String {
    tenths(hit, all).map_or_else(|| "-".to_string(), |t| format!("{}.{}%", t / 10, t % 10))
}

fn xml(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, kind: Kind, hash: u64, line: u32, count: u64) -> (Key, Entry) {
        (
            Key {
                path: path.into(),
                kind,
                hash,
                occ: 0,
                sig: String::new(),
            },
            Entry {
                detail: Detail::default(),
                file: "a&b.vhd".into(),
                line,
                count,
                text: "x := 1;".into(),
                tests: Vec::new(),
                waivers: Vec::new(),
                unit: "work.e".into(),
                process: String::new(),
                operands: Vec::new(),
            },
        )
    }

    #[test]
    fn hash_ignores_layout() {
        assert_eq!(fnv1a("a  :=\n  b;"), fnv1a("a := b;"));
        assert_ne!(fnv1a("a := b;"), fnv1a("a := c;"));
    }

    #[test]
    fn write_parse_merge() {
        let a = Database {
            items: [
                item(":top", Kind::Statement, 1, 3, 2),
                item(":top", Kind::Branch(0), 2, 3, 0),
            ]
            .into(),
            ..Default::default()
        };
        let b = Database {
            items: [
                item(":top", Kind::Branch(0), 2, 3, 5),
                item("work.p", Kind::Statement, 3, 9, 0),
            ]
            .into(),
            ..Default::default()
        };
        assert_eq!(Database::parse(&a.write()).unwrap(), a);
        let mut m = a.clone();
        m.merge(b);
        let counts: Vec<u64> = m.items.values().map(|e| e.count).collect();
        assert_eq!(counts, [2, 5, 0]);
        assert!(Database::parse("yavs-coverage 2\ns\tx\n").is_err());
        assert!(Database::parse("other").is_err());
        let x = m.cobertura();
        assert!(x.contains("filename=\"a&amp;b.vhd\""), "{x}");
        assert!(x.contains("lines-covered=\"1\" lines-valid=\"2\""), "{x}");
        assert!(x.contains("condition-coverage=\"100% (1/1)\""), "{x}");
        let s = m.summary();
        assert!(
            s.contains("\nstatement      2    1       1     50.0%\nbranch         1    1       0    100.0%\n"),
            "{s}"
        );
        let s = m.summary_with(&SummaryOptions {
            detailed: true,
            ..SummaryOptions::default()
        });
        assert!(s.contains("=== work.p  (a&b.vhd) ==="), "{s}");
        assert!(s.contains("x := 1;"), "{s}");
    }

    #[test]
    fn percentages_round_down() {
        assert_eq!(tenths(0, 0), None);
        assert_eq!(percent(0, 0), "-");
        assert_eq!(percent(8, 9), "88.8%");
        assert_eq!(percent(1999, 2000), "99.9%");
        assert_eq!(percent(2000, 2000), "100.0%");
        assert_eq!(percent(0, 7), "0.0%");
        assert_eq!(arrow(true), "rise (0->1)");
        assert_eq!(arrow(false), "fall (1->0)");
    }

    #[test]
    fn toggles_round_trip() {
        let mut d = Database::default();
        let (mut k, e) = item(":top:u1", Kind::Toggle(true), 0, 4, 3);
        k.sig = "r.d(7)".into();
        d.items.insert(
            k,
            Entry {
                text: String::new(),
                ..e
            },
        );
        let t = d.write();
        assert!(
            t.ends_with("t1\t:top:u1\tr.d(7)\t3\t-\t4\ta&b.vhd\twork.e\n"),
            "{t}"
        );
        assert_eq!(Database::parse(&t).unwrap(), d);
        // Older versions still read: 2 (no toggles), 3 (no tests).
        assert!(Database::parse("yavs-coverage 2\n").is_ok());
        let v3 = Database::parse("yavs-coverage 3\nt1\t:top:u1\tr.d(7)\t3\t4\ta&b.vhd\n");
        let v3 = v3.unwrap();
        assert!(v3.legacy);
        assert!(v3.no_units);
        assert_eq!(
            v3.items.keys().collect::<Vec<_>>(),
            d.items.keys().collect::<Vec<_>>()
        );
        assert!(d.cobertura().contains("lines-valid=\"0\""));
        assert!(
            d.summary()
                .contains("\ntoggle      1    1       0    100.0%\n"),
            "{}",
            d.summary()
        );
    }

    /// Embedded sources survive write and parse, whatever their text; merging keeps them.
    #[test]
    fn sources_round_trip() {
        let mut d = Database::default();
        let text = "a\\n\tb\r\nsource\tx\n\n-- \u{e9}\n";
        d.sources.insert("a.vhd".to_string(), text.to_string());
        let t = d.write();
        assert_eq!(t.lines().count(), 2, "{t}");
        let back = Database::parse(&t).unwrap();
        assert_eq!(back.sources["a.vhd"], text);
        assert!(!back.legacy);
        let mut m = Database::default();
        m.merge(back);
        assert_eq!(m.sources["a.vhd"], text);
        // Version 6 is still read: it has none and records in-source exclusions.
        let v6 = Database::parse("yavs-coverage 6\n").unwrap();
        assert!(!v6.legacy && v6.sources.is_empty());
    }

    /// What `--detailed` needs survives write and parse, and a level differing between runs is
    /// not claimed as one.
    #[test]
    fn details_round_trip_and_merge() {
        let mut d = Database::default();
        let key = Key {
            path: ":t".into(),
            kind: Kind::Toggle(true),
            hash: 0,
            occ: 0,
            sig: "s".into(),
        };
        let detail = Detail {
            line: 2,
            col: 10,
            start: 5,
            end: 6,
            digest: 0xabc,
            under: Some((0xdef, 1, 2)),
            owner: Some((0x12, 3)),
            needs: "a is 1".into(),
            directive: true,
            level: '0',
        };
        d.items.insert(
            key.clone(),
            Entry {
                file: "a.vhd".into(),
                line: 2,
                count: 0,
                text: String::new(),
                tests: Vec::new(),
                waivers: Vec::new(),
                unit: "work.e".into(),
                process: String::new(),
                operands: Vec::new(),
                detail,
            },
        );
        let back = Database::parse(&d.write()).unwrap();
        assert_eq!(back, d);
        let mut other = back.clone();
        other.items.get_mut(&key).unwrap().detail.level = '1';
        let mut m = back;
        m.merge(other);
        assert_eq!(m.items[&key].detail.level, '?');
    }

    /// What the other inputs must be for an input to decide the result.
    #[test]
    fn needs_of_an_input() {
        use yavs_ir::CovNode as N;
        let names: Vec<Option<String>> = ["a", "b", "c"].map(|n| Some(n.to_string())).to_vec();
        // a and (b or c)
        let nodes = [N::Input(0), N::Input(1), N::Input(2), N::Or, N::And];
        assert_eq!(needs(&nodes, 0, &names).unwrap(), "b is 1 or c is 1");
        assert_eq!(needs(&nodes, 1, &names).unwrap(), "a is 1 and c is 0");
        // a or not b: a's other input is `not b`, which must be false, so b is 1.
        let nodes = [N::Input(0), N::Input(1), N::Not, N::Or];
        assert_eq!(needs(&nodes, 0, &names).unwrap(), "b is 1");
        // xor: any value of the other lets this input decide.
        let nodes = [N::Input(0), N::Input(1), N::Xor];
        assert_eq!(needs(&nodes, 0, &names).unwrap(), "");
    }

    /// A combination with in-source waivers survives write and parse, and merging a database
    /// that waives it with one that does not is a conflict.
    #[test]
    fn combinations_keep_their_waivers() {
        let key = Key {
            path: ":t".into(),
            kind: Kind::Combination(0b1001),
            hash: 7,
            occ: 0,
            sig: format!("valid{INPUT_SEP}ready"),
        };
        let entry = |count, waivers| Entry {
            detail: Detail::default(),
            file: "a.vhd".into(),
            line: 3,
            count,
            text: "valid and ready".into(),
            tests: Vec::new(),
            waivers,
            unit: "work.e".into(),
            process: String::new(),
            operands: Vec::new(),
        };
        let waiver = Waiver {
            rule: "01".into(),
            reason: "impossible handshake".into(),
            file: "a.vhd".into(),
            line: 2,
        };
        let mut d = Database::default();
        d.items.insert(key.clone(), entry(2, vec![waiver.clone()]));
        let text = d.write();
        assert!(text.starts_with("yavs-coverage 12\n"));
        let back = Database::parse(&text).unwrap();
        assert_eq!(back, d);
        assert_eq!(pattern(&key), "01");
        assert_eq!(combination_words(&key), "valid is false, ready is true");
        // The same waiver in both: kept once, counts added.
        let mut m = back.clone();
        assert!(m.merge(d.clone()).is_empty());
        assert_eq!(m.items[&key].count, 4);
        assert_eq!(m.items[&key].waivers, vec![waiver]);
        // Waived in one database only: a conflict, and the requirement stays.
        let mut other = Database::default();
        other.items.insert(key.clone(), entry(0, Vec::new()));
        let mut m = back;
        let conflicts = m.merge(other);
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert!(conflicts[0].contains("a.vhd:2"));
        assert!(m.items[&key].waivers.is_empty());
    }

    /// A row with its waivers and input count survives write and parse, and follows the merge rule.
    #[test]
    fn rows_keep_their_waivers() {
        let (k, mut e) = item(":t", Kind::Condition(3), 1, 5, 2);
        let k = Key {
            sig: "count > 3".into(),
            ..k
        };
        e.operands = vec!["a".into(), "count > 3".into(), "b".into()];
        let plain = e.clone();
        e.waivers = vec![Waiver {
            rule: "row -1-".into(),
            reason: "why".into(),
            file: "a.vhd".into(),
            line: 4,
        }];
        let db = |e: Entry| {
            let mut d = Database::default();
            d.items.insert(k.clone(), e);
            d
        };
        let d = db(e.clone());
        assert_eq!(Database::parse(&d.write()).unwrap(), d);
        // One side waives it: the conflict names the row.
        let mut m = db(plain);
        let c = m.merge(db(e));
        assert_eq!(c.len(), 1);
        assert!(c[0].contains("condition count > 3 = 1 of t "), "{}", c[0]);
        // A row line of version 8 has neither field.
        let v8 = "yavs-coverage 8\nc3\t:t\t0000000000000001\t0\t2\t-\t5\ta.vhd\tcount > 3\t? a\twork.e\t-\n";
        let back = Database::parse(v8).unwrap();
        let (_, e) = back.items.iter().next().unwrap();
        assert!(e.waivers.is_empty() && e.operands.is_empty() && e.unit == "work.e");
        // Version 10 has a count of inputs there, and operands in its waivers: not what this keeps.
        let v10 = "yavs-coverage 10\nc3\t:t\t0000000000000001\t0\t2\t-\t5\ta.vhd\tcount > 3\t? a\trow -1\u{1f}\u{1f}a.vhd\u{1f}4\u{1f}a\u{1d}count > 3\t2\twork.e\t-\n";
        let back = Database::parse(v10).unwrap();
        let (_, e) = back.items.iter().next().unwrap();
        assert!(e.operands.is_empty() && e.waivers[0].rule == "row -1");
    }

    #[test]
    fn tests_merge_and_rank() {
        // Items 1..=4; test a hits 1 2 3, b hits 3 4, c hits 4: {a, b} is the set, c is not needed.
        let run = |name: &str, hits: &[u64]| {
            let mut d = Database::default();
            for h in 1..=5 {
                let (k, e) = item(
                    ":t",
                    Kind::Statement,
                    h,
                    h as u32,
                    u64::from(hits.contains(&h)),
                );
                d.items.insert(k, e);
            }
            d.attribute(name);
            d
        };
        let mut m = run("c", &[4]);
        m.merge(run("a", &[1, 2, 3]));
        m.merge(run("b", &[3, 4]));
        let back = Database::parse(&m.write()).unwrap();
        assert_eq!(back.tests, ["a", "b", "c"]);
        let names = |db: &Database, h: u64| -> Vec<String> {
            let e = db.items.values().nth(h as usize - 1).unwrap();
            e.tests
                .iter()
                .map(|&t| db.tests[t as usize].clone())
                .collect()
        };
        assert_eq!(names(&back, 3), ["a", "b"]);
        assert_eq!(names(&back, 4), ["b", "c"]);
        assert!(names(&back, 5).is_empty());
        // The same file whatever the merge order.
        let mut m2 = run("a", &[1, 2, 3]);
        m2.merge(run("b", &[3, 4]));
        m2.merge(run("c", &[4]));
        assert_eq!(m2.write(), m.write());
        let r = m.rank();
        assert!(r.starts_with("3 tests hit 4 of 5 items\n"), "{r}");
        // One test and one item read in the singular (#950).
        let mut one = Database::default();
        let (k, e) = item(":t", Kind::Statement, 1, 1, 1);
        one.items.insert(k, e);
        one.attribute("a");
        let r1 = one.rank();
        assert!(r1.starts_with("1 test hit 1 of 1 item\n"), "{r1}");
        assert!(r.contains("  a     +3  3/4\n  b     +1  4/4\n"), "{r}");
        assert!(r.contains("not needed: c"), "{r}");
        assert!(r.contains("a            3         2\n"), "{r}");
        assert!(r.contains("c            1         0\n"), "{r}");
    }
}
