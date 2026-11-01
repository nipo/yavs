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
            "--cov=a.ycov",
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
    let o = yavs(&tmp, &["run", &lib, "--cov=a.ycov", "cov_report"]);
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
            "--cov=b.ycov",
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
        "--cov=a.ycov",
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
    let table = out.split("path ").nth(1).expect("table");
    let names = table
        .lines()
        .skip(1)
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
            &format!("--cov={name}.ycov"),
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

#[test]
fn a_positional_directive_warns_when_its_inputs_change_under_it() {
    let tmp = tempdir("positional");
    assert!(
        decision(&tmp, "a", "x > 3 or r = '1'", "01")
            .status
            .success()
    );
    // Another formatting of the same operands: the same meaning, no warning.
    assert!(decision(&tmp, "b", "x>3 or R='1'", "01").status.success());
    let o = yavs(
        &tmp,
        &["cov", "merge", "a.ycov", "b.ycov", "-o", "same.ycov"],
    );
    assert!(
        o.status.success() && text(&o.stderr).is_empty(),
        "{}",
        text(&o.stderr)
    );
    // The operands swapped under the directive.
    assert!(
        decision(&tmp, "c", "r = '1' or x > 3", "01")
            .status
            .success()
    );
    let o = yavs(&tmp, &["cov", "merge", "a.ycov", "c.ycov", "-o", "m.ycov"]);
    let e = text(&o.stderr);
    assert!(o.status.success(), "a warning, not an error: {e}");
    assert!(e.contains("warning[W0119]") && e.contains("d.vhd:7"), "{e}");
    assert!(
        e.contains("x > 3, r = '1'") && e.contains("r = '1', x > 3"),
        "{e}"
    );
    // It stays in the merged file, and follows -W.
    let o = yavs(&tmp, &["cov", "report", "m.ycov", "--total"]);
    assert!(text(&o.stderr).contains("W0119"), "{}", text(&o.stderr));
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "c.ycov", "-Wno-W0119", "--total"],
    );
    assert!(!text(&o.stderr).contains("W0119"), "{}", text(&o.stderr));
    let o = yavs(
        &tmp,
        &["cov", "report", "a.ycov", "c.ycov", "-Werror", "--total"],
    );
    assert_eq!(o.status.code(), Some(1));
    // One run alone has nothing to compare; the named form says what it means.
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--total"]);
    assert!(text(&o.stderr).is_empty(), "{}", text(&o.stderr));
    assert!(
        decision(&tmp, "e", "x > 3 or r = '1'", "(r = '1' = true)")
            .status
            .success()
    );
    assert!(
        decision(&tmp, "f", "r = '1' or x > 3", "(r = '1' = true)")
            .status
            .success()
    );
    let o = yavs(&tmp, &["cov", "report", "e.ycov", "f.ycov", "--total"]);
    assert!(!text(&o.stderr).contains("W0119"), "{}", text(&o.stderr));
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
            "--cov=b.ycov",
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
            .find(|l| l.starts_with("conditions"))
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(
        total("p.ycov")
            .split_whitespace()
            .nth(1)
            .map(|t| t.split('/').nth(1).map(str::to_string)),
        Some(Some("4".to_string()))
    );
    assert_eq!(
        total("w.ycov")
            .split_whitespace()
            .nth(1)
            .map(|t| t.split('/').nth(1).map(str::to_string)),
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

/// E0137 and W0119 hold for rows as for combinations.
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
            "--cov=b.ycov",
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
    // Reordered under a positional row.
    assert!(
        decision_of(&tmp, "c", "r = '1' or x > 3", "row 0-", "condition")
            .status
            .success()
    );
    let o = yavs(&tmp, &["cov", "merge", "a.ycov", "c.ycov", "-o", "m.ycov"]);
    assert!(
        text(&o.stderr).contains("warning[W0119]"),
        "{}",
        text(&o.stderr)
    );
    // Not under one named by its text.
    assert!(
        decision_of(&tmp, "e", "x > 3 or r = '1'", "row x > 3 = 0", "condition")
            .status
            .success()
    );
    assert!(
        decision_of(&tmp, "f", "r = '1' or x > 3", "row x > 3 = 0", "condition")
            .status
            .success()
    );
    let o = yavs(&tmp, &["cov", "merge", "e.ycov", "f.ycov", "-o", "m.ycov"]);
    assert!(!text(&o.stderr).contains("W0119"), "{}", text(&o.stderr));
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
            "--cov=b.ycov",
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
                "--cov=a.ycov",
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
