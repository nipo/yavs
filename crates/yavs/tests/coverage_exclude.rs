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
    assert!(
        e.contains("error[E0112]") && e.contains("d/nope.txt"),
        "{e}"
    );
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
