//! State machine coverage (`--cov-kind=fsm`, #1020): an observer on the kernel's value-change bus
//! that counts, per instance, each state a machine's register reached and each change from one
//! state to another. It reads the register's value and nothing else, so it counts the same in
//! every tier and at any `--threads`, and it exists only with this kind on. Which signals are
//! machines is `yavs_lint::machines`.
//!
//! Items are keyed by the path of the scope declaring the register and `SIGNAL:STATE` or
//! `SIGNAL:FROM->TO` (`Key::sig`): names, no hash and no offsets, so an edit keeps them.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use yavs_elab::covdir::{MachineItem, MachineWaiver};
use yavs_elab::{Case, Elaborated, ObjKind, ScopeKind, Shape};
use yavs_kernel::{Hook, Observer, Phase, SignalId};
use yavs_lint::Machine;
use yavs_sem::Db;
use yavs_sem::hir::{ExprKind, StmtKind};

use crate::{Database, Entry, Key, Kind, Waiver};

/// A state or transition of one machine.
struct Item {
    /// Where it is reported.
    place: (String, u32),
    count: u64,
    /// In a `-- coverage off` region: the database does not list it.
    off: bool,
}

/// One machine in one instance.
struct Watch {
    path: String,
    /// `library.entity` of its instance.
    unit: String,
    /// The label of the process holding its `case`; empty if it has none.
    process: String,
    /// The register's name, as declared.
    sig: String,
    /// Kernel element of the register.
    at: usize,
    /// Declaration of the register: where a transition no `case` names is shown.
    place: (String, u32),
    /// By position: name, then where declared and times entered.
    states: Vec<(String, Item)>,
    /// By (from, to): where named, times taken. Every transition a `case` names, count 0 included;
    /// one that happens otherwise (a reset) comes in when it does.
    arcs: BTreeMap<(usize, usize), Item>,
    /// The `-- yavs cov exclude` directives on it.
    waivers: Vec<(MachineItem, Waiver)>,
    last: Option<usize>,
}

/// How the run reads coverage comments.
#[derive(Clone, Copy)]
pub struct Reading {
    /// `--cov-exclude-translate-off`.
    pub translate_off: bool,
    /// `--coverage-directives=apply`.
    pub directives: bool,
}

struct State {
    /// Per kernel signal: the watches on it.
    by_signal: Vec<Vec<u32>>,
    watches: Vec<Watch>,
}

/// The observer. Clone it into `SimControl::subscribe`, keep one for [`Fsm::add_to`].
#[derive(Clone)]
pub struct Fsm(Arc<Mutex<State>>);

/// The position of element `at` of a byte-stored enumeration signal.
fn position(hook: &Hook<'_>, sig: u32, at: usize) -> Option<usize> {
    let v = hook.view(SignalId(sig)).ok()?;
    let bytes = v.bytes()?;
    let w = bytes.len() / v.len().max(1);
    let mut raw = [0u8; 8];
    let cell = bytes.get(at * w..(at + 1) * w)?;
    raw.get_mut(..cell.len())?.copy_from_slice(cell);
    Some(u64::from_le_bytes(raw) as usize)
}

/// What the architectures of `e` hold: the observer over their machines in every instance, and
/// a line each for what looks like a machine but is not measured.
///
/// A machine whose register or every `case` is in a `-- coverage off` region is not watched; a
/// state or transition whose own line is in one is not listed. The third result holds the warnings
/// about the regions when nothing else reads them (no counters: [`crate::collect`] stops early).
pub fn fsm(
    db: &Db,
    e: &Elaborated,
    read: Reading,
) -> (
    Fsm,
    Vec<(yavs_diag::Span, String)>,
    Vec<yavs_diag::Diagnostic>,
) {
    let mut archs: Vec<yavs_sem::UnitId> = e
        .hier
        .scopes
        .iter()
        .filter_map(|s| match s.kind {
            ScopeKind::Instance { arch, .. } => Some(arch),
            _ => None,
        })
        .collect();
    archs.sort();
    archs.dedup();
    let mut machines: HashMap<yavs_sem::DeclId, Machine> = HashMap::new();
    let mut notes = Vec::new();
    let files: HashSet<yavs_diag::FileId> = archs.iter().map(|&u| db.units[u].file).collect();
    let mut waivers: HashMap<yavs_sem::DeclId, Vec<MachineWaiver>> = HashMap::new();
    if read.directives {
        for &f in &files {
            waivers.extend(yavs_elab::covdir::resolve(db, f).machines);
        }
    }
    for u in archs {
        let (found, missed) = yavs_lint::machines(db, u);
        machines.extend(found.into_iter().map(|m| (m.signal, m)));
        for m in missed {
            notes.push((m.span, format!("`{}`: {}", m.what, m.why)));
        }
    }
    let place = |s: yavs_diag::Span| {
        let f = db.sources.get(s.file);
        (f.path.clone(), f.line_col(s.start).0 as u32)
    };
    // The `case` statements on each register.
    let mut cases: HashMap<yavs_sem::DeclId, Vec<yavs_diag::Span>> = HashMap::new();
    for (_, s) in db.stmts.iter() {
        if let StmtKind::Case { expr, .. } = &s.kind
            && let ExprKind::Obj(d) = db.exprs[*expr].kind
            && machines.contains_key(&d)
        {
            cases.entry(d).or_default().push(s.span);
        }
    }
    let labels = crate::labelled_processes(db, e);
    let mut warns = Vec::new();
    let mut regions: HashMap<yavs_diag::FileId, Vec<(u32, u32)>> = HashMap::new();
    let mut off = |s: yavs_diag::Span| {
        let line = db.sources.get(s.file).line_col(s.start).0 as u32;
        regions
            .entry(s.file)
            .or_insert_with(|| {
                let f = db.sources.get(s.file);
                crate::regions(&f.path, f.text(), read.translate_off, &mut warns)
            })
            .iter()
            .any(|&(a, b)| a <= line && line <= b)
    };
    let mut gone: HashSet<yavs_sem::DeclId> = HashSet::new();
    let mut states_off: HashMap<yavs_sem::DeclId, Vec<bool>> = HashMap::new();
    let mut arcs_off: HashMap<yavs_sem::DeclId, HashSet<(usize, usize)>> = HashMap::new();
    let mut ids: Vec<_> = machines.keys().copied().collect();
    ids.sort();
    for d in ids {
        let m = &machines[&d];
        let in_cases = cases.get(&d).is_some_and(|c| c.iter().all(|&s| off(s)));
        if off(db.decls[d].span) || in_cases {
            gone.insert(d);
            continue;
        }
        states_off.insert(d, m.states.iter().map(|x| off(x.1)).collect());
        let named: HashSet<(usize, usize)> = m
            .transitions()
            .filter(|a| off(a.2))
            .map(|a| (a.0, a.1))
            .collect();
        arcs_off.insert(d, named);
    }
    // A run with counters reads the same regions in `collect`, and says these there.
    let counters = e.ir.as_ref().is_some_and(|i| !i.program.cov.is_empty());
    if counters {
        warns.clear();
    }
    let mut st = State {
        by_signal: vec![Vec::new(); e.nets.signal_count()],
        watches: Vec::new(),
    };
    for (si, sc) in e.hier.scopes.iter().enumerate() {
        let p = e.hier.path_name(yavs_elab::ScopeId(si as u32), Case::Lrm);
        let unit = match e
            .hier
            .scope(e.instance_of(yavs_elab::ScopeId(si as u32)))
            .kind
        {
            ScopeKind::Instance { entity, .. } => {
                let u = &db.units[entity];
                format!("{}.{}", db.libs[u.lib].name.text, u.name.text).to_ascii_lowercase()
            }
            _ => String::new(),
        };
        let path = p.strip_suffix(':').filter(|p| !p.is_empty()).unwrap_or(&p);
        for &o in &sc.objs {
            let obj = e.nets.obj(o);
            let (ObjKind::Signal, Some(m), Shape::Scalar { .. }) =
                (&obj.kind, machines.get(&obj.decl), &obj.shape)
            else {
                continue;
            };
            if gone.contains(&obj.decl) {
                continue;
            }
            let process = cases
                .get(&obj.decl)
                .and_then(|c| crate::process_at(&labels, c[0]))
                .unwrap_or_default();
            let of_state = |s: usize| states_off.get(&obj.decl).is_some_and(|v| v[s]);
            let of_arc =
                |a: (usize, usize)| arcs_off.get(&obj.decl).is_some_and(|v| v.contains(&a));
            let waivers = waivers.get(&obj.decl).map_or(Vec::new(), |v| {
                let of = |w: &MachineWaiver| {
                    let (file, line) = place(w.at);
                    let waiver = Waiver {
                        rule: w.rule.clone(),
                        reason: w.reason.replace('\t', " "),
                        file,
                        line,
                    };
                    (w.item, waiver)
                };
                v.iter().map(of).collect()
            });
            let (ks, at) = e.nets.kelem(obj.leaves.0, 0);
            st.by_signal[ks.0 as usize].push(st.watches.len() as u32);
            st.watches.push(Watch {
                path: path.to_string(),
                unit: unit.clone(),
                process,
                sig: obj.name.text.to_string(),
                at: at as usize,
                place: place(db.decls[obj.decl].span),
                states: m
                    .states
                    .iter()
                    .enumerate()
                    .map(|(i, (n, s))| {
                        let item = Item {
                            place: place(*s),
                            count: 0,
                            off: of_state(i),
                        };
                        (n.clone(), item)
                    })
                    .collect(),
                arcs: m
                    .transitions()
                    .map(|(f, t, s)| {
                        let item = Item {
                            place: place(s),
                            count: 0,
                            off: of_arc((f, t)),
                        };
                        ((f, t), item)
                    })
                    .collect(),
                waivers,
                last: None,
            });
        }
    }
    (Fsm(Arc::new(Mutex::new(st))), notes, warns)
}

impl State {
    /// The value of kernel signal `sig` may have changed.
    fn change(&mut self, hook: &Hook<'_>, sig: u32) {
        let Some(list) = self.by_signal.get(sig as usize) else {
            return;
        };
        for &w in list {
            let w = &mut self.watches[w as usize];
            let Some(now) = position(hook, sig, w.at) else {
                continue;
            };
            if w.last == Some(now) {
                continue;
            }
            if let Some((_, s)) = w.states.get_mut(now) {
                s.count = s.count.saturating_add(1);
            }
            if let Some(last) = w.last {
                let place = &w.place;
                let a = w.arcs.entry((last, now)).or_insert_with(|| Item {
                    place: place.clone(),
                    count: 0,
                    off: false,
                });
                a.count = a.count.saturating_add(1);
            }
            w.last = Some(now);
        }
    }
}

impl Observer for Fsm {
    /// Only the start of the simulation: the initial value is the first state reached.
    fn wants_phase(&self, phase: Phase) -> bool {
        phase == Phase::StartOfSim
    }

    fn phase(&mut self, hook: &mut Hook<'_>, phase: Phase) {
        if phase != Phase::StartOfSim {
            return;
        }
        let mut st = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        for sig in 0..st.by_signal.len() as u32 {
            st.change(hook, sig);
        }
    }

    /// One lock for the whole cycle.
    fn value_changes(&mut self, hook: &mut Hook<'_>, signals: &[u32]) {
        let mut st = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        for &s in signals {
            st.change(hook, s);
        }
    }
}

impl Fsm {
    /// Add every state and transition, count 0 included, to `db`.
    pub fn add_to(&self, db: &mut Database) {
        let st = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        for w in &st.watches {
            let mut add = |transition: bool,
                           name: String,
                           item: &Item,
                           taken: &dyn Fn(MachineItem) -> bool| {
                if item.off {
                    return;
                }
                let key = Key {
                    path: w.path.clone(),
                    kind: Kind::Fsm(transition),
                    hash: 0,
                    occ: 0,
                    sig: format!("{}:{name}", w.sig),
                };
                let e = db.items.entry(key).or_insert_with(|| Entry {
                    file: item.place.0.clone(),
                    line: item.place.1,
                    count: 0,
                    text: String::new(),
                    tests: Vec::new(),
                    waivers: w
                        .waivers
                        .iter()
                        .filter(|x| taken(x.0))
                        .map(|x| x.1.clone())
                        .collect(),
                    unit: w.unit.clone(),
                    process: w.process.clone(),
                    operands: Vec::new(),
                    detail: crate::Detail::default(),
                });
                e.count = e.count.saturating_add(item.count);
            };
            for (i, (name, item)) in w.states.iter().enumerate() {
                add(false, name.clone(), item, &|x| x == MachineItem::State(i));
            }
            for (&(from, to), item) in &w.arcs {
                let name = format!("{}->{}", w.states[from].0, w.states[to].0);
                add(true, name, item, &|x| x.takes_transition(from, to));
            }
        }
    }
}
