//! The unified exclusion report (`yavs cov report --exclusions`): every rule that takes
//! requirements out of the totals, in-source directives and exclusion-file rows alike, with what
//! it matched, how often those ran, and the totals before and after. An item more than one rule
//! matches is listed under each of them and counted once.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::exclude::Applied;
use crate::{Database, Entry, Key, Kind, percent};

/// What became of a rule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// It excluded at least one requirement no earlier rule did.
    Applied,
    /// Every requirement it matches an earlier rule excludes too.
    Redundant,
    /// The database has requirements of its kind, none of them match.
    MatchesNothing,
    /// The database has no requirement of its kind: the metric was not collected.
    NotCollected,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Status::Applied => "applied",
            Status::Redundant => "redundant (every requirement is excluded by an earlier rule)",
            Status::MatchesNothing => "matches nothing",
            Status::NotCollected => "its metric was not collected in these runs",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Status::Applied => "applied",
            Status::Redundant => "redundant",
            Status::MatchesNothing => "matches-nothing",
            Status::NotCollected => "metric-not-collected",
        }
    }
}

/// A rule with its status and matches.
pub struct RuleReport {
    pub origin: String,
    pub inline: bool,
    pub rule: String,
    pub reason: String,
    pub status: Status,
    pub items: Vec<(Key, Entry)>,
}

/// Requirements of one kind before and after the exclusions.
#[derive(Default, Clone, Copy)]
pub struct KindTotals {
    pub before: usize,
    pub before_hit: usize,
    pub excluded: usize,
    pub excluded_ran: usize,
}

pub struct Report {
    pub rules: Vec<RuleReport>,
    pub totals: BTreeMap<&'static str, KindTotals>,
    pub tests: Vec<String>,
    /// A database written before in-source exclusions were recorded is part of it.
    pub legacy: bool,
}

/// The kind a rule is about, from the first word of its text, if it names one.
fn rule_kind(rule: &str) -> Option<&'static str> {
    let w = rule.split_whitespace().next()?;
    [
        "statement",
        "branch",
        "condition",
        "expression",
        "combination",
        "toggle",
    ]
    .into_iter()
    .find(|k| *k == w)
}

/// The report of what `a` (the result of [`crate::exclude::apply`] on `raw`) did.
pub fn build(raw: &Database, a: &Applied) -> Report {
    let entries: BTreeMap<&Key, &Entry> = a
        .kept
        .items
        .iter()
        .chain(a.excluded.iter().map(|(k, e, _)| (k, e)))
        .collect();
    let present: BTreeSet<&str> = raw.items.keys().map(|k| k.kind.name()).collect();
    let mut totals: BTreeMap<&'static str, KindTotals> = BTreeMap::new();
    for (k, e) in raw.items.iter() {
        let t = totals.entry(k.kind.name()).or_default();
        t.before += 1;
        t.before_hit += usize::from(e.count > 0);
    }
    for (k, e, _) in &a.excluded {
        let t = totals.entry(k.kind.name()).or_default();
        t.excluded += 1;
        t.excluded_ran += usize::from(e.count > 0);
    }
    let mut seen: BTreeSet<&Key> = BTreeSet::new();
    let mut rules = Vec::new();
    for u in &a.uses {
        let fresh = u.items.iter().filter(|k| seen.insert(k)).count();
        let kind = if u.inline {
            Some("combination")
        } else {
            rule_kind(&u.rule)
        };
        let status = match (u.items.is_empty(), fresh) {
            (false, 0) => Status::Redundant,
            (false, _) => Status::Applied,
            (true, _) if kind.is_some_and(|k| !present.contains(k)) => Status::NotCollected,
            (true, _) => Status::MatchesNothing,
        };
        rules.push(RuleReport {
            origin: u.origin.clone(),
            inline: u.inline,
            rule: u.rule.clone(),
            reason: u.reason.clone(),
            status,
            items: u
                .items
                .iter()
                .filter_map(|k| entries.get(k).map(|e| (k.clone(), (*e).clone())))
                .collect(),
        });
    }
    Report {
        rules,
        totals,
        tests: raw.tests.clone(),
        legacy: raw.legacy,
    }
}

/// An item in words, for the lists of matches.
fn item_words(k: &Key, e: &Entry) -> String {
    match k.kind {
        Kind::Combination(_) => format!("{}  in {}", crate::combination_words(k), e.text),
        Kind::Condition(_) | Kind::Expression(_) => crate::fec_text(k, e),
        Kind::Toggle(r) => format!("{} {}", k.sig, crate::arrow(r)),
        Kind::Statement => e.text.clone(),
        Kind::Branch(_) => e.text.clone(),
    }
}

fn tests_of(r: &Report, e: &Entry) -> String {
    let t: Vec<&str> = e
        .tests
        .iter()
        .filter_map(|&i| r.tests.get(i as usize).map(String::as_str))
        .collect();
    t.join(", ")
}

/// The most matches listed for a rule.
const SHOWN: usize = 12;

impl Report {
    /// Show the source paths through `f` (relative to a root).
    pub fn map_files(&mut self, f: &dyn Fn(&str) -> String) {
        for r in &mut self.rules {
            if r.inline
                && let Some((file, line)) = r.origin.rsplit_once(':')
            {
                r.origin = format!("{}:{line}", f(file));
            }
            for (_, e) in &mut r.items {
                e.file = f(&e.file);
            }
        }
    }

    /// The totals as `hit/all pct`.
    fn col(h: usize, t: usize) -> String {
        format!("{h}/{t} {}", percent(h, t))
    }

    /// The text report.
    pub fn text(&self) -> String {
        let mut s = String::new();
        let n: usize = self.totals.values().map(|t| t.excluded).sum();
        let _ = writeln!(
            s,
            "coverage exclusions: {} rule{}, {n} requirement{} excluded",
            self.rules.len(),
            if self.rules.len() == 1 { "" } else { "s" },
            if n == 1 { "" } else { "s" }
        );
        if self.legacy {
            s.push_str(
                "note: a database written by an older yavs is included; it does not record \
                 in-source exclusions, so none are shown for it\n",
            );
        }
        for (i, r) in self.rules.iter().enumerate() {
            let _ = writeln!(
                s,
                "\n{:>3}. {}  ({})  {}",
                i + 1,
                r.origin,
                if r.inline {
                    "in the source"
                } else {
                    "exclusion file"
                },
                r.status.word()
            );
            let _ = writeln!(s, "       rule:    {}", r.rule);
            if !r.reason.is_empty() {
                let _ = writeln!(s, "       reason:  {}", r.reason);
            }
            if r.items.is_empty() {
                continue;
            }
            let ran = r.items.iter().filter(|x| x.1.count > 0).count();
            let _ = writeln!(
                s,
                "       matches: {} requirement{}, {ran} of them ran",
                r.items.len(),
                if r.items.len() == 1 { "" } else { "s" }
            );
            let w = r
                .items
                .iter()
                .take(SHOWN)
                .map(|(k, _)| k.path.chars().count())
                .max()
                .unwrap_or(0);
            for (k, e) in r.items.iter().take(SHOWN) {
                let t = tests_of(self, e);
                let _ = writeln!(
                    s,
                    "         {:w$}  {}:{}  {}  [{} hit{}{}]",
                    k.path,
                    e.file,
                    e.line,
                    item_words(k, e),
                    e.count,
                    if e.count == 1 { "" } else { "s" },
                    if t.is_empty() {
                        String::new()
                    } else {
                        format!(", tests: {t}")
                    }
                );
            }
            if r.items.len() > SHOWN {
                let _ = writeln!(s, "         ... and {} more", r.items.len() - SHOWN);
            }
        }
        if !self.totals.is_empty() {
            s.push('\n');
            let rows: Vec<[String; 5]> = self
                .totals
                .iter()
                .map(|(k, t)| {
                    let after = t.before - t.excluded;
                    let after_hit = t.before_hit - t.excluded_ran.min(t.before_hit);
                    [
                        (*k).to_string(),
                        Self::col(t.before_hit, t.before),
                        format!("{} ({} ran)", t.excluded, t.excluded_ran),
                        Self::col(after_hit, after),
                        String::new(),
                    ]
                })
                .collect();
            let head = ["kind", "before", "excluded", "after", ""];
            let w: Vec<usize> = (0..4)
                .map(|c| {
                    rows.iter()
                        .map(|r| r[c].chars().count())
                        .chain([head[c].len()])
                        .max()
                        .unwrap_or(0)
                })
                .collect();
            let _ = writeln!(
                s,
                "{:<w0$}  {:<w1$}  {:<w2$}  {}",
                head[0],
                head[1],
                head[2],
                head[3],
                w0 = w[0],
                w1 = w[1],
                w2 = w[2]
            );
            for r in &rows {
                let _ = writeln!(
                    s,
                    "{:<w0$}  {:<w1$}  {:<w2$}  {}",
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    w0 = w[0],
                    w1 = w[1],
                    w2 = w[2]
                );
            }
        }
        s
    }

    /// The versioned JSON report.
    pub fn json(&self) -> String {
        let mut s = String::from("{\n  \"version\": 1,\n");
        let _ = writeln!(s, "  \"inline_recorded\": {},", !self.legacy);
        s.push_str("  \"rules\": [");
        for (i, r) in self.rules.iter().enumerate() {
            let _ = write!(
                s,
                "{}\n    {{\"origin\": {}, \"source\": \"{}\", \"rule\": {}, \"reason\": {}, \"status\": \"{}\", \"matched\": [",
                if i > 0 { "," } else { "" },
                json_str(&r.origin),
                if r.inline { "inline" } else { "file" },
                json_str(&r.rule),
                json_str(&r.reason),
                r.status.key()
            );
            for (j, (k, e)) in r.items.iter().enumerate() {
                let tests: Vec<String> = e
                    .tests
                    .iter()
                    .filter_map(|&t| self.tests.get(t as usize))
                    .map(|t| json_str(t))
                    .collect();
                let _ = write!(
                    s,
                    "{}\n      {{\"kind\": \"{}\", \"instance\": {}, \"location\": {}, \"hits\": {}, \"tests\": [{}]",
                    if j > 0 { "," } else { "" },
                    k.kind.name(),
                    json_str(&k.path),
                    json_str(&format!("{}:{}", e.file, e.line)),
                    e.count,
                    tests.join(", ")
                );
                if let Kind::Combination(_) = k.kind {
                    let ops: Vec<String> = crate::inputs(k).iter().map(|x| json_str(x)).collect();
                    let _ = write!(
                        s,
                        ", \"operands\": [{}], \"pattern\": {}, \"expression\": {}",
                        ops.join(", "),
                        json_str(&crate::pattern(k)),
                        json_str(&e.text)
                    );
                } else {
                    let _ = write!(s, ", \"item\": {}", json_str(&item_words(k, e)));
                }
                s.push('}');
            }
            s.push_str(if r.items.is_empty() { "]}" } else { "\n    ]}" });
        }
        s.push_str(if self.rules.is_empty() {
            "],\n"
        } else {
            "\n  ],\n"
        });
        s.push_str("  \"totals\": [");
        for (i, (k, t)) in self.totals.iter().enumerate() {
            let _ = write!(
                s,
                "{}\n    {{\"kind\": \"{k}\", \"before\": {}, \"before_hit\": {}, \"excluded\": {}, \"excluded_ran\": {}}}",
                if i > 0 { "," } else { "" },
                t.before,
                t.before_hit,
                t.excluded,
                t.excluded_ran
            );
        }
        s.push_str(if self.totals.is_empty() {
            "]\n}\n"
        } else {
            "\n  ]\n}\n"
        });
        s
    }

    /// The body of the HTML page (the caller wraps it).
    pub fn html_body(&self) -> String {
        let mut b = String::from("<h1>Coverage exclusions</h1>\n");
        if self.legacy {
            b.push_str(
                "<p class=\"dim\">A database written by an older yavs is included; it does not \
                 record in-source exclusions.</p>\n",
            );
        }
        if self.rules.is_empty() {
            b.push_str("<p>No exclusion rule is in use.</p>\n");
        }
        for (i, r) in self.rules.iter().enumerate() {
            let _ = write!(
                b,
                "<h2>{}. {} <span class=\"dim\">({}, {})</span></h2>\n<p><code>{}</code>",
                i + 1,
                crate::html::esc(&r.origin),
                if r.inline {
                    "in the source"
                } else {
                    "exclusion file"
                },
                crate::html::esc(r.status.word()),
                crate::html::esc(&r.rule)
            );
            if !r.reason.is_empty() {
                let _ = write!(b, "<br>reason: {}", crate::html::esc(&r.reason));
            }
            b.push_str("</p>\n");
            if r.items.is_empty() {
                continue;
            }
            b.push_str("<table>\n<tr><th>instance</th><th>location</th><th>requirement</th><th>hits</th><th>tests</th></tr>\n");
            for (k, e) in &r.items {
                let _ = writeln!(
                    b,
                    "<tr><td>{}</td><td>{}:{}</td><td>{}</td><td class=\"n\">{}</td><td class=\"dim\">{}</td></tr>",
                    crate::html::esc(&k.path),
                    crate::html::esc(&e.file),
                    e.line,
                    crate::html::esc(&item_words(k, e)),
                    e.count,
                    crate::html::esc(&tests_of(self, e))
                );
            }
            b.push_str("</table>\n");
        }
        b.push_str("<h2>Totals</h2>\n<table>\n<tr><th>kind</th><th>before</th><th>excluded</th><th>after</th></tr>\n");
        for (k, t) in &self.totals {
            let after = t.before - t.excluded;
            let after_hit = t.before_hit - t.excluded_ran.min(t.before_hit);
            let _ = writeln!(
                b,
                "<tr><td>{k}</td><td>{}</td><td>{} ({} ran)</td><td>{}</td></tr>",
                Self::col(t.before_hit, t.before),
                t.excluded,
                t.excluded_ran,
                Self::col(after_hit, after)
            );
        }
        b.push_str("</table>\n");
        b
    }
}

/// `s` as a JSON string.
fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
