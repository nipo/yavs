//! `yavs-lint.yaml` (the `rule:` section) and `yavs-lint-waivers.yaml`. Unknown keys and rule ids
//! are errors naming them. Values are read as the text they were written as.

use crate::{Certainty, Rule};
use saphyr_parser::{Event, Parser};
use std::path::{Path, PathBuf};
use yavs_diag::Severity;
use yavs_diag::suggest::tail as near;

enum Node {
    Scalar(String),
    Seq(Vec<Node>),
    Map(Vec<(usize, String, Node)>),
}

type Events<'a> = Parser<'a, saphyr_parser::StrInput<'a>>;

fn next<'a>(p: &mut Events<'a>) -> Result<(Event<'a>, usize), String> {
    match p.next() {
        Some(Ok((ev, sp))) => Ok((ev, sp.start.line())),
        Some(Err(e)) => Err(format!("line {}: {}", e.marker().line(), e.info())),
        None => Err("unexpected end of the YAML stream".into()),
    }
}

fn node<'a>(p: &mut Events<'a>, ev: Event<'a>, line: usize) -> Result<Node, String> {
    match ev {
        Event::Scalar(v, ..) => Ok(Node::Scalar(v.to_string())),
        Event::SequenceStart(..) => {
            let mut v = Vec::new();
            loop {
                let (ev, l) = next(p)?;
                if matches!(ev, Event::SequenceEnd) {
                    return Ok(Node::Seq(v));
                }
                v.push(node(p, ev, l)?);
            }
        }
        Event::MappingStart(..) => {
            let mut v: Vec<(usize, String, Node)> = Vec::new();
            loop {
                let (ev, l) = next(p)?;
                let key = match ev {
                    Event::MappingEnd => return Ok(Node::Map(v)),
                    Event::Scalar(k, ..) => k.to_string(),
                    _ => return Err(format!("line {l}: a key must be a plain word")),
                };
                if v.iter().any(|(_, k, _)| *k == key) {
                    return Err(format!("line {l}: duplicate key `{key}`"));
                }
                let (ev, vl) = next(p)?;
                v.push((l, key, node(p, ev, vl)?));
            }
        }
        _ => Err(format!("line {line}: aliases are not supported")),
    }
}

/// The document's top-level map (empty for an empty file).
fn top(text: &str) -> Result<Vec<(usize, String, Node)>, String> {
    let mut p = Parser::new_from_str(text);
    let mut root = None;
    loop {
        let (ev, l) = next(&mut p)?;
        match ev {
            Event::StreamStart | Event::DocumentStart(_) | Event::DocumentEnd => {}
            Event::StreamEnd => break,
            ev if root.is_none() => root = Some((l, node(&mut p, ev, l)?)),
            _ => return Err(format!("line {l}: more than one YAML document")),
        }
    }
    match root {
        None => Ok(Vec::new()),
        Some((_, Node::Map(m))) => Ok(m),
        Some((_, Node::Scalar(s))) if s.is_empty() => Ok(Vec::new()),
        Some((l, _)) => Err(format!("line {l}: expected a map at the top level")),
    }
}

/// `disable` and `severity` as set at one level.
#[derive(Clone, Default)]
struct Layer {
    disable: Option<bool>,
    severity: Option<Severity>,
    /// `suffixes` of lint_602, `prefixes` of lint_603.
    list: Vec<String>,
}

fn layer(line: usize, name: &str, n: &Node) -> Result<Layer, String> {
    let Node::Map(fields) = n else {
        return Err(format!(
            "line {line}: the settings of `{name}` must be a map"
        ));
    };
    let mut out = Layer::default();
    for (l, key, v) in fields {
        if matches!(
            (name, key.as_str()),
            ("lint_602", "suffixes") | ("lint_603", "prefixes")
        ) {
            let Node::Seq(items) = v else {
                return Err(format!("line {l}: `{name}.{key}` must be a list"));
            };
            for n in items {
                let Node::Scalar(s) = n else {
                    return Err(format!("line {l}: `{name}.{key}` must be a list of words"));
                };
                out.list.push(s.clone());
            }
            continue;
        }
        let text = match v {
            Node::Scalar(s) => s.as_str(),
            _ => return Err(format!("line {l}: `{name}.{key}` must be a single value")),
        };
        let bad = || format!("line {l}: `{name}.{key}` has the invalid value `{text}`");
        match key.as_str() {
            "disable" => {
                out.disable = Some(match text {
                    "true" | "True" => true,
                    "false" | "False" => false,
                    _ => return Err(bad()),
                })
            }
            "severity" => {
                out.severity = Some(match text.to_ascii_lowercase().as_str() {
                    "error" => Severity::Error,
                    "warning" => Severity::Warning,
                    other => {
                        let tail = near(other, "severity", ["error", "warning"].into_iter());
                        return Err(bad() + &tail);
                    }
                })
            }
            _ => {
                let keys = ["disable", "severity", "suffixes", "prefixes"];
                return Err(format!(
                    "line {l}: unknown key `{key}` in the settings of `{name}` (expected `disable` or `severity`, and on lint_602 `suffixes`, on lint_603 `prefixes`){}",
                    near(key, "setting", keys.into_iter())
                ));
            }
        }
    }
    Ok(out)
}

/// Which rules run, and at which severity. A rule's own setting wins over its class's, which wins
/// over `rule.global`; with none, the definite class is on and everything reports as an error.
#[derive(Default)]
pub struct Config {
    global: Layer,
    groups: Vec<(Certainty, Layer)>,
    rules: Vec<(&'static str, Layer)>,
    /// `exclude`: paths or globs of what is not checked, relative to the configuration file.
    exclude: Vec<String>,
}

impl Config {
    /// Read a `yavs-lint.yaml`; the error names the line and the unknown key or rule.
    pub fn parse(text: &str) -> Result<Config, String> {
        let mut cfg = Config::default();
        for (l, key, v) in top(text)? {
            if key == "exclude" {
                let items = match v {
                    Node::Seq(items) => items,
                    Node::Scalar(s) if s.is_empty() => Vec::new(),
                    _ => return Err(format!("line {l}: `exclude` must be a list")),
                };
                for n in items {
                    let Node::Scalar(s) = n else {
                        return Err(format!(
                            "line {l}: `exclude` must be a list of paths or globs"
                        ));
                    };
                    cfg.exclude
                        .push(normalize(&s).trim_end_matches('/').to_owned());
                }
                continue;
            }
            if key != "rule" {
                return Err(format!(
                    "line {l}: unknown key `{key}` (expected `rule` or `exclude`){}",
                    near(&key, "key", ["rule", "exclude"].into_iter())
                ));
            }
            let Node::Map(rules) = &v else {
                return Err(format!("line {l}: `rule` must be a map"));
            };
            for (l, id, v) in rules {
                match id.as_str() {
                    "global" => cfg.global = layer(*l, id, v)?,
                    "group" => {
                        let Node::Map(groups) = v else {
                            return Err(format!("line {l}: `rule.group` must be a map"));
                        };
                        for (l, name, v) in groups {
                            let class = [
                                Certainty::Definite,
                                Certainty::Advisory,
                                Certainty::Experimental,
                                Certainty::Policy,
                            ]
                            .into_iter()
                            .find(|c| c.name() == name)
                            .ok_or_else(|| {
                                let groups = ["definite", "advisory", "experimental", "policy"];
                                format!(
                                    "line {l}: unknown rule group `{name}` (expected definite, advisory, experimental or policy){}",
                                    near(name, "rule group", groups.into_iter())
                                )
                            })?;
                            cfg.groups.push((class, layer(*l, name, v)?));
                        }
                    }
                    _ => {
                        let rule = crate::rule(id).ok_or_else(|| {
                            format!(
                                "line {l}: unknown rule `{id}` (`yavs lint --list-rules` lists the rules){}",
                                near_rule(id)
                            )
                        })?;
                        cfg.rules.push((rule.id, layer(*l, id, v)?));
                    }
                }
            }
        }
        Ok(cfg)
    }

    fn layers(&self, r: &Rule) -> [Layer; 3] {
        let own = self.rules.iter().rev().find(|(id, _)| *id == r.id);
        let class = self.groups.iter().rev().find(|(c, _)| *c == r.certainty);
        [
            own.map_or(Layer::default(), |x| x.1.clone()),
            class.map_or(Layer::default(), |x| x.1.clone()),
            self.global.clone(),
        ]
    }

    /// Whether `exclude` covers this file, `base` being the directory of the configuration file, or
    /// a directory the file is in. Both paths are absolute.
    pub fn excluded(&self, file: &Path, base: &Path) -> bool {
        if self.exclude.is_empty() {
            return false;
        }
        let place = Place::new(file, base);
        self.exclude.iter().any(|p| place.covered_by(p))
    }

    /// The list a rule's own settings give (`suffixes`, `prefixes`), empty when not set.
    pub(crate) fn option_list(&self, id: &str) -> &[String] {
        self.rules
            .iter()
            .rev()
            .find(|(r, _)| *r == id)
            .map_or(&[], |x| &x.1.list)
    }

    /// Whether anything but the definite class runs.
    pub(crate) fn beyond_definite(&self) -> bool {
        crate::RULES
            .iter()
            .any(|r| r.certainty != Certainty::Definite && self.enabled(r))
    }

    /// Whether this configuration runs the rule.
    pub fn enabled(&self, r: &Rule) -> bool {
        match self.layers(r).iter().find_map(|l| l.disable) {
            Some(disable) => !disable,
            None => r.certainty == Certainty::Definite,
        }
    }

    /// The severity of the rule's findings.
    pub fn severity(&self, r: &Rule) -> Severity {
        self.layers(r)
            .iter()
            .find_map(|l| l.severity)
            .unwrap_or(Severity::Error)
    }
}

/// A file as a pattern of `exclude` or a waiver sees it.
struct Place {
    /// Relative to the directory of the file holding the pattern, when the file is under it.
    relative: Option<String>,
    absolute: String,
}

impl Place {
    fn new(file: &Path, base: &Path) -> Place {
        let (file, base) = (lexical(file), lexical(base));
        Place {
            relative: file
                .strip_prefix(&base)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/")),
            absolute: file.to_string_lossy().replace('\\', "/"),
        }
    }

    /// Whether the pattern names the file or a directory above it. A pattern is relative to the
    /// base; one that starts with `/` or `**` also matches the absolute path, for files outside.
    fn covered_by(&self, pattern: &str) -> bool {
        let pattern = pattern.trim_end_matches('/');
        let covers = |path: &str| {
            let mut upto = path;
            loop {
                if glob(pattern.as_bytes(), upto.as_bytes()) {
                    return true;
                }
                match upto.rfind('/') {
                    Some(i) if i > 0 => upto = &upto[..i],
                    _ => return false,
                }
            }
        };
        self.relative.as_deref().is_some_and(covers)
            || ((pattern.starts_with('/')
                || pattern.starts_with("**")
                || pattern.as_bytes().get(1) == Some(&b':'))
                && covers(&self.absolute))
    }
}

struct Waiver {
    /// Rule ids, or `*`.
    rules: Vec<String>,
    /// The line of the pattern in the waiver file, and the pattern. Empty: every file.
    files: Vec<(usize, String)>,
    /// What `files` does not cover after all: it wins.
    files_exclude: Vec<(usize, String)>,
    /// Empty: the whole file.
    lines: Vec<usize>,
}

/// Findings a project has decided to accept, with a reason.
#[derive(Default)]
pub struct Waivers(Vec<Waiver>);

/// The tail of an error about the unknown rule `id`.
fn near_rule(id: &str) -> String {
    crate::similar_rule(id).map_or_else(String::new, |r| {
        format!("; {}", yavs_diag::suggest::help(&[(r.into(), "rule")]))
    })
}

fn normalize(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches("./").to_owned()
}

/// `*` and `?` stop at `/`, `**` does not.
pub(crate) fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern {
        [] => text.is_empty(),
        [b'*', b'*', rest @ ..] => {
            let rest = rest.strip_prefix(b"/").unwrap_or(rest);
            (0..=text.len()).any(|i| glob(rest, &text[i..]))
        }
        [b'*', rest @ ..] => (0..=text.len())
            .take_while(|&i| i == 0 || text[i - 1] != b'/')
            .any(|i| glob(rest, &text[i..])),
        [b'?', rest @ ..] => text.first().is_some_and(|&c| c != b'/') && glob(rest, &text[1..]),
        [c, rest @ ..] => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

impl Waivers {
    /// Read a `yavs-lint-waivers.yaml`; the error names the line.
    pub fn parse(text: &str) -> Result<Waivers, String> {
        let mut out = Vec::new();
        for (l, key, v) in top(text)? {
            if key != "waivers" {
                return Err(format!(
                    "line {l}: unknown key `{key}` (expected `waivers`){}",
                    near(&key, "key", ["waivers"].into_iter())
                ));
            }
            let items = match v {
                Node::Seq(items) => items,
                // `waivers:` with every entry commented out.
                Node::Scalar(s) if s.is_empty() => Vec::new(),
                _ => return Err(format!("line {l}: `waivers` must be a list")),
            };
            for item in items {
                let Node::Map(fields) = item else {
                    return Err(format!("line {l}: each waiver must be a map"));
                };
                let mut w = Waiver {
                    rules: Vec::new(),
                    files: Vec::new(),
                    files_exclude: Vec::new(),
                    lines: Vec::new(),
                };
                for (l, key, v) in fields {
                    match (key.as_str(), v) {
                        ("rule", v @ (Node::Scalar(_) | Node::Seq(_))) => {
                            for s in one_or_list(l, &key, v)? {
                                if s != "*" && crate::rule(&s).is_none() {
                                    return Err(format!(
                                        "line {l}: unknown rule `{s}` in a waiver{}",
                                        near_rule(&s)
                                    ));
                                }
                                w.rules.push(s);
                            }
                        }
                        ("files", v @ (Node::Scalar(_) | Node::Seq(_))) => {
                            for s in one_or_list(l, &key, v)? {
                                w.files.push((l, normalize(&s)));
                            }
                        }
                        ("files_exclude", v @ (Node::Scalar(_) | Node::Seq(_))) => {
                            for s in one_or_list(l, &key, v)? {
                                w.files_exclude.push((l, normalize(&s)));
                            }
                        }
                        ("reason", Node::Scalar(_)) => {}
                        ("lines", Node::Seq(lines)) => {
                            for n in lines {
                                match n {
                                    Node::Scalar(s) if s.parse::<usize>().is_ok() => {
                                        w.lines.push(s.parse().expect("checked"))
                                    }
                                    _ => {
                                        return Err(format!(
                                            "line {l}: `lines` must be a list of line numbers"
                                        ));
                                    }
                                }
                            }
                        }
                        ("rule" | "files" | "files_exclude" | "reason" | "lines", _) => {
                            return Err(format!("line {l}: `{key}` has the wrong form"));
                        }
                        _ => {
                            let keys = ["rule", "files", "files_exclude", "lines", "reason"];
                            return Err(format!(
                                "line {l}: unknown key `{key}` in a waiver (expected rule, files, files_exclude, lines or reason){}",
                                near(&key, "key", keys.into_iter())
                            ));
                        }
                    }
                }
                if w.rules.is_empty() {
                    return Err(format!("line {l}: a waiver needs a `rule`"));
                }
                out.push(w);
            }
        }
        Ok(Waivers(out))
    }

    /// Whether a waiver covers this finding. A pattern is relative to `base`, the directory of the
    /// waiver file, as for `exclude`. It also matches the path as reported, and a bare file name
    /// that file under any directory, which is what a generated waiver file relies on.
    pub fn waives(&self, file: &str, base: &Path, rule: &str, line: usize) -> bool {
        let found = Found::new(file, base);
        self.0.iter().any(|w| {
            (w.rules.iter().any(|r| r == rule || r == "*"))
                && (w.files.is_empty() || w.files.iter().any(|(_, p)| found.matches(p)))
                && !w.files_exclude.iter().any(|(_, p)| found.matches(p))
                && (w.lines.is_empty() || w.lines.contains(&line))
        })
    }

    /// Every pattern, with its line in the waiver file.
    pub fn patterns(&self) -> Vec<(usize, &str)> {
        let all = self
            .0
            .iter()
            .flat_map(|w| w.files.iter().chain(&w.files_exclude));
        all.map(|(l, p)| (*l, p.as_str())).collect()
    }
}

/// Those of the patterns that cover none of these files.
pub fn unmatched<'a>(
    patterns: Vec<(usize, &'a str)>,
    files: &[String],
    base: &Path,
) -> Vec<(usize, &'a str)> {
    let found: Vec<Found> = files.iter().map(|f| Found::new(f, base)).collect();
    patterns
        .into_iter()
        .filter(|(_, p)| !found.iter().any(|f| f.matches(p)))
        .collect()
}

/// A file a waiver may be about: as reported, and as a pattern sees it.
struct Found {
    reported: String,
    place: Place,
}

impl Found {
    fn new(file: &str, base: &Path) -> Found {
        let path = Path::new(file);
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        Found {
            reported: normalize(file),
            place: Place::new(&absolute, base),
        }
    }

    fn matches(&self, pattern: &str) -> bool {
        self.place.covered_by(pattern)
            || glob(pattern.as_bytes(), self.reported.as_bytes())
            || Path::new(&self.reported).ends_with(pattern)
    }
}

/// A value that is one word or a list of them, none empty.
fn one_or_list(l: usize, key: &str, v: Node) -> Result<Vec<String>, String> {
    let items = match v {
        Node::Seq(items) => items,
        n => vec![n],
    };
    if items.is_empty() {
        return Err(format!("line {l}: `{key}` is an empty list"));
    }
    items
        .into_iter()
        .map(|n| match n {
            Node::Scalar(s) if !s.is_empty() => Ok(s),
            _ => Err(format!(
                "line {l}: `{key}` must be a word or a list of them"
            )),
        })
        .collect()
}

/// `path` with `.` and `..` taken out, without asking the file system.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// The nearest file called `name` in `dir` or its ancestors.
pub fn find_up(dir: &Path, name: &str) -> Option<PathBuf> {
    dir.ancestors().map(|d| d.join(name)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_of_the_configuration() {
        let definite = crate::rule("lint_770").unwrap();
        let on = Config::default();
        assert!(on.enabled(definite));
        assert!(on.severity(definite) == Severity::Error);
        let cfg = Config::parse(
            "rule:\n  global:\n    severity: warning\n  group:\n    definite:\n      disable: true\n  lint_771: {disable: false, severity: error}\n",
        )
        .unwrap();
        assert!(!cfg.enabled(definite));
        assert!(cfg.severity(definite) == Severity::Warning);
        let own = crate::rule("lint_771").unwrap();
        assert!(cfg.enabled(own));
        assert!(cfg.severity(own) == Severity::Error);
    }

    #[test]
    fn exclude_paths_and_globs() {
        let cfg = Config::parse(
            "exclude:\n  - gen/\n  - 'src/**/old_*.vhd'\n  - ./one.vhd\n  - '**/vunit/vhdl/**'\n",
        )
        .unwrap();
        let base = Path::new("/p/q");
        let hit = |f: &str| cfg.excluded(Path::new(f), base);
        assert!(hit("/p/q/gen/a.vhd"));
        assert!(hit("/p/q/gen/deep/a.vhd"));
        assert!(hit("/p/q/src/x/old_a.vhd"));
        assert!(hit("/p/q/x/../one.vhd"));
        assert!(hit("/site/vunit/vhdl/check/src/a.vhd"), "outside the base");
        assert!(!hit("/p/q/src/x/new.vhd"));
        assert!(!hit("/p/q/x/gen/a.vhd"), "relative to the configuration");
        assert!(!hit("/other/gen/a.vhd"), "relative to the configuration");
        assert!(!hit("/p/q/generated.vhd"));
        assert!(Config::parse("exclude: gen\n").is_err());
        assert!(
            !Config::parse("exclude:\n")
                .unwrap()
                .excluded(Path::new("/a"), base)
        );
    }

    #[test]
    fn unknown_names_are_errors_that_name_them() {
        for (text, needle) in [
            ("vsg_rs: {}\n", "unknown key `vsg_rs`"),
            ("rule:\n  lint_100:\n    disable: false\n", "`lint_100`"),
            ("rule:\n  entity_019:\n    disable: false\n", "`entity_019`"),
            ("rule:\n  lint_770:\n    fixable: false\n", "`fixable`"),
            (
                "rule:\n  group:\n    style:\n      disable: true\n",
                "`style`",
            ),
            ("rule:\n  lint_770:\n    disable: maybe\n", "`maybe`"),
            ("rule:\n  lint_770:\n    severity: fatal\n", "`fatal`"),
            ("rule: [\n", "line"),
        ] {
            match Config::parse(text) {
                Err(e) => assert!(e.contains(needle), "{e} / {needle}"),
                Ok(_) => panic!("accepted: {text}"),
            }
        }
    }

    #[test]
    fn unknown_names_suggest_a_near_one() {
        let cfg = |t: &str| Config::parse(t).err().expect("rejected");
        assert!(cfg("rul:\n  global: {}\n").contains("a key with a similar name exists: `rule`"));
        assert!(
            cfg("rule:\n  lint_770:\n    disabel: true\n")
                .contains("a setting with a similar name exists: `disable`")
        );
        assert!(
            cfg("rule:\n  lint_770:\n    severity: eror\n")
                .contains("a severity with a similar name exists: `error`")
        );
        assert!(
            cfg("rule:\n  group:\n    definit:\n      disable: true\n")
                .contains("a rule group with a similar name exists: `definite`")
        );
        assert!(
            cfg("rule:\n  lint_0770:\n    disable: true\n")
                .contains("a rule with a similar name exists: `lint_770`")
        );
        // A well-formed id that does not exist is no typo of its neighbours.
        assert!(!cfg("rule:\n  lint_100:\n    disable: true\n").contains("similar"));
        let w = |t: &str| Waivers::parse(t).err().expect("rejected");
        assert!(w("waivers:\n  - rule: lint_0770\n").contains("a rule with a similar name exists"));
        assert!(
            w("waivers:\n  - rule: lint_770\n    reson: x\n")
                .contains("a key with a similar name exists: `reason`")
        );
        assert!(w("waiver:\n").contains("a key with a similar name exists: `waivers`"));
    }

    #[test]
    fn waivers_match_rule_file_and_line() {
        let w = Waivers::parse(
            "waivers:\n  - rule: lint_770\n    files: 'src/**/*.vhd'\n    lines: [7]\n    reason: legacy\n",
        )
        .unwrap();
        let at = Path::new("/p");
        assert!(w.waives("src/a/b.vhd", at, "lint_770", 7));
        assert!(w.waives("./src/b.vhd", at, "lint_770", 7));
        assert!(!w.waives("src/a/b.vhd", at, "lint_770", 8));
        assert!(!w.waives("src/a/b.vhd", at, "lint_771", 7));
        assert!(!w.waives("test/b.vhd", at, "lint_770", 7));
        let w = Waivers::parse("waivers:\n  - rule: '*'\n    reason: all\n").unwrap();
        assert!(w.waives("any/file.vhd", at, "lint_771", 99));
        let w = Waivers::parse("waivers:\n  - rule: lint_770\n    files: b.vhd\n").unwrap();
        assert!(w.waives("src/b.vhd", at, "lint_770", 4000));
    }

    #[test]
    fn waivers_of_directories_and_lists() {
        let at = Path::new("/p");
        let w = Waivers::parse(
            "waivers:\n  - rule: [lint_770, lint_771]\n    files: [src/legacy/, 'gen/*.vhd']\n",
        )
        .unwrap();
        for f in [
            "/p/src/legacy/a.vhd",
            "/p/src/legacy/x/y/a.vhd",
            "/p/gen/a.vhd",
        ] {
            assert!(w.waives(f, at, "lint_770", 1), "{f}");
            assert!(w.waives(f, at, "lint_771", 1), "{f}");
            assert!(!w.waives(f, at, "lint_772", 1), "{f}");
        }
        assert!(!w.waives("/p/src/legacy2/a.vhd", at, "lint_770", 1));
        assert!(!w.waives("/p/gen/x/a.vhd", at, "lint_770", 1));
        assert!(!w.waives("/q/src/legacy/a.vhd", at, "lint_770", 1));
        let files = ["/p/gen/a.vhd".to_string()];
        assert_eq!(unmatched(w.patterns(), &files, at), [(3, "src/legacy/")]);
        let w = Waivers::parse(
            "waivers:\n  - rule: lint_770\n    files: src\n    files_exclude:\n      - src/keep\n      - '*/skip.vhd'\n",
        )
        .unwrap();
        assert!(w.waives("/p/src/a.vhd", at, "lint_770", 1));
        assert!(
            !w.waives("/p/src/keep/a.vhd", at, "lint_770", 1),
            "exclude wins"
        );
        assert!(
            !w.waives("/p/src/skip.vhd", at, "lint_770", 1),
            "exclude wins"
        );
        let w = Waivers::parse("waivers:\n  - rule: lint_770\n    files_exclude: [src/keep]\n")
            .unwrap();
        assert!(
            w.waives("/elsewhere/a.vhd", at, "lint_770", 1),
            "no files: every file"
        );
        assert!(!w.waives("/p/src/keep/a.vhd", at, "lint_770", 1));
        let w = Waivers::parse("waivers:\n  - rule: lint_770\n    files: '**/third/**'\n").unwrap();
        assert!(w.waives("/elsewhere/third/a.vhd", at, "lint_770", 1));
    }

    #[test]
    fn a_bad_waiver_file_is_an_error() {
        assert!(Waivers::parse("waivers:\n  - reason: nothing\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: no_such\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: lint_770\n    line: 3\n").is_err());
        assert!(Waivers::parse("waive: []\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: lint_770\n    files: []\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: []\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: lint_770\n    files_exclude: []\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: [lint_770, no_such]\n").is_err());
        assert!(Waivers::parse("waivers:\n").unwrap().0.is_empty());
    }
}
