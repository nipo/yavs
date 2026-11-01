//! The exports of `yavs cov report`: versioned JSON with every kind, LCOV and SonarQube generic
//! coverage. Excluded items are left out, as in Cobertura; paths are as the caller shows them.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::exclusions::json_str;
use crate::{Database, Kind, tenths, xml};

/// Per source file and line: the hits of its most executed statement, if it has one, and per
/// decision on the line the counts of its arms, summed over instances.
#[derive(Default)]
struct Line {
    hits: Option<u64>,
    arms: BTreeMap<(u64, u32), BTreeMap<u32, u64>>,
}

fn lines(db: &Database) -> BTreeMap<&str, BTreeMap<u32, Line>> {
    let mut stmts: BTreeMap<(&str, u32, u64, u32), u64> = BTreeMap::new();
    let mut files: BTreeMap<&str, BTreeMap<u32, Line>> = BTreeMap::new();
    for (k, e) in &db.items {
        match k.kind {
            Kind::Statement => {
                let c = stmts.entry((&e.file, e.line, k.hash, k.occ)).or_default();
                *c = c.saturating_add(e.count);
            }
            Kind::Branch(a) => {
                let line = files.entry(&e.file).or_default().entry(e.line).or_default();
                let c = line
                    .arms
                    .entry((k.hash, k.occ))
                    .or_default()
                    .entry(a)
                    .or_default();
                *c = c.saturating_add(e.count);
            }
            _ => {}
        }
    }
    for ((f, l, ..), c) in stmts {
        let line = files.entry(f).or_default().entry(l).or_default();
        line.hits = Some(line.hits.unwrap_or(0).max(c));
    }
    files
}

impl Database {
    /// The LCOV tracefile: a line per statement line (`DA`), a branch per arm (`BRDA`). Toggles,
    /// conditions, expressions, combinations, instances and tests have no place in it.
    pub fn lcov(&self) -> String {
        let mut s = String::new();
        for (file, ls) in lines(self) {
            let _ = writeln!(s, "TN:\nSF:{file}");
            let (mut brf, mut brh) = (0, 0);
            for (n, l) in &ls {
                for (block, arms) in l.arms.values().enumerate() {
                    // `-` is a decision that never ran, 0 an arm it did not take.
                    let ran = arms.values().any(|&c| c > 0);
                    for (arm, c) in arms {
                        brf += 1;
                        brh += usize::from(*c > 0);
                        let taken = if ran { c.to_string() } else { "-".to_string() };
                        let _ = writeln!(s, "BRDA:{n},{block},{arm},{taken}");
                    }
                }
            }
            let (mut lf, mut lh) = (0, 0);
            for (n, l) in &ls {
                if let Some(h) = l.hits {
                    lf += 1;
                    lh += usize::from(h > 0);
                    let _ = writeln!(s, "DA:{n},{h}");
                }
            }
            let _ = writeln!(s, "LF:{lf}\nLH:{lh}\nBRF:{brf}\nBRH:{brh}\nend_of_record");
        }
        s
    }

    /// The SonarQube generic coverage report: a line is covered when a statement on it ran or,
    /// without a statement in the report, an arm did.
    pub fn sonarqube(&self) -> String {
        let mut s = String::from("<coverage version=\"1\">\n");
        for (file, ls) in lines(self) {
            let _ = writeln!(s, "  <file path=\"{}\">", xml(file));
            for (n, l) in &ls {
                let all = l.arms.values().flat_map(|a| a.values());
                let (mut to_cover, mut covered) = (0, 0);
                for c in all {
                    to_cover += 1;
                    covered += usize::from(*c > 0);
                }
                let hit = l.hits.map_or(covered > 0, |h| h > 0);
                let _ = write!(s, "    <lineToCover lineNumber=\"{n}\" covered=\"{hit}\"");
                if to_cover > 0 {
                    let _ = write!(
                        s,
                        " branchesToCover=\"{to_cover}\" coveredBranches=\"{covered}\""
                    );
                }
                s.push_str("/>\n");
            }
            s.push_str("  </file>\n");
        }
        s.push_str("</coverage>\n");
        s
    }

    /// The JSON report, schema version 1 (docs/coverage.md): the tests, totals per kind and every
    /// item that is counted.
    pub fn json(&self) -> String {
        let mut s = String::from("{\n  \"version\": 1,\n  \"tests\": [");
        let tests: Vec<String> = self.tests.iter().map(|t| json_str(t)).collect();
        let _ = writeln!(s, "{}],", tests.join(", "));
        let mut per: BTreeMap<(u8, &str), (usize, usize)> = BTreeMap::new();
        for (k, e) in &self.items {
            let t = per.entry((rank(k.kind), k.kind.name())).or_default();
            t.0 += 1;
            t.1 += usize::from(e.count > 0);
        }
        s.push_str("  \"totals\": [\n");
        let (hit, all) = self.totals();
        let mut rows: Vec<String> = per
            .iter()
            .map(|((_, kind), (items, h))| total_row(kind, *items, *h))
            .collect();
        rows.push(total_row("total", all, hit));
        let _ = writeln!(s, "{}\n  ],", rows.join(",\n"));
        s.push_str("  \"items\": [\n");
        let mut items = self.items.iter().peekable();
        while let Some((k, e)) = items.next() {
            let tests: Vec<String> = e
                .tests
                .iter()
                .filter_map(|&t| self.tests.get(t as usize))
                .map(|t| json_str(t))
                .collect();
            let _ = write!(
                s,
                "    {{\"kind\": \"{}\", \"instance\": {}, \"file\": {}, \"line\": {}, \"column\": {}, \
                 \"hits\": {}, \"tests\": [{}]",
                k.kind.name(),
                json_str(&k.shown()),
                json_str(&e.file),
                if e.detail.line > 0 {
                    e.detail.line
                } else {
                    e.line
                },
                e.detail.col,
                e.count,
                tests.join(", ")
            );
            match k.kind {
                Kind::Statement => {
                    let _ = write!(s, ", \"statement\": {}", json_str(&e.text));
                }
                Kind::Branch(a) => {
                    let _ = write!(s, ", \"arm\": {a}, \"arm_text\": {}", json_str(&e.text));
                }
                Kind::Toggle(r) => {
                    let dir = if r { "rise" } else { "fall" };
                    let _ = write!(
                        s,
                        ", \"signal\": {}, \"direction\": \"{dir}\"",
                        json_str(&k.sig)
                    );
                }
                Kind::Condition(_) | Kind::Expression(_) => {
                    let v = k.kind.row().map_or(0, |r| u8::from(r.1));
                    let whole = e.text.split_once(' ').map_or(e.text.as_str(), |x| x.1);
                    let _ = write!(
                        s,
                        ", \"input\": {}, \"value\": {v}, \"expression_text\": {}, \"other_inputs\": {}",
                        json_str(&k.sig),
                        json_str(whole),
                        json_str(&e.detail.needs)
                    );
                }
                Kind::Fsm(_) => {
                    let (signal, item) = k.sig.split_once(':').unwrap_or(("", &k.sig));
                    let _ = write!(s, ", \"signal\": {}", json_str(signal));
                    match item.split_once("->") {
                        Some((from, to)) => {
                            let _ = write!(
                                s,
                                ", \"from\": {}, \"to\": {}",
                                json_str(from),
                                json_str(to)
                            );
                        }
                        None => {
                            let _ = write!(s, ", \"state\": {}", json_str(item));
                        }
                    }
                }
                Kind::Combination(_) => {
                    let ops: Vec<String> = crate::inputs(k).iter().map(|x| json_str(x)).collect();
                    let _ = write!(
                        s,
                        ", \"inputs\": [{}], \"pattern\": {}, \"expression_text\": {}",
                        ops.join(", "),
                        json_str(&crate::pattern(k)),
                        json_str(&e.text)
                    );
                }
            }
            s.push('}');
            s.push_str(if items.peek().is_some() { ",\n" } else { "\n" });
        }
        s.push_str("  ],\n  \"unmeasured\": [");
        let rows: Vec<String> = self
            .unmeasured
            .iter()
            .map(|(file, line, text)| {
                format!(
                    "\n    {{\"file\": {}, \"line\": {line}, \"text\": {}}}",
                    json_str(file),
                    json_str(text)
                )
            })
            .collect();
        s.push_str(&rows.join(","));
        s.push_str(if rows.is_empty() {
            "]\n}\n"
        } else {
            "\n  ]\n}\n"
        });
        s
    }
}

fn rank(k: Kind) -> u8 {
    match k {
        Kind::Statement => 0,
        Kind::Branch(_) => 1,
        Kind::Toggle(_) => 2,
        Kind::Condition(_) => 3,
        Kind::Expression(_) => 4,
        Kind::Combination(_) => 5,
        Kind::Fsm(_) => 6,
    }
}

/// A totals row; `coverage` rounds down like every percentage.
fn total_row(kind: &str, items: usize, hit: usize) -> String {
    let t = tenths(hit, items).unwrap_or(0);
    format!(
        "    {{\"kind\": \"{kind}\", \"items\": {items}, \"hit\": {hit}, \"missed\": {}, \"coverage\": {}.{}}}",
        items - hit,
        t / 10,
        t % 10
    )
}

#[cfg(test)]
mod tests {
    use crate::{Database, Detail, Entry, Key, Kind};

    fn db() -> Database {
        let mut d = Database::default();
        for (kind, hash, line, count, text) in [
            (Kind::Statement, 1, 3, 2, "x := 1;"),
            (Kind::Statement, 2, 4, 0, "y := 1;"),
            (Kind::Branch(0), 3, 5, 4, "if a"),
            (Kind::Branch(1), 3, 5, 0, "else"),
            (Kind::Branch(0), 4, 9, 0, "if b"),
            (Kind::Branch(1), 4, 9, 0, "(no arm taken)"),
        ] {
            let key = Key {
                path: ":t".into(),
                kind,
                hash,
                occ: 0,
                sig: String::new(),
            };
            let entry = Entry {
                file: "a&b.vhd".into(),
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
            d.items.insert(key, entry);
        }
        d
    }

    #[test]
    fn lcov_has_lines_branches_and_summaries() {
        assert_eq!(
            db().lcov(),
            "TN:\nSF:a&b.vhd\nBRDA:5,0,0,4\nBRDA:5,0,1,0\nBRDA:9,0,0,-\nBRDA:9,0,1,-\n\
             DA:3,2\nDA:4,0\nLF:2\nLH:1\nBRF:4\nBRH:1\nend_of_record\n"
        );
        assert_eq!(Database::default().lcov(), "");
    }

    #[test]
    fn sonarqube_follows_the_published_format() {
        let s = db().sonarqube();
        assert!(
            s.starts_with("<coverage version=\"1\">\n  <file path=\"a&amp;b.vhd\">\n"),
            "{s}"
        );
        assert!(
            s.contains("<lineToCover lineNumber=\"3\" covered=\"true\"/>"),
            "{s}"
        );
        assert!(
            s.contains("<lineToCover lineNumber=\"4\" covered=\"false\"/>"),
            "{s}"
        );
        assert!(
            s.contains(
                "lineNumber=\"5\" covered=\"true\" branchesToCover=\"2\" coveredBranches=\"1\""
            ),
            "{s}"
        );
        assert!(
            s.contains(
                "lineNumber=\"9\" covered=\"false\" branchesToCover=\"2\" coveredBranches=\"0\""
            ),
            "{s}"
        );
    }
}
