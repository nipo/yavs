//! A wave layout: the signal list of a viewer, one dotted name per line. `#` starts a comment.

use std::path::Path;

const HEAD: &str = "# yavs wave layout: one signal, variable or scope per line\n";

/// Write `items`; how many.
pub fn save(file: &Path, items: &[String]) -> Result<usize, String> {
    if let Some(d) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot create `{}`: {e}", d.display()))?;
    }
    let body: String = items.iter().map(|i| format!("{i}\n")).collect();
    std::fs::write(file, format!("{HEAD}{body}"))
        .map_err(|e| format!("cannot write `{}`: {e}", file.display()))?;
    Ok(items.len())
}

/// The items of `file`, split by `known`: (those that exist now, those that do not).
pub fn load(
    file: &Path,
    known: &dyn Fn(&str) -> bool,
) -> Result<(Vec<String>, Vec<String>), String> {
    let text = std::fs::read_to_string(file).map_err(|e| {
        format!(
            "cannot read the wave layout `{}`: {e}; `wave save` writes one",
            file.display()
        )
    })?;
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from);
    Ok(lines.partition(|l| known(l)))
}
