//! The state machines of one architecture as coverage needs them (`--cov-kind=fsm`, #1020): which
//! signals hold a state, their states and the transitions the code names. Wider than `lint_710`
//! and `lint_711`, which judge a machine's whole type and need every assignment readable: a count
//! needs only the register. Rules stay with [`super::Arch::machines`]; the walk is the same one.

use std::collections::{BTreeSet, HashSet};

use yavs_diag::Span;
use yavs_sem::decl::{DeclKind, ObjClass, ObjRole};
use yavs_sem::hir::{Choice, ConstValue, ExprKind, StmtKind, UnitHir};
use yavs_sem::loader::LibSource;
use yavs_sem::types::TypeKind;
use yavs_sem::{Db, DeclId, ExprId, StmtId, TypeId, UnitId};

use super::{Conc, decl, deep, edge_in, exprs_of, flatten, name, span_text, values_of};

/// A signal of an enumeration type that a clocked process drives and a `case` selects on.
pub struct Machine {
    pub signal: DeclId,
    /// The literals of the type by position: name and declaration.
    pub states: Vec<(String, Span)>,
    /// The transitions a `case` alternative of the signal names, from one state to another, each
    /// once, at the first assignment that names it.
    pub arcs: Vec<(usize, usize, Span)>,
}

/// What looks like a state machine and is not one of [`Machine`]: the warning `W0116` and the
/// database's list of what was not measured.
pub struct NotMeasured {
    /// As written: `r.state`.
    pub what: String,
    pub span: Span,
    pub why: &'static str,
}

/// The enumeration type of `e`, when the user declared it: `bit`, `boolean` and `std_ulogic` hold
/// no states.
fn user_enum(db: &Db, sys: &HashSet<yavs_diag::FileId>, e: ExprId) -> Option<TypeId> {
    let t = db.base(db.exprs[e].subtype);
    let d = db.types[t].decl;
    (matches!(db.types[t].kind, TypeKind::Enum { .. }) && !sys.contains(&db.decls[d].span.file))
        .then_some(t)
}

/// The object at the root of the name `e`, and whether `e` is that object whole.
fn root_of(db: &Db, mut e: ExprId) -> Option<(DeclId, bool)> {
    let mut whole = true;
    loop {
        match &db.exprs[e].kind {
            ExprKind::Obj(d) => return Some((*d, whole)),
            ExprKind::Index { prefix, .. }
            | ExprKind::Slice { prefix, .. }
            | ExprKind::Field { prefix, .. } => {
                e = *prefix;
                whole = false;
            }
            _ => return None,
        }
    }
}

/// What the object `d` is, if it can hold a value a process keeps: (class, role).
fn object(db: &Db, d: DeclId) -> Option<(ObjClass, &ObjRole)> {
    match &db.decls[d].kind {
        DeclKind::Object(o) => Some((o.class, &o.role)),
        _ => None,
    }
}

/// The statements of a concurrent statement: a process's body, or the statement itself (an
/// assignment can be a register: `q <= d when rising_edge(clk)`).
fn body<'a>(c: &'a Conc<'a>) -> &'a [StmtId] {
    c.process.unwrap_or(std::slice::from_ref(&c.stmt))
}

fn drives(c: &Conc<'_>, d: DeclId) -> bool {
    c.drivers.iter().any(|l| decl(l.root) == Some(d))
}

/// The state machines of architecture `u`, and what looks like one but is not measured. The files
/// of the libraries yavs provides hold no user type.
pub fn machines(db: &Db, u: UnitId) -> (Vec<Machine>, Vec<NotMeasured>) {
    let unit = &db.units[u];
    let UnitHir::Architecture { stmts, .. } = &unit.hir else {
        return Default::default();
    };
    let (mut conc, mut scopes) = (Vec::new(), vec![unit.scope]);
    flatten(db, stmts, &[], &mut conc, &mut scopes);
    let sys: HashSet<_> = db
        .units
        .iter()
        .filter(|(_, u)| matches!(db.libs[u.lib].source, LibSource::StdDir(_)))
        .map(|(_, u)| u.file)
        .collect();
    let clocked: Vec<&Conc<'_>> = conc
        .iter()
        .filter(|c| {
            exprs_of(db, body(c))
                .iter()
                .any(|(e, _)| edge_in(db, *e).is_some())
        })
        .collect();
    let registered = |d: DeclId| clocked.iter().any(|c| drives(c, d));
    // Every `case` of a process, with whether that process is clocked.
    let mut cases: Vec<(StmtId, bool)> = Vec::new();
    for c in &conc {
        let Some(b) = c.process else { continue };
        let mut all = Vec::new();
        deep(db, b, &mut all);
        let is_clocked = clocked.iter().any(|x| x.stmt == c.stmt);
        cases.extend(
            all.into_iter()
                .filter(|&s| matches!(db.stmts[s].kind, StmtKind::Case { .. }))
                .map(|s| (s, is_clocked)),
        );
    }
    let selector = |s: StmtId| match &db.stmts[s].kind {
        StmtKind::Case { expr, .. } => Some(*expr),
        _ => None,
    };

    let (mut found, mut missed) = (Vec::new(), Vec::new());
    let mut selected: HashSet<DeclId> = HashSet::new();
    for &(s, is_clocked) in &cases {
        let Some(e) = selector(s) else { continue };
        if user_enum(db, &sys, e).is_none() {
            continue;
        }
        let Some((d, whole)) = root_of(db, e) else {
            continue;
        };
        let why = match object(db, d) {
            Some((ObjClass::Signal, ObjRole::Plain)) if whole && registered(d) => {
                selected.insert(d);
                continue;
            }
            Some((ObjClass::Signal, ObjRole::Plain)) if !whole && registered(d) => {
                "it is a part of a signal, not a whole one"
            }
            Some((ObjClass::Signal, ObjRole::Port)) if registered(d) => "it is a port",
            Some((ObjClass::Variable, _)) if is_clocked => "it is a variable",
            _ => continue,
        };
        missed.push(NotMeasured {
            what: span_text(db, db.exprs[e].span),
            span: db.exprs[e].span,
            why,
        });
    }
    for &sc in &scopes {
        for &d in &db.scopes[sc].order {
            let DeclKind::Object(o) = &db.decls[d].kind else {
                continue;
            };
            if o.class != ObjClass::Signal || o.role != ObjRole::Plain {
                continue;
            }
            let t = db.subtypes[o.subtype].base;
            let (TypeKind::Enum { lits }, false) = (
                &db.types[t].kind,
                sys.contains(&db.decls[db.types[t].decl].span.file),
            ) else {
                continue;
            };
            if selected.contains(&d) {
                found.push(Machine {
                    signal: d,
                    states: lits
                        .iter()
                        .map(|&l| (name(db, l), db.decls[l].name_span()))
                        .collect(),
                    arcs: arcs(db, d, lits.len(), &clocked, &cases),
                });
            } else if clocked
                .iter()
                .any(|c| drives(c, d) && c.reads.iter().any(|l| decl(l.root) == Some(d)))
            {
                // A register that depends on itself, with no `case`: `if state = a then`.
                missed.push(NotMeasured {
                    what: name(db, d),
                    span: db.decls[d].name_span(),
                    why: "no `case` selects on it",
                });
            }
        }
    }
    missed.sort_by_key(|m| (m.span.file, m.span.start));
    missed.dedup_by(|a, b| a.what == b.what && a.why == b.why);
    (found, missed)
}

/// The transitions of signal `s` that its `case` alternatives name. An assignment to `s` counts,
/// and one to a signal a clocked process copies into `s` (the next state of two processes). A
/// transition to the same state is left out: holding a state changes nothing to observe.
fn arcs(
    db: &Db,
    s: DeclId,
    n: usize,
    clocked: &[&Conc<'_>],
    cases: &[(StmtId, bool)],
) -> Vec<(usize, usize, Span)> {
    let obj = |e: ExprId| match db.exprs[e].kind {
        ExprKind::Obj(d) => Some(d),
        _ => None,
    };
    let mut targets: BTreeSet<DeclId> = BTreeSet::from([s]);
    for c in clocked.iter().filter(|c| drives(c, s)) {
        let mut all = Vec::new();
        deep(db, body(c), &mut all);
        for x in all {
            if let Some((t, values)) = values_of(db, x)
                && obj(t) == Some(s)
            {
                targets.extend(values.into_iter().filter_map(obj));
            }
        }
    }
    let mut out: Vec<(usize, usize, Span)> = Vec::new();
    for &(c, _) in cases {
        let StmtKind::Case { expr, alts, .. } = &db.stmts[c].kind else {
            continue;
        };
        if obj(*expr) != Some(s) {
            continue;
        }
        let named = |set: &yavs_sem::hir::ChoiceSet| -> Vec<usize> {
            set.intervals
                .iter()
                .flatten()
                .flat_map(|&(a, b)| a..=b)
                .map(|p| p as usize)
                .collect()
        };
        let others = |set: &yavs_sem::hir::ChoiceSet| {
            set.choices.iter().any(|c| matches!(c, Choice::Others))
        };
        let plain: BTreeSet<usize> = alts
            .iter()
            .filter(|(set, _)| !others(set))
            .flat_map(|(set, _)| named(set))
            .collect();
        for (set, list) in alts {
            let from: Vec<usize> = if others(set) {
                (0..n).filter(|p| !plain.contains(p)).collect()
            } else {
                named(set)
            };
            let mut all = Vec::new();
            deep(db, list, &mut all);
            for x in all {
                let Some((t, values)) = values_of(db, x) else {
                    continue;
                };
                if !obj(t).is_some_and(|d| targets.contains(&d)) {
                    continue;
                }
                for v in values {
                    let Some(ConstValue::Enum(to)) = db.value(v) else {
                        continue;
                    };
                    let to = *to as usize;
                    for &f in from.iter().filter(|&&f| f != to) {
                        out.push((f, to, db.stmts[x].span));
                    }
                }
            }
        }
    }
    out.sort_by_key(|a| (a.0, a.1, a.2.start));
    out.dedup_by_key(|a| (a.0, a.1));
    out
}
