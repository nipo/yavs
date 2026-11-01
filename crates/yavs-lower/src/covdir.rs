//! In-source coverage directives (`-- yavs coverage exclude ...`, docs/coverage.md "Excluding
//! combinations in the source", #883): parsed from the comments the lexer finds, bound to the
//! decision or assignment right after them, and resolved to the combinations they waive.
//!
//! [`check`] validates every directive of a design's files at analysis, whether or not coverage
//! is collected; [`resolve`] gives the lowering the waivers of one file. Both read the final
//! HIR, so a warm analysis cache changes nothing, and use the one decision reading of
//! `covcond.rs`, so a directive and the counters it waives agree on the inputs.
//!
//! `exclude state` and `exclude transition` bind to a `case` on a state machine's register instead,
//! read with `yavs_lint::machines` (docs/coverage.md "State machines").

use rustc_hash::FxHashMap as HashMap;
use std::collections::BTreeSet;

use yavs_diag::{Diagnostic, FileId, Span};
use yavs_ir::CovWaiver;
use yavs_sem::hir::{Rhs, StmtKind, Waveform};
use yavs_sem::loader::LibSource;
use yavs_sem::{Db, DeclId, ExprId, UnitId};

use crate::covcond::Decision;

/// A malformed directive: missing parenthesis, separator or value, text after the pattern.
pub const SYNTAX: &str = "E2020";
/// A pattern with the wrong number of positions or an invalid symbol.
pub const PATTERN: &str = "E2021";
/// An unknown, ambiguous or repeated operand name.
pub const OPERAND: &str = "E2022";
/// A value that is not `true` or `false`.
pub const VALUE: &str = "E2023";
/// No eligible target, an ambiguous or unsupported one, or a decision with nothing to waive.
pub const TARGET: &str = "E2024";
/// An unknown command in a recognised `yavs coverage` directive.
pub const COMMAND: &str = "E2025";
/// A duplicate or subsumed exclusion.
pub const REDUNDANT: &str = "W2020";
/// The exclusions together waive every combination of a decision.
pub const ALL_WAIVED: &str = "W2021";
/// An exclusion that matches no combination of its decision.
pub const MATCHES_NONE: &str = "W2022";
/// A decision whose combinations are not measured (`--cov-kind=combination`).
pub const NOT_MEASURED: &str = "W2023";

/// What `yavs coverage` may be followed by.
const COMMANDS: [&str; 4] = ["exclude", "off", "on", "off-line"];
/// What may stand where a pattern is expected, as a word.
const SPEC_WORDS: [&str; 5] = ["row", "condition", "expression", "state", "transition"];

/// The waivers of one file by decision, and the diagnostics of its directives.
#[derive(Default)]
pub struct Checked {
    pub waivers: HashMap<ExprId, Vec<CovWaiver>>,
    /// What `exclude state` and `exclude transition` say, by state register.
    pub machines: HashMap<DeclId, Vec<MachineWaiver>>,
    pub diags: Vec<Diagnostic>,
}

/// An `exclude state` or `exclude transition`: states by position in the type, `None` is `*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineWaiver {
    pub item: MachineItem,
    /// The directive as written after `exclude`, without the reason.
    pub rule: String,
    pub reason: String,
    /// The comment.
    pub at: Span,
}

/// What a [`MachineWaiver`] takes out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineItem {
    State(usize),
    Transition(Option<usize>, Option<usize>),
}

impl MachineItem {
    /// Whether it takes the transition `from` to `to` out.
    pub fn takes_transition(self, from: usize, to: usize) -> bool {
        match self {
            MachineItem::State(_) => false,
            MachineItem::Transition(f, t) => {
                f.is_none_or(|f| f == from) && t.is_none_or(|t| t == to)
            }
        }
    }

    /// Whether every item `other` takes out, this takes out too.
    fn covers(self, other: MachineItem) -> bool {
        match (self, other) {
            (MachineItem::State(a), MachineItem::State(b)) => a == b,
            (MachineItem::Transition(f, t), MachineItem::Transition(g, u)) => {
                (f.is_none() || f == g) && (t.is_none() || t == u)
            }
            _ => false,
        }
    }
}

/// Every source file of the design (the libraries yavs provides left out).
fn design_files(db: &Db) -> BTreeSet<FileId> {
    db.units
        .iter()
        .filter(|(_, u)| !matches!(db.libs[u.lib].source, LibSource::StdDir(_)))
        .map(|(_, u)| u.file)
        .collect()
}

/// The diagnostics of the directives in the files at `paths` (just analysed): the analysis
/// reports them, so they are part of what the compilation cache replays.
pub fn check_paths(db: &Db, paths: &[&std::path::Path]) -> Vec<Diagnostic> {
    let want: BTreeSet<std::path::PathBuf> = paths
        .iter()
        .filter_map(|p| std::path::absolute(p).ok())
        .collect();
    design_files(db)
        .into_iter()
        .filter(|f| std::path::absolute(&db.sources.get(*f).path).is_ok_and(|p| want.contains(&p)))
        .flat_map(|f| resolve(db, f).diags)
        .collect()
}

/// A warning for each decision of the design whose combinations are not measured
/// (`--cov-kind=combination`).
pub fn unmeasured_all(db: &Db) -> Vec<Diagnostic> {
    design_files(db)
        .into_iter()
        .flat_map(|f| unmeasured(db, f))
        .collect()
}

/// The number of `exclude` directives in the design's source files (`--coverage-directives=ignore`
/// says it left that many out).
pub fn count(db: &Db) -> usize {
    design_files(db)
        .into_iter()
        .map(|f| {
            let src = db.sources.get(f);
            if !mentions(src.bytes()) {
                return 0;
            }
            let bytes = src.bytes();
            yavs_syntax::line_comments(src, std_of(db))
                .iter()
                .filter_map(|c| bytes.get(c.start as usize..c.end as usize))
                .filter(|t| {
                    matches!(
                        parse(f, 0, &String::from_utf8_lossy(t)),
                        Parsed::Ok(_) | Parsed::Err(_)
                    )
                })
                .count()
        })
        .sum()
}

fn std_of(db: &Db) -> yavs_syntax::Std {
    match db.rev {
        yavs_sem::LangRev::Vhdl1993 => yavs_syntax::Std::Vhdl1993,
        yavs_sem::LangRev::Vhdl2008 => yavs_syntax::Std::Vhdl2008,
        yavs_sem::LangRev::Vhdl2019 => yavs_syntax::Std::Vhdl2019,
    }
}

/// What a directive can attach to.
#[derive(Clone)]
enum What {
    /// A decision (`cond`) or the value of an assignment, with its expression.
    Expr { expr: ExprId, decision: bool },
    /// A construct a directive cannot attach to, and why.
    No(&'static str),
}

/// The constructs of `file` by the start of their first token (the label, or the keyword).
fn sites(db: &Db, file: FileId) -> HashMap<u32, Vec<What>> {
    let text = db.sources.get(file).text();
    let mut out: HashMap<u32, Vec<What>> = HashMap::default();
    for (_, s) in db.stmts.iter().filter(|(_, s)| s.span.file == file) {
        let start = s.span.start;
        let mut add = |at: u32, w: What| out.entry(at).or_default().push(w);
        let value = |w: &Waveform| match w {
            Waveform::Elements(e) if e.len() == 1 && e[0].after.is_none() => e[0].value,
            _ => None,
        };
        match &s.kind {
            StmtKind::If { branches, .. } => {
                for (k, (c, _)) in branches.iter().enumerate() {
                    let what = What::Expr {
                        expr: *c,
                        decision: true,
                    };
                    if k == 0 {
                        add(start, what);
                    } else if let Some(at) = keyword_before(text, db.exprs[*c].span.start, "elsif")
                    {
                        add(at, what);
                    }
                }
            }
            StmtKind::Loop {
                scheme: yavs_sem::hir::LoopScheme::While(c),
                ..
            }
            | StmtKind::Next { when: Some(c), .. }
            | StmtKind::Exit { when: Some(c), .. } => add(
                start,
                What::Expr {
                    expr: *c,
                    decision: true,
                },
            ),
            StmtKind::VarAssign {
                rhs: Rhs::Simple(e),
                ..
            } => add(
                start,
                What::Expr {
                    expr: *e,
                    decision: false,
                },
            ),
            StmtKind::SignalAssign {
                rhs: Rhs::Simple(w),
                ..
            }
            | StmtKind::ConcSignalAssign {
                rhs: Rhs::Simple(w),
                ..
            } => match value(w) {
                Some(e) => add(
                    start,
                    What::Expr {
                        expr: e,
                        decision: false,
                    },
                ),
                None => add(
                    start,
                    What::No("this signal assignment has no single value without a delay"),
                ),
            },
            StmtKind::VarAssign { .. }
            | StmtKind::SignalAssign { .. }
            | StmtKind::ConcSignalAssign { .. } => add(
                start,
                What::No(
                    "this assignment has several conditions or choices, so the directive would \
                     fit more than one expression",
                ),
            ),
            _ => add(
                start,
                What::No(
                    "a directive applies to an `if`, `elsif`, `while`, `exit`, `next` or an \
                     assignment, sequential or concurrent",
                ),
            ),
        }
    }
    out
}

/// Whether only blanks stand before byte `pos` on its line.
fn own_line(bytes: &[u8], pos: u32) -> bool {
    let before = &bytes[..pos as usize];
    let line = before.rsplit(|&c| c == b'\n').next().unwrap_or(before);
    line.iter().all(u8::is_ascii_whitespace)
}

/// The start of keyword `kw` right before byte `pos` of `text`, parentheses and blanks between.
fn keyword_before(text: &str, pos: u32, kw: &str) -> Option<u32> {
    let head = text.get(..pos as usize)?;
    let head = head.trim_end_matches(|c: char| c.is_whitespace() || c == '(');
    let at = head.len().checked_sub(kw.len())?;
    let word = head.get(at..)?;
    let before_ok = !head[..at]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_');
    (word.eq_ignore_ascii_case(kw) && before_ok).then_some(at as u32)
}

/// The decisions and values of `file` that have more than one input but whose combinations
/// are not measured, as warnings.
fn unmeasured(db: &Db, file: FileId) -> Vec<Diagnostic> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut all: Vec<_> = sites(db, file).into_iter().collect();
    all.sort_by_key(|x| x.0);
    for (_, ws) in all {
        for w in ws {
            let What::Expr { expr, .. } = w else { continue };
            let Some(d) = Decision::new(db, expr) else {
                continue;
            };
            let span = db.exprs[d.whole].span;
            if d.leaves.len() < 2 || !seen.insert((span.start, span.end)) {
                continue;
            }
            if let Err(why) = d.combinations(db) {
                out.push(
                    Diagnostic::warning(
                        NOT_MEASURED,
                        "the combinations of this expression are not measured",
                    )
                    .with_primary(span, "no combination coverage")
                    .with_note(why),
                );
            }
        }
    }
    out
}

/// One value in a named exclusion: `true` or `false`.
type Named = (String, Span, bool);

/// What a directive says about the inputs.
enum Spec {
    /// One symbol per input: `0`, `1`, `-`, with the span of the pattern.
    Pattern(Vec<u8>, Span),
    Named(Vec<Named>),
    /// `row -0-`: one input at one value, by position, with the span of the pattern.
    RowPattern(Vec<u8>, Span),
    /// `row INPUT = VALUE`: one input at one value, by its text.
    RowNamed(Named),
    /// `state NAME`, with the span of the name.
    State(String, Span),
    /// `transition FROM -> TO`: each a name or `*`, with its span.
    Transition((String, Span), (String, Span)),
}

/// The inputs a pattern says it was written for, with the span of the list.
type Operands = Option<(Vec<String>, Span)>;

/// A parsed `exclude` directive.
struct Directive {
    /// The comment.
    at: Span,
    /// `Some(true)` for `condition`, `Some(false)` for `expression`.
    kind: Option<bool>,
    spec: Spec,
    /// The inputs a pattern says it was written for, `01 (a, b)`, with the span of the list.
    operands: Operands,
    /// The spec as written, for reports.
    rule: String,
    reason: String,
}

/// Read the directives of `file`, bind each to its target and resolve its waivers.
pub fn resolve(db: &Db, file: FileId) -> Checked {
    let src = db.sources.get(file);
    let std = std_of(db);
    let mut out = Checked::default();
    // The file is lexed only when a comment can be a directive.
    if !mentions(src.bytes()) {
        return out;
    }
    let bytes = src.bytes();
    let mut groups: Vec<(Option<u32>, Vec<Directive>)> = Vec::new();
    for c in yavs_syntax::line_comments(src, std) {
        let Some(raw) = bytes.get(c.start as usize..c.end as usize) else {
            continue;
        };
        let text = String::from_utf8_lossy(raw);
        match parse(file, c.start, &text) {
            Parsed::Other => {}
            Parsed::Err(d) => out.diags.push(*d),
            // After code on its line it would apply to the next statement, not to that code.
            Parsed::Ok(d) if !own_line(bytes, c.start) => out.diags.push(
                Diagnostic::error(TARGET, "this exclusion does not stand on its own line")
                    .with_primary(
                        d.at,
                        "a directive applies to what follows it, not to this line",
                    )
                    .with_help("write it on the line before the construct"),
            ),
            Parsed::Ok(d) => match groups.last_mut() {
                Some((next, v)) if *next == c.next && c.next.is_some() => v.push(*d),
                _ => groups.push((c.next, vec![*d])),
            },
        }
    }
    if groups.is_empty() {
        return out;
    }
    let sites = sites(db, file);
    let mut machines = None;
    let at = |d: &Diagnostic| {
        d.primary
            .as_ref()
            .map(|p| (p.span.file, p.span.start, p.span.end))
    };
    let mut seen = BTreeSet::new();
    for (next, ds) in groups {
        let targets = next.and_then(|n| sites.get(&n));
        for d in &ds {
            if matches!(d.spec, Spec::State(..) | Spec::Transition(..)) {
                let of = machines.get_or_insert_with(|| Machines::of(db, file));
                bind_machine(db, d, next, of, &mut out);
                continue;
            }
            let Some(targets) = targets else {
                out.diags.push(
                    Diagnostic::error(TARGET, "no construct follows this exclusion")
                        .with_primary(d.at, "there is nothing to attach it to")
                        .with_help(
                            "write it right before an `if`, `elsif`, `while`, `exit`, `next` \
                             or an assignment, sequential or concurrent",
                        ),
                );
                continue;
            };
            for w in targets {
                let diags = match w {
                    What::No(why) => vec![
                        Diagnostic::error(TARGET, "this construct cannot be excluded from")
                            .with_primary(d.at, *why),
                    ],
                    What::Expr { expr, decision } => {
                        bind(db, d, *expr, *decision, &mut out.waivers)
                    }
                };
                // The same source text analysed more than once (generic clones) reports once.
                for x in diags {
                    if seen.insert((x.code, x.message.clone(), at(&x))) {
                        out.diags.push(x);
                    }
                }
            }
        }
    }
    for (e, v) in &out.waivers {
        redundant(db, *e, v, &mut out.diags);
    }
    if let Some(of) = &machines {
        redundant_machine(of, &out.machines, &mut out.diags);
    }
    out.diags.sort_by_key(at);
    out.diags
        .dedup_by(|a, b| a.code == b.code && a.message == b.message && at(a) == at(b));
    out
}

/// The state machines of the architectures of one file.
struct Machines {
    found: HashMap<DeclId, yavs_lint::Machine>,
    missed: Vec<yavs_lint::NotMeasured>,
}

impl Machines {
    fn of(db: &Db, file: FileId) -> Self {
        let mut all = Machines {
            found: HashMap::default(),
            missed: Vec::new(),
        };
        let units: Vec<UnitId> = db
            .units
            .iter()
            .filter(|(_, u)| u.file == file)
            .map(|x| x.0)
            .collect();
        for u in units {
            let (found, missed) = yavs_lint::machines(db, u);
            all.found.extend(found.into_iter().map(|m| (m.signal, m)));
            all.missed.extend(missed);
        }
        all
    }
}

/// The selectors of the `case` a state machine directive stands before: the `case` starting at byte
/// `next`, or the one whose alternative `when` is there. A construct analysed more than once gives
/// one each. A `when` in a statement within an alternative (a conditional assignment) is not one.
fn case_selectors(db: &Db, file: FileId, next: u32) -> Vec<ExprId> {
    let text = db.sources.get(file).text();
    let rest = text.get(next as usize..).unwrap_or("");
    let word = rest
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or("");
    let of_file = || db.stmts.iter().filter(|(_, s)| s.span.file == file);
    let selector = |k: &StmtKind| match k {
        StmtKind::Case { expr, .. } => Some(*expr),
        _ => None,
    };
    if word.eq_ignore_ascii_case("when") {
        let around: Vec<_> = of_file()
            .filter(|(_, s)| s.span.start < next && next < s.span.end)
            .collect();
        let inner = around.iter().map(|(_, s)| s.span.end - s.span.start).min();
        return around
            .iter()
            .filter(|(_, s)| Some(s.span.end - s.span.start) == inner)
            .filter_map(|(_, s)| selector(&s.kind))
            .collect();
    }
    of_file()
        .filter(|(_, s)| s.span.start == next)
        .filter_map(|(_, s)| selector(&s.kind))
        .collect()
}

/// Bind `exclude state` or `exclude transition` to the `case` right after it.
fn bind_machine(db: &Db, d: &Directive, next: Option<u32>, of: &Machines, out: &mut Checked) {
    let file = d.at.file;
    let selectors = next.map_or(Vec::new(), |n| case_selectors(db, file, n));
    if selectors.is_empty() {
        out.diags.push(
            Diagnostic::error(
                TARGET,
                "a state or transition exclusion needs a `case` on the state register after it",
            )
            .with_primary(d.at, "no `case` or `when` follows")
            .with_help(
                "write it right before the `case` on the register, or before one of its `when` \
                 alternatives",
            ),
        );
        return;
    }
    let register = |e: &ExprId| match db.exprs[*e].kind {
        yavs_sem::hir::ExprKind::Obj(r) => of.found.contains_key(&r).then_some(r),
        _ => None,
    };
    let registers: BTreeSet<DeclId> = selectors.iter().filter_map(register).collect();
    if registers.is_empty() {
        let shown = db.exprs[selectors[0]].span;
        let src = db.sources.get(shown.file);
        let text = src
            .text()
            .get(src.text_offset(shown.start)..src.text_offset(shown.end))
            .unwrap_or("");
        let why = of
            .missed
            .iter()
            .find(|m| m.what.eq_ignore_ascii_case(text))
            .map_or(
                "a state machine is a signal of an enumeration type that a clocked statement \
                 drives and a `case` selects on",
                |m| m.why,
            );
        out.diags.push(
            Diagnostic::error(TARGET, "this `case` is not on a detected state machine")
                .with_primary(d.at, "there is no state or transition to exclude")
                .with_secondary(shown, "the `case` selects on this")
                .with_note(why),
        );
        return;
    }
    for r in registers {
        match machine_item(db, d, &of.found[&r]) {
            Ok((item, warning)) => {
                out.diags.extend(warning);
                let w = MachineWaiver {
                    item,
                    rule: d.rule.clone(),
                    reason: d.reason.clone(),
                    at: d.at,
                };
                let v = out.machines.entry(r).or_default();
                if !v.contains(&w) {
                    v.push(w);
                }
            }
            Err(e) => out.diags.push(*e),
        }
    }
}

/// What `exclude state` or `exclude transition` names in machine `m`, and W2022 if that is no item.
fn machine_item(
    db: &Db,
    d: &Directive,
    m: &yavs_lint::Machine,
) -> Result<(MachineItem, Option<Diagnostic>), Box<Diagnostic>> {
    let ty = db.decls[db.types[m.ty].decl].name.to_string();
    let find = |name: &str, at: Span| -> Result<usize, Box<Diagnostic>> {
        // Only a basic identifier is the same in another case.
        let basic = !name.starts_with(['\\', '\'']);
        let same = |s: &str| {
            if basic {
                s.eq_ignore_ascii_case(name)
            } else {
                s == name
            }
        };
        if let Some(i) = m.states.iter().position(|(s, _)| same(s)) {
            return Ok(i);
        }
        let list: Vec<&str> = m.states.iter().map(|s| s.0.as_str()).collect();
        let mut e = Diagnostic::error(OPERAND, format!("`{name}` is not a state of `{ty}`"))
            .with_primary(at, "unknown state")
            .with_note(format!("the states are: {}", list.join(", ")));
        let keys: Vec<(String, &str)> = list.iter().map(|s| (s.to_ascii_lowercase(), *s)).collect();
        if let Some(h) = yavs_sem::suggest::suggest(
            &name.to_ascii_lowercase(),
            "state",
            keys.iter().map(|(k, s)| (k.as_str(), *s)),
        ) {
            e = e.with_help(h);
        }
        Err(Box::new(e))
    };
    let (from, to) = match &d.spec {
        Spec::State(name, at) => return Ok((MachineItem::State(find(name, *at)?), None)),
        Spec::Transition(from, to) => (from, to),
        _ => unreachable!("only a state or transition exclusion comes here"),
    };
    let end = |(name, at): &(String, Span)| match name.as_str() {
        "*" => Ok(None),
        n => find(n, *at).map(Some),
    };
    let item = MachineItem::Transition(end(from)?, end(to)?);
    let none = !m.transitions().any(|a| item.takes_transition(a.0, a.1));
    let warning = none.then(|| {
        Diagnostic::warning(
            MATCHES_NONE,
            "this exclusion matches no transition of the state machine",
        )
        .with_primary(d.at, "no `case` alternative assigns this")
        .with_note(
            "a state staying as it is has no item; a transition no alternative names, such as a \
             reset, is an item only in a run where it happens, and is then excluded",
        )
    });
    Ok((item, warning))
}

/// Warnings about the state machine waivers of one file: duplicates, subsumed ones, all of a kind.
fn redundant_machine(
    of: &Machines,
    all: &HashMap<DeclId, Vec<MachineWaiver>>,
    diags: &mut Vec<Diagnostic>,
) {
    for (r, v) in all {
        for (j, b) in v.iter().enumerate() {
            let same = |a: &MachineWaiver| a.item == b.item;
            let over = v
                .iter()
                .enumerate()
                .find(|&(i, a)| i != j && a.item.covers(b.item) && (!same(a) || i < j));
            if let Some((_, a)) = over {
                let msg = if same(a) {
                    "this exclusion repeats an earlier one"
                } else {
                    "this exclusion is already covered by a broader one"
                };
                diags.push(
                    Diagnostic::warning(REDUNDANT, msg)
                        .with_primary(b.at, "redundant")
                        .with_secondary(a.at, "this one waives it already"),
                );
            }
        }
        let Some(m) = of.found.get(r) else { continue };
        let every_state =
            (0..m.states.len()).all(|s| v.iter().any(|w| w.item == MachineItem::State(s)));
        let mut arcs = m.transitions().peekable();
        let every_arc = arcs.peek().is_some()
            && arcs.all(|a| v.iter().any(|w| w.item.takes_transition(a.0, a.1)));
        for (all, what) in [(every_state, "state"), (every_arc, "transition")] {
            if all {
                diags.push(
                    Diagnostic::warning(
                        ALL_WAIVED,
                        format!("the exclusions together waive every {what} of this state machine"),
                    )
                    .with_primary(v[0].at, "nothing is left to cover"),
                );
            }
        }
    }
}

/// Resolve directive `d` against the decision `expr`; the waiver goes into `waivers`.
fn bind(
    db: &Db,
    d: &Directive,
    expr: ExprId,
    decision: bool,
    waivers: &mut HashMap<ExprId, Vec<CovWaiver>>,
) -> Vec<Diagnostic> {
    let err = |code, msg: &str, label: &str| {
        Diagnostic::error(code, msg.to_string()).with_primary(d.at, label.to_string())
    };
    if let Some(want) = d.kind
        && want != decision
    {
        return vec![
            err(
                TARGET,
                &format!(
                    "`exclude {}` does not fit {}",
                    if want { "condition" } else { "expression" },
                    if decision {
                        "a condition"
                    } else {
                        "the value of an assignment"
                    }
                ),
                "wrong kind",
            )
            .with_help(format!(
                "write `exclude {}`, or leave the word out",
                if decision { "condition" } else { "expression" }
            )),
        ];
    }
    let Some(dec) = Decision::new(db, expr) else {
        return vec![
            err(
                TARGET,
                "this is not an expression of `boolean`, `bit` or `std_ulogic` operators",
                "no inputs to exclude combinations of",
            )
            .with_secondary(db.exprs[expr].span, "the expression"),
        ];
    };
    let whole = db.exprs[dec.whole].span;
    let n = dec.leaves.len();
    let src = db.sources.get(whole.file);
    let flat = |s: Span| {
        let t = src
            .text()
            .get(src.text_offset(s.start)..src.text_offset(s.end))
            .unwrap_or("");
        t.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let names: Vec<Option<String>> = dec
        .leaves
        .iter()
        .map(|l| match &db.exprs[l.expr].kind {
            yavs_sem::hir::ExprKind::Obj(d) => Some(db.decls[*d].name.to_string()),
            _ => None,
        })
        .collect();
    // Each input as a directive may write it: any formatting.
    let source: Vec<&str> = dec
        .leaves
        .iter()
        .map(|l| {
            let s = db.exprs[l.expr].span;
            src.text()
                .get(src.text_offset(s.start)..src.text_offset(s.end))
                .unwrap_or("")
        })
        .collect();
    let texts: Vec<String> = source.iter().map(|t| yavs_diag::operand::key(t)).collect();
    let order = || {
        (0..n)
            .map(|i| {
                names[i]
                    .clone()
                    .unwrap_or_else(|| flat(db.exprs[dec.leaves[i].expr].span))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    // The index of the input a named form means, or the error.
    let find = |name: &str, at: Span| -> Result<usize, Box<Diagnostic>> {
        let key = yavs_diag::operand::key(name);
        let hits: Vec<usize> = (0..n)
            .filter(|&i| {
                texts[i] == key
                    || names[i].as_ref().is_some_and(|x| {
                        if name.starts_with('\\') {
                            x == name
                        } else {
                            x.eq_ignore_ascii_case(name)
                        }
                    })
            })
            .collect();
        match hits[..] {
            [i] => Ok(i),
            [] => {
                let mut e = Diagnostic::error(
                    OPERAND,
                    format!("`{name}` is not an input of this expression"),
                )
                .with_primary(at, "unknown input")
                .with_secondary(whole, "the expression")
                .with_note(format!("inputs, in order: {}", order()));
                let cands: Vec<(String, &str)> = names
                    .iter()
                    .flatten()
                    .map(|x| (x.to_ascii_lowercase(), x.as_str()))
                    .chain(texts.iter().map(|x| (x.clone(), x.as_str())))
                    .collect();
                if let Some(h) = yavs_sem::suggest::suggest(
                    &key,
                    "input",
                    cands.iter().map(|(k, x)| (k.as_str(), *x)),
                ) {
                    e = e.with_help(h);
                } else {
                    e = e.with_help(
                        "write the input as the note lists it, in any formatting, or address it \
                         by its position with the pattern form, such as `01-`",
                    );
                }
                Err(Box::new(e))
            }
            _ => Err(Box::new(
                Diagnostic::error(
                    OPERAND,
                    format!("`{name}` is {} inputs of this expression", hits.len()),
                )
                .with_primary(at, "ambiguous input")
                .with_secondary(whole, "the expression")
                .with_note(format!("inputs, in order: {}", order()))
                .with_help("address the inputs by position with the pattern form, such as `01-`"),
            )),
        }
    };
    if let Some((found, at)) = &d.operands {
        let keys: Vec<String> = found.iter().map(|o| yavs_diag::operand::key(o)).collect();
        if keys != texts {
            let wanted: Vec<String> = source.iter().map(|t| yavs_diag::operand::flat(t)).collect();
            return vec![stale_operands(d, found, &keys, &texts, &wanted, *at, whole)];
        }
    }
    let wrong_length = |len: usize, at: Span| {
        err(
            PATTERN,
            &format!(
                "exclusion has {len} position{}, but the expression has {n} inputs",
                if len == 1 { "" } else { "s" }
            ),
            "wrong number of positions",
        )
        .with_secondary(whole, "the expression")
        .with_note(format!("expected order: {}", order()))
        .with_help("provide one symbol per input; use `-` for an unrestricted input")
        .with_secondary(at, "the pattern")
    };
    // A condition or expression row: one input at one value. It has nothing to do with the
    // combinations, so a decision with too many of them can still have rows.
    let row = match &d.spec {
        Spec::RowPattern(p, at) => {
            if p.len() != n {
                return vec![wrong_length(p.len(), *at)];
            }
            let i = p.iter().position(|&c| c != b'-').unwrap_or(0);
            Some((i, p[i] == b'1', *at))
        }
        Spec::RowNamed((name, at, value)) => match find(name, *at) {
            Ok(i) => Some((i, *value, *at)),
            Err(e) => return vec![*e],
        },
        _ => None,
    };
    if let Some((i, value, at)) = row {
        let consts = dec.consts();
        if n - (consts.count_ones() as usize) < 2 {
            return vec![
                err(TARGET, "this expression has no rows to exclude", "no rows")
                    .with_secondary(whole, "the expression")
                    .with_note(
                        "condition and expression rows need two or more inputs that are not constant",
                    ),
            ];
        }
        let mut diags = Vec::new();
        if consts >> i & 1 == 1 {
            diags.push(
                Diagnostic::warning(MATCHES_NONE, "this exclusion names an input with no rows")
                    .with_primary(at, "constant after elaboration")
                    .with_secondary(whole, "the expression"),
            );
        }
        waivers.entry(dec.whole).or_default().push(CovWaiver {
            care: 0,
            want: 0,
            row: Some(2 * i as u32 + u32::from(value)),
            rule: d.rule.clone(),
            reason: d.reason.clone(),
            at: d.at,
        });
        return diags;
    }
    if let Err(why) = dec.combinations(db) {
        return vec![
            err(
                TARGET,
                "this expression has no combinations to exclude",
                "no requirements",
            )
            .with_secondary(whole, "the expression")
            .with_note(why),
        ];
    }
    let (mut care, mut want) = (0u32, 0u32);
    match &d.spec {
        Spec::Pattern(p, at) => {
            if p.len() != n {
                return vec![wrong_length(p.len(), *at)];
            }
            for (i, &c) in p.iter().enumerate() {
                match c {
                    b'0' | b'1' => {
                        care |= 3 << (2 * i);
                        want |= (1 + u32::from(c == b'1')) << (2 * i);
                    }
                    _ => {}
                }
            }
        }
        Spec::Named(list) => {
            let mut done: Vec<usize> = Vec::new();
            for (name, at, value) in list {
                let i = match find(name, *at) {
                    Ok(i) => i,
                    Err(e) => return vec![*e],
                };
                if done.contains(&i) {
                    return vec![
                        Diagnostic::error(OPERAND, format!("`{name}` is given twice"))
                            .with_primary(*at, "repeated input"),
                    ];
                }
                done.push(i);
                care |= 3 << (2 * i);
                want |= (1 + u32::from(*value)) << (2 * i);
            }
        }
        Spec::RowPattern(..) | Spec::RowNamed(..) => unreachable!("a row returned above"),
        Spec::State(..) | Spec::Transition(..) => unreachable!("bound by bind_machine"),
    }
    let states = dec.combinations(db).unwrap_or_default();
    let matched = states.iter().filter(|&&s| s & care == want).count();
    let mut diags = Vec::new();
    if matched == 0 {
        diags.push(
            Diagnostic::warning(
                MATCHES_NONE,
                "this exclusion matches no combination of the expression",
            )
            .with_primary(d.at, "nothing to exclude")
            .with_secondary(whole, "the expression")
            .with_note(
                "an operand that a short-circuit operator does not evaluate has no value in that \
                 combination: write `-` for it",
            ),
        );
    }
    waivers.entry(dec.whole).or_default().push(CovWaiver {
        care,
        want,
        row: None,
        rule: d.rule.clone(),
        reason: d.reason.clone(),
        at: d.at,
    });
    diags
}

/// E2022 for a directive whose list of inputs is not the decision's: what it found and expected, and
/// the directive to write. A symbol follows its input; an input the list did not have gets `-`.
fn stale_operands(
    d: &Directive,
    found: &[String],
    keys: &[String],
    texts: &[String],
    wanted: &[String],
    at: Span,
    whole: Span,
) -> Diagnostic {
    let mut e = Diagnostic::error(
        OPERAND,
        "the inputs this exclusion names no longer match the expression",
    )
    .with_primary(at, format!("found: {}", found.join(", ")))
    .with_secondary(whole, format!("expected: {}", wanted.join(", ")));
    let (symbols, row) = match &d.spec {
        Spec::Pattern(p, _) => (p, ""),
        Spec::RowPattern(p, _) => (p, "row "),
        _ => return e,
    };
    let mut free: Vec<usize> = (0..keys.len()).collect();
    let mut known = 0;
    let moved: String = texts
        .iter()
        .map(|t| {
            let j = free
                .iter()
                .position(|&j| keys[j] == *t)
                .map(|k| free.remove(k));
            known += usize::from(j.is_some());
            let symbol = j.and_then(|j| symbols.get(j)).copied().unwrap_or(b'-');
            char::from(symbol)
        })
        .collect();
    if moved.bytes().all(|c| c == b'-') {
        return e.with_help(format!(
            "what it waived is gone: write one symbol per input for ({}), or take it out",
            wanted.join(", ")
        ));
    }
    if known < texts.len() {
        e = e.with_note(
            "`-` stands for an input the directive did not name: check that it means what you want",
        );
    }
    let reason = if d.reason.is_empty() {
        String::new()
    } else {
        format!(": {}", d.reason)
    };
    e.with_help(format!(
        "write: -- yavs cov exclude {}{row}{moved} ({}){reason}",
        kind_word(d.kind),
        wanted.join(", ")
    ))
}

/// Warnings about the waivers of one decision: duplicates, subsumed ones, all combinations.
fn redundant(db: &Db, expr: ExprId, v: &[CovWaiver], diags: &mut Vec<Diagnostic>) {
    // Rows and combinations are different requirements: one never covers the other.
    let covers = |a: &CovWaiver, b: &CovWaiver| {
        a.row == b.row && a.care & b.care == a.care && b.want & a.care == a.want
    };
    for j in 0..v.len() {
        for i in 0..v.len() {
            let dup = v[i].row == v[j].row && v[i].care == v[j].care && v[i].want == v[j].want;
            let first = i < j;
            if i != j && covers(&v[i], &v[j]) && (!dup || first) {
                diags.push(
                    Diagnostic::warning(
                        REDUNDANT,
                        if dup {
                            "this exclusion repeats an earlier one"
                        } else {
                            "this exclusion is already covered by a broader one"
                        },
                    )
                    .with_primary(v[j].at, "redundant")
                    .with_secondary(v[i].at, "this one waives it already"),
                );
                break;
            }
        }
    }
    let Some(dec) = Decision::new(db, expr) else {
        return;
    };
    let states = dec.combinations(db).unwrap_or_default();
    if !states.is_empty()
        && states
            .iter()
            .all(|&s| v.iter().any(|w| w.row.is_none() && s & w.care == w.want))
    {
        diags.push(
            Diagnostic::warning(
                ALL_WAIVED,
                "the exclusions together waive every combination of this expression",
            )
            .with_primary(v[0].at, "nothing is left to cover")
            .with_secondary(db.exprs[expr].span, "the expression"),
        );
    }
    let rows = |r: &&CovWaiver| r.row.is_some();
    let all: BTreeSet<_> = v.iter().filter(rows).filter_map(|w| w.row).collect();
    if all.len() == 2 * dec.leaves.len() {
        diags.push(
            Diagnostic::warning(
                ALL_WAIVED,
                "the exclusions together waive every row of this expression",
            )
            .with_primary(v[0].at, "nothing is left to cover")
            .with_secondary(db.exprs[expr].span, "the expression"),
        );
    }
}

/// Whether `bytes` has the word `yavs` in any case: the only files whose comments are read.
pub fn mentions(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|w| w.eq_ignore_ascii_case(b"yavs"))
}

/// A comment seen by [`parse`].
enum Parsed {
    /// Not a coverage directive (or a pragma of the old `off`/`on` family).
    Other,
    Err(Box<Diagnostic>),
    Ok(Box<Directive>),
}

/// Read comment `text`, which starts at byte `start` of `file`.
fn parse(file: FileId, start: u32, text: &str) -> Parsed {
    let span =
        |from: usize, len: usize| Span::new(file, start + from as u32, start + (from + len) as u32);
    let all = span(0, text.len());
    let body = text.trim_start_matches('-');
    let skipped = text.len() - body.len();
    let mut cur = Cursor {
        s: body,
        at: skipped,
    };
    cur.blank();
    if !cur.word().eq_ignore_ascii_case("yavs") {
        return Parsed::Other;
    }
    cur.blank();
    let second = cur.word();
    if !(second.eq_ignore_ascii_case("cov") || second.eq_ignore_ascii_case("coverage")) {
        return Parsed::Other;
    }
    let fail = |code, msg: String, from: usize, len: usize, label: &str| {
        Parsed::Err(Box::new(
            Diagnostic::error(code, msg).with_primary(span(from, len.max(1)), label.to_string()),
        ))
    };
    cur.blank();
    let cmd_at = cur.at;
    let cmd = cur.command();
    match cmd.to_ascii_lowercase().as_str() {
        "off" | "on" | "off-line" => return Parsed::Other,
        "exclude" => {}
        "" => {
            return Parsed::Err(Box::new(
                Diagnostic::error(COMMAND, "a coverage directive needs a command")
                    .with_primary(all, "no command after `coverage`")
                    .with_help("the commands are `exclude`, `off`, `on` and `off-line`"),
            ));
        }
        other => {
            let mut e = Diagnostic::error(COMMAND, format!("unknown coverage command `{cmd}`"))
                .with_primary(span(cmd_at, cmd.len()), "unknown command");
            if let Some(h) =
                yavs_sem::suggest::suggest(other, "command", COMMANDS.iter().map(|c| (*c, *c)))
            {
                e = e.with_help(h);
            }
            return Parsed::Err(Box::new(
                e.with_note("the commands are `exclude`, `off`, `on` and `off-line`"),
            ));
        }
    }
    cur.blank();
    let rule_at = cur.at;
    // An optional `condition` or `expression`.
    let mut kind = None;
    let save = (cur.s, cur.at);
    let w_at = cur.at;
    let w = cur.word();
    match w.to_ascii_lowercase().as_str() {
        "condition" => kind = Some(true),
        "expression" => kind = Some(false),
        "row" | "state" | "transition" => (cur.s, cur.at) = save,
        _ if !w.is_empty() && w.starts_with(|c: char| c.is_ascii_alphabetic()) => {
            // A word in front of `(`: a misspelt kind.
            let after = cur.s.trim_start();
            if after.starts_with('(') {
                let mut e = Diagnostic::error(SYNTAX, format!("unknown exclusion kind `{w}`"))
                    .with_primary(span(w_at, w.len()), "expected `condition` or `expression`");
                if let Some(h) = yavs_sem::suggest::suggest(
                    &w.to_ascii_lowercase(),
                    "kind",
                    [("condition", "condition"), ("expression", "expression")].into_iter(),
                ) {
                    e = e.with_help(h);
                }
                return Parsed::Err(Box::new(e));
            }
            (cur.s, cur.at) = save;
        }
        _ => (cur.s, cur.at) = save,
    }
    cur.blank();
    let spec_at = cur.at;
    let row_word = {
        let w = cur
            .s
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next();
        w.is_some_and(|w| w.eq_ignore_ascii_case("row"))
    };
    let machine_word = kind.is_none() && {
        let w = &cur.s[..cur
            .s
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(cur.s.len())];
        w.eq_ignore_ascii_case("state") || w.eq_ignore_ascii_case("transition")
    };
    let spec = if machine_word {
        match machine_spec(&mut cur, &span, &fail) {
            Ok(s) => s,
            Err(e) => return e,
        }
    } else if row_word {
        cur.advance(3);
        cur.blank();
        match row_spec(&mut cur, kind, spec_at, &span, &fail) {
            Ok(s) => s,
            Err(e) => return e,
        }
    } else if cur.s.starts_with('(') {
        let Some(close) = closing_paren(cur.s) else {
            return fail(
                SYNTAX,
                "missing `)` in the exclusion".into(),
                spec_at,
                cur.s.len(),
                "this `(` is never closed",
            );
        };
        let inner = &cur.s[1..close];
        let mut list = Vec::new();
        let mut off = spec_at + 1;
        if inner.trim().is_empty() {
            return fail(
                SYNTAX,
                "the exclusion names no input".into(),
                spec_at,
                close + 1,
                "expected `input=true` or `input=false`",
            );
        }
        for part in top_commas(inner) {
            let lead = part.len() - part.trim_start().len();
            let p = part.trim();
            let at = off + lead;
            off += part.len() + 1;
            let Some(eq) = value_sign(p) else {
                return fail(
                    SYNTAX,
                    format!("expected `input=value`, found `{p}`"),
                    at,
                    p.len(),
                    "missing `=`",
                );
            };
            let (name, value) = (p[..eq].trim(), p[eq + 1..].trim());
            if name.is_empty() {
                return fail(
                    SYNTAX,
                    "an exclusion needs an input before `=`".to_string(),
                    at,
                    p.len(),
                    "expected an input of the expression",
                );
            }
            let v = match value.to_ascii_lowercase().as_str() {
                "true" => true,
                "false" => false,
                other => {
                    let vat = at + eq + 1 + (p[eq + 1..].len() - p[eq + 1..].trim_start().len());
                    let mut e =
                        Diagnostic::error(VALUE, format!("invalid Boolean value `{value}`"))
                            .with_primary(
                                span(vat, value.len().max(1)),
                                "expected `true` or `false`",
                            );
                    if let Some(h) = yavs_sem::suggest::suggest(
                        other,
                        "value",
                        [("true", "true"), ("false", "false")].into_iter(),
                    ) {
                        e = e.with_help(h);
                    }
                    return Parsed::Err(Box::new(e));
                }
            };
            list.push((name.to_string(), span(at, name.len()), v));
        }
        let rule = format!("{}({})", kind_word(kind), inner.trim());
        cur.advance(close + 1);
        (Spec::Named(list), rule, None)
    } else {
        let tok = cur.pattern();
        if tok.is_empty() {
            return fail(
                SYNTAX,
                "the exclusion names no combination".into(),
                rule_at,
                text.len() - rule_at,
                "expected a pattern such as `01-`, or `(name=value, ...)`",
            );
        }
        let mut sym = Vec::new();
        for (i, c) in tok.bytes().enumerate() {
            if !matches!(c, b'0' | b'1' | b'-') {
                let shown = tok[i..].chars().next().unwrap_or('?');
                let word = tok.starts_with(|c: char| c.is_ascii_alphabetic());
                let near = yavs_diag::suggest::tail(tok, "word", SPEC_WORDS.into_iter());
                return fail(
                    if word { SYNTAX } else { PATTERN },
                    if word {
                        format!("expected a pattern or `(input=value, ...)`, found `{tok}`{near}")
                    } else {
                        format!("invalid pattern symbol `{shown}`")
                    },
                    spec_at + i,
                    shown.len_utf8(),
                    "expected `0`, `1` or `-`",
                );
            }
            sym.push(c);
        }
        let pattern = Spec::Pattern(sym, span(spec_at, tok.len()));
        cur.blank();
        let operands = match operand_list(&mut cur, &span, &fail) {
            Ok(l) => l,
            Err(e) => return e,
        };
        (
            pattern,
            format!("{}{tok}{}", kind_word(kind), written(&operands)),
            operands,
        )
    };
    cur.blank();
    let reason = if cur.s.is_empty() {
        String::new()
    } else if let Some(r) = cur.s.strip_prefix(':') {
        r.trim().to_string()
    } else {
        return fail(
            SYNTAX,
            format!("unexpected text `{}` after the exclusion", cur.s.trim_end()),
            cur.at,
            cur.s.trim_end().len(),
            "a reason follows `:`",
        );
    };
    Parsed::Ok(Box::new(Directive {
        at: all,
        kind,
        spec: spec.0,
        rule: spec.1,
        operands: spec.2,
        reason,
    }))
}

/// What a spec keeps of a list of inputs, for reports.
fn written(operands: &Operands) -> String {
    operands
        .as_ref()
        .map_or(String::new(), |(l, _)| format!(" ({})", l.join(", ")))
}

/// The list of inputs after a pattern, if the cursor is at one: `(a, b)`, operands in source order, each
/// any text. It cannot be a named form, which starts where the pattern would.
fn operand_list(
    cur: &mut Cursor<'_>,
    span: &dyn Fn(usize, usize) -> Span,
    fail: &dyn Fn(&'static str, String, usize, usize, &str) -> Parsed,
) -> Result<Operands, Parsed> {
    if !cur.s.starts_with('(') {
        return Ok(None);
    }
    let at = cur.at;
    let Some(close) = closing_paren(cur.s) else {
        return Err(fail(
            SYNTAX,
            "missing `)` in the list of inputs".into(),
            at,
            cur.s.len(),
            "this `(` is never closed",
        ));
    };
    let inner = &cur.s[1..close];
    let list: Vec<String> = top_commas(inner)
        .iter()
        .map(|p| p.trim().to_string())
        .collect();
    if list.iter().any(String::is_empty) {
        return Err(fail(
            SYNTAX,
            "the list of inputs has an empty entry".into(),
            at,
            close + 1,
            "expected the inputs in source order, such as `(a, count > 3)`",
        ));
    }
    cur.advance(close + 1);
    Ok(Some((list, span(at, close + 1))))
}

/// `state NAME` or `transition FROM -> TO`, the cursor at the word. It ends at the reason.
fn machine_spec(
    cur: &mut Cursor<'_>,
    span: &dyn Fn(usize, usize) -> Span,
    fail: &dyn Fn(&'static str, String, usize, usize, &str) -> Parsed,
) -> Result<(Spec, String, Operands), Parsed> {
    let word_at = cur.at;
    let word = cur.word();
    cur.blank();
    let name = |cur: &mut Cursor<'_>, what: &str, wild: bool| {
        let at = cur.at;
        let n = cur.state_name(wild);
        if n.is_empty() {
            let shown = cur.s.trim_end();
            return Err(fail(
                SYNTAX,
                format!("`{word}` needs {what}, found `{shown}`"),
                word_at,
                word.len(),
                "expected a name such as `idle`",
            ));
        }
        Ok((n.to_string(), span(at, n.len())))
    };
    if word.eq_ignore_ascii_case("state") {
        let (n, at) = name(cur, "the name of a state", false)?;
        let rule = format!("state {n}");
        return Ok((Spec::State(n, at), rule, None));
    }
    let from = name(cur, "a state before `->`", true)?;
    cur.blank();
    if !cur.s.starts_with("->") {
        let shown = cur.s.trim_end();
        return Err(fail(
            SYNTAX,
            format!("expected `->` between the states of a transition, found `{shown}`"),
            cur.at,
            shown.len(),
            "expected `->`",
        ));
    }
    cur.advance(2);
    cur.blank();
    let to = name(cur, "a state after `->`", true)?;
    let rule = format!("transition {} -> {}", from.0, to.0);
    Ok((Spec::Transition(from, to), rule, None))
}

/// What follows `row`: a pattern with one symbol that is not `-` and maybe its list of inputs, or
/// `INPUT = VALUE`. The cursor ends at the reason.
fn row_spec(
    cur: &mut Cursor<'_>,
    kind: Option<bool>,
    spec_at: usize,
    span: &dyn Fn(usize, usize) -> Span,
    fail: &dyn Fn(&'static str, String, usize, usize, &str) -> Parsed,
) -> Result<(Spec, String, Operands), Parsed> {
    let at = cur.at;
    let colon = outside_literals(cur.s)
        .iter()
        .find(|x| x.1 == ':' && x.2 == 0)
        .map(|x| x.0);
    let head = cur.s[..colon.unwrap_or(cur.s.len())].trim_end();
    if head.is_empty() {
        return Err(fail(
            SYNTAX,
            "`row` needs a pattern or an input and a value".into(),
            spec_at,
            3,
            "expected a pattern such as `-0-`, or `count > 3 = false`",
        ));
    }
    // A pattern alone or before its list: `row 0 = 1` and `row (a) = 1` are not.
    let n = head
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(head.len());
    let (tok, rest) = (&head[..n], head[n..].trim_start());
    if !tok.is_empty()
        && tok.bytes().all(|c| matches!(c, b'0' | b'1' | b'-'))
        && (rest.is_empty() || rest.starts_with('('))
    {
        let set = tok.bytes().filter(|c| *c != b'-').count();
        if set != 1 {
            let msg = format!(
                "a row is one input at one value, so its pattern has exactly one `0` or `1`, not {set}"
            );
            let Parsed::Err(e) = fail(PATTERN, msg, at, tok.len(), "not one row") else {
                unreachable!("fail makes an error");
            };
            return Err(Parsed::Err(Box::new(e.with_help(
                "a combination of inputs is written without `row`, such as `01-`",
            ))));
        }
        let pattern = Spec::RowPattern(tok.as_bytes().to_vec(), span(at, tok.len()));
        cur.advance(tok.len());
        cur.blank();
        let operands = operand_list(cur, span, fail)?;
        let rule = format!("{}row {tok}{}", kind_word(kind), written(&operands));
        return Ok((pattern, rule, operands));
    }
    let rule = format!("{}row {head}", kind_word(kind));
    let eq = value_sign(head);
    let spec = match eq {
        None if head.chars().any(char::is_whitespace) || head.starts_with(char::is_alphabetic) => {
            return Err(fail(
                SYNTAX,
                format!("expected `row INPUT = VALUE` or a pattern, found `{head}`"),
                at,
                head.len(),
                "missing `=`",
            ));
        }
        None => {
            if let Some((i, c)) = head
                .char_indices()
                .find(|x| !matches!(x.1, '0' | '1' | '-'))
            {
                return Err(fail(
                    PATTERN,
                    format!("invalid pattern symbol `{c}`"),
                    at + i,
                    c.len_utf8(),
                    "expected `0`, `1` or `-`",
                ));
            }
            unreachable!("a pattern of these symbols returned above")
        }
        Some(eq) => {
            let (name, value) = (head[..eq].trim(), head[eq + 1..].trim());
            if name.is_empty() {
                return Err(fail(
                    SYNTAX,
                    "an exclusion needs an input before `=`".into(),
                    at,
                    head.len(),
                    "expected an input of the expression",
                ));
            }
            let lower = value.to_ascii_lowercase();
            let v = match lower.as_str() {
                "1" | "true" => true,
                "0" | "false" => false,
                other => {
                    let vat =
                        at + eq + 1 + (head[eq + 1..].len() - head[eq + 1..].trim_start().len());
                    let Parsed::Err(e) = fail(
                        VALUE,
                        format!("invalid row value `{value}`"),
                        vat,
                        value.len().max(1),
                        "expected `0`, `1`, `false` or `true`",
                    ) else {
                        unreachable!("fail makes an error");
                    };
                    let near = yavs_sem::suggest::suggest(
                        other,
                        "value",
                        [("true", "true"), ("false", "false")].into_iter(),
                    );
                    return Err(Parsed::Err(Box::new(match near {
                        Some(h) => e.with_help(h),
                        None => *e,
                    })));
                }
            };
            Spec::RowNamed((name.to_string(), span(at, name.len()), v))
        }
    };
    cur.advance(head.len());
    Ok((spec, rule, None))
}

/// The byte offsets and characters of `s` that are outside string and character literals and
/// extended identifiers, with the depth of parentheses there.
fn outside_literals(s: &str) -> Vec<(usize, char, i32)> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut lit = None;
    let mut skip = 0;
    for (i, c) in s.char_indices() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        match lit {
            Some(q) if q == c => lit = None,
            Some(_) => {}
            None => match c {
                '"' | '\\' => lit = Some(c),
                '\'' if yavs_diag::operand::char_literal(s, i) => skip = 2,
                _ => {
                    depth -= i32::from(c == ')');
                    out.push((i, c, depth));
                    depth += i32::from(c == '(');
                }
            },
        }
    }
    out
}

/// The `)` that closes the `(` that `s` starts with.
fn closing_paren(s: &str) -> Option<usize> {
    let all = outside_literals(s);
    all.iter().find(|x| x.1 == ')' && x.2 == 0).map(|x| x.0)
}

/// `s` split at its commas outside parentheses and literals.
fn top_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut from = 0;
    for (i, c, depth) in outside_literals(s) {
        if c == ',' && depth == 0 {
            parts.push(&s[from..i]);
            from = i + 1;
        }
    }
    parts.push(&s[from..]);
    parts
}

/// The `=` of `input=value`: the last one outside parentheses and literals that is not part of `/=`,
/// `<=`, `>=` or `?=`, so an input may hold `=`.
fn value_sign(s: &str) -> Option<usize> {
    let all = outside_literals(s);
    let of_op = |i: usize| matches!(s[..i].chars().next_back(), Some('/' | '<' | '>' | '?'));
    all.iter()
        .rev()
        .find(|x| x.1 == '=' && x.2 == 0 && !of_op(x.0))
        .map(|x| x.0)
}

fn kind_word(kind: Option<bool>) -> &'static str {
    match kind {
        Some(true) => "condition ",
        Some(false) => "expression ",
        None => "",
    }
}

/// A position in a comment's text.
struct Cursor<'a> {
    s: &'a str,
    /// Byte offset of `s` in the comment.
    at: usize,
}

impl<'a> Cursor<'a> {
    fn advance(&mut self, n: usize) {
        self.s = &self.s[n..];
        self.at += n;
    }

    fn blank(&mut self) {
        let n = self.s.len() - self.s.trim_start().len();
        self.advance(n);
    }

    /// Letters, digits and underscores.
    fn word(&mut self) -> &'a str {
        let n = self
            .s
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(self.s.len());
        let w = &self.s[..n];
        self.advance(n);
        w
    }

    /// A command: a word that may hold `-` (`off-line`).
    fn command(&mut self) -> &'a str {
        let n = self
            .s
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(self.s.len());
        let w = &self.s[..n];
        self.advance(n);
        w
    }

    /// A state: an identifier, an extended identifier, a character literal, or with `wild` a `*`.
    /// Empty if none stands here.
    fn state_name(&mut self, wild: bool) -> &'a str {
        let s = self.s;
        let n = if wild && s.starts_with('*') {
            1
        } else if let Some(r) = s.strip_prefix('\\') {
            r.find('\\').map_or(0, |e| e + 2)
        } else if s.starts_with('\'') && yavs_diag::operand::char_literal(s, 0) {
            s.char_indices().nth(3).map_or(s.len(), |x| x.0)
        } else {
            s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(s.len())
        };
        self.advance(n);
        &s[..n]
    }

    /// Up to a blank or the `:` of the reason.
    fn pattern(&mut self) -> &'a str {
        let n = self
            .s
            .find(|c: char| c.is_whitespace() || c == ':' || c == '(')
            .unwrap_or(self.s.len());
        let w = &self.s[..n];
        self.advance(n);
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directive(text: &str) -> Directive {
        match parse(FileId(0), 0, text) {
            Parsed::Ok(d) => *d,
            Parsed::Err(d) => panic!("{text}: {}", d.message),
            Parsed::Other => panic!("{text}: not a directive"),
        }
    }

    fn error(text: &str) -> (String, String) {
        match parse(FileId(0), 0, text) {
            Parsed::Err(d) => (d.code.to_string(), d.message.clone()),
            Parsed::Ok(_) => panic!("{text}: accepted"),
            Parsed::Other => panic!("{text}: not a directive"),
        }
    }

    #[test]
    fn state_machine_forms() {
        let d = directive("-- yavs cov exclude state spare: never used");
        assert!(matches!(&d.spec, Spec::State(n, _) if n == "spare"));
        assert_eq!(
            (d.rule.as_str(), d.reason.as_str()),
            ("state spare", "never used")
        );
        for t in [
            "-- yavs cov exclude transition busy -> done : reset path",
            "-- yavs coverage exclude transition busy->done: reset path",
            "-- yavs cov exclude TRANSITION busy  ->done: reset path",
        ] {
            let d = directive(t);
            assert_eq!(d.rule, "transition busy -> done", "{t}");
            assert_eq!(d.reason, "reset path", "{t}");
        }
        assert_eq!(
            directive("-- yavs cov exclude transition spare -> *").rule,
            "transition spare -> *"
        );
        let d = directive("-- yavs cov exclude state ':': colon");
        assert_eq!((d.rule.as_str(), d.reason.as_str()), ("state ':'", "colon"));
        let d = directive(r"-- yavs cov exclude transition \a b\->'x': why");
        assert_eq!(d.rule, r"transition \a b\ -> 'x'");
        for (t, code) in [
            ("-- yavs cov exclude state", "E2020"),
            ("-- yavs cov exclude state : why", "E2020"),
            ("-- yavs cov exclude state a b", "E2020"),
            ("-- yavs cov exclude transition a", "E2020"),
            ("-- yavs cov exclude transition a b", "E2020"),
            ("-- yavs cov exclude transition a ->", "E2020"),
            ("-- yavs cov exclude condition state a", "E2020"),
            ("-- yavs cov exclude stat a", "E2020"),
        ] {
            assert_eq!(error(t).0, code, "{t}");
        }
        assert!(error("-- yavs cov exclude stat a").1.contains("state"));
    }

    #[test]
    fn forms_and_reasons() {
        let d = directive("-- yavs cov exclude 01-: impossible handshake");
        assert!(matches!(&d.spec, Spec::Pattern(p, _) if p == b"01-"));
        assert_eq!(d.reason, "impossible handshake");
        assert_eq!(d.kind, None);
        let d = directive("--yavs coverage exclude condition (valid=false, Ready=TRUE) : idle");
        assert_eq!(d.kind, Some(true));
        assert!(matches!(&d.spec, Spec::Named(n) if n.len() == 2 && n[1].0 == "Ready" && n[1].2));
        assert_eq!(d.reason, "idle");
        let d = directive("-- yavs cov exclude (reset=true)");
        assert_eq!(d.reason, "");
    }

    #[test]
    fn named_inputs_take_any_operand_text() {
        let names = |t: &str| match directive(t).spec {
            Spec::Named(n) => n
                .into_iter()
                .map(|x| (x.0, x.2))
                .collect::<Vec<(String, bool)>>(),
            _ => panic!("{t}"),
        };
        let one = |s: &str, v| vec![(s.to_string(), v)];
        assert_eq!(
            names("-- yavs cov exclude (count > 3 = false)"),
            one("count > 3", false)
        );
        assert_eq!(
            names("-- yavs cov exclude (a /= b = true)"),
            one("a /= b", true)
        );
        assert_eq!(
            names("-- yavs cov exclude (a <= b=false): why"),
            one("a <= b", false)
        );
        assert_eq!(
            names("-- yavs cov exclude (en = '1' = true)"),
            one("en = '1'", true)
        );
        assert_eq!(
            names("-- yavs cov exclude (c = ')' = true)"),
            one("c = ')'", true)
        );
        assert_eq!(
            names("-- yavs cov exclude (c = ',' = true)"),
            one("c = ','", true)
        );
        assert_eq!(
            names("-- yavs cov exclude (s = \"a,)\" = true)"),
            one("s = \"a,)\"", true)
        );
        assert_eq!(
            names("-- yavs cov exclude (f(a, b) = true)"),
            one("f(a, b)", true)
        );
        assert_eq!(
            names("-- yavs cov exclude ((a) = true, b'length > 1 = false)"),
            vec![
                ("(a)".to_string(), true),
                ("b'length > 1".to_string(), false)
            ]
        );
        let d = directive("-- yavs cov exclude (f(a, b) = true): the reason");
        assert_eq!(d.reason, "the reason");
        // No value sign of its own: `/=` and `<=` are not one.
        assert_eq!(error("-- yavs cov exclude (a /= b)").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude (a <= b)").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude (= true)").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude (a = b)").0, VALUE);
        assert_eq!(error("-- yavs cov exclude (f(a = true)").0, SYNTAX);
    }

    #[test]
    fn row_forms_and_errors() {
        let d = directive("-- yavs cov exclude row -0-: never");
        assert!(matches!(&d.spec, Spec::RowPattern(p, _) if p == b"-0-"));
        assert_eq!(d.rule, "row -0-");
        assert_eq!(d.reason, "never");
        let d = directive("-- yavs cov exclude condition ROW count < 15 = 1");
        assert!(matches!(&d.spec, Spec::RowNamed((n, _, true)) if n == "count < 15"));
        assert_eq!(d.kind, Some(true));
        let d = directive("-- yavs cov exclude row (a or b) = false : both low");
        assert!(matches!(&d.spec, Spec::RowNamed((n, _, false)) if n == "(a or b)"));
        assert_eq!(d.reason, "both low");
        let d = directive("-- yavs cov exclude row x = ':' = 0");
        assert!(matches!(&d.spec, Spec::RowNamed((n, _, false)) if n == "x = ':'"));
        // A pattern with several symbols stays what it was: a combination.
        assert!(matches!(
            directive("-- yavs cov exclude 0-").spec,
            Spec::Pattern(..)
        ));
        assert!(matches!(
            directive("-- yavs cov exclude (r=false)").spec,
            Spec::Named(..)
        ));
        assert_eq!(error("-- yavs cov exclude row").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude row : why").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude row 01").0, PATTERN);
        assert_eq!(error("-- yavs cov exclude row --").0, PATTERN);
        assert_eq!(error("-- yavs cov exclude row 02").0, PATTERN);
        assert_eq!(error("-- yavs cov exclude row a").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude row a b").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude row = 1").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude row a = 2").0, VALUE);
        let (c, m) = error("-- yavs cov exclude rwo -0-");
        assert_eq!(c, SYNTAX);
        assert!(m.contains("`row`"), "{m}");
    }

    #[test]
    fn a_list_of_inputs_after_a_pattern() {
        let list = |t: &str| {
            let d = directive(t);
            (d.rule, d.operands.map(|x| x.0), d.reason)
        };
        let v = |o: &[&str]| Some(o.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(
            list("-- yavs cov exclude 01 (en, count < 15): why"),
            (
                "01 (en, count < 15)".into(),
                v(&["en", "count < 15"]),
                "why".into()
            )
        );
        assert_eq!(
            list("-- yavs cov exclude row -0-(a, f(b, c), d = ',' )"),
            (
                "row -0- (a, f(b, c), d = ',')".into(),
                v(&["a", "f(b, c)", "d = ','"]),
                String::new()
            )
        );
        assert_eq!(
            list("-- yavs cov exclude condition 01 (a = b, c = '1')").1,
            v(&["a = b", "c = '1'"])
        );
        // The bare forms have none, and a named form stays one.
        assert_eq!(list("-- yavs cov exclude 01").1, None);
        assert_eq!(list("-- yavs cov exclude row -0-").1, None);
        assert!(matches!(
            directive("-- yavs cov exclude (a = b = true)").spec,
            Spec::Named(..)
        ));
        assert!(matches!(
            directive("-- yavs cov exclude row (a = b) = 1").spec,
            Spec::RowNamed(..)
        ));
        assert_eq!(error("-- yavs cov exclude 01 (a, b").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude 01 ()").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude 01 (a,,b)").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude row -0 (a, b) = 1").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude (a = true) (a)").0, SYNTAX);
    }

    #[test]
    fn prose_and_pragmas_are_not_directives() {
        for t in [
            "-- yavs is a simulator",
            "-- exclude 01",
            "-- yavs cover exclude 01",
            "-- yavs coverage off",
            "-- yavs cov on",
            "-- yavs cov off-line",
        ] {
            assert!(matches!(parse(FileId(0), 0, t), Parsed::Other), "{t}");
        }
    }

    #[test]
    fn malformed() {
        assert_eq!(error("-- yavs cov exclude 02").0, PATTERN);
        assert_eq!(error("-- yavs cov exclude (a=flase)").0, VALUE);
        assert_eq!(error("-- yavs cov exclude (a=true").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude (a)").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclude 01 because").0, SYNTAX);
        assert_eq!(error("-- yavs cov exclud 01").0, COMMAND);
        assert_eq!(error("-- yavs cov").0, COMMAND);
        assert_eq!(error("-- yavs cov exclude cndition (a=true)").0, SYNTAX);
    }
}
