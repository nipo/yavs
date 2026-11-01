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
//! Both files are declared as schemas here (`EXCLUSION_FILE`, `CONFIG_FILE`) and read and checked by
//! `yavs_diag::yaml`, which reports every problem at once; what is left in this module turns the
//! checked tree into rows. Every value is read as the text it was written as, so `no`, `on` and a
//! hash of digits only stay strings.

use super::{ItemKind, Row, Rule, direction, item_rule};
use yavs_diag::files::normalize;
use yavs_diag::yaml::{self, Field, Node, Rest, Shape, TaggedShape, Value};

fn err(line: usize, m: &str) -> String {
    format!("line {line}: {m}")
}

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

macro_rules! field {
    ($k:expr, $shape:expr) => {
        Field {
            key: $k,
            shape: $shape,
            required: false,
        }
    };
    ($k:expr, $shape:expr, needed) => {
        Field {
            key: $k,
            shape: $shape,
            required: true,
        }
    };
}

/// The fields of an entry of each kind: `kind` first, then what it needs, then what it may have.
static ENTRY: TaggedShape = TaggedShape {
    tag: "kind",
    noun: "exclusion kind",
    variants: &[
        ("statement", &ITEM),
        ("branch", &ITEM),
        (
            "condition",
            &[
                field!("kind", &Shape::Word, needed),
                field!("instance", &Shape::Word, needed),
                field!("hash", &Shape::Word, needed),
                field!("row", &Shape::Word, needed),
                field!("line", &Shape::Text),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "expression",
            &[
                field!("kind", &Shape::Word, needed),
                field!("instance", &Shape::Word, needed),
                field!("hash", &Shape::Word, needed),
                field!("row", &Shape::Word, needed),
                field!("line", &Shape::Text),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "combination",
            &[
                field!("kind", &Shape::Word, needed),
                field!("instance", &Shape::Word, needed),
                field!("hash", &Shape::Word, needed),
                field!("pattern", &Shape::Word, needed),
                field!("line", &Shape::Text),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "toggle",
            &[
                field!("kind", &Shape::Word, needed),
                field!("instance", &Shape::Word, needed),
                field!("signal", &Shape::Word, needed),
                field!("direction", &Shape::Text),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "fsm",
            &[
                field!("kind", &Shape::Word, needed),
                field!("instance", &Shape::Word, needed),
                field!("signal", &Shape::Word, needed),
                field!("item", &Shape::Word, needed),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "instance",
            &[
                field!("kind", &Shape::Word, needed),
                field!("instance", &Shape::Word, needed),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "unit",
            &[
                field!("kind", &Shape::Word, needed),
                field!("unit", &Shape::Word, needed),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "file",
            &[
                field!("kind", &Shape::Word, needed),
                field!("file", &Shape::Text),
                field!("files", &Shape::OneOrList(&Shape::Word)),
                field!("files_exclude", &Shape::OneOrList(&Shape::Word)),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "library",
            &[
                field!("kind", &Shape::Word, needed),
                field!("library", &Shape::Word, needed),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "entity",
            &[
                field!("kind", &Shape::Word, needed),
                field!("entity", &Shape::Word, needed),
                field!("reason", &Shape::Text),
            ],
        ),
        (
            "process",
            &[
                field!("kind", &Shape::Word, needed),
                field!("instance", &Shape::Word, needed),
                field!("process", &Shape::Word, needed),
                field!("reason", &Shape::Text),
            ],
        ),
    ],
};

/// The fields of a `statement` or `branch` entry.
const ITEM: [Field; 5] = [
    field!("kind", &Shape::Word, needed),
    field!("instance", &Shape::Word, needed),
    field!("hash", &Shape::Word, needed),
    field!("line", &Shape::Text),
    field!("reason", &Shape::Text),
];

/// An exclusion file in YAML form.
pub static EXCLUSION_FILE: Shape = Shape::Map(&yaml::MapShape {
    what: "the exclusion file",
    fields: &[field!("exclude", &Shape::List(&Shape::Tagged(&ENTRY)))],
    rest: Rest::Unknown,
});

/// `exclude` of a coverage configuration file: exclusion files, not exclusions.
fn exclusion_files(n: &Node, path: &str) -> Option<String> {
    let words = |items: &[Node]| {
        !items.is_empty()
            && items
                .iter()
                .all(|i| matches!(&i.value, Value::Scalar(s) if !s.is_empty()))
    };
    match &n.value {
        Value::Scalar(s) if !s.is_empty() => None,
        Value::Seq(items) if items.iter().any(|i| matches!(i.value, Value::Map(_))) => {
            Some(format!(
                "`{path}` lists exclusion files; this looks like an exclusion file itself: pass it with `--exclude`"
            ))
        }
        Value::Seq(items) if words(items) => None,
        _ => Some(format!("`{path}` must be a word or a list of them")),
    }
}

/// A coverage configuration file (`yavs cov report --config`).
pub static CONFIG_FILE: Shape = Shape::Map(&yaml::MapShape {
    what: "the configuration file",
    fields: &[
        field!("exclude", &Shape::Custom(exclusion_files)),
        field!("root", &Shape::Word),
        field!(
            "kind",
            &Shape::OneOrList(&Shape::Choice("kind", &ITEM_KINDS))
        ),
    ],
    rest: Rest::Unknown,
});

/// Parse a YAML exclusion file; the error names the line of every problem the schema finds.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let root = yaml::read(text, &EXCLUSION_FILE)?;
    let items = root.get("exclude").map_or(&[][..], Node::items);
    items.iter().map(entry).collect()
}

/// One entry of a checked exclusion file: its keys are the kind's, so what is left to refuse is
/// what no schema says (`file` against `files`, the form of a hash).
fn entry(n: &Node) -> Result<Row, String> {
    let line = n.line;
    let mut get: Vec<(usize, &str, &str)> = Vec::new();
    // `files` and `files_exclude`, of a `file` entry: one word or a list.
    let mut lists: Vec<(usize, &str, Vec<String>)> = Vec::new();
    for e in n.entries() {
        match e.key.as_str() {
            "files" | "files_exclude" => lists.push((e.line, &e.key, e.value.words())),
            _ => get.push((e.line, &e.key, e.value.text())),
        }
    }
    let find = |k: &str| get.iter().find(|f| f.1 == k).map(|f| f.2);
    let kind = find("kind").unwrap_or_default();
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

/// Parse a coverage configuration file; the error names the line of every problem.
pub fn parse_config(text: &str) -> Result<Config, String> {
    let root = yaml::read(text, &CONFIG_FILE)?;
    let words = |key: &str| root.get(key).map(Node::words).unwrap_or_default();
    Ok(Config {
        exclude: words("exclude"),
        root: words("root").pop(),
        kind: words("kind"),
    })
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
