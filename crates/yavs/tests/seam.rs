//! The images a gather and a scatter keep of a whole record (#1050) change nothing: each design
//! is run with them from the first use (`YAVS_SEAM_AFTER=1`) and without them, on one and on four
//! threads, shuffled too, and the output must be byte for byte the same.

mod common;
use common::tempdir;

use std::path::Path;
use std::process::{Command, Output};

const OFF: &str = "4294967295";

fn yavs(dir: &Path, after: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yavs"))
        .current_dir(dir)
        .env("YAVS_SEAM_AFTER", after)
        .args(["--color", "never"])
        .args(args)
        .output()
        .expect("run yavs")
}

#[test]
fn images_change_nothing() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/composite");
    for design in [
        "sigrec_seam",
        "sigrec_seam_array",
        "sigrec_whole",
        "sigrec_arrays_scatter",
    ] {
        let d = tempdir(design);
        let file = corpus.join(format!("{design}.vhd"));
        let work = format!("work:{}", d.join("lib").display());
        let a = yavs(
            &d,
            OFF,
            &["analyze", "--work", &work, file.to_str().unwrap()],
        );
        assert!(a.status.success(), "analyze {design}: {a:?}");
        let (wa, wb) = (d.join("off.fst"), d.join("on.fst"));
        for (after, f) in [(OFF, &wa), ("1", &wb)] {
            let wave = format!("--wave={}", f.display());
            let r = yavs(
                &d,
                after,
                &[
                    "run",
                    "--work",
                    &work,
                    "--tier=interp",
                    "--no-cache",
                    &wave,
                    design,
                ],
            );
            assert!(r.status.code().is_some(), "{design} waves: {r:?}");
        }
        assert_eq!(
            std::fs::read(&wa).unwrap(),
            std::fs::read(&wb).unwrap(),
            "{design}: waves"
        );
        for extra in [
            &["--threads", "1"][..],
            &["--threads", "4"],
            &["--threads", "4", "--shuffle", "7"],
        ] {
            let mut args = vec!["run", "--work", &work, "--tier=interp", "--no-cache"];
            args.extend_from_slice(extra);
            args.push(design);
            let (off, on) = (yavs(&d, OFF, &args), yavs(&d, "1", &args));
            assert_eq!(
                off.status.code(),
                on.status.code(),
                "{design} {extra:?}: exit status"
            );
            assert_eq!(off.stdout, on.stdout, "{design} {extra:?}: stdout");
            assert_eq!(off.stderr, on.stderr, "{design} {extra:?}: stderr");
        }
    }
}
