//! Rule 35 outside the debugger: a name the user types and yavs rejects gets a "did you mean"
//! from the shared helper (`yavs_diag::suggest`), in plain words.

mod common;
use common::tempdir;

use std::path::Path;
use std::process::Command;

const DESIGN: &str = "\
entity counter is
  generic (WIDTH : natural := 4; FLAG : boolean := false; DLY : time := 1 ns);
end;
architecture rtl of counter is begin end;
";

/// Runs `yavs ARGS` in `dir`; the exit code and standard error.
fn yavs(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["--color", "never"])
        .args(args)
        .current_dir(dir)
        .env("YAVS_STD_CACHE", dir.join("std"))
        .output()
        .expect("run");
    (
        o.status.code().expect("exit code"),
        String::from_utf8(o.stderr).expect("utf-8"),
    )
}

/// A scratch directory with `counter` analyzed into library `work`.
fn setup(name: &str) -> common::Tmp {
    let t = tempdir(name);
    std::fs::write(t.join("counter.vhd"), DESIGN).unwrap();
    let (code, err) = yavs(&t, &["analyze", "--work=work:lib", "counter.vhd"]);
    assert_eq!(code, 0, "{err}");
    t
}

/// `args` fail with `code` and an error that says `needle`.
fn says(t: &Path, args: &[&str], code: i32, needle: &str) {
    let (c, err) = yavs(t, args);
    assert_eq!(c, code, "{args:?}: {err}");
    assert!(err.contains(needle), "{args:?}: want `{needle}` in\n{err}");
}

#[test]
fn clap_errors_use_the_shared_helper() {
    let t = setup("names_clap");
    // An option, a subcommand, a nested subcommand and the value of an option.
    says(
        &t,
        &["run", "--stop-tim=1ns", "counter"],
        2,
        "an option with a similar name exists: `--stop-time`",
    );
    says(
        &t,
        &["rn", "counter"],
        2,
        "a subcommand with a similar name exists: `run`",
    );
    says(
        &t,
        &["cov", "reprt", "x"],
        2,
        "a subcommand with a similar name exists: `report`",
    );
    says(
        &t,
        &["--color=alwys", "run"],
        2,
        "a value with a similar name exists: `always`",
    );
    says(
        &t,
        &["cov", "report", "--kind=statment", "x"],
        2,
        "a value with a similar name exists: `statement`",
    );
    // Far from every name: no suggestion, and not clap's own either.
    let (_, err) = yavs(&t, &["run", "--zzzzzzzz", "counter"]);
    assert!(!err.contains("similar"), "{err}");
}

#[test]
fn explain_codes_and_rules() {
    let t = setup("names_explain");
    says(
        &t,
        &["--explain", "E1234x"],
        2,
        "a diagnostic code with a similar name exists: `E1234`",
    );
    says(
        &t,
        &["--explain", "lint_0011"],
        2,
        "a lint rule with a similar name exists: `lint_001`",
    );
    says(
        &t,
        &["lint", "--explain", "lint_0011"],
        2,
        "a lint rule with a similar name exists: `lint_001`",
    );
    // A well-formed code that does not exist is no typo of its neighbours.
    let (code, err) = yavs(&t, &["--explain", "E0999"]);
    assert_eq!(code, 2);
    assert!(!err.contains("similar"), "{err}");
}

#[test]
fn warning_options() {
    let t = setup("names_warn");
    says(
        &t,
        &["analyze", "--work=work:lib", "-Werrr", "counter.vhd"],
        2,
        "a warning option with a similar name exists: `error`",
    );
    says(
        &t,
        &["lint", "--work=work:lib", "-Wno-lint_0011", "counter.vhd"],
        2,
        "a lint rule with a similar name exists: `lint_001`",
    );
}

#[test]
fn library_unit_generic_and_value_names() {
    let t = setup("names_design");
    // A library the top level's closure looks up.
    says(
        &t,
        &["elab", "--work=wrk:lib", "counter"],
        2,
        "a library with a similar name exists: `wrk`",
    );
    // The top level and its architecture.
    says(
        &t,
        &["elab", "--work=work:lib", "countr"],
        2,
        "an entity with a similar name exists: `counter`",
    );
    says(
        &t,
        &["elab", "--work=work:lib", "counter.rtll"],
        2,
        "an architecture with a similar name exists: `rtl`",
    );
    // A generic that is not there, an enumeration literal and a time unit that are not.
    says(
        &t,
        &["elab", "--work=work:lib", "counter", "-gWIDT=3"],
        0,
        "a generic with a similar name exists: `WIDTH`",
    );
    says(
        &t,
        &["elab", "--work=work:lib", "counter", "-gFLAG=ture"],
        2,
        "a literal with a similar name exists: `TRUE`",
    );
    says(
        &t,
        &["elab", "--work=work:lib", "counter", "-gDLY=3nss"],
        2,
        "a unit with a similar name exists: `ns`",
    );
    says(
        &t,
        &["run", "--work=work:lib", "counter", "--stop-time=10nss"],
        2,
        "a time unit with a similar name exists: `ns`",
    );
}

#[test]
fn print_hierarchy_says_what_an_object_is() {
    let t = tempdir("names_hier");
    std::fs::write(
        t.join("h.vhd"),
        "entity h is end; architecture a of h is signal s : bit; begin end;",
    )
    .unwrap();
    assert_eq!(yavs(&t, &["analyze", "--work=work:lib", "h.vhd"]).0, 0);
    let o = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["elab", "--work=work:lib", "--print-hierarchy", "h"])
        .current_dir(&*t)
        .env("YAVS_STD_CACHE", t.join("std"))
        .output()
        .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.contains("s (signal)"), "{out}");
}
