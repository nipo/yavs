//! State machine coverage (`--cov-kind=fsm`, #1020): an observer on the kernel's value-change bus
//! that counts, per instance, each state a machine's register reached and each change from one
//! state to another. It reads the register's value and nothing else, so it counts the same in
//! every tier and at any `--threads`, and it exists only with this kind on. Which signals are
//! machines is `yavs_lint::machines`.
//!
//! Items are keyed by the path of the scope declaring the register and `SIGNAL:STATE` or
//! `SIGNAL:FROM->TO` (`Key::sig`): names, no hash and no offsets, so an edit keeps them.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use yavs_elab::{Case, Elaborated, ObjKind, ScopeKind, Shape};
use yavs_kernel::{Hook, Observer, Phase, SignalId};
use yavs_lint::Machine;
use yavs_sem::Db;

use crate::{Database, Entry, Key, Kind};

/// One machine in one instance.
struct Watch {
    path: String,
    /// `library.entity` of its instance.
    unit: String,
    /// The register's name, as declared.
    sig: String,
    /// Kernel element of the register.
    at: usize,
    /// Declaration of the register: where a transition no `case` names is shown.
    place: (String, u32),
    /// By position: name, where declared, times entered.
    states: Vec<(String, (String, u32), u64)>,
    /// By (from, to): where named, times taken. Every transition a `case` names, count 0 included;
    /// one that happens otherwise (a reset) comes in when it does.
    arcs: BTreeMap<(usize, usize), ((String, u32), u64)>,
    last: Option<usize>,
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
pub fn fsm(db: &Db, e: &Elaborated) -> (Fsm, Vec<(yavs_diag::Span, String)>) {
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
            let (ks, at) = e.nets.kelem(obj.leaves.0, 0);
            st.by_signal[ks.0 as usize].push(st.watches.len() as u32);
            st.watches.push(Watch {
                path: path.to_string(),
                unit: unit.clone(),
                sig: obj.name.text.to_string(),
                at: at as usize,
                place: place(db.decls[obj.decl].span),
                states: m
                    .states
                    .iter()
                    .map(|(n, s)| (n.clone(), place(*s), 0))
                    .collect(),
                arcs: m
                    .arcs
                    .iter()
                    .filter(|a| a.0 != a.1)
                    .map(|&(f, t, s)| ((f, t), (place(s), 0)))
                    .collect(),
                last: None,
            });
        }
    }
    (Fsm(Arc::new(Mutex::new(st))), notes)
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
            if let Some((_, _, c)) = w.states.get_mut(now) {
                *c = c.saturating_add(1);
            }
            if let Some(last) = w.last {
                let place = &w.place;
                let a = w
                    .arcs
                    .entry((last, now))
                    .or_insert_with(|| (place.clone(), 0));
                a.1 = a.1.saturating_add(1);
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
            let mut add = |transition: bool, item: String, place: &(String, u32), count: u64| {
                let key = Key {
                    path: w.path.clone(),
                    kind: Kind::Fsm(transition),
                    hash: 0,
                    occ: 0,
                    sig: format!("{}:{item}", w.sig),
                };
                let e = db.items.entry(key).or_insert_with(|| Entry {
                    file: place.0.clone(),
                    line: place.1,
                    count: 0,
                    text: String::new(),
                    tests: Vec::new(),
                    waivers: Vec::new(),
                    unit: w.unit.clone(),
                    process: String::new(),
                    operands: Vec::new(),
                    detail: crate::Detail::default(),
                });
                e.count = e.count.saturating_add(count);
            };
            for (name, place, count) in &w.states {
                add(false, name.clone(), place, *count);
            }
            for (&(from, to), (place, count)) in &w.arcs {
                let item = format!("{}->{}", w.states[from].0, w.states[to].0);
                add(true, item, place, *count);
            }
        }
    }
}
