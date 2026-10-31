//! The console's tools on a design with real unknown values: `drivers`, `describe`, `find`,
//! `waits`, `trace x`, `examine NAME @ TIME`, `break assert`, `asserts`, `activity`; a misspelling
//! of each ends in a suggestion. `tests/cli/debug_tools.vhd`.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use common::tempdir;

fn yavs() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yavs"));
    c.env("YAVS_STD_CACHE", "off");
    c
}

fn setup(name: &str) -> common::Tmp {
    let d = tempdir(&format!("tools-{name}"));
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cli/debug_tools.vhd");
    std::fs::copy(src, d.join("debug_tools.vhd")).expect("copy");
    let ok = yavs()
        .current_dir(&d)
        .args([
            "analyze",
            "--std=2008",
            "--work=work:lib",
            "debug_tools.vhd",
        ])
        .status()
        .expect("yavs");
    assert!(ok.success());
    d
}

/// `--do` with `flags`; (stdout, stderr).
fn session(d: &Path, flags: &[&str], cmds: &str) -> (String, String, Output) {
    let o = yavs()
        .current_dir(d)
        .args(["run", "--work=work:lib", "debug_tools"])
        .args(flags)
        .arg(format!("--do={cmds}"))
        .stdin(Stdio::null())
        .output()
        .expect("yavs");
    let (a, b) = (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    );
    (a, b, o)
}

fn fails_with(d: &Path, flags: &[&str], cmd: &str, wanted: &str) {
    let (_, err, o) = session(d, flags, cmd);
    assert_eq!(o.status.code(), Some(1), "{cmd}: {err}");
    assert!(err.contains(wanted), "{cmd}: wanted `{wanted}` in {err}");
}

#[test]
fn drivers_shows_each_driver_the_resolved_value_and_the_lines() {
    let d = setup("drivers");
    let (out, err, _) = session(
        &d,
        &[],
        "run 30 ns; drivers bus_s; drivers q; drivers never; quit",
    );
    assert!(
        out.contains("debug_tools.bus_s : STD_LOGIC_VECTOR = \"01X1\""),
        "{out}{err}"
    );
    assert!(out.contains("resolved by resolved (per element)"), "{out}");
    for line in [
        "\"0101\"  debug_tools.vhd:28",
        "(1 downto 0)",
        "\"1Z\"    debug_tools.vhd:29",
    ] {
        assert!(out.contains(line), "{line} in {out}");
    }
    assert!(out.contains("port debug_tools.u1.q"), "{out}");
    assert!(
        out.contains("no driver: it keeps its initial value"),
        "{out}"
    );
}

#[test]
fn drivers_shows_a_force_and_a_deposit() {
    let d = setup("force");
    let (out, err, _) = session(
        &d,
        &[],
        "run 30 ns; force never '1'; deposit a '1'; run 1 ns; drivers never; drivers a; quit",
    );
    assert!(out.contains("force: '1'"), "{out}{err}");
    assert!(out.contains("a deposit at 30 ns set it"), "{out}");
}

#[test]
fn describe_gives_type_bounds_resolution_mode_and_declaration() {
    let d = setup("describe");
    let (out, err, _) = session(
        &d,
        &[],
        "run 30 ns; describe bus_s; describe :debug_tools:u1:d; describe st; quit",
    );
    for want in [
        "type        STD_LOGIC_VECTOR",
        "bounds      3 downto 0 (4 elements)",
        "resolution  resolved (per element)",
        "declared    debug_tools.vhd:17",
        "kind        port, mode in",
        "bounds      idle to busy (2 literals)",
        "resolution  none",
    ] {
        assert!(out.contains(want), "{want} in {out}{err}");
    }
}

#[test]
fn find_lists_what_wave_add_and_examine_take() {
    let d = setup("find");
    let (out, err, _) = session(
        &d,
        &[],
        "run 30 ns; find signals *cn*; find signals u1.*; find variables tot*; examine debug_tools.cnt; quit",
    );
    assert!(out.contains("debug_tools.cnt"), "{out}{err}");
    assert!(out.contains("debug_tools.u1.clk  port, mode in"), "{out}");
    assert!(out.contains("debug_tools.counter.total"), "{out}");
    assert!(out.contains("\"0011\""), "{out}");
}

#[test]
fn waits_says_where_each_process_stands() {
    let d = setup("waits");
    let (out, err, _) = session(&d, &[], "run 30 ns; waits; quit");
    let row = out
        .lines()
        .find(|l| l.contains("debug_tools.check"))
        .expect(&out);
    assert!(
        row.contains("waiting") && row.contains("debug_tools.cnt"),
        "{row}"
    );
    assert!(row.contains("debug_tools.vhd:44"), "{row} {err}");
    assert!(out.contains("nothing: `wait;`"), "{out}");
}

#[test]
fn hang_report_of_a_session_comes_with_the_option_only() {
    let d = setup("hang");
    let flags = ["--stop-time=30ns", "--hang-report"];
    let (_, err, _) = session(&d, &flags, "run; quit");
    assert!(
        err.contains("W3101") || err.contains("reached --stop-time"),
        "{err}"
    );
    let (_, err, _) = session(&d, &["--stop-time=30ns"], "run; quit");
    assert!(!err.contains("reached --stop-time"), "{err}");
}

#[test]
fn trace_x_follows_a_real_unknown_to_where_it_starts() {
    let d = setup("trace");
    let (out, err, _) = session(&d, &["--wave=t.fst"], "run 30 ns; trace x q; quit");
    let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("{s} in {out}{err}"));
    // Through the port, the flip-flop and its input port, in that order.
    let chain = [
        "debug_tools.q = 'U', since 0 fs",
        "debug_tools.u1.q = 'U'",
        "by process debug_tools.u1.reg, at debug_tools.vhd:7",
        "debug_tools.u1.d = 'U'",
        "debug_tools.never = 'U'",
        "Where it starts:",
        "debug_tools.never: nothing drives it",
    ];
    assert!(chain.windows(2).all(|w| at(w[0]) < at(w[1])), "{out}");
    assert!(out.contains("It cannot see variables"), "{out}");
    // Two drivers that disagree are the start, with no wave file too.
    let (out, _, _) = session(&d, &[], "run 30 ns; trace x bus_s; trace x cnt; quit");
    assert!(
        out.contains(
            "element (1): process debug_tools._p2 drives '0' and process debug_tools._p3 drives '1'"
        ),
        "{out}"
    );
    assert!(out.contains("all known, nothing to trace"), "{out}");
}

#[test]
fn examine_at_a_time_reads_the_wave_file() {
    let d = setup("examine");
    let (out, err, _) = session(
        &d,
        &["--wave=t.fst"],
        "run 30 ns; examine cnt @ 15 ns; examine st @ 10 ns; examine num @ 20 ns; examine rc @ 15 ns; quit",
    );
    assert_eq!(
        out.lines()
            .filter(|l| !l.starts_with("**") && !l.starts_with("    Process"))
            .collect::<Vec<_>>(),
        [
            "\"0010\"",
            "idle",
            "2",
            "  debug_tools.rc.f  '0'",
            "  debug_tools.rc.g  3"
        ],
        "{err}"
    );
    fails_with(
        &d,
        &["--wave=t.fst"],
        "run 30 ns; examine cnt @ 99 ns",
        "still to come",
    );
    fails_with(
        &d,
        &[],
        "run 30 ns; examine cnt @ 10 ns",
        "run with `--wave=FILE.fst`",
    );
    fails_with(
        &d,
        &["--wave=t.fst", "--wave-exclude=**.num"],
        "run 30 ns; examine num @ 10 ns",
        "not recorded",
    );
    fails_with(
        &d,
        &["--wave=t.fst"],
        "run 30 ns; examine debug_tools.cn @ 10 ns",
        "`debug_tools.cnt`",
    );
    fails_with(
        &d,
        &["--wave=t.fst"],
        "run 30 ns; examine cnt @ soon",
        "is not a time",
    );
}

#[test]
fn break_assert_stops_at_the_report_with_the_stack() {
    let d = setup("break");
    let (out, err, _) = session(
        &d,
        &[],
        "break assert error; run; where; examine cnt; break list; run; quit",
    );
    assert!(
        err.contains("assertion of severity error at 25 ns"),
        "{err}"
    );
    assert!(err.contains("debug_tools.vhd:43: cnt reached 3"), "{err}");
    assert!(out.contains(":debug_tools:check"), "{out}");
    assert!(out.contains("assert: severity error, failure"), "{out}");
    // The warning is below it: the run goes on to the end.
    assert!(
        err.contains("the simulation ended") || out.contains("fatal now"),
        "{out}{err}"
    );
    let (_, err, _) = session(
        &d,
        &[],
        "break assert warning; run; run; break delete assert; run; quit",
    );
    assert_eq!(err.matches("assertion of severity").count(), 2, "{err}");
    let (_, err, _) = session(&d, &[], "break assert note; run; quit");
    assert!(err.contains("assertion of severity note"), "{err}");
}

#[test]
fn asserts_and_activity_at_the_console_and_at_the_end() {
    let d = setup("tables");
    let (out, err, _) = session(&d, &[], "run 30 ns; asserts; activity; quit");
    assert!(
        out.contains("Assertion summary: 0 failures, 1 error, 0 warnings, 1 note"),
        "{out}{err}"
    );
    assert!(out.contains("debug_tools.vhd:43  cnt reached 3"), "{out}");
    assert!(out.contains("Process activations:"), "{out}");
    assert!(out.contains(":debug_tools:clk"), "{out}");
    let (out, ..) = session(&d, &["--assert-summary", "--activity"], "run; quit");
    assert!(
        out.contains("Assertion summary:") && out.contains("Signal activity:"),
        "{out}"
    );
}

#[test]
fn every_misspelling_suggests() {
    let d = setup("misspell");
    let q = "run 1 ns; ";
    for (cmd, wanted) in [
        ("drivrs cnt", "`drivers`"),
        ("describ cnt", "`describe`"),
        ("fnd signals cnt", "`find`"),
        ("wait", "`waits`"),
        ("trce x cnt", "`trace`"),
        ("activty", "`activity`"),
        ("assrts", "`asserts`"),
        ("drivers cn", "`cnt`"),
        ("describe cn", "`cnt`"),
        ("trace x cn", "`cnt`"),
        ("find sigs x", "`signals`"),
        ("find signals cn", "`debug_tools.cnt`"),
        ("find variables totl", "`debug_tools.counter.total`"),
        ("trace y cnt", "`x`"),
        ("break assert eror", "`error`"),
        ("break delete asert", "`assert`"),
        ("wave sav", "`--surfer`"),
    ] {
        fails_with(&d, &[], &format!("{q}{cmd}"), wanted);
    }
    fails_with(&d, &[], "vunit_run", "--yavs-interactive");
}

#[test]
fn help_lists_every_new_command() {
    let d = setup("help");
    let (out, ..) = session(&d, &[], "help; quit");
    for c in [
        "drivers NAME",
        "describe NAME",
        "find signals",
        "waits",
        "trace x NAME",
        "activity",
        "asserts",
        "@ TIME",
        "break",
        "wave save",
    ] {
        assert!(out.contains(c), "{c} in {out}");
    }
}
