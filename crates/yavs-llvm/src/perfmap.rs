//! The perf map (`/tmp/perf-<pid>.map`, one `address size name` line per function of generated
//! code) both compiled tiers write for `perf report` (`YAVS_LLVM_PERFMAP`, `YAVS_PERFMAP`,
//! `YAVS_CLIFT_PERFMAP`). One append-only writer, so the tiers of a hybrid run (`--tier=auto3`)
//! and the code of several compiles share the file.

use std::io::Write;
use std::sync::Mutex;

/// Whether the LLVM tier lists its code (`YAVS_LLVM_PERFMAP`, or `YAVS_PERFMAP`, the older name).
pub fn llvm_requested() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var_os("YAVS_LLVM_PERFMAP").is_some()
            || std::env::var_os("YAVS_PERFMAP").is_some()
    })
}

/// The symbol of generated code: `f12_m.u1.worker` (`kind` `f` frame-mode code, `n` native-frame
/// code, `c` its `CallFn` adapter, then the function id and its `Program::label`). The id keeps
/// it unique; the label is cleaned for the map, which is one line of text per function.
pub fn symbol(kind: &str, id: u32, label: &str) -> String {
    let clean: String = label
        .chars()
        .filter(|&c| c != '"')
        .map(|c| {
            if c.is_whitespace() || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("{kind}{id}_{clean}")
}

/// Add the line of a function whose code is `size` bytes at `addr`.
pub fn append(addr: usize, size: usize, name: &str) {
    static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
    let mut f = FILE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if f.is_none() {
        let path = format!("/tmp/perf-{}.map", std::process::id());
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(file) => *f = Some(file),
            Err(e) => {
                static WARNED: std::sync::Once = std::sync::Once::new();
                WARNED.call_once(|| eprintln!("warning: cannot write the perf map {path}: {e}"));
            }
        }
    }
    if let Some(f) = f.as_mut() {
        let _ = writeln!(f, "{addr:x} {size:x} {name}");
    }
}
