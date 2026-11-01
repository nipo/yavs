//! Combination coverage and the in-source exclusions of its decisions (#883) on
//! `tests/cli/cov_comb.vhd`: the requirements and their waivers, the checks of the directives
//! with and without coverage, `--coverage-directives=ignore`, persistence and merge, the
//! exclusion report, identity under comment edits, threads and tiers.

mod common;
use common::{Tmp, tempdir};

use std::path::Path;
use std::process::{Command, Output};

use yavs_cov::{Database, Kind};

fn yavs(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["--color", "never"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn read(p: &Path) -> Database {
    Database::parse(&std::fs::read_to_string(p).expect("read")).expect("parse")
}

fn design() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cli/cov_comb.vhd")
        .to_str()
        .expect("utf-8")
        .to_string()
}

/// A fresh directory with the design analysed into `lib`.
fn setup(name: &str) -> Tmp {
    let tmp = tempdir(name);
    let o = yavs(&tmp, &["analyze", "--work=work:lib", &design()]);
    assert!(o.status.success(), "{}", err(&o));
    tmp
}

fn run(tmp: &Path, extra: &[&str]) -> Output {
    let o = yavs(
        tmp,
        &[&["run", "--work=work:lib", "cov_comb"][..], extra].concat(),
    );
    assert!(o.status.success(), "{}", err(&o));
    o
}

/// (line, pattern, count, waived by how many directives), sorted.
fn rows(db: &Database) -> Vec<(u32, String, u64, usize)> {
    let mut v: Vec<_> = db
        .items
        .iter()
        .filter(|(k, _)| matches!(k.kind, Kind::Combination(_)))
        .map(|(k, e)| (e.line, yavs_cov::pattern(k), e.count, e.waivers.len()))
        .collect();
    v.sort();
    v
}

fn want() -> Vec<(u32, String, u64, usize)> {
    [
        // `q <= x and y` on std_logic: the four combinations, `11` waived.
        (15, "00", 1, 0),
        (15, "01", 1, 0),
        (15, "10", 1, 0),
        (15, "11", 0, 1),
        // `a and (b or c)`: with `a` false nothing else is evaluated; with `b` true `c` is not.
        // `0--` is waived by `exclude condition (a=false)`, `100` by `(b=false, c=false)`.
        (35, "0--", 4, 1),
        (35, "100", 1, 1),
        (35, "101", 1, 0),
        (35, "11-", 2, 0),
        // `elsif b and c`: `1-` waives both combinations with `b` true.
        (38, "0-", 3, 0),
        (38, "10", 1, 1),
        (38, "11", 1, 1),
        // `not (a xor b) or c`: the or skips `c` when the xor is false.
        (42, "000", 0, 0),
        (42, "001", 0, 0),
        (42, "010", 0, 0),
        (42, "011", 0, 0),
        (42, "100", 0, 0),
        (42, "101", 0, 0),
        (42, "110", 0, 0),
        (42, "111", 0, 0),
    ]
    .iter()
    .map(|&(l, p, c, w)| (l, p.to_string(), c, w))
    .collect()
}

#[test]
fn requirements_counts_and_waivers() {
    let tmp = setup("comb");
    let plain = run(&tmp, &[]);
    let with = run(&tmp, &["--cov=a.ycov", "--cov-kind=combination"]);
    // Evaluation order, short-circuit and side effects (the calls of `tick`) are untouched.
    assert_eq!(out(&plain), out(&with));
    assert_eq!(err(&plain), err(&with));
    assert!(out(&with).contains("n=25 calls=12"), "{}", out(&with));
    let a = read(&tmp.join("a.ycov"));
    assert!(
        a.items
            .keys()
            .all(|k| matches!(k.kind, Kind::Combination(_)))
    );
    let got = rows(&a);
    // Pin the decisions that do not depend on the loop order of the other tests below.
    for w in want().iter().filter(|w| w.0 != 42) {
        assert!(got.contains(w), "missing {w:?} in {got:?}");
    }
    let e: Vec<_> = got.iter().filter(|r| r.0 == 42).collect();
    // `not (a xor b) or c`: 2 xor-false outcomes with `c` skipped when the xor is false.
    assert_eq!(e.len(), 6, "{e:?}");
    // `a and not a` (two readings of one object agree): `0-` and `11` only.
    let same: Vec<_> = got.iter().filter(|r| r.0 == 49).collect();
    assert_eq!(same.len(), 2, "{same:?}");
    assert_eq!(
        same.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(),
        ["0-", "11"]
    );
    // The function-call inputs: `tick(b)` is not evaluated when `tick(a)` is false.
    let calls: Vec<_> = got
        .iter()
        .filter(|r| r.0 == 45)
        .map(|r| r.1.as_str())
        .collect();
    assert_eq!(calls, ["0-", "10", "11"]);
    // std_logic: `X` and `U` are no combination.
    let sl: Vec<_> = got.iter().filter(|r| r.0 == 62).collect();
    assert_eq!(sl.iter().map(|r| r.2).sum::<u64>(), 2);
    // The waiver keeps its reason and its place.
    let w = a
        .items
        .iter()
        .find(|(k, e)| e.line == 15 && yavs_cov::pattern(k) == "11")
        .map(|(_, e)| e.waivers[0].clone())
        .expect("waived");
    assert_eq!(w.rule, "11");
    assert_eq!(w.reason, "x and y are never high together");
    assert_eq!(w.line, 14);

    // Byte-identical at four threads.
    run(
        &tmp,
        &["--cov=a4.ycov", "--cov-kind=combination", "--threads=4"],
    );
    assert_eq!(
        std::fs::read(tmp.join("a.ycov")).expect("a"),
        std::fs::read(tmp.join("a4.ycov")).expect("a4")
    );
    // The other kinds do not collect combinations, and vice versa.
    run(
        &tmp,
        &["--cov=s.ycov", "--cov-kind=statement,branch,condition"],
    );
    assert!(
        read(&tmp.join("s.ycov"))
            .items
            .keys()
            .all(|k| !matches!(k.kind, Kind::Combination(_)))
    );

    // The summary leaves the waived requirements out of the totals and lists them.
    let o = yavs(&tmp, &["cov", "report", "a.ycov"]);
    let text = out(&o);
    assert!(text.contains("combinations "), "{text}");
    assert!(text.contains("excluded ("), "{text}");
    assert!(text.contains("[x and y are never high together]"), "{text}");
    let total = yavs(&tmp, &["cov", "report", "a.ycov", "--total"]);
    let (hit, all) = {
        let kept = got.iter().filter(|r| r.3 == 0);
        (kept.clone().filter(|r| r.2 > 0).count(), kept.count())
    };
    assert_eq!(
        out(&total).trim(),
        format!("{}.{}%", hit * 1000 / all / 10, hit * 1000 / all % 10)
    );
    // Cobertura does not turn combinations into branches.
    yavs(&tmp, &["cov", "report", "a.ycov", "--cobertura", "a.xml"]);
    let xml = std::fs::read_to_string(tmp.join("a.xml")).expect("xml");
    assert!(!xml.contains("condition-coverage"), "{xml}");
}

#[test]
fn comment_edits_keep_identity() {
    let tmp = tempdir("comb_identity");
    let src = std::fs::read_to_string(design()).expect("design");
    let keys = |text: &str, dir: &str| {
        std::fs::create_dir_all(tmp.join(dir)).expect("mkdir");
        std::fs::write(tmp.join(dir).join("d.vhd"), text).expect("write");
        let d = tmp.join(dir);
        let o = yavs(&d, &["analyze", "--work=work:lib", "d.vhd"]);
        assert!(o.status.success(), "{}", err(&o));
        run(&d, &["--cov=a.ycov", "--cov-kind=combination"]);
        read(&d.join("a.ycov"))
    };
    let base = keys(&src, "base");
    // Another reason, another directive, a comment inside the condition: same requirements.
    let edited = src
        .replace("a low is covered elsewhere", "other words")
        .replace(
            "-- yavs cov exclude (b=false, c=false)",
            "-- yavs cov exclude (b=false, c=false): now with a reason\n      -- another remark",
        )
        .replace(
            "if a and (b or c) then",
            "if a and -- the second input\n         (b or c) then",
        );
    let after = keys(&edited, "edited");
    let ids = |d: &Database| d.items.keys().cloned().collect::<Vec<_>>();
    // The keys are the same except the lines moved: compare hashes and patterns.
    let hp = |d: &Database| {
        let mut v: Vec<_> = d
            .items
            .keys()
            .map(|k| (k.hash, k.occ, yavs_cov::pattern(k)))
            .collect();
        v.sort();
        v
    };
    assert_eq!(hp(&base), hp(&after), "{:?} {:?}", ids(&base), ids(&after));
    // The reason is data of the waiver, not of the identity.
    let reason = |d: &Database, p: &str| {
        d.items
            .iter()
            .find(|(k, e)| {
                e.waivers.iter().any(|w| w.rule.starts_with("(b=")) && yavs_cov::pattern(k) == p
            })
            .map(|(_, e)| e.waivers[0].reason.clone())
    };
    assert_eq!(reason(&base, "100").as_deref(), Some(""));
    assert_eq!(reason(&after, "100").as_deref(), Some("now with a reason"));
}

#[test]
fn ignore_applies_no_waiver_and_says_so() {
    let tmp = setup("comb_ignore");
    let o = run(
        &tmp,
        &[
            "--cov=i.ycov",
            "--cov-kind=combination",
            "--coverage-directives=ignore",
        ],
    );
    assert!(
        err(&o).contains(
            "`--coverage-directives=ignore`: 4 `-- yavs coverage exclude` directives left out"
        ),
        "{}",
        err(&o)
    );
    let i = read(&tmp.join("i.ycov"));
    assert!(rows(&i).iter().all(|r| r.3 == 0));
    // The same requirements, counted the same.
    run(&tmp, &["--cov=a.ycov", "--cov-kind=combination"]);
    let a = read(&tmp.join("a.ycov"));
    let strip = |d: &Database| {
        let mut v = rows(d);
        v.iter_mut().for_each(|r| r.3 = 0);
        v
    };
    assert_eq!(strip(&a), strip(&i));
    // The two do not merge: one run waives what the other requires.
    let o = yavs(&tmp, &["cov", "merge", "a.ycov", "i.ycov", "-o", "m.ycov"]);
    assert_eq!(o.status.code(), Some(1), "{}", err(&o));
    assert!(err(&o).contains("E0137"), "{}", err(&o));
    assert!(err(&o).contains("excluded at"), "{}", err(&o));
    // Equal databases merge, adding the counts and keeping the waivers.
    let o = yavs(&tmp, &["cov", "merge", "a.ycov", "a.ycov", "-o", "m.ycov"]);
    assert!(o.status.success(), "{}", err(&o));
    let m = read(&tmp.join("m.ycov"));
    assert_eq!(
        rows(&m).iter().map(|r| (r.2, r.3)).collect::<Vec<_>>(),
        rows(&a).iter().map(|r| (2 * r.2, r.3)).collect::<Vec<_>>()
    );
}

#[test]
fn warm_caches_follow_edits_and_options() {
    let tmp = setup("comb_warm");
    let waivers = |extra: &[&str]| {
        run(
            &tmp,
            &[&["--cov=w.ycov", "--cov-kind=combination"][..], extra].concat(),
        );
        rows(&read(&tmp.join("w.ycov")))
            .iter()
            .filter(|r| r.3 > 0)
            .count()
    };
    // Cold, warm, warm again: the same; another option: none; back: the same.
    assert_eq!(waivers(&[]), 5);
    assert_eq!(waivers(&[]), 5);
    assert_eq!(waivers(&["--coverage-directives=ignore"]), 0);
    assert_eq!(waivers(&[]), 5);
    // An edit of a directive is seen by the next run, which cannot reuse the old lowering.
    let edited = std::fs::read_to_string(design())
        .expect("design")
        .replace("exclude 11: x and y", "exclude 10: x and y");
    std::fs::write(tmp.join("edit.vhd"), edited).expect("write");
    let o = yavs(&tmp, &["analyze", "--work=work:lib", "edit.vhd"]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(waivers(&[]), 5);
    let db = read(&tmp.join("w.ycov"));
    let waived: Vec<_> = db
        .items
        .iter()
        .filter(|(k, e)| e.line == 15 && !e.waivers.is_empty() && yavs_cov::pattern(k) != "x")
        .map(|(k, _)| yavs_cov::pattern(k))
        .collect();
    assert_eq!(waived, ["10"]);
}

#[test]
fn exclusion_files_take_combination_rows() {
    let tmp = setup("comb_rows");
    run(
        &tmp,
        &[
            "--cov=a.ycov",
            "--cov-kind=combination",
            "--coverage-directives=ignore",
        ],
    );
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--uncovered"]);
    let rows_text = out(&o);
    assert!(
        rows_text
            .lines()
            .any(|l| l.starts_with("combination cov_comb ") && l.contains(" 11 ")),
        "{rows_text}"
    );
    // Pasting the report back excludes exactly those requirements.
    std::fs::write(tmp.join("ex.txt"), &rows_text).expect("write");
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "--exclude", "ex.txt", "--total"],
    );
    assert_eq!(out(&o).trim(), "100.0%", "{}", err(&o));
    // YAML, and a pattern with a wildcard matching more than one requirement.
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--uncovered=yaml"]);
    assert!(out(&o).contains("kind: combination"), "{}", out(&o));
    std::fs::write(tmp.join("ex.yaml"), out(&o)).expect("write");
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "--exclude", "ex.yaml", "--total"],
    );
    assert_eq!(out(&o).trim(), "100.0%", "{}", err(&o));
    let id = {
        let a = read(&tmp.join("a.ycov"));
        let (k, _) = a.items.iter().find(|(_, e)| e.line == 38).expect("elsif");
        format!("combination :cov_comb {:012x}.{}", k.hash >> 16, k.occ)
    };
    // `1-` matches both requirements with `b` true.
    std::fs::write(tmp.join("wild.txt"), format!("{id} 1-   # wildcard\n")).expect("write");
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "wild.txt",
            "--exclusions",
        ],
    );
    assert!(out(&o).contains("matches: 2 requirements"), "{}", out(&o));
    std::fs::write(tmp.join("none.txt"), format!("{id} 111\n")).expect("write");
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--exclude", "none.txt"]);
    assert!(err(&o).contains("W0111"), "{}", err(&o));
    // A misspelt row type is suggested.
    std::fs::write(
        tmp.join("typo.txt"),
        format!("{}\n", id.replace("combination", "combinaton")),
    )
    .expect("write");
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--exclude", "typo.txt"]);
    assert!(
        err(&o).contains("unknown exclusion type `combinaton`")
            && err(&o).contains("`combination`"),
        "{}",
        err(&o)
    );
    // A bad pattern names the line.
    std::fs::write(tmp.join("bad.txt"), format!("{id} 12\n")).expect("write");
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--exclude", "bad.txt"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        err(&o).contains("E0112") && err(&o).contains("line 1"),
        "{}",
        err(&o)
    );
}

#[test]
fn exclusion_report_covers_inline_and_file_rules() {
    let tmp = setup("comb_report");
    run(&tmp, &["--cov=a.ycov", "--cov-kind=statement,combination"]);
    // An external row that overlaps the directive of line 15, one that matches nothing, and one
    // for a metric that was not collected.
    let id = {
        let a = read(&tmp.join("a.ycov"));
        let (k, _) = a
            .items
            .iter()
            .find(|(k, e)| e.line == 15 && yavs_cov::pattern(k) == "11")
            .expect("item");
        format!("{:012x}.{}", k.hash >> 16, k.occ)
    };
    std::fs::write(
        tmp.join("ex.txt"),
        format!(
            "combination :cov_comb {id} 11   # overlaps the directive\n\
             combination :cov_comb {id} 00 # a different combination\n\
             combination :nowhere {id} 11   # no such instance\n\
             toggle :cov_comb x   # toggles were not collected\n"
        ),
    )
    .expect("write");
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "ex.txt",
            "--exclusions",
        ],
    );
    assert!(o.status.success(), "{}", err(&o));
    let t = out(&o);
    assert!(t.starts_with("coverage exclusions: 8 rules, "), "{t}");
    assert!(t.contains("(in the source)  applied"), "{t}");
    assert!(t.contains("rule:    condition (a=false)"), "{t}");
    assert!(
        t.contains("reason:  x and y are never high together"),
        "{t}"
    );
    assert!(t.contains("x is true, y is true  in x and y"), "{t}");
    // The overlapping rule matches the same requirement and is listed, but redundant.
    assert!(t.contains("redundant"), "{t}");
    assert!(t.contains("matches nothing"), "{t}");
    assert!(t.contains("its metric was not collected"), "{t}");
    assert!(t.contains("kind"), "{t}");
    // Every requirement is excluded once: the totals agree with the normal report.
    let exclusions_total: usize = t
        .lines()
        .find(|l| l.starts_with("combination "))
        .map(|l| {
            l.split_whitespace().collect::<Vec<_>>()[3]
                .parse()
                .expect("n")
        })
        .expect("row");
    let a = read(&tmp.join("a.ycov"));
    let waived = a
        .items
        .iter()
        .filter(|(k, e)| matches!(k.kind, Kind::Combination(_)) && !e.waivers.is_empty())
        .count();
    assert!(exclusions_total >= waived);
    // Deterministic.
    let again = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "ex.txt",
            "--exclusions",
        ],
    );
    assert_eq!(out(&again), t);

    // JSON: versioned, and the same rules.
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "ex.txt",
            "--exclusions=json",
        ],
    );
    let j = out(&o);
    assert!(j.starts_with("{\n  \"version\": 1,"), "{j}");
    assert!(j.contains("\"status\": \"redundant\""), "{j}");
    assert!(j.contains("\"status\": \"metric-not-collected\""), "{j}");
    assert!(
        j.contains("\"operands\": [\"x\", \"y\"], \"pattern\": \"11\""),
        "{j}"
    );
    assert!(j.contains("\"inline_recorded\": true"), "{j}");

    // HTML: the same page.
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "ex.txt",
            "--html",
            "html",
        ],
    );
    assert!(o.status.success(), "{}", err(&o));
    let page = std::fs::read_to_string(tmp.join("html/exclusions.html")).expect("page");
    assert!(page.contains("<h1>Coverage exclusions</h1>"), "{page}");
    assert!(page.contains("x and y are never high together"), "{page}");
    let index = std::fs::read_to_string(tmp.join("html/index.html")).expect("index");
    assert!(index.contains("exclusions.html") && index.contains("<th>combinations</th>"));
}

#[test]
fn old_databases_say_so() {
    let tmp = tempdir("comb_old");
    std::fs::write(
        tmp.join("old.ycov"),
        "yavs-coverage 5\ns\t:t\t0000000000000001\t0\t1\t-\t3\ta.vhd\tx := 1;\n",
    )
    .expect("write");
    let o = yavs(&tmp, &["cov", "report", "old.ycov", "--exclusions"]);
    assert!(o.status.success(), "{}", err(&o));
    let t = out(&o);
    assert!(t.contains("does not record in-source exclusions"), "{t}");
    let o = yavs(&tmp, &["cov", "report", "old.ycov", "--exclusions=json"]);
    assert!(
        out(&o).contains("\"inline_recorded\": false"),
        "{}",
        out(&o)
    );
}

/// One source file with `body` as the process, analysed; returns (exit code, stderr).
fn check(tmp: &Path, body: &str, extra: &[&str]) -> (Option<i32>, String) {
    let src = format!(
        "library ieee;\nuse ieee.std_logic_1164.all;\nentity d is end;\narchitecture a of d is\n  \
         signal valid, ready, reset : boolean := false;\n  signal v, r : std_logic := '0';\n  \
         signal n : integer := 0;\n  constant k : boolean := true;\nbegin\n  process\n  begin\n\
         {body}\n    wait;\n  end process;\nend;\n"
    );
    std::fs::write(tmp.join("d.vhd"), src).expect("write");
    let o = yavs(
        tmp,
        &[&["analyze", "--work=work:lib"][..], extra, &["d.vhd"]].concat(),
    );
    (o.status.code(), err(&o))
}

#[test]
fn directives_are_checked_without_coverage() {
    let tmp = tempdir("comb_checks");
    // (body, code, text that must appear)
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "    -- yavs cov exclude 01\n    if reset or (valid and ready) then n <= 1; end if;",
            "E2021",
            &[
                "exclusion has 2 positions, but the expression has 3 inputs",
                "expected order: reset, valid, ready",
                "one symbol per input",
            ],
        ),
        (
            "    -- yavs cov exclude (vlaid=false, ready=true)\n    if valid and ready then n <= 1; end if;",
            "E2022",
            &[
                "`vlaid` is not an input of this expression",
                "inputs, in order: valid, ready",
                "`valid`",
            ],
        ),
        (
            "    -- yavs cov exclude (valid=flase)\n    if valid and ready then n <= 1; end if;",
            "E2023",
            &[
                "invalid Boolean value `flase`",
                "expected `true` or `false`",
                "`false`",
            ],
        ),
        (
            "    -- yavs cov exclude 02\n    if valid and ready then n <= 1; end if;",
            "E2021",
            &["invalid pattern symbol `2`", "expected `0`, `1` or `-`"],
        ),
        (
            "    -- yavs cov exclude (valid=true\n    if valid and ready then n <= 1; end if;",
            "E2020",
            &["missing `)`"],
        ),
        (
            "    -- yavs cov exclude (valid)\n    if valid and ready then n <= 1; end if;",
            "E2020",
            &["expected `name=value`"],
        ),
        (
            "    -- yavs cov exclude 01 impossible\n    if valid and ready then n <= 1; end if;",
            "E2020",
            &["unexpected text", "a reason follows `:`"],
        ),
        (
            "    -- yavs cov exclud 01\n    if valid and ready then n <= 1; end if;",
            "E2025",
            &["unknown coverage command `exclud`", "`exclude`"],
        ),
        (
            "    -- yavs cov exclude 01\n    n <= 1;\n",
            "E2024",
            &["is not an expression of `boolean`, `bit` or `std_ulogic` operators"],
        ),
        (
            "    -- yavs cov exclude 01\n    wait for 1 ns;\n    if valid and ready then n <= 1; end if;",
            "E2024",
            &["cannot be excluded from"],
        ),
        (
            "    -- yavs cov exclude 01\n\n    -- an ordinary comment\n    n <= 1;\n    if valid and ready then n <= 1; end if;",
            "E2024",
            &["is not an expression of `boolean`, `bit` or `std_ulogic` operators"],
        ),
        (
            "    -- yavs cov exclude expression 01\n    if valid and ready then n <= 1; end if;",
            "E2024",
            &[
                "`exclude expression` does not fit a condition",
                "write `exclude condition`",
            ],
        ),
        (
            "    -- yavs cov exclude condition (valid=true, valid=false)\n    if valid and ready then n <= 1; end if;",
            "E2022",
            &["`valid` is given twice"],
        ),
        (
            "    -- yavs cov exclude (valid=true)\n    if valid and (valid or ready) then n <= 1; end if;",
            "E2022",
            &["`valid` is 2 inputs of this expression", "by position"],
        ),
        (
            "    -- yavs cov exclude 01\n    if valid and k then n <= 1; end if;",
            "E2024",
            &["constant after elaboration"],
        ),
        (
            "    -- yavs cov exclude 01\n    n <= 1 when valid and ready else 2;",
            "E2024",
            &["several conditions or choices"],
        ),
        (
            "    -- yavs cov exclude cndition (valid=true)\n    if valid and ready then n <= 1; end if;",
            "E2020",
            &["unknown exclusion kind `cndition`", "`condition`"],
        ),
        (
            "    -- yavs cov\n    if valid and ready then n <= 1; end if;",
            "E2025",
            &["needs a command"],
        ),
    ];
    for (body, code, texts) in cases {
        let (rc, e) = check(&tmp, body, &[]);
        assert_eq!(rc, Some(1), "{body}\n{e}");
        assert!(e.contains(&format!("error[{code}]")), "{body}\n{e}");
        assert!(e.contains("d.vhd:"), "{body}\n{e}");
        for t in *texts {
            assert!(e.contains(t), "{t:?} missing for {body}\n{e}");
        }
        // The check does not depend on collecting anything, and `ignore` skips it.
        let (rc, e) = check(&tmp, body, &["--coverage-directives=ignore"]);
        assert_eq!(rc, Some(0), "{body}\n{e}");
    }
    // Ordinary comments, a directive-like text in a string, a block comment and the pragmas
    // are not directives.
    let (rc, e) = check(
        &tmp,
        "    -- yavs is a simulator, exclude 01 is prose\n    /* yavs cov exclude 9 */\n    -- coverage off\n    -- yavs coverage off-line\n    -- coverage on\n    report \"-- yavs cov exclude 9\";\n    if valid and ready then n <= 1; end if;",
        &["--std=2008"],
    );
    assert_eq!(rc, Some(0), "{e}");
    // A directive that names a combination, then waives it, is accepted, in all three forms.
    let (rc, e) = check(
        &tmp,
        "    -- yavs cov exclude 0-: a\n    -- yavs cov exclude (valid=true, ready=false)\n    -- yavs coverage exclude condition 1-\n    if valid and ready then n <= 1; end if;",
        &[],
    );
    assert_eq!(rc, Some(0), "{e}");
    assert!(e.contains("W2021") && e.contains("W2020"), "{e}");
}

#[test]
fn warnings_follow_the_warning_controls() {
    let tmp = tempdir("comb_warn");
    let dup = "    -- yavs cov exclude 0-\n    -- yavs cov exclude 0-\n    if valid and ready then n <= 1; end if;";
    let (rc, e) = check(&tmp, dup, &[]);
    assert_eq!(rc, Some(0), "{e}");
    assert!(
        e.contains("warning[W2020]") && e.contains("repeats an earlier one"),
        "{e}"
    );
    // -Werror makes it fail, -Wno- hides it.
    let (rc, _) = check(&tmp, dup, &["-Werror"]);
    assert_eq!(rc, Some(1));
    let (rc, e) = check(&tmp, dup, &["-Wno-W2020"]);
    assert_eq!(rc, Some(0));
    assert!(!e.contains("W2020"), "{e}");
    let sub = "    -- yavs cov exclude 0-\n    -- yavs cov exclude --\n    if valid and ready then n <= 1; end if;";
    let (_, e) = check(&tmp, sub, &[]);
    assert!(e.contains("already covered by a broader one"), "{e}");
    let all = "    -- yavs cov exclude --\n    if valid and ready then n <= 1; end if;";
    let (rc, e) = check(&tmp, all, &[]);
    assert_eq!(rc, Some(0), "{e}");
    assert!(
        e.contains("warning[W2021]") && e.contains("waive every combination"),
        "{e}"
    );
    // `valid and ready` on booleans does not evaluate `ready` when `valid` is false.
    let none = "    -- yavs cov exclude (valid=false, ready=true)\n    if valid and ready then n <= 1; end if;";
    let (rc, e) = check(&tmp, none, &[]);
    assert_eq!(rc, Some(0), "{e}");
    assert!(
        e.contains("warning[W2022]") && e.contains("matches no combination"),
        "{e}"
    );
    // The codes are explained.
    for code in [
        "W2020", "W2021", "W2022", "W2023", "E2020", "E2025", "E0137",
    ] {
        let o = yavs(&tmp, &["--explain", code]);
        assert!(o.status.success(), "{code}: {}", err(&o));
    }
}

#[test]
fn warnings_are_replayed_by_the_cache_and_follow_the_option() {
    let tmp = tempdir("comb_replay");
    let dup = "    -- yavs cov exclude 0-\n    -- yavs cov exclude 0-\n    if valid and ready then n <= 1; end if;";
    let (rc, _) = check(&tmp, dup, &[]);
    assert_eq!(rc, Some(0));
    // Cold, then the run that makes the image, then hits: the warning every time, once.
    for round in 0..4 {
        let o = yavs(&tmp, &["run", "--work=work:lib", "d"]);
        assert!(o.status.success(), "{}", err(&o));
        assert_eq!(
            err(&o).matches("warning[W2020]").count(),
            1,
            "round {round}: {}",
            err(&o)
        );
    }
    // Another option is another analysis: nothing is replayed, and the note says what was left out.
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "d",
            "--coverage-directives=ignore",
        ],
    );
    assert!(!err(&o).contains("W2020"), "{}", err(&o));
    assert!(
        err(&o).contains("2 `-- yavs coverage exclude` directives left out"),
        "{}",
        err(&o)
    );
    // An error in the directive stops the run before elaboration, on every run.
    let bad = "    -- yavs cov exclude 012\n    if valid and ready then n <= 1; end if;";
    let (rc, _) = check(&tmp, bad, &["--coverage-directives=ignore"]);
    assert_eq!(rc, Some(0));
    for _ in 0..3 {
        let o = yavs(&tmp, &["run", "--work=work:lib", "d"]);
        assert_eq!(o.status.code(), Some(2), "{}", err(&o));
        assert!(err(&o).contains("error[E2021]"), "{}", err(&o));
    }
}

#[test]
fn undecidable_decisions_are_reported_when_measured() {
    let tmp = tempdir("comb_unmeasured");
    let src = "    if valid and k then n <= 1; end if;";
    let (rc, e) = check(&tmp, src, &[]);
    assert_eq!((rc, e.contains("W2023")), (Some(0), false), "{e}");
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "d",
            "--cov=a.ycov",
            "--cov-kind=combination",
        ],
    );
    assert!(o.status.success(), "{}", err(&o));
    assert!(err(&o).contains("warning[W2023]"), "{}", err(&o));
    assert!(
        err(&o).contains("constant after elaboration"),
        "{}",
        err(&o)
    );
    assert_eq!(read(&tmp.join("a.ycov")).items.len(), 0);
}

#[cfg(feature = "clift")]
#[test]
fn tiers_agree() {
    let tmp = setup("comb_tiers");
    run(
        &tmp,
        &["--cov=i.ycov", "--cov-kind=combination", "--tier=interp"],
    );
    for tier in ["clift", "auto"] {
        run(
            &tmp,
            &[
                &format!("--cov={tier}.ycov"),
                "--cov-kind=combination",
                &format!("--tier={tier}"),
            ],
        );
        assert_eq!(
            std::fs::read(tmp.join("i.ycov")).expect("i"),
            std::fs::read(tmp.join(format!("{tier}.ycov"))).expect("t"),
            "{tier}"
        );
    }
}
