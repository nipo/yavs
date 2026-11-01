//! Telling testbenches from hardware, for the `rtl:` and `testbench:` rule blocks. What a project
//! lists decides first, then the code and the name of the file: an inference, documented as one.

use crate::config::{Config, Kind};
use std::path::Path;
use yavs_diag::FileId;
use yavs_sem::hir::UnitHir;
use yavs_sem::{Db, UnitId};

/// The kind of a file, or `None` when the configuration has no block to choose.
pub(crate) fn kind(
    db: &Db,
    file: FileId,
    units: &[UnitId],
    cfg: &Config,
    base: &Path,
) -> Option<Kind> {
    if !cfg.has_kinds() {
        return None;
    }
    let path = db.sources.get(file).path.to_string();
    let abs = std::path::absolute(&path).unwrap_or_else(|_| path.clone().into());
    let mine = || {
        units
            .iter()
            .map(|&u| &db.units[u])
            .filter(|u| u.file == file)
    };
    let testbench = cfg.testbench_file(&abs, base)
        || mine().any(|u| cfg.testbench_library(db.libs[u.lib].name.key.as_str()))
        || code_says_so(db, file)
        || mine().any(|u| matches!(&u.hir, UnitHir::Entity { ports, .. } if ports.is_empty()))
        || named_like_one(&path);
    Some(if testbench {
        Kind::Testbench
    } else {
        Kind::Rtl
    })
}

/// The directive `-- yavs: testbench` near the top, or a verification library named outside comments.
fn code_says_so(db: &Db, file: FileId) -> bool {
    let Ok(text) = db.sources.get(file).try_text() else {
        return false;
    };
    let text = text.to_ascii_lowercase();
    let head: String = text.chars().take(2000).collect();
    if head.contains("-- yavs: testbench") {
        return true;
    }
    text.lines().any(|l| {
        let code = l.split("--").next().unwrap_or_default();
        ["vunit_lib", "osvvm", "uvvm_util", "runner_cfg", "bitvis"]
            .iter()
            .any(|w| code.contains(w))
    })
}

/// The directories and file names that say testbench by convention.
fn named_like_one(path: &str) -> bool {
    let text = path.replace('\\', "/").to_ascii_lowercase();
    let name = text.rsplit('/').next().unwrap_or(&text);
    let stem = name.split('.').next().unwrap_or(name);
    stem.starts_with("tb_")
        || stem.ends_with("_tb")
        || text.split('/').any(|part| {
            matches!(
                part,
                "test"
                    | "tests"
                    | "tb"
                    | "testbench"
                    | "testbenches"
                    | "sim"
                    | "bench"
                    | "verification"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::named_like_one;

    #[test]
    fn names_and_directories_count() {
        assert!(named_like_one("test/adder.vhd"));
        assert!(named_like_one("src/tb_adder.vhd"));
        assert!(named_like_one("src/adder_tb.vhd"));
        assert!(!named_like_one("src/testbed_helper.vhd"));
    }
}
