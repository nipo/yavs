//! The end-of-run tables: `--assert-summary` (reports per severity and place) and `--activity`
//! (the busiest processes and signals). The console shows the same text.

use yavs_diag::{SourceMap, Span};
use yavs_interp::{AssertSite, Runtime};
use yavs_kernel::{ProcId, Severity, SignalId};

use crate::{Elaborated, ProcTable};

/// How many lines each activity table shows.
pub const TOP: usize = 20;

fn place(sources: &SourceMap, span: Option<Span>) -> String {
    span.map_or_else(
        || "(no source place)".to_string(),
        |s| {
            let f = sources.get(s.file);
            format!("{}:{}", f.path, f.line_col(s.start).0)
        },
    )
}

fn word(s: Severity) -> &'static str {
    match s {
        Severity::Note => "note",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Failure => "failure",
    }
}

const SEVERITIES: [Severity; 4] = [
    Severity::Failure,
    Severity::Error,
    Severity::Warning,
    Severity::Note,
];

/// Sites, most severe first, then the most frequent, then by place.
fn sorted(rt: &Runtime, sources: &SourceMap) -> Vec<(AssertSite, String)> {
    let mut v: Vec<(AssertSite, String)> = rt
        .assert_sites()
        .into_iter()
        .map(|a| {
            let span = rt.prog.stmts.get(a.stmt).map(|i| i.span);
            let at = place(sources, span);
            (a, at)
        })
        .collect();
    v.sort_by(|a, b| {
        (b.0.severity.cmp(&a.0.severity))
            .then(b.0.count.cmp(&a.0.count))
            .then_with(|| a.1.cmp(&b.1))
    });
    v
}

/// The first line of a message, cut to fit a table.
fn short(m: &str) -> String {
    let line = m.lines().next().unwrap_or_default();
    match line.char_indices().nth(60) {
        Some((i, _)) => format!("{}...", &line[..i]),
        None => line.to_string(),
    }
}

/// The assertion summary as a table.
pub fn assert_summary_text(rt: Option<&Runtime>, sources: &SourceMap) -> String {
    let sites = rt.map(|r| sorted(r, sources)).unwrap_or_default();
    let count = |s: Severity| -> u64 {
        (sites.iter().filter(|a| a.0.severity == s))
            .map(|a| a.0.count)
            .sum()
    };
    let total: Vec<String> = SEVERITIES
        .iter()
        .map(|&s| match count(s) {
            1 => format!("1 {}", word(s)),
            n => format!("{n} {}s", word(s)),
        })
        .collect();
    let mut o = format!("Assertion summary: {}\n", total.join(", "));
    let elab: u64 = SEVERITIES
        .iter()
        .map(|&s| crate::elaboration_reports(s))
        .sum();
    if elab > 0 {
        o.push_str(&format!(
            "  and {elab} made while elaborating (not in this table)\n"
        ));
    }
    if sites.is_empty() {
        o.push_str("  no assertion failed and no report was made\n");
        return o;
    }
    let w = sites.iter().map(|a| a.1.len()).max().unwrap_or(0).max(5);
    o.push_str(&format!(
        "  severity  count  {:w$}  first message\n",
        "place"
    ));
    for (a, at) in &sites {
        o.push_str(&format!(
            "  {:8}  {:5}  {at:w$}  {}\n",
            word(a.severity),
            a.count,
            short(&a.message)
        ));
    }
    o
}

/// The assertion summary as one JSON object (`--message-format=json`).
pub fn assert_summary_json(rt: Option<&Runtime>, sources: &SourceMap) -> serde_json::Value {
    let sites = rt.map(|r| sorted(r, sources)).unwrap_or_default();
    let count = |s: Severity| -> u64 {
        (sites.iter().filter(|a| a.0.severity == s))
            .map(|a| a.0.count)
            .sum()
    };
    let mut totals = serde_json::Map::new();
    for s in SEVERITIES {
        totals.insert(word(s).to_string(), count(s).into());
    }
    let places: Vec<serde_json::Value> = rt
        .map(|r| {
            sorted(r, sources)
                .iter()
                .map(|(a, _)| {
                    let span = r.prog.stmts.get(a.stmt).map(|i| i.span);
                    let (file, line) = span.map_or((None, None), |s| {
                        let f = sources.get(s.file);
                        (Some(f.path.clone()), Some(f.line_col(s.start).0))
                    });
                    serde_json::json!({
                        "severity": word(a.severity),
                        "count": a.count,
                        "file": file,
                        "line": line,
                        "message": a.message,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    serde_json::json!({"assert_summary": {"totals": totals, "places": places}})
}

/// `(count, name, place)` of the non-zero counts, busiest first.
fn ranked(mut v: Vec<(u64, String, String)>) -> Vec<(u64, String, String)> {
    v.retain(|x| x.0 > 0);
    v.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    v
}

/// The names and places of what `--activity` counted.
#[derive(Clone, Default)]
pub struct ActivityNames {
    /// Process names, by process number.
    pub procs: Vec<String>,
    /// Signal names, by signal id.
    pub signals: Vec<String>,
}

type Ranked = (Vec<(u64, String, String)>, Vec<(u64, String, String)>);

fn activity(
    e: &Elaborated,
    t: &ProcTable,
    runs: &[u64],
    events: &[u64],
    names: &ActivityNames,
    sources: &SourceMap,
) -> Ranked {
    let procs = (runs.iter().zip(&names.procs).enumerate())
        .map(|(p, (&n, name))| {
            let at = place(sources, e.proc_place(t, ProcId(p as u32)));
            (n, name.clone(), at)
        })
        .collect();
    let sigs = (events.iter().zip(&names.signals).enumerate())
        .map(|(s, (&n, name))| {
            let at = place(sources, t.signal_place(SignalId(s as u32)));
            (n, name.clone(), at)
        })
        .collect();
    (ranked(procs), ranked(sigs))
}

fn table(o: &mut String, what: &str, unit: &str, rows: &[(u64, String, String)]) {
    let total: u64 = rows.iter().map(|r| r.0).sum();
    o.push_str(&format!(
        "{what}: {total} {unit} in all, {} shown\n",
        rows.len().min(TOP)
    ));
    let rows = &rows[..rows.len().min(TOP)];
    let nw = rows.iter().map(|r| r.1.len()).max().unwrap_or(0).max(4);
    let cw = (rows.iter().map(|r| r.0.to_string().len()).max())
        .unwrap_or(0)
        .max(unit.len());
    o.push_str(&format!("  {unit:>cw$}      %  {:nw$}  place\n", "name"));
    for (n, name, at) in rows {
        let pct = 100.0 * *n as f64 / total.max(1) as f64;
        o.push_str(&format!("  {n:>cw$}  {pct:5.1}  {name:nw$}  {at}\n"));
    }
}

/// The activity tables: process runs, and for each signal the delta cycles with an event.
pub fn activity_text(
    e: &Elaborated,
    t: &ProcTable,
    runs: &[u64],
    events: &[u64],
    names: &ActivityNames,
    sources: &SourceMap,
) -> String {
    let (procs, sigs) = activity(e, t, runs, events, names, sources);
    let mut o = String::new();
    table(&mut o, "Process activations", "runs", &procs);
    table(&mut o, "Signal activity", "events", &sigs);
    o
}

/// The activity as JSON (`--activity=FILE`), everything that counted.
pub fn activity_json(
    e: &Elaborated,
    t: &ProcTable,
    runs: &[u64],
    events: &[u64],
    names: &ActivityNames,
    sources: &SourceMap,
) -> serde_json::Value {
    let (procs, sigs) = activity(e, t, runs, events, names, sources);
    let row = |k: &str, r: &[(u64, String, String)]| -> serde_json::Value {
        r.iter()
            .map(|(n, name, at)| serde_json::json!({k: n, "name": name, "place": at}))
            .collect()
    };
    serde_json::json!({"processes": row("runs", &procs), "signals": row("events", &sigs)})
}
