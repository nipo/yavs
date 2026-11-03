//! The one reader and the one schema check of every YAML file yavs reads: `yavs-lint.yaml`,
//! `yavs-lint-waivers.yaml`, the YAML form of a coverage exclusion file and a coverage
//! configuration file.
//!
//! A file is declared once, as a [`Shape`]: its keys, the type of each value and how they nest.
//! [`read`] parses the text and checks it against that shape, and reports every problem of the
//! file in one error: an unknown key, rule or group (with a suggestion through
//! [`suggest::tail`](crate::suggest::tail)), a value of the wrong type, a missing required key.
//! The code that turns the checked tree into settings can then rely on the shape and does not
//! check types again; a key that is not in the shape never reaches it.
//!
//! Every value is read as the text it was written as, so `no`, `on` and a hash of digits only stay
//! strings. The parser is `saphyr-parser` (events with line numbers), not a data model.

use crate::suggest::tail;
use saphyr_parser::{Event, Parser};

/// A value of the document with the line it starts on (1-based).
#[derive(Debug)]
pub struct Node {
    pub line: usize,
    pub value: Value,
}

#[derive(Debug)]
pub enum Value {
    Scalar(String),
    Seq(Vec<Node>),
    /// Key, the line of the key, value; in the order of the file.
    Map(Vec<Entry>),
    /// Valid YAML this reader does not follow.
    Alias,
}

#[derive(Debug)]
pub struct Entry {
    pub line: usize,
    pub key: String,
    pub value: Node,
}

impl Node {
    /// The text of a scalar; empty for anything else, which a checked tree does not have where a
    /// scalar is declared.
    pub fn text(&self) -> &str {
        match &self.value {
            Value::Scalar(s) => s,
            _ => "",
        }
    }

    /// The entries of a map; none for anything else.
    pub fn entries(&self) -> &[Entry] {
        match &self.value {
            Value::Map(m) => m,
            _ => &[],
        }
    }

    /// The items of a list; none for anything else.
    pub fn items(&self) -> &[Node] {
        match &self.value {
            Value::Seq(s) => s,
            _ => &[],
        }
    }

    /// The value of a key of a map.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entries()
            .iter()
            .find(|e| e.key == key)
            .map(|e| &e.value)
    }

    /// A word or a list of them as words; an empty value (a list with every entry commented out)
    /// has none.
    pub fn words(&self) -> Vec<String> {
        match &self.value {
            Value::Scalar(s) if s.is_empty() => Vec::new(),
            Value::Scalar(s) => vec![s.clone()],
            Value::Seq(items) => items.iter().map(|n| n.text().to_owned()).collect(),
            _ => Vec::new(),
        }
    }

    /// What this value is, for "not ...".
    fn found(&self) -> String {
        match &self.value {
            Value::Scalar(s) if s.is_empty() => "nothing".into(),
            Value::Scalar(s) => format!("`{s}`"),
            Value::Seq(_) => "a list".into(),
            Value::Map(_) => "a map".into(),
            Value::Alias => "an alias".into(),
        }
    }
}

/// What a value may be.
pub enum Shape {
    /// Any single value, empty included.
    Text,
    /// A single value that is not empty.
    Word,
    /// `true` or `false` (also capitalised).
    Bool,
    /// A whole number, zero or more.
    Count,
    /// One of these words, without regard to case; the noun is for the suggestion.
    Choice(&'static str, &'static [&'static str]),
    /// A list of values of one shape. An empty value counts as an empty list.
    List(&'static Shape),
    /// One value or a non-empty list of them.
    OneOrList(&'static Shape),
    /// A map with declared keys.
    Map(&'static MapShape),
    /// A map whose `tag` key picks the keys it may have.
    Tagged(&'static TaggedShape),
    /// What the shape cannot say: given the value and its path, a message for what is wrong, or
    /// `None`. It decides the type itself.
    Custom(fn(&Node, &str) -> Option<String>),
}

/// A map with declared keys.
pub struct MapShape {
    /// `a waiver`, for "a waiver needs `rule`".
    pub what: &'static str,
    pub fields: &'static [Field],
    /// What a key that is not in `fields` is.
    pub rest: Rest,
}

pub struct Field {
    pub key: &'static str,
    pub shape: &'static Shape,
    pub required: bool,
}

/// A key that is not in the declared fields.
pub enum Rest {
    /// Unknown: an error.
    Unknown,
    /// A name (such as a rule id) that `known` accepts, giving the shape of its value; any other
    /// is an unknown `noun` with `near` as the tail of its message.
    Names {
        noun: &'static str,
        known: fn(&str) -> Option<&'static Shape>,
        near: fn(&str) -> String,
    },
}

/// A map whose `tag` key says which keys it may have.
pub struct TaggedShape {
    pub tag: &'static str,
    /// `exclusion kind`.
    pub noun: &'static str,
    /// The tag values and the fields of each; the tag itself is among the fields.
    pub variants: &'static [(&'static str, &'static [Field])],
}

impl Shape {
    /// What the shape is, after "must be".
    fn describe(&self) -> String {
        match self {
            Shape::Text | Shape::Word => "a word".into(),
            Shape::Bool => "true or false".into(),
            Shape::Count => "a whole number".into(),
            Shape::Choice(_, words) => {
                let list: Vec<String> = words.iter().map(|w| format!("`{w}`")).collect();
                format!("one of {}", list.join(", "))
            }
            Shape::List(item) => format!("a list of {}", item.plural()),
            Shape::OneOrList(_) => "a word or a list of them".into(),
            Shape::Map(_) | Shape::Tagged(_) => "a map".into(),
            Shape::Custom(_) => "a valid value".into(),
        }
    }

    fn plural(&self) -> &'static str {
        match self {
            Shape::Text | Shape::Word | Shape::Choice(..) | Shape::OneOrList(_) => "words",
            Shape::Bool => "true or false values",
            Shape::Count => "whole numbers",
            Shape::List(_) => "lists",
            Shape::Map(_) | Shape::Tagged(_) => "maps",
            Shape::Custom(_) => "entries",
        }
    }

    /// Every key the shape declares, with its dotted path, depth first: the reference of the file.
    pub fn keys(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_keys("", &mut out);
        out
    }

    fn collect_keys(&self, path: &str, out: &mut Vec<String>) {
        let fields = |fields: &[Field], out: &mut Vec<String>| {
            for f in fields {
                let at = join(path, f.key);
                out.push(at.clone());
                f.shape.collect_keys(&at, out);
            }
        };
        match self {
            Shape::List(item) | Shape::OneOrList(item) => item.collect_keys(path, out),
            Shape::Map(m) => fields(m.fields, out),
            Shape::Tagged(t) => {
                for (_, f) in t.variants {
                    fields(f, out);
                }
            }
            _ => {}
        }
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
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
    let value = match ev {
        Event::Scalar(v, ..) => Value::Scalar(v.to_string()),
        Event::SequenceStart(..) => {
            let mut v = Vec::new();
            loop {
                let (ev, l) = next(p)?;
                if matches!(ev, Event::SequenceEnd) {
                    break;
                }
                v.push(node(p, ev, l)?);
            }
            Value::Seq(v)
        }
        Event::MappingStart(..) => {
            let mut v: Vec<Entry> = Vec::new();
            loop {
                let (ev, l) = next(p)?;
                let key = match ev {
                    Event::MappingEnd => break,
                    Event::Scalar(k, ..) => k.to_string(),
                    _ => return Err(format!("line {l}: a key must be a plain word")),
                };
                if v.iter().any(|e| e.key == key) {
                    return Err(format!("line {l}: duplicate key `{key}`"));
                }
                let (ev, vl) = next(p)?;
                let value = node(p, ev, vl)?;
                v.push(Entry {
                    line: l,
                    key,
                    value,
                });
            }
            Value::Map(v)
        }
        Event::Alias(_) => Value::Alias,
        _ => return Err(format!("line {line}: expected a value")),
    };
    Ok(Node { line, value })
}

/// The document's root (an empty map for an empty file).
fn parse(text: &str) -> Result<Node, String> {
    let mut p = Parser::new_from_str(text);
    let mut root = None;
    loop {
        let (ev, l) = next(&mut p)?;
        match ev {
            Event::StreamStart | Event::DocumentStart(_) | Event::DocumentEnd => {}
            Event::StreamEnd => break,
            ev if root.is_none() => root = Some(node(&mut p, ev, l)?),
            _ => return Err(format!("line {l}: more than one YAML document")),
        }
    }
    Ok(match root {
        Some(n) if !matches!(&n.value, Value::Scalar(s) if s.is_empty()) => n,
        _ => Node {
            line: 1,
            value: Value::Map(Vec::new()),
        },
    })
}

/// Read a YAML file and check it against `shape`, which is a map. The error names the line of
/// every problem, one line each: a syntax error stops at the first, the checks report them all.
pub fn read(text: &str, shape: &Shape) -> Result<Node, String> {
    let root = parse(text)?;
    let mut problems = Vec::new();
    if !matches!(root.value, Value::Map(_)) {
        return Err(format!(
            "line {}: expected a map at the top level",
            root.line
        ));
    }
    check(&root, shape, "", &mut problems);
    if problems.is_empty() {
        return Ok(root);
    }
    problems.sort_by_key(|p| p.0);
    let lines: Vec<String> = problems
        .into_iter()
        .map(|(l, m)| format!("line {l}: {m}"))
        .collect();
    Err(lines.join("\n"))
}

type Problems = Vec<(usize, String)>;

fn check(n: &Node, shape: &Shape, path: &str, out: &mut Problems) {
    let wrong = |out: &mut Problems| {
        out.push((
            n.line,
            format!("`{path}` must be {}, not {}", shape.describe(), n.found()),
        ));
    };
    match (shape, &n.value) {
        (Shape::Custom(f), _) => out.extend(f(n, path).map(|m| (n.line, m))),
        (Shape::Text, Value::Scalar(_)) => {}
        (Shape::Word, Value::Scalar(s)) if !s.is_empty() => {}
        (Shape::Bool, Value::Scalar(s))
            if matches!(s.as_str(), "true" | "True" | "false" | "False") => {}
        (Shape::Count, Value::Scalar(s)) if s.parse::<usize>().is_ok() => {}
        (Shape::Choice(noun, words), Value::Scalar(s)) => {
            let low = s.to_ascii_lowercase();
            if !words.contains(&low.as_str()) {
                let near = tail(&low, noun, words.iter().copied());
                let said = format!("`{path}` must be {}, not `{s}`{near}", shape.describe());
                out.push((n.line, said));
            }
        }
        (Shape::List(_), Value::Scalar(s)) if s.is_empty() => {}
        (Shape::List(item), Value::Seq(items)) => {
            for i in items {
                check(i, item, path, out);
            }
        }
        (Shape::OneOrList(item), Value::Seq(items)) => {
            if items.is_empty() {
                out.push((n.line, format!("`{path}` is an empty list")));
            }
            for i in items {
                check(i, item, path, out);
            }
        }
        (Shape::OneOrList(item), Value::Scalar(_)) => check(n, item, path, out),
        (Shape::Map(m), Value::Map(entries)) => {
            check_map(n, entries, m.fields, &m.rest, (path, None, m.what), out);
        }
        (Shape::Tagged(t), Value::Map(entries)) => check_tagged(n, entries, t, path, out),
        _ => wrong(out),
    }
}

fn check_tagged(n: &Node, entries: &[Entry], t: &TaggedShape, path: &str, out: &mut Problems) {
    let Some(tag) = entries.iter().find(|e| e.key == t.tag) else {
        out.push((n.line, format!("missing `{}`", t.tag)));
        return;
    };
    let name = tag.value.text();
    let Some((_, fields)) = t.variants.iter().find(|(v, _)| *v == name) else {
        let near = tail(name, t.noun, t.variants.iter().map(|(v, _)| *v));
        let said = if matches!(tag.value.value, Value::Scalar(_)) {
            format!("unknown {} `{name}`{near}", t.noun)
        } else {
            let at = join(path, t.tag);
            format!("`{at}` must be a word, not {}", tag.value.found())
        };
        out.push((tag.value.line, said));
        return;
    };
    let place = Some(format!(" for {} `{name}`", t.tag));
    let needs = format!("{} `{name}`", t.tag);
    check_map(
        n,
        entries,
        fields,
        &Rest::Unknown,
        (path, place, &needs),
        out,
    );
}

/// `at`: the path of the map, what to say instead of "in `path`" after an unknown key, and what
/// "needs" a missing required key.
fn check_map(
    n: &Node,
    entries: &[Entry],
    fields: &[Field],
    rest: &Rest,
    at: (&str, Option<String>, &str),
    out: &mut Problems,
) {
    let (path, place, needs) = at;
    let place = place.unwrap_or_else(|| {
        if path.is_empty() {
            String::new()
        } else {
            format!(" in `{path}`")
        }
    });
    for e in entries {
        if let Some(f) = fields.iter().find(|f| f.key == e.key) {
            check(&e.value, f.shape, &join(path, f.key), out);
            continue;
        }
        match rest {
            Rest::Names { noun, known, near } => match known(&e.key) {
                Some(shape) => check(&e.value, shape, &join(path, &e.key), out),
                None => out.push((
                    e.line,
                    format!("unknown {noun} `{}`{place}{}", e.key, near(&e.key)),
                )),
            },
            Rest::Unknown => {
                let near = tail(&e.key, "key", fields.iter().map(|f| f.key));
                out.push((e.line, format!("unknown key `{}`{place}{near}", e.key)));
            }
        }
    }
    for f in fields.iter().filter(|f| f.required) {
        let given = entries.iter().find(|e| e.key == f.key);
        if given.is_none_or(|e| matches!(&e.value.value, Value::Scalar(s) if s.is_empty())) {
            out.push((n.line, format!("{needs} needs `{}`", f.key)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static WORDS: Shape = Shape::List(&Shape::Word);
    static ENTRY: MapShape = MapShape {
        what: "a waiver",
        fields: &[
            Field {
                key: "rule",
                shape: &Shape::OneOrList(&Shape::Word),
                required: true,
            },
            Field {
                key: "lines",
                shape: &Shape::List(&Shape::Count),
                required: false,
            },
            Field {
                key: "on",
                shape: &Shape::Bool,
                required: false,
            },
            Field {
                key: "level",
                shape: &Shape::Choice("level", &["error", "warning"]),
                required: false,
            },
        ],
        rest: Rest::Unknown,
    };
    static FILE: Shape = Shape::Map(&MapShape {
        what: "the file",
        fields: &[
            Field {
                key: "waivers",
                shape: &Shape::List(&Shape::Map(&ENTRY)),
                required: false,
            },
            Field {
                key: "exclude",
                shape: &Shape::List(&Shape::Word),
                required: false,
            },
        ],
        rest: Rest::Unknown,
    });

    fn err(t: &str) -> String {
        read(t, &FILE).unwrap_err()
    }

    #[test]
    fn a_valid_file_is_read_and_an_empty_one_is_an_empty_map() {
        let n = read(
            "waivers:\n  - rule: [a, b]\n    lines: [1, 2]\n    on: True\n    level: Error\nexclude:\n",
            &FILE,
        )
        .unwrap();
        assert_eq!(n.get("waivers").unwrap().items().len(), 1);
        assert_eq!(n.get("exclude").unwrap().words(), Vec::<String>::new());
        for t in ["", "# nothing\n", "---\n"] {
            assert!(read(t, &FILE).unwrap().entries().is_empty(), "{t}");
        }
        assert!(read("waivers:\n", &FILE).is_ok(), "an empty list");
    }

    #[test]
    fn an_empty_block_scalar_at_the_end_is_empty() {
        // saphyr-parser 0.3 reads it as "", 0.2 as "\n".
        let n = read("exclude: |\n", &FILE).unwrap();
        assert_eq!(n.get("exclude").unwrap().words(), Vec::<String>::new());
    }

    #[test]
    fn every_problem_of_a_file_is_reported_with_its_line() {
        let e = err(
            "wavers: 1\nwaivers:\n  - rule: a\n    lins: [1]\n    lines: [x]\n    on: maybe\n    level: fatal\n  - lines: [1]\nexclude: a\n",
        );
        let lines: Vec<&str> = e.lines().collect();
        assert_eq!(lines.len(), 7, "{e}");
        assert!(lines[0].starts_with("line 1: unknown key `wavers`"));
        assert!(lines[0].contains("a key with a similar name exists: `waivers`"));
        assert!(lines[1].starts_with("line 4: unknown key `lins` in `waivers`"));
        assert!(lines[1].contains("`lines`"));
        assert_eq!(
            lines[2],
            "line 5: `waivers.lines` must be a whole number, not `x`"
        );
        assert!(lines[3].contains("`waivers.on` must be true or false, not `maybe`"));
        assert!(
            lines[4].contains("`waivers.level` must be one of `error`, `warning`, not `fatal`")
        );
        assert!(lines[5].starts_with("line 8: a waiver needs `rule`"));
        assert!(lines[6].contains("`exclude` must be a list of words, not `a`"));
    }

    #[test]
    fn types_and_nesting_are_checked() {
        for (t, needle) in [
            ("waivers: x\n", "`waivers` must be a list of maps, not `x`"),
            ("waivers: [a]\n", "`waivers` must be a map, not `a`"),
            ("waivers:\n  - [a]\n", "must be a map, not a list"),
            (
                "waivers:\n  - rule: {a: 1}\n",
                "`waivers.rule` must be a word or a list of them, not a map",
            ),
            (
                "waivers:\n  - rule: []\n",
                "`waivers.rule` is an empty list",
            ),
            (
                "waivers:\n  - rule: &a x\n    level: *a\n",
                "must be one of",
            ),
            ("exclude: [[a]]\n", "must be a word, not a list"),
        ] {
            let e = err(t);
            assert!(e.contains(needle), "{t}: {e} / {needle}");
        }
        for t in ["- a\n", "a\n", "[a]\n"] {
            assert!(err(t).contains("expected a map at the top level"), "{t}");
        }
    }

    #[test]
    fn syntax_errors_and_duplicate_keys_stop_at_the_first() {
        assert!(err("a: 1\na: 2\n").contains("line 2: duplicate key `a`"));
        assert!(err("waivers: [\n").starts_with("line "));
        assert!(err("a: 1\n---\nb: 2\n").contains("more than one"));
        assert!(err("? [a]\n: 1\n").contains("a key must be a plain word"));
    }

    #[test]
    fn values_stay_the_text_they_were_written_as() {
        let n = read("exclude: [no, on, 0123, 1e5]\n", &FILE).unwrap();
        assert_eq!(
            n.get("exclude").unwrap().words(),
            ["no", "on", "0123", "1e5"]
        );
    }

    static LAYER: Shape = Shape::Map(&MapShape {
        what: "a layer",
        fields: &[Field {
            key: "disable",
            shape: &Shape::Bool,
            required: false,
        }],
        rest: Rest::Unknown,
    });

    fn known(id: &str) -> Option<&'static Shape> {
        id.starts_with("lint_").then_some(&LAYER)
    }

    fn near(id: &str) -> String {
        tail(id, "rule", ["lint_770"].into_iter())
    }

    fn even(n: &Node, path: &str) -> Option<String> {
        let odd = n.text().parse::<usize>().is_ok_and(|v| v % 2 == 1);
        odd.then(|| format!("`{}` in `{path}` is odd", n.text()))
    }

    #[test]
    fn names_and_custom_checks() {
        static ROOT: Shape = Shape::Map(&MapShape {
            what: "the file",
            fields: &[Field {
                key: "even",
                shape: &Shape::List(&Shape::Custom(even)),
                required: false,
            }],
            rest: Rest::Names {
                noun: "rule",
                known,
                near,
            },
        });
        let e = read(
            "lint_770: {disable: maybe, other: 1}\nlnt_770: {}\neven: [2, 3]\n",
            &ROOT,
        )
        .unwrap_err();
        assert!(
            e.contains("line 1: `lint_770.disable` must be true or false"),
            "{e}"
        );
        assert!(
            e.contains("line 1: unknown key `other` in `lint_770`"),
            "{e}"
        );
        assert!(
            e.contains(
                "line 2: unknown rule `lnt_770`; a rule with a similar name exists: `lint_770`"
            ),
            "{e}"
        );
        assert!(e.contains("line 3: `3` in `even` is odd"), "{e}");
    }

    #[test]
    fn tagged_maps_pick_their_keys_by_the_tag() {
        static T: TaggedShape = TaggedShape {
            tag: "kind",
            noun: "kind",
            variants: &[
                (
                    "statement",
                    &[
                        Field {
                            key: "kind",
                            shape: &Shape::Word,
                            required: true,
                        },
                        Field {
                            key: "hash",
                            shape: &Shape::Word,
                            required: true,
                        },
                    ],
                ),
                (
                    "unit",
                    &[
                        Field {
                            key: "kind",
                            shape: &Shape::Word,
                            required: true,
                        },
                        Field {
                            key: "unit",
                            shape: &Shape::Word,
                            required: true,
                        },
                    ],
                ),
            ],
        };
        static ROOT: Shape = Shape::Map(&MapShape {
            what: "the file",
            fields: &[Field {
                key: "exclude",
                shape: &Shape::List(&Shape::Tagged(&T)),
                required: false,
            }],
            rest: Rest::Unknown,
        });
        let e = read(
            "exclude:\n  - kind: statement\n    hash: a\n    unit: b\n  - kind: unit\n  - kind: unt\n    unit: x\n  - hash: a\n",
            &ROOT,
        )
        .unwrap_err();
        assert!(
            e.contains("line 4: unknown key `unit` for kind `statement`"),
            "{e}"
        );
        assert!(e.contains("line 5: kind `unit` needs `unit`"), "{e}");
        assert!(
            e.contains("line 6: unknown kind `unt`; a kind with a similar name exists: `unit`"),
            "{e}"
        );
        assert!(e.contains("line 8: missing `kind`"), "{e}");
        assert_eq!(WORDS.plural(), "lists");
        assert_eq!(
            ROOT.keys(),
            [
                "exclude",
                "exclude.kind",
                "exclude.hash",
                "exclude.kind",
                "exclude.unit"
            ]
        );
    }
}
