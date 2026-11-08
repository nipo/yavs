//! Records whose fields nothing reads one element of keep no byte codes: record packs, arrays of
//! records read element by element through a run-time index, a forced field, a field waited for.
//! The report, the events and the wave file are those of a run that keeps every signal's codes
//! (`YAVS_CODES=all`), on four threads and under each compiled tier the build has.

mod common;

use std::path::Path;
use std::process::Command;

use common::tempdir;

/// Design, source, what it reports and how many signals keep no codes.
const DESIGNS: [(&str, &str, &str, u64); 2] = [
    (
        "record_codes",
        include_str!("perf/record_codes.vhd"),
        "n=300 s=32FE4C p3=054FE0558C a=FC9F5FAA9D m='0'18 f=F828BF5B97 h=0004BF7C2A w=2850",
        // `p0` to `p3`, `f`, `h`, the vector fields of `a`, `sum` and `q`; `m` keeps its codes for
        // its scalar field, and `e` for its read of one element.
        10,
    ),
    (
        "record_codes_force",
        include_str!("perf/record_codes_force.vhd"),
        "n=300 s=00AE3D f=FACA4FAC2A g=FAD55FACA3",
        // The fields of `f` and `g`, and `sum`.
        5,
    ),
];

/// The report line, the count of events and the wave file of a run, and its count of signals
/// without codes.
type Run = ((String, u64, Vec<u8>), u64);

/// With `watch`, counters and a wave file; without, neither (no bytes): a signal nothing asks the
/// history of is then compared and copied whole, not word by word.
fn run(dir: &Path, top: &str, flags: &[&str], codes: &str, watch: bool) -> Run {
    let wave = dir.join(format!("{top}.fst"));
    let mut c = Command::new(env!("CARGO_BIN_EXE_yavs"));
    c.args([
        "--color",
        "never",
        "run",
        "--work",
        "work:lib",
        "--no-cache",
    ]);
    match watch {
        true => c.env("YAVS_COUNTERS", "1").arg("--wave").arg(&wave),
        false => c.env_remove("YAVS_COUNTERS"),
    };
    let o = c
        .current_dir(dir)
        .env("YAVS_CODES", codes)
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
    let bytes = match watch {
        true => std::fs::read(&wave).expect("wave file"),
        false => Vec::new(),
    };
    (
        (line.to_string(), counter("events"), bytes),
        counter("no_code_signals"),
    )
}

#[test]
fn records_without_byte_codes_match_records_with_them() {
    let d = tempdir("record_codes");
    for (top, src, want, n) in DESIGNS {
        let file = format!("{top}.vhd");
        std::fs::write(d.join(&file), src).expect("write");
        let a = Command::new(env!("CARGO_BIN_EXE_yavs"))
            .current_dir(&*d)
            .args(["analyze", "--std=2008", "--work", "work:lib", &file])
            .output()
            .expect("analyze");
        assert!(a.status.success(), "{top}: {a:?}");
        let (all, none) = run(&d, top, &["--tier=interp"], "all", true);
        assert!(all.0.contains(want), "{top}: {}", all.0);
        assert_eq!(none, 0, "{top}");
        for flags in [
            vec!["--tier=interp"],
            vec!["--tier=interp", "--threads", "4"],
            #[cfg(feature = "clift")]
            vec!["--tier=clift"],
            #[cfg(feature = "clift")]
            vec!["--tier=clift", "--threads", "4"],
            #[cfg(feature = "clift")]
            vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
        ] {
            let (got, k) = run(&d, top, &flags, "", true);
            assert!(
                got == all,
                "{top} {flags:?}: {} {} against {} {}",
                got.0,
                got.1,
                all.0,
                all.1
            );
            assert_eq!(k, n, "{top} {flags:?}");
            let (plain, _) = run(&d, top, &flags, "", false);
            assert!(
                plain.0 == all.0,
                "{top} {flags:?} unwatched: {} against {}",
                plain.0,
                all.0
            );
        }
    }
}
