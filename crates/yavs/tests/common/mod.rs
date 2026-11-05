//! Shared by the integration tests.

use std::ops::Deref;
use std::path::Path;

/// A command as program and arguments. Never print a `Command` with `{:?}`: it lists the
/// environment it was given too, and a test log is public.
#[allow(dead_code)]
pub fn command_line(c: &std::process::Command) -> String {
    std::iter::once(c.get_program())
        .chain(c.get_args())
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A scratch directory under the system temp dir, removed when dropped (also when the test
/// panics). `name` only makes a leftover after a killed run recognisable.
pub struct Tmp(tempfile::TempDir);

/// Made in the resolved temp directory, so a path the test builds is spelled
/// as the one yavs reports from its working directory (macOS's temp dir is
/// a symlink into `/private`). Not on Windows, where resolving adds `\\?\`.
pub fn tempdir(name: &str) -> Tmp {
    let base = std::env::temp_dir();
    #[cfg(unix)]
    let base = std::fs::canonicalize(&base).expect("temp dir");
    Tmp(tempfile::Builder::new()
        .prefix(&format!("yavs-{name}-"))
        .tempdir_in(base)
        .expect("tmp"))
}

impl Deref for Tmp {
    type Target = Path;
    fn deref(&self) -> &Path {
        self.0.path()
    }
}

impl AsRef<Path> for Tmp {
    fn as_ref(&self) -> &Path {
        self.0.path()
    }
}
