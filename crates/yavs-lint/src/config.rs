//! `yavs-lint.yaml` (`rule`, `exclude`, `file_list`) and `yavs-lint-waivers.yaml`. Both are
//! declared as a schema (`CONFIG`, `WAIVERS`) and checked by `yavs_diag::yaml` before anything is
//! read from them: a key, rule id or rule group the schema does not have is an error, all of them
//! reported together, and so is a value of the wrong type. What reads the checked tree below relies
//! on that. Values are read as the text they were written as.

use crate::{Certainty, Rule};
use std::path::{Path, PathBuf};
use yavs_diag::Severity;
use yavs_diag::files::{Found, Place, glob, normalize};
use yavs_diag::suggest::tail as near;
use yavs_diag::yaml::{self, Field, MapShape, Node, Rest, Shape, Value};

const CLASSES: [Certainty; 4] = [
    Certainty::Definite,
    Certainty::Advisory,
    Certainty::Experimental,
    Certainty::Policy,
];

/// A regular expression after `re:`, as `lint_602` and `lint_603` take.
fn pattern(n: &Node, path: &str) -> Option<String> {
    let Value::Scalar(s) = &n.value else {
        return Some(format!("`{path}` must be a list of words"));
    };
    let r = s.strip_prefix("re:")?;
    let e = regex::Regex::new(r).err()?.to_string();
    // The last line of the error says what is wrong; the others draw the pattern.
    let why = e.lines().last().unwrap_or_default();
    let why = why.strip_prefix("error: ").unwrap_or(why);
    Some(format!(
        "`{s}` in `{path}` is not a regular expression: {why}"
    ))
}

/// One entry of `file_list`: a path or glob, or a map from one to its settings, of which only
/// empty ones are taken. Settings of one file are refused, never dropped.
fn file_entry(n: &Node, path: &str) -> Option<String> {
    match &n.value {
        Value::Scalar(_) => None,
        Value::Map(entries) => entries.iter().find_map(|e| {
            let empty = match &e.value.value {
                Value::Scalar(s) => s.is_empty(),
                Value::Map(m) => m.is_empty(),
                _ => false,
            };
            (!empty).then(|| {
                let pattern = &e.key;
                format!(
                    "settings for one `{path}` entry are not implemented: list `{pattern}` as a plain path, and use a waiver to turn a rule off for it"
                )
            })
        }),
        _ => Some(format!("a `{path}` entry must be a path")),
    }
}

/// A rule id or `*`, in a waiver.
fn waived_rule(n: &Node, path: &str) -> Option<String> {
    let id = n.text();
    if !matches!(n.value, Value::Scalar(_)) || id.is_empty() {
        return Some(format!("`{path}` must be a word or a list of them"));
    }
    (id != "*" && crate::rule(id).is_none())
        .then(|| format!("unknown rule `{id}` in `{path}`{}", near_rule(id)))
}

/// The tail of an error about the unknown rule `id`.
fn near_rule(id: &str) -> String {
    crate::similar_rule(id).map_or_else(String::new, |r| {
        format!("; {}", yavs_diag::suggest::help(&[(r.into(), "rule")]))
    })
}

/// The settings of a rule, a class or `global`: on or off, and the severity.
macro_rules! layer {
    ($($more:expr),*) => {
        Shape::Map(&MapShape {
            what: "a rule",
            fields: &[
                Field { key: "disable", shape: &Shape::Bool, required: false },
                Field { key: "severity", shape: &Shape::Choice("severity", &["error", "warning"]), required: false },
                $($more),*
            ],
            rest: Rest::Unknown,
        })
    };
}

static LAYER: Shape = layer!();
static LAYER_SUFFIXES: Shape = layer!(Field {
    key: "suffixes",
    shape: &Shape::List(&Shape::Custom(pattern)),
    required: false,
});
static LAYER_PREFIXES: Shape = layer!(Field {
    key: "prefixes",
    shape: &Shape::List(&Shape::Custom(pattern)),
    required: false,
});

fn rule_layer(id: &str) -> Option<&'static Shape> {
    crate::rule(id).map(|r| match r.id {
        "lint_602" => &LAYER_SUFFIXES,
        "lint_603" => &LAYER_PREFIXES,
        _ => &LAYER,
    })
}

fn rule_tail(id: &str) -> String {
    let tail = near_rule(id);
    if tail.is_empty() {
        near(id, "key", ["global", "group"].into_iter())
    } else {
        tail
    }
}

fn group_layer(name: &str) -> Option<&'static Shape> {
    CLASSES.iter().any(|c| c.name() == name).then_some(&LAYER)
}

fn group_tail(name: &str) -> String {
    near(name, "rule group", CLASSES.iter().map(|c| c.name()))
}

/// A `rule:` section: `global`, `group`, and a rule id for each rule.
static RULES: Shape = Shape::Map(&MapShape {
    what: "the rules",
    fields: &[
        Field {
            key: "global",
            shape: &LAYER,
            required: false,
        },
        Field {
            key: "group",
            shape: &Shape::Map(&MapShape {
                what: "the groups",
                fields: &[],
                rest: Rest::Names {
                    noun: "rule group",
                    known: group_layer,
                    near: group_tail,
                },
            }),
            required: false,
        },
    ],
    rest: Rest::Names {
        noun: "rule",
        known: rule_layer,
        near: rule_tail,
    },
});

/// `rtl:` and `testbench:`: the rules for files of that kind.
static KIND: Shape = Shape::Map(&MapShape {
    what: "the block",
    fields: &[Field {
        key: "rule",
        shape: &RULES,
        required: false,
    }],
    rest: Rest::Unknown,
});

/// `yavs-lint.yaml`.
pub static CONFIG: Shape = Shape::Map(&MapShape {
    what: "the configuration",
    fields: &[
        Field {
            key: "rule",
            shape: &RULES,
            required: false,
        },
        Field {
            key: "exclude",
            shape: &Shape::List(&Shape::Word),
            required: false,
        },
        Field {
            key: "file_list",
            shape: &Shape::List(&Shape::Custom(file_entry)),
            required: false,
        },
        Field {
            key: "testbench_files",
            shape: &Shape::List(&Shape::Word),
            required: false,
        },
        Field {
            key: "testbench_libraries",
            shape: &Shape::List(&Shape::Word),
            required: false,
        },
        Field {
            key: "rtl",
            shape: &KIND,
            required: false,
        },
        Field {
            key: "testbench",
            shape: &KIND,
            required: false,
        },
    ],
    rest: Rest::Unknown,
});

/// `yavs-lint-waivers.yaml`.
pub static WAIVERS: Shape = Shape::Map(&MapShape {
    what: "the waiver file",
    fields: &[Field {
        key: "waivers",
        shape: &Shape::List(&Shape::Map(&MapShape {
            what: "a waiver",
            fields: &[
                Field {
                    key: "rule",
                    shape: &Shape::OneOrList(&Shape::Custom(waived_rule)),
                    required: true,
                },
                Field {
                    key: "files",
                    shape: &Shape::OneOrList(&Shape::Word),
                    required: false,
                },
                Field {
                    key: "files_exclude",
                    shape: &Shape::OneOrList(&Shape::Word),
                    required: false,
                },
                Field {
                    key: "lines",
                    shape: &Shape::List(&Shape::Count),
                    required: false,
                },
                Field {
                    key: "reason",
                    shape: &Shape::Text,
                    required: false,
                },
            ],
            rest: Rest::Unknown,
        })),
        required: false,
    }],
    rest: Rest::Unknown,
});

/// `disable` and `severity` as set at one level.
#[derive(Clone, Default)]
struct Layer {
    disable: Option<bool>,
    severity: Option<Severity>,
    /// `suffixes` of lint_602, `prefixes` of lint_603.
    list: Vec<String>,
}

/// The settings of one level, from a node `LAYER` has checked.
fn layer(n: &Node) -> Layer {
    let mut out = Layer::default();
    for e in n.entries() {
        let text = e.value.text();
        match e.key.as_str() {
            "disable" => out.disable = Some(matches!(text, "true" | "True")),
            "severity" => {
                out.severity = Some(if text.eq_ignore_ascii_case("error") {
                    Severity::Error
                } else {
                    Severity::Warning
                });
            }
            _ => out.list.extend(e.value.words()),
        }
    }
    out
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
    /// `testbench_files`: patterns of the files that are testbenches.
    testbench_files: Vec<String>,
    /// `testbench_libraries`: the libraries that hold testbenches, folded.
    testbench_libraries: Vec<String>,
    /// The `rule:` sections of `rtl:` and `testbench:`, which win for files of that kind.
    kinds: [Option<Box<Config>>; 2],
    /// `file_list`: the path or glob of each entry, as written.
    file_list: Vec<String>,
}

/// Which rule block of the configuration applies to a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rtl = 0,
    Testbench = 1,
}

impl Config {
    /// Read a `yavs-lint.yaml`; the error names the line of every problem: an unknown key, rule
    /// or rule group, or a value of the wrong type (`CONFIG` says what is there).
    pub fn parse(text: &str) -> Result<Config, String> {
        Ok(Self::from_node(&yaml::read(text, &CONFIG)?))
    }

    /// The settings of a checked tree: every key in it is declared by `CONFIG`.
    fn from_node(root: &Node) -> Config {
        let mut cfg = Config::default();
        for e in root.entries() {
            let v = &e.value;
            match e.key.as_str() {
                "exclude" => {
                    for s in v.words() {
                        cfg.exclude
                            .push(normalize(&s).trim_end_matches('/').to_owned());
                    }
                }
                "testbench_files" => {
                    cfg.testbench_files = v.words().iter().map(|s| normalize(s)).collect();
                }
                "testbench_libraries" => {
                    cfg.testbench_libraries =
                        v.words().iter().map(|s| s.to_ascii_lowercase()).collect();
                }
                "rtl" | "testbench" => {
                    let mut block = Config::default();
                    if let Some(rules) = v.get("rule") {
                        block.rules_section(rules);
                    }
                    let at = if e.key == "rtl" {
                        Kind::Rtl
                    } else {
                        Kind::Testbench
                    };
                    cfg.kinds[at as usize] = Some(Box::new(block));
                }
                "rule" => cfg.rules_section(v),
                "file_list" => {
                    for n in v.items() {
                        match &n.value {
                            Value::Map(entries) => {
                                cfg.file_list.extend(entries.iter().map(|e| e.key.clone()));
                            }
                            _ => cfg.file_list.push(n.text().to_owned()),
                        }
                    }
                }
                _ => {}
            }
        }
        cfg
    }

    /// One `rule:` section, at the top level or of a kind of file.
    fn rules_section(&mut self, section: &Node) {
        for e in section.entries() {
            match e.key.as_str() {
                "global" => self.global = layer(&e.value),
                "group" => {
                    for g in e.value.entries() {
                        let class = CLASSES.into_iter().find(|c| c.name() == g.key);
                        self.groups
                            .extend(class.map(|class| (class, layer(&g.value))));
                    }
                }
                id => {
                    if let Some(rule) = crate::rule(id) {
                        self.rules.push((rule.id, layer(&e.value)));
                    }
                }
            }
        }
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

    /// The entries of `file_list`; [`expand`] gives the files of one.
    pub fn file_list(&self) -> &[String] {
        &self.file_list
    }

    /// The list a rule's own settings give (`suffixes`, `prefixes`), empty when not set. The
    /// top level first, then a kind of file: one list for the whole run.
    pub(crate) fn option_list(&self, id: &str) -> &[String] {
        let own = self
            .rules
            .iter()
            .rev()
            .find(|(r, _)| *r == id)
            .map_or(&[][..], |x| &x.1.list[..]);
        if !own.is_empty() {
            return own;
        }
        self.kinds
            .iter()
            .flatten()
            .map(|k| k.option_list(id))
            .find(|l| !l.is_empty())
            .unwrap_or(&[])
    }

    /// Whether anything but the definite class runs.
    pub(crate) fn beyond_definite(&self) -> bool {
        crate::RULES
            .iter()
            .any(|r| r.certainty != Certainty::Definite && self.enabled_anywhere(r))
    }

    /// Whether files are told apart at all: a rule block for a kind of file exists.
    pub(crate) fn has_kinds(&self) -> bool {
        self.kinds.iter().any(Option::is_some)
    }

    /// Whether `testbench_files` covers the file: a pattern like those of `exclude`, a bare name
    /// wherever the file is, and a relative path below any directory.
    pub(crate) fn testbench_file(&self, file: &Path, base: &Path) -> bool {
        let place = Place::new(file, base);
        let name = place.absolute.rsplit('/').next().unwrap_or_default();
        self.testbench_files.iter().any(|p| {
            place.covered_by(p)
                || (!p.contains('/') && glob(p.as_bytes(), name.as_bytes()))
                || glob(format!("**/{p}").as_bytes(), place.absolute.as_bytes())
        })
    }

    pub(crate) fn testbench_library(&self, library: &str) -> bool {
        self.testbench_libraries.iter().any(|l| l == library)
    }

    /// Whether this rule runs for some kind of file.
    pub(crate) fn enabled_anywhere(&self, r: &Rule) -> bool {
        self.enabled(r)
            || [Kind::Rtl, Kind::Testbench]
                .iter()
                .any(|k| self.enabled_in(r, Some(*k)))
    }

    fn kind_layers(&self, r: &Rule, kind: Option<Kind>) -> Option<[Layer; 3]> {
        let block = self.kinds[kind? as usize].as_ref()?;
        Some(block.layers(r))
    }

    /// Whether this rule runs for a file of this kind: its block wins over the top level.
    pub(crate) fn enabled_in(&self, r: &Rule, kind: Option<Kind>) -> bool {
        self.kind_layers(r, kind)
            .and_then(|ls| ls.iter().find_map(|l| l.disable))
            .map_or_else(|| self.enabled(r), |disable| !disable)
    }

    /// The severity of the rule's findings in a file of this kind.
    pub(crate) fn severity_in(&self, r: &Rule, kind: Option<Kind>) -> Severity {
        self.kind_layers(r, kind)
            .and_then(|ls| ls.iter().find_map(|l| l.severity))
            .unwrap_or_else(|| self.severity(r))
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

/// `$NAME` and `${NAME}` replaced from the environment; one that is not set stays as written, and
/// then matches no file.
fn expand_vars(text: &str) -> String {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let after = &rest[1..];
        let (name, len) = match after.strip_prefix('{').and_then(|a| a.split_once('}')) {
            Some((name, _)) if !name.is_empty() && name.chars().all(word) => (name, name.len() + 3),
            _ => {
                let end = after.find(|c| !word(c)).unwrap_or(after.len());
                (&after[..end], end + 1)
            }
        };
        match std::env::var(name) {
            Ok(value) if !name.is_empty() => out.push_str(&value),
            _ => out.push_str(&rest[..len]),
        }
        rest = &rest[len..];
    }
    out + rest
}

/// The files below `dir`, leaving out hidden entries; symbolic links to directories are not followed.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let cannot = |e: std::io::Error| format!("cannot read directory `{}`: {e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(cannot)? {
        let entry = entry.map_err(cannot)?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if entry.file_type().map_err(cannot)?.is_dir() {
            walk(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// What one `file_list` entry names, relative to the working directory: the sorted files a glob
/// matches, or a path that exists, which may be a directory. Empty when nothing is there.
pub fn expand(entry: &str) -> Result<Vec<PathBuf>, String> {
    let pattern = normalize(&expand_vars(entry));
    let Some(wild) = pattern.find(['*', '?']) else {
        let path = PathBuf::from(&pattern);
        return Ok(if path.exists() {
            vec![path]
        } else {
            Vec::new()
        });
    };
    let base = match pattern[..wild].rfind('/') {
        Some(0) => "/",
        Some(i) => &pattern[..i],
        None => ".",
    };
    let mut files = Vec::new();
    if Path::new(base).is_dir() {
        walk(Path::new(base), &mut files)?;
    }
    let mut files: Vec<PathBuf> = files
        .into_iter()
        .map(|f| f.strip_prefix(".").map(Path::to_path_buf).unwrap_or(f))
        .filter(|f| {
            glob(
                pattern.as_bytes(),
                normalize(&f.to_string_lossy()).as_bytes(),
            )
        })
        .collect();
    files.sort();
    Ok(files)
}

/// The tail of an error about an entry that matches nothing: the name close to its first part
/// that is not there. Empty when none is close, or when the parts before a wildcard are all there.
pub fn near_entry(entry: &str) -> String {
    let pattern = normalize(&expand_vars(entry));
    let mut dir = PathBuf::new();
    for part in Path::new(&pattern).components() {
        let text = part.as_os_str().to_string_lossy();
        if text.contains(['*', '?']) {
            break;
        }
        let next = dir.join(part);
        if !next.exists() {
            let shown = if dir.as_os_str().is_empty() {
                Path::new(".")
            } else {
                &dir
            };
            let names: Vec<String> = std::fs::read_dir(shown)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            return near(&text, "path", names.iter().map(String::as_str));
        }
        dir = next;
    }
    String::new()
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

impl Waivers {
    /// Read a `yavs-lint-waivers.yaml`; the error names the line of every problem. A misspelled
    /// `files` or `lines` would widen a waiver, and an unknown rule would waive nothing, so
    /// neither is accepted (`WAIVERS` says what is).
    pub fn parse(text: &str) -> Result<Waivers, String> {
        let root = yaml::read(text, &WAIVERS)?;
        let mut out = Vec::new();
        for item in root.get("waivers").map_or(&[][..], Node::items) {
            let list = |key: &str| item.get(key).map(Node::words).unwrap_or_default();
            let patterns = |key: &str| {
                let line = item.entries().iter().find(|e| e.key == key);
                let line = line.map_or(0, |e| e.line);
                list(key)
                    .into_iter()
                    .map(|s| (line, normalize(&s)))
                    .collect()
            };
            out.push(Waiver {
                rules: list("rule"),
                files: patterns("files"),
                files_exclude: patterns("files_exclude"),
                lines: item
                    .get("lines")
                    .map(|n| {
                        n.items()
                            .iter()
                            .filter_map(|l| l.text().parse().ok())
                            .collect()
                    })
                    .unwrap_or_default(),
            });
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
    fn testbench_file_patterns() {
        let cfg = Config::parse(
            "testbench_files:\n  - 'tb_*.vhd'\n  - 'test/**'\n  - '**/verify/*.vhd'\n  - 'sim/*.vhd'\n",
        )
        .unwrap();
        let base = Path::new("/p");
        let is = |f: &str| cfg.testbench_file(Path::new(f), base);
        assert!(is("/p/modules/fifo/test/tb_fifo.vhd"));
        assert!(!is("/p/modules/fifo/src/fifo.vhd"));
        assert!(is("/p/test/a.vhd"));
        assert!(is("/p/modules/fifo/test/a.vhd"));
        assert!(is("/p/a/b/verify/thing.vhd"));
        assert!(is("/p/sim/harness.vhd"));
        assert!(!is("/p/sim/deep/harness.vhd"));
    }

    #[test]
    fn file_list_forms() {
        let list = |t: &str| Config::parse(t).map(|c| c.file_list);
        assert_eq!(
            list("file_list:\n  - a.vhd\n  - 'src/*.vhd'\n  - b.vhd: {}\n  - c.vhd:\n").unwrap(),
            ["a.vhd", "src/*.vhd", "b.vhd", "c.vhd"]
        );
        assert!(
            list("file_list: a.vhd\n")
                .unwrap_err()
                .contains("must be a list")
        );
        assert!(list("file_list:\n").unwrap().is_empty(), "no entries");
        assert!(
            list("file_list:\n  - [a.vhd]\n")
                .unwrap_err()
                .contains("must be a path")
        );
        let per_file = list("file_list:\n  - a.vhd:\n      rule: {}\n").unwrap_err();
        assert!(
            per_file.contains("`a.vhd`") && per_file.contains("not implemented"),
            "{per_file}"
        );
    }

    #[test]
    fn variables_in_an_entry() {
        let dir = env!("CARGO_MANIFEST_DIR");
        assert_eq!(
            expand_vars("${CARGO_MANIFEST_DIR}/x/$CARGO_MANIFEST_DIR/$LINT_TEST_NOT_SET/${}/$"),
            format!("{dir}/x/{dir}/$LINT_TEST_NOT_SET/${{}}/$")
        );
    }

    #[test]
    fn unknown_names_at_every_level_are_one_error_naming_all() {
        let e = Config::parse(
            "indent: &a {tokens: [a]}\nno_such_key: *a\nrule:\n  length_001: {length: 120}\n  lint_100:\n    disable: false\n  global:\n    fixable: true\n  group:\n    alignment:\n      disable: true\n    definite:\n      fix: [a]\n  lint_770:\n    disable: true\n    synchronizers: [a, b]\n  lint_700:\n    suffixes: [a]\nrtl:\n  other: 1\n  rule:\n    port_007: {}\n",
        )
        .err()
        .unwrap();
        let lines: Vec<&str> = e.lines().collect();
        let expect = [
            "line 1: unknown key `indent`",
            "line 2: unknown key `no_such_key`",
            "line 4: unknown rule `length_001` in `rule`",
            "line 5: unknown rule `lint_100` in `rule`",
            "line 8: unknown key `fixable` in `rule.global`",
            "line 10: unknown rule group `alignment` in `rule.group`",
            "line 13: unknown key `fix` in `rule.group.definite`",
            "line 16: unknown key `synchronizers` in `rule.lint_770`",
            "line 18: unknown key `suffixes` in `rule.lint_700`",
            "line 20: unknown key `other` in `rtl`",
            "line 22: unknown rule `port_007` in `rtl.rule`",
        ];
        assert_eq!(lines.len(), expect.len(), "{e}");
        for (said, want) in lines.iter().zip(expect) {
            assert!(said.starts_with(want), "{said} / {want}");
        }
        let cfg = Config::parse("rule:\n  lint_770:\n    disable: true\n").unwrap();
        assert!(!cfg.enabled(crate::rule("lint_770").unwrap()));
        let e = Waivers::parse(
            "waiver: 1\nwaivers:\n  - rule: [length_001, lint_770]\n    line: 3\n  - rule: port_007\n",
        )
        .err()
        .unwrap();
        for want in [
            "line 1: unknown key `waiver`",
            "unknown rule `length_001` in `waivers.rule`",
            "unknown key `line` in `waivers`",
            "unknown rule `port_007` in `waivers.rule`",
        ] {
            assert!(e.contains(want), "{e} / {want}");
        }
        assert_eq!(e.lines().count(), 4, "{e}");
    }

    #[test]
    fn unknown_names_and_bad_values_are_listed_together() {
        let e = Config::parse(
            "indent: 1\nrule:\n  lint_770:\n    severity: fatal\n    disable: [a]\n  lint_602:\n    suffixes: ['re:(', ok]\n  lint_100: {}\nexclude: gen\nfile_list:\n  - [a]\n  - b.vhd: {rule: {}}\n",
        )
        .err()
        .unwrap();
        let lines: Vec<&str> = e.lines().collect();
        assert_eq!(lines.len(), 8, "{e}");
        assert!(lines[0].starts_with("line 1: unknown key `indent`"), "{e}");
        assert!(
            lines[1].contains(
                "`rule.lint_770.severity` must be one of `error`, `warning`, not `fatal`"
            ),
            "{e}"
        );
        assert!(
            lines[2].contains("`rule.lint_770.disable` must be true or false, not a list"),
            "{e}"
        );
        assert!(
            lines[3].contains("`re:(` in `rule.lint_602.suffixes` is not a regular expression"),
            "{e}"
        );
        assert!(
            lines[4].starts_with("line 8: unknown rule `lint_100` in `rule`"),
            "{e}"
        );
        assert!(
            lines[5].contains("`exclude` must be a list of words, not `gen`"),
            "{e}"
        );
        assert!(
            lines[6].contains("a `file_list` entry must be a path"),
            "{e}"
        );
        assert!(
            lines[7].contains("settings for one `file_list` entry are not implemented"),
            "{e}"
        );
    }

    #[test]
    fn the_waiver_schema_checks_types_and_the_required_rule() {
        let e = Waivers::parse(
            "waivers:\n  - files: a.vhd\n  - rule: lint_770\n    lines: [1, x]\n    files: []\n    reason: [a]\n  - rule: {a: b}\n",
        )
        .err()
        .unwrap();
        for want in [
            "line 2: a waiver needs `rule`",
            "line 4: `waivers.lines` must be a whole number, not `x`",
            "`waivers.files` is an empty list",
            "`waivers.reason` must be a word, not a list",
            "`waivers.rule` must be a word or a list of them, not a map",
        ] {
            assert!(e.contains(want), "{e} / {want}");
        }
        assert!(Waivers::parse("waivers:\n  - rule: '*'\n").is_ok());
    }

    #[test]
    fn the_schema_declares_every_key_a_file_may_have() {
        let keys = CONFIG.keys();
        for k in [
            "rule",
            "rule.global.disable",
            "rule.group",
            "exclude",
            "file_list",
            "rtl.rule",
            "testbench.rule.global.severity",
        ] {
            assert!(keys.iter().any(|x| x == k), "{k} in {keys:?}");
        }
        let keys = WAIVERS.keys();
        assert_eq!(
            keys,
            [
                "waivers",
                "waivers.rule",
                "waivers.files",
                "waivers.files_exclude",
                "waivers.lines",
                "waivers.reason"
            ]
        );
    }

    #[test]
    fn a_misspelled_waiver_key_is_an_error_not_a_wider_waiver() {
        let e = Waivers::parse("waivers:\n  - rule: lint_770\n    fils: a.vhd\n    lins: [3]\n")
            .err()
            .unwrap();
        assert!(
            e.contains(
                "line 3: unknown key `fils` in `waivers`; a key with a similar name exists: `files`"
            ),
            "{e}"
        );
        assert!(
            e.contains(
                "line 4: unknown key `lins` in `waivers`; a key with a similar name exists: `lines`"
            ),
            "{e}"
        );
    }

    #[test]
    fn bad_values_of_known_keys_stay_errors() {
        for (text, needle) in [
            ("rule:\n  lint_770:\n    disable: maybe\n", "`maybe`"),
            ("rule:\n  lint_770:\n    severity: fatal\n", "`fatal`"),
            (
                "rule:\n  lint_770:\n    disable: [a]\n",
                "must be true or false, not a list",
            ),
            ("rule: [a]\n", "`rule` must be a map"),
            ("rule:\n  group: [a]\n", "`rule.group` must be a map"),
            ("rule:\n  lint_602:\n    suffixes: a\n", "must be a list"),
            ("rtl: 1\n", "`rtl` must be a map"),
            ("rtl:\n  rule: 1\n", "`rtl.rule` must be a map"),
            ("exclude: a\n", "must be a list"),
            ("exclude: &a [b]\nrule: *a\n", "`rule` must be a map"),
            ("rule: [\n", "line"),
            ("a: 1\na: 2\n", "duplicate key"),
        ] {
            match Config::parse(text) {
                Err(e) => assert!(e.contains(needle), "{e} / {needle}"),
                Ok(_) => panic!("accepted: {text}"),
            }
        }
    }

    #[test]
    fn unknown_names_suggest_a_near_one() {
        let said = |t: &str| Config::parse(t).err().unwrap();
        assert!(said("exclud:\n").contains("a key with a similar name exists: `exclude`"));
        assert!(
            said("rule:\n  lint_770:\n    disabel: true\n")
                .contains("a key with a similar name exists: `disable`")
        );
        assert!(
            said("rule:\n  group:\n    definit:\n      disable: true\n")
                .contains("a rule group with a similar name exists: `definite`")
        );
        for typo in ["lint_0771", "lnt_771", "lint771"] {
            let text = said(&format!("rule:\n  {typo}:\n    disable: true\n"));
            assert!(
                text.contains("a rule with a similar name exists: `lint_771`"),
                "{text}"
            );
        }
        assert!(said("rule:\n  groop: {}\n").contains("a key with a similar name exists: `group`"));
        // A well-formed id that does not exist is no typo of its neighbours, nor is another stem.
        assert!(!said("rule:\n  lint_100:\n    disable: true\n").contains("similar"));
        assert!(!said("rule:\n  length_001:\n    disable: true\n").contains("similar"));
        let bad = Config::parse("rule:\n  lint_770:\n    severity: eror\n").err();
        assert!(
            bad.unwrap()
                .contains("a severity with a similar name exists: `error`")
        );
        let w = |t: &str| Waivers::parse(t).err().unwrap();
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
        // Absolute on every platform, like the directory of a real waiver file.
        let at = &std::path::absolute("/p").unwrap();
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
        assert!(Waivers::parse("waivers:\n  - rule: lint_770\n    files: []\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: []\n").is_err());
        assert!(Waivers::parse("waivers:\n  - rule: lint_770\n    files_exclude: []\n").is_err());
        assert!(Waivers::parse("waivers:\n").unwrap().0.is_empty());
    }
}
