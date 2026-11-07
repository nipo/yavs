//! `--threads=auto` (`docs/parallel.md`): the most threads it uses, and the memory of what the
//! last run of the same command line found, in the work library's compilation cache. The kernel
//! decides, and uses no more threads than the CPUs the run may use.

use yavs_cache::{Key, KeyBuilder, Store};

/// The most threads `auto` uses: the gain of the designs measured flattens by 8 (`docs/parallel.md`).
// ponytail: a fixed cap; measure designs on more cores before raising it.
pub(crate) const MAX: usize = 8;

const KIND: &str = "thr";

/// The memory of one command line: `None` with `--no-cache` or without a work directory.
pub(crate) struct Memory {
    store: Store,
    key: Key,
    /// What the last run found: it gained, or it did not.
    pub(crate) hint: Option<bool>,
}

impl Memory {
    pub(crate) fn open(work: &str, no_cache: bool, run_id: &str) -> Option<Memory> {
        let dir = work.split_once(':').map_or("", |(_, d)| d);
        if no_cache || dir.is_empty() {
            return None;
        }
        let store = Store::of_library(std::path::Path::new(dir));
        let mut k = KeyBuilder::new(KIND);
        k.text(run_id);
        let key = k.finish();
        let hint = store.get(KIND, &key).and_then(|e| match &e[..] {
            b"gained" => Some(true),
            b"no gain" => Some(false),
            _ => None,
        });
        Some(Memory { store, key, hint })
    }

    /// Keep what this run found, when it judged anything and that differs from the hint.
    pub(crate) fn save(&self, v: Option<yavs_kernel::AutoVerdict>) {
        let gained = match v {
            Some(yavs_kernel::AutoVerdict::Judged { gained, .. }) => gained > 0,
            _ => return,
        };
        if self.hint != Some(gained) {
            let text: &[u8] = if gained { b"gained" } else { b"no gain" };
            self.store.put(KIND, &self.key, text);
        }
    }
}

/// The `--verbose` note of what `auto` decided.
pub(crate) fn note(v: yavs_kernel::AutoVerdict, hint: Option<bool>) -> String {
    use yavs_kernel::AutoVerdict::*;
    let from = match hint {
        Some(true) => " (the last run gained)",
        Some(false) => " (the last run did not gain)",
        None => "",
    };
    match v {
        Static => {
            "note: --threads=auto: one thread: too few processes run in parallel, or one CPU".into()
        }
        Unjudged => {
            format!("note: --threads=auto: the run ended before the threads were judged{from}")
        }
        Judged { windows, gained } => format!(
            "note: --threads=auto: the threads gained in {gained} of {windows} windows judged{from}"
        ),
    }
}
