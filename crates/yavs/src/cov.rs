//! `yavs cov merge`, `yavs cov report` and `yavs cov rank` (docs/cli-contract.md section 5).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;
use yavs_cov::Database;
use yavs_diag::{Diagnostic, MessageFormat, SourceMap};

use crate::analyze::Out;

/// The form of `--uncovered`.
#[cfg_attr(test, derive(Debug))]
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum UncoveredFormat {
    Text,
    Yaml,
}

/// A kind of item `--kind` selects.
#[cfg_attr(test, derive(Debug))]
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum KindArg {
    Statement,
    Branch,
    Toggle,
    Condition,
    Expression,
    Combination,
    Fsm,
}

/// The form of `--exclusions`.
#[cfg_attr(test, derive(Debug))]
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ExclusionsFormat {
    Text,
    Json,
}

impl KindArg {
    fn name(self) -> String {
        let k = match self {
            KindArg::Statement => yavs_cov::Kind::Statement,
            KindArg::Branch => yavs_cov::Kind::Branch(0),
            KindArg::Toggle => yavs_cov::Kind::Toggle(true),
            KindArg::Condition => yavs_cov::Kind::Condition(0),
            KindArg::Expression => yavs_cov::Kind::Expression(0),
            KindArg::Combination => yavs_cov::Kind::Combination(0),
            KindArg::Fsm => yavs_cov::Kind::Fsm(false),
        };
        k.name().to_string()
    }
}

/// A percentage for `--fail-under`: 0 to 100, in tenths.
fn percentage(text: &str) -> Result<u64, String> {
    let v: f64 = text
        .parse()
        .map_err(|_| format!("`{text}` is not a number; write a percentage such as 80 or 92.5"))?;
    if !(0.0..=100.0).contains(&v) {
        return Err(format!("`{text}` is outside 0 to 100"));
    }
    // Needs at least the tenths the threshold names: 92.55 asks for 92.6.
    Ok((v * 10.0 - 1e-9).ceil().max(0.0) as u64)
}

/// What `--fail-under` requires: a total, a percentage per kind, or both, all in tenths.
#[cfg_attr(test, derive(Debug))]
#[derive(Clone, Default)]
pub struct FailUnder {
    total: Option<u64>,
    kinds: Vec<(String, u64)>,
}

/// `--fail-under`: comma separated `PCT` (the total) and `KIND:PCT`.
fn fail_under(text: &str) -> Result<FailUnder, String> {
    use clap::ValueEnum;
    let mut f = FailUnder::default();
    for part in text.split(',') {
        let Some((kind, pct)) = part.split_once(':') else {
            if f.total.replace(percentage(part)?).is_some() {
                return Err("the total is given twice".to_string());
            }
            continue;
        };
        let Some(k) = KindArg::value_variants()
            .iter()
            .map(|k| k.name())
            .find(|k| k == kind)
        else {
            let names = KindArg::value_variants().iter().map(|k| k.name());
            let names: Vec<String> = names.collect();
            let close = yavs_diag::suggest::tail(kind, "kind", names.iter().map(String::as_str));
            return Err(format!(
                "`{kind}` is not a coverage kind{close}; the kinds are {}",
                names.join(", ")
            ));
        };
        if f.kinds.iter().any(|(n, _)| *n == k) {
            return Err(format!("`{k}` is given twice"));
        }
        f.kinds.push((k, percentage(pct)?));
    }
    Ok(f)
}

#[cfg_attr(test, derive(Debug))]
// The parsed command line lives once and briefly: its size does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub enum Command {
    /// Add up the counts of several coverage databases into one.
    Merge {
        /// The merged database.
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        /// The coverage databases (`.ycov` files written by `yavs run --cov`).
        #[arg(required = true, value_name = "DB")]
        inputs: Vec<PathBuf>,
    },
    /// Print a text summary of coverage databases (merged), and write other reports.
    Report {
        /// Also write a Cobertura XML report.
        #[arg(long, value_name = "FILE.xml")]
        cobertura: Option<PathBuf>,
        /// Also write a JSON report with every kind and test (schema in the coverage guide).
        #[arg(long, value_name = "FILE.json")]
        json: Option<PathBuf>,
        /// Also write an LCOV tracefile: lines and branches only.
        #[arg(long, value_name = "FILE.info")]
        lcov: Option<PathBuf>,
        /// Also write a SonarQube generic coverage report: lines and branches only.
        #[arg(long, value_name = "FILE.xml")]
        sonarqube: Option<PathBuf>,
        /// Also write an HTML report (static files; sources embedded with
        /// `--cov-embed-sources`, else read from the paths in the database) into DIR.
        #[arg(long, value_name = "DIR")]
        html: Option<PathBuf>,
        /// Strip DIR from the front of the source paths the reports show (default: the
        /// current directory); a file outside DIR keeps its full path. The HTML report still
        /// reads sources from the paths in the database.
        #[arg(long, value_name = "DIR")]
        root: Option<PathBuf>,
        /// Report only these kinds of item (`statement`, `branch`, `toggle`, `condition`,
        /// `expression`, `combination`, comma separated); `--total` and `--fail-under` then count only them.
        #[arg(long, value_name = "LIST", value_delimiter = ',')]
        kind: Vec<KindArg>,
        /// Print only the total coverage, as one percentage (`70.0%`, rounded down): items that
        /// ran over all items, after exclusions. A database with nothing to count is `E0115`.
        #[arg(long, conflicts_with_all = ["uncovered", "tests"])]
        total: bool,
        /// Exit with status 1 and `E0114` when the total coverage is below PCT (0 to 100, such
        /// as 80 or 92.5), or a kind is below its own (`statement:90,branch:80`); the report is
        /// printed as usual. A PCT outside 0 to 100 or an unknown kind is a usage error.
        #[arg(long, value_name = "PCT|KIND:PCT,...", value_parser = fail_under)]
        fail_under: Option<FailUnder>,
        /// Limit the table of instances and packages to those below PCT, worst first, with a
        /// line counting the rest. Counts what the report counts (`--kind`, exclusions), rounded
        /// down. Totals and `--fail-under` are not affected.
        #[arg(
            long,
            value_name = "PCT",
            value_parser = percentage,
            conflicts_with_all = ["uncovered", "tests", "total", "exclusions"]
        )]
        below: Option<u64>,
        /// Also show, per instance after `--below`, `--kind` and the exclusions, a table per kind
        /// with every item, and for what never ran its source, what is missing and a tip.
        /// `--uncovered` is the row format for exclusion files; `--exclusions` the full audit.
        #[arg(long, conflicts_with_all = ["uncovered", "tests", "total", "exclusions"])]
        detailed: bool,
        /// Leave out the items an exclusion file lists (rows pasted from `--uncovered`; a
        /// `.yaml` or `.yml` file is read as YAML). Repeatable: the rules of all files add up. A
        /// file that cannot be read or has a bad row is `E0112`.
        #[arg(long, value_name = "FILE")]
        exclude: Vec<PathBuf>,
        /// Read options from a YAML file: `exclude` (exclusion files, added before those of
        /// `--exclude`), `root` (both relative to the file) and `kind`; `--root` and `--kind` win.
        /// Only this option reads such a file: nothing is found on its own. A file that cannot be
        /// read or has an unknown key is `E0138`.
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Print the exclusion report instead of the summary: every rule that takes requirements
        /// out of the totals (`-- yavs coverage exclude` directives and the rows of `--exclude`)
        /// with its origin, reason, what it matched, how often that ran, and the totals before
        /// and after (`--exclusions=json`: versioned JSON). The HTML report gets the same page.
        #[arg(
            long,
            value_name = "FORMAT",
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "text",
            conflicts_with_all = ["uncovered", "tests", "total"]
        )]
        exclusions: Option<ExclusionsFormat>,
        /// Print only the items that never ran, as exclusion rows (`--uncovered=yaml`: as a
        /// YAML exclusion file).
        #[arg(
            long,
            value_name = "FORMAT",
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "text"
        )]
        uncovered: Option<UncoveredFormat>,
        /// Print every item that ran, as an exclusion row with the tests that hit it.
        #[arg(long, conflicts_with = "uncovered")]
        tests: bool,
        /// Warning control, as for `analyze`: `-Werror`, `-Wno-CODE`, ...
        #[arg(short = 'W', value_name = "FLAG")]
        warn: Vec<String>,
        /// The coverage databases (`.ycov` files written by `yavs run --cov`).
        #[arg(required = true, value_name = "DB")]
        inputs: Vec<PathBuf>,
    },
    /// Rank the tests of merged databases: a minimal set reaching the same coverage, and
    /// what each test alone covers.
    Rank {
        /// Leave out the items an exclusion file lists. Repeatable: the rules of all files add
        /// up. A file that cannot be read or has a bad row is `E0112`.
        #[arg(long, value_name = "FILE")]
        exclude: Vec<PathBuf>,
        /// Read the `exclude` key of a YAML file, as `cov report --config` does (its other keys are
        /// checked and have no effect here). A file that cannot be read or has an unknown key is
        /// `E0138`.
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Warning control, as for `analyze`: `-Werror`, `-Wno-CODE`, ...
        #[arg(short = 'W', value_name = "FLAG")]
        warn: Vec<String>,
        /// The coverage databases (`.ycov` files written by `yavs run --cov`).
        #[arg(required = true, value_name = "DB")]
        inputs: Vec<PathBuf>,
    },
}

/// Cannot write a coverage database or report.
const WRITE_ERROR: &str = "E0110";
/// Cannot read or parse an exclusion file.
const EXCLUDE_ERROR: &str = "E0112";
/// Cannot read a coverage database.
const READ_ERROR: &str = "E0111";
/// Cannot read a source file named by the database (`--html`).
const SOURCE_ERROR: &str = "E0113";
/// The total coverage is below `--fail-under`.
const BELOW_ERROR: &str = "E0114";
/// `--total` or `--fail-under` over a database with no item of the selected kinds.
const EMPTY_ERROR: &str = "E0115";
/// Merged databases embed different text for one source file.
const SOURCE_CHANGED: &str = "W0121";
/// Merged databases disagree about an in-source exclusion.
const CONFLICT_ERROR: &str = "E0137";
/// Cannot read or parse the file of `--config`.
const CONFIG_ERROR: &str = "E0138";

/// What `--config FILE` asks for, its paths relative to the file.
#[derive(Default)]
struct Settings {
    exclude: Vec<PathBuf>,
    root: Option<PathBuf>,
    kinds: Vec<String>,
}

fn settings(config: Option<&PathBuf>) -> Result<Settings, Box<Diagnostic>> {
    let Some(p) = config else {
        return Ok(Settings::default());
    };
    let c = std::fs::read_to_string(p)
        .map_err(|e| e.to_string())
        .and_then(|t| yavs_cov::exclude::parse_config(&t))
        .map_err(|m| {
            Box::new(
                Diagnostic::error(
                    CONFIG_ERROR,
                    format!("cannot use coverage configuration file `{}`", p.display()),
                )
                .with_note(m),
            )
        })?;
    let dir = p.parent().unwrap_or(std::path::Path::new(""));
    Ok(Settings {
        exclude: c.exclude.iter().map(|f| dir.join(f)).collect(),
        root: c.root.map(|r| dir.join(r)),
        kinds: c.kind,
    })
}

fn merged(out: &mut Out, inputs: &[PathBuf]) -> Result<Database, Box<Diagnostic>> {
    let mut db = Database::default();
    for p in inputs {
        let d = std::fs::read_to_string(p)
            .map_err(|e| e.to_string())
            .and_then(|t| Database::parse(&t));
        match d {
            Ok(d) => {
                for (f, text) in &d.sources {
                    if db.sources.get(f).is_some_and(|t| t != text) {
                        let w = Diagnostic::warning(
                            SOURCE_CHANGED,
                            format!(
                                "the source `{f}` embedded in `{}` differs from the one before it; \
                                 the HTML report shows the last",
                                p.display()
                            ),
                        );
                        out.emit(&SourceMap::default(), &w);
                    }
                }
                let conflicts = db.merge(d);
                if !conflicts.is_empty() {
                    let mut e = Diagnostic::error(
                        CONFLICT_ERROR,
                        format!(
                            "`{}` disagrees with the databases before it about in-source exclusions",
                            p.display()
                        ),
                    );
                    for c in conflicts.iter().take(5) {
                        e = e.with_note(c.clone());
                    }
                    if conflicts.len() > 5 {
                        e = e.with_note(format!("... and {} more", conflicts.len() - 5));
                    }
                    return Err(Box::new(e.with_help(
                        "a waiver from one run would hide a requirement the other run kept: run                          all databases from the same sources, or drop the directive",
                    )));
                }
            }
            Err(m) => {
                return Err(Box::new(
                    Diagnostic::error(
                        READ_ERROR,
                        format!("cannot read coverage database `{}`", p.display()),
                    )
                    .with_note(m),
                ));
            }
        }
    }
    Ok(db)
}

fn write(path: &PathBuf, text: String) -> Result<(), Box<Diagnostic>> {
    std::fs::write(path, text).map_err(|e| {
        Box::new(
            Diagnostic::error(WRITE_ERROR, format!("cannot write `{}`", path.display()))
                .with_note(e.to_string()),
        )
    })
}

/// Excluded items with the reasons of their rows.
type Excluded = Vec<(yavs_cov::Key, yavs_cov::Entry, String)>;

/// `db` without the items the exclusion files list and the in-source directives waive, those
/// items, and the report of every rule.
fn excluded(
    out: &mut Out,
    db: Database,
    exclude: &[PathBuf],
    root: &str,
) -> Result<(Database, Excluded, yavs_cov::exclusions::Report), Box<Diagnostic>> {
    let files = exclude
        .iter()
        .map(|p| {
            yavs_cov::exclude::Rules::read(p).map_err(|m| {
                Box::new(
                    Diagnostic::error(
                        EXCLUDE_ERROR,
                        format!("cannot use exclusion file `{}`", p.display()),
                    )
                    .with_note(m),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if db.no_units
        && let Some((f, (line, word))) = files.iter().find_map(|f| f.needs_units().map(|n| (f, n)))
    {
        return Err(Box::new(
            Diagnostic::error(
                EXCLUDE_ERROR,
                format!(
                    "{}:{line}: a `{word}` row cannot be applied to these databases",
                    f.name
                ),
            )
            .with_note(
                "a database written by an older yavs is included; it does not record the library, \
                 entity and process of each item",
            )
            .with_help("run the simulations again with this yavs, or take the row out"),
        ));
    }
    let a = yavs_cov::exclude::apply_showing(&db, &files, &|f| relative(f, root));
    for w in &a.warnings {
        out.emit(&SourceMap::default(), w);
    }
    let report = yavs_cov::exclusions::build(&db, &a);
    Ok((a.kept, a.excluded, report))
}

/// Split a path into (Windows style, components) with `.` and `..` collapsed lexically. A path
/// is Windows style when it starts with a drive letter or `\\` / `//`: then both separators
/// split, `\\?\` (verbatim) prefixes drop and the drive letter folds to lower case.
fn components(p: &str) -> (bool, Vec<String>) {
    let b = p.as_bytes();
    let win = (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
        || p.starts_with("\\\\")
        || p.starts_with("//");
    let p = if win {
        p.replace('\\', "/")
    } else {
        p.to_string()
    };
    let p = p
        .strip_prefix("//?/UNC/")
        .map(|r| format!("//{r}"))
        .or_else(|| p.strip_prefix("//?/").map(str::to_string))
        .unwrap_or(p);
    let mut out: Vec<String> = Vec::new();
    for (i, c) in p.split('/').enumerate() {
        match c {
            "." => {}
            ".." if out.last().is_some_and(|l| l != "..") => {
                out.pop();
            }
            "" if i > 0 => {}
            c if win && i == 0 && c.ends_with(':') => out.push(c.to_ascii_lowercase()),
            c => out.push(c.to_string()),
        }
    }
    (win, out)
}

/// `file` without the prefix `root`, if it starts with it. Either side may use `\` or `/`, have a
/// drive letter in any case, a `\\?\` prefix or `.`/`..` components. Windows paths compare
/// case-insensitively and are shown with `/`.
fn relative(file: &str, root: &str) -> String {
    let lexical = relative_lexical(file, root);
    if lexical != file || !Path::new(file).is_absolute() || !Path::new(root).is_absolute() {
        return lexical;
    }
    // A symlink in the file or the root: compare what they resolve to. The shown path is the
    // resolved one, as `--root` is not about how the file was reached.
    match relative_lexical(&resolved(file), &resolved(root)) {
        r if Path::new(&r).is_absolute() => file.to_string(),
        r => r,
    }
}

/// `p` resolved as [`yavs_diag::files::resolve`] does, remembered: a report asks per item.
fn resolved(p: &str) -> String {
    thread_local!(static MEMO: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default());
    MEMO.with_borrow_mut(|m| {
        m.entry(p.to_string())
            .or_insert_with(|| {
                yavs_diag::files::resolve(Path::new(p))
                    .display()
                    .to_string()
            })
            .clone()
    })
}

fn relative_lexical(file: &str, root: &str) -> String {
    let ((fw, f), (rw, r)) = (components(file), components(root));
    let same = |a: &String, b: &String| {
        if fw {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    if fw == rw && f.len() > r.len() && r.iter().zip(&f).all(|(a, b)| same(a, b)) {
        f[r.len()..].join("/")
    } else {
        file.to_string()
    }
}

/// The directory the shown paths are relative to: `--root`, else the current directory.
fn root_dir(root: Option<PathBuf>) -> Result<String, Box<Diagnostic>> {
    let root = match root {
        Some(r) => std::path::absolute(&r).map_err(|e| {
            Box::new(
                Diagnostic::error(SOURCE_ERROR, format!("cannot resolve `{}`", r.display()))
                    .with_note(e.to_string()),
            )
        })?,
        None => std::env::current_dir().map_err(|e| {
            Box::new(
                Diagnostic::error(SOURCE_ERROR, "cannot find the current directory")
                    .with_note(e.to_string())
                    .with_help("pass `--root DIR`"),
            )
        })?,
    };
    Ok(root.display().to_string())
}

/// The database and excluded items with source paths relative to `root`.
fn shown(db: &Database, excluded: &Excluded, root: &str) -> (Database, Excluded) {
    let (mut db, mut excluded) = (db.clone(), excluded.clone());
    let all = db
        .items
        .values_mut()
        .chain(excluded.iter_mut().map(|x| &mut x.1));
    all.for_each(|e| e.file = relative(&e.file, root));
    db.unmeasured = std::mem::take(&mut db.unmeasured)
        .into_iter()
        .map(|(f, l, t)| (relative(&f, root), l, t))
        .collect();
    (db, excluded)
}

/// Write the HTML report into `dir`; every source file the database names and does not embed
/// must be readable.
/// Pages show the paths without the prefix `root` (default the current directory).
fn write_html(
    dir: &PathBuf,
    root: Option<PathBuf>,
    db: &Database,
    excluded: &Excluded,
    exclusions: &yavs_cov::exclusions::Report,
) -> Result<(), Box<Diagnostic>> {
    let mut sources = std::collections::BTreeMap::new();
    for f in yavs_cov::html::files(db, excluded) {
        if let Some(t) = db.sources.get(f) {
            sources.insert(f.to_string(), t.clone());
            continue;
        }
        let t = std::fs::read_to_string(f).map_err(|e| {
            Box::new(
                Diagnostic::error(SOURCE_ERROR, format!("cannot read source file `{f}`"))
                    .with_note(e.to_string())
                    .with_help(
                        "run `yavs cov report --html` where the paths in the database resolve, \
                         or run the simulation with `--cov-embed-sources`",
                    ),
            )
        })?;
        sources.insert(f.to_string(), t);
    }
    let root = root_dir(root)?;
    let (db, excluded) = shown(db, excluded, &root);
    let sources = sources
        .into_iter()
        .map(|(f, t)| (relative(&f, &root), t))
        .collect();
    let (db, excluded) = (&db, &excluded);
    std::fs::create_dir_all(dir).map_err(|e| {
        Box::new(
            Diagnostic::error(WRITE_ERROR, format!("cannot write `{}`", dir.display()))
                .with_note(e.to_string()),
        )
    })?;
    for (name, text) in yavs_cov::html::pages(db, excluded, &sources, Some(exclusions)) {
        write(&dir.join(name), text)?;
    }
    Ok(())
}

/// The sources `--detailed` shows, by the path it shows: embedded in the database, else read from
/// the stored path. A file that cannot be read is said so there, not an error.
fn detailed_sources(db: &Database, excluded: &Excluded, root: &str) -> yavs_cov::detail::Sources {
    let mut out = yavs_cov::detail::Sources::new();
    for f in yavs_cov::html::files(db, excluded) {
        let text = match db.sources.get(f) {
            Some(t) => Ok(t.clone()),
            None => std::fs::read_to_string(f).map_err(|e| {
                format!("{e}; run the simulation with `--cov-embed-sources` to keep the text")
            }),
        };
        out.insert(relative(f, root), text);
    }
    out
}

/// What `yavs cov report` was asked for besides the database.
struct Report {
    cobertura: Option<PathBuf>,
    json: Option<PathBuf>,
    lcov: Option<PathBuf>,
    sonarqube: Option<PathBuf>,
    html: Option<PathBuf>,
    root: Option<PathBuf>,
    exclude: Vec<PathBuf>,
    exclusions: Option<ExclusionsFormat>,
    kinds: Vec<String>,
    uncovered: Option<UncoveredFormat>,
    tests: bool,
    total: bool,
    fail_under: Option<FailUnder>,
    below: Option<u64>,
    detailed: bool,
    color: bool,
}

fn report(out: &mut Out, db: Database, r: Report) -> Result<(), Box<Diagnostic>> {
    let root = root_dir(r.root.clone())?;
    let (db, mut excluded, mut exclusions) = excluded(out, db, &r.exclude, &root)?;
    let db = if r.kinds.is_empty() {
        db
    } else {
        excluded.retain(|x| r.kinds.iter().any(|n| n == x.0.kind.name()));
        db.only(&r.kinds)
    };
    let (shown_db, shown_excluded) = shown(&db, &excluded, &root);
    exclusions.map_files(&|f| relative(f, &root));
    let (hit, all) = db.totals();
    let need = r.fail_under.unwrap_or_default();
    if (r.total || need.total.is_some()) && all == 0 {
        return Err(Box::new(
            Diagnostic::error(EMPTY_ERROR, "no coverage items to total")
                .with_note(if r.kinds.is_empty() {
                    "the database has no items after the exclusions".to_string()
                } else {
                    format!("the database has no `{}` items", r.kinds.join("`, `"))
                })
                .with_help("run with `--cov-kind` naming the kinds to count"),
        ));
    }
    let of_kind = |kind: &str| {
        let d = db.only(std::slice::from_ref(&kind.to_string()));
        d.totals()
    };
    let empty: Vec<&str> = need
        .kinds
        .iter()
        .filter(|(k, _)| of_kind(k).1 == 0)
        .map(|(k, _)| k.as_str())
        .collect();
    if !empty.is_empty() {
        return Err(Box::new(
            Diagnostic::error(EMPTY_ERROR, "no coverage items to total")
                .with_note(format!(
                    "`--fail-under` names `{}`, and the database has no such items after \
                     the exclusions and `--kind`",
                    empty.join("`, `")
                ))
                .with_help("run with `--cov-kind` naming the kinds to count"),
        ));
    }
    if let Some(f) = r.exclusions {
        match f {
            ExclusionsFormat::Text => print!("{}", exclusions.text()),
            ExclusionsFormat::Json => print!("{}", exclusions.json()),
        }
    } else if let Some(f) = r.uncovered {
        match f {
            UncoveredFormat::Text => print!("{}", shown_db.uncovered()),
            UncoveredFormat::Yaml => print!("{}", shown_db.uncovered_yaml()),
        }
    } else if r.tests {
        print!("{}", shown_db.hits_by_test());
    } else if r.total {
        println!("{}", yavs_cov::percent(hit, all));
    } else {
        let sources = r.detailed.then(|| detailed_sources(&db, &excluded, &root));
        let options = yavs_cov::SummaryOptions {
            below: r.below,
            detailed: r.detailed,
            excluded: Some(&shown_excluded),
            sources: sources.as_ref(),
            color: r.color,
        };
        print!("{}", shown_db.summary_with(&options));
        if !r.detailed {
            print!("{}", yavs_cov::exclude::excluded_text(&shown_excluded));
        }
    }
    if let Some(d) = r.html {
        write_html(&d, r.root, &db, &excluded, &exclusions)?;
    }
    if let Some(p) = r.cobertura {
        write(&p, shown_db.cobertura())?;
    }
    if let Some(p) = r.json {
        write(&p, shown_db.json())?;
    }
    if let Some(p) = r.lcov {
        write(&p, shown_db.lcov())?;
    }
    if let Some(p) = r.sonarqube {
        write(&p, shown_db.sonarqube())?;
    }
    let required = |t: u64| format!("{}.{}%", t / 10, t % 10);
    let mut below = Vec::new();
    if let (Some(t), Some(have)) = (need.total, yavs_cov::tenths(hit, all))
        && have < t
    {
        let got = yavs_cov::percent(hit, all);
        below.push(format!(
            "coverage {got} is below the required {}",
            required(t)
        ));
    }
    for (k, t) in &need.kinds {
        let (h, a) = of_kind(k);
        if yavs_cov::tenths(h, a).is_some_and(|have| have < *t) {
            let got = yavs_cov::percent(h, a);
            below.push(format!(
                "{k} coverage {got} is below the required {}",
                required(*t)
            ));
        }
    }
    match below.len() {
        0 => Ok(()),
        _ => Err(Box::new(Diagnostic::error(BELOW_ERROR, below.join("; ")))),
    }
}

pub fn run(c: Command, format: MessageFormat, color: bool) -> ExitCode {
    let mut out = Out::new(format, color);
    let r = match c {
        Command::Merge { output, inputs } => {
            merged(&mut out, &inputs).and_then(|db| write(&output, db.write()))
        }
        Command::Report {
            cobertura,
            json,
            lcov,
            sonarqube,
            html,
            root,
            kind,
            total,
            fail_under,
            below,
            detailed,
            exclude,
            config,
            exclusions,
            uncovered,
            tests,
            warn,
            inputs,
        } => out
            .set_warn(&warn)
            .and_then(|()| settings(config.as_ref()))
            .and_then(|mut set| {
                merged(&mut out, &inputs).map(|db| {
                    set.exclude.extend(exclude);
                    (db, set)
                })
            })
            .and_then(|(db, set)| {
                let kinds: Vec<String> = kind.into_iter().map(KindArg::name).collect();
                let r = Report {
                    cobertura,
                    json,
                    lcov,
                    sonarqube,
                    html,
                    root: root.or(set.root),
                    exclude: set.exclude,
                    exclusions,
                    kinds: if kinds.is_empty() { set.kinds } else { kinds },
                    uncovered,
                    tests,
                    total,
                    fail_under,
                    below,
                    detailed,
                    color,
                };
                report(&mut out, db, r)
            }),
        Command::Rank {
            exclude,
            config,
            warn,
            inputs,
        } => out
            .set_warn(&warn)
            .and_then(|()| settings(config.as_ref()))
            .and_then(|mut set| {
                merged(&mut out, &inputs).map(|db| {
                    set.exclude.extend(exclude);
                    (db, set.exclude)
                })
            })
            .and_then(|(db, exclude)| excluded(&mut out, db, &exclude, &root_dir(None)?))
            .map(|(db, ..)| print!("{}", db.rank())),
    };
    match r {
        Ok(()) => out.finish(u8::from(out.errors > 0)),
        Err(d) => {
            out.emit(&SourceMap::default(), &d);
            out.finish(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::relative;

    #[test]
    fn relative_unix() {
        assert_eq!(relative("/a/b/c.vhd", "/a/b"), "c.vhd");
        assert_eq!(relative("/a/b/d/c.vhd", "/a/x/../b/./"), "d/c.vhd");
        assert_eq!(relative("/a/bc/c.vhd", "/a/b"), "/a/bc/c.vhd");
        assert_eq!(relative("/A/b/c.vhd", "/a/b"), "/A/b/c.vhd");
        assert_eq!(relative("rel/c.vhd", "/a"), "rel/c.vhd");
    }

    #[test]
    fn relative_windows() {
        let want = "tests/cli/c.vhd";
        // Backslashes, a root with `..` and mixed separators, as cargo builds it.
        let root = r"D:\a\yavs\yavs\crates\yavs/../../tests";
        assert_eq!(
            relative(r"D:\a\yavs\yavs\tests\cli\c.vhd", root),
            "cli/c.vhd"
        );
        assert_eq!(
            relative(r"D:\a\yavs\yavs\tests\cli\c.vhd", r"D:\a\yavs\yavs"),
            want
        );
        // Drive letter and directory case, forward slashes, verbatim and UNC prefixes.
        assert_eq!(
            relative(r"d:/a/YAVS/yavs/tests/cli/c.vhd", r"D:\a\yavs\yavs"),
            want
        );
        assert_eq!(
            relative(r"\\?\D:\a\yavs\yavs\tests\cli\c.vhd", r"D:\a\yavs\yavs"),
            want
        );
        assert_eq!(
            relative(r"D:\a\yavs\yavs\tests\cli\c.vhd", r"\\?\d:\a\yavs\yavs"),
            want
        );
        assert_eq!(relative(r"\\?\UNC\srv\sh\x\c.vhd", r"\\srv\sh"), "x/c.vhd");
        // Another drive or a sibling directory stays absolute.
        assert_eq!(relative(r"E:\a\c.vhd", r"D:\a"), r"E:\a\c.vhd");
        assert_eq!(relative(r"D:\ab\c.vhd", r"D:\a"), r"D:\ab\c.vhd");
        // A Windows file against a Unix root is not relative.
        assert_eq!(relative(r"D:\a\c.vhd", "/a"), r"D:\a\c.vhd");
    }
}
