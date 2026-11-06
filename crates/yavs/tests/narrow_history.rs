//! Narrow signals that nothing asks the history of keep none (#1111), and the ones that are asked
//! answer exactly: the same report with every history kept (`YAVS_LEAN_EVENTS=off`), on four
//! threads, and under each compiled tier the build has, whose `'event` and edge tests read the
//! event stamps of a narrow signal that is not lean; the same events and the same wave file.
//! Records too, through a field, an element or the whole (#1117).

mod common;

use std::path::Path;
use std::process::Command;

use common::tempdir;

const DESIGNS: [(&str, &str, &str); 3] = [
    (
        "narrow_history",
        include_str!("perf/narrow_history.vhd"),
        "n=254010 m=404000360 q=60199524 t=100000000 fs dev=7 ded=4 k=165",
    ),
    (
        "narrow_history_ext",
        include_str!("perf/narrow_history_ext.vhd"),
        "n=27 q=4006",
    ),
    (
        "record_history",
        include_str!("perf/record_history.vhd"),
        "n=37663 m=4165 g=2400 p=377 e=6060 o=1944",
    ),
];

/// What a run of `top` reports, its count of events and the bytes of its wave file, and its count
/// of lean signals.
type Run = ((String, u64, Vec<u8>), u64);

fn run(dir: &Path, top: &str, flags: &[&str], lean: &str) -> Run {
    let wave = dir.join(format!("{top}.fst"));
    let o = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .current_dir(dir)
        .env("YAVS_COUNTERS", "1")
        .env("YAVS_LEAN_EVENTS", lean)
        .args([
            "--color",
            "never",
            "run",
            "--work",
            "work:lib",
            "--no-cache",
        ])
        .arg("--wave")
        .arg(&wave)
        .args(flags)
        .arg(top)
        .output()
        .expect("run yavs");
    let (out, err) = (
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr),
    );
    assert!(o.status.success(), "{top} {flags:?}: {out}{err}");
    let line = out.lines().find(|l| l.contains("n=")).unwrap_or_default();
    let counter = |name: &str| {
        err.lines()
            .find_map(|l| l.strip_prefix(&format!("counters: {name}=")))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    let bytes = std::fs::read(&wave).expect("wave file");
    (
        (line.to_string(), counter("events"), bytes),
        counter("lean_signals"),
    )
}

fn tiers() -> Vec<Vec<&'static str>> {
    vec![
        vec!["--tier=interp", "--threads", "4"],
        #[cfg(feature = "llvm")]
        vec!["--tier=llvm"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift", "--threads", "4"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
    ]
}

#[test]
fn narrow_signals_keep_the_history_that_is_asked() {
    let d = tempdir("narrow_history");
    for (top, src, want) in DESIGNS {
        let file = format!("{top}.vhd");
        std::fs::write(d.join(&file), src).expect("write");
        let a = Command::new(env!("CARGO_BIN_EXE_yavs"))
            .current_dir(&*d)
            .args(["analyze", "--std=2008", "--work", "work:lib", &file])
            .output()
            .expect("analyze");
        assert!(a.status.success(), "{top}: {a:?}");
        let (full, none) = run(&d, top, &["--tier=interp"], "off");
        assert!(full.0.contains(want), "{top}: {}", full.0);
        assert_eq!(none, 0, "{top}");
        let (lean, n) = run(&d, top, &["--tier=interp"], "on");
        assert!(
            lean == full,
            "{top}: {} {} against {} {}",
            lean.0,
            lean.1,
            full.0,
            full.1
        );
        // Floors on the counts are in `perf.rs`.
        assert!(n >= 2, "{top}: {n} lean signals");
        for flags in tiers() {
            let (got, k) = run(&d, top, &flags, "on");
            assert!(
                got == full,
                "{top} {flags:?}: {} {} against {} {}",
                got.0,
                got.1,
                full.0,
                full.1
            );
            assert_eq!(k, n, "{top} {flags:?}");
        }
    }
}
