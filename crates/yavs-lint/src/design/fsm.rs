//! The state machines of one architecture, one reading for `lint_710`, `lint_711`, the design
//! facts and `--cov-kind=fsm` (#1020, maintainer 2026-11-01). A machine is a signal of an enumeration
//! that a clocked statement drives and a `case` selects on. The rules also need it `readable`: every
//! assignment says which state it gives, so what no assignment names is known not to happen.

use std::collections::{BTreeSet, HashSet};

use serde_json::{Value, json};
use yavs_diag::Span;
use yavs_sem::decl::{DeclKind, ObjClass, ObjRole};
use yavs_sem::hir::{Choice, ChoiceSet, ConstValue, ExprKind, StmtKind, UnitHir};
use yavs_sem::loader::LibSource;
use yavs_sem::types::{Constraint, RangeC, TypeKind};
use yavs_sem::{Db, DeclId, ExprId, StmtId, TypeId, UnitId};

use super::{Arch, Conc, decl, deep, edge_in, exprs_of, flatten, name, span_text, values_of};

/// A signal of an enumeration type that a clocked statement drives and a `case` selects on.
pub struct Machine {
    pub signal: DeclId,
    pub ty: TypeId,
    /// The register, then the signals a clocked statement copies into it (a next state).
    pub targets: Vec<DeclId>,
    /// The literals of the type by position: name and declaration.
    pub states: Vec<(String, Span)>,
    /// The transitions a `case` alternative of the register names, a state to itself included,
    /// each once, at the first assignment that names it.
    pub arcs: Vec<(usize, usize, Span)>,
    /// Every assignment to `targets` gives a literal or copies another of them, and nothing else
    /// can drive them. Without it the arcs may be incomplete and the rules say nothing.
    pub readable: bool,
    /// States of its subtype no assignment or initial value gives (`lint_710`).
    pub never_entered: Vec<usize>,
    /// States whose alternative assigns no other state (`lint_711`).
    pub no_exit: Vec<(usize, Span)>,
    /// The register's initial value, when it folds.
    pub reset: Option<usize>,
}

impl Machine {
    /// The transitions that are coverage items: what a `case` names, staying in a state left out.
    pub fn transitions(&self) -> impl Iterator<Item = (usize, usize, Span)> + '_ {
        self.arcs.iter().copied().filter(|a| a.0 != a.1)
    }
}

/// What looks like a state machine and is not one of [`Machine`]: the warning `W0120` and the
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

fn obj(db: &Db, e: ExprId) -> Option<DeclId> {
    match db.exprs[e].kind {
        ExprKind::Obj(d) => Some(d),
        _ => None,
    }
}

fn pos(db: &Db, e: ExprId) -> Option<usize> {
    match db.value(e) {
        Some(ConstValue::Enum(p)) => Some(*p as usize),
        _ => None,
    }
}

/// One alternative of a `case` on a register.
struct Alt {
    /// The states it covers, `others` as the ones no other alternative names.
    from: Vec<usize>,
    /// It names one state and nothing else.
    single: bool,
    /// The literals assigned to a target below it, and where.
    to: Vec<(usize, Span)>,
    span: Span,
}

/// The literals assigned to `targets` below `s`, and where.
fn literals(db: &Db, s: StmtId, targets: &[DeclId]) -> Vec<(usize, Span)> {
    let mut all = Vec::new();
    deep(db, std::slice::from_ref(&s), &mut all);
    let mut out = Vec::new();
    for x in all {
        if let Some((t, values)) = values_of(db, x)
            && obj(db, t).is_some_and(|d| targets.contains(&d))
        {
            out.extend(
                values
                    .into_iter()
                    .filter_map(|v| pos(db, v))
                    .map(|p| (p, db.stmts[x].span)),
            );
        }
    }
    out
}

/// Calls `f` with the alternatives of each `case` on `signal` below `list`, and whether a literal
/// was assigned to a target before it (a default: the machine then leaves every state by it). A
/// default that holds the state (`next <= state`) gives no literal and is no exit.
fn cases(
    db: &Db,
    list: &[StmtId],
    mut defaulted: bool,
    (signal, targets, n): (DeclId, &[DeclId], usize),
    f: &mut dyn FnMut(&[Alt], bool),
) {
    let at = (signal, targets, n);
    for &s in list {
        match &db.stmts[s].kind {
            StmtKind::Case { expr, alts, .. } if obj(db, *expr) == Some(signal) => {
                let others =
                    |set: &ChoiceSet| set.choices.iter().any(|c| matches!(c, Choice::Others));
                let named = |set: &ChoiceSet| -> Vec<usize> {
                    let i = set.intervals.iter().flatten();
                    i.flat_map(|&(a, b)| a..=b).map(|p| p as usize).collect()
                };
                let plain: BTreeSet<usize> = alts
                    .iter()
                    .filter(|(set, _)| !others(set))
                    .flat_map(|(set, _)| named(set))
                    .collect();
                let rows: Vec<Alt> = alts
                    .iter()
                    .map(|(set, stmts)| {
                        let from = if others(set) {
                            (0..n).filter(|p| !plain.contains(p)).collect()
                        } else {
                            named(set)
                        };
                        Alt {
                            single: matches!(set.intervals.as_deref(), Some([(a, b)]) if a == b),
                            from,
                            to: stmts
                                .iter()
                                .flat_map(|&b| literals(db, b, targets))
                                .collect(),
                            span: stmts
                                .first()
                                .map_or(db.stmts[s].span, |&b| db.stmts[b].span),
                        }
                    })
                    .collect();
                f(&rows, defaulted);
                defaulted |= !literals(db, s, targets).is_empty();
            }
            StmtKind::If { branches, else_ } => {
                for (_, b) in branches {
                    cases(db, b, defaulted, at, f);
                }
                if let Some(e) = else_ {
                    cases(db, e, defaulted, at, f);
                }
            }
            StmtKind::Loop { body, .. } | StmtKind::SeqBlock { body, .. } => {
                cases(db, body, defaulted, at, f);
            }
            StmtKind::Case { alts, .. } => {
                for (_, b) in alts {
                    cases(db, b, defaulted, at, f);
                }
            }
            _ => defaulted |= !literals(db, s, targets).is_empty(),
        }
    }
}

/// The positions the subtype of an object allows, of `n`: `None` when its range does not fold.
fn allowed(db: &Db, d: DeclId, n: usize) -> Option<std::ops::RangeInclusive<usize>> {
    let DeclKind::Object(o) = &db.decls[d].kind else {
        return None;
    };
    match &db.subtypes[o.subtype].constraint {
        Constraint::None => Some(0..=n.saturating_sub(1)),
        Constraint::Range(RangeC::Explicit { left, right, .. }) => {
            let (a, b) = (pos(db, *left)?, pos(db, *right)?);
            Some(a.min(b)..=a.max(b))
        }
        _ => None,
    }
}

/// The state machines of architecture `u`, and what looks like one but is not measured.
pub fn machines(db: &Db, u: UnitId) -> (Vec<Machine>, Vec<NotMeasured>) {
    let unit = &db.units[u];
    let UnitHir::Architecture { stmts, .. } = &unit.hir else {
        return Default::default();
    };
    let (mut conc, mut scopes) = (Vec::new(), vec![unit.scope]);
    flatten(db, stmts, &[], &mut conc, &mut scopes);
    let (mut out, mut related) = (Vec::new(), Default::default());
    let a = Arch {
        db,
        stmts,
        conc,
        scopes,
        out: &mut out,
        related: &mut related,
    };
    a.read_machines()
}

impl Arch<'_> {
    /// `lint_710` and `lint_711` on the machines of this architecture, and their design facts.
    pub(super) fn machines(&mut self) -> Value {
        let db = self.db;
        let (found, _) = self.read_machines();
        let mut facts = Vec::new();
        for m in &found {
            let ty = name(db, db.types[m.ty].decl);
            let here = self.scopes.contains(&db.decls[db.types[m.ty].decl].scope);
            let register = name(db, m.signal);
            for &s in &m.never_entered {
                let (state, at) = &m.states[s];
                // A type of a package is not this machine's alone: the register says it.
                let span = if here {
                    *at
                } else {
                    db.decls[m.signal].name_span()
                };
                self.push(
                    "lint_710",
                    span,
                    format!("State `{state}` of `{ty}` is never entered: nothing assigns it to `{register}`"),
                );
            }
            for &(s, span) in &m.no_exit {
                let state = &m.states[s].0;
                self.push(
                    "lint_711",
                    span,
                    format!("State `{state}` of `{ty}` has no exit: its alternative never assigns another state"),
                );
            }
            facts.push(json!({
                "type": ty,
                "register": register,
                "signals": m.targets.iter().map(|&d| name(db, d)).collect::<Vec<_>>(),
                "states": m.states.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
                "transitions": m.arcs.iter().map(|a| json!({"from": m.states[a.0].0, "to": m.states[a.1].0})).collect::<Vec<_>>(),
                "readable": m.readable,
                "reset_state": m.reset.map(|p| m.states[p].0.as_str()),
            }));
        }
        Value::Array(facts)
    }

    fn read_machines(&self) -> (Vec<Machine>, Vec<NotMeasured>) {
        let db = self.db;
        let conc = &self.conc;
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
        // Every statement, and every `case` of a process with whether that process is clocked.
        let mut all: Vec<StmtId> = Vec::new();
        let mut selectors: Vec<(ExprId, bool)> = Vec::new();
        for c in conc {
            all.push(c.stmt);
            let Some(b) = c.process else { continue };
            let mut below = Vec::new();
            deep(db, b, &mut below);
            let is_clocked = clocked.iter().any(|x| x.stmt == c.stmt);
            selectors.extend(below.iter().filter_map(|&s| match &db.stmts[s].kind {
                StmtKind::Case { expr, .. } => Some((*expr, is_clocked)),
                _ => None,
            }));
            all.extend(below);
        }

        let mut missed = Vec::new();
        let mut selected: HashSet<DeclId> = HashSet::new();
        for &(e, is_clocked) in &selectors {
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

        let named = self.named_in_subprograms();
        let mut found = Vec::new();
        for &sc in &self.scopes {
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
                    found.push(self.machine(d, t, lits, &clocked, &all, &named));
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

    /// The machine of register `s` of type `t`.
    fn machine(
        &self,
        s: DeclId,
        t: TypeId,
        lits: &[DeclId],
        clocked: &[&Conc<'_>],
        all: &[StmtId],
        named: &HashSet<DeclId>,
    ) -> Machine {
        let db = self.db;
        let n = lits.len();
        // The signals of the type that a clocked statement copies into the register.
        let mut targets = vec![s];
        for c in clocked.iter().filter(|c| drives(c, s)) {
            let mut below = Vec::new();
            deep(db, body(c), &mut below);
            for x in below {
                if let Some((to, values)) = values_of(db, x)
                    && obj(db, to) == Some(s)
                {
                    for v in values.into_iter().filter_map(|v| obj(db, v)) {
                        let same = matches!(
                            object(db, v),
                            Some((ObjClass::Signal, ObjRole::Plain))
                        ) && matches!(&db.decls[v].kind, DeclKind::Object(o) if db.subtypes[o.subtype].base == t);
                        if same && !targets.contains(&v) {
                            targets.push(v);
                        }
                    }
                }
            }
        }
        let initial = |d: DeclId| match &db.decls[d].kind {
            DeclKind::Object(o) => o.default,
            _ => None,
        };
        // Entered by an initial value, or by default the first literal of the type.
        let mut entered: HashSet<usize> = HashSet::new();
        for &d in &targets {
            match initial(d) {
                Some(e) => entered.extend(pos(db, e)),
                None => {
                    entered.insert(0);
                }
            }
        }
        let mut readable = !targets.iter().any(|d| named.contains(d));
        for &x in all {
            if let Some((to, values)) = values_of(db, x)
                && obj(db, to).is_some_and(|d| targets.contains(&d))
            {
                for v in values {
                    match pos(db, v) {
                        Some(p) => {
                            entered.insert(p);
                        }
                        None if obj(db, v).is_some_and(|d| targets.contains(&d)) => {}
                        None => readable = false,
                    }
                }
            }
            // A driver that is not an assignment (a parameter) says nothing readable.
            if matches!(
                db.stmts[x].kind,
                StmtKind::ProcCall(_) | StmtKind::ConcProcCall { .. }
            ) {
                let mut d = Vec::new();
                db.collect_drivers(&[x], &mut d);
                readable &= !d
                    .iter()
                    .any(|l| decl(l.root).is_some_and(|d| targets.contains(&d)));
            }
        }
        // A port map says nothing readable either.
        readable &= !self.conc.iter().any(|c| {
            matches!(db.stmts[c.stmt].kind, StmtKind::Instance { .. })
                && targets.iter().any(|&d| drives(c, d))
        });

        let (mut arcs, mut no_exit) = (Vec::new(), Vec::new());
        for c in &self.conc {
            let Some(b) = c.process else { continue };
            cases(db, b, false, (s, &targets, n), &mut |alts, defaulted| {
                for alt in alts {
                    for &(to, at) in &alt.to {
                        arcs.extend(alt.from.iter().map(|&f| (f, to, at)));
                    }
                }
                // A `case` that assigns no state is a multiplexer; after a default, that is the exit.
                if defaulted || alts.iter().all(|a| a.to.is_empty()) {
                    return;
                }
                for alt in alts.iter().filter(|a| a.single) {
                    if let [state] = alt.from[..]
                        && alt.to.iter().all(|&(to, _)| to == state)
                    {
                        no_exit.push((state, alt.span));
                    }
                }
            });
        }
        arcs.sort_by_key(|a| (a.0, a.1, a.2.start));
        arcs.dedup_by_key(|a| (a.0, a.1));
        let in_subtype = allowed(db, s, n);
        let never_entered = match (readable, in_subtype) {
            (true, Some(r)) => (0..n)
                .filter(|p| r.contains(p) && !entered.contains(p))
                .collect(),
            _ => Vec::new(),
        };
        Machine {
            signal: s,
            ty: t,
            states: lits
                .iter()
                .map(|&l| (name(db, l), db.decls[l].name_span()))
                .collect(),
            arcs,
            readable,
            never_entered,
            no_exit: if readable { no_exit } else { Vec::new() },
            reset: initial(s).and_then(|e| pos(db, e)),
            targets,
        }
    }
}
