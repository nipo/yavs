//! What coverage measures beyond statements: the conditions of `wait until` and `assert`
//! (`tests/cli/cov_wait.vhd`, #1020), and that counting them changes nothing the design does.

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
    assert!(at(conc).iter().any(|r| r.2 > 0) && at(conc).iter().any(|r| r.2 == 0), "{all:#?}");
    assert!(at(seq).iter().any(|r| r.2 > 0) && at(seq).iter().any(|r| r.2 == 0), "{all:#?}");

    // Without the option there are no rows of an `assert`, with it alone no rows of a wait.
    let o = run(&["--cov=w.ycov", "--cov-kind=condition"]);
    assert_eq!(trace(&o), plain);
    let wait_only = rows(&read(&tmp.join("w.ycov")));
    assert!(wait_only.iter().any(|r| r.0 == edge_en));
    assert!(!wait_only.iter().any(|r| r.0 == conc || r.0 == seq), "{wait_only:#?}");
    let o = run(&["--cov=s.ycov", "--cov-kind=assert"]);
    assert_eq!(trace(&o), plain);
    let assert_only = rows(&read(&tmp.join("s.ycov")));
    assert!(assert_only.iter().any(|r| r.0 == conc));
    assert!(!assert_only.iter().any(|r| r.0 == edge_en), "{assert_only:#?}");
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
    assert!(yavs(&tmp, &["analyze", "--work=work:lib", "f.vhd"]).status.success());
    let o = yavs(&tmp, &["run", "--work=work:lib", "f", "--cov=a.ycov", "--cov-kind=assert"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let r = rows(&read(&tmp.join("a.ycov")));
    assert_eq!(
        r.iter().map(|x| (x.1, x.3)).collect::<Vec<_>>(),
        [(0, 1), (1, 0), (2, 1), (3, 1)],
        "{r:#?}"
    );
    let o = yavs(&tmp, &["run", "--work=work:lib", "f", "--cov=b.ycov", "--cov-kind=asert"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("assert"));
    assert!(!tmp.join("b.ycov").exists());
}
