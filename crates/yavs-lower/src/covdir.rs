//! In-source coverage directives (`-- yavs coverage exclude ...`, docs/coverage.md "Excluding
//! combinations in the source", #883): parsed from the comments the lexer finds, bound to the
//! decision or assignment right after them, and resolved to the combinations they waive.
//!
//! [`check`] validates every directive of a design's files at analysis, whether or not coverage
//! is collected; [`resolve`] gives the lowering the waivers of one file. Both read the final
//! HIR, so a warm analysis cache changes nothing, and use the one decision reading of
//! `covcond.rs`, so a directive and the counters it waives agree on the inputs.

use rustc_hash::FxHashMap as HashMap;
use std::collections::BTreeSet;

use yavs_diag::{Diagnostic, FileId, Span};
use yavs_ir::CovWaiver;
use yavs_sem::hir::{Rhs, StmtKind, Waveform};
use yavs_sem::loader::LibSource;
use yavs_sem::{Db, ExprId};

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

/// The waivers of one file by decision, and the diagnostics of its directives.
#[derive(Default)]
pub struct Checked {
    pub waivers: HashMap<ExprId, Vec<CovWaiver>>,
    pub diags: Vec<Diagnostic>,
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
}

/// A parsed `exclude` directive.
struct Directive {
    /// The comment.
    at: Span,
    /// `Some(true)` for `condition`, `Some(false)` for `expression`.
    kind: Option<bool>,
    spec: Spec,
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
                Some((next, v)) if *next == c.next && c.next.is_some() => v.push(d),
                _ => groups.push((c.next, vec![d])),
            },
        }
    }
    if groups.is_empty() {
        return out;
    }
    let sites = sites(db, file);
    let at = |d: &Diagnostic| {
        d.primary
            .as_ref()
            .map(|p| (p.span.file, p.span.start, p.span.end))
    };
    let mut seen = BTreeSet::new();
    for (next, ds) in groups {
        let targets = next.and_then(|n| sites.get(&n));
        for d in &ds {
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
    out.diags.sort_by_key(at);
    out.diags
        .dedup_by(|a, b| a.code == b.code && a.message == b.message && at(a) == at(b));
    out
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
    let (mut care, mut want) = (0u32, 0u32);
    match &d.spec {
        Spec::Pattern(p, at) => {
            if p.len() != n {
                return vec![
                    err(
                        PATTERN,
                        &format!(
                            "exclusion has {} position{}, but the expression has {n} inputs",
                            p.len(),
                            if p.len() == 1 { "" } else { "s" }
                        ),
                        "wrong number of positions",
                    )
                    .with_secondary(whole, "the expression")
                    .with_note(format!("expected order: {}", order()))
                    .with_help("provide one symbol per input; use `-` for an unrestricted input")
                    .with_secondary(*at, "the pattern"),
                ];
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
                let key = name.to_ascii_lowercase();
                let hits: Vec<usize> = (0..n)
                    .filter(|&i| {
                        names[i].as_ref().is_some_and(|x| {
                            if name.starts_with('\\') {
                                x == name
                            } else {
                                x.to_ascii_lowercase() == key
                            }
                        })
                    })
                    .collect();
                let i = match hits[..] {
                    [i] => i,
                    [] => {
                        let mut e = Diagnostic::error(
                            OPERAND,
                            format!("`{name}` is not an input of this expression"),
                        )
                        .with_primary(*at, "unknown input")
                        .with_secondary(whole, "the expression")
                        .with_note(format!("inputs, in order: {}", order()));
                        let cands: Vec<(String, &str)> = names
                            .iter()
                            .flatten()
                            .map(|x| (x.to_ascii_lowercase(), x.as_str()))
                            .collect();
                        if let Some(h) = yavs_sem::suggest::suggest(
                            &key,
                            "input",
                            cands.iter().map(|(k, x)| (k.as_str(), *x)),
                        ) {
                            e = e.with_help(h);
                        } else {
                            e = e.with_help(
                                "an input that is not a plain name is addressed by its position: \
                                 write the pattern form, such as `01-`",
                            );
                        }
                        return vec![e];
                    }
                    _ => {
                        return vec![
                            Diagnostic::error(
                                OPERAND,
                                format!(
                                    "`{name}` is {} inputs of this expression",
                                    hits.len()
                                ),
                            )
                            .with_primary(*at, "ambiguous input")
                            .with_secondary(whole, "the expression")
                            .with_note(format!("inputs, in order: {}", order()))
                            .with_help(
                                "address the inputs by position with the pattern form, such as `01-`",
                            ),
                        ];
                    }
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
        rule: d.rule.clone(),
        reason: d.reason.clone(),
        at: d.at,
    });
    diags
}

/// Warnings about the waivers of one decision: duplicates, subsumed ones, all combinations.
fn redundant(db: &Db, expr: ExprId, v: &[CovWaiver], diags: &mut Vec<Diagnostic>) {
    let covers =
        |a: &CovWaiver, b: &CovWaiver| a.care & b.care == a.care && b.want & a.care == a.want;
    for j in 0..v.len() {
        for i in 0..v.len() {
            let dup = v[i].care == v[j].care && v[i].want == v[j].want;
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
            .all(|&s| v.iter().any(|w| s & w.care == w.want))
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
    Ok(Directive),
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
    let spec = if cur.s.starts_with('(') {
        let Some(close) = cur.s.find(')') else {
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
                "expected `name=true` or `name=false`",
            );
        }
        for part in inner.split(',') {
            let lead = part.len() - part.trim_start().len();
            let p = part.trim();
            let at = off + lead;
            off += part.len() + 1;
            let Some((name, value)) = p.split_once('=') else {
                return fail(
                    SYNTAX,
                    format!("expected `name=value`, found `{p}`"),
                    at,
                    p.len(),
                    "missing `=`",
                );
            };
            let (name, value) = (name.trim(), value.trim());
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
            {
                return fail(
                    SYNTAX,
                    format!("`{name}` is not an input name"),
                    at,
                    name.len(),
                    "expected an identifier",
                );
            }
            let v = match value.to_ascii_lowercase().as_str() {
                "true" => true,
                "false" => false,
                other => {
                    let vat = at
                        + p.find('=').map_or(0, |i| i + 1)
                        + (p[p.find('=').map_or(0, |i| i + 1)..].len()
                            - p[p.find('=').map_or(0, |i| i + 1)..].trim_start().len());
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
        (Spec::Named(list), rule)
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
                return fail(
                    if word { SYNTAX } else { PATTERN },
                    if word {
                        format!("expected a pattern or `(name=value, ...)`, found `{tok}`")
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
        let rule = format!("{}{tok}", kind_word(kind));
        (Spec::Pattern(sym, span(spec_at, tok.len())), rule)
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
    Parsed::Ok(Directive {
        at: all,
        kind,
        spec: spec.0,
        rule: spec.1,
        reason,
    })
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

    /// Up to a blank or the `:` of the reason.
    fn pattern(&mut self) -> &'a str {
        let n = self
            .s
            .find(|c: char| c.is_whitespace() || c == ':')
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
            Parsed::Ok(d) => d,
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
