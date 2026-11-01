//! How a pattern in a configuration or exclusion file names source files: the one matching of
//! `yavs lint`'s `exclude` and waivers and of coverage exclusion files. A pattern is relative to
//! the directory of the file holding it, may name a directory, and uses `*` and `?` (which stop
//! at `/`) and `**`.

use std::path::{Path, PathBuf};

/// A pattern as written, with `\` as `/` and no leading `./`.
pub fn normalize(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches("./").to_owned()
}

/// `*` and `?` stop at `/`, `**` does not.
pub fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern {
        [] => text.is_empty(),
        [b'*', b'*', rest @ ..] => {
            let rest = rest.strip_prefix(b"/").unwrap_or(rest);
            (0..=text.len()).any(|i| glob(rest, &text[i..]))
        }
        [b'*', rest @ ..] => (0..=text.len())
            .take_while(|&i| i == 0 || text[i - 1] != b'/')
            .any(|i| glob(rest, &text[i..])),
        [b'?', rest @ ..] => text.first().is_some_and(|&c| c != b'/') && glob(rest, &text[1..]),
        [c, rest @ ..] => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

/// `path` with `.` and `..` taken out, without asking the file system.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// A file as a pattern sees it.
pub struct Place {
    /// Relative to the directory of the file holding the pattern, when the file is under it.
    relative: Option<String>,
    pub absolute: String,
}

impl Place {
    pub fn new(file: &Path, base: &Path) -> Place {
        let (file, base) = (lexical(file), lexical(base));
        Place {
            relative: file
                .strip_prefix(&base)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/")),
            absolute: file.to_string_lossy().replace('\\', "/"),
        }
    }

    /// Whether the pattern names the file or a directory above it. A pattern is relative to the
    /// base; one that starts with `/` or `**` also matches the absolute path, for files outside.
    pub fn covered_by(&self, pattern: &str) -> bool {
        let pattern = pattern.trim_end_matches('/');
        let covers = |path: &str| {
            let mut upto = path;
            loop {
                if glob(pattern.as_bytes(), upto.as_bytes()) {
                    return true;
                }
                match upto.rfind('/') {
                    Some(i) if i > 0 => upto = &upto[..i],
                    _ => return false,
                }
            }
        };
        self.relative.as_deref().is_some_and(covers)
            || ((pattern.starts_with('/')
                || pattern.starts_with("**")
                || pattern.as_bytes().get(1) == Some(&b':'))
                && covers(&self.absolute))
    }
}

/// A file a `files` list may be about: as reported, and as a pattern sees it.
pub struct Found {
    reported: String,
    place: Place,
}

impl Found {
    pub fn new(file: &str, base: &Path) -> Found {
        let path = Path::new(file);
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        Found {
            reported: normalize(file),
            place: Place::new(&absolute, base),
        }
    }

    /// The pattern names the file as [`Place::covered_by`] says, or matches the path as reported,
    /// or is its last components (a bare file name under any directory).
    pub fn matches(&self, pattern: &str) -> bool {
        self.place.covered_by(pattern)
            || glob(pattern.as_bytes(), self.reported.as_bytes())
            || Path::new(&self.reported).ends_with(pattern)
    }

    /// What a pattern most likely means for this file: its path relative to the base, else as
    /// reported.
    pub fn shown(&self) -> &str {
        self.place.relative.as_deref().unwrap_or(&self.reported)
    }
}
