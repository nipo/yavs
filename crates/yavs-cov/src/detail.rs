//! `yavs cov report --detailed`: a debugging view per instance. A table per kind with every item,
//! covered and missed, then an entry for what never ran, in the style of the diagnostics: where,
//! what is missing, a tip the data supports. Where one cause explains several items they are
//! grouped under it. `--uncovered` stays the row format for exclusion files.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use yavs_diag::{Diagnostic, FileId, MessageFormat, Severity, SourceMap, Span};

use crate::{Database, Entry, Key, Kind, SummaryOptions, count, percent, table};

/// One line of a list in an entry, or tests named, before `and N more`.
const MAX_TESTS: usize = 3;

/// `data(3)` as (`data`, 3); `None` for a name that does not end in a number.
fn indexed(sig: &str) -> Option<(&str, i64)> {
    let inner = sig.strip_suffix(')')?;
    let at = inner.rfind('(')?;
    Some((&sig[..at], inner[at + 1..].parse().ok()?))
}

/// Sorted indices as ranges, whatever the declared direction: `0..3, 5`.
fn ranges(mut v: Vec<i64>) -> String {
    v.sort_unstable();
    let mut runs: Vec<(i64, i64)> = Vec::new();
    for i in v {
        match runs.last_mut() {
            Some((_, b)) if *b + 1 == i => *b = i,
            _ => runs.push((i, i)),
        }
    }
    let run = |&(a, b): &(i64, i64)| {
        if a == b {
            a.to_string()
        } else {
            format!("{a}..{b}")
        }
    };
    runs.iter().map(run).collect::<Vec<_>>().join(", ")
}

/// An item's status in a table.
fn status(e: &Entry, why: Option<&str>) -> String {
    match why {
        Some("") => "excluded".to_string(),
        Some(w) => format!("excluded: {w}"),
        None if e.count > 0 => "covered".to_string(),
        None => "MISSED".to_string(),
    }
}

/// The exact text that excludes one missed item, ready to paste, and whether it is short enough
/// to stand beside the row: for a condition or expression row or a combination the bare short
/// directive where one is allowed, one symbol per input in source order; for a state or transition
/// its directive; everything else, or a database too old to give the directive, an exclusion file
/// row.
fn exclude_with(k: &Key, e: &Entry) -> (String, bool) {
    if let Kind::Fsm(_) = k.kind
        && let Some(text) = crate::exclude::directive(k, e)
    {
        return (format!("exclude with {text}"), true);
    }
    if e.detail.directive
        && let Some(text) = crate::exclude::directive(k, e)
    {
        // The bare form (user, 2026-11-01): the pattern, without the list of inputs.
        let bare = text.split_once(" (").map_or(text.as_str(), |x| x.0);
        return (format!("exclude with {bare}"), true);
    }
    let row = crate::exclude::head(k, e);
    (
        format!("exclude with this row in an `--exclude` file: {row}"),
        false,
    )
}

/// The mark of a missed row: `missed` with the text beside it, or on the line under the row.
fn missed_mark(word: &str, k: &Key, e: &Entry) -> String {
    match exclude_with(k, e) {
        (text, true) => format!("{word} ({text})"),
        (text, false) => format!("{word}\n{text}"),
    }
}

/// What one instance has: its items, and those excluded with their reasons.
struct Instance<'a> {
    db: &'a Database,
    items: Vec<(&'a Key, &'a Entry, Option<&'a str>)>,
}

impl<'a> Instance<'a> {
    fn of_kind(&self, f: fn(Kind) -> bool) -> Vec<(&'a Key, &'a Entry, Option<&'a str>)> {
        let mut v: Vec<_> = self.items.iter().copied().filter(|x| f(x.0.kind)).collect();
        v.sort_by(|a, b| (&a.1.file, a.1.line, a.0).cmp(&(&b.1.file, b.1.line, b.0)));
        v
    }

    /// `12`, or `f.vhd:12` when the instance has several files.
    fn line(&self, e: &Entry) -> String {
        let files: BTreeSet<&str> = self.items.iter().map(|x| x.1.file.as_str()).collect();
        if files.len() > 1 {
            format!("{}:{}", e.file, e.line)
        } else {
            e.line.to_string()
        }
    }

    /// The tables of statements and branches: a row per item.
    fn item_table(&self, s: &mut String, title: &str, f: fn(Kind) -> bool) {
        let v = self.of_kind(f);
        if v.is_empty() {
            return;
        }
        let mut rows = vec![
            ["Line", "Item", "Hits", "Status"]
                .map(String::from)
                .to_vec(),
        ];
        for (k, e, why) in v {
            let item = match k.kind {
                Kind::Branch(_) => format!("arm: {}", e.text),
                Kind::Fsm(_) => crate::fsm_text(k),
                _ => e.text.clone(),
            };
            let st = if why.is_none() && e.count == 0 {
                missed_mark("MISSED", k, e)
            } else {
                status(e, why)
            };
            // An arm is on its own line, an `elsif` not on that of its `if`.
            let mut at = self.line(e);
            if matches!(k.kind, Kind::Branch(_)) && e.detail.line > 0 {
                at = at.replace(&e.line.to_string(), &e.detail.line.to_string());
            }
            rows.push(vec![at, item, e.count.to_string(), st]);
        }
        let _ = writeln!(s, "{title}:\n{}", table(&rows, &[2], "  "));
    }

    /// Toggles: bits in a run of one direction and one count share a row.
    fn toggle_table(&self, s: &mut String) {
        let v = self.of_kind(|k| matches!(k, Kind::Toggle(_)));
        if v.is_empty() {
            return;
        }
        type Run<'b> = (u32, String, bool, u64, String);
        let mut bits: BTreeMap<Run, Vec<i64>> = BTreeMap::new();
        let mut size: BTreeMap<(u32, String, bool), usize> = BTreeMap::new();
        let mut all: Vec<(u32, String, u64, String)> = Vec::new();
        let path = v[0].0.shown();
        for (k, e, why) in v {
            let Kind::Toggle(rise) = k.kind else { continue };
            let st = status(e, why);
            match indexed(&k.sig) {
                Some((base, i)) => {
                    *size.entry((e.line, base.to_string(), rise)).or_default() += 1;
                    bits.entry((e.line, base.to_string(), rise, e.count, st))
                        .or_default()
                        .push(i);
                }
                None => {
                    let st = if st == "MISSED" {
                        let dir = if rise { "rise" } else { "fall" };
                        format!(
                            "MISSED\nexclude with this row in an `--exclude` file: toggle {} {} {dir}",
                            k.shown(),
                            k.sig
                        )
                    } else {
                        st
                    };
                    all.push((
                        e.line,
                        format!("{} {}", k.sig, crate::arrow(rise)),
                        e.count,
                        st,
                    ));
                }
            }
        }
        for ((line, base, rise, c, st), idx) in bits {
            let item = format!("{base}({}) {}", ranges(idx.clone()), crate::arrow(rise));
            let st = if st == "MISSED" {
                let dir = if rise { "rise" } else { "fall" };
                // Every bit of the signal is missed: one glob row. Else a row per bit.
                let lines: Vec<String> =
                    if size.get(&(line, base.clone(), rise)) == Some(&idx.len()) {
                        vec![format!("toggle {path} {base}(*) {dir}")]
                    } else {
                        idx.iter()
                            .map(|i| format!("toggle {path} {base}({i}) {dir}"))
                            .collect()
                    };
                format!(
                    "MISSED\nexclude with these rows in an `--exclude` file:\n{}",
                    lines.join("\n")
                )
            } else {
                st
            };
            all.push((line, item, c, st));
        }
        all.sort();
        let mut rows = vec![
            ["Line", "Item", "Hits", "Status"]
                .map(String::from)
                .to_vec(),
        ];
        for (l, item, c, st) in all {
            rows.push(vec![l.to_string(), item, c.to_string(), st]);
        }
        let _ = writeln!(s, "toggle:\n{}", table(&rows, &[2], "  "));
    }

    /// Conditions or expressions: per decision a line when fully covered, else its rows.
    fn focused_tables(&self, s: &mut String, title: &str, word: &str, f: fn(Kind) -> bool) {
        let v = self.of_kind(f);
        if v.is_empty() {
            return;
        }
        let _ = writeln!(s, "{title}:");
        for rows in decisions(&v) {
            focused(s, self, &rows, word);
        }
    }

    fn combination_tables(&self, s: &mut String) {
        let v = self.of_kind(|k| matches!(k, Kind::Combination(_)));
        if v.is_empty() {
            return;
        }
        let _ = writeln!(s, "combination:");
        let mut groups: BTreeMap<(u32, u64, u32), Vec<_>> = BTreeMap::new();
        for it in v {
            groups
                .entry((it.1.line, it.0.hash, it.0.occ))
                .or_default()
                .push(it);
        }
        for ((line, ..), its) in groups {
            let _ = writeln!(s, "Line {line}: {}", its[0].1.text);
            let mut rows = vec![["Combination", "Hits", "Status"].map(String::from).to_vec()];
            for (k, e, why) in its {
                let st = if why.is_none() && e.count == 0 {
                    missed_mark("MISSED", k, e)
                } else {
                    status(e, why)
                };
                rows.push(vec![crate::combination_words(k), e.count.to_string(), st]);
            }
            let _ = writeln!(s, "{}", table(&rows, &[1], "  "));
        }
    }
}

type Item<'a> = (&'a Key, &'a Entry, Option<&'a str>);

/// The rows of `v` per decision, in source order.
fn decisions<'a>(v: &[Item<'a>]) -> Vec<Vec<Item<'a>>> {
    let mut groups: BTreeMap<(&str, u32, u64, u32), Vec<Item<'a>>> = BTreeMap::new();
    for &it in v {
        groups
            .entry((&it.1.file, it.1.line, it.0.hash, it.0.occ))
            .or_default()
            .push(it);
    }
    groups.into_values().collect()
}

/// The text after the result character of a condition row: the whole condition.
fn whole(e: &Entry) -> &str {
    e.text.split_once(' ').map_or(e.text.as_str(), |x| x.1)
}

/// The value of a row.
fn value(k: &Key) -> u8 {
    k.kind.row().map_or(0, |r| u8::from(r.1))
}

/// What a row's input needs of the others; empty when no other input matters.
fn others(e: &Entry) -> &str {
    &e.detail.needs
}

/// A tip for a row: its input at its value with what the others must be.
fn row_hint(k: &Key, e: &Entry) -> String {
    let v = value(k);
    match others(e) {
        "" => format!("make `{}` {v}; no other input matters", k.sig),
        o => format!("make `{}` {v} with {o}", k.sig),
    }
}

/// The focused table of one decision: a line when every row ran.
fn focused(s: &mut String, inst: &Instance, rows: &[Item], word: &str) {
    let e0 = rows[0].1;
    let head = format!("Line {}: {}   ({word})", inst.line(e0), whole(e0));
    let open: Vec<_> = rows
        .iter()
        .filter(|r| r.1.count == 0 && r.2.is_none())
        .collect();
    if open.is_empty() {
        let excluded = rows.iter().filter(|r| r.2.is_some()).count();
        let n = count(rows.len(), "row", "rows");
        if excluded == 0 {
            let _ = writeln!(s, "{head}  all {n} hit");
        } else {
            let _ = writeln!(s, "{head}  {n} hit or excluded, {excluded} excluded");
        }
        return;
    }
    let mut inputs: Vec<&str> = Vec::new();
    for r in rows {
        if !inputs.contains(&r.0.sig.as_str()) {
            inputs.push(&r.0.sig);
        }
    }
    let of = |i: &str, v: u8| rows.iter().find(|r| r.0.sig == i && value(r.0) == v);
    let hit = |r: &Item| r.1.count > 0 || r.2.is_some();
    let done = inputs
        .iter()
        .filter(|i| [0, 1].iter().all(|&v| of(i, v).is_none_or(hit)))
        .count();
    let _ = writeln!(
        s,
        "{head}\nInput totals: {done} of {} covered, {}",
        count(inputs.len(), "input", "inputs"),
        percent(done, inputs.len())
    );
    let mut t = vec![
        ["Input", "Covered", "Reason not covered", "Hint"]
            .map(String::from)
            .to_vec(),
    ];
    for i in &inputs {
        let missed: Vec<&Item> = [0, 1]
            .iter()
            .filter_map(|&v| of(i, v))
            .filter(|r| !hit(r))
            .collect();
        let reason = match missed[..] {
            [] => String::new(),
            [a] => format!("value {} not hit", value(a.0)),
            _ => "neither value hit".to_string(),
        };
        // A second hint goes on the line under the row.
        let hint: Vec<String> = missed.iter().map(|r| row_hint(r.0, r.1)).collect();
        let covered = if missed.is_empty() { "yes" } else { "no" };
        t.push(vec![
            i.to_string(),
            covered.to_string(),
            reason,
            hint.join("; "),
        ]);
    }
    let _ = writeln!(s, "{}", crate::ruled(&t, &[]));
    let mut t = vec![
        ["Row", "Hits", "Target", "Other inputs must be", ""]
            .map(String::from)
            .to_vec(),
    ];
    for (n, (k, e, why)) in rows.iter().enumerate() {
        let mark = match why {
            Some(_) => "<- excluded".to_string(),
            None if e.count == 0 => missed_mark("<- missed", k, e),
            None => String::new(),
        };
        let other = if others(e).is_empty() {
            "any"
        } else {
            others(e)
        };
        t.push(vec![
            (n + 1).to_string(),
            e.count.to_string(),
            format!("{} is {}", k.sig, value(k)),
            other.to_string(),
            mark,
        ]);
    }
    let _ = writeln!(s, "{}", crate::ruled(&t, &[0, 1]));
}

/// The sources an entry can show: per shown path the text, or why there is none.
pub type Sources = BTreeMap<String, Result<String, String>>;

/// Sources in a [`SourceMap`] the renderer takes.
struct Shown<'a> {
    map: SourceMap,
    ids: BTreeMap<&'a str, FileId>,
    sources: Option<&'a Sources>,
}

impl<'a> Shown<'a> {
    fn new(sources: Option<&'a Sources>) -> Self {
        let mut map = SourceMap::default();
        let mut ids = BTreeMap::new();
        for (path, text) in sources.into_iter().flatten() {
            if let Ok(t) = text {
                ids.insert(path.as_str(), map.add(path.clone(), t.clone()));
            }
        }
        Shown { map, ids, sources }
    }

    /// The span of an item in the text at hand; or why it cannot be shown.
    fn span(&self, e: &Entry) -> Result<Span, String> {
        let d = &e.detail;
        if d.line == 0 {
            return Err("the database holds no position in the file for it".to_string());
        }
        let Some(&file) = self.ids.get(e.file.as_str()) else {
            return Err(match self.sources.and_then(|s| s.get(&e.file)) {
                Some(Err(why)) => format!("cannot read it: {why}"),
                _ => "it is not in the database".to_string(),
            });
        };
        let text = self.map.get(file).text();
        let slice = text.get(d.start as usize..d.end as usize);
        if slice.is_none_or(|t| crate::fnv1a(t) != d.digest) {
            return Err("the file changed since the run".to_string());
        }
        // Long statements show their first line only.
        let t = slice.unwrap_or("");
        let end = if t.matches('\n').count() > 2 {
            d.start + t.find('\n').unwrap_or(t.len()) as u32
        } else {
            d.end
        };
        Ok(Span {
            file,
            start: d.start,
            end,
        })
    }
}

/// A short name of an item, for lists under a cause.
fn label(k: &Key, e: &Entry) -> String {
    match k.kind {
        Kind::Statement => format!("line {}: statement `{}`", e.line, e.text),
        Kind::Branch(_) => format!("line {}: arm `{}`", e.line, e.text),
        Kind::Condition(_) | Kind::Expression(_) => {
            format!(
                "line {}: {} is {} in `{}`",
                e.line,
                k.sig,
                value(k),
                whole(e)
            )
        }
        Kind::Combination(_) => format!("line {}: {}", e.line, crate::comb_text(k, e)),
        Kind::Toggle(r) => format!("line {}: {} {}", e.line, k.sig, crate::arrow(r)),
        Kind::Fsm(_) => format!("line {}: {}", e.line, crate::fsm_text(k)),
    }
}

/// The names of `tests` for a note.
fn names(db: &Database, tests: &BTreeSet<u32>) -> String {
    let mut v: Vec<&str> = tests
        .iter()
        .filter_map(|&t| db.tests.get(t as usize))
        .map(String::as_str)
        .collect();
    v.sort_unstable();
    let extra = v.len().saturating_sub(MAX_TESTS);
    v.truncate(MAX_TESTS);
    let mut s = v.join(", ");
    if extra > 0 {
        let _ = write!(s, " and {extra} more");
    }
    s
}

/// A missed item.
type Missed<'a> = (&'a Key, &'a Entry);

/// One thing to act on: an item, a signal's toggles or a decision's rows, with what it explains.
struct Finding<'a> {
    items: Vec<(&'a Key, &'a Entry)>,
    /// What never ran because of this.
    inside: Vec<(&'a Key, &'a Entry)>,
}

/// The item that explains `k`: up through its statement and the arm around it, while they never
/// ran. `k` itself when nothing above it is missed.
fn root<'a>(db: &'a Database, k: &'a Key, e: &'a Entry) -> &'a Key {
    let (mut k, mut e) = (k, e);
    loop {
        let d = &e.detail;
        let owner = d.owner.and_then(|(hash, occ)| {
            let key = Key {
                path: k.path.clone(),
                kind: Kind::Statement,
                hash,
                occ,
                sig: String::new(),
            };
            db.items.get_key_value(&key)
        });
        let under = d.under.and_then(|(hash, occ, arm)| {
            let key = Key {
                path: k.path.clone(),
                kind: Kind::Branch(arm),
                hash,
                occ,
                sig: String::new(),
            };
            db.items.get_key_value(&key)
        });
        match owner.into_iter().chain(under).find(|(_, x)| x.count == 0) {
            Some((ok, oe)) if ok != k => (k, e) = (ok, oe),
            _ => return k,
        }
    }
}

/// The arms of the decision of `k`: name and count, in order.
fn arms<'a>(db: &'a Database, k: &Key) -> Vec<(&'a Key, &'a Entry)> {
    db.items
        .iter()
        .filter(|(o, _)| {
            matches!(o.kind, Kind::Branch(_))
                && o.path == k.path
                && o.hash == k.hash
                && o.occ == k.occ
        })
        .collect()
}

/// A note, tip and tests for a branch arm.
fn arm_entry(db: &Database, k: &Key, e: &Entry, mut d: Diagnostic) -> Diagnostic {
    let all = arms(db, k);
    let total: u64 = all.iter().map(|x| x.1.count).sum();
    let others: Vec<String> = all
        .iter()
        .filter(|x| x.0 != k)
        .map(|x| {
            format!(
                "`{}` ran {}",
                x.1.text,
                count(x.1.count as usize, "time", "times")
            )
        })
        .collect();
    if !others.is_empty() {
        d = d.with_note(format!("the other arms: {}", others.join(", ")));
    }
    let earlier: Vec<String> = all
        .iter()
        .take_while(|x| x.0 != k)
        .filter_map(|x| condition_of(&x.1.text))
        .map(|c| format!("`{c}`"))
        .collect();
    let tip = if total == 0 {
        "no arm of this decision ran, so it was never reached: run the code around it first"
            .to_string()
    } else if e.text == "loop body" {
        "the loop was reached and never iterated: make its range non-empty or its condition true"
            .to_string()
    } else if let Some(c) = e.text.strip_prefix("when ").filter(|c| *c != "others") {
        if all.iter().any(|x| condition_of(&x.1.text).is_some()) {
            format!("make `{c}` true while the conditions before it are false")
        } else {
            format!("drive the selector to {c}")
        }
    } else if let Some(c) = condition_of(&e.text) {
        match earlier.as_slice() {
            [] => format!("`{c}` was never true when this ran: make it true"),
            list => format!(
                "`{c}` was never true with {} false: make it so",
                list.join(", ")
            ),
        }
    } else if earlier.is_empty() {
        "every run took one of the other arms: drive a value they do not match".to_string()
    } else {
        format!(
            "every run took an earlier arm: make {} false",
            earlier.join(", ")
        )
    };
    d = d.with_help(tip);
    let reached: BTreeSet<u32> = all.iter().flat_map(|x| x.1.tests.iter().copied()).collect();
    if db.tests.len() > 1 && !reached.is_empty() {
        d = d.with_note(format!(
            "closest tests, which ran this decision and took another arm: {}",
            names(db, &reached)
        ));
    }
    d
}

/// The condition of `if C`, `elsif C`.
fn condition_of(arm: &str) -> Option<&str> {
    arm.strip_prefix("if ")
        .or_else(|| arm.strip_prefix("elsif "))
}

/// A note and tip for a statement that is the cause itself.
fn statement_entry(db: &Database, k: &Key, e: &Entry, mut d: Diagnostic) -> Diagnostic {
    let arm = e.detail.under.and_then(|(hash, occ, arm)| {
        db.items.get(&Key {
            path: k.path.clone(),
            kind: Kind::Branch(arm),
            hash,
            occ,
            sig: String::new(),
        })
    });
    match arm {
        Some(a) => {
            d = d.with_note(format!(
                "it is inside `{}`, which ran {}",
                a.text,
                count(a.count as usize, "time", "times")
            ));
            d = d.with_help(
                "the data does not say what stopped that arm before this line: look for a `wait`, \
                 `exit`, `next`, `return` or a failed check before it",
            );
            if db.tests.len() > 1 && !a.tests.is_empty() {
                let t: BTreeSet<u32> = a.tests.iter().copied().collect();
                d = d.with_note(format!(
                    "closest tests, which ran that arm: {}",
                    names(db, &t)
                ));
            }
        }
        None => {
            d = d.with_help(
                "it is in no branch arm, and the data does not say what kept it from running: a \
                 subprogram that was never called, or a process stopped before this line",
            );
        }
    }
    d
}

/// Notes and a tip for the missing rows of a decision.
fn rows_entry(db: &Database, items: &[(&Key, &Entry)], mut d: Diagnostic) -> Diagnostic {
    let (k0, _) = items[0];
    let siblings: Vec<(&Key, &Entry)> = db
        .items
        .iter()
        .filter(|(o, _)| {
            o.path == k0.path
                && o.hash == k0.hash
                && o.occ == k0.occ
                && std::mem::discriminant(&o.kind) == std::mem::discriminant(&k0.kind)
        })
        .collect();
    let seen: Vec<String> = siblings
        .iter()
        .filter(|x| x.1.count > 0)
        .map(|x| format!("{} at {}", x.0.sig, value(x.0)))
        .collect();
    d = d.with_note(if seen.is_empty() {
        "no input decided the result in these runs".to_string()
    } else {
        format!("seen deciding the result: {}", seen.join(", "))
    });
    for (k, e) in items {
        d = d.with_help(row_hint(k, e));
    }
    let reached: BTreeSet<u32> = siblings
        .iter()
        .flat_map(|x| x.1.tests.iter().copied())
        .collect();
    if db.tests.len() > 1 && !reached.is_empty() {
        d = d.with_note(format!(
            "closest tests, which had another input decide: {}",
            names(db, &reached)
        ));
    }
    d
}

/// The toggles of one signal, in one entry.
fn toggle_entry(items: &[(&Key, &Entry)], mut d: Diagnostic) -> Diagnostic {
    let mut by_scalar: BTreeMap<&str, (bool, bool, char)> = BTreeMap::new();
    for (k, e) in items {
        let Kind::Toggle(rise) = k.kind else { continue };
        let x = by_scalar
            .entry(&k.sig)
            .or_insert((false, false, e.detail.level));
        *(if rise { &mut x.0 } else { &mut x.1 }) = true;
    }
    // Per finding (rise and fall missed, level), the bits or the plain names.
    type Scalars<'b> = (Vec<i64>, Vec<&'b str>);
    let mut groups: BTreeMap<(bool, bool, char), Scalars> = BTreeMap::new();
    for (sig, s) in by_scalar {
        let g = groups.entry(s).or_default();
        match indexed(sig) {
            Some((_, i)) => g.0.push(i),
            None => g.1.push(sig),
        }
    }
    let base = indexed(&items[0].0.sig).map_or(items[0].0.sig.as_str(), |x| x.0);
    for ((rise, fall, level), (bits, plain)) in groups {
        let who = if bits.is_empty() {
            plain.join(", ")
        } else {
            format!("{base}({})", ranges(bits))
        };
        let (note, tip) = match (rise, fall, level) {
            (true, true, '0') => (
                format!("{who}: no transition, and it ended at 0: stuck at 0"),
                format!("drive {who} to 1 and back to 0"),
            ),
            (true, true, '1') => (
                format!("{who}: no transition, and it ended at 1: stuck at 1"),
                format!("drive {who} to 0 and back to 1"),
            ),
            (true, true, '-') => (
                format!(
                    "{who}: no transition, and it ended neither 0 nor 1, so it may never have been driven"
                ),
                format!("drive {who} to 0 and to 1"),
            ),
            (true, true, _) => (
                format!("{who}: no transition; the level differs between runs or is not recorded"),
                format!("drive {who} to 0 and to 1"),
            ),
            (true, false, _) => (
                format!("{who}: it fell and never rose: rise (0->1) is missing"),
                format!("drive {who} back to 1 after it falls"),
            ),
            _ => (
                format!("{who}: it rose and never fell: fall (1->0) is missing"),
                format!("drive {who} back to 0 after it rises"),
            ),
        };
        d = d.with_note(note).with_help(tip);
    }
    d
}

/// A tip for a state or transition of a state machine.
fn fsm_entry(k: &Key, d: Diagnostic) -> Diagnostic {
    let (signal, item) = k.sig.split_once(':').unwrap_or(("", &k.sig));
    match item.split_once("->") {
        Some((from, to)) => d.with_help(format!(
            "take `{signal}` from {from} to {to} in one step: the data does not say whether its other transitions can reach {from}"
        )),
        None => d.with_help(format!(
            "drive what makes `{signal}` enter {item}; it never held that value after the register changed"
        )),
    }
}

/// The entries of what never ran in one instance, worst first.
fn entries(inst: &Instance, shown: &Shown, color: bool) -> String {
    let db = inst.db;
    let missed: Vec<(&Key, &Entry)> = inst
        .items
        .iter()
        .filter(|x| x.2.is_none() && x.1.count == 0)
        .map(|x| (x.0, x.1))
        .collect();
    // Findings by what groups them; an item with a missed cause above it goes under that.
    let mut units: BTreeMap<(u8, u32, String), Finding> = BTreeMap::new();
    let mut under: Vec<(&Key, Missed)> = Vec::new();
    for &(k, e) in &missed {
        let r = root(db, k, e);
        if r != k {
            under.push((r, (k, e)));
            continue;
        }
        let key = unit_key(k, e);
        units
            .entry(key)
            .or_insert_with(|| Finding {
                items: Vec::new(),
                inside: Vec::new(),
            })
            .items
            .push((k, e));
    }
    for (r, item) in under {
        let at = db.items.get(r).map(|e| unit_key(r, e));
        match at.and_then(|key| units.get_mut(&key)) {
            Some(u) => u.inside.push(item),
            // The cause is filtered out of this report: the item stands alone.
            None => {
                let key = unit_key(item.0, item.1);
                units
                    .entry(key)
                    .or_insert_with(|| Finding {
                        items: Vec::new(),
                        inside: Vec::new(),
                    })
                    .items
                    .push(item);
            }
        }
    }
    let mut list: Vec<Finding> = units.into_values().collect();
    list.sort_by_key(|f| {
        let e = f.items[0].1;
        (usize::MAX - f.items.len() - f.inside.len(), e.line)
    });
    let mut s = String::new();
    for f in &list {
        s.push_str(entry(db, f, shown, color).trim_end());
        s.push_str("\n\n");
    }
    s
}

/// What groups items into one finding: a signal, a decision, or each on its own.
fn unit_key(k: &Key, e: &Entry) -> (u8, u32, String) {
    match k.kind {
        Kind::Statement => (0, e.line, format!("{:x}.{}", k.hash, k.occ)),
        Kind::Branch(a) => (1, e.line, format!("{:x}.{}.{a}", k.hash, k.occ)),
        Kind::Toggle(_) => {
            let base = indexed(&k.sig).map_or(k.sig.as_str(), |x| x.0);
            (2, e.line, base.to_string())
        }
        Kind::Condition(_) => (3, e.line, format!("{:x}.{}", k.hash, k.occ)),
        Kind::Expression(_) => (4, e.line, format!("{:x}.{}", k.hash, k.occ)),
        Kind::Combination(_) => (5, e.line, format!("{:x}.{}", k.hash, k.occ)),
        Kind::Fsm(_) => (6, e.line, k.sig.clone()),
    }
}

/// One finding, rendered as a diagnostic.
fn entry(db: &Database, f: &Finding, shown: &Shown, color: bool) -> String {
    let (k, e) = f.items[0];
    let (title, mark) = match k.kind {
        Kind::Statement => (format!("statement never ran: `{}`", e.text), "never ran"),
        Kind::Branch(_) => (
            format!("branch arm never taken: `{}`", e.text),
            "never taken",
        ),
        Kind::Toggle(_) => (
            format!(
                "signal did not toggle fully: `{}`",
                indexed(&k.sig).map_or(k.sig.as_str(), |x| x.0)
            ),
            "declared here",
        ),
        Kind::Condition(_) => (
            format!("condition not fully covered: `{}`", whole(e)),
            "here",
        ),
        Kind::Expression(_) => (
            format!("expression not fully covered: `{}`", whole(e)),
            "here",
        ),
        Kind::Combination(_) => (format!("combination never seen in `{}`", e.text), "here"),
        Kind::Fsm(false) => (
            format!("state never reached: `{}`", crate::fsm_text(k)),
            "here",
        ),
        Kind::Fsm(true) => (
            format!("transition never taken: `{}`", crate::fsm_text(k)),
            "here",
        ),
    };
    let mut d = Diagnostic::new(Severity::Note, "", title);
    d = match shown.span(e) {
        Ok(span) => d.with_primary(span, mark),
        Err(why) => {
            let at = if e.detail.col > 0 {
                format!("{}:{}:{}", e.file, e.detail.line, e.detail.col)
            } else {
                format!("{}:{}", e.file, e.line)
            };
            d.with_note(format!("at {at}; source not shown: {why}"))
        }
    };
    d = match k.kind {
        Kind::Statement => statement_entry(db, k, e, d),
        Kind::Branch(_) => arm_entry(db, k, e, d),
        Kind::Condition(_) | Kind::Expression(_) => rows_entry(db, &f.items, d),
        Kind::Toggle(_) => toggle_entry(&f.items, d),
        Kind::Fsm(_) => fsm_entry(k, d),
        Kind::Combination(_) => {
            for (k, _) in &f.items {
                d = d.with_note(format!("never seen: {}", crate::combination_words(k)));
            }
            d.with_help("drive the inputs to each combination above in one evaluation; `not evaluated` means an earlier input decided the result first")
        }
    };
    if !f.inside.is_empty() {
        let mut inside: Vec<_> = f.inside.iter().map(|(k, e)| label(k, e)).collect();
        inside.sort();
        d = d.with_note(format!(
            "this explains {} more, which cannot run until it does:\n{}",
            count(f.inside.len(), "item", "items"),
            inside.join("\n")
        ));
    }
    yavs_diag::render(&shown.map, &d, MessageFormat::Human, color)
}

/// The summary at the end of an instance: what is missing, per kind.
fn closing(name: &str, inst: &Instance) -> String {
    let mut per: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut excluded = 0;
    for (k, e, why) in &inst.items {
        if why.is_some() {
            excluded += 1;
            continue;
        }
        let c = per.entry(k.kind.name()).or_default();
        c.1 += 1;
        c.0 += usize::from(e.count == 0);
    }
    let missed: usize = per.values().map(|x| x.0).sum();
    let all: usize = per.values().map(|x| x.1).sum();
    let parts: Vec<String> = per
        .iter()
        .filter(|x| x.1.0 > 0)
        .map(|(k, x)| format!("{} {k}", x.0))
        .collect();
    let mut s = format!(
        "{name}: {missed} of {} not covered",
        count(all, "item", "items")
    );
    if !parts.is_empty() {
        let _ = write!(s, " ({})", parts.join(", "));
    }
    if excluded > 0 {
        let _ = write!(s, ", {excluded} excluded");
    }
    s.push('\n');
    s
}

/// The whole view for the paths of `order`, in that order.
pub(crate) fn report(db: &Database, order: &[&str], o: &SummaryOptions) -> String {
    let shown = Shown::new(o.sources);
    let excluded = o.excluded.unwrap_or(&[]);
    let mut s = String::new();
    for path in order {
        let mut items: Vec<Item> = db
            .items
            .iter()
            .filter(|(k, _)| k.path == *path)
            .map(|(k, e)| (k, e, None))
            .collect();
        items.extend(
            excluded
                .iter()
                .filter(|x| x.0.path == *path)
                .map(|x| (&x.0, &x.1, Some(x.2.as_str()))),
        );
        let inst = Instance { db, items };
        let name = yavs_diag::dotted(path);
        let files: BTreeSet<&str> = inst.items.iter().map(|x| x.1.file.as_str()).collect();
        let files: Vec<&str> = files.into_iter().collect();
        let _ = writeln!(s, "\n=== {name}  ({}) ===\n", files.join(", "));
        inst.item_table(&mut s, "statement", |k| matches!(k, Kind::Statement));
        inst.item_table(&mut s, "branch", |k| matches!(k, Kind::Branch(_)));
        inst.toggle_table(&mut s);
        inst.focused_tables(&mut s, "condition", "condition", |k| {
            matches!(k, Kind::Condition(_))
        });
        inst.focused_tables(&mut s, "expression", "expression", |k| {
            matches!(k, Kind::Expression(_))
        });
        inst.combination_tables(&mut s);
        inst.item_table(&mut s, "fsm", |k| matches!(k, Kind::Fsm(_)));
        s.push_str(&entries(&inst, &shown, o.color));
        s.push_str(&closing(&name, &inst));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Detail;

    fn item(path: &str, kind: Kind, sig: &str, line: u32, text: &str, count: u64) -> (Key, Entry) {
        let key = Key {
            path: path.into(),
            kind,
            hash: 1,
            occ: 0,
            sig: sig.into(),
        };
        let entry = Entry {
            file: "a.vhd".into(),
            line,
            count,
            text: text.into(),
            tests: Vec::new(),
            waivers: Vec::new(),
            detail: Detail::default(),
            unit: String::new(),
            process: String::new(),
            operands: Vec::new(),
        };
        (key, entry)
    }

    fn database(items: Vec<(Key, Entry)>) -> Database {
        let mut d = Database::default();
        d.items.extend(items);
        d
    }

    fn detailed(d: &Database, below: Option<u64>) -> String {
        d.summary_with(&SummaryOptions {
            below,
            detailed: true,
            ..SummaryOptions::default()
        })
    }

    #[test]
    fn the_default_has_tables_and_no_rows() {
        let d = database(vec![
            item(":t:u1", Kind::Statement, "", 3, "x <= 1;", 0),
            item(":t:u1", Kind::Toggle(true), "s", 2, "", 0),
            item("work.p", Kind::Statement, "", 9, "y := 2;", 0),
        ]);
        assert_eq!(
            d.summary(),
            "Kind       Items  Hit  Missed  Coverage\n\
             ---------  -----  ---  ------  --------\n\
             statement      2    0       2      0.0%\n\
             toggle         1    0       1      0.0%\n\
             total          3    0       3      0.0%\n\n\
             Instance  Total     statement  toggle\n\
             --------  --------  ---------  --------\n\
             t.u1      0/2 0.0%  0/1 0.0%   0/1 0.0%\n\
             work.p    0/1 0.0%  0/1 0.0%   -\n\n\
             3 items not covered; `--detailed` shows them.\n"
        );
        assert_eq!(Database::default().summary(), "no coverage items\n");
        let hit = database(vec![item(":t", Kind::Statement, "", 3, "x;", 1)]);
        assert!(
            hit.summary().ends_with("\nevery item is covered\n"),
            "{}",
            hit.summary()
        );
    }

    #[test]
    fn toggle_runs_share_a_row_by_direction_and_count() {
        let mut v = Vec::new();
        for i in [0, 1, 2, 5] {
            v.push(item(":t", Kind::Toggle(true), &format!("d({i})"), 2, "", 0));
        }
        v.push(item(":t", Kind::Toggle(true), "d(7)", 2, "", 3));
        let r = detailed(&database(v), None);
        assert!(r.contains("d(0..2, 5) rise (0->1)"), "{r}");
        assert!(r.contains("d(7) rise (0->1)"), "{r}");
        // Not all bits are missed, so the rows name the missed bits, never the glob.
        assert!(r.contains("toggle t d(5) rise"), "{r}");
        assert!(!r.contains("d(*)"), "{r}");
    }

    #[test]
    fn a_stuck_signal_says_so_and_only_what_the_data_shows() {
        let with = |level| {
            let mut v = Vec::new();
            for rise in [false, true] {
                let mut it = item(":t", Kind::Toggle(rise), "s", 2, "", 0);
                it.1.detail.level = level;
                v.push(it);
            }
            detailed(&database(v), None)
        };
        assert!(with('0').contains("s: no transition, and it ended at 0: stuck at 0"));
        assert!(with('1').contains("s: no transition, and it ended at 1: stuck at 1"));
        assert!(with('-').contains("it ended neither 0 nor 1, so it may never have been driven"));
        assert!(with('?').contains("the level differs between runs or is not recorded"));
    }

    #[test]
    fn a_focused_table_marks_the_missed_row_and_a_covered_decision_is_one_line() {
        let mut v = Vec::new();
        for (r, sig, c) in [(0, "a", 4), (1, "a", 0), (2, "b", 3), (3, "b", 2)] {
            let mut it = item(":t", Kind::Condition(r), sig, 7, "? a and b", c);
            it.1.detail.needs = if sig == "a" { "b is 1" } else { "a is 1" }.to_string();
            v.push(it);
        }
        let r = detailed(&database(v), None);
        assert!(
            r.contains(
                "Line 7: a and b   (condition)\nInput totals: 1 of 2 inputs covered, 50.0%\n"
            ),
            "{r}"
        );
        assert!(r.contains("value 1 not hit"), "{r}");
        assert!(r.contains("make `a` 1 with b is 1"), "{r}");
        assert!(r.contains("a is 1  b is 1"), "{r}");
        assert!(r.contains("<- missed\n"), "{r}");
        let all: Vec<_> = (0..2)
            .map(|x| item(":t", Kind::Expression(x), "a", 7, "? a or b", 1))
            .collect();
        let r = detailed(&database(all), None);
        assert!(
            r.contains("Line 7: a or b   (expression)  all 2 rows hit"),
            "{r}"
        );
        assert!(!r.contains("Input totals"), "{r}");
    }

    #[test]
    fn a_cause_groups_what_it_explains_and_worst_comes_first() {
        let mut a = item(":t", Kind::Branch(0), "", 5, "if z", 0);
        a.0.hash = 7;
        let mut other = item(":t", Kind::Branch(1), "", 5, "else", 9);
        other.0.hash = 7;
        let mut s1 = item(":t", Kind::Statement, "", 6, "p <= 1;", 0);
        s1.1.detail.under = Some((7, 0, 0));
        s1.0.hash = 8;
        let mut s2 = item(":t", Kind::Statement, "", 7, "q <= 1;", 0);
        s2.1.detail.under = Some((7, 0, 0));
        s2.0.hash = 9;
        let lone = item(":t", Kind::Statement, "", 2, "r <= 1;", 0);
        let r = detailed(&database(vec![a, other, s1, s2, lone]), None);
        assert!(r.contains("this explains 2 items more"), "{r}");
        assert!(r.contains("line 6: statement `p <= 1;`"), "{r}");
        assert!(!r.contains("statement never ran: `p <= 1;`"), "{r}");
        assert!(
            r.find("branch arm never taken").unwrap() < r.find("statement never ran").unwrap(),
            "{r}"
        );
        assert!(
            r.contains("source not shown: the database holds no position in the file for it"),
            "{r}"
        );
    }

    #[test]
    fn below_lists_the_worst_first_and_counts_the_rest() {
        let t = |p: &str, s: &str, c| item(p, Kind::Toggle(true), s, 2, "", c);
        // u1 1/2 (50.0%), u2 0/1, u3 1/1, u4 2/3 (66.6%, rounded down).
        let d = database(vec![
            t(":t:u1", "a", 1),
            t(":t:u1", "b", 0),
            t(":t:u2", "a", 0),
            t(":t:u3", "a", 1),
            t(":t:u4", "a", 1),
            t(":t:u4", "b", 1),
            t(":t:u4", "c", 0),
        ]);
        let below = |p| {
            d.summary_with(&SummaryOptions {
                below: Some(p),
                ..SummaryOptions::default()
            })
        };
        let s = below(667);
        let order: Vec<_> = s
            .lines()
            .filter(|l| l.starts_with("t.u"))
            .map(|l| &l[..4])
            .collect();
        assert_eq!(order, ["t.u2", "t.u1", "t.u4"], "{s}");
        assert!(s.contains("1 of 4 at or above 66.7% left out"), "{s}");
        // 66.6 is not under 66.6: rounded down before the compare.
        let s = below(666);
        assert!(!s.contains("t.u4  "), "{s}");
        assert!(s.contains("2 of 4 at or above 66.6% left out"), "{s}");
        // Nothing under the limit; the totals do not move.
        let s = below(0);
        assert!(
            s.contains("no instance or package is below 0.0%") && !s.contains("Instance"),
            "{s}"
        );
        assert!(
            s.contains("\ntotal       7    4       3     57.1%\n"),
            "{s}"
        );
        // The sections follow the limit, in its order; one at 100% has none.
        let s = detailed(&d, Some(1000));
        assert!(
            s.find("=== t.u2").unwrap() < s.find("=== t.u1").unwrap(),
            "{s}"
        );
        assert!(!s.contains("=== t.u3"), "{s}");
    }

    #[test]
    fn below_100_lists_every_instance_not_fully_covered() {
        let mut v = Vec::new();
        // 1999 of 2000 is 99.95%: it rounds down to 99.9% and is listed, never up to 100.
        for i in 0..2000 {
            v.push(item(
                ":t:almost",
                Kind::Toggle(true),
                &format!("s({i})"),
                2,
                "",
                u64::from(i != 0),
            ));
        }
        v.push(item(":t:full", Kind::Statement, "", 3, "x <= 1;", 1));
        // Toggles only here: nothing to count once `--kind=statement` narrows the report.
        v.push(item(":t:empty", Kind::Toggle(true), "s", 2, "", 0));
        let d = database(v);
        let s = d.summary_with(&SummaryOptions {
            below: Some(1000),
            ..SummaryOptions::default()
        });
        let line = s
            .lines()
            .find(|l| l.starts_with("t.almost "))
            .expect("listed");
        assert!(line.contains("1999/2000 99.9%"), "{s}");
        assert!(!s.contains("t.full"), "{s}");
        assert!(s.contains("1 of 3 at or above 100.0% left out"), "{s}");
        let only = d.only(&["statement".to_string()]);
        let s = only.summary_with(&SummaryOptions {
            below: Some(1000),
            ..SummaryOptions::default()
        });
        assert!(s.contains("no instance or package is below 100.0%"), "{s}");
        assert!(!s.contains("t.empty") && !s.contains("t.almost"), "{s}");
    }

    #[test]
    fn excluded_items_show_as_excluded() {
        let d = database(vec![item(":t", Kind::Statement, "", 3, "x;", 1)]);
        let (k, e) = item(":t", Kind::Statement, "", 4, "y;", 0);
        let ex = vec![(k, e, "debug only".to_string())];
        let s = d.summary_with(&SummaryOptions {
            detailed: true,
            excluded: Some(&ex),
            ..SummaryOptions::default()
        });
        assert!(s.contains("excluded: debug only"), "{s}");
        assert!(!s.contains("MISSED"), "{s}");
        assert!(s.contains("t: 0 of 1 item not covered, 1 excluded"), "{s}");
    }
}
