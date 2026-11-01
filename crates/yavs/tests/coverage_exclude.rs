//! `yavs cov report` and `yavs cov rank` exclusions: several files, their forms and origins.

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

fn write(dir: &Path, name: &str, body: &str) {
    if let Some(p) = dir.join(name).parent() {
        std::fs::create_dir_all(p).expect("mkdir");
    }
    std::fs::write(dir.join(name), body).expect("write");
}

/// A directory holding the design and its database `a.ycov`.
fn database(name: &str) -> Tmp {
    let tmp = tempdir(name);
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cli/cov_report.vhd");
    std::fs::copy(&src, tmp.join("cov_report.vhd")).expect("copy");
    let o = yavs(&tmp, &["analyze", "--work=work:lib", "cov_report.vhd"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "--cov-file=a.ycov",
            "--cov-kind=statement,branch",
            "cov_report",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    tmp
}

/// The `--uncovered` rows of the database.
fn missed(tmp: &Path) -> Vec<String> {
    let o = yavs(tmp, &["cov", "report", "a.ycov", "--uncovered"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    text(&o.stdout).lines().map(str::to_string).collect()
}

#[test]
fn exclusion_files_add_up_and_the_report_names_each_one() {
    let tmp = database("repeat");
    let rows = missed(&tmp);
    assert!(rows.len() >= 3, "{rows:?}");
    write(&tmp, "one.txt", &format!("{}\n", rows[0]));
    write(&tmp, "two.txt", &format!("{}\n{}\n", rows[1], rows[0]));
    let args = |more: &[&'static str]| -> Vec<&str> {
        let mut v = vec!["cov", "report", "a.ycov"];
        v.extend(["--exclude", "one.txt", "--exclude", "two.txt"]);
        v.extend(more);
        v
    };
    let left = text(&yavs(&tmp, &args(&["--uncovered"])).stdout);
    assert_eq!(left.lines().count(), rows.len() - 2, "{left}");
    let r = yavs(&tmp, &args(&["--exclusions"]));
    let r = text(&r.stdout);
    assert!(r.contains("one.txt:1  (exclusion file)  applied"), "{r}");
    assert!(r.contains("two.txt:1  (exclusion file)  applied"), "{r}");
    // The row both files hold is the earlier file's; the later one is redundant.
    assert!(r.contains("two.txt:2  (exclusion file)  redundant"), "{r}");
    let j = text(&yavs(&tmp, &args(&["--exclusions=json"])).stdout);
    assert!(j.contains("\"origin\": \"two.txt:2\""), "{j}");
    let rank = yavs(
        &tmp,
        &[
            "cov",
            "rank",
            "a.ycov",
            "--exclude",
            "one.txt",
            "--exclude",
            "two.txt",
        ],
    );
    assert!(rank.status.success(), "{}", text(&rank.stderr));
}

#[test]
fn a_bad_second_file_is_named() {
    let tmp = database("badsecond");
    write(&tmp, "one.txt", "");
    for rank in [false, true] {
        let cmd = if rank { "rank" } else { "report" };
        let o = yavs(
            &tmp,
            &[
                "cov",
                cmd,
                "a.ycov",
                "--exclude",
                "one.txt",
                "--exclude",
                "gone.txt",
            ],
        );
        assert_eq!(o.status.code(), Some(1));
        let e = text(&o.stderr);
        assert!(
            e.contains("error[E0112]: cannot use exclusion file `gone.txt`"),
            "{e}"
        );
    }
    write(&tmp, "bad.txt", "frobnicate x\n");
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "one.txt",
            "--exclude",
            "bad.txt",
        ],
    );
    let e = text(&o.stderr);
    assert!(e.contains("`bad.txt`") && e.contains("line 1:"), "{e}");
}

/// Patterns are relative to the file holding them, whatever the stored paths and the working
/// directory are.
#[test]
fn file_patterns_are_relative_to_their_file() {
    let tmp = tempdir("filelists");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cli/cov_report.vhd");
    write(&tmp, "rtl/gen/x", "");
    std::fs::copy(&src, tmp.join("rtl/gen/cov_report.vhd")).expect("copy");
    // Analysed with an absolute path, so the database holds an absolute path.
    let abs = tmp.join("rtl/gen/cov_report.vhd").display().to_string();
    let lib = format!("--work=work:{}/lib", tmp.display());
    let o = yavs(&tmp, &["analyze", &lib, &abs]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = yavs(
        &tmp,
        &["run", &lib, "--cov=sbt", "--cov-file=a.ycov", "cov_report"],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    write(
        &tmp,
        "rules/up.yaml",
        "exclude:\n  - kind: file\n    files: ../rtl/gen\n",
    );
    write(&tmp, "dir.txt", "file rtl/gen\n");
    write(
        &tmp,
        "except.txt",
        "files rtl files_exclude rtl/gen/*.vhd\n",
    );
    write(&tmp, "elsewhere/x.txt", "files rtl/gen\n");
    let total = |file: &str, cwd: &Path| {
        let db = tmp.join("a.ycov").display().to_string();
        let f = tmp.join(file).display().to_string();
        yavs(cwd, &["cov", "report", &db, "--exclude", &f, "--uncovered"])
    };
    // A pattern reaches only below the directory of its file, as in a lint waiver.
    let up = total("rules/up.yaml", &tmp);
    assert!(text(&up.stderr).contains("W0111"), "{}", text(&up.stderr));
    let o = total("dir.txt", &tmp);
    assert_eq!(text(&o.stdout), "", "{}", text(&o.stderr));
    // From another directory the file's own directory still counts.
    let o = total("dir.txt", &tmp.join("rtl"));
    assert_eq!(text(&o.stdout), "", "{}", text(&o.stderr));
    let o = total("except.txt", &tmp);
    assert!(!text(&o.stdout).is_empty(), "the file is excepted");
    // Relative to the file: `rtl/gen` from another directory covers nothing.
    let o = total("elsewhere/x.txt", &tmp);
    let e = text(&o.stderr);
    assert!(
        e.contains("warning[W0111]") && e.contains("`rtl/gen`"),
        "{e}"
    );
}

#[test]
fn configuration_file_gives_exclusions_root_and_kind() {
    let tmp = database("config");
    let rows = missed(&tmp);
    let stmt = rows
        .iter()
        .find(|r| r.starts_with("statement "))
        .expect("row");
    let branch = rows.iter().find(|r| r.starts_with("branch ")).expect("row");
    write(&tmp, "cfg/stmt.txt", &format!("{stmt}\n"));
    write(&tmp, "more.txt", &format!("{branch}\n"));
    write(
        &tmp,
        "cfg/c.yaml",
        "exclude: stmt.txt\nroot: ..\nkind: [statement]\n",
    );
    let run = |more: &[&str]| {
        let mut v = vec!["cov", "report", "a.ycov", "--config", "cfg/c.yaml"];
        v.extend(more);
        yavs(&tmp, &v)
    };
    // The file's exclusion, relative to it, and its kind.
    let u = text(&run(&["--uncovered"]).stdout);
    assert!(!u.contains(stmt.as_str()) && !u.contains("branch "), "{u}");
    assert_eq!(
        u.lines().count(),
        rows.iter().filter(|r| r.starts_with("statement ")).count() - 1
    );
    // Its root, relative to it: paths are shown from the directory above `cfg`.
    assert!(u.contains(" cov_report.vhd:"), "{u}");
    // The command line wins for kind and root, and adds up for exclusions.
    let o = run(&[
        "--uncovered",
        "--kind=branch",
        "--exclude",
        "more.txt",
        "--root",
        "cfg",
    ]);
    let u = text(&o.stdout);
    assert!(
        !u.contains(branch.as_str()) && !u.contains("statement "),
        "{u}"
    );
    let r = text(&run(&["--exclusions", "--exclude", "more.txt"]).stdout);
    let first = r.find("stmt.txt:1").expect("config rule");
    assert!(first < r.find("more.txt:1").expect("flag rule"), "{r}");
    // rank takes the same file.
    let o = yavs(&tmp, &["cov", "rank", "a.ycov", "--config", "cfg/c.yaml"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
}

#[test]
fn a_configuration_is_never_found_on_its_own() {
    let tmp = database("noconfig");
    let before = text(&yavs(&tmp, &["cov", "report", "a.ycov"]).stdout);
    for name in [
        "yavs-cov.yaml",
        ".yavs-cov.yaml",
        "yavs-coverage.yaml",
        "yavs.yaml",
    ] {
        write(&tmp, name, "kind: [toggle]\nbad key: 1\n");
    }
    assert_eq!(
        text(&yavs(&tmp, &["cov", "report", "a.ycov"]).stdout),
        before
    );
}

#[test]
fn a_bad_configuration_is_e0138() {
    let tmp = database("badconfig");
    write(&tmp, "c.yaml", "exlude: a\n");
    for cmd in ["report", "rank"] {
        for (f, want) in [
            ("c.yaml", "a key with a similar name exists: `exclude`"),
            ("gone.yaml", "`gone.yaml`"),
        ] {
            let o = yavs(&tmp, &["cov", cmd, "a.ycov", "--config", f]);
            assert_eq!(o.status.code(), Some(1));
            let e = text(&o.stderr);
            assert!(e.contains("error[E0138]") && e.contains(want), "{e}");
        }
    }
    // An exclusion file it names is that file's error, under its path from the configuration.
    write(&tmp, "d/c.yaml", "exclude: nope.txt\n");
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--config", "d/c.yaml"]);
    let e = text(&o.stderr);
    assert!(e.contains("error[E0112]") && e.contains("nope.txt"), "{e}");
}

/// An edited statement: its row goes stale, and W0111 says what stands there now.
#[test]
fn w0111_names_the_edited_statement() {
    let tmp = database("replaced");
    let rows = missed(&tmp);
    let row = rows
        .iter()
        .find(|r| r.starts_with("statement ") && r.contains("v := 0;"))
        .expect("row");
    let (head, _) = row.split_once("   # ").expect("comment");
    write(&tmp, "ex.txt", &format!("{head}\n"));
    let src = std::fs::read_to_string(tmp.join("cov_report.vhd")).expect("read");
    assert_eq!(src.matches("v := 0;").count(), 1);
    write(&tmp, "cov_report.vhd", &src.replace("v := 0;", "v := 7;"));
    for args in [
        vec!["analyze", "--work=work:lib", "cov_report.vhd"],
        vec![
            "run",
            "--work=work:lib",
            "--cov-file=b.ycov",
            "--cov-kind=statement,branch",
            "cov_report",
        ],
    ] {
        let o = yavs(&tmp, &args);
        assert!(o.status.success(), "{}", text(&o.stderr));
    }
    let o = yavs(
        &tmp,
        &["cov", "report", "b.ycov", "--exclude", "ex.txt", "--total"],
    );
    let e = text(&o.stderr);
    assert!(
        e.contains("warning[W0111]") && e.contains("ex.txt:1"),
        "{e}"
    );
    assert!(e.contains("now holds `v := 7;`"), "{e}");
    assert!(e.contains("it probably replaced the excluded one"), "{e}");
    assert!(e.contains("the row is now: statement "), "{e}");
    // The new row excludes it, and the warning is gone.
    let help = e
        .lines()
        .find(|l| l.contains("the row is now: "))
        .expect("help");
    let new = help.split("the row is now: ").nth(1).expect("row");
    write(&tmp, "ex.txt", &format!("{new}\n"));
    let o = yavs(
        &tmp,
        &["cov", "report", "b.ycov", "--exclude", "ex.txt", "--total"],
    );
    assert!(text(&o.stderr).is_empty(), "{}", text(&o.stderr));
}

/// `cov_run.vhd`: entity `cov_run` over two `cov_sub`, process `decode` in each, package `cov_pkg`.
fn run_database(name: &str, args: &[&str]) -> Tmp {
    let tmp = tempdir(name);
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cli/cov_run.vhd");
    std::fs::copy(&src, tmp.join("cov_run.vhd")).expect("copy");
    let o = yavs(&tmp, &["analyze", "--work=mylib:lib", "cov_run.vhd"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let mut v = vec![
        "run",
        "--work=mylib:lib",
        "--cov-file=a.ycov",
        "--cov-kind=statement,branch,toggle",
    ];
    v.extend(args);
    v.push("cov_run");
    let o = yavs(&tmp, &v);
    assert!(o.status.success(), "{}", text(&o.stderr));
    tmp
}

/// The item lines of the summary's path table left after a rule, by path.
fn paths_after(tmp: &Path, rule: &str) -> (Vec<String>, String) {
    write(tmp, "r.txt", rule);
    let o = yavs(tmp, &["cov", "report", "a.ycov", "--exclude", "r.txt"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let out = text(&o.stdout);
    let table = out.split("Instance ").nth(1).unwrap_or("");
    let names = table
        .lines()
        .skip(2)
        .take_while(|l| !l.trim().is_empty())
        .map(|l| l.split_whitespace().next().unwrap_or("").to_string())
        .collect();
    (names, text(&o.stderr))
}

#[test]
fn library_entity_and_process_rows_on_a_real_run() {
    let tmp = run_database("names", &[]);
    let all = paths_after(&tmp, "").0;
    assert_eq!(
        all,
        ["cov_run", "cov_run.u1", "cov_run.u2", "mylib.cov_pkg"]
    );
    assert_eq!(
        paths_after(&tmp, "entity cov_sub\n").0,
        ["cov_run", "mylib.cov_pkg"]
    );
    assert_eq!(
        paths_after(&tmp, "entity mylib.cov_s*\n").0,
        ["cov_run", "mylib.cov_pkg"]
    );
    assert!(paths_after(&tmp, "library mylib\n").0.is_empty());
    // A process takes its statements and branches.
    let rows = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--uncovered"]).stdout);
    assert!(rows.contains("[process decode]"), "{rows}");
    write(&tmp, "r.txt", "process cov_run.u* decode\n");
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "r.txt",
            "--uncovered",
        ],
    );
    let left = text(&o.stdout);
    assert!(
        !left.contains("[process decode]") && left.contains("[process stim]"),
        "{left}"
    );
    let r = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "r.txt",
            "--exclusions",
        ],
    );
    let r = text(&r.stdout);
    assert!(r.contains("rule:    process cov_run.u* decode"), "{r}");
    // A near name, on a row that matches nothing.
    let (_, e) = paths_after(&tmp, "entity cov_sb\n");
    assert!(
        e.contains("warning[W0111]") && e.contains("`cov_sub`"),
        "{e}"
    );
    let (_, e) = paths_after(&tmp, "process cov_run.u1 decod\n");
    assert!(e.contains("`decode`"), "{e}");
    // The report rows of what never ran are still exclusion rows.
    write(&tmp, "r.txt", &rows);
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "r.txt",
            "--uncovered",
        ],
    );
    assert_eq!(text(&o.stdout), "", "{}", text(&o.stderr));
    let y = yavs(&tmp, &["cov", "report", "a.ycov", "--uncovered=yaml"]);
    write(&tmp, "r.yaml", &text(&y.stdout));
    let o = yavs(
        &tmp,
        &[
            "cov",
            "report",
            "a.ycov",
            "--exclude",
            "r.yaml",
            "--uncovered",
        ],
    );
    assert_eq!(text(&o.stdout), "", "{}", text(&o.stderr));
}

#[test]
fn an_older_database_says_it_lacks_the_names() {
    let tmp = tempdir("oldnames");
    write(
        &tmp,
        "old.ycov",
        "yavs-coverage 6\ns\t:t\t0000000000000007\t0\t0\t-\t3\ta.vhd\tx;\n",
    );
    for (row, word) in [
        ("library work\n", "library"),
        ("entity e\n", "entity"),
        ("process t p\n", "process"),
    ] {
        write(&tmp, "r.txt", row);
        for cmd in ["report", "rank"] {
            let o = yavs(&tmp, &["cov", cmd, "old.ycov", "--exclude", "r.txt"]);
            assert_eq!(o.status.code(), Some(1));
            let e = text(&o.stderr);
            assert!(
                e.contains("error[E0112]") && e.contains(&format!("a `{word}` row")),
                "{e}"
            );
            assert!(e.contains("written by an older yavs"), "{e}");
            assert!(e.contains("run the simulations again"), "{e}");
        }
    }
    // Merged with a new one the file says so still; other rows work.
    let new = run_database("oldnames2", &[]);
    let db = new.join("a.ycov").display().to_string();
    let o = yavs(&tmp, &["cov", "merge", "old.ycov", &db, "-o", "m.ycov"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    write(&tmp, "r.txt", "entity cov_sub\n");
    let o = yavs(&tmp, &["cov", "report", "m.ycov", "--exclude", "r.txt"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stderr));
    write(&tmp, "r.txt", "instance t\n");
    let o = yavs(&tmp, &["cov", "report", "old.ycov", "--exclude", "r.txt"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
}

/// What the new names need is read from the design: the counts and the rest of each database line are
/// the same at any thread count.
#[test]
fn names_do_not_change_with_threads() {
    let one = run_database("t1", &["--threads=1"]);
    let four = run_database("t4", &["--threads=4"]);
    let (a, b) = (
        std::fs::read_to_string(one.join("a.ycov")).expect("read"),
        std::fs::read_to_string(four.join("a.ycov")).expect("read"),
    );
    assert!(a.contains("\tmylib.cov_sub\tdecode\n"), "{a}");
    // Paths in the two directories differ only by the directory.
    let (a, b) = (
        a.replace(&one.display().to_string(), ""),
        b.replace(&four.display().to_string(), ""),
    );
    assert_eq!(a, b);
}

/// A design with one decision under a directive, analysed and run into `name.ycov`.
fn decision(tmp: &Path, name: &str, cond: &str, directive: &str) -> Output {
    decision_of(tmp, name, cond, directive, "combination")
}

fn decision_of(tmp: &Path, name: &str, cond: &str, directive: &str, kinds: &str) -> Output {
    write(
        tmp,
        "d.vhd",
        &format!(
            "entity d is end entity;\narchitecture a of d is\n  signal x, y : integer := 0;\n  \
             signal r : bit := '1';\nbegin\n  process begin\n    -- yavs cov exclude {directive}\n    \
             if {cond} then\n      y <= 1;\n    end if;\n    wait;\n  end process;\nend architecture;\n"
        ),
    );
    let o = yavs(tmp, &["analyze", "--work=work:lib", "d.vhd"]);
    if !o.status.success() {
        return o;
    }
    yavs(
        tmp,
        &[
            "run",
            "--work=work:lib",
            &format!("--cov-file={name}.ycov"),
            &format!("--cov-kind={kinds}"),
            "d",
        ],
    )
}

#[test]
fn named_inputs_take_any_operand_text() {
    let tmp = tempdir("named");
    for form in [
        "(x > 3 = false, r = '1' = true)",
        "(X>3=false)",
        "( ( x  >  3 ) = false )",
        "condition (x > 3 = true)",
    ] {
        let o = decision(&tmp, "n", "x > 3 or r = '1'", form);
        let e = text(&o.stderr);
        assert!(o.status.success() && !e.contains("error"), "{form}: {e}");
        let r = yavs(&tmp, &["cov", "report", "n.ycov", "--exclusions"]);
        assert!(
            text(&r.stdout).contains("applied") || text(&r.stdout).contains("matches"),
            "{form}"
        );
    }
    // Not an input: the error lists them and says how to write one, with a near one.
    let o = decision(&tmp, "n", "x > 3 or r = '1'", "(x > 4 = false)");
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stderr));
    let e = text(&o.stderr);
    assert!(
        e.contains("error[E2022]") && e.contains("inputs, in order"),
        "{e}"
    );
    assert!(e.contains("`x>3`"), "{e}");
    let o = decision(&tmp, "n", "x > 3 or x > 3", "(x > 3 = false)");
    let e = text(&o.stderr);
    assert!(
        e.contains("error[E2022]") && e.contains("is 2 inputs"),
        "{e}"
    );
}

/// A waiver on one side only stays the error it was, whatever the operand text.
#[test]
fn e0137_is_unchanged_by_the_operand_text() {
    let tmp = tempdir("conflict");
    assert!(
        decision(&tmp, "a", "x > 3 or r = '1'", "01")
            .status
            .success()
    );
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "--cov-file=b.ycov",
            "--cov-kind=combination",
            "--coverage-directives=ignore",
            "d",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = yavs(&tmp, &["cov", "merge", "a.ycov", "b.ycov", "-o", "m.ycov"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        text(&o.stderr).contains("error[E0137]"),
        "{}",
        text(&o.stderr)
    );
}

/// What a run of `x > 3 or r = '1'` (x is 0, r is '1') holds under a directive.
fn rows_after(tmp: &Path, directive: &str, kinds: &str) -> (String, String) {
    let o = decision_of(tmp, "r", "x > 3 or r = '1'", directive, kinds);
    assert!(o.status.success(), "{directive}: {}", text(&o.stderr));
    let total = yavs(
        tmp,
        &["cov", "report", "r.ycov", "--kind=condition", "--uncovered"],
    );
    assert!(total.status.success(), "{}", text(&total.stderr));
    let ex = yavs(tmp, &["cov", "report", "r.ycov", "--exclusions"]);
    (text(&total.stdout), text(&ex.stdout))
}

#[test]
fn a_row_directive_waives_one_row() {
    let tmp = tempdir("rowdir");
    let (all, _) = rows_after(&tmp, "(r = '1' = true)", "condition");
    let count = |t: &str| t.lines().filter(|l| l.starts_with("condition ")).count();
    let before = count(&all);
    assert!(before >= 2, "{all}");
    // Whichever row never ran: the waived row leaves, the others of the decision stay.
    for (dir, row) in [
        ("row 0-: never low", "x > 3 is 0"),
        ("row x > 3 = 0: never low", "x > 3 is 0"),
        ("condition row X>3 = false", "x > 3 is 0"),
        ("row 1-", "x > 3 is 1"),
        ("row r = '1' = 0", "r = '1' is 0"),
    ] {
        let (rows, report) = rows_after(&tmp, dir, "condition");
        let missed = all.contains(row);
        assert!(!rows.contains(row), "{dir}: {rows}");
        assert_eq!(count(&rows), before - usize::from(missed), "{dir}: {rows}");
        assert!(
            report.contains("(in the source)  applied"),
            "{dir}: {report}"
        );
        assert!(
            report.contains("0 of them ran") || !missed,
            "{dir}: {report}"
        );
    }
    let (_, report) = rows_after(&tmp, "row 0-: never low", "condition");
    assert!(
        report.contains("rule:    row 0-") && report.contains("reason:  never low"),
        "{report}"
    );
    // The totals drop it: four rows, three after the waiver.
    let o = decision_of(&tmp, "w", "x > 3 or r = '1'", "row 0-", "condition");
    assert!(o.status.success());
    let plain = decision_of(
        &tmp,
        "p",
        "x > 3 or r = '1'",
        "(r = '1' = true)",
        "condition",
    );
    assert!(plain.status.success());
    let total = |db: &str| {
        let o = yavs(&tmp, &["cov", "report", db, "--kind=condition"]);
        let out = text(&o.stdout);
        out.lines()
            .find(|l| l.starts_with("condition"))
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(
        total("p.ycov")
            .split_whitespace()
            .nth(1)
            .map(|t| Some(t.to_string())),
        Some(Some("4".to_string()))
    );
    assert_eq!(
        total("w.ycov")
            .split_whitespace()
            .nth(1)
            .map(|t| Some(t.to_string())),
        Some(Some("3".to_string()))
    );
}

/// A row that ran: it leaves the total and the exclusion report shows that it ran.
#[test]
fn a_waived_row_that_ran_is_shown() {
    let tmp = tempdir("rowran");
    // `r = '1'` true with x > 3 false decides the result, so this row runs.
    let (_, report) = rows_after(&tmp, "row -1", "condition");
    assert!(report.contains("1 of them ran"), "{report}");
    assert!(!report.contains("[0 hits]"), "{report}");
    let o = yavs(&tmp, &["cov", "report", "r.ycov", "--exclusions=json"]);
    assert!(
        text(&o.stdout).contains("\"kind\": \"condition\""),
        "{}",
        text(&o.stdout)
    );
}

/// `row` and a combination pattern on one decision are different requirements.
#[test]
fn a_row_is_not_a_combination() {
    let tmp = tempdir("rowcomb");
    let kinds = "condition,combination";
    let o = decision_of(&tmp, "a", "x > 3 or r = '1'", "row 0-", kinds);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let db = yavs_cov::Database::parse(&std::fs::read_to_string(tmp.join("a.ycov")).expect("read"))
        .expect("parse");
    let waived = |name: &str| {
        db.items
            .iter()
            .filter(|(k, e)| k.kind.name() == name && !e.waivers.is_empty())
            .count()
    };
    assert_eq!((waived("condition"), waived("combination")), (1, 0));
    // And the same pattern without `row` is the combination it was.
    let o = decision_of(&tmp, "b", "x > 3 or r = '1'", "0-", kinds);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let db = yavs_cov::Database::parse(&std::fs::read_to_string(tmp.join("b.ycov")).expect("read"))
        .expect("parse");
    let waived = |name: &str| {
        db.items
            .iter()
            .filter(|(k, e)| k.kind.name() == name && !e.waivers.is_empty())
            .count()
    };
    assert_eq!(waived("condition"), 0);
    assert!(waived("combination") >= 1);
}

#[test]
fn row_directive_errors() {
    let tmp = tempdir("rowerr");
    for (dir, code, want) in [
        ("row", "E2020", "`row` needs"),
        ("row 01", "E2021", "exactly one"),
        ("row -0-", "E2021", "3 positions"),
        ("row y > 3 = 0", "E2022", "not an input"),
        ("row x > 4 = 0", "E2022", "`x>3`"),
        ("row x > 3 = 2", "E2023", "invalid row value"),
        ("expression row 0-", "E2024", "does not fit"),
        ("rwo 0-", "E2020", "`row`"),
    ] {
        let o = decision_of(&tmp, "e", "x > 3 or r = '1'", dir, "condition");
        assert_eq!(o.status.code(), Some(1), "{dir}");
        let e = text(&o.stderr);
        assert!(
            e.contains(&format!("error[{code}]")) && e.contains(want),
            "{dir}: {e}"
        );
    }
    let o = decision_of(&tmp, "d", "x > 3 or r = '1'", "row 0-", "condition");
    assert!(o.status.success());
    // Repeated: W2020.
    write(
        &tmp,
        "d2.vhd",
        "entity d2 is end entity;\narchitecture a of d2 is\n  signal x : integer := 0;\n  signal r : bit := '1';\nbegin\n  process begin\n    -- yavs cov exclude row 0-\n    -- yavs cov exclude row x > 3 = 0\n    if x > 3 or r = '1' then null; end if;\n    wait;\n  end process;\nend architecture;\n",
    );
    let o = yavs(&tmp, &["analyze", "--work=work:lib", "d2.vhd"]);
    assert!(
        text(&o.stderr).contains("warning[W2020]"),
        "{}",
        text(&o.stderr)
    );
}

/// E0137 holds for rows as for combinations.
#[test]
fn rows_follow_the_merge_rules() {
    let tmp = tempdir("rowmerge");
    assert!(
        decision_of(&tmp, "a", "x > 3 or r = '1'", "row 0-", "condition")
            .status
            .success()
    );
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "--cov-file=b.ycov",
            "--cov-kind=condition",
            "--coverage-directives=ignore",
            "d",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = yavs(&tmp, &["cov", "merge", "a.ycov", "b.ycov", "-o", "m.ycov"]);
    assert_eq!(o.status.code(), Some(1));
    let e = text(&o.stderr);
    assert!(
        e.contains("error[E0137]") && e.contains("condition x > 3 = 0 of d"),
        "{e}"
    );
}

/// Waivers change no counts: the counters of a run are the same with and without the directive.
#[test]
fn a_row_waiver_changes_no_count() {
    let tmp = tempdir("rowcount");
    assert!(
        decision_of(&tmp, "a", "x > 3 or r = '1'", "row 0-", "condition")
            .status
            .success()
    );
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "--cov-file=b.ycov",
            "--cov-kind=condition",
            "--threads=4",
            "--coverage-directives=ignore",
            "d",
        ],
    );
    assert!(o.status.success());
    let read = |n: &str| {
        yavs_cov::Database::parse(&std::fs::read_to_string(tmp.join(n)).expect("read"))
            .expect("parse")
    };
    let (a, b) = (read("a.ycov"), read("b.ycov"));
    let counts = |d: &yavs_cov::Database| {
        d.items
            .iter()
            .map(|(k, e)| (k.clone(), e.count))
            .collect::<Vec<_>>()
    };
    assert_eq!(counts(&a), counts(&b));
    // The report's line of what to write is what the source takes.
    let (k, e) = a
        .items
        .iter()
        .find(|(k, _)| k.kind.name() == "condition")
        .expect("row");
    assert!(
        yavs_cov::exclude::directive(k, e)
            .expect("text")
            .starts_with("-- yavs cov exclude row ")
    );
}

/// The value of an assignment has expression rows, and the kind word must fit it.
#[test]
fn a_row_directive_waives_an_expression_row() {
    let tmp = tempdir("rowexpr");
    let design = |directive: &str| {
        format!(
            "entity d is end entity;\narchitecture a of d is\n  signal a, b, y : bit;\nbegin\n  process begin\n    -- yavs cov exclude {directive}\n    y <= a and b;\n    wait;\n  end process;\nend architecture;\n"
        )
    };
    let run = |directive: &str| {
        write(&tmp, "d.vhd", &design(directive));
        let o = yavs(&tmp, &["analyze", "--work=work:lib", "d.vhd"]);
        if !o.status.success() {
            return o;
        }
        yavs(
            &tmp,
            &[
                "run",
                "--work=work:lib",
                "--cov-file=a.ycov",
                "--cov-kind=expression",
                "d",
            ],
        )
    };
    let o = run("expression row -1: b is never high");
    assert!(o.status.success(), "{}", text(&o.stderr));
    let r = yavs(&tmp, &["cov", "report", "a.ycov", "--exclusions"]);
    let r = text(&r.stdout);
    assert!(r.contains("rule:    expression row -1"), "{r}");
    assert!(r.contains("expression  ") && r.contains("1 (0 ran)"), "{r}");
    let e = text(&run("condition row -1").stderr);
    assert!(
        e.contains("error[E2024]") && e.contains("does not fit"),
        "{e}"
    );
}

/// The directive of a source file, analysed with and without a run.
fn analyse_directive(tmp: &Path, cond: &str, directive: &str) -> Output {
    write(
        tmp,
        "d.vhd",
        &format!(
            "entity d is end entity;\narchitecture a of d is\n  signal x, y : integer := 0;\n  \
             signal r : bit := '1';\nbegin\n  process begin\n    -- yavs cov exclude {directive}\n    \
             if {cond} then\n      y <= 1;\n    end if;\n    wait;\n  end process;\nend architecture;\n"
        ),
    );
    yavs(tmp, &["analyze", "--work=work:lib", "d.vhd"])
}

const COND: &str = "x > 3 or r = '1'";

#[test]
fn the_list_of_inputs_is_checked_at_analysis() {
    let tmp = tempdir("listok");
    for good in [
        "01 (x > 3, r = '1')",
        "01(x>3,R='1')",
        "01 ( (X  >  3) , r='1' ): why",
        "condition 01 (x > 3, r = '1')",
        "row -0 (x > 3, r = '1')",
        "row 1-(x>3, r = '1'): why",
        "condition row 1- (x > 3, r = '1')",
    ] {
        let o = analyse_directive(&tmp, COND, good);
        assert!(
            o.status.success() && text(&o.stderr).is_empty(),
            "{good}: {}",
            text(&o.stderr)
        );
    }
    // Reordered: both orders named, and the directive to write has the symbols moved with them.
    for (dir, help) in [
        (
            "01 (r = '1', x > 3): why",
            "write: -- yavs cov exclude 10 (x > 3, r = '1'): why",
        ),
        (
            "row -0 (r = '1', x > 3)",
            "write: -- yavs cov exclude row 0- (x > 3, r = '1')",
        ),
        (
            "condition 01 (r = '1', x > 3)",
            "write: -- yavs cov exclude condition 10 (x > 3, r = '1')",
        ),
    ] {
        let o = analyse_directive(&tmp, COND, dir);
        assert_eq!(o.status.code(), Some(1), "{dir}");
        let e = text(&o.stderr);
        assert!(
            e.contains("error[E2022]") && e.contains("no longer match"),
            "{e}"
        );
        assert!(e.contains("found: r = '1', x > 3"), "{e}");
        assert!(e.contains("expected: x > 3, r = '1'"), "{e}");
        assert!(e.contains(help), "{dir}: {e}");
        // What it says to write is accepted.
        let fixed = e
            .lines()
            .find_map(|l| l.split("write: -- yavs cov exclude ").nth(1))
            .expect("help");
        assert!(
            analyse_directive(&tmp, COND, fixed).status.success(),
            "{fixed}"
        );
    }
    // Text changed or an input added: the input it did not name is `-`, and it says so.
    for dir in [
        "01 (x > 4, r = '1')",
        "01 (x > 3)",
        "01- (x > 3, r = '1', y = 1)",
    ] {
        let o = analyse_directive(&tmp, COND, dir);
        let e = text(&o.stderr);
        assert_eq!(o.status.code(), Some(1), "{dir}");
        assert!(
            e.contains("error[E2022]") && e.contains("expected: x > 3, r = '1'"),
            "{dir}: {e}"
        );
        assert!(e.contains("write: -- yavs cov exclude "), "{dir}: {e}");
    }
    let e = text(&analyse_directive(&tmp, COND, "01 (x > 4, r = '1')").stderr);
    assert!(
        e.contains("-1 (x > 3, r = '1')") && e.contains("did not name"),
        "{e}"
    );
    // What it waived is gone.
    let e = text(&analyse_directive(&tmp, COND, "row 0- (x > 4, r = '1')").stderr);
    assert!(e.contains("what it waived is gone"), "{e}");
    // A list that fits with a pattern that does not is the pattern's error.
    let e = text(&analyse_directive(&tmp, COND, "010 (x > 3, r = '1')").stderr);
    assert!(e.contains("error[E2021]"), "{e}");
    // Malformed.
    for dir in ["01 (x > 3", "01 ()", "01 (x > 3,)"] {
        let e = text(&analyse_directive(&tmp, COND, dir).stderr);
        assert!(e.contains("error[E2020]"), "{dir}: {e}");
    }
    // Under ignore nothing is read.
    write(&tmp, "x", "");
    let bad = analyse_directive(&tmp, COND, "01 (r = '1', x > 3)");
    assert_eq!(bad.status.code(), Some(1));
    let o = yavs(
        &tmp,
        &[
            "analyze",
            "--work=work:lib",
            "--coverage-directives=ignore",
            "d.vhd",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
}

/// An operand that holds `=`, a comma or a parenthesis, in the list and in the named form.
#[test]
fn the_list_and_the_named_form_do_not_clash() {
    let tmp = tempdir("listnamed");
    let cond = "f(x, 1) = 2 or s = \"a,)\" or c = ','";
    let wrap = |dir: &str| {
        format!(
            "entity d is end entity;\narchitecture a of d is\n  signal x : integer := 0;\n  signal s : string(1 to 3) := \"abc\";\n  signal c : character := 'a';\n  function f(a, b : integer) return integer is begin return a + b; end function;\nbegin\n  process begin\n    -- yavs cov exclude {dir}\n    if {cond} then\n      null;\n    end if;\n    wait;\n  end process;\nend architecture;\n"
        )
    };
    for dir in [
        "0-- (f(x, 1) = 2, s = \"a,)\", c = ',')",
        "row 0-- (F(x,1)=2, s = \"a,)\", C = ',')",
        "(f(x, 1) = 2 = false)",
        "(s = \"a,)\" = false, c = ',' = true)",
    ] {
        write(&tmp, "d.vhd", &wrap(dir));
        let o = yavs(&tmp, &["analyze", "--work=work:lib", "d.vhd"]);
        assert!(
            o.status.success() && text(&o.stderr).is_empty(),
            "{dir}: {}",
            text(&o.stderr)
        );
    }
    // Case in a literal counts.
    write(
        &tmp,
        "d.vhd",
        &wrap("0-- (f(x, 1) = 2, s = \"A,)\", c = ',')"),
    );
    let o = yavs(&tmp, &["analyze", "--work=work:lib", "d.vhd"]);
    assert!(
        text(&o.stderr).contains("error[E2022]"),
        "{}",
        text(&o.stderr)
    );
}

/// The short forms without the list are full forms: no warning, also under -Werror.
#[test]
fn the_bare_forms_are_accepted_without_a_warning() {
    let tmp = tempdir("bare");
    for (dir, kind) in [
        ("01", "combination"),
        ("row 0-", "condition"),
        ("condition 00", "combination"),
    ] {
        let o = analyse_directive(&tmp, COND, dir);
        assert!(
            o.status.success() && text(&o.stderr).is_empty(),
            "{dir}: {}",
            text(&o.stderr)
        );
        let o = yavs(&tmp, &["analyze", "-Werror", "--work=work:lib", "d.vhd"]);
        assert!(o.status.success(), "{dir}: {}", text(&o.stderr));
        let o = yavs(
            &tmp,
            &[
                "run",
                "--work=work:lib",
                "--cov-file=a.ycov",
                &format!("--cov-kind={kind}"),
                "d",
            ],
        );
        assert!(
            o.status.success() && text(&o.stderr).is_empty(),
            "{dir}: {}",
            text(&o.stderr)
        );
        let db =
            yavs_cov::Database::parse(&std::fs::read_to_string(tmp.join("a.ycov")).expect("read"))
                .expect("parse");
        assert!(db.items.values().any(|e| !e.waivers.is_empty()), "{dir}");
        let o = yavs(&tmp, &["cov", "report", "a.ycov", "--exclusions"]);
        assert!(text(&o.stdout).contains("(in the source)"), "{dir}");
        assert!(text(&o.stderr).is_empty(), "{dir}: {}", text(&o.stderr));
    }
}

/// What a report suggests is the checked form, and it can be pasted before the decision as it is.
#[test]
fn a_suggestion_can_be_pasted() {
    let tmp = tempdir("paste");
    for cond in [COND, "x > 3\n      or -- limit\n      r = '1'"] {
        let o = decision_of(&tmp, "a", cond, "(r = '1' = true)", "condition,combination");
        assert!(o.status.success(), "{}", text(&o.stderr));
        let db =
            yavs_cov::Database::parse(&std::fs::read_to_string(tmp.join("a.ycov")).expect("read"))
                .expect("parse");
        let mut seen = 0;
        for (k, e) in &db.items {
            let Some(d) = yavs_cov::exclude::directive(k, e) else {
                continue;
            };
            seen += 1;
            assert!(d.contains(" (x > 3, r = '1')"), "{d}");
            let dir = d.strip_prefix("-- yavs cov exclude ").expect("prefix");
            let o = analyse_directive(&tmp, cond, dir);
            assert!(
                o.status.success() && !text(&o.stderr).contains("error"),
                "{d}: {}",
                text(&o.stderr)
            );
        }
        assert!(seen >= 5, "{seen}");
    }
}

/// Three state machines, each with a state nothing enters and a transition nothing takes: one
/// process, two processes, and a `wait until` process, with a test bench that instantiates the
/// first one twice. The `case` lines stand alone, so a test can put a directive before them.
const MACHINES: &str = "\
entity one_p is port (clk : in bit); end entity;
architecture a of one_p is
  type one_t is (idle, busy, done, spare);
  signal one : one_t := idle;
begin
  one_proc : process (clk) begin
    if rising_edge(clk) then
      case one is
        when idle => one <= busy;
        when busy => one <= idle;
        when done => one <= idle;
        when spare => one <= done;
      end case;
    end if;
  end process;
end architecture;

entity two_p is port (clk : in bit); end entity;
architecture a of two_p is
  type two_t is (a0, a1, a2);
  signal two, nxt : two_t := a0;
begin
  reg_proc : process (clk) begin
    if rising_edge(clk) then two <= nxt; end if;
  end process;
  next_proc : process (two) begin
    case two is
      when a0 => nxt <= a1;
      when a1 => nxt <= a0;
      when a2 => nxt <= a0;
    end case;
  end process;
end architecture;

entity wait_p is port (clk : in bit); end entity;
architecture a of wait_p is
  type wt_t is (b0, b1, b2);
  signal wt : wt_t := b0;
begin
  wait_proc : process begin
    wait until rising_edge(clk);
    case wt is
      when b0 => wt <= b1;
      when b1 => wt <= b0;
      when b2 => wt <= b0;
    end case;
  end process;
end architecture;

entity tb is end entity;
architecture t of tb is
  signal clk : bit;
begin
  u1 : entity work.one_p port map (clk);
  u1b : entity work.one_p port map (clk);
  u2 : entity work.two_p port map (clk);
  u3 : entity work.wait_p port map (clk);
  process begin
    for i in 1 to 6 loop
      clk <= '1'; wait for 1 ns; clk <= '0'; wait for 1 ns;
    end loop;
    wait;
  end process;
end architecture;
";

/// Analyse `src` and run the test bench with state machine coverage into `a.ycov`.
fn fsm_run(tmp: &Path, src: &str, extra: &[&str]) -> String {
    write(tmp, "m.vhd", src);
    let _ = std::fs::remove_dir_all(tmp.join("lib"));
    let o = yavs(tmp, &["analyze", "--work=work:lib", "m.vhd"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let args = [
        &[
            "run",
            "--work=work:lib",
            "--cov-file=a.ycov",
            "--cov-kind=fsm",
        ][..],
        extra,
        &["tb"][..],
    ]
    .concat();
    let o = yavs(tmp, &args);
    assert!(o.status.success(), "{}", text(&o.stderr));
    text(&o.stderr)
}

/// The `fsm` line of the kinds table of a report: items, hit, missed.
fn fsm_totals(tmp: &Path, extra: &[&str]) -> (u32, u32, u32) {
    let args = [&["cov", "report", "a.ycov"][..], extra].concat();
    let o = yavs(tmp, &args);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let t = text(&o.stdout);
    let Some(l) = t.lines().find(|l| l.starts_with("fsm ")) else {
        return (0, 0, 0);
    };
    let n: Vec<u32> = l
        .split_whitespace()
        .skip(1)
        .take(3)
        .map(|x| x.parse().expect("number"))
        .collect();
    (n[0], n[1], n[2])
}

/// Everything in the design: three machines, one of them twice: 8, 8, 6 and 6 items.
const FSM_ALL: (u32, u32, u32) = (28, 16, 12);

#[test]
fn fsm_rows_take_a_state_and_a_transition() {
    let tmp = tempdir("fsm_rows");
    fsm_run(&tmp, MACHINES, &[]);
    assert_eq!(fsm_totals(&tmp, &[]), FSM_ALL);
    let (i, h, m) = FSM_ALL;
    for (form, row) in [
        ("state", "fsm tb.u1 one spare # unused\n"),
        ("transition", "fsm tb.u1 one spare->done # unused\n"),
        ("to anything", "fsm tb.u1 one spare->* # unused\n"),
    ] {
        write(&tmp, "r.txt", row);
        let args = ["--exclude", "r.txt"];
        assert_eq!(fsm_totals(&tmp, &args), (i - 1, h, m - 1), "{form}");
        let args = ["--exclude", "r.txt", "--detailed"];
        let o = yavs(
            &tmp,
            &[&["cov", "report", "a.ycov"][..], &args[..]].concat(),
        );
        assert!(text(&o.stdout).contains("excluded: unused"), "{form}");
    }
    for (form, item) in [("state", "spare"), ("transition", "spare->done")] {
        let y = format!(
            "exclude:\n  - kind: fsm\n    instance: tb.u1\n    signal: one\n    item: {item}\n    reason: unused\n"
        );
        write(&tmp, "r.yaml", &y);
        let args = ["--exclude", "r.yaml"];
        assert_eq!(fsm_totals(&tmp, &args), (i - 1, h, m - 1), "{form}");
    }
    // A state keeps its transitions, and the other way round.
    write(&tmp, "r.txt", "fsm tb.u1 one spare\n");
    let args = ["--exclude", "r.txt", "--detailed"];
    let t = text(
        &yavs(
            &tmp,
            &[&["cov", "report", "a.ycov"][..], &args[..]].concat(),
        )
        .stdout,
    );
    let after = t.split("=== tb.u1 ").nth(1).expect("section");
    let row = after.lines().find(|l| l.contains("one: spare -> done"));
    assert!(row.is_some_and(|l| l.contains("MISSED")), "{t}");
}

#[test]
fn fsm_items_follow_the_other_rows() {
    let tmp = tempdir("fsm_other_rows");
    fsm_run(&tmp, MACHINES, &[]);
    let (i, ..) = FSM_ALL;
    for (row, gone) in [
        ("instance tb.u1\n", 8),
        ("instance tb\n", 28),
        ("entity one_p\n", 16),
        ("entity work.one_p\n", 16),
        ("file m.vhd\n", 28),
        ("library work\n", 28),
        // The process with the `case`, not the one that clocks the register.
        ("process tb.u1 one_proc\n", 8),
        ("process tb.u2 next_proc\n", 6),
        ("process tb.u3 wait_proc\n", 6),
    ] {
        write(&tmp, "r.txt", row);
        let (items, ..) = fsm_totals(&tmp, &["--exclude", "r.txt"]);
        assert_eq!(items, i - gone, "{row}");
    }
    // No package holds them, and the clocked process of a two-process machine has no `case`.
    for row in ["unit work.one_p\n", "process tb.u2 reg_proc\n"] {
        write(&tmp, "r.txt", row);
        let o = yavs(&tmp, &["cov", "report", "a.ycov", "--exclude", "r.txt"]);
        assert!(text(&o.stderr).contains("W0111"), "{row}");
        assert_eq!(fsm_totals(&tmp, &["--exclude", "r.txt"]), FSM_ALL, "{row}");
    }
}

#[test]
fn fsm_items_follow_the_regions() {
    let tmp = tempdir("fsm_regions");
    let (i, ..) = FSM_ALL;
    // Around the `case`: the machine, in both its instances.
    let around = MACHINES
        .replace(
            "      case one is",
            "      -- coverage off\n      case one is",
        )
        .replace(
            "      end case;\n    end if;",
            "      end case;\n      -- coverage on\n    end if;",
        );
    fsm_run(&tmp, &around, &[]);
    assert_eq!(fsm_totals(&tmp, &[]).0, i - 16);
    // Around the register: the same.
    let reg = MACHINES.replace(
        "  signal wt : wt_t := b0;",
        "  signal wt : wt_t := b0; -- coverage off-line",
    );
    fsm_run(&tmp, &reg, &[]);
    assert_eq!(fsm_totals(&tmp, &[]).0, i - 6);
    // One literal, one assignment: that item.
    let one = MACHINES
        .replace(
            "type wt_t is (b0, b1, b2);",
            "type wt_t is (b0, b1,\n    b2 -- coverage off-line\n  );",
        )
        .replace(
            "      when b2 => wt <= b0;",
            "      when b2 =>\n        wt <= b0; -- coverage off-line",
        );
    fsm_run(&tmp, &one, &[]);
    assert_eq!(fsm_totals(&tmp, &[]).0, i - 2);
    // `translate_off` only with the option.
    let synth = MACHINES
        .replace(
            "    case wt is",
            "    -- pragma translate_off\n    case wt is",
        )
        .replace(
            "    end case;\n  end process;\nend architecture;\n\nentity tb",
            "    end case;\n    -- pragma translate_on\n  end process;\nend architecture;\n\nentity tb",
        );
    fsm_run(&tmp, &synth, &[]);
    assert_eq!(fsm_totals(&tmp, &[]).0, i);
    fsm_run(&tmp, &synth, &["--cov-exclude-translate-off"]);
    assert_eq!(fsm_totals(&tmp, &[]).0, i - 6);
    // A region never closed warns once, with counters too.
    let open = MACHINES.replace(
        "  signal wt : wt_t",
        "  -- coverage off\n  signal wt : wt_t",
    );
    let said = fsm_run(&tmp, &open, &[]);
    assert_eq!(said.matches("W0112").count(), 1, "{said}");
    let said = fsm_run(&tmp, &open, &["--cov-kind=fsm,statement"]);
    assert_eq!(said.matches("W0112").count(), 1, "{said}");
}

/// The directive a report prints goes before the `case` of its register and takes out that item,
/// in every instance of the design unit: one process, two processes, `wait until`.
#[test]
fn fsm_directive_printed_by_the_report_works_when_pasted() {
    let tmp = tempdir("fsm_paste");
    fsm_run(&tmp, MACHINES, &[]);
    let detailed = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--detailed"]).stdout);
    let printed: std::collections::BTreeSet<(String, String)> = detailed
        .lines()
        .filter_map(|l| {
            let (row, rest) = l.split_once("MISSED (exclude with ")?;
            let register = row.split_whitespace().nth(1)?.trim_end_matches(':');
            Some((register.to_string(), rest.strip_suffix(')')?.to_string()))
        })
        .collect();
    assert_eq!(printed.len(), 8, "{detailed}");
    for (register, directive) in &printed {
        assert!(directive.starts_with("-- yavs cov exclude "), "{directive}");
        let case = format!("case {register} is");
        let src = MACHINES.replace(&case, &format!("{directive}: pasted\n{case}"));
        let said = fsm_run(&tmp, &src, &[]);
        assert!(!said.contains("W20"), "{directive}: {said}");
        let t = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--detailed"]).stdout);
        assert!(t.contains("excluded: pasted"), "{directive}: {t}");
        // Gone from the missed rows.
        assert!(!t.contains(&format!("(exclude with {directive})")), "{t}");
        let (items, ..) = fsm_totals(&tmp, &[]);
        let copies = if register == "one" { 2 } else { 1 };
        assert_eq!(items, FSM_ALL.0 - copies, "{directive}");
        // The exclusion report lists each instance with where the directive is.
        let e = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--exclusions"]).stdout);
        assert!(e.contains("m.vhd:") && e.contains("(in the source)"), "{e}");
        assert!(e.contains("reason:  pasted"), "{e}");
    }
}

#[test]
fn fsm_directive_before_a_when_and_the_forms() {
    let tmp = tempdir("fsm_forms");
    let src = MACHINES
        .replace(
            "        when spare =>",
            "        -- yavs cov exclude state spare: not used\n        \
             -- yavs coverage exclude transition spare->*\n        when spare =>",
        )
        .replace(
            "      when b2 =>",
            "      -- YAVS COV EXCLUDE STATE B2\n      when b2 =>",
        )
        .replace(
            "      when a2 =>",
            "      -- yavs cov exclude transition A2  ->  *: only named pair\n      when a2 =>",
        );
    let said = fsm_run(&tmp, &src, &[]);
    assert!(!said.contains("W20"), "{said}");
    assert_eq!(fsm_totals(&tmp, &[]), (28 - 6, 16, 12 - 6));
    let e = text(&yavs(&tmp, &["cov", "report", "a.ycov", "--exclusions"]).stdout);
    for rule in [
        "rule:    state spare",
        "rule:    transition spare -> *",
        "rule:    state B2",
        "rule:    transition A2 -> *",
    ] {
        assert!(e.contains(rule), "{rule}: {e}");
    }
    // Ignored on request; then a merge with a run that applies them is E0137.
    std::fs::rename(tmp.join("a.ycov"), tmp.join("applied.ycov")).expect("rename");
    let said = fsm_run(&tmp, &src, &["--coverage-directives=ignore"]);
    assert!(
        said.contains("4 `-- yavs coverage exclude` directives"),
        "{said}"
    );
    assert_eq!(fsm_totals(&tmp, &[]), FSM_ALL);
    let o = yavs(
        &tmp,
        &["cov", "merge", "applied.ycov", "a.ycov", "-o", "m.ycov"],
    );
    assert!(!o.status.success());
    let t = text(&o.stderr);
    assert!(
        t.contains("E0137") && t.contains("fsm one: spare of tb.u1"),
        "{t}"
    );
    // Equal databases merge and keep them.
    let o = yavs(
        &tmp,
        &[
            "cov",
            "merge",
            "applied.ycov",
            "applied.ycov",
            "-o",
            "a.ycov",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(fsm_totals(&tmp, &[]).0, 28 - 6);
}

/// Whether `src` analyses, and what analysis says.
fn fsm_check(tmp: &Path, name: &str, src: &str) -> (bool, String) {
    write(tmp, name, src);
    let lib = format!("--work=work:lib_{name}");
    let o = yavs(tmp, &["analyze", &lib, name]);
    (o.status.success(), text(&o.stderr))
}

#[test]
fn fsm_directive_checks() {
    let tmp = tempdir("fsm_checks");
    let before = |d: &str| MACHINES.replace("case wt is", &format!("{d}\n    case wt is"));

    // Names: did you mean, and the list.
    let (ok, t) = fsm_check(&tmp, "a.vhd", &before("-- yavs cov exclude state b3"));
    assert!(
        !ok && t.contains("E2022") && t.contains("`b3` is not a state of `wt_t`"),
        "{t}"
    );
    assert!(t.contains("the states are: b0, b1, b2"), "{t}");
    assert!(t.contains("similar names exist: `b0` (state)"), "{t}");
    let (ok, t) = fsm_check(
        &tmp,
        "b.vhd",
        &before("-- yavs cov exclude transition b0 -> b9"),
    );
    assert!(!ok && t.contains("E2022") && t.contains("`b9`"), "{t}");
    let (ok, t) = fsm_check(&tmp, "c.vhd", &before("-- yavs cov exclude state *"));
    assert!(!ok && t.contains("E2020"), "{t}");
    let (ok, t) = fsm_check(&tmp, "d.vhd", &before("-- yavs cov exclude stat b0"));
    assert!(!ok && t.contains("similar") && t.contains("`state`"), "{t}");

    // A valid pair no alternative names, a state to itself, and `*` with nothing to match.
    for d in [
        "transition b2 -> b1",
        "transition b1 -> b1",
        "transition b1->b2",
    ] {
        let (ok, t) = fsm_check(&tmp, "e.vhd", &before(&format!("-- yavs cov exclude {d}")));
        assert!(ok && t.contains("W2022"), "{d}: {t}");
    }
    let (ok, t) = fsm_check(
        &tmp,
        "f.vhd",
        &before("-- yavs cov exclude transition * -> b2"),
    );
    assert!(ok && t.contains("W2022"), "{t}");

    // Repeated, subsumed, everything.
    let two = before("-- yavs cov exclude state b1\n-- yavs cov exclude state b1");
    let (ok, t) = fsm_check(&tmp, "g.vhd", &two);
    assert!(ok && t.contains("W2020") && t.contains("repeats"), "{t}");
    let sub =
        before("-- yavs cov exclude transition b1 -> b0\n-- yavs cov exclude transition * -> b0");
    let (ok, t) = fsm_check(&tmp, "h.vhd", &sub);
    assert!(ok && t.contains("W2020") && t.contains("broader"), "{t}");
    let all = before("-- yavs cov exclude transition * -> *");
    let (ok, t) = fsm_check(&tmp, "i.vhd", &all);
    assert!(
        ok && t.contains("W2021") && t.contains("every transition"),
        "{t}"
    );
    let states = before(
        "-- yavs cov exclude state b0\n-- yavs cov exclude state b1\n-- yavs cov exclude state b2",
    );
    let (ok, t) = fsm_check(&tmp, "j.vhd", &states);
    assert!(
        ok && t.contains("W2021") && t.contains("every state"),
        "{t}"
    );

    // A `case` that is no state machine, and a `when` that is no alternative.
    let other = "entity e is port (clk, x : in bit; y : out bit); end entity;
architecture a of e is
  type t is (s0, s1);
  signal v, w : t;
begin
  process (clk) begin
    if rising_edge(clk) then
      -- yavs cov exclude state s0
      case x is
        when '0' => y <= '1';
        -- yavs cov exclude state s0
        when '1' => y <= '0';
      end case;
      -- yavs cov exclude state s0
      if x = '1' then y <= '0'; end if;
      y <= '1'
        -- yavs cov exclude state s0
        when x = '1' else '0';
    end if;
  end process;
end architecture;
";
    let (ok, t) = fsm_check(&tmp, "k.vhd", other);
    assert!(!ok, "{t}");
    assert_eq!(
        t.matches("this `case` is not on a detected state machine")
            .count(),
        2,
        "{t}"
    );
    assert_eq!(
        t.matches("needs a `case` on the state register").count(),
        2,
        "{t}"
    );
    assert!(t.contains("the `case` selects on this"), "{t}");

    // A repeat analysis with the cache gives the same.
    let (ok, again) = fsm_check(&tmp, "k.vhd", other);
    assert!(!ok && again.contains("E2024"), "{again}");
}

/// #1037: an exclusion file reached through a symlink, and a database holding a file reached
/// through one, still take the files their patterns name.
#[cfg(unix)]
#[test]
fn a_file_pattern_matches_through_a_symlink() {
    let tmp = database("symlink");
    std::os::unix::fs::symlink(&*tmp, tmp.join("alias")).expect("symlink");
    std::fs::create_dir_all(tmp.join("src")).expect("dir");
    std::fs::rename(tmp.join("cov_report.vhd"), tmp.join("src/cov_report.vhd")).expect("move");
    let lib = format!("--work=work:{}/lib2", tmp.display());
    let seen = |ex: &str, db: &str| {
        let o = yavs(&tmp, &["cov", "report", db, "--exclude", ex, "--uncovered"]);
        assert!(o.status.success(), "{}", text(&o.stderr));
        (text(&o.stdout), text(&o.stderr))
    };
    for (file, pattern) in [
        ("src/cov_report.vhd", "src/*.vhd"),
        ("alias/src/cov_report.vhd", "src/*.vhd"),
        ("alias/src/cov_report.vhd", "alias/src/cov_report.vhd"),
        ("src/cov_report.vhd", "alias/src/cov_report.vhd"),
        ("src/cov_report.vhd", "alias/src"),
    ] {
        assert!(yavs(&tmp, &["analyze", &lib, file]).status.success());
        let o = yavs(
            &tmp,
            &["run", &lib, "--cov=sbt", "--cov-file=b.ycov", "cov_report"],
        );
        assert!(o.status.success(), "{}", text(&o.stderr));
        write(
            &tmp,
            "ex.yaml",
            &format!("exclude:\n  - kind: file\n    files: \"{pattern}\"\n"),
        );
        // Through the plain name of the exclusion file and through the symlink.
        for ex in ["ex.yaml", "alias/ex.yaml"] {
            let (rows, warnings) = seen(ex, "b.ycov");
            assert_eq!(rows, "", "{file} by {pattern} in {ex}");
            assert!(
                !warnings.contains("W0111"),
                "{file} by {pattern} in {ex}: {warnings}"
            );
        }
    }
}
