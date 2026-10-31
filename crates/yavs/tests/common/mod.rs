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

pub fn tempdir(name: &str) -> Tmp {
    Tmp(tempfile::Builder::new()
        .prefix(&format!("yavs-{name}-"))
        .tempdir()
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
