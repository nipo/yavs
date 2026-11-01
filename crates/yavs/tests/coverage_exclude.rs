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
