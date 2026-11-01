//! The text output of `yavs cov report` (#877) on `tests/cli/cov_report.vhd`: relative paths,
//! named branch arms, aligned columns, `--kind`, `--total` and `--fail-under`.

mod common;
use common::{Tmp, tempdir};

use std::path::Path;
use std::process::{Command, Output};

fn yavs(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["--color", "never"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run")
}

fn text(o: &[u8]) -> String {
    String::from_utf8(o.to_vec()).expect("utf-8")
}

/// A directory holding the design and its database (statements and branches, 19/23 and 10/16).
fn database(name: &str) -> Tmp {
    let tmp = tempdir(name);
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cli/cov_report.vhd");
    std::fs::copy(&src, tmp.join("cov_report.vhd")).expect("copy");
    let lib = format!("--work=work:{}/lib", tmp.display());
    let o = yavs(&tmp, &["analyze", &lib, "cov_report.vhd"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = yavs(
        &tmp,
        &[
            "run",
            &lib,
            "--cov=a.ycov",
            "--cov-kind=statement,branch",
            "cov_report",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    tmp
}

#[test]
fn text_report_names_arms_and_aligns_columns() {
    let tmp = database("text");
    let o = yavs(&tmp, &["cov", "report", "a.ycov"]);
    assert!(o.status.success());
    assert_eq!(
        text(&o.stdout),
        "\
Kind       Items  Hit  Missed  Coverage
---------  -----  ---  ------  --------
statement     23   19       4     82.6%
branch        16   10       6     62.5%
total         39   29      10     74.3%

Instance    Total        statement    branch
----------  -----------  -----------  -----------
cov_report  29/39 74.3%  19/23 82.6%  10/16 62.5%

10 items not covered; `--detailed` shows them.
"
    );
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--detailed"]);
    assert!(o.status.success());
    let out = text(&o.stdout);
    for want in [
        "=== cov_report  (cov_report.vhd) ===",
        "  Line  Item ",
        "  26    n <= 0;",
        "arm: if rst = '1'",
        "arm: elsif n > 100",
        "arm: (no arm taken)",
        "covered",
        "MISSED\n",
        "exclude with this row in an `--exclude` file: statement cov_report ",
        "cov_report: 10 of 39 items not covered (6 branch, 4 statement)\n",
    ] {
        assert!(out.contains(want), "{want}: {out}");
    }
}

/// `--detailed` entries: where, what is missing and a tip, in plain words, the wording pinned.
#[test]
fn detailed_entries_say_what_is_missing() {
    let tmp = database("entries");
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--detailed"]);
    let out = text(&o.stdout);
    for want in [
        // An arm: the source underlined, the other arms, and the statement it explains.
        "note: branch arm never taken: `if rst = '1'`\n  --> cov_report.vhd:25:8\n",
        "25 |     if rst = '1' then\n   |        ^^^^^^^^^ never taken\n",
        "= note: the other arms: `elsif en = '1' and n < 15` ran",
        "= note: this explains 1 item more, which cannot run until it does:\n",
        "line 26: statement `n <= 0;`",
        "= help: `rst = '1'` was never true when this ran: make it true",
        "= help: `n > 100` was never true with `rst = '1'`, `en = '1' and n < 15` false: make it so",
        // An arm with no condition of its own.
        "= help: every run took one of the other arms: drive a value they do not match",
    ] {
        assert!(out.contains(want), "{want}: {out}");
    }
    // The statements under a missed arm have no entry of their own.
    assert!(!out.contains("statement never ran: `n <= 0;`"), "{out}");
}

/// A file with no source at hand: said so, the rest still printed.
#[test]
fn detailed_says_when_the_source_is_gone() {
    let tmp = database("gone");
    std::fs::remove_file(tmp.join("cov_report.vhd")).expect("remove");
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--detailed"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let out = text(&o.stdout);
    assert!(
        out.contains("at cov_report.vhd:25:8; source not shown: cannot read it:"),
        "{out}"
    );
    assert!(out.contains("--cov-embed-sources"), "{out}");
    assert!(
        out.contains("branch arm never taken: `if rst = '1'`"),
        "{out}"
    );
    // An edited file is not underlined at the wrong place.
    std::fs::write(tmp.join("cov_report.vhd"), "-- edited\n").expect("write");
    let out = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--detailed"]).stdout);
    assert!(
        out.contains("source not shown: the file changed since the run"),
        "{out}"
    );
}

#[test]
fn arm_names_are_the_comments_of_the_uncovered_rows() {
    let tmp = database("rows");
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--uncovered"]);
    let out = text(&o.stdout);
    for arm in [
        "arm 0: if rst = '1'",
        "arm 2: elsif n > 100",
        "arm 1: else",
        "arm 1: (no arm taken)",
        "arm 2: when others",
    ] {
        // A labelled process follows in the comment.
        let row = out.lines().find(|l| l.contains(&format!("# {arm}")));
        let rest = row.and_then(|l| l.split_once(arm)).map(|x| x.1);
        assert!(
            rest.is_some_and(|r| r.is_empty() || r.starts_with("  [process ")),
            "{arm}\n{out}"
        );
    }
    // Rows carry the same relative path as the report.
    assert!(out.contains(" cov_report.vhd:25   # "), "{out}");
}

#[test]
fn root_changes_the_prefix_stripped() {
    let tmp = database("root");
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--detailed",
            "--root",
            "elsewhere",
        ],
    );
    let out = text(&o.stdout);
    // The source is outside the root: its full path stays.
    assert!(
        out.contains("/cov_report.vhd) ===") || out.contains("\\cov_report.vhd) ==="),
        "{out}"
    );
    assert!(!out.contains("(cov_report.vhd) ==="), "{out}");
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "--detailed", "--root", "."],
    );
    assert!(
        text(&o.stdout).contains("=== cov_report  (cov_report.vhd) ==="),
        "relative"
    );
}

#[test]
fn total_and_kind() {
    let tmp = database("total");
    let total = |args: &[&str]| {
        let o = yavs(&tmp, &[&["cov", "report", "a.ycov"][..], args].concat());
        (o.status.code(), text(&o.stdout), text(&o.stderr))
    };
    // 29 of 39 items, rounded down: a gate never rounds a missing item up.
    assert_eq!(total(&["--total"]), (Some(0), "74.3%\n".into(), "".into()));
    assert_eq!(
        total(&["--total", "--kind=statement"]),
        (Some(0), "82.6%\n".into(), "".into())
    );
    assert_eq!(total(&["--total", "--kind=branch,statement"]).1, "74.3%\n");
    // The normal report with a kind filter has only that kind.
    let o = total(&["--kind=branch"]);
    assert!(
        o.1.contains(
            "\nbranch     16   10       6     62.5%\ntotal      16   10       6     62.5%\n\nInstance"
        ),
        "{}",
        o.1
    );
    assert!(!o.1.contains("statement  "), "{}", o.1);
    // A kind the database lacks has nothing to total: E0115, nothing on stdout.
    let (code, out, err) = total(&["--total", "--kind=toggle"]);
    assert_eq!((code, out.as_str()), (Some(1), ""));
    assert!(err.contains("error[E0115]"), "{err}");
    // A total is not a listing.
    assert_eq!(total(&["--total", "--uncovered"]).0, Some(2));
}

#[test]
fn total_counts_after_exclusions() {
    let tmp = database("excl");
    // Excluding a statement that never ran: 19 of 22.
    let rows = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--uncovered"]).stdout);
    let row = rows
        .lines()
        .find(|l| l.starts_with("statement ") && l.contains(":26"))
        .expect("row");
    std::fs::write(tmp.join("ex.txt"), format!("{row}\n")).expect("write");
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "ex.txt",
            "--total",
            "--kind=statement",
        ],
    );
    assert_eq!(text(&o.stdout), "86.3%\n", "{}", text(&o.stderr));
}

#[test]
fn fail_under() {
    let tmp = database("gate");
    let gate = |pct: &str, more: &[&str]| {
        let o = yavs(
            &tmp,
            &[
                &["cov", "report", "a.ycov", "--kind=statement"][..],
                more,
                &[&format!("--fail-under={pct}")],
            ]
            .concat(),
        );
        (o.status.code(), text(&o.stdout), text(&o.stderr))
    };
    // Statements are at 82.6%: at the threshold passes, just above fails.
    let (code, out, err) = gate("82.6", &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("\nstatement     23   19       4     82.6%\n"),
        "{out}"
    );
    assert!(err.is_empty(), "{err}");
    assert_eq!(gate("82.5", &[]).0, Some(0));
    assert_eq!(gate("0", &[]).0, Some(0));
    // Below: the report is printed as usual and the message follows on stderr.
    let (code, out, err) = gate("82.7", &[]);
    assert_eq!(code, Some(1));
    assert!(
        out.contains("\nstatement     23   19       4     82.6%\n"),
        "{out}"
    );
    assert!(
        err.contains("error[E0114]: coverage 82.6% is below the required 82.7%"),
        "{err}"
    );
    // 99.95 would round to 100.0: a gate at 100 still fails with an item missing.
    assert_eq!(gate("100", &[]).0, Some(1));
    // With --total the percentage is printed too.
    let (code, out, err) = gate("90", &["--total"]);
    assert_eq!((code, out.as_str()), (Some(1), "82.6%\n"));
    assert!(err.contains("below the required 90.0%"), "{err}");
    // Without a kind filter it checks everything: 74.3%.
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "--total", "--fail-under=74.3"],
    );
    assert_eq!(o.status.code(), Some(0));
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "--total", "--fail-under=74.4"],
    );
    assert_eq!(o.status.code(), Some(1));
    // Not a percentage: a usage error, exit 2, nothing printed.
    for bad in ["abc", "-1", "100.5", "nan", ""] {
        let (code, out, err) = gate(bad, &[]);
        assert_eq!((code, out.as_str()), (Some(2), ""), "{bad}");
        assert!(err.contains("--fail-under"), "{bad}: {err}");
    }
    // An empty selection cannot be checked.
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--kind=toggle",
            "--fail-under=50",
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o.stderr).contains("E0115"), "{}", text(&o.stderr));
}

#[test]
fn fail_under_per_kind() {
    let tmp = database("fail_under_kinds");
    let run = |arg: &str| {
        let o = yavs(&tmp, &["cov", "report", "a.ycov", "--total", arg]);
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let pct = |kind: &str| {
        let o = yavs(
            &tmp,
            &["cov", "report", "a.ycov", "--total", "--kind", kind],
        );
        let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
        t.trim_end_matches('%')
            .parse::<f64>()
            .expect("a percentage")
    };
    let (statement, branch) = (pct("statement"), pct("branch"));
    assert!(statement < 100.0 && branch < 100.0);

    // Exactly at each kind's own percentage passes; a tenth more fails and names only that kind.
    let (code, err) = run(&format!(
        "--fail-under=statement:{statement},branch:{branch}"
    ));
    assert_eq!(code, Some(0), "{err}");
    let (code, err) = run(&format!(
        "--fail-under=statement:{statement},branch:{:.1}",
        branch + 0.1
    ));
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("E0114") && err.contains("branch coverage"),
        "{err}"
    );
    assert!(!err.contains("statement coverage"), "{err}");

    // Several kinds fail: both named. A plain total can stand beside them.
    let (code, err) = run("--fail-under=statement:100,branch:100");
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("statement coverage") && err.contains("branch coverage"),
        "{err}"
    );
    let (code, err) = run("--fail-under=100,branch:0");
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("coverage") && !err.contains("branch coverage"),
        "{err}"
    );

    // A kind the database lacks is E0115, naming it, not 0% or 100%.
    let (code, err) = run("--fail-under=statement:0,combination:10");
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("E0115") && err.contains("`combination`"),
        "{err}"
    );

    // Usage errors: a near kind gets a suggestion; range, repeats.
    for (bad, want) in [
        ("--fail-under=statment:90", "`statement`"),
        ("--fail-under=nonsense:90", "the kinds are"),
        ("--fail-under=branch:101", "outside 0 to 100"),
        ("--fail-under=branch:x", "not a number"),
        ("--fail-under=branch:1,branch:2", "given twice"),
        ("--fail-under=80,90", "given twice"),
    ] {
        let (code, err) = run(bad);
        assert_eq!(code, Some(2), "{bad}: {err}");
        assert!(err.contains(want), "{bad}: {err}");
    }
}

#[test]
fn cobertura_follows_root_and_counts_lines_without_a_statement() {
    let tmp = database("cobertura");
    let xml = |args: &[&str]| {
        let o = yavs(
            &tmp,
            &[
                &["cov", "report", "a.ycov", "--cobertura", "c.xml"][..],
                args,
            ]
            .concat(),
        );
        assert!(o.status.success(), "{}", text(&o.stderr));
        std::fs::read_to_string(tmp.join("c.xml")).expect("xml")
    };
    // #1023: branches ran on this line; the statement is only filtered out.
    let x = xml(&["--kind=branch"]);
    assert!(
        x.contains("<line number=\"25\" hits=\"2\" branch=\"true\""),
        "{x}"
    );
    assert!(!x.contains("line-rate=\"0\""), "{x}");
    // A statement that never ran still reads 0.
    let x = xml(&[]);
    assert!(
        x.contains("<line number=\"26\" hits=\"0\" branch=\"false\"/>"),
        "{x}"
    );
    // Relative to the current directory by default, to `--root` when given, else in full.
    assert!(x.contains("filename=\"cov_report.vhd\""), "{x}");
    std::fs::create_dir_all(tmp.join("sub")).expect("dir");
    let x = xml(&["--root", "sub"]);
    assert!(!x.contains("filename=\"cov_report.vhd\""), "{x}");
}

#[test]
fn below_and_detailed_limit_the_display_only() {
    let tmp = database("below");
    let run = |args: &[&str]| {
        let o = yavs(&tmp, &[&["cov", "report", "a.ycov"][..], args].concat());
        (o.status.code(), text(&o.stdout), text(&o.stderr))
    };
    let plain = run(&[]).1;
    let totals = &plain[..plain.find("\n\n").unwrap()];
    // One instance at 74.3%, statements and branches.
    let (code, out, _) = run(&["--below=80"]);
    assert_eq!(code, Some(0));
    assert!(out.starts_with(totals), "{out}");
    assert!(
        out.contains("cov_report  29/39 74.3%  19/23 82.6%  10/16 62.5%"),
        "{out}"
    );
    let (_, out, _) = run(&["--below=74.3"]);
    assert!(out.starts_with(totals), "{out}");
    assert!(
        out.contains("no instance or package is below 74.3%"),
        "{out}"
    );
    // `--kind` counts: statements alone are over 80, branches alone under.
    assert!(
        run(&["--below=80", "--kind=statement"])
            .1
            .contains("is below 80.0%")
    );
    assert!(
        run(&["--below=80", "--kind=branch"])
            .1
            .contains("10/16 62.5%  10/16 62.5%")
    );
    // What is left to do: the rows, after the limit. At the limit there is nothing to list.
    let (_, out, _) = run(&["--below=100", "--detailed"]);
    assert!(
        out.contains("cov_report: 10 of 39 items not covered"),
        "{out}"
    );
    assert!(!out.contains("`--detailed`"), "{out}");
    let (_, out, _) = run(&["--below=74.3", "--detailed"]);
    assert!(!out.contains("=== cov_report"), "{out}");
    // An exclusion counts: the missed statements out, the instance rises; `--detailed` lists them.
    let (_, ex, _) = run(&["--uncovered", "--kind=statement"]);
    std::fs::write(tmp.join("ex.txt"), ex).expect("write");
    let (_, out, _) = run(&["--below=100", "--exclude", "ex.txt", "--kind=statement"]);
    assert!(out.contains("is below 100.0%"), "{out}");
    assert!(
        out.contains("4 items excluded, not in the totals; `--detailed` shows them"),
        "{out}"
    );
    let (_, out, _) = run(&["--detailed", "--exclude", "ex.txt", "--kind=statement"]);
    assert!(out.contains("excluded"), "{out}");
    assert!(out.contains("v := 3;"), "{out}");
    // Neither moves a gate; a bad value or a clash is a usage error.
    assert_eq!(
        run(&["--below=100", "--detailed", "--fail-under=70"]).0,
        Some(0)
    );
    assert_eq!(
        run(&["--below=0", "--detailed", "--fail-under=80"]).0,
        Some(1)
    );
    assert_eq!(run(&["--below=101"]).0, Some(2));
    for (a, b) in [
        ("--below=80", "--total"),
        ("--detailed", "--total"),
        ("--detailed", "--uncovered"),
        ("--detailed", "--tests"),
        ("--detailed", "--exclusions"),
    ] {
        assert_eq!(run(&[a, b]).0, Some(2), "{a} {b}");
    }
}

#[test]
fn embedded_sources_need_no_checkout() {
    let tmp = database("embed");
    let lib = format!("--work=work:{}/lib", tmp.display());
    let run = |db: &str, extra: &[&str]| {
        let cov = format!("--cov={db}");
        let args = [
            &["run", lib.as_str(), "cov_report", cov.as_str()][..],
            extra,
        ]
        .concat();
        let o = yavs(&tmp, &args);
        assert!(o.status.success(), "{}", text(&o.stderr));
    };
    run("plain.ycov", &[]);
    run("embed.ycov", &["--cov-embed-sources"]);
    let db = std::fs::read_to_string(tmp.join("embed.ycov")).expect("db");
    assert!(db.contains("\nsource\t") && db.contains("entity cov_report is"));
    assert!(
        !std::fs::read_to_string(tmp.join("plain.ycov"))
            .expect("db")
            .contains("\nsource\t")
    );
    let original = std::fs::read_to_string(tmp.join("cov_report.vhd")).expect("source");
    std::fs::remove_file(tmp.join("cov_report.vhd")).expect("remove");

    // Without the option E0113 stays, naming the way out; nothing written.
    let o = yavs(&tmp, &["cov", "report", "plain.ycov", "--html", "h1"]);
    assert_eq!(o.status.code(), Some(1));
    let err = text(&o.stderr);
    assert!(
        err.contains("E0113") && err.contains("--cov-embed-sources"),
        "{err}"
    );
    assert!(!tmp.join("h1").exists());
    // With it, and through a merge, the report needs no source file.
    let o = yavs(&tmp, &["cov", "report", "embed.ycov", "--html", "h2"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = yavs(
        &tmp,
        &["cov", "merge", "embed.ycov", "plain.ycov", "-o", "m.ycov"],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = yavs(&tmp, &["cov", "report", "m.ycov", "--html", "h3"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    for dir in ["h2", "h3"] {
        let page = std::fs::read_to_string(tmp.join(dir).join("src-0.html")).expect("page");
        assert!(page.contains("signal en, rst"), "{dir}: {page}");
    }

    // A changed text between merged runs warns; the databases still merge.
    std::fs::write(tmp.join("cov_report.vhd"), format!("-- edited\n{original}")).expect("write");
    let o = yavs(&tmp, &["analyze", &lib, "cov_report.vhd"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    run("edited.ycov", &["--cov-embed-sources"]);
    let o = yavs(
        &tmp,
        &["cov", "merge", "embed.ycov", "edited.ycov", "-o", "n.ycov"],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(
        text(&o.stderr).contains("warning[W0121]"),
        "{}",
        text(&o.stderr)
    );

    // The option needs `--cov`.
    let o = yavs(
        &tmp,
        &["run", lib.as_str(), "cov_report", "--cov-embed-sources"],
    );
    assert_eq!(o.status.code(), Some(2));
}

/// The text printed beside a missed row works when pasted: that row and nothing else goes.
#[test]
fn printed_exclusions_work_when_pasted() {
    let tmp = tempdir("paste");
    let src = "entity t is end entity;\narchitecture a of t is\n  signal x, y : boolean := false;\n  signal s : bit;\nbegin\n  process begin\n    if x and y then\n      null;\n    end if;\n    wait;\n  end process;\nend architecture;\n";
    std::fs::write(tmp.join("t.vhd"), src).expect("write");
    let lib = format!("--work=work:{}/lib", tmp.display());
    let run = |source: &str| {
        std::fs::write(tmp.join("t.vhd"), source).expect("write");
        assert!(yavs(&tmp, &["analyze", &lib, "t.vhd"]).status.success());
        let args = [
            "run",
            lib.as_str(),
            "t",
            "--cov=a.ycov",
            "--cov-kind=statement,branch,toggle,condition,combination",
        ];
        let o = yavs(&tmp, &args);
        assert!(o.status.success(), "{}", text(&o.stderr));
    };
    let totals = |extra: &[&str]| {
        let args = [&["cov", "report", "a.ycov"][..], extra].concat();
        let t = text(&yavs(&tmp, &args).stdout);
        let line = |k: &str| {
            t.lines().find(|l| l.starts_with(k)).map_or(0, |l| {
                l.split_whitespace()
                    .nth(1)
                    .unwrap_or("0")
                    .parse::<u32>()
                    .unwrap_or(0)
            })
        };
        ["statement", "branch", "toggle", "condition", "combination"].map(line)
    };
    run(src);
    let detailed = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--detailed"]).stdout);
    let before = totals(&[]);

    // A combination or a condition row: the comment, before the decision. Each takes one item
    // of its kind out and nothing else.
    let comments: Vec<&str> = detailed
        .lines()
        .filter_map(|l| l.split("exclude with: ").nth(1))
        .map(|c| c.strip_suffix(')').unwrap_or(c))
        .collect();
    assert!(
        comments.iter().any(|c| c.contains("exclude row ")),
        "{detailed}"
    );
    assert!(
        comments.iter().any(|c| !c.contains("exclude row ")),
        "{detailed}"
    );
    for comment in comments {
        assert!(comment.starts_with("-- yavs cov exclude "), "{comment}");
        let edited = src.replace(
            "    if x and y then",
            &format!("    {comment}\n    if x and y then"),
        );
        run(&edited);
        let mut want = before;
        want[if comment.contains("exclude row ") {
            3
        } else {
            4
        }] -= 1;
        assert_eq!(totals(&[]), want, "{comment}");
    }
    run(src);

    // Every other kind: its file row takes exactly one item out, or one signal's direction.
    let rows: Vec<&str> = detailed
        .lines()
        .filter_map(|l| l.split("`--exclude` file: ").nth(1))
        .collect();
    assert!(
        rows.iter().any(|r| r.starts_with("statement ")),
        "{detailed}"
    );
    assert!(rows.iter().any(|r| r.starts_with("branch ")), "{detailed}");
    assert!(rows.iter().any(|r| r.starts_with("toggle ")), "{detailed}");
    for row in rows {
        std::fs::write(tmp.join("ex.txt"), format!("{row}\n")).expect("write");
        let at = ["statement", "branch", "toggle", "condition", "combination"]
            .iter()
            .position(|k| row.starts_with(k))
            .expect("a kind");
        let mut want = before;
        want[at] -= 1;
        assert_eq!(totals(&["--exclude", "ex.txt"]), want, "{row}");
    }
}

#[test]
fn exports_follow_exclusions_kinds_and_root() {
    let tmp = database("exports");
    let (json, lcov, sonar) = ("c.json", "c.info", "s.xml");
    let run = |extra: &[&str]| {
        let base = [
            "cov",
            "report",
            "a.ycov",
            "--json",
            json,
            "--lcov",
            lcov,
            "--sonarqube",
            sonar,
        ];
        let o = yavs(&tmp, &[&base[..], extra].concat());
        assert!(o.status.success(), "{}", text(&o.stderr));
        let read = |n| std::fs::read_to_string(tmp.join(n)).expect("export");
        let j: serde_json::Value = serde_json::from_str(&read(json)).expect("JSON");
        (j, read(lcov), read(sonar))
    };
    let (j, l, s) = run(&[]);
    assert_eq!(j["version"], 1);
    assert_eq!(j["tests"][0], "cov_report");
    let items = j["items"].as_array().expect("items");
    assert_eq!(items.len(), 39);
    let total = j["totals"]
        .as_array()
        .expect("totals")
        .last()
        .cloned()
        .expect("a row");
    assert_eq!(
        (&total["kind"], &total["hit"], &total["coverage"]),
        (&"total".into(), &29.into(), &74.3.into())
    );
    assert!(
        items
            .iter()
            .any(|i| i["arm_text"] == "if rst = '1'" && i["hits"] == 0)
    );
    assert!(l.contains("SF:cov_report.vhd\n"), "{l}");
    assert!(l.contains("LF:23\n") || l.contains("LF:"), "{l}");
    assert!(s.contains("<file path=\"cov_report.vhd\">"), "{s}");

    // An exclusion: left out of each, as in Cobertura.
    let (_, ex, _) = {
        let o = yavs(
            &tmp,
            &["cov", "report", "a.ycov", "--uncovered", "--kind=statement"],
        );
        (0, text(&o.stdout), 0)
    };
    std::fs::write(tmp.join("ex.txt"), ex).expect("write");
    let (j, l, s) = run(&["--exclude", "ex.txt"]);
    assert_eq!(j["items"].as_array().expect("items").len(), 35);
    assert!(!l.contains("DA:26,"), "{l}");
    assert!(!s.contains("lineNumber=\"26\" covered=\"false\"/>"), "{s}");
    // `--kind`.
    let (j, l, _) = run(&["--kind=branch"]);
    assert!(
        j["items"]
            .as_array()
            .expect("items")
            .iter()
            .all(|i| i["kind"] == "branch")
    );
    assert!(!l.contains("\nDA:"), "{l}");
    // `--root`: a file outside it keeps its full path.
    std::fs::create_dir_all(tmp.join("sub")).expect("dir");
    let (j, l, s) = run(&["--root", "sub"]);
    assert!(
        j["items"][0]["file"]
            .as_str()
            .is_some_and(|f| f.ends_with("/cov_report.vhd") || f.ends_with("\\cov_report.vhd"))
    );
    assert!(!l.contains("SF:cov_report.vhd\n"), "{l}");
    assert!(!s.contains("<file path=\"cov_report.vhd\">"), "{s}");
    // A file that cannot be written is the usual error.
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "--json", "no/such/dir/c.json"],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o.stderr).contains("E0110"), "{}", text(&o.stderr));
}

#[test]
fn a_database_of_another_format_version_says_so() {
    let tmp = tempdir("db_version");
    std::fs::write(tmp.join("new.ycov"), "yavs-coverage 999\nfuture\tline\n").expect("write");
    std::fs::write(tmp.join("other.ycov"), "yavs-coverage soon\n").expect("write");
    for args in [
        &["cov", "report", "new.ycov"][..],
        &["cov", "merge", "new.ycov", "-o", "m.ycov"],
        &["cov", "rank", "new.ycov"],
    ] {
        let o = yavs(&tmp, args);
        let err = text(&o.stderr);
        assert_eq!(o.status.code(), Some(1), "{args:?}: {err}");
        assert!(
            err.contains("E0111") && err.contains("`new.ycov`"),
            "{args:?}: {err}"
        );
        assert!(err.contains("format version 999"), "{args:?}: {err}");
        assert!(err.contains("reads versions 2 to "), "{args:?}: {err}");
        assert!(err.contains("re-run the simulation"), "{args:?}: {err}");
        assert!(
            !err.contains("not a yavs coverage database"),
            "{args:?}: {err}"
        );
    }
    assert!(!tmp.join("m.ycov").exists());
    // No version to name: the file is not a database at all.
    let o = yavs(&tmp, &["cov", "report", "other.ycov"]);
    assert!(text(&o.stderr).contains("not a yavs coverage database"));
}

#[test]
fn fail_under_in_json_messages() {
    let tmp = database("json");
    let o = yavs(
        &tmp,
        &[
            "--message-format=json",
            "cov",
            "report",
            "a.ycov",
            "--total",
            "--fail-under=99",
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    let err = text(&o.stderr);
    assert!(
        err.contains("\"E0114\"") && err.contains("below the required 99.0%"),
        "{err}"
    );
}
