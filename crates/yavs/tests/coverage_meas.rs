//! What coverage measures beyond statements: the conditions of `wait until` and `assert`
//! (`tests/cli/cov_wait.vhd`, #1020), and that counting them changes nothing the design does.

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
