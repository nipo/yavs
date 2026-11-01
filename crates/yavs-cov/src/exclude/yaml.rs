//! The YAML form of an exclusion file (`*.yaml`, `*.yml`):
//!
//! ```yaml
//! exclude:
//!   - kind: statement          # statement | branch | condition | expression | combination | toggle | instance | unit | file | library | entity | process
//!     instance: ":tb:dut:u_fsm"
//!     hash: "4d2f0a91c3be.0"   # branch: HEX12.OCC.ARM
//!     line: "src/fsm.vhd:61"   # optional, never matched
//!     reason: free text        # optional
//! ```
//!
//! A `condition` or `expression` entry has `hash: "HEX12.OCC"` and `row: N` (2 * input + value).
//! A `toggle` entry has `signal` instead of `hash` and an optional `direction: rise|fall`.
//! A `file` entry has `file`, or `files` and `files_exclude` (one word or a list each), the keys
//! of a lint waiver.
//!
//! Every value is read as the text it was written as, so `no`, `on` and a hash of digits only
//! stay strings. The parser is `saphyr-parser` (events with line numbers), not a data model.

use super::{ItemKind, Row, Rule, direction, item_rule};
use saphyr_parser::{Event, Parser};
use yavs_diag::files::normalize;
use yavs_diag::suggest::tail;

/// The kinds of exclusion an entry may have.
pub(super) const KINDS: [&str; 13] = [
    "statement",
    "branch",
    "condition",
    "expression",
    "combination",
    "toggle",
    "fsm",
    "instance",
    "unit",
    "file",
    "library",
    "entity",
    "process",
];

enum Node {
    Scalar(String),
    Seq(Vec<(usize, Node)>),
    Map(Vec<(usize, String, Node)>),
}

type Events<'a> = Parser<'a, saphyr_parser::StrInput<'a>>;

fn err(line: usize, m: &str) -> String {
    format!("line {line}: {m}")
}

/// The next event with its 1-based line.
fn next<'a>(p: &mut Events<'a>) -> Result<(Event<'a>, usize), String> {
    match p.next() {
        Some(Ok((ev, sp))) => Ok((ev, sp.start.line())),
        Some(Err(e)) => Err(err(e.marker().line(), e.info())),
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
                v.push((l, node(p, ev, l)?));
            }
        }
        Event::MappingStart(..) => {
            let mut v: Vec<(usize, String, Node)> = Vec::new();
            loop {
                let (ev, l) = next(p)?;
                let key = match ev {
                    Event::MappingEnd => return Ok(Node::Map(v)),
                    Event::Scalar(k, ..) => k.to_string(),
                    _ => return Err(err(l, "a key must be a plain word")),
                };
                if v.iter().any(|(_, k, _)| *k == key) {
                    return Err(err(l, &format!("duplicate key `{key}`")));
                }
                let (ev, vl) = next(p)?;
                v.push((l, key, node(p, ev, vl)?));
            }
        }
        Event::Alias(_) => Err(err(line, "aliases are not supported")),
        _ => Err(err(line, "unexpected YAML structure")),
    }
}

/// The root node of a YAML document, if it has one.
fn document(text: &str) -> Result<Option<Node>, String> {
    let mut p = Parser::new_from_str(text);
    let mut root = None;
    loop {
        let (ev, l) = next(&mut p)?;
        match ev {
            Event::StreamStart | Event::DocumentStart(_) | Event::DocumentEnd => {}
            Event::StreamEnd => break,
            ev if root.is_none() => root = Some((l, node(&mut p, ev, l)?)),
            _ => return Err(err(l, "more than one YAML document")),
        }
    }
    Ok(root.map(|r: (usize, Node)| r.1))
}

/// Parse a YAML exclusion file; the error names the line.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let Some(root) = document(text)? else {
        return Ok(Vec::new());
    };
    let Node::Map(top) = root else {
        return Err(err(1, "expected a top-level `exclude:` list"));
    };
    let mut rows = Vec::new();
    for (kl, key, v) in top {
        if key != "exclude" {
            return Err(err(
                kl,
                &format!(
                    "unknown key `{key}` (expected `exclude`){}",
                    tail(&key, "key", ["exclude"].into_iter())
                ),
            ));
        }
        match v {
            Node::Seq(items) => {
                for (il, item) in items {
                    rows.push(entry(il, item)?);
                }
            }
            // `exclude:` with every entry commented out
            Node::Scalar(s) if s.is_empty() => {}
            _ => return Err(err(kl, "`exclude` must be a list of entries")),
        }
    }
    Ok(rows)
}

fn entry(line: usize, n: Node) -> Result<Row, String> {
    let Node::Map(fields) = n else {
        return Err(err(
            line,
            "an entry must be a map with `kind:` and its fields",
        ));
    };
    let mut get: Vec<(usize, &str, &str)> = Vec::new();
    // `files` and `files_exclude`, of a `file` entry: one word or a list.
    let mut lists: Vec<(usize, &str, Vec<String>)> = Vec::new();
    let is_file = fields
        .iter()
        .any(|(_, k, v)| k == "kind" && matches!(v, Node::Scalar(s) if s == "file"));
    for (l, k, v) in &fields {
        match v {
            Node::Scalar(_) | Node::Seq(_) if is_file && (k == "files" || k == "files_exclude") => {
                lists.push((*l, k, words(*l, k, v)?));
            }
            Node::Scalar(s) => get.push((*l, k, s)),
            _ => return Err(err(*l, &format!("`{k}` must be a single value"))),
        }
    }
    let find = |k: &str| get.iter().find(|f| f.1 == k).map(|f| f.2);
    let kind = find("kind").ok_or_else(|| err(line, "missing `kind`"))?;
    let (allowed, need): (&[&str], &[&str]) = match kind {
        "statement" | "branch" => (
            &["kind", "instance", "hash", "line", "reason"],
            &["instance", "hash"],
        ),
        "condition" | "expression" => (
            &["kind", "instance", "hash", "row", "line", "reason"],
            &["instance", "hash", "row"],
        ),
        "combination" => (
            &["kind", "instance", "hash", "pattern", "line", "reason"],
            &["instance", "hash", "pattern"],
        ),
        "toggle" => (
            &["kind", "instance", "signal", "direction", "reason"],
            &["instance", "signal"],
        ),
        "fsm" => (
            &["kind", "instance", "signal", "item", "reason"],
            &["instance", "signal", "item"],
        ),
        "instance" => (&["kind", "instance", "reason"], &["instance"]),
        "unit" => (&["kind", "unit", "reason"], &["unit"]),
        "file" => (&["kind", "file", "reason"], &[]),
        "library" => (&["kind", "library", "reason"], &["library"]),
        "entity" => (&["kind", "entity", "reason"], &["entity"]),
        "process" => (
            &["kind", "instance", "process", "reason"],
            &["instance", "process"],
        ),
        _ => {
            let l = get.iter().find(|f| f.1 == "kind").map_or(line, |f| f.0);
            let kinds = KINDS.into_iter();
            return Err(err(
                l,
                &format!(
                    "unknown exclusion kind `{kind}`{}",
                    tail(kind, "kind", kinds)
                ),
            ));
        }
    };
    for (l, k, _) in &get {
        if !allowed.contains(k) {
            let tail = tail(k, "key", allowed.iter().copied());
            return Err(err(
                *l,
                &format!("unknown key `{k}` for kind `{kind}`{tail}"),
            ));
        }
    }
    for k in need {
        if find(k).is_none_or(str::is_empty) {
            return Err(err(line, &format!("kind `{kind}` needs `{k}`")));
        }
    }
    if kind == "file" && find("file").is_some_and(str::is_empty) {
        return Err(err(line, "kind `file` needs `file`"));
    }
    if kind == "file" && find("file").is_some() == !lists.is_empty() {
        return Err(err(
            line,
            "kind `file` needs `file`, or `files` and `files_exclude`, not both",
        ));
    }
    let bad = |m: &str| err(line, &format!("{m}: kind `{kind}`"));
    let rule = match kind {
        "statement" | "branch" | "condition" | "expression" | "combination" => item_rule(
            ItemKind::word(kind).unwrap_or(ItemKind::Statement),
            find("instance").unwrap_or_default(),
            find("hash").unwrap_or_default(),
            find("row").or_else(|| find("pattern")),
            find("line").filter(|s| !s.is_empty()),
            &bad,
        )?,
        "toggle" => Rule::Toggle(
            yavs_diag::dotted(find("instance").unwrap_or_default()),
            find("signal").unwrap_or_default().into(),
            find("direction").map(|d| direction(d, &bad)).transpose()?,
        ),
        "fsm" => Rule::Fsm(
            yavs_diag::dotted(find("instance").unwrap_or_default()),
            find("signal").unwrap_or_default().into(),
            find("item").unwrap_or_default().into(),
        ),
        "instance" => Rule::Instance(yavs_diag::dotted(find("instance").unwrap_or_default())),
        "unit" => Rule::Unit(find("unit").unwrap_or_default().into()),
        "library" => Rule::Library(find("library").unwrap_or_default().into()),
        "entity" => Rule::Entity(find("entity").unwrap_or_default().into()),
        "process" => Rule::Process(
            yavs_diag::dotted(find("instance").unwrap_or_default()),
            find("process").unwrap_or_default().into(),
        ),
        _ if lists.is_empty() => Rule::File(find("file").unwrap_or_default().into()),
        _ => {
            let list = |key: &str| -> Vec<String> {
                let of = lists.iter().filter(|x| x.1 == key);
                of.flat_map(|x| x.2.iter().map(|p| normalize(p))).collect()
            };
            Rule::Files {
                files: list("files"),
                files_exclude: list("files_exclude"),
            }
        }
    };
    let text = std::iter::once(kind.to_string())
        .chain(
            get.iter()
                .filter(|f| f.1 != "kind" && f.1 != "reason")
                .map(|f| format!("{}={}", f.1, f.2)),
        )
        .chain(lists.iter().map(|x| format!("{}={}", x.1, x.2.join(","))))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Row {
        rule,
        text,
        reason: find("reason").unwrap_or_default().trim().to_string(),
        line,
    })
}

/// A coverage configuration file (`yavs cov report --config`): the options that say what a project
/// counts, so that they need not be repeated. Paths in it are as written; the caller joins them to
/// the file's directory.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// `--exclude`: exclusion files.
    pub exclude: Vec<String>,
    /// `--root`.
    pub root: Option<String>,
    /// `--kind`: the kinds, checked.
    pub kind: Vec<String>,
}

/// The kinds `--kind` and the `kind` key of a configuration file take.
const ITEM_KINDS: [&str; 6] = [
    "statement",
    "branch",
    "toggle",
    "condition",
    "expression",
    "combination",
];

/// Parse a coverage configuration file; the error names the line.
pub fn parse_config(text: &str) -> Result<Config, String> {
    let mut cfg = Config::default();
    let Some(root) = document(text)? else {
        return Ok(cfg);
    };
    let Node::Map(top) = root else {
        return Err(err(
            1,
            "expected a map with the keys `exclude`, `root` and `kind`",
        ));
    };
    for (l, key, v) in &top {
        match (key.as_str(), v) {
            ("exclude", Node::Seq(items)) if items.iter().any(|i| matches!(i.1, Node::Map(_))) => {
                return Err(err(
                    *l,
                    "`exclude` lists exclusion files; this looks like an exclusion file itself: \
                     pass it with `--exclude`",
                ));
            }
            ("exclude", v) => cfg.exclude = words(*l, key, v)?,
            ("root", Node::Scalar(s)) if !s.is_empty() => cfg.root = Some(s.clone()),
            ("root", _) => return Err(err(*l, "`root` must be one directory")),
            ("kind", v) => {
                cfg.kind = words(*l, key, v)?;
                if let Some(k) = cfg.kind.iter().find(|k| !ITEM_KINDS.contains(&k.as_str())) {
                    let near = tail(k, "kind", ITEM_KINDS.into_iter());
                    return Err(err(
                        *l,
                        &format!(
                            "unknown kind `{k}` (expected {}){near}",
                            ITEM_KINDS.join(", ")
                        ),
                    ));
                }
            }
            _ => {
                let near = tail(key, "key", ["exclude", "root", "kind"].into_iter());
                return Err(err(
                    *l,
                    &format!("unknown key `{key}` (expected `exclude`, `root` or `kind`){near}"),
                ));
            }
        }
    }
    Ok(cfg)
}

/// A value that is one word or a list of them, none empty.
fn words(line: usize, key: &str, v: &Node) -> Result<Vec<String>, String> {
    let bad = || err(line, &format!("`{key}` must be a word or a list of them"));
    let out = match v {
        Node::Scalar(s) if !s.is_empty() => vec![s.clone()],
        Node::Seq(items) if !items.is_empty() => items
            .iter()
            .map(|(_, n)| match n {
                Node::Scalar(s) if !s.is_empty() => Ok(s.clone()),
                _ => Err(bad()),
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(bad()),
    };
    Ok(out)
}

/// `s` as a YAML double-quoted scalar.
pub fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if c.is_control() => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
