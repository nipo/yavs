//! What coverage measures beyond statements: the conditions of `wait until` and `assert`
//! (`tests/cli/cov_wait.vhd`, #1020), that counting them changes nothing the design does, and
//! state machines (`tests/cli/cov_fsm.vhd`).

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

fn read(p: &Path) -> Database {
    Database::parse(&std::fs::read_to_string(p).expect("read")).expect("parse")
}

/// A fresh directory with `tests/cli/FILE` analysed into `lib`.
fn setup(name: &str, file: &str) -> Tmp {
    let tmp = tempdir(name);
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cli")
        .join(file);
    let o = yavs(
        &tmp,
        &["analyze", "--work=work:lib", src.to_str().expect("utf-8")],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    tmp
}

/// The tiers of this build, the compiled ones at one and four threads.
fn tiers() -> Vec<Vec<&'static str>> {
    #[allow(unused_mut)]
    let mut v = vec![vec!["--tier=interp"], vec!["--tier=interp", "--threads=4"]];
    #[cfg(feature = "clift")]
    v.extend([
        vec!["--tier=clift"],
        vec!["--tier=clift", "--threads=4"],
        vec!["--tier=clift-auto", "--tier-heat=1,0,0", "--threads=4"],
    ]);
    #[cfg(feature = "llvm")]
    v.extend([
        vec!["--tier=llvm"],
        vec!["--tier=llvm", "--threads=4"],
        vec!["--tier=auto", "--tier-heat=1,0,0", "--threads=4"],
    ]);
    v
}

/// The notes of a run, each with its time: what the design did.
fn trace(o: &Output) -> Vec<String> {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let (out, err) = (
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr),
    );
    out.lines()
        .chain(err.lines())
        .filter(|l| l.contains("Note") || l.contains("Error"))
        .map(str::to_string)
        .collect()
}

/// An `and` of a function and an edge test is evaluated at every event of its signals, whatever
/// the tier: the function reports and counts each evaluation (#1022).
#[test]
fn wait_until_evaluates_at_every_event() {
    let tmp = setup("meas_wait_events", "cov_wait.vhd");
    for flags in tiers() {
        let o = yavs(
            &tmp,
            &[&["run", "--work=work:lib", "cov_wait"][..], &flags].concat(),
        );
        let t = trace(&o);
        let calls = t.iter().filter(|l| l.contains("impure call")).count();
        assert_eq!(calls, 6, "{flags:?}: {t:#?}");
        assert!(t.iter().any(|l| l.contains("calls=6")), "{flags:?}: {t:#?}");
    }
}

/// The condition rows of a database: (line, row, input, count), sorted.
fn rows(db: &Database) -> Vec<(u32, u32, String, u64)> {
    let mut v: Vec<_> = db
        .items
        .iter()
        .filter_map(|(k, e)| match k.kind {
            Kind::Condition(r) => Some((e.line, r, k.sig.clone(), e.count)),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

/// `wait until` is under `condition`, `assert` apart; both give every tier the same rows, and
/// counting changes no note of the run: no evaluation more, no wake at another time.
#[test]
fn wait_and_assert_conditions() {
    let tmp = setup("meas_conditions", "cov_wait.vhd");
    let run = |flags: &[&str]| {
        yavs(
            &tmp,
            &[&["run", "--work=work:lib", "cov_wait"][..], flags].concat(),
        )
    };
    let plain = trace(&run(&[]));
    let mut want = None;
    for flags in tiers() {
        let args = [
            &["--cov=a.ycov", "--cov-kind=condition,assert"][..],
            &flags[..],
        ]
        .concat();
        assert_eq!(trace(&run(&args)), plain, "{flags:?}");
        let got = rows(&read(&tmp.join("a.ycov")));
        match &want {
            None => want = Some(got),
            Some(w) => assert_eq!(&got, w, "{flags:?}"),
        }
    }
    let all = want.expect("rows");
    // Lines of tests/cli/cov_wait.vhd.
    let (edge_en, level, timed, conc, seq) = (48, 55, 57, 64, 67);
    let at = |l: u32| -> Vec<(u32, String, u64)> {
        all.iter()
            .filter(|r| r.0 == l)
            .map(|r| (r.1, r.2.clone(), r.3))
            .collect()
    };
    // The edge is what resumes the wait: no rows. `en` is 0 at 10 ns and 1 at 30 ns, `ok` not
    // evaluated at 10 ns.
    assert_eq!(
        at(edge_en),
        [
            (2, "en = '1'".to_string(), 1),
            (3, "en = '1'".to_string(), 1),
            (4, "ok = '1'".to_string(), 0),
            (5, "ok = '1'".to_string(), 1),
        ],
        "{all:#?}"
    );
    assert!(at(level).iter().any(|r| r.2 > 0), "{all:#?}");
    // The wait with a timeout never held, and its false evaluations still count.
    assert!(at(timed).iter().all(|r| r.0 != 2 || r.2 == 0), "{all:#?}");
    // An assertion that never fails leaves the rows of its false result at 0.
    assert!(
        at(conc).iter().any(|r| r.2 > 0) && at(conc).iter().any(|r| r.2 == 0),
        "{all:#?}"
    );
    assert!(
        at(seq).iter().any(|r| r.2 > 0) && at(seq).iter().any(|r| r.2 == 0),
        "{all:#?}"
    );

    // Without the option there are no rows of an `assert`, with it alone no rows of a wait.
    let o = run(&["--cov=w.ycov", "--cov-kind=condition"]);
    assert_eq!(trace(&o), plain);
    let wait_only = rows(&read(&tmp.join("w.ycov")));
    assert!(wait_only.iter().any(|r| r.0 == edge_en));
    assert!(
        !wait_only.iter().any(|r| r.0 == conc || r.0 == seq),
        "{wait_only:#?}"
    );
    let o = run(&["--cov=s.ycov", "--cov-kind=assert"]);
    assert_eq!(trace(&o), plain);
    let assert_only = rows(&read(&tmp.join("s.ycov")));
    assert!(assert_only.iter().any(|r| r.0 == conc));
    assert!(
        !assert_only.iter().any(|r| r.0 == edge_en),
        "{assert_only:#?}"
    );
    // A combination is not measured at either.
    let o = run(&["--cov=m.ycov", "--cov-kind=combination"]);
    assert_eq!(trace(&o), plain);
    assert!(
        !read(&tmp.join("m.ycov"))
            .items
            .keys()
            .any(|k| matches!(k.kind, Kind::Combination(_))),
    );
}

/// A false assertion is a row an assertion that holds cannot reach: a failing run hits it and the
/// merged database shows it. A misspelt kind is refused with a suggestion.
#[test]
fn a_failing_assertion_hits_its_row_and_a_bad_kind_is_refused() {
    let tmp = tempdir("meas_fail");
    std::fs::write(
        tmp.join("f.vhd"),
        "entity f is end;\narchitecture a of f is\n  signal x, y : bit := '0';\nbegin\n  y <= '1' after 1 ns;\n  assert x = '1' or y = '0' report \"held\" severity note;\nend;\n",
    )
    .expect("write");
    assert!(
        yavs(&tmp, &["analyze", "--work=work:lib", "f.vhd"])
            .status
            .success()
    );
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "f",
            "--cov=a.ycov",
            "--cov-kind=assert",
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let r = rows(&read(&tmp.join("a.ycov")));
    assert_eq!(
        r.iter().map(|x| (x.1, x.3)).collect::<Vec<_>>(),
        [(0, 1), (1, 0), (2, 1), (3, 1)],
        "{r:#?}"
    );
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "f",
            "--cov=b.ycov",
            "--cov-kind=asert",
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("assert"));
    assert!(!tmp.join("b.ycov").exists());
}

/// The state machine items of a database: (path, item, count), sorted.
fn fsm_items(db: &Database) -> Vec<(String, String, u64)> {
    let mut v: Vec<_> = db
        .items
        .iter()
        .filter(|(k, _)| matches!(k.kind, Kind::Fsm(_)))
        .map(|(k, e)| (k.path.clone(), k.sig.clone(), e.count))
        .collect();
    v.sort();
    v
}

fn fsm_setup(name: &str) -> Tmp {
    setup(name, "cov_fsm.vhd")
}

fn count(items: &[(String, String, u64)], path: &str, sig: &str) -> u64 {
    items
        .iter()
        .find(|i| i.0 == path && i.1 == sig)
        .unwrap_or_else(|| panic!("no {path} {sig} in {items:#?}"))
        .2
}

/// Every form of machine counts per instance, the same in every tier and at any thread count, and
/// a run with the kind prints what a run without it prints.
#[test]
fn fsm_states_and_transitions() {
    let tmp = fsm_setup("meas_fsm");
    let run = |flags: &[&str]| {
        yavs(
            &tmp,
            &[&["run", "--work=work:lib", "cov_fsm"][..], flags].concat(),
        )
    };
    let plain = run(&[]);
    assert!(plain.status.success());
    let mut want = None;
    for flags in tiers() {
        let args = [&["--cov=a.ycov", "--cov-kind=fsm"][..], &flags[..]].concat();
        let o = run(&args);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(o.stdout, plain.stdout, "{flags:?}");
        let got = fsm_items(&read(&tmp.join("a.ycov")));
        match &want {
            None => want = Some(got),
            Some(w) => assert_eq!(&got, w, "{flags:?}"),
        }
    }
    let got = want.expect("items");
    let (far, near) = (":cov_fsm:u_far", ":cov_fsm:u_near");
    // One process: per instance, the initial value counts as reached.
    assert_eq!(count(&got, far, "one:busy->done"), 1);
    assert_eq!(count(&got, far, "one:busy->idle"), 0);
    assert_eq!(count(&got, near, "one:busy->done"), 0);
    assert!(count(&got, near, "one:busy->idle") > 0);
    assert_eq!(count(&got, far, "one:spare"), 0);
    assert_eq!(count(&got, far, "pk:p_idle"), 1);
    // Two processes, `when others` included; the next state is no machine of its own.
    assert_eq!(count(&got, far, "two:a2->a0"), 1);
    assert!(!got.iter().any(|i| i.1.starts_with("two_next")), "{got:#?}");
    // A transition no `case` names comes in when it happens; a self-loop is never an item.
    assert_eq!(count(&got, far, "three:r2->r0"), 1);
    assert!(!got.iter().any(|i| i.1 == "three:r2->r2"), "{got:#?}");
    // One machine per generate iteration.
    assert_eq!(count(&got, ":cov_fsm:u_far:gen(0)", "g:i0->i1"), 0);
    assert_eq!(count(&got, ":cov_fsm:u_far:gen(1)", "g:i0->i1"), 1);
    // Not machines: nothing clocks `mode`; `copy` does not read itself.
    assert!(
        !got.iter()
            .any(|i| i.1.starts_with("mode") || i.1.starts_with("copy")),
        "{got:#?}"
    );
}

/// What looks like a machine and is not measured is a warning and a section of the report, not a
/// silent gap; plain runs say nothing.
#[test]
fn fsm_not_detected_is_visible() {
    let tmp = fsm_setup("meas_fsm_gaps");
    let o = yavs(&tmp, &["run", "--work=work:lib", "cov_fsm"]);
    assert!(!String::from_utf8_lossy(&o.stderr).contains("W0120"));
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "cov_fsm",
            "--cov=a.ycov",
            "--cov-kind=fsm",
        ],
    );
    let err = String::from_utf8_lossy(&o.stderr);
    for what in [
        "`by_if`: no `case` selects on it",
        "`v`: it is a variable",
        "`r.st`: it is a part of a signal",
        "`held`: it is a port",
    ] {
        assert!(err.contains(what), "{what}: {err}");
    }
    assert!(err.contains("warning[W0120]"), "{err}");
    assert!(!err.contains("`mode`") && !err.contains("`copy`"), "{err}");
    let o = yavs(&tmp, &["cov", "report", "a.ycov"]);
    let rep = String::from_utf8_lossy(&o.stdout);
    assert!(rep.contains("not measured as state machines:"), "{rep}");
    assert!(rep.contains("`r.st`: it is a part of a signal"), "{rep}");
    assert!(rep.contains("fsm  one: busy -> idle"), "{rep}");
    // A kind other than `fsm` does not show them.
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--kind=toggle"]);
    assert!(!String::from_utf8_lossy(&o.stdout).contains("not measured"));
    // A merge keeps them.
    assert!(
        yavs(&tmp, &["cov", "merge", "a.ycov", "a.ycov", "-o", "m.ycov"])
            .status
            .success()
    );
    assert_eq!(read(&tmp.join("m.ycov")).unmeasured.len(), 4);
}

/// Text and YAML rows from `--uncovered` take the missed items out; globs work; a malformed row
/// and an unknown key are E0112; a row that matches nothing is W0111; a row of an item that ran is
/// W0114. Merge adds the counts and `rank` counts the items.
#[test]
fn fsm_exclusions_merge_and_rank() {
    let tmp = fsm_setup("meas_fsm_excl");
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "cov_fsm",
            "--cov=a.ycov",
            "--cov-kind=fsm",
            "--cov-test=a",
        ],
    );
    assert!(o.status.success());
    let o = yavs(
        &tmp,
        &[
            "run",
            "--work=work:lib",
            "cov_fsm",
            "--cov=b.ycov",
            "--cov-kind=fsm",
            "--cov-test=b",
            "-gruns=9",
        ],
    );
    assert!(o.status.success());
    let (a, b) = (read(&tmp.join("a.ycov")), read(&tmp.join("b.ycov")));
    let o = yavs(&tmp, &["cov", "merge", "a.ycov", "b.ycov", "-o", "m.ycov"]);
    assert!(o.status.success());
    let m = read(&tmp.join("m.ycov"));
    for (k, e) in &m.items {
        assert_eq!(e.count, a.items[k].count + b.items[k].count, "{k:?}");
    }
    let o = yavs(&tmp, &["cov", "rank", "m.ycov"]);
    let rank = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success() && rank.contains('a') && rank.contains("items"),
        "{rank}"
    );

    let report = |args: &[&str]| {
        let o = yavs(&tmp, &[&["cov", "report", "m.ycov"][..], args].concat());
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let (_, rows, _) = report(&["--uncovered", "--kind=fsm"]);
    assert!(rows.contains("fsm cov_fsm.u_far one spare"), "{rows}");
    assert!(rows.contains("fsm cov_fsm.u_far one spare->idle"), "{rows}");
    // The rows back: nothing is left.
    std::fs::write(tmp.join("x.txt"), &rows).expect("write");
    let (code, rep, _) = report(&["--exclude", "x.txt", "--kind=fsm", "--fail-under=100"]);
    assert_eq!(code, Some(0), "{rep}");
    let (_, yaml, _) = report(&["--uncovered=yaml", "--kind=fsm"]);
    std::fs::write(tmp.join("x.yaml"), &yaml).expect("write");
    let (code, rep, err) = report(&["--exclude", "x.yaml", "--kind=fsm", "--fail-under=100"]);
    assert_eq!(code, Some(0), "{rep}{err}\n{yaml}");

    // Globs, a row that ran, a row that matches nothing.
    std::fs::write(
        tmp.join("g.txt"),
        "fsm *.u_far one spare*\nfsm cov_fsm.u_far pk p_run\nfsm cov_fsm.u_far nope *\n",
    )
    .expect("write");
    let (_, rep, err) = report(&["--exclude", "g.txt"]);
    assert!(err.contains("W0114"), "{err}");
    assert!(err.contains("W0111"), "{err}");
    assert!(
        !rep.contains("one: spare -> idle                 cov_fsm.u_far"),
        "{rep}"
    );

    // Failure: too few fields, and a YAML entry with no item or a bad key.
    std::fs::write(tmp.join("bad.txt"), "fsm cov_fsm.u_far one\n").expect("write");
    let (code, _, err) = report(&["--exclude", "bad.txt"]);
    assert!(
        code == Some(1) && err.contains("E0112") && err.contains("wrong number"),
        "{err}"
    );
    std::fs::write(
        tmp.join("bad.yaml"),
        "exclude:\n  - kind: fsm\n    instance: a\n    signal: b\n",
    )
    .expect("write");
    let (code, _, err) = report(&["--exclude", "bad.yaml"]);
    assert!(code == Some(1) && err.contains("needs `item`"), "{err}");

    // The HTML report shows them, and Cobertura leaves them out.
    let (code, _, err) = report(&["--html", "h", "--cobertura", "c.xml"]);
    assert_eq!(code, Some(0), "{err}");
    let index = std::fs::read_to_string(tmp.join("h/index.html")).expect("index");
    assert!(index.contains("<th>fsm</th>"), "{index}");
    let page = std::fs::read_dir(tmp.join("h"))
        .expect("dir")
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .collect::<String>();
    assert!(page.contains("transition one: busy -&gt; done"), "html");
    assert!(
        !std::fs::read_to_string(tmp.join("c.xml"))
            .expect("xml")
            .contains("spare")
    );
}

/// A database of the version before state machines still reads; `--kind=fsm` on one without any
/// is E0115, and `--cov-kind` off counts nothing at all.
#[test]
fn fsm_old_databases_and_no_items() {
    let tmp = fsm_setup("meas_fsm_old");
    let o = yavs(&tmp, &["run", "--work=work:lib", "cov_fsm", "--cov=a.ycov"]);
    assert!(o.status.success());
    assert!(fsm_items(&read(&tmp.join("a.ycov"))).is_empty());
    assert!(read(&tmp.join("a.ycov")).unmeasured.is_empty());
    let o = yavs(&tmp, &["cov", "report", "a.ycov", "--kind=fsm", "--total"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("E0115"));
    let text = std::fs::read_to_string(tmp.join("a.ycov")).expect("read");
    let (head, rest) = text.split_once('\n').expect("head");
    assert_eq!(head, "yavs-coverage 10");
    let old = Database::parse(&format!("yavs-coverage 9\n{rest}")).expect("parse");
    assert!(!old.legacy);
    assert!(Database::parse(&format!("yavs-coverage 99\n{rest}")).is_err());
}
