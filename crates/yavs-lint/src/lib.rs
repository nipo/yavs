//! `yavs lint`: what can be proved about a design from its analysed HIR, before it is simulated.
//!
//! A rule reports what it can derive and nothing it would have to guess: when the evidence is
//! missing (a generic decides a width, a call may hide a `wait`), it is silent. [`RULES`] is the
//! one list: reporting, `--list-rules`, `--explain`, the configuration and the rule reference page
//! all read it. See `AGENTS.md` in this crate.

mod config;
mod design;
pub use design::fsm::{Machine, NotMeasured, machines};
mod report;
mod rules;
mod testbench;

pub use config::{CONFIG, Config, WAIVERS, Waivers, expand, find_up, near_entry, unmatched};
pub use report::{Entry, Reports};

use serde_json::Value;
use yavs_diag::{Severity, Span};
use yavs_sem::{Db, UnitId};

/// How sure a rule is that what it reports is wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Certainty {
    /// The program cannot do what it says. On by default.
    Definite,
    /// An exact fact whose significance depends on intent. Off by default.
    Advisory,
    /// Inferred design intent. Off by default.
    Experimental,
    /// A house convention. Off by default.
    Policy,
}

impl Certainty {
    /// The name of the class in `rule.group` and in reports.
    pub fn name(self) -> &'static str {
        match self {
            Certainty::Definite => "definite",
            Certainty::Advisory => "advisory",
            Certainty::Experimental => "experimental",
            Certainty::Policy => "policy",
        }
    }
}

/// One rule: what it is called, how sure it is, what it reports and what it needs to report it.
#[derive(Debug)]
pub struct Rule {
    pub id: &'static str,
    pub certainty: Certainty,
    pub summary: &'static str,
    pub explain: &'static str,
}

pub use rules::RULES;

/// The rule with this id.
pub fn rule(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

/// The rule whose id `id` is a typo of, when exactly one is close. Rule ids are numbered
/// densely: a well-formed one that does not exist (`lint_999`) is no typo of its neighbours, and
/// neither is an id with another stem (`length_001`).
pub fn similar_rule(id: &str) -> Option<&'static str> {
    let key = id.to_ascii_lowercase();
    if key
        .strip_prefix("lint_")
        .is_some_and(|d| d.len() == 3 && d.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    if key
        .split_once('_')
        .is_some_and(|(stem, _)| stem.len().abs_diff(4) > 1)
    {
        return None;
    }
    let found = yavs_diag::suggest::similar(&key, RULES.iter().map(|r| (r.id, r.id)));
    match found[..] {
        [id] => Some(id),
        _ => None,
    }
}

/// What one rule found.
#[derive(Debug)]
pub struct Finding {
    pub rule: &'static Rule,
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    /// The other places it is about: every driver, every signal on a loop.
    pub related: Vec<Span>,
}

/// What a run found: the findings of the enabled rules in `units` in source order, and the design
/// facts when `facts` asks for them. A rule beyond the definite class costs a walk of its own, which
/// a default run does not take. Instances of generics carry the spans of the generic, so a finding
/// is reported once per place. `base` is the directory of the configuration file: what
/// `testbench_files` is relative to.
pub fn lint(
    db: &Db,
    units: &[UnitId],
    cfg: &Config,
    base: &std::path::Path,
    facts: bool,
) -> (Vec<Finding>, Value) {
    let (mut found, mut related, facts) = rules::check(db, units, cfg, facts);
    let mut kinds = std::collections::HashMap::new();
    let mut kind = |file: yavs_diag::FileId| {
        *kinds
            .entry(file)
            .or_insert_with(|| testbench::kind(db, file, units, cfg, base))
    };
    found.retain(|(id, span, _)| rule(id).is_some_and(|r| cfg.enabled_in(r, kind(span.file))));
    // The message too: one place can hold two findings of a rule, and a rule's order is not stable.
    found.sort_by(|a, b| {
        (a.1.file.0, a.1.start, a.1.end, a.0, &a.2)
            .cmp(&(b.1.file.0, b.1.start, b.1.end, b.0, &b.2))
    });
    found.dedup();
    let found = found
        .into_iter()
        .map(|(id, span, message)| {
            let rule = rule(id).expect("kept above");
            Finding {
                rule,
                severity: cfg.severity_in(rule, kind(span.file)),
                span,
                related: related.remove(&(span, message.clone())).unwrap_or_default(),
                message,
            }
        })
        .collect();
    (found, facts)
}
